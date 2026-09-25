use crate::{
    audio::ffmpeg,
    db::Database,
    error::{AppError, AppResult},
    models::{BookExportBlocker, BookExportPreflight, ExportState},
    services::{audio_service, book_service, pronunciation_service, text_service},
    AppState,
};
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, ExitStatus},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use uuid::Uuid;

const STDERR_LIMIT: usize = 8 * 1024;
const EXPORT_FORMATS: &[&str] = &["mp3", "wav"];

#[derive(Clone)]
struct ExportSegment {
    id: String,
    chapter_title: Option<String>,
    order_index: i64,
    original_text: String,
    reading_text: Option<String>,
    status: String,
    current_audio_id: Option<String>,
    audio_path: PathBuf,
}

impl ExportSegment {
    fn effective_text(&self) -> &str {
        self.reading_text
            .as_deref()
            .unwrap_or(self.original_text.as_str())
    }
}

#[derive(Clone, PartialEq)]
struct AudioSignature {
    provider: String,
    voice_type: i64,
    sample_rate: i64,
    codec: String,
    speed: f64,
    volume: f64,
}

struct RunningChild {
    child: Child,
    stderr: Option<JoinHandle<Vec<u8>>>,
}

pub struct ExportController {
    inner: Mutex<ExportInner>,
}

struct ExportInner {
    state: ExportState,
    cancel_requested: bool,
    child: Option<RunningChild>,
}

impl ExportController {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ExportInner {
                state: idle_state(),
                cancel_requested: false,
                child: None,
            }),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| matches!(inner.state.status.as_str(), "running" | "cancelling"))
            .unwrap_or(true)
    }

    pub fn snapshot(&self) -> ExportState {
        self.inner
            .lock()
            .map(|inner| inner.state.clone())
            .unwrap_or_else(|_| ExportState {
                status: "failed".to_string(),
                phase: "state".to_string(),
                error_code: Some("EXPORT_STATE_ERROR".to_string()),
                error_message: Some("导出状态锁不可用".to_string()),
                ..idle_state()
            })
    }

    fn reserve(&self, book_id: &str, format: &str) -> AppResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        if matches!(inner.state.status.as_str(), "running" | "cancelling") {
            return Err(AppError::new(
                "EXPORT_ALREADY_RUNNING",
                "已有一个完整音频导出正在运行",
            ));
        }
        inner.state = idle_state();
        inner.state.book_id = Some(book_id.to_string());
        inner.state.format = Some(format.to_string());
        inner.state.status = "running".to_string();
        inner.state.phase = "preparing".to_string();
        inner.cancel_requested = false;
        inner.child = None;
        Ok(())
    }

    fn reset(&self) -> AppResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        inner.state = idle_state();
        inner.cancel_requested = false;
        inner.child = None;
        Ok(())
    }

    fn request_cancel(&self) -> AppResult<ExportState> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        if inner.state.status == "running" {
            inner.state.status = "cancelling".to_string();
            inner.state.phase = "cancelling".to_string();
            inner.cancel_requested = true;
            if let Some(running) = inner.child.as_mut() {
                let _ = running.child.kill();
            }
        }
        Ok(inner.state.clone())
    }

    fn cancelled(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.cancel_requested)
            .unwrap_or(true)
    }

    fn update<F>(&self, update: F) -> AppResult<ExportState>
    where
        F: FnOnce(&mut ExportState),
    {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        update(&mut inner.state);
        Ok(inner.state.clone())
    }

    fn install_child(&self, child: RunningChild) -> AppResult<bool> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        if inner.cancel_requested {
            let mut child = child.child;
            let _ = child.kill();
            let _ = child.wait();
            if let Some(reader) = child.stderr.take() {
                drop(reader);
            }
            return Ok(false);
        }
        inner.child = Some(child);
        Ok(true)
    }

    fn try_wait_child(&self) -> AppResult<Option<ExitStatus>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?;
        let Some(running) = inner.child.as_mut() else {
            return Ok(None);
        };
        running
            .child
            .try_wait()
            .map_err(|error| AppError::new("EXPORT_FAILED", error.to_string()))
    }

    fn take_child(&self) -> AppResult<Option<RunningChild>> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| AppError::new("EXPORT_STATE_ERROR", "导出状态锁不可用"))?
            .child
            .take())
    }
}

impl Default for ExportController {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn get_book_export_preflight(
    database: &Database,
    book_id: &str,
) -> AppResult<BookExportPreflight> {
    let stats = book_service::get_book_text_stats(database, book_id).await?;
    let segments = load_export_segments(database, book_id).await?;
    let mut blockers = Vec::new();
    let mut generated_segments = 0_i64;
    let mut signature: Option<AudioSignature> = None;
    let mut wav_format: Option<audio_service::WavFormat> = None;
    let mut first_voice = None;
    let mut first_sample_rate = None;
    let mut first_channels = None;
    let mut first_bits = None;
    let mut first_speed = None;
    let mut first_volume = None;

    if stats.han_character_count as usize > text_service::MAX_BOOK_HAN_CHARACTERS {
        blockers.push(BookExportBlocker {
            code: "PROJECT_TEXT_LIMIT_EXCEEDED".to_string(),
            message: format!(
                "当前 Book 包含 {} 个汉字，超过 {} 个汉字的全文导出上限",
                stats.han_character_count,
                text_service::MAX_BOOK_HAN_CHARACTERS
            ),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }
    if segments.is_empty() {
        blockers.push(BookExportBlocker {
            code: "BOOK_EMPTY".to_string(),
            message: "当前 Book 没有可导出的 Segment".to_string(),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }

    for segment in &segments {
        if segment.status != "generated" {
            let (code, message) = match segment.status.as_str() {
                "ready" => (
                    "EXPORT_AUDIO_STALE",
                    "发音修改后当前 Segment 音频已过期，请重新生成",
                ),
                "pending" => ("EXPORT_AUDIO_NOT_READY", "当前 Segment 尚未进行发音分析"),
                "needs_review" => ("EXPORT_AUDIO_NOT_READY", "当前 Segment 仍有发音待确认"),
                _ => ("EXPORT_AUDIO_NOT_READY", "当前 Segment 尚未生成有效语音"),
            };
            blockers.push(segment_blocker(segment, code, message));
        } else {
            generated_segments += 1;
        }

        let Some(audio_id) = segment.current_audio_id.as_deref() else {
            blockers.push(segment_blocker(
                segment,
                "AUDIO_FILE_MISSING",
                "当前 Segment 没有 current_audio_id",
            ));
            continue;
        };
        let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, f64, f64, String)>(
            "SELECT id, segment_id, provider, voice_type, sample_rate, codec, speed, volume, audio_path
             FROM audio_versions WHERE id = ?",
        )
        .bind(audio_id)
        .fetch_optional(database.pool())
        .await?;
        let Some((
            id,
            audio_segment_id,
            provider,
            voice_type,
            sample_rate,
            codec,
            speed,
            volume,
            audio_path,
        )) = row
        else {
            blockers.push(segment_blocker(
                segment,
                "AUDIO_FILE_MISSING",
                "current_audio_id 对应的 AudioVersion 不存在",
            ));
            continue;
        };
        if id != audio_id || audio_segment_id != segment.id {
            blockers.push(segment_blocker(
                segment,
                "AUDIO_FORMAT_MISMATCH",
                "current AudioVersion 不属于当前 Segment",
            ));
            continue;
        }
        let path = Path::new(&audio_path);
        if !path.is_file() {
            blockers.push(segment_blocker(
                segment,
                "AUDIO_FILE_MISSING",
                "current AudioVersion 的 WAV 文件不存在",
            ));
            continue;
        }
        let actual_format = match audio_service::read_wav_format(path) {
            Ok(format) => format,
            Err(error) => {
                blockers.push(segment_blocker(
                    segment,
                    "AUDIO_FORMAT_MISMATCH",
                    error.message,
                ));
                continue;
            }
        };
        if codec != "wav" || sample_rate != actual_format.sample_rate as i64 {
            blockers.push(segment_blocker(
                segment,
                "AUDIO_FORMAT_MISMATCH",
                "AudioVersion 参数与 WAV 实际格式不一致",
            ));
        }
        let current_signature = AudioSignature {
            provider: provider.clone(),
            voice_type,
            sample_rate,
            codec: codec.clone(),
            speed,
            volume,
        };
        if let Some(expected) = signature.as_ref() {
            if expected != &current_signature {
                blockers.push(segment_blocker(
                    segment,
                    "EXPORT_AUDIO_SETTINGS_MISMATCH",
                    "当前文档中的语音片段使用了不同的语音设置",
                ));
            }
        } else {
            first_voice = Some(voice_type);
            first_sample_rate = Some(sample_rate);
            first_speed = Some(speed);
            first_volume = Some(volume);
            signature = Some(current_signature);
        }
        if let Some(expected) = wav_format.as_ref() {
            if expected != &actual_format {
                blockers.push(segment_blocker(
                    segment,
                    "AUDIO_FORMAT_MISMATCH",
                    "当前文档中的 WAV sample rate、声道或 sample format 不一致",
                ));
            }
        } else {
            first_channels = Some(actual_format.channels as i64);
            first_bits = Some(actual_format.bits_per_sample as i64);
            wav_format = Some(actual_format);
        }
    }

    Ok(BookExportPreflight {
        can_export: blockers.is_empty(),
        han_character_count: stats.han_character_count,
        total_segments: stats.segment_count,
        generated_segments,
        provider: signature.as_ref().map(|value| value.provider.clone()),
        voice_type: first_voice,
        sample_rate: first_sample_rate,
        channels: first_channels,
        bits_per_sample: first_bits,
        speed: first_speed,
        volume: first_volume,
        blockers,
    })
}

pub async fn start_export(
    app: AppHandle,
    controller: Arc<ExportController>,
    database: Database,
    book_id: String,
    destination_path: String,
    format: String,
    overwrite: bool,
) -> AppResult<()> {
    let format = validate_format(&format)?;
    let destination = normalize_destination(&destination_path, format)?;
    if destination.as_os_str().is_empty()
        || destination.to_string_lossy().contains('\n')
        || destination.to_string_lossy().contains('\r')
    {
        return Err(AppError::new("INVALID_EXPORT_PATH", "导出路径无效"));
    }
    if destination.exists() && !overwrite {
        return Err(AppError::new(
            "EXPORT_OUTPUT_EXISTS",
            "目标文件已存在，请确认覆盖后重试",
        ));
    }
    controller.reserve(&book_id, format)?;
    let preflight = match get_book_export_preflight(&database, &book_id).await {
        Ok(value) => value,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    if !preflight.can_export {
        controller.reset()?;
        return Err(AppError::new(
            "EXPORT_PREFLIGHT_BLOCKED",
            preflight
                .blockers
                .first()
                .map(|blocker| blocker.message.clone())
                .unwrap_or_else(|| "导出预检未通过".to_string()),
        ));
    }
    let ffmpeg_info = match ffmpeg::check_available(&app, format == "mp3") {
        Ok(info) => info,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    let segments = match load_export_segments(&database, &book_id).await {
        Ok(value) => value,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    let book_title = match book_title_for_metadata(&database, &book_id).await {
        Ok(value) => value,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    let data_dir = app
        .state::<AppState>()
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
        .clone()
        .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用目录尚未初始化"))?;
    let export_dir = data_dir
        .join("cache")
        .join("exports")
        .join(Uuid::now_v7().to_string());
    if let Err(error) = fs::create_dir_all(&export_dir) {
        controller.reset()?;
        return Err(AppError::from(error));
    }
    let result = run_export(
        &app,
        &controller,
        ffmpeg_info,
        segments,
        &export_dir,
        &destination,
        format,
        overwrite,
        &book_title,
        preflight.sample_rate.unwrap_or(16000),
        preflight.channels.unwrap_or(1),
        preflight.bits_per_sample.unwrap_or(16),
    )
    .await;
    let _ = fs::remove_dir_all(&export_dir);
    clear_tts_busy(&app);
    result
}

fn clear_tts_busy(app: &AppHandle) {
    if let Ok(mut busy) = app.state::<AppState>().tts_busy.lock() {
        *busy = false;
    }
}

async fn book_title_for_metadata(database: &Database, book_id: &str) -> AppResult<String> {
    sqlx::query_scalar::<_, String>("SELECT title FROM books WHERE id = ?")
        .bind(book_id)
        .fetch_optional(database.pool())
        .await?
        .ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "Book 不存在"))
}

pub fn cancel_export(app: &AppHandle, controller: &ExportController) -> AppResult<ExportState> {
    let state = controller.request_cancel()?;
    emit_state(app, controller);
    Ok(state)
}

pub fn get_export_state(controller: &ExportController) -> ExportState {
    controller.snapshot()
}

async fn run_export(
    app: &AppHandle,
    controller: &ExportController,
    ffmpeg_info: ffmpeg::FfmpegInfo,
    segments: Vec<ExportSegment>,
    export_dir: &Path,
    destination: &Path,
    format: &str,
    overwrite: bool,
    book_title: &str,
    sample_rate: i64,
    channels: i64,
    bits_per_sample: i64,
) -> AppResult<()> {
    let result = async {
        let concat_path = export_dir.join("concat.txt");
        let merged_wav = export_dir.join("merged.wav");
        let output_path = export_dir.join(format!("output.{format}"));
        let concat = build_concat_list(&segments)?;
        fs::write(&concat_path, concat).map_err(AppError::from)?;
        set_phase(controller, app, "merging", segments.len() as i64);
        let merge_args = vec![
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-y".to_string(),
            "-f".to_string(),
            "concat".to_string(),
            "-safe".to_string(),
            "0".to_string(),
            "-i".to_string(),
            concat_path.to_string_lossy().to_string(),
            "-c".to_string(),
            "copy".to_string(),
            merged_wav.to_string_lossy().to_string(),
        ];
        run_ffmpeg_stage(controller, &ffmpeg_info.path, &merge_args).await?;
        let merged_format = audio_service::read_wav_format(&merged_wav)?;
        if merged_format.sample_rate as i64 != sample_rate
            || merged_format.channels as i64 != channels
            || merged_format.bits_per_sample as i64 != bits_per_sample
        {
            return Err(AppError::new(
                "AUDIO_FORMAT_MISMATCH",
                "FFmpeg 合并后的 WAV 参数与输入不一致",
            ));
        }
        let final_source = if format == "mp3" {
            set_phase(controller, app, "encoding_mp3", segments.len() as i64);
            let encode_args = vec![
                "-hide_banner".to_string(),
                "-loglevel".to_string(),
                "error".to_string(),
                "-y".to_string(),
                "-i".to_string(),
                merged_wav.to_string_lossy().to_string(),
                "-vn".to_string(),
                "-codec:a".to_string(),
                "libmp3lame".to_string(),
                "-b:a".to_string(),
                "96k".to_string(),
                "-ar".to_string(),
                sample_rate.to_string(),
                "-ac".to_string(),
                channels.to_string(),
                "-metadata".to_string(),
                format!("title={book_title}"),
                output_path.to_string_lossy().to_string(),
            ];
            run_ffmpeg_stage(controller, &ffmpeg_info.path, &encode_args).await?;
            output_path
        } else {
            merged_wav
        };
        let metadata = fs::metadata(&final_source)
            .map_err(|error| AppError::new("EXPORT_FAILED", format!("导出文件不存在: {error}")))?;
        if metadata.len() == 0 {
            return Err(AppError::new("EXPORT_FAILED", "FFmpeg 输出文件为空"));
        }
        set_phase(controller, app, "saving", segments.len() as i64);
        if destination.exists() {
            if !overwrite {
                return Err(AppError::new(
                    "EXPORT_OUTPUT_EXISTS",
                    "目标文件已存在，请确认覆盖后重试",
                ));
            }
            fs::remove_file(destination).map_err(AppError::from)?;
        }
        fs::copy(&final_source, destination).map_err(|error| {
            AppError::new("FILE_IO_ERROR", format!("导出文件保存失败: {error}"))
        })?;
        fs::remove_file(&final_source).map_err(|error| {
            AppError::new("FILE_IO_ERROR", format!("临时导出文件清理失败: {error}"))
        })?;
        Ok::<(), AppError>(())
    }
    .await;

    match result {
        Ok(()) => {
            let _ = controller.update(|state| {
                state.status = "completed".to_string();
                state.phase = "completed".to_string();
                state.processed_segments = state.total_segments;
                state.output_path = Some(destination.to_string_lossy().to_string());
            });
            emit_state(app, controller);
            Ok(())
        }
        Err(error) => {
            let cancelled = error.code == "EXPORT_CANCELLED" || controller.cancelled();
            let _ = controller.update(|state| {
                state.status = if cancelled { "cancelled" } else { "failed" }.to_string();
                state.phase = if cancelled { "cancelled" } else { "failed" }.to_string();
                state.error_code = Some(if cancelled {
                    "EXPORT_CANCELLED".to_string()
                } else {
                    error.code.clone()
                });
                state.error_message = Some(if cancelled {
                    "导出已取消".to_string()
                } else {
                    error.message.clone()
                });
            });
            emit_state(app, controller);
            if cancelled {
                Err(AppError::new("EXPORT_CANCELLED", "导出已取消"))
            } else {
                Err(error)
            }
        }
    }
}

async fn run_ffmpeg_stage(
    controller: &ExportController,
    path: &Path,
    args: &[String],
) -> AppResult<()> {
    if controller.cancelled() {
        return Err(AppError::new("EXPORT_CANCELLED", "导出已取消"));
    }
    let mut child = ffmpeg::spawn(path, args)?;
    let stderr_reader = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            let mut output = VecDeque::new();
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(size) => {
                        output.extend(&buffer[..size]);
                        while output.len() > STDERR_LIMIT {
                            output.pop_front();
                        }
                    }
                }
            }
            output.into_iter().collect()
        })
    });
    if !controller.install_child(RunningChild {
        child,
        stderr: stderr_reader,
    })? {
        return Err(AppError::new("EXPORT_CANCELLED", "导出已取消"));
    }
    loop {
        if let Some(status) = controller.try_wait_child()? {
            let mut running = controller
                .take_child()?
                .ok_or_else(|| AppError::new("EXPORT_STATE_ERROR", "FFmpeg 子进程状态丢失"))?;
            let _ = running.child.wait();
            let stderr = running
                .stderr
                .and_then(|reader| reader.join().ok())
                .unwrap_or_default();
            if controller.cancelled() {
                return Err(AppError::new("EXPORT_CANCELLED", "导出已取消"));
            }
            if !status.success() {
                return Err(AppError::new(
                    "EXPORT_FAILED",
                    format!("FFmpeg 导出失败: {}", stderr_summary(&stderr)),
                ));
            }
            return Ok(());
        }
        if controller.cancelled() {
            if let Ok(mut inner) = controller.inner.lock() {
                if let Some(running) = inner.child.as_mut() {
                    let _ = running.child.kill();
                }
            }
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn load_export_segments(database: &Database, book_id: &str) -> AppResult<Vec<ExportSegment>> {
    let rows = sqlx::query_as::<_, (String, Option<String>, i64, String, Option<String>, i64, String, Option<String>)>(
        "SELECT s.id, c.title, s.order_index, s.original_text, s.reading_text, s.speak_enabled, s.status, s.current_audio_id
         FROM segments s JOIN chapters c ON c.id = s.chapter_id
         WHERE c.book_id = ? AND s.status <> 'superseded' AND s.speak_enabled = 1
         ORDER BY c.order_index ASC, s.order_index ASC",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;
    let mut segments = Vec::with_capacity(rows.len());
    for (
        id,
        chapter_title,
        order_index,
        original_text,
        reading_text,
        _speak_enabled,
        status,
        current_audio_id,
    ) in rows
    {
        let audio_path: Option<String> = match current_audio_id {
            Some(ref audio_id) => {
                sqlx::query_scalar(
                    "SELECT audio_path FROM audio_versions WHERE id = ? AND segment_id = ?",
                )
                .bind(audio_id)
                .bind(&id)
                .fetch_optional(database.pool())
                .await?
            }
            None => None,
        };
        if let Some(audio_path) = audio_path {
            segments.push(ExportSegment {
                id,
                chapter_title,
                order_index,
                original_text,
                reading_text,
                status,
                current_audio_id,
                audio_path: PathBuf::from(audio_path),
            });
        } else {
            segments.push(ExportSegment {
                id,
                chapter_title,
                order_index,
                original_text,
                reading_text,
                status,
                current_audio_id,
                audio_path: PathBuf::new(),
            });
        }
        let _ = status;
    }
    Ok(segments)
}

fn build_concat_list(segments: &[ExportSegment]) -> AppResult<String> {
    let mut result = String::from("ffconcat version 1.0\n");
    for segment in segments {
        if segment.audio_path.as_os_str().is_empty() {
            return Err(AppError::new(
                "AUDIO_FILE_MISSING",
                format!("Segment {} 没有可用 current WAV", segment.id),
            ));
        }
        let path = segment.audio_path.to_string_lossy().replace('\\', "/");
        if path.contains('\n') || path.contains('\r') {
            return Err(AppError::new(
                "INVALID_EXPORT_PATH",
                "音频路径包含换行符，无法安全写入 concat list",
            ));
        }
        let escaped = path.replace('\'', "'\\''");
        result.push_str("file '");
        result.push_str(&escaped);
        result.push_str("'\n");
    }
    Ok(result)
}

fn segment_blocker(
    segment: &ExportSegment,
    code: &str,
    message: impl Into<String>,
) -> BookExportBlocker {
    BookExportBlocker {
        code: code.to_string(),
        message: message.into(),
        segment_id: Some(segment.id.clone()),
        chapter_title: segment.chapter_title.clone(),
        segment_order: Some(segment.order_index),
        preview: Some(preview(segment.effective_text())),
    }
}

fn preview(text: &str) -> String {
    pronunciation_service::grapheme_tokens(text)
        .into_iter()
        .take(32)
        .map(|token| token.text)
        .collect()
}

fn set_phase(controller: &ExportController, app: &AppHandle, phase: &str, total: i64) {
    let _ = controller.update(|state| {
        state.phase = phase.to_string();
        state.total_segments = total;
    });
    emit_state(app, controller);
}

fn emit_state(app: &AppHandle, controller: &ExportController) {
    let _ = app.emit("export-progress", controller.snapshot());
}

fn validate_format(format: &str) -> AppResult<&'static str> {
    if EXPORT_FORMATS.contains(&format) {
        Ok(if format == "mp3" { "mp3" } else { "wav" })
    } else {
        Err(AppError::new(
            "INVALID_EXPORT_FORMAT",
            "导出格式只支持 MP3 或 WAV",
        ))
    }
}

fn normalize_destination(path: &str, format: &str) -> AppResult<PathBuf> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err(AppError::new("INVALID_EXPORT_PATH", "导出路径不能为空"));
    }
    let mut destination = path;
    destination.set_extension(format);
    Ok(destination)
}

fn stderr_summary(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.len() > STDERR_LIMIT {
        text[text.len() - STDERR_LIMIT..].to_string()
    } else {
        text
    }
}

fn idle_state() -> ExportState {
    ExportState {
        book_id: None,
        status: "idle".to_string(),
        format: None,
        phase: "idle".to_string(),
        processed_segments: 0,
        total_segments: 0,
        output_path: None,
        error_code: None,
        error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_concat_list, normalize_destination, ExportSegment};
    use crate::db::Database;
    use crate::services::audio_service::read_wav_format;
    use std::{fs, path::PathBuf, process::Command};

    #[test]
    fn concat_list_escapes_windows_separators_spaces_and_single_quotes() {
        let result = build_concat_list(&[ExportSegment {
            id: "segment".to_string(),
            chapter_title: Some("Chapter".to_string()),
            order_index: 0,
            original_text: "腧穴".to_string(),
            reading_text: None,
            status: "generated".to_string(),
            current_audio_id: Some("audio".to_string()),
            audio_path: PathBuf::from("C:\\Users\\测试\\Sean's Book\\001.wav"),
        }])
        .expect("concat list");
        assert_eq!(
            result,
            "ffconcat version 1.0\nfile 'C:/Users/测试/Sean'\\''s Book/001.wav'\n"
        );
    }

    #[test]
    fn destination_extension_is_controlled_by_export_format() {
        assert_eq!(
            normalize_destination("/tmp/黄帝内经.wav", "mp3")
                .expect("destination")
                .extension()
                .and_then(|value| value.to_str()),
            Some("mp3")
        );
    }

    #[test]
    fn empty_concat_path_is_rejected() {
        let result = build_concat_list(&[ExportSegment {
            id: "segment".to_string(),
            chapter_title: None,
            order_index: 0,
            original_text: "甲".to_string(),
            reading_text: None,
            status: "generated".to_string(),
            current_audio_id: Some("audio".to_string()),
            audio_path: PathBuf::new(),
        }]);
        assert_eq!(result.expect_err("missing path").code, "AUDIO_FILE_MISSING");
    }

    #[test]
    fn ffmpeg_integration_concat_and_mp3_are_opt_in() {
        if std::env::var("RUN_FFMPEG_INTEGRATION").ok().as_deref() != Some("1") {
            return;
        }
        let ffmpeg = std::env::var_os("ANCIENT_MEDICAL_TTS_FFMPEG")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::split_paths(&std::env::var_os("PATH")?).find_map(|directory| {
                    let path = directory.join(if cfg!(windows) {
                        "ffmpeg.exe"
                    } else {
                        "ffmpeg"
                    });
                    path.is_file().then_some(path)
                })
            })
            .expect("FFmpeg integration requires ANCIENT_MEDICAL_TTS_FFMPEG or PATH");
        let temp =
            std::env::temp_dir().join(format!("ancient-tts-export-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&temp).expect("temp directory");
        let mut segments = Vec::new();
        for (index, frequency) in [440, 554, 659].into_iter().enumerate() {
            let path = temp.join(format!("{index}.wav"));
            let status = Command::new(&ffmpeg)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    &format!("sine=frequency={frequency}:duration=0.05"),
                    "-ac",
                    "1",
                    "-ar",
                    "16000",
                    "-c:a",
                    "pcm_s16le",
                ])
                .arg(&path)
                .status()
                .expect("start ffmpeg tone");
            assert!(status.success(), "tone generation failed");
            segments.push(ExportSegment {
                id: index.to_string(),
                chapter_title: None,
                order_index: index as i64,
                original_text: "测".to_string(),
                reading_text: None,
                status: "generated".to_string(),
                current_audio_id: Some(index.to_string()),
                audio_path: path,
            });
        }
        let concat_path = temp.join("concat.txt");
        std::fs::write(
            &concat_path,
            build_concat_list(&segments).expect("concat list"),
        )
        .expect("write concat list");
        let merged = temp.join("merged.wav");
        let status = Command::new(&ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
            ])
            .arg(&concat_path)
            .args(["-c", "copy"])
            .arg(&merged)
            .status()
            .expect("start ffmpeg concat");
        assert!(status.success(), "concat failed");
        assert!(read_wav_format(&merged).is_ok());
        let mp3 = temp.join("output.mp3");
        let status = Command::new(&ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&merged)
            .args([
                "-codec:a",
                "libmp3lame",
                "-b:a",
                "96k",
                "-ar",
                "16000",
                "-ac",
                "1",
            ])
            .arg(&mp3)
            .status()
            .expect("start ffmpeg mp3");
        assert!(status.success(), "mp3 encoding failed");
        assert!(std::fs::metadata(&mp3).expect("mp3 metadata").len() > 0);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn export_preflight_requires_generated_current_audio_and_orders_segments() {
        let database_path = std::env::temp_dir().join(format!(
            "ancient-tts-preflight-{}.sqlite",
            uuid::Uuid::now_v7()
        ));
        let database = tauri::async_runtime::block_on(Database::open(&database_path)).expect("db");
        let book_id = uuid::Uuid::now_v7().to_string();
        let chapter_one = uuid::Uuid::now_v7().to_string();
        let chapter_two = uuid::Uuid::now_v7().to_string();
        let segments = [
            (
                uuid::Uuid::now_v7().to_string(),
                &chapter_two,
                0_i64,
                "第三",
            ),
            (
                uuid::Uuid::now_v7().to_string(),
                &chapter_one,
                1_i64,
                "第二",
            ),
            (
                uuid::Uuid::now_v7().to_string(),
                &chapter_one,
                0_i64,
                "第一",
            ),
        ];
        let audio_dir = std::env::temp_dir().join(format!(
            "ancient-tts-preflight-audio-{}",
            uuid::Uuid::now_v7()
        ));
        fs::create_dir_all(&audio_dir).expect("audio dir");
        tauri::async_runtime::block_on(async {
            let pool = database.pool();
            sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, '测试书', 'now', 'now')")
                .bind(&book_id).execute(pool).await.expect("book");
            for (chapter_id, order) in [(&chapter_two, 2_i64), (&chapter_one, 1_i64)] {
                sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, ?, ?, 'now', 'now')")
                    .bind(chapter_id).bind(&book_id).bind(format!("章{order}")).bind(order).execute(pool).await.expect("chapter");
            }
            for (segment_id, chapter_id, order, text) in &segments {
                let audio_id = uuid::Uuid::now_v7().to_string();
                let path = audio_dir.join(format!("{order}-{segment_id}.wav"));
                fs::write(&path, valid_wav_bytes()).expect("wav");
                sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, current_audio_id, created_at, updated_at) VALUES (?, ?, ?, ?, 'generated', ?, 'now', 'now')")
                    .bind(segment_id).bind(chapter_id).bind(order).bind(text).bind(&audio_id).execute(pool).await.expect("segment");
                sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, audio_path, created_at) VALUES (?, ?, 1, 'tencent', 501000, 16000, 'wav', 0, 0, ?, 'now')")
                    .bind(audio_id).bind(segment_id).bind(path.to_string_lossy().to_string()).execute(pool).await.expect("audio");
            }
            let preflight = super::get_book_export_preflight(&database, &book_id)
                .await
                .expect("preflight");
            assert!(preflight.can_export);
            assert_eq!(preflight.total_segments, 3);
            let ordered = super::load_export_segments(&database, &book_id)
                .await
                .expect("ordered segments");
            assert_eq!(
                ordered
                    .iter()
                    .map(|segment| segment.original_text.as_str())
                    .collect::<Vec<_>>(),
                vec!["第一", "第二", "第三"]
            );
            sqlx::query("UPDATE segments SET status = 'ready' WHERE id = ?")
                .bind(&segments[0].0)
                .execute(pool)
                .await
                .expect("stale");
            let stale = super::get_book_export_preflight(&database, &book_id)
                .await
                .expect("stale preflight");
            assert!(!stale.can_export);
            assert!(stale
                .blockers
                .iter()
                .any(|blocker| blocker.code == "EXPORT_AUDIO_STALE"));
            let missing_id = &segments[0].0;
            sqlx::query(
                "UPDATE segments SET status = 'generated', current_audio_id = NULL WHERE id = ?",
            )
            .bind(missing_id)
            .execute(pool)
            .await
            .expect("clear current audio");
            let missing = super::get_book_export_preflight(&database, &book_id)
                .await
                .expect("missing preflight");
            assert!(missing
                .blockers
                .iter()
                .any(|blocker| blocker.code == "AUDIO_FILE_MISSING"));
            let current_audio_id: String =
                sqlx::query_scalar("SELECT id FROM audio_versions WHERE segment_id = ?")
                    .bind(missing_id)
                    .fetch_one(pool)
                    .await
                    .expect("current audio id");
            sqlx::query("UPDATE segments SET current_audio_id = ? WHERE id = ?")
                .bind(current_audio_id)
                .bind(missing_id)
                .execute(pool)
                .await
                .expect("restore current audio");
            sqlx::query("UPDATE audio_versions SET speed = 1 WHERE segment_id = ?")
                .bind(&segments[1].0)
                .execute(pool)
                .await
                .expect("settings mismatch");
            let mismatch = super::get_book_export_preflight(&database, &book_id)
                .await
                .expect("mismatch preflight");
            assert!(mismatch
                .blockers
                .iter()
                .any(|blocker| blocker.code == "EXPORT_AUDIO_SETTINGS_MISMATCH"));
        });
        let _ = fs::remove_dir_all(audio_dir);
        let _ = fs::remove_file(database_path);
    }

    fn valid_wav_bytes() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(46);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(38_u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&(16_u32).to_le_bytes());
        bytes.extend_from_slice(&(1_u16).to_le_bytes());
        bytes.extend_from_slice(&(1_u16).to_le_bytes());
        bytes.extend_from_slice(&(16_000_u32).to_le_bytes());
        bytes.extend_from_slice(&(32_000_u32).to_le_bytes());
        bytes.extend_from_slice(&(2_u16).to_le_bytes());
        bytes.extend_from_slice(&(16_u16).to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(2_u32).to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes
    }
}
