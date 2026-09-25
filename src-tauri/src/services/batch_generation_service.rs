use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{
        BatchFailedSegment, BatchFatalError, BatchGenerationBlocker, BatchGenerationState,
        BookGenerationPreflight, TtsSettings,
    },
    security::credentials,
    services::{
        audio_service, book_service, pronunciation_service, settings_service, text_service,
    },
    AppState,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;

const SEGMENT_MAX_CHARACTERS: usize = 150;
const MAX_RETRIES: usize = 2;

#[derive(Clone)]
struct BatchSegment {
    id: String,
    chapter_title: Option<String>,
    order_index: i64,
    original_text: String,
    reading_text: Option<String>,
    speak_enabled: bool,
    status: String,
}

impl BatchSegment {
    fn effective_text(&self) -> &str {
        self.reading_text
            .as_deref()
            .unwrap_or(self.original_text.as_str())
    }
}

pub struct BatchGenerationController {
    inner: Mutex<ControllerInner>,
}

struct ControllerInner {
    state: BatchGenerationState,
    cancel_requested: bool,
}

impl BatchGenerationController {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ControllerInner {
                state: idle_state(),
                cancel_requested: false,
            }),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| matches!(inner.state.status.as_str(), "running" | "cancelling"))
            .unwrap_or(true)
    }

    pub fn snapshot(&self) -> BatchGenerationState {
        self.inner
            .lock()
            .map(|inner| inner.state.clone())
            .unwrap_or_else(|_| {
                let mut state = idle_state();
                state.status = "failed".to_string();
                state.fatal_error = Some(BatchFatalError {
                    code: "BATCH_STATE_ERROR".to_string(),
                    message: "批量生成状态锁不可用".to_string(),
                });
                state
            })
    }

    fn reserve(&self, book_id: &str) -> AppResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("BATCH_STATE_ERROR", "批量生成状态锁不可用"))?;
        if matches!(inner.state.status.as_str(), "running" | "cancelling") {
            return Err(AppError::new(
                "BATCH_GENERATION_ALREADY_RUNNING",
                "已有一个 Book 正在批量生成语音",
            ));
        }
        inner.state = idle_state();
        inner.state.book_id = Some(book_id.to_string());
        inner.state.status = "running".to_string();
        inner.cancel_requested = false;
        Ok(())
    }

    fn reset(&self) -> AppResult<()> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("BATCH_STATE_ERROR", "批量生成状态锁不可用"))?;
        inner.state = idle_state();
        inner.cancel_requested = false;
        Ok(())
    }

    fn request_cancel(&self) -> AppResult<BatchGenerationState> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("BATCH_STATE_ERROR", "批量生成状态锁不可用"))?;
        if inner.state.status == "running" {
            inner.state.status = "cancelling".to_string();
            inner.cancel_requested = true;
        }
        Ok(inner.state.clone())
    }

    fn cancelled(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.cancel_requested)
            .unwrap_or(true)
    }

    fn update<F>(&self, update: F) -> AppResult<BatchGenerationState>
    where
        F: FnOnce(&mut BatchGenerationState),
    {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::new("BATCH_STATE_ERROR", "批量生成状态锁不可用"))?;
        update(&mut inner.state);
        Ok(inner.state.clone())
    }
}

impl Default for BatchGenerationController {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn get_book_generation_preflight(
    database: &Database,
    book_id: &str,
    data_dir: &Path,
) -> AppResult<BookGenerationPreflight> {
    let stats = book_service::get_book_text_stats(database, book_id).await?;
    let segments = load_book_segments(database, book_id).await?;
    let settings = settings_service::get_tts_settings(database).await.ok();
    let mut blockers = Vec::new();

    if stats.han_character_count as usize > text_service::MAX_BOOK_HAN_CHARACTERS {
        blockers.push(BatchGenerationBlocker {
            code: "PROJECT_TEXT_LIMIT_EXCEEDED".to_string(),
            message: format!(
                "当前 Book 包含 {} 个汉字，超过 {} 个汉字的批量语音上限",
                stats.han_character_count,
                text_service::MAX_BOOK_HAN_CHARACTERS
            ),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }

    let pending_segments = segments
        .iter()
        .filter(|segment| segment.speak_enabled && segment.status == "pending")
        .count();
    let needs_review_segments = segments
        .iter()
        .filter(|segment| segment.speak_enabled && segment.status == "needs_review")
        .count();
    if pending_segments > 0 {
        blockers.push(BatchGenerationBlocker {
            code: "SEGMENT_PRONUNCIATION_PENDING".to_string(),
            message: format!("有 {pending_segments} 个 Segment 尚未进行发音分析"),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }
    if needs_review_segments > 0 {
        blockers.push(BatchGenerationBlocker {
            code: "SEGMENT_HAS_UNRESOLVED_PRONUNCIATION".to_string(),
            message: format!("有 {needs_review_segments} 个 Segment 仍有发音待确认"),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }

    let unsupported_segments = segments
        .iter()
        .filter(|segment| {
            segment.speak_enabled && {
                !matches!(
                    segment.status.as_str(),
                    "pending" | "needs_review" | "analyzed" | "ready" | "generated"
                )
            }
        })
        .count();
    if unsupported_segments > 0 {
        blockers.push(BatchGenerationBlocker {
            code: "SEGMENT_NOT_READY_FOR_TTS".to_string(),
            message: format!("有 {unsupported_segments} 个 Segment 处于不支持批量生成的状态"),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }

    for segment in segments.iter().filter(|segment| {
        segment.speak_enabled && segment.effective_text().chars().count() > SEGMENT_MAX_CHARACTERS
    }) {
        blockers.push(segment_blocker(
            segment,
            "SEGMENT_TEXT_TOO_LONG",
            format!(
                "当前 Segment 有 {} 个字符，超过腾讯云单句 {} 字限制",
                segment.effective_text().chars().count(),
                SEGMENT_MAX_CHARACTERS
            ),
        ));
    }

    match credentials::status(data_dir) {
        Ok(status) if status.secret_id_configured && status.secret_key_configured => {}
        Ok(_) => blockers.push(BatchGenerationBlocker {
            code: "TTS_CREDENTIALS_MISSING".to_string(),
            message: "腾讯云 SecretId / SecretKey 尚未完整配置".to_string(),
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        }),
        Err(error) => blockers.push(BatchGenerationBlocker {
            code: error.code,
            message: error.message,
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        }),
    }

    if let Err(error) = settings_service::get_tts_settings(database).await {
        blockers.push(BatchGenerationBlocker {
            code: error.code,
            message: error.message,
            segment_id: None,
            chapter_title: None,
            segment_order: None,
            preview: None,
        });
    }

    let mut generated_and_reusable = 0_i64;
    let mut need_generation = 0_i64;
    if let Some(settings) = settings {
        for segment in segments.iter().filter(|segment| {
            segment.speak_enabled
                && audio_service::ALLOWED_STATUS.contains(&segment.status.as_str())
        }) {
            if segment.status == "generated"
                && audio_service::latest_audio_matches(database, &segment.id, &settings).await?
            {
                generated_and_reusable += 1;
            } else {
                need_generation += 1;
            }
        }
    } else {
        need_generation = segments
            .iter()
            .filter(|segment| {
                segment.speak_enabled
                    && audio_service::ALLOWED_STATUS.contains(&segment.status.as_str())
            })
            .count() as i64;
    }

    Ok(BookGenerationPreflight {
        can_generate: blockers.is_empty(),
        han_character_count: stats.han_character_count,
        total_segments: stats.segment_count,
        pending_segments: pending_segments as i64,
        needs_review_segments: needs_review_segments as i64,
        generated_and_reusable,
        need_generation,
        blockers,
    })
}

pub async fn start_book_audio_generation(
    app: AppHandle,
    controller: Arc<BatchGenerationController>,
    database: Database,
    book_id: String,
    data_dir: PathBuf,
) -> AppResult<()> {
    controller.reserve(&book_id)?;
    let preflight = match get_book_generation_preflight(&database, &book_id, &data_dir).await {
        Ok(preflight) => preflight,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    if !preflight.can_generate {
        controller.reset()?;
        return Err(AppError::new(
            "BATCH_PREFLIGHT_BLOCKED",
            preflight
                .blockers
                .first()
                .map(|blocker| blocker.message.clone())
                .unwrap_or_else(|| "批量生成预检未通过".to_string()),
        ));
    }
    let settings = match settings_service::get_tts_settings(&database).await {
        Ok(settings) => settings,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    let segments = match load_book_segments(&database, &book_id).await {
        Ok(segments) => segments,
        Err(error) => {
            controller.reset()?;
            return Err(error);
        }
    };
    controller.update(|state| {
        state.total_segments = preflight.total_segments;
        state.segments_requiring_generation = preflight.need_generation;
    })?;
    emit_state(&app, &controller);

    let run_controller = Arc::clone(&controller);
    let run_app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_batch(run_app, run_controller, database, settings, segments).await;
    });
    Ok(())
}

pub fn cancel_book_audio_generation(
    app: &AppHandle,
    controller: &BatchGenerationController,
) -> AppResult<BatchGenerationState> {
    let state = controller.request_cancel()?;
    emit_state(app, controller);
    Ok(state)
}

pub fn get_batch_generation_state(controller: &BatchGenerationController) -> BatchGenerationState {
    controller.snapshot()
}

async fn run_batch(
    app: AppHandle,
    controller: Arc<BatchGenerationController>,
    database: Database,
    settings: TtsSettings,
    segments: Vec<BatchSegment>,
) {
    let app_state = app.state::<AppState>();
    for segment in segments.into_iter().filter(|segment| {
        segment.speak_enabled && audio_service::ALLOWED_STATUS.contains(&segment.status.as_str())
    }) {
        if controller.cancelled() {
            finish_cancelled(&app, &controller);
            clear_tts_busy(&app_state);
            return;
        }

        let reusable = if segment.status == "generated" {
            match audio_service::latest_audio_matches(&database, &segment.id, &settings).await {
                Ok(value) => value,
                Err(error) => {
                    finish_fatal(&app, &controller, error);
                    clear_tts_busy(&app_state);
                    return;
                }
            }
        } else {
            false
        };
        if reusable {
            let _ = controller.update(|state| {
                state.skipped += 1;
            });
            emit_state(&app, &controller);
            continue;
        }

        let _ = controller.update(|state| {
            state.current_segment_id = Some(segment.id.clone());
            state.current_segment_order = Some(segment.order_index);
            state.current_preview = Some(preview(segment.effective_text()));
        });
        emit_state(&app, &controller);
        let result = generate_with_retry(&app_state, &database, &segment.id, &settings).await;
        match result {
            Ok(()) => {
                let _ = controller.update(|state| {
                    state.processed += 1;
                    state.generated += 1;
                    clear_current(state);
                });
            }
            Err(error) if is_fatal_error(&error.code) => {
                finish_fatal(&app, &controller, error);
                clear_tts_busy(&app_state);
                return;
            }
            Err(error) => {
                let _ = controller.update(|state| {
                    state.processed += 1;
                    state.failed += 1;
                    state.has_failures = true;
                    state.failed_segments.push(BatchFailedSegment {
                        segment_id: segment.id.clone(),
                        chapter_title: segment.chapter_title.clone(),
                        segment_order: segment.order_index,
                        preview: preview(segment.effective_text()),
                        code: error.code,
                        message: error.message,
                    });
                    clear_current(state);
                });
            }
        }
        emit_state(&app, &controller);
        if controller.cancelled() {
            finish_cancelled(&app, &controller);
            clear_tts_busy(&app_state);
            return;
        }
    }

    let _ = controller.update(|state| {
        state.status = "completed".to_string();
        clear_current(state);
    });
    emit_state(&app, &controller);
    clear_tts_busy(&app_state);
}

async fn generate_with_retry(
    state: &AppState,
    database: &Database,
    segment_id: &str,
    settings: &TtsSettings,
) -> AppResult<()> {
    for attempt in 0..=MAX_RETRIES {
        match audio_service::generate_segment_audio_with_settings(
            state,
            database,
            segment_id,
            settings,
            "batch_generation",
        )
        .await
        {
            Ok(_) => return Ok(()),
            Err(error) if is_retryable(&error.code) && attempt < MAX_RETRIES => {
                if error.code == "TTS_TIMEOUT" {
                    state.restart_worker()?;
                }
                let delay_ms = if attempt == 0 { 500 } else { 1_500 };
                sleep(Duration::from_millis(delay_ms)).await;
            }
            Err(error) => return Err(error),
        }
    }
    Err(AppError::new("TTS_PROVIDER_ERROR", "TTS 重试失败"))
}

async fn load_book_segments(database: &Database, book_id: &str) -> AppResult<Vec<BatchSegment>> {
    let rows = sqlx::query_as::<_, (String, Option<String>, i64, String, Option<String>, i64, String)>(
        "SELECT s.id, c.title, s.order_index, s.original_text, s.reading_text, s.speak_enabled, s.status
         FROM segments s JOIN chapters c ON c.id = s.chapter_id
         WHERE c.book_id = ? AND s.status <> 'superseded' ORDER BY c.order_index, s.order_index",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                chapter_title,
                order_index,
                original_text,
                reading_text,
                speak_enabled,
                status,
            )| BatchSegment {
                id,
                chapter_title,
                order_index,
                original_text,
                reading_text,
                speak_enabled: speak_enabled != 0,
                status,
            },
        )
        .collect())
}

fn segment_blocker(segment: &BatchSegment, code: &str, message: String) -> BatchGenerationBlocker {
    BatchGenerationBlocker {
        code: code.to_string(),
        message,
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
        .collect::<String>()
}

fn clear_current(state: &mut BatchGenerationState) {
    state.current_segment_id = None;
    state.current_segment_order = None;
    state.current_preview = None;
}

fn idle_state() -> BatchGenerationState {
    BatchGenerationState {
        book_id: None,
        status: "idle".to_string(),
        total_segments: 0,
        segments_requiring_generation: 0,
        processed: 0,
        generated: 0,
        skipped: 0,
        failed: 0,
        current_segment_id: None,
        current_segment_order: None,
        current_preview: None,
        has_failures: false,
        failed_segments: Vec::new(),
        fatal_error: None,
    }
}

fn emit_state(app: &AppHandle, controller: &BatchGenerationController) {
    let _ = app.emit("batch-generation-progress", controller.snapshot());
}

fn finish_cancelled(app: &AppHandle, controller: &BatchGenerationController) {
    let _ = controller.update(|state| {
        state.status = "cancelled".to_string();
        clear_current(state);
    });
    emit_state(app, controller);
}

fn finish_fatal(app: &AppHandle, controller: &BatchGenerationController, error: AppError) {
    let _ = controller.update(|state| {
        state.status = "failed".to_string();
        state.has_failures = true;
        state.fatal_error = Some(BatchFatalError {
            code: error.code,
            message: error.message,
        });
        clear_current(state);
    });
    emit_state(app, controller);
}

fn clear_tts_busy(state: &AppState) {
    if let Ok(mut busy) = state.tts_busy.lock() {
        *busy = false;
    }
}

fn is_retryable(code: &str) -> bool {
    matches!(
        code,
        "TTS_TIMEOUT"
            | "TTS_RATE_LIMITED"
            | "TTS_PROVIDER_ERROR"
            | "TTS_TEMPORARY_ERROR"
            | "TTS_SERVICE_UNAVAILABLE"
    )
}

fn is_fatal_error(code: &str) -> bool {
    matches!(
        code,
        "DB_ERROR"
            | "DB_TRANSACTION_FAILED"
            | "FILE_IO_ERROR"
            | "WORKER_NOT_RUNNING"
            | "WORKER_ERROR"
            | "WORKER_PROTOCOL_ERROR"
            | "TTS_CREDENTIALS_MISSING"
            | "TTS_AUTH_ERROR"
            | "TTS_SERVICE_NOT_ENABLED"
            | "TTS_QUOTA_EXHAUSTED"
            | "TTS_INVALID_VOICE"
            | "TTS_INVALID_FORMAT"
            | "TTS_INVALID_SPEED"
            | "TTS_INVALID_VOLUME"
            | "TTS_PROVIDER_UNSUPPORTED"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        get_book_generation_preflight, is_fatal_error, is_retryable, BatchGenerationController,
    };
    use crate::{db::Database, services::book_service};
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path =
            std::env::temp_dir().join(format!("ancient-tts-batch-{}.sqlite", Uuid::now_v7()));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    #[test]
    fn only_one_batch_can_be_reserved() {
        let controller = BatchGenerationController::new();
        controller.reserve("book-a").expect("first reservation");
        let error = controller
            .reserve("book-b")
            .expect_err("second reservation should fail");
        assert_eq!(error.code, "BATCH_GENERATION_ALREADY_RUNNING");
        assert_eq!(controller.snapshot().book_id.as_deref(), Some("book-a"));
    }

    #[test]
    fn retry_and_fatal_error_classes_are_explicit() {
        assert!(is_retryable("TTS_TIMEOUT"));
        assert!(is_retryable("TTS_RATE_LIMITED"));
        assert!(is_retryable("TTS_PROVIDER_ERROR"));
        assert!(is_retryable("TTS_TEMPORARY_ERROR"));
        assert!(!is_retryable("TTS_AUTH_ERROR"));
        assert!(is_fatal_error("TTS_AUTH_ERROR"));
        assert!(is_fatal_error("DB_TRANSACTION_FAILED"));
        assert!(!is_fatal_error("TTS_INVALID_SSML"));
    }

    #[test]
    fn preflight_reports_pending_segments_before_generation() {
        let database = temp_db();
        let path = std::env::temp_dir().join(format!("ancient-tts-batch-{}.txt", Uuid::now_v7()));
        std::fs::write(&path, "腧穴").expect("fixture should write");
        tauri::async_runtime::block_on(async {
            let imported =
                book_service::import_txt_book(&database, path.to_str().expect("temp path"), None)
                    .await
                    .expect("book should import");
            let data_dir =
                std::env::temp_dir().join(format!("ancient-tts-batch-data-{}", Uuid::now_v7()));
            let preflight =
                get_book_generation_preflight(&database, &imported.book.book.id, &data_dir)
                    .await
                    .expect("preflight should succeed");
            assert!(!preflight.can_generate);
            assert_eq!(preflight.pending_segments, 1);
            assert!(preflight
                .blockers
                .iter()
                .any(|blocker| blocker.code == "SEGMENT_PRONUNCIATION_PENDING"));
            let segment_id: String = sqlx::query_scalar("SELECT s.id FROM segments s JOIN chapters c ON c.id = s.chapter_id WHERE c.book_id = ? LIMIT 1")
                .bind(&imported.book.book.id)
                .fetch_one(database.pool())
                .await
                .expect("segment id");
            sqlx::query("UPDATE segments SET original_text = ?, status = 'analyzed' WHERE id = ?")
                .bind("甲".repeat(151))
                .bind(&segment_id)
                .execute(database.pool())
                .await
                .expect("make segment overlong");
            let overlong =
                get_book_generation_preflight(&database, &imported.book.book.id, &data_dir)
                    .await
                    .expect("overlong preflight should succeed");
            assert!(overlong
                .blockers
                .iter()
                .any(|blocker| blocker.code == "SEGMENT_TEXT_TOO_LONG"));
            let _ = std::fs::remove_dir_all(data_dir);
        });
        let _ = std::fs::remove_file(path);
    }
}
