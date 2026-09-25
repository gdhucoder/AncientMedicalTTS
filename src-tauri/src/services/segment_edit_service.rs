use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{Segment, SegmentEditResult},
    services::{book_service, pronunciation_service},
};
use sqlx::{Sqlite, Transaction};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const SUPERSEDED_STATUS: &str = "superseded";

pub async fn update_reading_text(
    database: &Database,
    segment_id: &str,
    reading_text: Option<&str>,
) -> AppResult<SegmentEditResult> {
    let segment = book_service::get_segment(database, segment_id).await?;
    let next_reading_text = normalize_reading_text(&segment.original_text, reading_text)?;
    let old_effective = segment.effective_text().to_string();
    let next_effective = next_reading_text
        .as_deref()
        .unwrap_or(segment.original_text.as_str())
        .to_string();
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    if old_effective != next_effective {
        invalidate_segment_tx(&mut transaction, segment_id, &now).await?;
    }
    sqlx::query(
        "UPDATE segments SET reading_text = ?, updated_at = ? WHERE id = ? AND status <> ?",
    )
    .bind(&next_reading_text)
    .bind(&now)
    .bind(segment_id)
    .bind(SUPERSEDED_STATUS)
    .execute(&mut *transaction)
    .await
    .map_err(transaction_error)?;
    transaction.commit().await.map_err(transaction_error)?;
    result_for(database, segment_id, vec![segment_id.to_string()], vec![]).await
}

pub async fn restore_reading_text(
    database: &Database,
    segment_id: &str,
) -> AppResult<SegmentEditResult> {
    update_reading_text(database, segment_id, None).await
}

pub async fn set_speak_enabled(
    database: &Database,
    segment_id: &str,
    speak_enabled: bool,
) -> AppResult<SegmentEditResult> {
    let _ = book_service::get_segment(database, segment_id).await?;
    let now = now_rfc3339()?;
    let result = sqlx::query(
        "UPDATE segments SET speak_enabled = ?, updated_at = ? WHERE id = ? AND status <> ?",
    )
    .bind(if speak_enabled { 1_i64 } else { 0_i64 })
    .bind(&now)
    .bind(segment_id)
    .bind(SUPERSEDED_STATUS)
    .execute(database.pool())
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"));
    }
    result_for(database, segment_id, vec![segment_id.to_string()], vec![]).await
}

pub async fn split_segment(
    database: &Database,
    segment_id: &str,
    token_index: usize,
) -> AppResult<SegmentEditResult> {
    let segment = book_service::get_segment(database, segment_id).await?;
    let effective_tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
    if token_index == 0 || token_index >= effective_tokens.len() {
        return Err(AppError::new(
            "SEGMENT_SPLIT_INVALID",
            "分段光标必须位于 Segment 中间",
        ));
    }
    let original_tokens = pronunciation_service::grapheme_tokens(&segment.original_text);
    let original_split = token_index.min(original_tokens.len().saturating_sub(1));
    if original_split == 0 || original_split >= original_tokens.len() {
        return Err(AppError::new(
            "SEGMENT_SPLIT_INVALID",
            "当前编辑文本与原文无法安全分段，请先恢复原文后再分段",
        ));
    }
    let first_effective = join_tokens(&effective_tokens[..token_index]);
    let second_effective = join_tokens(&effective_tokens[token_index..]);
    let first_original = join_tokens(&original_tokens[..original_split]);
    let second_original = join_tokens(&original_tokens[original_split..]);
    if first_effective.is_empty()
        || second_effective.is_empty()
        || first_original.is_empty()
        || second_original.is_empty()
    {
        return Err(AppError::new(
            "SEGMENT_SPLIT_INVALID",
            "分段后不能产生空 Segment",
        ));
    }

    let first_id = Uuid::now_v7().to_string();
    let second_id = Uuid::now_v7().to_string();
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    supersede_segment_tx(&mut transaction, &segment, &now).await?;
    shift_following_segments_tx(&mut transaction, &segment.chapter_id, segment.order_index).await?;
    insert_segment_tx(
        &mut transaction,
        &first_id,
        &segment.chapter_id,
        segment.order_index,
        &first_original,
        reading_text_for(&first_original, &first_effective),
        segment.speak_enabled,
        &now,
    )
    .await?;
    insert_segment_tx(
        &mut transaction,
        &second_id,
        &segment.chapter_id,
        segment.order_index + 1,
        &second_original,
        reading_text_for(&second_original, &second_effective),
        segment.speak_enabled,
        &now,
    )
    .await?;
    reindex_chapter_tx(&mut transaction, &segment.chapter_id).await?;
    transaction.commit().await.map_err(transaction_error)?;
    result_for(
        database,
        &first_id,
        vec![segment_id.to_string()],
        vec![first_id.clone(), second_id],
    )
    .await
}

pub async fn merge_with_previous(
    database: &Database,
    segment_id: &str,
) -> AppResult<SegmentEditResult> {
    merge_adjacent(database, segment_id, true).await
}

pub async fn merge_with_next(
    database: &Database,
    segment_id: &str,
) -> AppResult<SegmentEditResult> {
    merge_adjacent(database, segment_id, false).await
}

async fn merge_adjacent(
    database: &Database,
    segment_id: &str,
    previous: bool,
) -> AppResult<SegmentEditResult> {
    let segment = book_service::get_segment(database, segment_id).await?;
    let adjacent_order = if previous {
        segment.order_index - 1
    } else {
        segment.order_index + 1
    };
    let adjacent = load_adjacent(database, &segment.chapter_id, adjacent_order).await?;
    let (left, right) = if previous {
        (adjacent, segment)
    } else {
        (segment, adjacent)
    };
    if left.chapter_id != right.chapter_id {
        return Err(AppError::new(
            "SEGMENT_MERGE_INVALID",
            "只能合并同一 Chapter 中相邻的 Segment",
        ));
    }
    let original_text = format!("{}{}", left.original_text, right.original_text);
    let effective_text = format!("{}{}", left.effective_text(), right.effective_text());
    if effective_text.trim().is_empty() {
        return Err(AppError::new(
            "SEGMENT_TEXT_EMPTY",
            "合并后的朗读文本不能为空",
        ));
    }
    let new_id = Uuid::now_v7().to_string();
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    supersede_segment_tx(&mut transaction, &left, &now).await?;
    supersede_segment_tx(&mut transaction, &right, &now).await?;
    insert_segment_tx(
        &mut transaction,
        &new_id,
        &left.chapter_id,
        left.order_index,
        &original_text,
        reading_text_for(&original_text, &effective_text),
        left.speak_enabled && right.speak_enabled,
        &now,
    )
    .await?;
    reindex_chapter_tx(&mut transaction, &left.chapter_id).await?;
    transaction.commit().await.map_err(transaction_error)?;
    result_for(
        database,
        &new_id,
        vec![left.id, right.id],
        vec![new_id.clone()],
    )
    .await
}

async fn load_adjacent(
    database: &Database,
    chapter_id: &str,
    order_index: i64,
) -> AppResult<Segment> {
    let row = sqlx::query_as::<_, (String, String, i64, String, Option<String>, i64, String, Option<String>, String, String)>(
        "SELECT id, chapter_id, order_index, original_text, reading_text, speak_enabled, status, current_audio_id, created_at, updated_at
         FROM segments WHERE chapter_id = ? AND order_index = ? AND status <> ?",
    )
    .bind(chapter_id)
    .bind(order_index)
    .bind(SUPERSEDED_STATUS)
    .fetch_optional(database.pool())
    .await?;
    row.map(segment_from_row).ok_or_else(|| {
        AppError::new(
            "SEGMENT_MERGE_INVALID",
            "当前 Segment 没有可合并的相邻 Segment",
        )
    })
}

async fn result_for(
    database: &Database,
    segment_id: &str,
    affected_segment_ids: Vec<String>,
    new_segment_ids: Vec<String>,
) -> AppResult<SegmentEditResult> {
    Ok(SegmentEditResult {
        segment: book_service::get_segment(database, segment_id).await?,
        affected_segment_ids,
        new_segment_ids,
    })
}

async fn invalidate_segment_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    segment_id: &str,
    now: &str,
) -> AppResult<()> {
    sqlx::query("DELETE FROM segment_annotations WHERE segment_id = ?")
        .bind(segment_id)
        .execute(&mut **transaction)
        .await
        .map_err(transaction_error)?;
    sqlx::query("UPDATE segments SET status = 'pending', current_audio_id = NULL, updated_at = ? WHERE id = ? AND status <> ?")
        .bind(now)
        .bind(segment_id)
        .bind(SUPERSEDED_STATUS)
        .execute(&mut **transaction)
        .await
        .map_err(transaction_error)?;
    Ok(())
}

async fn supersede_segment_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    segment: &Segment,
    now: &str,
) -> AppResult<()> {
    sqlx::query("DELETE FROM segment_annotations WHERE segment_id = ?")
        .bind(&segment.id)
        .execute(&mut **transaction)
        .await
        .map_err(transaction_error)?;
    sqlx::query("UPDATE segments SET status = ?, current_audio_id = NULL, speak_enabled = 0, updated_at = ? WHERE id = ?")
        .bind(SUPERSEDED_STATUS)
        .bind(now)
        .bind(&segment.id)
        .execute(&mut **transaction)
        .await
        .map_err(transaction_error)?;
    Ok(())
}

async fn insert_segment_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    id: &str,
    chapter_id: &str,
    order_index: i64,
    original_text: &str,
    reading_text: Option<String>,
    speak_enabled: bool,
    now: &str,
) -> AppResult<()> {
    sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, reading_text, speak_enabled, status, current_audio_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'pending', NULL, ?, ?)")
        .bind(id)
        .bind(chapter_id)
        .bind(order_index)
        .bind(original_text)
        .bind(reading_text)
        .bind(if speak_enabled { 1_i64 } else { 0_i64 })
        .bind(now)
        .bind(now)
        .execute(&mut **transaction)
        .await
        .map_err(transaction_error)?;
    Ok(())
}

async fn shift_following_segments_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    chapter_id: &str,
    order_index: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE segments
         SET order_index = order_index + 1
         WHERE chapter_id = ? AND status <> ? AND order_index > ?",
    )
    .bind(chapter_id)
    .bind(SUPERSEDED_STATUS)
    .bind(order_index)
    .execute(&mut **transaction)
    .await
    .map_err(transaction_error)?;
    Ok(())
}

async fn reindex_chapter_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    chapter_id: &str,
) -> AppResult<()> {
    let ids = sqlx::query_scalar::<_, String>(
        "SELECT id FROM segments WHERE chapter_id = ? AND status <> ? ORDER BY order_index, id",
    )
    .bind(chapter_id)
    .bind(SUPERSEDED_STATUS)
    .fetch_all(&mut **transaction)
    .await
    .map_err(transaction_error)?;
    for (index, id) in ids.into_iter().enumerate() {
        sqlx::query("UPDATE segments SET order_index = ? WHERE id = ?")
            .bind(index as i64)
            .bind(id)
            .execute(&mut **transaction)
            .await
            .map_err(transaction_error)?;
    }
    Ok(())
}

fn normalize_reading_text(
    original_text: &str,
    reading_text: Option<&str>,
) -> AppResult<Option<String>> {
    let value = reading_text.unwrap_or(original_text);
    if value.trim().is_empty() {
        return Err(AppError::new("SEGMENT_TEXT_EMPTY", "朗读文本不能为空"));
    }
    Ok((value != original_text).then(|| value.to_string()))
}

fn reading_text_for(original_text: &str, effective_text: &str) -> Option<String> {
    (original_text != effective_text).then(|| effective_text.to_string())
}

fn join_tokens(tokens: &[crate::models::GraphemeToken]) -> String {
    tokens.iter().map(|token| token.text.as_str()).collect()
}

fn segment_from_row(
    (
        id,
        chapter_id,
        order_index,
        original_text,
        reading_text,
        speak_enabled,
        status,
        current_audio_id,
        created_at,
        updated_at,
    ): (
        String,
        String,
        i64,
        String,
        Option<String>,
        i64,
        String,
        Option<String>,
        String,
        String,
    ),
) -> Segment {
    Segment {
        id,
        chapter_id,
        order_index,
        original_text,
        reading_text,
        speak_enabled: speak_enabled != 0,
        status,
        current_audio_id,
        created_at,
        updated_at,
    }
}

fn transaction_error(error: sqlx::Error) -> AppError {
    AppError::new("DB_TRANSACTION_FAILED", error.to_string())
}

fn now_rfc3339() -> AppResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("DB_ERROR", error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{merge_with_next, set_speak_enabled, split_segment, update_reading_text};
    use crate::{db::Database, services::book_service};
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path =
            std::env::temp_dir().join(format!("ancient-medical-edit-{}.sqlite", Uuid::now_v7()));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    async fn seed_segment(
        database: &Database,
        text: &str,
        status: &str,
    ) -> (String, String, String) {
        let book_id = Uuid::now_v7().to_string();
        let chapter_id = Uuid::now_v7().to_string();
        let segment_id = Uuid::now_v7().to_string();
        sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, '测试', 'now', 'now')")
            .bind(&book_id)
            .execute(database.pool())
            .await
            .expect("book");
        sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, '正文', 0, 'now', 'now')")
            .bind(&chapter_id)
            .bind(&book_id)
            .execute(database.pool())
            .await
            .expect("chapter");
        sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, current_audio_id, created_at, updated_at) VALUES (?, ?, 0, ?, ?, 'audio-old', 'now', 'now')")
            .bind(&segment_id)
            .bind(&chapter_id)
            .bind(text)
            .bind(status)
            .execute(database.pool())
            .await
            .expect("segment");
        sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, audio_path, created_at) VALUES ('audio-old', ?, 1, 'tencent', 501000, 16000, 'wav', 0, 0, '/tmp/old.wav', 'now')")
            .bind(&segment_id)
            .execute(database.pool())
            .await
            .expect("audio version");
        (book_id, chapter_id, segment_id)
    }

    #[test]
    fn reading_text_is_editable_without_overwriting_original_and_invalidates_audio() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (_, _, segment_id) = seed_segment(&database, "腧穴。", "generated").await;
            let result = update_reading_text(&database, &segment_id, Some("腧穴。 "))
                .await
                .expect("reading text should update");
            assert_eq!(result.segment.original_text, "腧穴。");
            assert_eq!(result.segment.reading_text.as_deref(), Some("腧穴。 "));
            assert_eq!(result.segment.effective_text(), "腧穴。 ");
            assert_eq!(result.segment.status, "pending");
            assert!(result.segment.current_audio_id.is_none());
            let audio_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM audio_versions WHERE id = 'audio-old'")
                    .fetch_one(database.pool())
                    .await
                    .expect("audio should remain");
            assert_eq!(audio_count, 1);
            let restored = super::restore_reading_text(&database, &segment_id)
                .await
                .expect("reading text should restore");
            assert!(restored.segment.reading_text.is_none());
            assert_eq!(restored.segment.original_text, "腧穴。");
        });
    }

    #[test]
    fn split_and_merge_preserve_source_rows_and_old_audio_versions() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (_, chapter_id, segment_id) = seed_segment(&database, "甲乙丙", "generated").await;
            let split = split_segment(&database, &segment_id, 1)
                .await
                .expect("segment should split");
            assert_eq!(split.new_segment_ids.len(), 2);
            let page = book_service::list_segments(&database, &chapter_id, 0, 100)
                .await
                .expect("active segments");
            assert_eq!(page.total, 2);
            assert_eq!(page.items[0].original_text, "甲");
            assert_eq!(page.items[1].original_text, "乙丙");
            let old_status: String = sqlx::query_scalar("SELECT status FROM segments WHERE id = ?")
                .bind(&segment_id)
                .fetch_one(database.pool())
                .await
                .expect("superseded source");
            assert_eq!(old_status, "superseded");
            let audio_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM audio_versions WHERE segment_id = ?")
                    .bind(&segment_id)
                    .fetch_one(database.pool())
                    .await
                    .expect("old audio");
            assert_eq!(audio_count, 1);

            let merged = merge_with_next(&database, &split.segment.id)
                .await
                .expect("segments should merge");
            assert_eq!(merged.segment.original_text, "甲乙丙");
            assert_eq!(merged.segment.status, "pending");
            let merged_page = book_service::list_segments(&database, &chapter_id, 0, 100)
                .await
                .expect("merged active segment");
            assert_eq!(merged_page.total, 1);
            assert_eq!(merged_page.items[0].effective_text(), "甲乙丙");
        });
    }

    #[test]
    fn split_inserts_the_right_half_before_the_original_next_segment() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let book_id = Uuid::now_v7().to_string();
            let chapter_id = Uuid::now_v7().to_string();
            let first_id = Uuid::now_v7().to_string();
            let next_id = Uuid::now_v7().to_string();
            sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, '测试', 'now', 'now')")
                .bind(&book_id)
                .execute(database.pool())
                .await
                .expect("book");
            sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, '正文', 0, 'now', 'now')")
                .bind(&chapter_id)
                .bind(&book_id)
                .execute(database.pool())
                .await
                .expect("chapter");
            for (id, order_index, text) in [(&first_id, 0_i64, "甲乙"), (&next_id, 1_i64, "丙丁")]
            {
                sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'pending', 'now', 'now')")
                    .bind(id)
                    .bind(&chapter_id)
                    .bind(order_index)
                    .bind(text)
                    .execute(database.pool())
                    .await
                    .expect("segment");
            }

            let split = split_segment(&database, &first_id, 1)
                .await
                .expect("segment should split");
            let page = book_service::list_segments(&database, &chapter_id, 0, 100)
                .await
                .expect("active segments");
            assert_eq!(
                page.items
                    .iter()
                    .map(|item| item.original_text.as_str())
                    .collect::<Vec<_>>(),
                vec!["甲", "乙", "丙丁"]
            );
            assert_eq!(split.segment.original_text, "甲");
            assert_eq!(page.items[0].order_index, 0);
            assert_eq!(page.items[1].order_index, 1);
            assert_eq!(page.items[2].order_index, 2);
        });
    }

    #[test]
    fn speak_enabled_is_persisted_and_excluded_from_enabled_state() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (_, _, segment_id) = seed_segment(&database, "甲", "generated").await;
            let result = set_speak_enabled(&database, &segment_id, false)
                .await
                .expect("speak setting should update");
            assert!(!result.segment.speak_enabled);
            let stored: i64 = sqlx::query_scalar("SELECT speak_enabled FROM segments WHERE id = ?")
                .bind(&segment_id)
                .fetch_one(database.pool())
                .await
                .expect("speak flag");
            assert_eq!(stored, 0);
        });
    }
}
