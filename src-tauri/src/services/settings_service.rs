use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ReaderDisplaySettings, TencentVoice, TtsSettings},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const PROVIDER_KEY: &str = "tts.provider";
const VOICE_KEY: &str = "tts.tencent.voice_type";
const SPEED_KEY: &str = "tts.tencent.speed";
const VOLUME_KEY: &str = "tts.tencent.volume";
const SAMPLE_RATE_KEY: &str = "tts.tencent.sample_rate";
const READER_FONT_SIZE_KEY: &str = "reader.display.font_size";
const READER_PINYIN_MODE_KEY: &str = "reader.display.pinyin_mode";

pub const DEFAULT_TTS_SETTINGS: TtsSettings = TtsSettings {
    provider: String::new(),
    voice_type: 501000,
    speed: 0.0,
    volume: 0.0,
    sample_rate: 16000,
    codec: String::new(),
};

pub const DEFAULT_READER_FONT_SIZE: i64 = 25;
pub const DEFAULT_READER_PINYIN_MODE: &str = "risky";

pub fn default_reader_display_settings() -> ReaderDisplaySettings {
    ReaderDisplaySettings {
        font_size: DEFAULT_READER_FONT_SIZE,
        pinyin_mode: DEFAULT_READER_PINYIN_MODE.to_string(),
    }
}

pub fn tencent_voices() -> Vec<TencentVoice> {
    vec![
        TencentVoice {
            id: 501000,
            name: "智斌".to_string(),
            category: "大模型·阅读".to_string(),
            description: "阅读男声".to_string(),
        },
        TencentVoice {
            id: 501002,
            name: "智菊".to_string(),
            category: "大模型·阅读".to_string(),
            description: "阅读女声".to_string(),
        },
        TencentVoice {
            id: 501003,
            name: "智宇".to_string(),
            category: "大模型·阅读".to_string(),
            description: "阅读男声".to_string(),
        },
        TencentVoice {
            id: 601013,
            name: "爱小伊".to_string(),
            category: "大模型·阅读".to_string(),
            description: "阅读女声".to_string(),
        },
        TencentVoice {
            id: 101030,
            name: "智柯".to_string(),
            category: "精品·通用".to_string(),
            description: "通用男声".to_string(),
        },
        TencentVoice {
            id: 101055,
            name: "智付".to_string(),
            category: "精品·通用".to_string(),
            description: "通用女声".to_string(),
        },
    ]
}

pub async fn get_tts_settings(database: &Database) -> AppResult<TtsSettings> {
    let provider = get_value(database, PROVIDER_KEY)
        .await?
        .unwrap_or_else(|| "tencent".to_string());
    let voice_type = parse_i64(
        get_value(database, VOICE_KEY).await?,
        DEFAULT_TTS_SETTINGS.voice_type,
        VOICE_KEY,
    )?;
    let speed = parse_f64(
        get_value(database, SPEED_KEY).await?,
        DEFAULT_TTS_SETTINGS.speed,
        SPEED_KEY,
    )?;
    let volume = parse_f64(
        get_value(database, VOLUME_KEY).await?,
        DEFAULT_TTS_SETTINGS.volume,
        VOLUME_KEY,
    )?;
    let sample_rate = parse_i64(
        get_value(database, SAMPLE_RATE_KEY).await?,
        DEFAULT_TTS_SETTINGS.sample_rate,
        SAMPLE_RATE_KEY,
    )?;
    let settings = TtsSettings {
        provider,
        voice_type,
        speed,
        volume,
        sample_rate,
        codec: "wav".to_string(),
    };
    validate(&settings)?;
    Ok(settings)
}

pub async fn save_tts_settings(
    database: &Database,
    settings: &TtsSettings,
) -> AppResult<TtsSettings> {
    validate(settings)?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    for (key, value) in [
        (PROVIDER_KEY, settings.provider.clone()),
        (VOICE_KEY, settings.voice_type.to_string()),
        (SPEED_KEY, settings.speed.to_string()),
        (VOLUME_KEY, settings.volume.to_string()),
        (SAMPLE_RATE_KEY, settings.sample_rate.to_string()),
    ] {
        sqlx::query("INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
            .bind(key).bind(value).bind(&now).execute(&mut *transaction).await.map_err(transaction_error)?;
    }
    transaction.commit().await.map_err(transaction_error)?;
    get_tts_settings(database).await
}

pub async fn get_reader_display_settings(database: &Database) -> AppResult<ReaderDisplaySettings> {
    let defaults = default_reader_display_settings();
    let settings = ReaderDisplaySettings {
        font_size: parse_i64(
            get_value(database, READER_FONT_SIZE_KEY).await?,
            defaults.font_size,
            READER_FONT_SIZE_KEY,
        )?,
        pinyin_mode: get_value(database, READER_PINYIN_MODE_KEY)
            .await?
            .unwrap_or(defaults.pinyin_mode),
    };
    validate_reader_display_settings(&settings)?;
    Ok(settings)
}

pub async fn save_reader_display_settings(
    database: &Database,
    settings: &ReaderDisplaySettings,
) -> AppResult<ReaderDisplaySettings> {
    validate_reader_display_settings(settings)?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    for (key, value) in [
        (READER_FONT_SIZE_KEY, settings.font_size.to_string()),
        (READER_PINYIN_MODE_KEY, settings.pinyin_mode.clone()),
    ] {
        sqlx::query("INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(&mut *transaction)
            .await
            .map_err(transaction_error)?;
    }
    transaction.commit().await.map_err(transaction_error)?;
    get_reader_display_settings(database).await
}

pub fn validate_reader_display_settings(settings: &ReaderDisplaySettings) -> AppResult<()> {
    if !(20..=40).contains(&settings.font_size) {
        return Err(AppError::new(
            "READER_FONT_SIZE_INVALID",
            "正文字号必须在 20 到 40px 之间",
        ));
    }
    if !matches!(settings.pinyin_mode.as_str(), "off" | "risky" | "all") {
        return Err(AppError::new(
            "READER_PINYIN_MODE_INVALID",
            "注音模式必须是 off、risky 或 all",
        ));
    }
    Ok(())
}

pub fn validate(settings: &TtsSettings) -> AppResult<()> {
    if settings.provider != "tencent" {
        return Err(AppError::new(
            "TTS_PROVIDER_UNSUPPORTED",
            "当前仅支持腾讯云 TTS",
        ));
    }
    if !tencent_voices()
        .iter()
        .any(|voice| voice.id == settings.voice_type)
    {
        return Err(AppError::new(
            "TTS_INVALID_VOICE",
            "当前音色不在内置音色列表中",
        ));
    }
    if !(-2.0..=6.0).contains(&settings.speed) {
        return Err(AppError::new(
            "TTS_INVALID_SPEED",
            "腾讯云 Speed 必须在 -2 到 6 之间",
        ));
    }
    if !(-10.0..=10.0).contains(&settings.volume) {
        return Err(AppError::new(
            "TTS_INVALID_VOLUME",
            "腾讯云 Volume 必须在 -10 到 10 之间",
        ));
    }
    if settings.sample_rate != 16000 || settings.codec != "wav" {
        return Err(AppError::new(
            "TTS_INVALID_FORMAT",
            "当前只支持 16000Hz WAV",
        ));
    }
    Ok(())
}

async fn get_value(database: &Database, key: &str) -> AppResult<Option<String>> {
    Ok(
        sqlx::query_scalar::<_, String>("SELECT value FROM app_settings WHERE key = ?")
            .bind(key)
            .fetch_optional(database.pool())
            .await?,
    )
}

fn parse_i64(value: Option<String>, default: i64, key: &str) -> AppResult<i64> {
    value
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| AppError::new("DB_ERROR", format!("设置 {key} 不是整数")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_f64(value: Option<String>, default: f64, key: &str) -> AppResult<f64> {
    value
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| AppError::new("DB_ERROR", format!("设置 {key} 不是数字")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
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
        get_reader_display_settings, get_tts_settings, save_reader_display_settings,
        save_tts_settings, tencent_voices,
    };
    use crate::db::Database;
    use crate::models::TtsSettings;
    use uuid::Uuid;

    #[test]
    fn defaults_and_settings_round_trip() {
        let path =
            std::env::temp_dir().join(format!("ancient-tts-settings-{}.sqlite", Uuid::now_v7()));
        let database = tauri::async_runtime::block_on(Database::open(path)).expect("db");
        tauri::async_runtime::block_on(async {
            let defaults = get_tts_settings(&database).await.expect("defaults");
            assert_eq!(defaults.voice_type, 501000);
            assert_eq!(defaults.sample_rate, 16000);
            let saved = save_tts_settings(
                &database,
                &TtsSettings {
                    provider: "tencent".to_string(),
                    voice_type: 501002,
                    speed: -1.0,
                    volume: 2.0,
                    sample_rate: 16000,
                    codec: "wav".to_string(),
                },
            )
            .await
            .expect("save");
            assert_eq!(saved.voice_type, 501002);
            assert_eq!(get_tts_settings(&database).await.expect("load").speed, -1.0);

            let reader_defaults = get_reader_display_settings(&database)
                .await
                .expect("reader defaults");
            assert_eq!(reader_defaults.font_size, 25);
            assert_eq!(reader_defaults.pinyin_mode, "risky");
            let reader_saved = save_reader_display_settings(
                &database,
                &crate::models::ReaderDisplaySettings {
                    font_size: 40,
                    pinyin_mode: "all".to_string(),
                },
            )
            .await
            .expect("reader settings save");
            assert_eq!(reader_saved.font_size, 40);
            assert_eq!(reader_saved.pinyin_mode, "all");
            assert_eq!(
                get_reader_display_settings(&database)
                    .await
                    .expect("reader settings load")
                    .pinyin_mode,
                "all"
            );
        });
        let voices = tencent_voices();
        assert_eq!(voices.len(), 6);
        assert_eq!(
            voices.iter().map(|voice| voice.id).collect::<Vec<_>>(),
            vec![501000, 501002, 501003, 601013, 101030, 101055]
        );
        assert_eq!(voices[0].name, "智斌");
        assert_eq!(voices[2].name, "智宇");
        assert_eq!(voices[3].name, "爱小伊");
        assert!(voices[..4]
            .iter()
            .all(|voice| voice.category == "大模型·阅读"));
        assert!(voices[4..]
            .iter()
            .all(|voice| voice.category == "精品·通用"));
    }
}
