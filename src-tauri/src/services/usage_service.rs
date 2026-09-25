use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{ApiUsageSummary, ApiUsageTtsSummary, ApiUsageVoiceSummary},
};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use uuid::Uuid;

const VALID_RANGES: &[&str] = &["today", "7d", "30d", "all"];

pub async fn record_event(
    database: &Database,
    provider: &str,
    service: &str,
    model: Option<&str>,
    operation: &str,
    unit_type: &str,
    input_units: i64,
    output_units: i64,
    success: bool,
    error_code: Option<&str>,
) -> AppResult<()> {
    if input_units < 0 || output_units < 0 {
        return Err(AppError::new(
            "USAGE_INVALID_UNITS",
            "API 用量单位不能小于 0",
        ));
    }
    let created_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("DB_ERROR", error.to_string()))?;
    sqlx::query(
        "INSERT INTO api_usage_events
         (id, created_at, provider, service, model, operation, unit_type,
          input_units, output_units, success, error_code, metadata_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(created_at)
    .bind(provider)
    .bind(service)
    .bind(model)
    .bind(operation)
    .bind(unit_type)
    .bind(input_units)
    .bind(output_units)
    .bind(if success { 1_i64 } else { 0_i64 })
    .bind(error_code)
    .execute(database.pool())
    .await
    .map_err(AppError::from)?;
    Ok(())
}

pub async fn get_summary(database: &Database, range: &str) -> AppResult<ApiUsageSummary> {
    if !VALID_RANGES.contains(&range) {
        return Err(AppError::new(
            "USAGE_RANGE_INVALID",
            "用量统计范围必须是 today、7d、30d 或 all",
        ));
    }
    let cutoff = range_cutoff(range)?;
    let (requests, success, failed, characters): (i64, i64, i64, i64) =
        if let Some(cutoff) = &cutoff {
            sqlx::query_as(
                "SELECT COUNT(*), COALESCE(SUM(success), 0),
                    COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN success = 1 THEN input_units ELSE 0 END), 0)
             FROM api_usage_events
             WHERE provider = 'tencent' AND service = 'tts' AND created_at >= ?",
            )
            .bind(cutoff)
            .fetch_one(database.pool())
            .await?
        } else {
            sqlx::query_as(
                "SELECT COUNT(*), COALESCE(SUM(success), 0),
                    COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN success = 1 THEN input_units ELSE 0 END), 0)
             FROM api_usage_events
             WHERE provider = 'tencent' AND service = 'tts'",
            )
            .fetch_one(database.pool())
            .await?
        };

    let rows: Vec<(i64, i64)> = if let Some(cutoff) = &cutoff {
        sqlx::query_as(
            "SELECT CAST(model AS INTEGER),
                    COALESCE(SUM(CASE WHEN success = 1 THEN input_units ELSE 0 END), 0)
             FROM api_usage_events
             WHERE provider = 'tencent' AND service = 'tts'
               AND model IS NOT NULL AND created_at >= ?
             GROUP BY model
             ORDER BY 2 DESC
             LIMIT 3",
        )
        .bind(cutoff)
        .fetch_all(database.pool())
        .await?
    } else {
        sqlx::query_as(
            "SELECT CAST(model AS INTEGER),
                    COALESCE(SUM(CASE WHEN success = 1 THEN input_units ELSE 0 END), 0)
             FROM api_usage_events
             WHERE provider = 'tencent' AND service = 'tts' AND model IS NOT NULL
             GROUP BY model
             ORDER BY 2 DESC
             LIMIT 3",
        )
        .fetch_all(database.pool())
        .await?
    };

    Ok(ApiUsageSummary {
        range: range.to_string(),
        tts: ApiUsageTtsSummary {
            requests,
            success,
            failed,
            characters,
            by_voice: rows
                .into_iter()
                .map(|(voice_type, characters)| ApiUsageVoiceSummary {
                    voice_type,
                    characters,
                })
                .collect(),
        },
    })
}

fn range_cutoff(range: &str) -> AppResult<Option<String>> {
    let duration = match range {
        "today" => Some(Duration::days(1)),
        "7d" => Some(Duration::days(7)),
        "30d" => Some(Duration::days(30)),
        "all" => None,
        _ => return Err(AppError::new("USAGE_RANGE_INVALID", "用量统计范围无效")),
    };
    duration
        .map(|duration| {
            (OffsetDateTime::now_utc() - duration)
                .format(&Rfc3339)
                .map_err(|error| AppError::new("DB_ERROR", error.to_string()))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{get_summary, record_event};
    use crate::db::Database;
    use uuid::Uuid;

    #[test]
    fn usage_summary_counts_success_characters_and_voices() {
        let path = std::env::temp_dir().join(format!("ancient-usage-{}.sqlite", Uuid::now_v7()));
        let database = tauri::async_runtime::block_on(Database::open(&path)).expect("db");
        tauri::async_runtime::block_on(async {
            record_event(
                &database,
                "tencent",
                "tts",
                Some("501000"),
                "voice_preview",
                "characters",
                12,
                0,
                true,
                None,
            )
            .await
            .expect("success");
            record_event(
                &database,
                "tencent",
                "tts",
                Some("501002"),
                "segment_generation",
                "characters",
                20,
                0,
                true,
                None,
            )
            .await
            .expect("success");
            record_event(
                &database,
                "tencent",
                "tts",
                Some("501002"),
                "connection_test",
                "characters",
                0,
                0,
                false,
                Some("TTS_AUTH_ERROR"),
            )
            .await
            .expect("failure");
            let summary = get_summary(&database, "all").await.expect("summary");
            assert_eq!(summary.tts.requests, 3);
            assert_eq!(summary.tts.success, 2);
            assert_eq!(summary.tts.failed, 1);
            assert_eq!(summary.tts.characters, 32);
            assert_eq!(summary.tts.by_voice[0].voice_type, 501002);
        });
        let _ = std::fs::remove_file(path);
    }
}
