use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{AudioVersion, SegmentReader, TtsSettings},
    services::{book_service, pronunciation_service, settings_service, usage_service},
    AppState,
};
use serde_json::Value;
use std::{fs, io::Read, path::Path, time::Duration};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub(crate) const ALLOWED_STATUS: &[&str] = &["analyzed", "ready", "generated"];

pub async fn generate_segment_audio(
    state: &AppState,
    segment_id: &str,
) -> AppResult<SegmentReader> {
    let database = state.database()?;
    let settings = settings_service::get_tts_settings(&database).await?;
    generate_segment_audio_with_settings(
        state,
        &database,
        segment_id,
        &settings,
        "segment_generation",
    )
    .await
}

pub(crate) async fn generate_segment_audio_with_settings(
    state: &AppState,
    database: &Database,
    segment_id: &str,
    settings: &TtsSettings,
    operation: &str,
) -> AppResult<SegmentReader> {
    let segment = book_service::get_segment(&database, segment_id).await?;
    if !segment.speak_enabled {
        return Err(AppError::new(
            "SEGMENT_SPEAK_DISABLED",
            "当前 Segment 已设置为不参与朗读",
        ));
    }
    if segment.status == "pending" {
        return Err(AppError::new(
            "SEGMENT_PRONUNCIATION_PENDING",
            "请先进行发音分析",
        ));
    }
    if segment.status == "needs_review" {
        return Err(AppError::new(
            "SEGMENT_HAS_UNRESOLVED_PRONUNCIATION",
            "仍有发音待确认，请处理后再生成语音",
        ));
    }
    if !ALLOWED_STATUS.contains(&segment.status.as_str()) {
        return Err(AppError::new(
            "SEGMENT_NOT_READY_FOR_TTS",
            "当前 Segment 不能生成语音",
        ));
    }

    let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
    let pronunciations = pronunciation_service::confirmed_overrides(&database, segment_id).await?;
    let pronunciation_signature = pronunciation_service::serialize_pronunciation_signature(
        &pronunciation_service::effective_signature(database, segment_id).await?,
    )?;
    let book_id = book_service::get_book_id_for_segment(&database, segment_id).await?;
    let data_dir = state
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
        .clone()
        .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用目录尚未初始化"))?;
    let audio_dir = data_dir
        .join("projects")
        .join(book_id)
        .join("audio")
        .join(segment_id);
    fs::create_dir_all(&audio_dir).map_err(AppError::from)?;
    let generation_id = Uuid::now_v7().to_string();
    let temp_path = audio_dir.join(format!(".tmp-{generation_id}.wav"));
    let params = serde_json::json!({
        "provider": settings.provider,
        "text": segment.effective_text(),
        "tokens": tokens,
        "pronunciations": pronunciations,
        "voice_type": settings.voice_type,
        "sample_rate": settings.sample_rate,
        "codec": settings.codec,
        "speed": settings.speed,
        "volume": settings.volume,
        "session_id": generation_id,
        "output_path": temp_path,
    });

    let worker_result =
        state.worker_call_with_timeout("tts.synthesize", params, Some(Duration::from_secs(60)));
    let _ = usage_service::record_event(
        database,
        &settings.provider,
        "tts",
        Some(&settings.voice_type.to_string()),
        operation,
        "characters",
        segment.effective_text().chars().count() as i64,
        0,
        worker_result.is_ok(),
        worker_result
            .as_ref()
            .err()
            .map(|error| error.code.as_str()),
    )
    .await;
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
    let result = match response.as_object() {
        Some(result) => result,
        None => {
            remove_file_quietly(&temp_path);
            return Err(AppError::new(
                "WORKER_PROTOCOL_ERROR",
                "tts.synthesize result 不是对象",
            ));
        }
    };
    let output_path = match result.get("output_path").and_then(Value::as_str) {
        Some(output_path) => output_path,
        None => {
            remove_file_quietly(&temp_path);
            return Err(AppError::new(
                "WORKER_PROTOCOL_ERROR",
                "tts.synthesize 缺少 output_path",
            ));
        }
    };
    if output_path != temp_path.to_string_lossy() {
        remove_file_quietly(&temp_path);
        return Err(AppError::new(
            "WORKER_PROTOCOL_ERROR",
            "Worker output_path 与临时路径不一致",
        ));
    }
    validate_wav(&temp_path)?;
    let provider_request_id = result
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let provider_session_id = result
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let ssml = result
        .get("ssml")
        .and_then(Value::as_str)
        .map(str::to_string);
    let duration_ms = result.get("duration_ms").and_then(Value::as_i64);
    let version_no: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version_no), 0) + 1 FROM audio_versions WHERE segment_id = ?",
    )
    .bind(segment_id)
    .fetch_one(database.pool())
    .await?;
    let final_path = audio_dir.join(format!("v{version_no}.wav"));
    if let Err(error) = fs::rename(&temp_path, &final_path) {
        remove_file_quietly(&temp_path);
        return Err(AppError::new(
            "FILE_IO_ERROR",
            format!("音频文件落盘失败: {error}"),
        ));
    }
    let now = now_rfc3339()?;
    let audio_id = Uuid::now_v7().to_string();
    let transaction_result = async {
        let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
        sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, ssml, pronunciation_signature, audio_path, provider_request_id, provider_session_id, duration_ms, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&audio_id).bind(segment_id).bind(version_no).bind(&settings.provider).bind(settings.voice_type)
            .bind(settings.sample_rate).bind(&settings.codec).bind(settings.speed).bind(settings.volume).bind(ssml).bind(pronunciation_signature)
            .bind(final_path.to_string_lossy().to_string()).bind(provider_request_id).bind(provider_session_id).bind(duration_ms).bind(&now)
            .execute(&mut *transaction).await.map_err(transaction_error)?;
        sqlx::query("UPDATE segments SET current_audio_id = ?, status = 'generated', updated_at = ? WHERE id = ?")
            .bind(&audio_id).bind(&now).bind(segment_id).execute(&mut *transaction).await.map_err(transaction_error)?;
        transaction.commit().await.map_err(transaction_error)
    }.await;
    if let Err(error) = transaction_result {
        remove_file_quietly(&final_path);
        return Err(error);
    }
    pronunciation_service::get_segment_reader(&database, segment_id).await
}

pub(crate) async fn latest_audio_matches(
    database: &Database,
    segment_id: &str,
    settings: &TtsSettings,
) -> AppResult<bool> {
    let row = sqlx::query_as::<_, (String, i64, i64, String, f64, f64, String)>(
        "SELECT provider, voice_type, sample_rate, codec, speed, volume, audio_path
         FROM audio_versions
         WHERE segment_id = ?
         ORDER BY version_no DESC
         LIMIT 1",
    )
    .bind(segment_id)
    .fetch_optional(database.pool())
    .await?;
    let Some((provider, voice_type, sample_rate, codec, speed, volume, audio_path)) = row else {
        return Ok(false);
    };
    Ok(Path::new(&audio_path).is_file()
        && provider == settings.provider
        && voice_type == settings.voice_type
        && sample_rate == settings.sample_rate
        && codec == settings.codec
        && speed == settings.speed
        && volume == settings.volume)
}

pub async fn list_audio_versions(
    database: &Database,
    segment_id: &str,
) -> AppResult<Vec<AudioVersion>> {
    let _ = book_service::get_segment(database, segment_id).await?;
    let rows = sqlx::query_as::<_, (String, String, i64, String, i64, i64, String, f64, f64, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<i64>, String)>(
        "SELECT id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, ssml, pronunciation_signature, audio_path, provider_request_id, provider_session_id, duration_ms, created_at FROM audio_versions WHERE segment_id = ? ORDER BY version_no DESC")
        .bind(segment_id).fetch_all(database.pool()).await?;
    rows.into_iter().map(audio_from_row).collect()
}

pub async fn select_audio_version(database: &Database, audio_id: &str) -> AppResult<SegmentReader> {
    let (segment_id, audio_path): (String, String) =
        sqlx::query_as("SELECT segment_id, audio_path FROM audio_versions WHERE id = ?")
            .bind(audio_id)
            .fetch_optional(database.pool())
            .await?
            .ok_or_else(|| AppError::new("AUDIO_VERSION_NOT_FOUND", "语音版本不存在"))?;
    let segment = book_service::get_segment(database, &segment_id).await?;
    if (segment.current_audio_id.is_none() && segment.status != "generated")
        || segment.status == "pending"
        || segment.status == "needs_review"
    {
        return Err(AppError::new(
            "AUDIO_VERSION_STALE",
            "当前 Segment 的文本或发音分析已变化，旧语音不能重新设为当前音频",
        ));
    }
    if !Path::new(&audio_path).is_file() {
        return Err(AppError::new(
            "AUDIO_FILE_NOT_FOUND",
            "语音版本文件不存在，无法选择播放",
        ));
    }
    let now = now_rfc3339()?;
    let result =
        sqlx::query("UPDATE segments SET current_audio_id = ?, updated_at = ? WHERE id = ?")
            .bind(audio_id)
            .bind(now)
            .bind(&segment_id)
            .execute(database.pool())
            .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"));
    }
    crate::services::pronunciation_service::get_segment_reader(database, &segment_id).await
}

pub fn validate_wav(path: &Path) -> AppResult<u64> {
    let metadata = fs::metadata(path).map_err(|error| {
        AppError::new("INVALID_AUDIO_OUTPUT", format!("音频文件不存在: {error}"))
    })?;
    if metadata.len() < 44 {
        remove_file_quietly(path);
        return Err(AppError::new("INVALID_AUDIO_OUTPUT", "WAV 文件过小"));
    }
    let mut file = fs::File::open(path)
        .map_err(|error| AppError::new("INVALID_AUDIO_OUTPUT", error.to_string()))?;
    let mut header = [0_u8; 12];
    file.read_exact(&mut header).map_err(|error| {
        remove_file_quietly(path);
        AppError::new("INVALID_AUDIO_OUTPUT", error.to_string())
    })?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        remove_file_quietly(path);
        return Err(AppError::new(
            "INVALID_AUDIO_OUTPUT",
            "输出文件不是有效 WAV",
        ));
    }
    Ok(metadata.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WavFormat {
    pub(crate) audio_format: u16,
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) bits_per_sample: u16,
}

pub(crate) fn read_wav_format(path: &Path) -> AppResult<WavFormat> {
    let bytes = fs::read(path).map_err(|error| {
        AppError::new("AUDIO_FILE_MISSING", format!("无法读取 WAV 文件: {error}"))
    })?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(AppError::new(
            "AUDIO_FORMAT_MISMATCH",
            "音频文件不是有效 RIFF/WAVE",
        ));
    }
    let mut cursor = 12_usize;
    let mut format = None;
    let mut has_data = false;
    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[cursor + 4],
            bytes[cursor + 5],
            bytes[cursor + 6],
            bytes[cursor + 7],
        ]) as usize;
        let data_start = cursor + 8;
        let data_end = data_start
            .checked_add(chunk_size)
            .ok_or_else(|| AppError::new("AUDIO_FORMAT_MISMATCH", "WAV chunk 长度无效"))?;
        if data_end > bytes.len() {
            return Err(AppError::new(
                "AUDIO_FORMAT_MISMATCH",
                "WAV chunk 超出文件范围",
            ));
        }
        if chunk_id == b"fmt " {
            if chunk_size < 16 {
                return Err(AppError::new(
                    "AUDIO_FORMAT_MISMATCH",
                    "WAV fmt chunk 不完整",
                ));
            }
            format = Some(WavFormat {
                audio_format: u16::from_le_bytes([bytes[data_start], bytes[data_start + 1]]),
                channels: u16::from_le_bytes([bytes[data_start + 2], bytes[data_start + 3]]),
                sample_rate: u32::from_le_bytes([
                    bytes[data_start + 4],
                    bytes[data_start + 5],
                    bytes[data_start + 6],
                    bytes[data_start + 7],
                ]),
                bits_per_sample: u16::from_le_bytes([
                    bytes[data_start + 14],
                    bytes[data_start + 15],
                ]),
            });
        } else if chunk_id == b"data" {
            has_data = chunk_size > 0;
        }
        cursor = data_end
            .checked_add(chunk_size % 2)
            .ok_or_else(|| AppError::new("AUDIO_FORMAT_MISMATCH", "WAV chunk 长度无效"))?;
    }
    if !has_data {
        return Err(AppError::new(
            "AUDIO_FORMAT_MISMATCH",
            "WAV 缺少有效 data chunk",
        ));
    }
    let format =
        format.ok_or_else(|| AppError::new("AUDIO_FORMAT_MISMATCH", "WAV 缺少 fmt chunk"))?;
    if format.audio_format == 0
        || format.channels == 0
        || format.sample_rate == 0
        || format.bits_per_sample == 0
    {
        return Err(AppError::new("AUDIO_FORMAT_MISMATCH", "WAV 音频参数无效"));
    }
    Ok(format)
}

pub fn cleanup_book_audio(data_dir: &Path, book_id: &str) -> AppResult<()> {
    let path = data_dir.join("projects").join(book_id).join("audio");
    if path.exists() {
        fs::remove_dir_all(path).map_err(AppError::from)?;
    }
    Ok(())
}

fn audio_from_row(
    row: (
        String,
        String,
        i64,
        String,
        i64,
        i64,
        String,
        f64,
        f64,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        String,
    ),
) -> AppResult<AudioVersion> {
    let (
        id,
        segment_id,
        version_no,
        provider,
        voice_type,
        sample_rate,
        codec,
        speed,
        volume,
        ssml,
        pronunciation_signature,
        audio_path,
        provider_request_id,
        provider_session_id,
        duration_ms,
        created_at,
    ) = row;
    Ok(AudioVersion {
        id,
        segment_id,
        version_no,
        provider,
        voice_type,
        sample_rate,
        codec,
        speed,
        volume,
        ssml,
        pronunciation_signature,
        audio_path,
        provider_request_id,
        provider_session_id,
        duration_ms,
        created_at,
    })
}

fn remove_file_quietly(path: &Path) {
    let _ = fs::remove_file(path);
}

fn now_rfc3339() -> AppResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("DB_ERROR", error.to_string()))
}
fn transaction_error(error: sqlx::Error) -> AppError {
    AppError::new("DB_TRANSACTION_FAILED", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        latest_audio_matches, list_audio_versions, read_wav_format, select_audio_version,
        validate_wav,
    };
    use crate::db::Database;
    use crate::models::TtsSettings;
    use std::{fs, io::Write};
    use uuid::Uuid;

    #[test]
    fn validates_riff_wave_header_and_rejects_invalid_audio() {
        let path = std::env::temp_dir().join(format!("ancient-tts-{}.wav", Uuid::now_v7()));
        let mut file = fs::File::create(&path).expect("file");
        let mut header = [0_u8; 44];
        header[0..4].copy_from_slice(b"RIFF");
        header[8..12].copy_from_slice(b"WAVE");
        file.write_all(&header).expect("write");
        assert_eq!(validate_wav(&path).expect("wav"), 44);
        assert!(read_wav_format(&path).is_err());
        fs::write(&path, b"not wav").expect("rewrite");
        assert!(validate_wav(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn lists_versions_and_selects_current_audio() {
        let database_path =
            std::env::temp_dir().join(format!("ancient-tts-audio-{}.sqlite", Uuid::now_v7()));
        let database = tauri::async_runtime::block_on(Database::open(&database_path)).expect("db");
        let book_id = Uuid::now_v7().to_string();
        let chapter_id = Uuid::now_v7().to_string();
        let segment_id = Uuid::now_v7().to_string();
        let audio_dir = std::env::temp_dir().join(format!("ancient-tts-audio-{}", Uuid::now_v7()));
        fs::create_dir_all(&audio_dir).expect("audio dir");
        let first_path = audio_dir.join("v1.wav");
        let second_path = audio_dir.join("v2.wav");
        let valid_wav = {
            let mut bytes = vec![0_u8; 44];
            bytes[0..4].copy_from_slice(b"RIFF");
            bytes[8..12].copy_from_slice(b"WAVE");
            bytes
        };
        fs::write(&first_path, &valid_wav).expect("first audio");
        fs::write(&second_path, &valid_wav).expect("second audio");

        tauri::async_runtime::block_on(async {
            let pool = database.pool();
            for query in [
                "INSERT INTO books (id, title, created_at, updated_at) VALUES (?, '测试', 'now', 'now')",
                "INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, '正文', 0, 'now', 'now')",
                "INSERT INTO segments (id, chapter_id, order_index, original_text, status, created_at, updated_at) VALUES (?, ?, 0, '测试', 'generated', 'now', 'now')",
            ] {
                let mut statement = sqlx::query(query);
                statement = match query {
                    query if query.starts_with("INSERT INTO books") => statement.bind(&book_id),
                    query if query.starts_with("INSERT INTO chapters") => statement.bind(&chapter_id).bind(&book_id),
                    _ => statement.bind(&segment_id).bind(&chapter_id),
                };
                statement.execute(pool).await.expect("seed row");
            }
            for (id, version_no, path) in [
                (Uuid::now_v7().to_string(), 1_i64, &first_path),
                (Uuid::now_v7().to_string(), 2_i64, &second_path),
            ] {
                sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, audio_path, created_at) VALUES (?, ?, ?, 'tencent', 501000, 16000, 'wav', 0, 0, ?, 'now')")
                    .bind(&id).bind(&segment_id).bind(version_no).bind(path.to_string_lossy().to_string())
                    .execute(pool).await.expect("audio row");
            }
            let settings = TtsSettings {
                provider: "tencent".to_string(),
                voice_type: 501000,
                sample_rate: 16000,
                codec: "wav".to_string(),
                speed: 0.0,
                volume: 0.0,
            };
            assert!(latest_audio_matches(&database, &segment_id, &settings)
                .await
                .expect("latest audio should match"));
            let mut changed_settings = settings.clone();
            changed_settings.speed = 1.0;
            assert!(
                !latest_audio_matches(&database, &segment_id, &changed_settings)
                    .await
                    .expect("changed settings should not match")
            );
            let versions = list_audio_versions(&database, &segment_id)
                .await
                .expect("list versions");
            assert_eq!(
                versions
                    .iter()
                    .map(|version| version.version_no)
                    .collect::<Vec<_>>(),
                vec![2, 1]
            );
            let selected = select_audio_version(&database, &versions[0].id)
                .await
                .expect("select version");
            assert_eq!(
                selected.segment.current_audio_id.as_deref(),
                Some(versions[0].id.as_str())
            );
        });
        let _ = fs::remove_dir_all(audio_dir);
        let _ = fs::remove_file(database_path);
    }
}
