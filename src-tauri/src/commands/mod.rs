use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{
        Annotation, ApiUsageSummary, BatchGenerationState, BookDetail, BookExportPreflight,
        BookGenerationPreflight, BookPronunciationAnalysisResult, BookSummary, ChapterSummary,
        ComponentStatus, DeveloperStatus, ExportState, FfmpegStatus, ImportResult,
        PaginatedSegments, ReaderDisplaySettings, Segment, SegmentDisplayPinyin, SegmentEditResult,
        SegmentReader, TtsPreviewResult, WorkerPing, WorkerStatus,
    },
    models::{
        CredentialStatus, PronunciationRule, PronunciationRuleApplyResult,
        PronunciationRuleMutation, TencentVoice, TtsSettings,
    },
    security::credentials,
    services::{
        audio_service, batch_generation_service, book_service, export_service, preview_service,
        pronunciation_rule_service, pronunciation_service, segment_edit_service, settings_service,
        usage_service,
    },
    AppState,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<DeveloperStatus, AppError> {
    let database = match state.database() {
        Ok(database) => match database.health().await {
            Ok(()) => ComponentStatus {
                ok: true,
                message: "SQLite 已连接".to_string(),
            },
            Err(error) => ComponentStatus {
                ok: false,
                message: error.message,
            },
        },
        Err(error) => ComponentStatus {
            ok: false,
            message: error.message,
        },
    };

    let worker = match state.worker_ping() {
        Ok(ping) => WorkerStatus {
            ok: true,
            running: true,
            version: Some(ping.version),
            message: "运行中".to_string(),
        },
        Err(error) => WorkerStatus {
            ok: false,
            running: false,
            version: None,
            message: error.message,
        },
    };

    Ok(DeveloperStatus {
        application: ComponentStatus {
            ok: true,
            message: "应用正在运行".to_string(),
        },
        database,
        worker,
    })
}

#[tauri::command]
pub fn ping_worker(state: State<'_, AppState>) -> AppResult<WorkerPing> {
    state.worker_ping()
}

#[tauri::command]
pub async fn import_txt_book(
    state: State<'_, AppState>,
    path: String,
    title: Option<String>,
) -> AppResult<ImportResult> {
    ensure_batch_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能导入 Book",
    )?;
    book_service::import_txt_book(&state.database()?, &path, title).await
}

#[tauri::command]
pub async fn list_books(state: State<'_, AppState>) -> AppResult<Vec<BookSummary>> {
    book_service::list_books(&state.database()?).await
}

#[tauri::command]
pub async fn get_book(state: State<'_, AppState>, book_id: String) -> AppResult<BookDetail> {
    book_service::get_book(&state.database()?, &book_id).await
}

#[tauri::command]
pub async fn list_chapters(
    state: State<'_, AppState>,
    book_id: String,
) -> AppResult<Vec<ChapterSummary>> {
    book_service::list_chapters(&state.database()?, &book_id).await
}

#[tauri::command]
pub async fn list_segments(
    state: State<'_, AppState>,
    chapter_id: String,
    offset: i64,
    limit: i64,
) -> AppResult<PaginatedSegments> {
    book_service::list_segments(&state.database()?, &chapter_id, offset, limit).await
}

#[tauri::command]
pub async fn get_segment(state: State<'_, AppState>, segment_id: String) -> AppResult<Segment> {
    book_service::get_segment(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn update_segment_reading_text(
    state: State<'_, AppState>,
    segment_id: String,
    reading_text: Option<String>,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    segment_edit_service::update_reading_text(
        &state.database()?,
        &segment_id,
        reading_text.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn restore_segment_reading_text(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    segment_edit_service::restore_reading_text(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn set_segment_speak_enabled(
    state: State<'_, AppState>,
    segment_id: String,
    speak_enabled: bool,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    segment_edit_service::set_speak_enabled(&state.database()?, &segment_id, speak_enabled).await
}

#[tauri::command]
pub async fn split_segment(
    state: State<'_, AppState>,
    segment_id: String,
    token_index: i64,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    if token_index < 0 {
        return Err(AppError::new("SEGMENT_SPLIT_INVALID", "分段光标不能小于 0"));
    }
    segment_edit_service::split_segment(&state.database()?, &segment_id, token_index as usize).await
}

#[tauri::command]
pub async fn merge_segment_with_previous(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    segment_edit_service::merge_with_previous(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn merge_segment_with_next(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentEditResult> {
    ensure_segment_edit_idle(
        &state,
        "BATCH_BOOK_MUTATION_LOCKED",
        "批量生成期间不能修改 Segment",
    )?;
    segment_edit_service::merge_with_next(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn delete_book(state: State<'_, AppState>, book_id: String) -> AppResult<()> {
    ensure_batch_idle(
        &state,
        "BOOK_GENERATION_IN_PROGRESS",
        "批量生成期间不能删除 Book",
    )?;
    let database = state.database()?;
    book_service::delete_book(&database, &book_id).await?;
    if let Some(data_dir) = state
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
        .clone()
    {
        if let Err(error) = audio_service::cleanup_book_audio(&data_dir, &book_id) {
            crate::append_app_log(
                &data_dir.join("logs/app.log"),
                "WARN",
                &format!("Book 音频目录清理失败: {error}"),
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_tts_settings(state: State<'_, AppState>) -> AppResult<TtsSettings> {
    settings_service::get_tts_settings(&state.database()?).await
}

#[tauri::command]
pub async fn save_tts_settings(
    state: State<'_, AppState>,
    provider: String,
    voice_type: i64,
    speed: f64,
    volume: f64,
    sample_rate: i64,
) -> AppResult<TtsSettings> {
    ensure_batch_idle(
        &state,
        "BATCH_SETTINGS_LOCKED",
        "批量生成期间不能修改 TTS 设置",
    )?;
    settings_service::save_tts_settings(
        &state.database()?,
        &TtsSettings {
            provider,
            voice_type,
            speed,
            volume,
            sample_rate,
            codec: "wav".to_string(),
        },
    )
    .await
}

#[tauri::command]
pub async fn get_reader_display_settings(
    state: State<'_, AppState>,
) -> AppResult<ReaderDisplaySettings> {
    settings_service::get_reader_display_settings(&state.database()?).await
}

#[tauri::command]
pub async fn save_reader_display_settings(
    state: State<'_, AppState>,
    font_size: i64,
    pinyin_mode: String,
) -> AppResult<ReaderDisplaySettings> {
    settings_service::save_reader_display_settings(
        &state.database()?,
        &ReaderDisplaySettings {
            font_size,
            pinyin_mode,
        },
    )
    .await
}

#[tauri::command]
pub fn get_tencent_voices() -> Vec<TencentVoice> {
    settings_service::tencent_voices()
}

#[tauri::command]
pub fn get_tts_credential_status(state: State<'_, AppState>) -> AppResult<CredentialStatus> {
    credentials::status(&state.data_directory()?)
}

#[tauri::command]
pub fn save_tencent_credentials(
    state: State<'_, AppState>,
    secret_id: String,
    secret_key: String,
) -> AppResult<CredentialStatus> {
    ensure_batch_idle(
        &state,
        "BATCH_SETTINGS_LOCKED",
        "批量生成期间不能修改 TTS 凭据",
    )?;
    credentials::save_tencent_credentials(&state.data_directory()?, &secret_id, &secret_key)?;
    state.restart_worker()?;
    credentials::status(&state.data_directory()?)
}

#[tauri::command]
pub fn delete_tencent_credentials(state: State<'_, AppState>) -> AppResult<CredentialStatus> {
    ensure_batch_idle(
        &state,
        "BATCH_SETTINGS_LOCKED",
        "批量生成期间不能修改 TTS 凭据",
    )?;
    credentials::delete_tencent_credentials(&state.data_directory()?)?;
    state.restart_worker()?;
    credentials::status(&state.data_directory()?)
}

#[tauri::command]
pub async fn test_tts_connection(
    state: State<'_, AppState>,
    voice_type: i64,
    sample_rate: i64,
) -> AppResult<Value> {
    ensure_batch_idle(
        &state,
        "TTS_BATCH_IN_PROGRESS",
        "批量生成期间不能测试 TTS 连接",
    )?;
    let model = voice_type.to_string();
    let result = state
        .worker_call_with_timeout(
            "tts.test_connection",
            json!({"provider": "tencent", "voice_type": voice_type, "sample_rate": sample_rate}),
            Some(Duration::from_secs(30)),
        )
        .map_err(|error| {
            if error.code == "WORKER_TIMEOUT" {
                AppError::new("TTS_TIMEOUT", "腾讯云 TTS 连接测试超时")
            } else {
                error
            }
        });
    let _ = usage_service::record_event(
        &state.database()?,
        "tencent",
        "tts",
        Some(&model),
        "connection_test",
        "characters",
        0,
        0,
        result.is_ok(),
        result.as_ref().err().map(|error| error.code.as_str()),
    )
    .await;
    result
}

#[tauri::command]
pub async fn get_api_usage_summary(
    state: State<'_, AppState>,
    range: String,
) -> AppResult<ApiUsageSummary> {
    usage_service::get_summary(&state.database()?, &range).await
}

#[tauri::command]
pub async fn generate_tts_preview(
    state: State<'_, AppState>,
    text: String,
    provider: String,
    voice_type: i64,
    speed: f64,
    volume: f64,
    sample_rate: i64,
) -> AppResult<TtsPreviewResult> {
    ensure_batch_idle(&state, "TTS_BATCH_IN_PROGRESS", "批量生成期间不能试听 TTS")?;
    {
        let mut busy = state
            .tts_busy
            .lock()
            .map_err(|_| AppError::new("TTS_BUSY_STATE_ERROR", "TTS 状态锁不可用"))?;
        if *busy {
            return Err(AppError::new(
                "TTS_ALREADY_RUNNING",
                "已有一个 TTS 请求正在运行",
            ));
        }
        *busy = true;
    }
    let settings = TtsSettings {
        provider,
        voice_type,
        speed,
        volume,
        sample_rate,
        codec: "wav".to_string(),
    };
    let result =
        preview_service::generate_preview(&state, &state.database()?, &text, &settings).await;
    if let Ok(mut busy) = state.tts_busy.lock() {
        *busy = false;
    }
    result
}

#[tauri::command]
pub async fn generate_segment_audio(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "TTS_BATCH_IN_PROGRESS",
        "批量生成期间不能单独生成 Segment 语音",
    )?;
    {
        let mut busy = state
            .tts_busy
            .lock()
            .map_err(|_| AppError::new("TTS_BUSY_STATE_ERROR", "TTS 状态锁不可用"))?;
        if *busy {
            return Err(AppError::new(
                "TTS_ALREADY_RUNNING",
                "已有一个 Segment 正在生成语音",
            ));
        }
        *busy = true;
    }
    let result = audio_service::generate_segment_audio(&state, &segment_id).await;
    if let Ok(mut busy) = state.tts_busy.lock() {
        *busy = false;
    }
    result
}

#[tauri::command]
pub async fn select_audio_version(
    state: State<'_, AppState>,
    audio_id: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "TTS_BATCH_IN_PROGRESS",
        "批量生成期间不能切换单句语音版本",
    )?;
    audio_service::select_audio_version(&state.database()?, &audio_id).await
}

#[tauri::command]
pub async fn analyze_segment_pronunciation(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音分析",
    )?;
    let database = state.database()?;
    let segment = book_service::get_segment(&database, &segment_id).await?;
    let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
    let analysis = state.worker_call(
        "pronunciation.analyze",
        json!({
            "segment_id": segment_id,
            "text": segment.effective_text(),
            "tokens": tokens,
        }),
    )?;
    pronunciation_service::apply_analysis(&database, &segment, &tokens, analysis).await
}

#[tauri::command]
pub async fn reanalyze_book_pronunciation(
    state: State<'_, AppState>,
    book_id: String,
) -> AppResult<BookPronunciationAnalysisResult> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音分析",
    )?;
    let database = state.database()?;
    book_service::get_book(&database, &book_id).await?;
    let chapters = book_service::list_chapters(&database, &book_id).await?;
    let mut total_segments = 0_i64;
    let mut analyzed_segments = 0_i64;
    for chapter in chapters {
        let mut offset = 0_i64;
        loop {
            let page =
                book_service::list_segments(&database, &chapter.chapter.id, offset, 100).await?;
            let page_count = page.items.len() as i64;
            total_segments += page_count;
            for segment in page.items {
                let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
                let analysis = state.worker_call(
                    "pronunciation.analyze",
                    json!({
                        "segment_id": segment.id,
                        "text": segment.effective_text(),
                        "tokens": tokens,
                    }),
                )?;
                pronunciation_service::apply_analysis(&database, &segment, &tokens, analysis)
                    .await?;
                analyzed_segments += 1;
            }
            offset += page_count;
            if offset >= page.total {
                break;
            }
        }
    }
    Ok(BookPronunciationAnalysisResult {
        book_id,
        total_segments,
        analyzed_segments,
    })
}

#[tauri::command]
pub async fn get_segment_reader(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentReader> {
    pronunciation_service::get_segment_reader(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn get_segment_display_pinyin(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<SegmentDisplayPinyin> {
    let database = state.database()?;
    let segment = book_service::get_segment(&database, &segment_id).await?;
    let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
    let result = state.worker_call(
        "pronunciation.display_pinyin",
        json!({
            "text": segment.effective_text(),
            "tokens": tokens,
        }),
    )?;
    let values = result
        .get("token_pinyin")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::new("WORKER_PROTOCOL_ERROR", "全文拼音结果缺少 token_pinyin"))?;
    if values.len() != tokens.len() {
        return Err(AppError::new(
            "WORKER_PROTOCOL_ERROR",
            "全文拼音结果与 token 数量不一致",
        ));
    }
    let token_pinyin = values
        .iter()
        .map(|value| match value {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(value.clone())),
            _ => Err(AppError::new(
                "WORKER_PROTOCOL_ERROR",
                "全文拼音结果包含无效值",
            )),
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(SegmentDisplayPinyin { token_pinyin })
}

#[tauri::command]
pub async fn list_segment_annotations(
    state: State<'_, AppState>,
    segment_id: String,
) -> AppResult<Vec<Annotation>> {
    pronunciation_service::list_annotations(&state.database()?, &segment_id).await
}

#[tauri::command]
pub async fn confirm_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
    target_pinyin: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音标注",
    )?;
    pronunciation_service::confirm_annotation(&state.database()?, &annotation_id, &target_pinyin)
        .await
}

#[tauri::command]
pub async fn ignore_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音标注",
    )?;
    pronunciation_service::ignore_annotation(&state.database()?, &annotation_id).await
}

#[tauri::command]
pub async fn reset_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音标注",
    )?;
    pronunciation_service::reset_annotation(&state.database()?, &annotation_id).await
}

#[tauri::command]
pub async fn create_manual_annotation(
    state: State<'_, AppState>,
    segment_id: String,
    token_index: i64,
    target_pinyin: String,
) -> AppResult<SegmentReader> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音标注",
    )?;
    if token_index < 0 {
        return Err(AppError::new(
            "INVALID_ANALYSIS_RESPONSE",
            "token_index 不能小于 0",
        ));
    }
    pronunciation_service::create_manual_annotation(
        &state.database()?,
        &segment_id,
        token_index as usize,
        &target_pinyin,
    )
    .await
}

#[tauri::command]
pub async fn create_pronunciation_rule(
    state: State<'_, AppState>,
    scope: String,
    book_id: Option<String>,
    pattern_text: String,
    target_pinyin: String,
    rule_type: Option<String>,
    source: Option<String>,
) -> AppResult<PronunciationRuleMutation> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::create_rule(
        &state.database()?,
        &scope,
        book_id.as_deref(),
        &pattern_text,
        &target_pinyin,
        rule_type.as_deref().unwrap_or("manual"),
        source.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn create_rule_from_annotation(
    state: State<'_, AppState>,
    annotation_id: String,
    scope: String,
) -> AppResult<PronunciationRuleMutation> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::create_rule_from_annotation(
        &state.database()?,
        &annotation_id,
        &scope,
    )
    .await
}

#[tauri::command]
pub async fn update_pronunciation_rule(
    state: State<'_, AppState>,
    rule_id: String,
    pattern_text: String,
    target_pinyin: String,
    rule_type: Option<String>,
    source: Option<String>,
) -> AppResult<PronunciationRuleMutation> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::update_rule(
        &state.database()?,
        &rule_id,
        &pattern_text,
        &target_pinyin,
        rule_type.as_deref().unwrap_or("manual"),
        source.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn enable_pronunciation_rule(
    state: State<'_, AppState>,
    rule_id: String,
) -> AppResult<PronunciationRuleMutation> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::set_rule_enabled(&state.database()?, &rule_id, true).await
}

#[tauri::command]
pub async fn disable_pronunciation_rule(
    state: State<'_, AppState>,
    rule_id: String,
) -> AppResult<PronunciationRuleMutation> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::set_rule_enabled(&state.database()?, &rule_id, false).await
}

#[tauri::command]
pub async fn get_pronunciation_rule(
    state: State<'_, AppState>,
    rule_id: String,
) -> AppResult<PronunciationRule> {
    pronunciation_rule_service::get_rule(&state.database()?, &rule_id).await
}

#[tauri::command]
pub async fn list_book_pronunciation_rules(
    state: State<'_, AppState>,
    book_id: String,
) -> AppResult<Vec<PronunciationRule>> {
    pronunciation_rule_service::list_book_rules(&state.database()?, &book_id).await
}

#[tauri::command]
pub async fn list_global_pronunciation_rules(
    state: State<'_, AppState>,
) -> AppResult<Vec<PronunciationRule>> {
    pronunciation_rule_service::list_global_rules(&state.database()?).await
}

#[tauri::command]
pub async fn apply_pronunciation_rules_to_book(
    state: State<'_, AppState>,
    book_id: String,
) -> AppResult<PronunciationRuleApplyResult> {
    ensure_batch_idle(
        &state,
        "BATCH_PRONUNCIATION_LOCKED",
        "批量生成期间不能修改发音词典",
    )?;
    pronunciation_rule_service::apply_pronunciation_rules_to_book(&state.database()?, &book_id)
        .await
}

#[tauri::command]
pub async fn get_book_generation_preflight(
    state: State<'_, AppState>,
    book_id: String,
) -> AppResult<BookGenerationPreflight> {
    batch_generation_service::get_book_generation_preflight(
        &state.database()?,
        &book_id,
        &state.data_directory()?,
    )
    .await
}

#[tauri::command]
pub async fn start_book_audio_generation(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
) -> AppResult<()> {
    if state.export.is_running() {
        return Err(AppError::new(
            "EXPORT_IN_PROGRESS",
            "完整音频导出期间不能生成全文语音",
        ));
    }
    let database = state.database()?;
    {
        let mut busy = state
            .tts_busy
            .lock()
            .map_err(|_| AppError::new("TTS_BUSY_STATE_ERROR", "TTS 状态锁不可用"))?;
        if *busy {
            return Err(AppError::new(
                "TTS_ALREADY_RUNNING",
                "已有一个 TTS 生成操作正在运行",
            ));
        }
        *busy = true;
    }
    let result = batch_generation_service::start_book_audio_generation(
        app,
        state.batch.clone(),
        database,
        book_id,
        state.data_directory()?,
    )
    .await;
    if result.is_err() {
        if let Ok(mut busy) = state.tts_busy.lock() {
            *busy = false;
        }
    }
    result
}

#[tauri::command]
pub fn cancel_book_audio_generation(
    state: State<'_, AppState>,
    app: AppHandle,
) -> AppResult<BatchGenerationState> {
    batch_generation_service::cancel_book_audio_generation(&app, &state.batch)
}

#[tauri::command]
pub fn get_batch_generation_state(state: State<'_, AppState>) -> BatchGenerationState {
    batch_generation_service::get_batch_generation_state(&state.batch)
}

#[tauri::command]
pub fn check_ffmpeg(app: AppHandle) -> AppResult<FfmpegStatus> {
    let info = crate::audio::ffmpeg::check_available(&app, false)?;
    Ok(FfmpegStatus {
        available: true,
        version: Some(info.version),
    })
}

#[tauri::command]
pub async fn get_book_export_preflight(
    state: State<'_, AppState>,
    book_id: String,
    segment_ids: Option<Vec<String>>,
) -> AppResult<BookExportPreflight> {
    export_service::get_book_export_preflight(&state.database()?, &book_id, segment_ids.as_deref())
        .await
}

#[tauri::command]
pub async fn export_book_audio(
    state: State<'_, AppState>,
    app: AppHandle,
    book_id: String,
    destination_path: String,
    format: String,
    overwrite: bool,
    segment_ids: Option<Vec<String>>,
) -> AppResult<()> {
    if state.export.is_running() {
        return Err(AppError::new(
            "EXPORT_ALREADY_RUNNING",
            "已有一个完整音频导出正在运行",
        ));
    }
    if state.batch.is_running() {
        return Err(AppError::new(
            "BATCH_GENERATION_IN_PROGRESS",
            "全文语音生成期间不能导出完整音频",
        ));
    }
    {
        let mut busy = state
            .tts_busy
            .lock()
            .map_err(|_| AppError::new("TTS_BUSY_STATE_ERROR", "TTS 状态锁不可用"))?;
        if *busy {
            return Err(AppError::new(
                "TTS_ALREADY_RUNNING",
                "已有一个 TTS 生成操作正在运行",
            ));
        }
        *busy = true;
    }
    let database = match state.database() {
        Ok(database) => database,
        Err(error) => {
            if let Ok(mut busy) = state.tts_busy.lock() {
                *busy = false;
            }
            return Err(error);
        }
    };
    let result = export_service::start_export(
        app,
        state.export.clone(),
        database,
        book_id,
        destination_path,
        format,
        overwrite,
        segment_ids,
    )
    .await;
    if result.is_err() {
        if let Ok(mut busy) = state.tts_busy.lock() {
            *busy = false;
        }
    }
    result
}

#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>, app: AppHandle) -> AppResult<ExportState> {
    export_service::cancel_export(&app, &state.export)
}

#[tauri::command]
pub fn get_export_state(state: State<'_, AppState>) -> ExportState {
    export_service::get_export_state(&state.export)
}

fn ensure_batch_idle(state: &AppState, code: &str, message: &str) -> AppResult<()> {
    if state.export.is_running() {
        return Err(AppError::new(
            "EXPORT_IN_PROGRESS",
            "完整音频导出期间不能修改当前文档或发音设置",
        ));
    }
    if state.batch.is_running() {
        return Err(AppError::new(code, message));
    }
    Ok(())
}

fn ensure_segment_edit_idle(state: &AppState, code: &str, message: &str) -> AppResult<()> {
    ensure_batch_idle(state, code, message)?;
    if *state
        .tts_busy
        .lock()
        .map_err(|_| AppError::new("TTS_BUSY_STATE_ERROR", "TTS 状态锁不可用"))?
    {
        return Err(AppError::new(
            "TTS_IN_PROGRESS",
            "语音生成期间不能修改 Segment",
        ));
    }
    Ok(())
}

impl AppState {
    pub fn database(&self) -> AppResult<Database> {
        self.db
            .lock()
            .map_err(|_| AppError::new("DB_ERROR", "数据库状态锁不可用"))?
            .clone()
            .ok_or_else(|| AppError::new("DB_ERROR", "数据库尚未初始化"))
    }

    pub fn worker_ping(&self) -> AppResult<WorkerPing> {
        let started = Instant::now();
        let result = self.worker_call("system.ping", json!({}))?;
        let version = result
            .get("version")
            .and_then(|value| value.as_str())
            .ok_or_else(|| AppError::new("WORKER_PROTOCOL_ERROR", "system.ping 缺少 version"))?;
        Ok(WorkerPing {
            version: version.to_string(),
            elapsed_ms: started.elapsed().as_millis(),
        })
    }

    pub fn worker_call(&self, method: &str, params: Value) -> AppResult<Value> {
        self.worker_call_with_timeout(method, params, None)
    }

    pub fn worker_call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
    ) -> AppResult<Value> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "Worker 状态锁不可用"))?;
        let worker = worker
            .as_mut()
            .ok_or_else(|| AppError::new("WORKER_NOT_RUNNING", "Python Worker 尚未启动"))?;
        match timeout {
            Some(timeout) => worker.call_with_timeout(method, params, timeout),
            None => worker.call(method, params),
        }
    }
}
