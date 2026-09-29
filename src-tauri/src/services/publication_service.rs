use crate::{
    audio::ffmpeg,
    db::Database,
    error::{AppError, AppResult},
    models::{
        AudioProviderMetadata, PublicationBundleBlocker, PublicationBundlePreflight,
        PublicationExportState,
    },
    services::{audio_service, pronunciation_service},
    AppState,
};
use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const FORMAT: &str = "ancient-medical-publication-bundle";
const FORMAT_VERSION: &str = "1.0";
const MP3_TIMELINE_TOLERANCE_MS: i64 = 100;

pub struct PublicationExportController {
    inner: Mutex<PublicationExportState>,
}

impl PublicationExportController {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(idle_state()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner
            .lock()
            .map(|state| state.status == "running")
            .unwrap_or(true)
    }

    pub fn snapshot(&self) -> PublicationExportState {
        self.inner
            .lock()
            .map(|state| state.clone())
            .unwrap_or_else(|_| PublicationExportState {
                status: "failed".to_string(),
                phase: "failed".to_string(),
                error_code: Some("PUBLICATION_STATE_ERROR".to_string()),
                error_message: Some("发布包状态锁不可用".to_string()),
                ..idle_state()
            })
    }

    fn reserve(&self, book_id: &str, chapters_total: i64, segments_total: i64) -> AppResult<()> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| AppError::new("PUBLICATION_STATE_ERROR", "发布包状态锁不可用"))?;
        if state.status == "running" {
            return Err(AppError::new(
                "PUBLICATION_EXPORT_ALREADY_RUNNING",
                "已有一个移动端发布包正在导出",
            ));
        }
        *state = idle_state();
        state.book_id = Some(book_id.to_string());
        state.status = "running".to_string();
        state.phase = "preparing".to_string();
        state.chapters_total = chapters_total;
        state.segments_total = segments_total;
        Ok(())
    }

    fn update<F>(&self, update: F) -> AppResult<PublicationExportState>
    where
        F: FnOnce(&mut PublicationExportState),
    {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| AppError::new("PUBLICATION_STATE_ERROR", "发布包状态锁不可用"))?;
        update(&mut state);
        Ok(state.clone())
    }
}

impl Default for PublicationExportController {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct BookRow {
    id: String,
    title: String,
    author: Option<String>,
    dynasty: Option<String>,
    edition: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ChapterRow {
    id: String,
    title: Option<String>,
    collection: Option<String>,
    subtitle: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct SegmentRow {
    chapter_id: String,
    chapter_title: Option<String>,
    segment_id: String,
    original_text: String,
    reading_text: Option<String>,
    translation: Option<String>,
    speak_enabled: i64,
    status: String,
    current_audio_id: Option<String>,
    audio_version_id: Option<String>,
    audio_segment_id: Option<String>,
    provider: Option<String>,
    voice_type: Option<i64>,
    sample_rate: Option<i64>,
    codec: Option<String>,
    speed: Option<f64>,
    volume: Option<f64>,
    audio_path: Option<String>,
    provider_metadata_json: Option<String>,
}

#[derive(Clone)]
struct AudioInput {
    path: PathBuf,
    duration_ms: i64,
    provider: String,
    voice_type: i64,
    sample_rate: i64,
    codec: String,
    speed: f64,
    volume: f64,
    channels: i64,
    bits_per_sample: i64,
    tts_actual: Option<Vec<PublicationTtsActual>>,
}

#[derive(Clone)]
struct PublicationSegmentPlan {
    id: String,
    text: String,
    translation: Option<String>,
    speak_enabled: bool,
    audio: Option<AudioInput>,
    pronunciation: Option<PublicationPronunciation>,
}

#[derive(Clone)]
struct PublicationChapterPlan {
    id: String,
    title: Option<String>,
    collection: Option<String>,
    subtitle: Option<String>,
    segments: Vec<PublicationSegmentPlan>,
}

#[derive(Clone)]
struct PublicationPlan {
    book: BookRow,
    chapters: Vec<PublicationChapterPlan>,
    blockers: Vec<PublicationBundleBlocker>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationGenerator {
    name: String,
    version: String,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationManifest {
    format: String,
    format_version: String,
    generator: PublicationGenerator,
    generated_at: String,
    book: String,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationAudioFormat {
    format: String,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationBookChapter {
    id: String,
    order: i64,
    title: String,
    content: String,
    audio: String,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationBook {
    id: String,
    title: String,
    author: Option<String>,
    dynasty: Option<String>,
    edition: Option<String>,
    language: String,
    reading_mode: String,
    audio: PublicationAudioFormat,
    chapters: Vec<PublicationBookChapter>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationChapterAudio {
    src: String,
    duration_ms: i64,
    format: String,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationTtsActual {
    text: String,
    pinyin: Option<String>,
    begin_ms: Option<i64>,
    end_ms: Option<i64>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationPronunciation {
    tokens: Option<Vec<PublicationToken>>,
    tts_actual: Option<Vec<PublicationTtsActual>>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationToken {
    text: String,
    reference_pinyin: Option<String>,
    confirmed_pinyin: Option<String>,
    forced: bool,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationSegment {
    id: String,
    order: i64,
    text: String,
    translation: Option<String>,
    speak_enabled: bool,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    duration_ms: Option<i64>,
    pronunciation: Option<PublicationPronunciation>,
}

#[derive(Debug, Serialize, serde::Deserialize, Clone)]
struct PublicationChapter {
    id: String,
    order: i64,
    title: String,
    collection: Option<String>,
    subtitle: Option<String>,
    audio: PublicationChapterAudio,
    segments: Vec<PublicationSegment>,
}

pub async fn get_publication_bundle_preflight(
    database: &Database,
    book_id: &str,
) -> AppResult<PublicationBundlePreflight> {
    let plan = load_plan(database, book_id).await?;
    let segment_count = plan
        .chapters
        .iter()
        .map(|chapter| chapter.segments.len() as i64)
        .sum();
    let speakable_segment_count = plan
        .chapters
        .iter()
        .flat_map(|chapter| chapter.segments.iter())
        .filter(|segment| segment.speak_enabled)
        .count() as i64;
    Ok(PublicationBundlePreflight {
        can_publish: plan.blockers.is_empty(),
        book_id: plan.book.id.clone(),
        book_title: plan.book.title.clone(),
        chapter_count: plan.chapters.len() as i64,
        segment_count,
        speakable_segment_count,
        blockers: plan.blockers,
    })
}

pub async fn start_publication_export(
    app: AppHandle,
    controller: std::sync::Arc<PublicationExportController>,
    database: Database,
    book_id: String,
    destination_dir: String,
) -> AppResult<()> {
    let plan = load_plan(&database, &book_id).await?;
    let segment_count = plan
        .chapters
        .iter()
        .map(|chapter| chapter.segments.len() as i64)
        .sum();
    controller.reserve(&book_id, plan.chapters.len() as i64, segment_count)?;
    if let Some(blocker) = plan.blockers.first() {
        finish_with_error(
            &app,
            &controller,
            AppError::new("PUBLICATION_PREFLIGHT_BLOCKED", blocker.message.clone()),
        );
        clear_tts_busy(&app);
        return Err(AppError::new(
            "PUBLICATION_PREFLIGHT_BLOCKED",
            blocker.message.clone(),
        ));
    }
    let ffmpeg_info = match ffmpeg::check_available(&app, true) {
        Ok(info) => info,
        Err(error) => {
            finish_with_error(&app, &controller, error.clone());
            clear_tts_busy(&app);
            return Err(error);
        }
    };
    let data_dir = match app
        .state::<AppState>()
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))
        .and_then(|path| {
            path.clone()
                .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用目录尚未初始化"))
        }) {
        Ok(path) => path,
        Err(error) => {
            finish_with_error(&app, &controller, error.clone());
            clear_tts_busy(&app);
            return Err(error);
        }
    };
    let work_dir = data_dir
        .join("cache")
        .join("publication")
        .join(Uuid::now_v7().to_string());
    let bundle_dir = work_dir.join("bundle");
    if let Err(error) = fs::create_dir_all(bundle_dir.join("chapters"))
        .and_then(|_| fs::create_dir_all(bundle_dir.join("audio")))
    {
        let error = AppError::from(error);
        finish_with_error(&app, &controller, error.clone());
        clear_tts_busy(&app);
        return Err(error);
    }

    let result = run_publication_export(
        &app,
        &controller,
        ffmpeg_info,
        &plan,
        &bundle_dir,
        Path::new(destination_dir.trim()),
    )
    .await;
    let _ = fs::remove_dir_all(&work_dir);
    clear_tts_busy(&app);
    result
}

pub fn get_publication_export_state(
    controller: &PublicationExportController,
) -> PublicationExportState {
    controller.snapshot()
}

async fn load_plan(database: &Database, book_id: &str) -> AppResult<PublicationPlan> {
    let book = sqlx::query_as::<_, BookRow>(
        "SELECT id, title, author, dynasty, edition FROM books WHERE id = ?",
    )
    .bind(book_id)
    .fetch_optional(database.pool())
    .await?
    .ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "Book 不存在"))?;
    let chapter_rows = sqlx::query_as::<_, ChapterRow>(
        "SELECT id, title, collection, subtitle FROM chapters WHERE book_id = ? ORDER BY order_index ASC",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;
    let rows = sqlx::query_as::<_, SegmentRow>(
        "SELECT c.id AS chapter_id, c.title AS chapter_title,
                s.id AS segment_id, s.original_text,
                s.reading_text, s.translation, s.speak_enabled, s.status, s.current_audio_id,
                av.id AS audio_version_id, av.segment_id AS audio_segment_id,
                av.provider, av.voice_type, av.sample_rate, av.codec, av.speed, av.volume,
                av.audio_path, av.provider_metadata_json
         FROM chapters c
         JOIN segments s ON s.chapter_id = c.id AND s.status <> 'superseded'
         LEFT JOIN audio_versions av ON av.id = s.current_audio_id AND av.segment_id = s.id
         WHERE c.book_id = ?
         ORDER BY c.order_index ASC, s.order_index ASC",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;

    let mut chapters = chapter_rows
        .iter()
        .map(|chapter| PublicationChapterPlan {
            id: chapter.id.clone(),
            title: chapter.title.clone(),
            collection: chapter.collection.clone(),
            subtitle: chapter.subtitle.clone(),
            segments: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if chapters.is_empty() {
        blockers.push(book_blocker("BOOK_NO_CHAPTERS", "当前古籍没有可发布的章节"));
    }

    let mut chapter_signatures: Vec<Option<AudioSignature>> = vec![None; chapters.len()];
    for row in rows {
        let Some(chapter_index) = chapters
            .iter()
            .position(|chapter| chapter.id == row.chapter_id)
        else {
            continue;
        };
        let segment_id = row.segment_id.clone();
        let text = row
            .reading_text
            .clone()
            .unwrap_or_else(|| row.original_text.clone());
        let speak_enabled = row.speak_enabled != 0;
        let pronunciation = build_publication_pronunciation(database, &segment_id, &text).await?;
        let mut audio = None;
        if speak_enabled {
            if row.status != "generated" {
                blockers.push(segment_blocker(
                    &row,
                    if row.status == "ready" {
                        "STALE_AUDIO"
                    } else {
                        "SEGMENT_NOT_READY"
                    },
                    if row.status == "ready" {
                        "这段语音已过期，请重新生成后再发布"
                    } else {
                        "这段尚未生成可发布语音"
                    },
                ));
            }
            if row.current_audio_id.is_none() {
                blockers.push(segment_blocker(
                    &row,
                    "CURRENT_AUDIO_MISSING",
                    "可朗读 Segment 没有 current_audio_id",
                ));
            }
            let fields_present = row.audio_version_id.is_some()
                && row.audio_segment_id.as_deref() == Some(segment_id.as_str())
                && row.provider.is_some()
                && row.voice_type.is_some()
                && row.sample_rate.is_some()
                && row.codec.is_some()
                && row.speed.is_some()
                && row.volume.is_some()
                && row.audio_path.is_some();
            if !fields_present {
                blockers.push(segment_blocker(
                    &row,
                    "AUDIO_VERSION_MISSING",
                    "current_audio_id 对应的 AudioVersion 不存在或不属于当前 Segment",
                ));
            } else {
                let path = PathBuf::from(row.audio_path.as_deref().unwrap_or_default());
                if !path.is_file() {
                    blockers.push(segment_blocker(
                        &row,
                        "AUDIO_FILE_MISSING",
                        "current AudioVersion 的 WAV 文件不存在",
                    ));
                } else {
                    match validate_audio_input(&row, &path) {
                        Ok(input) => {
                            let signature = AudioSignature::from(&input);
                            if let Some(expected) = chapter_signatures[chapter_index].as_ref() {
                                if expected != &signature {
                                    blockers.push(segment_blocker(
                                        &row,
                                        "PUBLICATION_AUDIO_SETTINGS_MISMATCH",
                                        "同一章节内的语音设置或 WAV 参数不一致",
                                    ));
                                }
                            } else {
                                chapter_signatures[chapter_index] = Some(signature);
                            }
                            audio = Some(input);
                        }
                        Err(error) => {
                            blockers.push(segment_blocker(&row, error.code.as_str(), error.message))
                        }
                    }
                }
            }
        }
        chapters[chapter_index]
            .segments
            .push(PublicationSegmentPlan {
                id: segment_id,
                text,
                translation: row.translation.clone(),
                speak_enabled,
                audio,
                pronunciation,
            });
    }

    for chapter in &chapters {
        if chapter.segments.is_empty() {
            blockers.push(PublicationBundleBlocker {
                code: "CHAPTER_EMPTY".to_string(),
                message: "章节没有可发布的 Segment".to_string(),
                chapter_id: Some(chapter.id.clone()),
                chapter_title: chapter.title.clone(),
                segment_id: None,
            });
        } else if !chapter.segments.iter().any(|segment| segment.speak_enabled) {
            blockers.push(PublicationBundleBlocker {
                code: "CHAPTER_NO_AUDIO".to_string(),
                message: "章节没有参与朗读的 Segment，无法生成章节音频".to_string(),
                chapter_id: Some(chapter.id.clone()),
                chapter_title: chapter.title.clone(),
                segment_id: None,
            });
        }
    }
    Ok(PublicationPlan {
        book,
        chapters,
        blockers,
    })
}

fn validate_audio_input(row: &SegmentRow, path: &Path) -> AppResult<AudioInput> {
    let format = audio_service::read_wav_format(path)?;
    if row.codec.as_deref() != Some("wav")
        || row.sample_rate != Some(format.sample_rate as i64)
        || format.audio_format != 1
    {
        return Err(AppError::new(
            "AUDIO_FORMAT_MISMATCH",
            "AudioVersion 参数与实际 PCM WAV 格式不一致",
        ));
    }
    let duration_ms = audio_service::read_wav_duration_ms(path)?;
    let tts_actual = row
        .provider_metadata_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<AudioProviderMetadata>(value).ok())
        .and_then(|metadata| metadata.realized_pronunciation)
        .map(|items| {
            items
                .into_iter()
                .map(|item| PublicationTtsActual {
                    text: item.text,
                    pinyin: item.phoneme,
                    begin_ms: item.begin_ms,
                    end_ms: item.end_ms,
                })
                .collect()
        });
    Ok(AudioInput {
        path: path.to_path_buf(),
        duration_ms,
        provider: row.provider.clone().unwrap_or_default(),
        voice_type: row.voice_type.unwrap_or_default(),
        sample_rate: row.sample_rate.unwrap_or_default(),
        codec: row.codec.clone().unwrap_or_default(),
        speed: row.speed.unwrap_or_default(),
        volume: row.volume.unwrap_or_default(),
        channels: format.channels as i64,
        bits_per_sample: format.bits_per_sample as i64,
        tts_actual,
    })
}

async fn build_publication_pronunciation(
    database: &Database,
    segment_id: &str,
    text: &str,
) -> AppResult<Option<PublicationPronunciation>> {
    let tokens = pronunciation_service::grapheme_tokens(text);
    let annotations = pronunciation_service::list_annotations(database, segment_id).await?;
    let forced =
        pronunciation_service::build_effective_forced_pronunciations(database, segment_id).await?;
    let mut reference = vec![None::<String>; tokens.len()];
    let mut confirmed = vec![None::<String>; tokens.len()];

    let mut annotations = annotations;
    annotations.sort_by(|left, right| {
        let priority = |annotation: &crate::models::Annotation| {
            if annotation.source_rule_id.is_some() || annotation.source.as_deref() == Some("manual")
            {
                3_u8
            } else if annotation.source.as_deref() == Some("imported_authoritative") {
                2
            } else {
                1
            }
        };
        priority(right).cmp(&priority(left)).then_with(|| {
            right
                .end_token
                .saturating_sub(right.start_token)
                .cmp(&left.end_token.saturating_sub(left.start_token))
        })
    });

    for annotation in annotations {
        let Some(pinyin) = annotation.default_pinyin else {
            continue;
        };
        let syllables = pronunciation_service::normalize_and_validate_pinyin(&pinyin)?
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let indexes = (annotation.start_token..annotation.end_token)
            .filter(|index| {
                *index < tokens.len() && pronunciation_service::is_han_token(&tokens[*index].text)
            })
            .collect::<Vec<_>>();
        if indexes.len() != syllables.len() {
            continue;
        }
        for (index, syllable) in indexes.into_iter().zip(syllables) {
            if reference[index].is_none() {
                reference[index] = Some(syllable);
            }
        }
    }

    for range in forced {
        let syllables = range
            .pinyin
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let indexes = (range.start_token..range.end_token)
            .filter(|index| {
                *index < tokens.len() && pronunciation_service::is_han_token(&tokens[*index].text)
            })
            .collect::<Vec<_>>();
        if indexes.len() != syllables.len() {
            return Err(AppError::new(
                "PINYIN_TOKEN_COUNT_MISMATCH",
                "发布包有效读音与正文 token 数量不一致",
            ));
        }
        for (index, syllable) in indexes.into_iter().zip(syllables) {
            confirmed[index] = Some(syllable);
        }
    }

    let output_tokens = tokens
        .into_iter()
        .enumerate()
        .map(|(index, token)| PublicationToken {
            text: token.text,
            reference_pinyin: reference[index].clone(),
            confirmed_pinyin: confirmed[index].clone(),
            forced: confirmed[index].is_some(),
        })
        .collect::<Vec<_>>();
    let has_pronunciation = output_tokens
        .iter()
        .any(|token| token.reference_pinyin.is_some() || token.confirmed_pinyin.is_some());
    Ok(has_pronunciation.then_some(PublicationPronunciation {
        tokens: Some(output_tokens),
        tts_actual: None,
    }))
}

#[derive(Clone, PartialEq)]
struct AudioSignature {
    provider: String,
    voice_type: i64,
    sample_rate: i64,
    codec: String,
    speed: f64,
    volume: f64,
    channels: i64,
    bits_per_sample: i64,
}

impl From<&AudioInput> for AudioSignature {
    fn from(input: &AudioInput) -> Self {
        Self {
            provider: input.provider.clone(),
            voice_type: input.voice_type,
            sample_rate: input.sample_rate,
            codec: input.codec.clone(),
            speed: input.speed,
            volume: input.volume,
            channels: input.channels,
            bits_per_sample: input.bits_per_sample,
        }
    }
}

async fn run_publication_export(
    app: &AppHandle,
    controller: &PublicationExportController,
    ffmpeg_info: ffmpeg::FfmpegInfo,
    plan: &PublicationPlan,
    bundle_dir: &Path,
    destination_dir: &Path,
) -> AppResult<()> {
    let result = async {
        if destination_dir.as_os_str().is_empty()
            || destination_dir.to_string_lossy().contains('\n')
            || destination_dir.to_string_lossy().contains('\r')
        {
            return Err(AppError::new("INVALID_EXPORT_PATH", "发布包目标目录无效"));
        }
        if !destination_dir.is_dir() {
            return Err(AppError::new("FILE_IO_ERROR", "请选择一个已存在的目标目录"));
        }
        let mut book_chapters = Vec::with_capacity(plan.chapters.len());
        for (chapter_index, chapter) in plan.chapters.iter().enumerate() {
            set_phase(app, controller, "generating_audio", chapter_index as i64);
            let chapter_number = chapter_index + 1;
            let chapter_name = format!("chapter-{chapter_number:03}");
            let audio_output = bundle_dir.join("audio").join(format!("{chapter_name}.mp3"));
            let merged_wav = bundle_dir.join(format!("{chapter_name}.merged.wav"));
            let concat_path = bundle_dir.join(format!("{chapter_name}.concat.txt"));
            let audio_inputs = chapter
                .segments
                .iter()
                .filter(|segment| segment.speak_enabled)
                .map(|segment| {
                    segment
                        .audio
                        .as_ref()
                        .map(|audio| audio.path.clone())
                        .ok_or_else(|| {
                            AppError::new("AUDIO_FILE_MISSING", "章节的可朗读 Segment 缺少当前音频")
                        })
                })
                .collect::<AppResult<Vec<_>>>()?;
            fs::write(&concat_path, ffmpeg::build_concat_list(&audio_inputs)?)
                .map_err(AppError::from)?;
            run_ffmpeg(
                &ffmpeg_info.path,
                ffmpeg::concat_wav_args(&concat_path, &merged_wav),
                "PUBLICATION_EXPORT_FAILED",
            )
            .await?;
            let merged_format = audio_service::read_wav_format(&merged_wav)?;
            let expected = chapter
                .segments
                .iter()
                .filter_map(|segment| segment.audio.as_ref())
                .next()
                .ok_or_else(|| AppError::new("CHAPTER_NO_AUDIO", "章节没有可发布音频"))?;
            if merged_format.sample_rate as i64 != expected.sample_rate
                || merged_format.channels as i64 != expected.channels
                || merged_format.bits_per_sample as i64 != expected.bits_per_sample
            {
                return Err(AppError::new(
                    "AUDIO_FORMAT_MISMATCH",
                    "FFmpeg 合并后的章节 WAV 参数与输入不一致",
                ));
            }
            run_ffmpeg(
                &ffmpeg_info.path,
                ffmpeg::encode_mp3_args(
                    &merged_wav,
                    &audio_output,
                    expected.sample_rate,
                    expected.channels,
                    &plan.book.title,
                ),
                "PUBLICATION_EXPORT_FAILED",
            )
            .await?;
            let output_size = fs::metadata(&audio_output)
                .map_err(|error| AppError::new("FILE_IO_ERROR", error.to_string()))?
                .len();
            if output_size == 0 {
                return Err(AppError::new("PUBLICATION_EXPORT_FAILED", "章节 MP3 为空"));
            }
            let chapter_duration_ms = probe_duration(&ffmpeg_info.path, &audio_output).await?;
            let segments = build_timeline(&chapter.segments)?;
            let timeline_ms = segments
                .iter()
                .filter_map(|segment| segment.end_ms)
                .max()
                .unwrap_or(0);
            if (chapter_duration_ms - timeline_ms).abs() > MP3_TIMELINE_TOLERANCE_MS {
                return Err(AppError::new(
                    "PUBLICATION_TIMELINE_MISMATCH",
                    format!(
                        "章节时间轴与 MP3 时长相差 {}ms，超过允许的 {}ms",
                        (chapter_duration_ms - timeline_ms).abs(),
                        MP3_TIMELINE_TOLERANCE_MS
                    ),
                ));
            }
            let title = chapter_title(chapter.title.as_deref(), chapter_number);
            let chapter_json = PublicationChapter {
                id: chapter.id.clone(),
                order: chapter_number as i64,
                title: title.clone(),
                collection: chapter.collection.clone(),
                subtitle: chapter.subtitle.clone(),
                audio: PublicationChapterAudio {
                    src: format!("audio/{chapter_name}.mp3"),
                    duration_ms: chapter_duration_ms,
                    format: "mp3".to_string(),
                },
                segments,
            };
            write_json(
                &bundle_dir
                    .join("chapters")
                    .join(format!("{chapter_name}.json")),
                &chapter_json,
            )?;
            let _ = fs::remove_file(&concat_path);
            let _ = fs::remove_file(&merged_wav);
            book_chapters.push(PublicationBookChapter {
                id: chapter.id.clone(),
                order: chapter_number as i64,
                title,
                content: format!("chapters/{chapter_name}.json"),
                audio: format!("audio/{chapter_name}.mp3"),
            });
            let _ = controller.update(|state| state.chapters_completed = chapter_number as i64);
            emit_state(app, controller);
        }
        set_phase(
            app,
            controller,
            "writing_bundle",
            plan.chapters.len() as i64,
        );
        let book_json = PublicationBook {
            id: plan.book.id.clone(),
            title: plan.book.title.clone(),
            author: plan.book.author.clone(),
            dynasty: plan.book.dynasty.clone(),
            edition: plan.book.edition.clone(),
            language: "zh-CN".to_string(),
            reading_mode: "modern_standard_mandarin".to_string(),
            audio: PublicationAudioFormat {
                format: "mp3".to_string(),
            },
            chapters: book_chapters,
        };
        write_json(&bundle_dir.join("book.json"), &book_json)?;
        let generated_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|error| AppError::new("FILE_IO_ERROR", error.to_string()))?;
        write_json(
            &bundle_dir.join("manifest.json"),
            &PublicationManifest {
                format: FORMAT.to_string(),
                format_version: FORMAT_VERSION.to_string(),
                generator: PublicationGenerator {
                    name: "AncientMedicalTTS".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                generated_at,
                book: "book.json".to_string(),
            },
        )?;
        validate_bundle_directory(bundle_dir)?;
        let destination = destination_dir.join(sanitize_bundle_directory_name(&plan.book.title));
        if destination.exists() {
            return Err(AppError::new(
                "PUBLICATION_OUTPUT_EXISTS",
                "目标目录中已经存在同名发布包，请选择其他目录",
            ));
        }
        if let Err(error) = copy_directory(bundle_dir, &destination) {
            let _ = fs::remove_dir_all(&destination);
            return Err(error);
        }
        Ok::<PathBuf, AppError>(destination)
    }
    .await;
    match result {
        Ok(destination) => {
            let _ = controller.update(|state| {
                state.status = "completed".to_string();
                state.phase = "completed".to_string();
                state.output_path = Some(destination.to_string_lossy().to_string());
            });
            emit_state(app, controller);
            Ok(())
        }
        Err(error) => {
            finish_with_error(app, controller, error.clone());
            Err(error)
        }
    }
}

fn build_timeline(segments: &[PublicationSegmentPlan]) -> AppResult<Vec<PublicationSegment>> {
    let mut timeline_ms = 0_i64;
    let mut result = Vec::with_capacity(segments.len());
    for segment in segments {
        if !segment.speak_enabled {
            result.push(PublicationSegment {
                id: segment.id.clone(),
                order: result.len() as i64 + 1,
                text: segment.text.clone(),
                translation: segment.translation.clone(),
                speak_enabled: false,
                start_ms: None,
                end_ms: None,
                duration_ms: None,
                pronunciation: None,
            });
            continue;
        }
        let audio = segment
            .audio
            .as_ref()
            .ok_or_else(|| AppError::new("AUDIO_FILE_MISSING", "可朗读 Segment 缺少当前音频"))?;
        let start_ms = timeline_ms;
        timeline_ms = timeline_ms.saturating_add(audio.duration_ms);
        result.push(PublicationSegment {
            id: segment.id.clone(),
            order: result.len() as i64 + 1,
            text: segment.text.clone(),
            translation: segment.translation.clone(),
            speak_enabled: true,
            start_ms: Some(start_ms),
            end_ms: Some(timeline_ms),
            duration_ms: Some(audio.duration_ms),
            pronunciation: match (segment.pronunciation.clone(), audio.tts_actual.clone()) {
                (Some(mut pronunciation), Some(tts_actual)) => {
                    pronunciation.tts_actual = Some(tts_actual);
                    Some(pronunciation)
                }
                (Some(pronunciation), None) => Some(pronunciation),
                (None, Some(tts_actual)) => Some(PublicationPronunciation {
                    tokens: None,
                    tts_actual: Some(tts_actual),
                }),
                (None, None) => None,
            },
        });
    }
    Ok(result)
}

async fn run_ffmpeg(path: &Path, args: Vec<String>, code: &str) -> AppResult<()> {
    let path = path.to_path_buf();
    let output = tauri::async_runtime::spawn_blocking(move || ffmpeg::run_output(&path, &args))
        .await
        .map_err(|error| AppError::new(code, format!("FFmpeg 任务异常: {error}")))??;
    if !output.status.success() {
        return Err(AppError::new(
            code,
            format!("FFmpeg 导出失败: {}", stderr_summary(&output.stderr)),
        ));
    }
    Ok(())
}

async fn probe_duration(path: &Path, audio_path: &Path) -> AppResult<i64> {
    let path = path.to_path_buf();
    let audio_path = audio_path.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || ffmpeg::probe_duration_ms(&audio_path, &path))
        .await
        .map_err(|error| {
            AppError::new("PUBLICATION_AUDIO_DURATION_UNAVAILABLE", error.to_string())
        })?
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let data = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::new("FILE_IO_ERROR", error.to_string()))?;
    fs::write(path, data).map_err(AppError::from)
}

fn validate_bundle_directory(root: &Path) -> AppResult<()> {
    let manifest_path = root.join("manifest.json");
    let book_path = root.join("book.json");
    let manifest: serde_json::Value = read_json(&manifest_path)?;
    if manifest.get("format").and_then(serde_json::Value::as_str) != Some(FORMAT)
        || manifest
            .get("format_version")
            .and_then(serde_json::Value::as_str)
            != Some(FORMAT_VERSION)
    {
        return Err(AppError::new(
            "PUBLICATION_VALIDATION_FAILED",
            "发布包 format 或 format_version 无效",
        ));
    }
    let book: PublicationBook = read_json(&book_path)?;
    let mut chapter_ids = HashSet::new();
    let mut segment_ids = HashSet::new();
    let mut previous_chapter_order = 0_i64;
    for chapter in &book.chapters {
        if chapter.order <= previous_chapter_order || !chapter_ids.insert(chapter.id.clone()) {
            return Err(AppError::new(
                "PUBLICATION_VALIDATION_FAILED",
                "章节 ID 或顺序无效",
            ));
        }
        previous_chapter_order = chapter.order;
        validate_relative_resource_path(&chapter.content)?;
        validate_relative_resource_path(&chapter.audio)?;
        let chapter_json_path = root.join(&chapter.content);
        let chapter_json: PublicationChapter = read_json(&chapter_json_path)?;
        if chapter_json.id != chapter.id || chapter_json.audio.src != chapter.audio {
            return Err(AppError::new(
                "PUBLICATION_VALIDATION_FAILED",
                "book.json 与章节 JSON 不一致",
            ));
        }
        if !root.join(&chapter.audio).is_file() {
            return Err(AppError::new(
                "PUBLICATION_VALIDATION_FAILED",
                "章节音频文件不存在",
            ));
        }
        let mut previous_segment_order = 0_i64;
        let mut previous_end = 0_i64;
        let mut last_end = None;
        for segment in &chapter_json.segments {
            if segment.order <= previous_segment_order || !segment_ids.insert(segment.id.clone()) {
                return Err(AppError::new(
                    "PUBLICATION_VALIDATION_FAILED",
                    "Segment ID 或顺序无效",
                ));
            }
            previous_segment_order = segment.order;
            if segment.speak_enabled {
                let (start, end, duration) =
                    match (segment.start_ms, segment.end_ms, segment.duration_ms) {
                        (Some(start), Some(end), Some(duration)) => (start, end, duration),
                        _ => {
                            return Err(AppError::new(
                                "PUBLICATION_VALIDATION_FAILED",
                                "可朗读 Segment 缺少时间轴",
                            ))
                        }
                    };
                if start < previous_end || end < start || end - start != duration {
                    return Err(AppError::new(
                        "PUBLICATION_VALIDATION_FAILED",
                        "Segment 时间轴重叠或不单调",
                    ));
                }
                previous_end = end;
                last_end = Some(end);
            } else if segment.start_ms.is_some()
                || segment.end_ms.is_some()
                || segment.duration_ms.is_some()
            {
                return Err(AppError::new(
                    "PUBLICATION_VALIDATION_FAILED",
                    "不朗读 Segment 不应包含时间轴",
                ));
            }
        }
        if last_end.unwrap_or(0) > chapter_json.audio.duration_ms
            || chapter_json.audio.duration_ms - last_end.unwrap_or(0) > MP3_TIMELINE_TOLERANCE_MS
        {
            return Err(AppError::new(
                "PUBLICATION_VALIDATION_FAILED",
                "章节末尾时间轴与音频时长差异过大",
            ));
        }
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> AppResult<T> {
    let data = fs::read(path).map_err(|error| {
        AppError::new(
            "PUBLICATION_VALIDATION_FAILED",
            format!("无法读取 {}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&data).map_err(|error| {
        AppError::new(
            "PUBLICATION_VALIDATION_FAILED",
            format!("{} JSON 无效: {error}", path.display()),
        )
    })
}

fn validate_relative_resource_path(path: &str) -> AppResult<()> {
    let candidate = Path::new(path);
    if path.is_empty()
        || candidate.is_absolute()
        || path.contains('\\')
        || path.contains(':')
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::new(
            "PUBLICATION_VALIDATION_FAILED",
            "发布包资源路径必须是安全的相对路径",
        ));
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> AppResult<()> {
    fs::create_dir_all(destination).map_err(AppError::from)?;
    for entry in fs::read_dir(source).map_err(AppError::from)? {
        let entry = entry.map_err(AppError::from)?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(AppError::from)?;
        }
    }
    Ok(())
}

fn chapter_title(title: Option<&str>, number: usize) -> String {
    title
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("第{number}章"))
}

fn sanitize_bundle_directory_name(title: &str) -> String {
    let filtered = title
        .chars()
        .map(|character| {
            if matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let trimmed = filtered.trim().trim_end_matches('.').trim_end_matches(' ');
    if trimmed.is_empty() {
        "ancient-medical-publication".to_string()
    } else {
        trimmed.to_string()
    }
}

fn segment_blocker(
    row: &SegmentRow,
    code: &str,
    message: impl Into<String>,
) -> PublicationBundleBlocker {
    PublicationBundleBlocker {
        code: code.to_string(),
        message: message.into(),
        chapter_id: Some(row.chapter_id.clone()),
        chapter_title: row.chapter_title.clone(),
        segment_id: Some(row.segment_id.clone()),
    }
}

fn book_blocker(code: &str, message: &str) -> PublicationBundleBlocker {
    PublicationBundleBlocker {
        code: code.to_string(),
        message: message.to_string(),
        chapter_id: None,
        chapter_title: None,
        segment_id: None,
    }
}

fn set_phase(
    app: &AppHandle,
    controller: &PublicationExportController,
    phase: &str,
    completed: i64,
) {
    let _ = controller.update(|state| {
        state.phase = phase.to_string();
        state.chapters_completed = completed;
    });
    emit_state(app, controller);
}

fn emit_state(app: &AppHandle, controller: &PublicationExportController) {
    let _ = app.emit("publication-progress", controller.snapshot());
}

fn finish_with_error(app: &AppHandle, controller: &PublicationExportController, error: AppError) {
    let _ = controller.update(|state| {
        state.status = "failed".to_string();
        state.phase = "failed".to_string();
        state.error_code = Some(error.code);
        state.error_message = Some(error.message);
    });
    emit_state(app, controller);
}

fn clear_tts_busy(app: &AppHandle) {
    if let Ok(mut busy) = app.state::<AppState>().tts_busy.lock() {
        *busy = false;
    }
}

fn stderr_summary(bytes: &[u8]) -> String {
    const LIMIT: usize = 8 * 1024;
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.len() > LIMIT {
        text[text.len() - LIMIT..].to_string()
    } else {
        text
    }
}

fn idle_state() -> PublicationExportState {
    PublicationExportState {
        book_id: None,
        status: "idle".to_string(),
        phase: "idle".to_string(),
        chapters_completed: 0,
        chapters_total: 0,
        segments_total: 0,
        output_path: None,
        error_code: None,
        error_message: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_timeline, sanitize_bundle_directory_name, validate_relative_resource_path,
        AudioInput, AudioSignature, PublicationExportController, PublicationSegmentPlan,
    };
    use crate::db::Database;
    use crate::models::PublicationExportState;
    use crate::{audio::ffmpeg, services::audio_service};
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn sanitizes_unicode_bundle_directory_name_without_absolute_paths() {
        assert_eq!(
            sanitize_bundle_directory_name("黄帝内经:素问"),
            "黄帝内经_素问"
        );
        assert_eq!(
            sanitize_bundle_directory_name("..."),
            "ancient-medical-publication"
        );
        assert!(validate_relative_resource_path("audio/chapter-001.mp3").is_ok());
        assert!(validate_relative_resource_path("/Users/test/audio.mp3").is_err());
        assert!(validate_relative_resource_path("../secret").is_err());
        assert!(validate_relative_resource_path("C:\\secret").is_err());
    }

    #[test]
    fn controller_starts_idle() {
        let controller = PublicationExportController::new();
        let state: PublicationExportState = controller.snapshot();
        assert_eq!(state.status, "idle");
        assert!(!controller.is_running());
        let _ = AudioSignature {
            provider: "tencent".to_string(),
            voice_type: 501000,
            sample_rate: 16000,
            codec: "wav".to_string(),
            speed: 0.0,
            volume: 0.0,
            channels: 1,
            bits_per_sample: 16,
        };
    }

    #[test]
    fn timing_regression_uses_actual_duration_and_skips_non_speaking_segment() {
        let segment = |id: &str, text: &str, duration_ms: i64| PublicationSegmentPlan {
            id: id.to_string(),
            text: text.to_string(),
            translation: None,
            speak_enabled: true,
            audio: Some(AudioInput {
                path: std::path::PathBuf::from(format!("{id}.wav")),
                duration_ms,
                provider: "tencent".to_string(),
                voice_type: 501000,
                sample_rate: 16000,
                codec: "wav".to_string(),
                speed: 0.0,
                volume: 0.0,
                channels: 1,
                bits_per_sample: 16,
                tts_actual: None,
            }),
            pronunciation: None,
        };
        let result = build_timeline(&[
            segment("s1", "甲", 1000),
            PublicationSegmentPlan {
                id: "title".to_string(),
                text: "标题".to_string(),
                translation: Some("说明".to_string()),
                speak_enabled: false,
                audio: None,
                pronunciation: None,
            },
            segment("s2", "乙", 1500),
            segment("s3", "丙𠀀", 750),
        ])
        .expect("timeline");
        assert_eq!(
            (result[0].start_ms, result[0].end_ms),
            (Some(0), Some(1000))
        );
        assert_eq!((result[1].start_ms, result[1].end_ms), (None, None));
        assert_eq!(
            (result[2].start_ms, result[2].end_ms),
            (Some(1000), Some(2500))
        );
        assert_eq!(
            (result[3].start_ms, result[3].end_ms),
            (Some(2500), Some(3250))
        );
        assert_eq!(result[3].text, "丙𠀀");
    }

    #[test]
    fn preflight_orders_chapters_uses_reading_text_and_blocks_stale_audio() {
        let database_path =
            std::env::temp_dir().join(format!("publication-preflight-{}.sqlite", Uuid::now_v7()));
        let audio_dir = std::env::temp_dir().join(format!("publication-audio-{}", Uuid::now_v7()));
        fs::create_dir_all(&audio_dir).expect("audio directory");
        let database =
            tauri::async_runtime::block_on(Database::open(&database_path)).expect("database");
        let book_id = Uuid::now_v7().to_string();
        let chapter_one = Uuid::now_v7().to_string();
        let chapter_two = Uuid::now_v7().to_string();
        let segment_one = Uuid::now_v7().to_string();
        let segment_title = Uuid::now_v7().to_string();
        let segment_two = Uuid::now_v7().to_string();
        let audio_one = Uuid::now_v7().to_string();
        let audio_two = Uuid::now_v7().to_string();
        let path_one = audio_dir.join("s1.wav");
        let path_two = audio_dir.join("s2.wav");
        fs::write(&path_one, wav_bytes(1000)).expect("wav one");
        fs::write(&path_two, wav_bytes(1500)).expect("wav two");
        tauri::async_runtime::block_on(async {
            let pool = database.pool();
            sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, '测试古籍', 'now', 'now')")
                .bind(&book_id).execute(pool).await.expect("book");
            for (id, order, title) in [
                (&chapter_two, 1_i64, "第二章"),
                (&chapter_one, 0_i64, "第一章"),
            ] {
                sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, ?, ?, 'now', 'now')")
                    .bind(id).bind(&book_id).bind(title).bind(order).execute(pool).await.expect("chapter");
            }
            sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, reading_text, translation, speak_enabled, status, current_audio_id, created_at, updated_at) VALUES (?, ?, 1, '原文甲', '朗读甲', '译文甲', 1, 'generated', ?, 'now', 'now')")
                .bind(&segment_one).bind(&chapter_one).bind(&audio_one).execute(pool).await.expect("segment one");
            sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, speak_enabled, status, created_at, updated_at) VALUES (?, ?, 0, '篇名', 0, 'pending', 'now', 'now')")
                .bind(&segment_title).bind(&chapter_one).execute(pool).await.expect("title segment");
            sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, speak_enabled, status, current_audio_id, created_at, updated_at) VALUES (?, ?, 0, '原文乙', 1, 'generated', ?, 'now', 'now')")
                .bind(&segment_two).bind(&chapter_two).bind(&audio_two).execute(pool).await.expect("segment two");
            for (id, segment_id, path, duration) in [
                (&audio_one, &segment_one, &path_one, 1000_i64),
                (&audio_two, &segment_two, &path_two, 1500_i64),
            ] {
                sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, audio_path, duration_ms, created_at) VALUES (?, ?, 1, 'tencent', 501000, 16000, 'wav', 0, 0, ?, ?, 'now')")
                    .bind(id).bind(segment_id).bind(path.to_string_lossy().to_string()).bind(duration).execute(pool).await.expect("audio version");
            }
            let preflight = super::get_publication_bundle_preflight(&database, &book_id)
                .await
                .expect("preflight");
            assert!(preflight.can_publish);
            assert_eq!(preflight.chapter_count, 2);
            assert_eq!(preflight.speakable_segment_count, 2);
            let plan = super::load_plan(&database, &book_id).await.expect("plan");
            assert_eq!(plan.chapters[0].id, chapter_one);
            assert_eq!(plan.chapters[0].segments[0].text, "篇名");
            assert_eq!(plan.chapters[0].segments[1].text, "朗读甲");
            assert_eq!(
                plan.chapters[0].segments[1].translation.as_deref(),
                Some("译文甲")
            );
            sqlx::query("UPDATE segments SET status = 'ready' WHERE id = ?")
                .bind(&segment_one)
                .execute(pool)
                .await
                .expect("stale");
            let stale = super::get_publication_bundle_preflight(&database, &book_id)
                .await
                .expect("stale preflight");
            assert!(stale
                .blockers
                .iter()
                .any(|blocker| blocker.code == "STALE_AUDIO"));
        });
        let _ = fs::remove_dir_all(audio_dir);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn publication_ffmpeg_integration_is_opt_in() {
        if std::env::var("RUN_FFMPEG_INTEGRATION").ok().as_deref() != Some("1") {
            return;
        }
        let ffmpeg_path = std::env::var_os("ANCIENT_MEDICAL_TTS_FFMPEG")
            .map(std::path::PathBuf::from)
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
        let temp = std::env::temp_dir().join(format!("publication-ffmpeg-{}", Uuid::now_v7()));
        fs::create_dir_all(&temp).expect("temp directory");
        let durations = [1000_i64, 1500, 750];
        let mut paths = Vec::new();
        for (index, duration) in durations.into_iter().enumerate() {
            let path = temp.join(format!("{index}.wav"));
            fs::write(&path, wav_bytes(duration)).expect("write wav");
            paths.push(path);
        }
        let concat = temp.join("concat.txt");
        fs::write(
            &concat,
            ffmpeg::build_concat_list(&paths).expect("concat list"),
        )
        .expect("write concat");
        let merged = temp.join("merged.wav");
        let merge = ffmpeg::run_output(&ffmpeg_path, &ffmpeg::concat_wav_args(&concat, &merged))
            .expect("run concat");
        assert!(
            merge.status.success(),
            "concat failed: {}",
            String::from_utf8_lossy(&merge.stderr)
        );
        assert_eq!(
            audio_service::read_wav_duration_ms(&merged).expect("merged duration"),
            3250
        );
        let mp3 = temp.join("chapter-001.mp3");
        let encode = ffmpeg::run_output(
            &ffmpeg_path,
            &ffmpeg::encode_mp3_args(&merged, &mp3, 16000, 1, "测试古籍"),
        )
        .expect("run mp3");
        assert!(
            encode.status.success(),
            "mp3 failed: {}",
            String::from_utf8_lossy(&encode.stderr)
        );
        assert!(fs::metadata(&mp3).expect("mp3 metadata").len() > 0);
        let duration = ffmpeg::probe_duration_ms(&mp3, &ffmpeg_path).expect("mp3 duration");
        assert!((duration - 3250).abs() <= super::MP3_TIMELINE_TOLERANCE_MS);
        let _ = fs::remove_dir_all(temp);
    }

    fn wav_bytes(duration_ms: i64) -> Vec<u8> {
        let sample_rate = 16_000_u32;
        let samples = (u64::from(sample_rate) * duration_ms as u64 / 1000) as u32;
        let data_size = samples * 2;
        let mut bytes = Vec::with_capacity(44 + data_size as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        bytes.resize(44 + data_size as usize, 0);
        bytes
    }
}
