use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{TtsPreviewResult, TtsSettings},
    services::{audio_service, pronunciation_service, usage_service},
    AppState,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const MAX_PREVIEW_CHARACTERS: usize = 200;

pub fn preview_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cache").join("tts-preview")
}

pub fn cleanup_preview_cache(data_dir: &Path) -> AppResult<()> {
    let path = preview_cache_dir(data_dir);
    if path.exists() {
        fs::remove_dir_all(&path).map_err(AppError::from)?;
    }
    fs::create_dir_all(path).map_err(AppError::from)
}

pub async fn generate_preview(
    state: &AppState,
    database: &Database,
    text: &str,
    settings: &TtsSettings,
) -> AppResult<TtsPreviewResult> {
    validate_preview_text(text)?;
    crate::services::settings_service::validate(settings)?;
    let data_dir = state
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
        .clone()
        .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用目录尚未初始化"))?;
    let cache_dir = preview_cache_dir(&data_dir);
    fs::create_dir_all(&cache_dir).map_err(AppError::from)?;
    let preview_id = Uuid::now_v7().to_string();
    let temp_path = cache_dir.join(format!(".tmp-{preview_id}.wav"));
    let final_path = cache_dir.join(format!("preview-{preview_id}.wav"));
    let tokens = pronunciation_service::grapheme_tokens(text);
    let params = serde_json::json!({
        "provider": settings.provider,
        "text": text,
        "tokens": tokens,
        "pronunciations": [],
        "voice_type": settings.voice_type,
        "sample_rate": settings.sample_rate,
        "codec": settings.codec,
        "speed": settings.speed,
        "volume": settings.volume,
        "session_id": preview_id,
        "output_path": temp_path,
    });
    let worker_result =
        state.worker_call_with_timeout("tts.synthesize", params, Some(Duration::from_secs(60)));
    let usage_result = match &worker_result {
        Ok(_) => {
            usage_service::record_event(
                database,
                &settings.provider,
                "tts",
                Some(&settings.voice_type.to_string()),
                "voice_preview",
                "characters",
                text.chars().count() as i64,
                0,
                true,
                None,
            )
            .await
        }
        Err(error) => {
            usage_service::record_event(
                database,
                &settings.provider,
                "tts",
                Some(&settings.voice_type.to_string()),
                "voice_preview",
                "characters",
                text.chars().count() as i64,
                0,
                false,
                Some(&error.code),
            )
            .await
        }
    };
    let _ = usage_result;
    let response = match worker_result {
        Ok(response) => response,
        Err(error) => {
            remove_file_quietly(&temp_path);
            return Err(if error.code == "WORKER_TIMEOUT" {
                AppError::new("TTS_TIMEOUT", "腾讯云 TTS 请求超时")
            } else {
                error
            });
        }
    };
    let output_path = match response
        .as_object()
        .and_then(|object| object.get("output_path"))
        .and_then(Value::as_str)
    {
        Some(output_path) => output_path,
        None => {
            remove_file_quietly(&temp_path);
            return Err(AppError::new(
                "WORKER_PROTOCOL_ERROR",
                "试听结果缺少 output_path",
            ));
        }
    };
    if output_path != temp_path.to_string_lossy() {
        remove_file_quietly(&temp_path);
        return Err(AppError::new(
            "WORKER_PROTOCOL_ERROR",
            "Worker output_path 与试听临时路径不一致",
        ));
    }
    if let Err(error) = audio_service::validate_wav(&temp_path) {
        remove_file_quietly(&temp_path);
        return Err(error);
    }
    fs::rename(&temp_path, &final_path).map_err(|error| {
        remove_file_quietly(&temp_path);
        AppError::new("FILE_IO_ERROR", format!("试听音频落盘失败: {error}"))
    })?;
    remove_previous_previews(&cache_dir, &final_path);
    Ok(TtsPreviewResult {
        audio_path: final_path.to_string_lossy().to_string(),
        duration_ms: response
            .as_object()
            .and_then(|object| object.get("duration_ms"))
            .and_then(Value::as_i64),
        voice_type: settings.voice_type,
        speed: settings.speed,
        volume: settings.volume,
        sample_rate: settings.sample_rate,
    })
}

pub fn validate_preview_text(text: &str) -> AppResult<()> {
    let character_count = text.chars().count();
    if text.trim().is_empty() {
        return Err(AppError::new("TTS_PREVIEW_TEXT_EMPTY", "试听文本不能为空"));
    }
    if character_count > MAX_PREVIEW_CHARACTERS {
        return Err(AppError::new(
            "TTS_PREVIEW_TEXT_TOO_LONG",
            format!("试听文本不能超过 {MAX_PREVIEW_CHARACTERS} 个汉字"),
        ));
    }
    Ok(())
}

fn remove_previous_previews(cache_dir: &Path, current: &Path) {
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != current && path.extension().and_then(|value| value.to_str()) == Some("wav") {
            remove_file_quietly(&path);
        }
    }
}

fn remove_file_quietly(path: &Path) {
    let _ = fs::remove_file(path);
}

#[cfg(test)]
mod tests {
    use super::validate_preview_text;

    #[test]
    fn preview_text_has_a_two_hundred_character_limit() {
        assert!(validate_preview_text("上古之人").is_ok());
        assert_eq!(
            validate_preview_text(&"甲".repeat(201))
                .expect_err("long preview should fail")
                .code,
            "TTS_PREVIEW_TEXT_TOO_LONG"
        );
    }
}
