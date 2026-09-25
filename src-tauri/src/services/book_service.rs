use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{
        Book, BookDetail, BookSummary, BookTextStats, Chapter, ChapterSummary, ImportResult,
        PaginatedSegments, Segment,
    },
    services::{segment_service, text_service},
};
use sqlx::sqlite::SqliteQueryResult;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const MAX_SEGMENT_PAGE_SIZE: i64 = 100;

#[derive(Debug, Clone)]
struct ChapterInput {
    title: String,
    segments: Vec<String>,
}

pub async fn import_txt_book(
    database: &Database,
    path: &str,
    requested_title: Option<String>,
) -> AppResult<ImportResult> {
    let (text, source_file) = text_service::read_utf8_txt(path)?;
    validate_han_character_limit(&text)?;
    let title = requested_title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| text_service::title_from_file_name(&source_file));
    let chapters = text_service::split_into_chapters(&text)
        .into_iter()
        .map(|chapter| {
            let segments = if chapter.text.trim().is_empty() {
                Vec::new()
            } else {
                segment_service::segment_text(&chapter.text)?
            };
            Ok(ChapterInput {
                title: chapter.title,
                segments,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    persist_chapters(database, &title, &source_file, &chapters, None).await
}

#[cfg(test)]
async fn persist_book(
    database: &Database,
    title: &str,
    source_file: &str,
    segments: &[String],
    fail_after: Option<usize>,
) -> AppResult<ImportResult> {
    persist_chapters(
        database,
        title,
        source_file,
        &[ChapterInput {
            title: "正文".to_string(),
            segments: segments.to_vec(),
        }],
        fail_after,
    )
    .await
}

async fn persist_chapters(
    database: &Database,
    title: &str,
    source_file: &str,
    chapters: &[ChapterInput],
    fail_after: Option<usize>,
) -> AppResult<ImportResult> {
    let all_segments = chapters
        .iter()
        .flat_map(|chapter| chapter.segments.iter())
        .cloned()
        .collect::<Vec<_>>();
    validate_han_character_limit(&all_segments.concat())?;
    if all_segments.is_empty() {
        return Err(AppError::new("EMPTY_TEXT", "文本内容为空"));
    }
    let now = now_rfc3339()?;
    let book_id = Uuid::now_v7().to_string();
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|error| AppError::new("DB_TRANSACTION_FAILED", error.to_string()))?;

    sqlx::query(
        "INSERT INTO books (id, title, source_file, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&book_id)
    .bind(title)
    .bind(source_file)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await
    .map_err(transaction_error)?;

    let mut segment_index = 0;
    for (chapter_index, chapter) in chapters.iter().enumerate() {
        let chapter_id = Uuid::now_v7().to_string();
        sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&chapter_id).bind(&book_id).bind(&chapter.title).bind(chapter_index as i64).bind(&now).bind(&now)
            .execute(&mut *transaction).await.map_err(transaction_error)?;

        for (index, segment) in chapter.segments.iter().enumerate() {
            if fail_after == Some(segment_index) {
                return Err(AppError::new(
                    "DB_TRANSACTION_FAILED",
                    "测试用 Segment 写入失败",
                ));
            }
            sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'pending', ?, ?)")
                .bind(Uuid::now_v7().to_string()).bind(&chapter_id).bind(index as i64).bind(segment).bind(&now).bind(&now)
                .execute(&mut *transaction).await.map_err(transaction_error)?;
            segment_index += 1;
        }
    }
    transaction.commit().await.map_err(transaction_error)?;

    let book = get_book(database, &book_id).await?;
    let chapters = list_chapters(database, &book_id)
        .await?
        .into_iter()
        .map(|summary| summary.chapter)
        .collect::<Vec<_>>();
    let chapter = chapters
        .first()
        .cloned()
        .ok_or_else(|| AppError::new("CHAPTER_NOT_FOUND", "导入后的 Chapter 不存在"))?;
    Ok(ImportResult {
        book,
        chapter,
        chapters,
        segment_count: all_segments.len() as i64,
    })
}

fn validate_han_character_limit(text: &str) -> AppResult<()> {
    let count = text_service::count_han_characters(text);
    if count > text_service::MAX_BOOK_HAN_CHARACTERS {
        return Err(AppError::new(
            "PROJECT_TEXT_LIMIT_EXCEEDED",
            format!(
                "当前版本单个文档最多支持 {} 个汉字，当前文档包含 {} 个汉字。请拆分后重新导入。",
                text_service::MAX_BOOK_HAN_CHARACTERS,
                count
            ),
        ));
    }
    Ok(())
}

pub async fn list_books(database: &Database) -> AppResult<Vec<BookSummary>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, i64, String)>(
        "SELECT b.id, b.title, b.source_file, COUNT(DISTINCT c.id), COUNT(s.id), b.created_at
         FROM books b LEFT JOIN chapters c ON c.book_id = b.id
         LEFT JOIN segments s ON s.chapter_id = c.id AND s.status <> 'superseded'
         GROUP BY b.id ORDER BY b.created_at DESC",
    )
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, source_file, chapter_count, segment_count, created_at)| BookSummary {
                id,
                title,
                source_file,
                chapter_count,
                segment_count,
                created_at,
            },
        )
        .collect())
}

pub async fn get_book(database: &Database, book_id: &str) -> AppResult<BookDetail> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, String, String, i64, i64)>(
        "SELECT b.id, b.title, b.author, b.dynasty, b.edition, b.source_file, b.created_at, b.updated_at,
                COUNT(DISTINCT c.id), COUNT(s.id)
         FROM books b LEFT JOIN chapters c ON c.book_id = b.id
         LEFT JOIN segments s ON s.chapter_id = c.id AND s.status <> 'superseded'
         WHERE b.id = ? GROUP BY b.id")
        .bind(book_id).fetch_optional(database.pool()).await?;
    let (
        id,
        title,
        author,
        dynasty,
        edition,
        source_file,
        created_at,
        updated_at,
        chapter_count,
        segment_count,
    ) = row.ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "Book 不存在"))?;
    Ok(BookDetail {
        book: Book {
            id,
            title,
            author,
            dynasty,
            edition,
            source_file,
            created_at,
            updated_at,
        },
        chapter_count,
        segment_count,
    })
}

pub async fn get_book_text_stats(database: &Database, book_id: &str) -> AppResult<BookTextStats> {
    let detail = get_book(database, book_id).await?;
    let texts = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT s.original_text, s.reading_text
         FROM segments s
         JOIN chapters c ON c.id = s.chapter_id
         WHERE c.book_id = ? AND s.status <> 'superseded'
         ORDER BY c.order_index, s.order_index",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;
    let han_character_count = texts
        .iter()
        .map(|(original_text, reading_text)| {
            text_service::count_han_characters(reading_text.as_deref().unwrap_or(&original_text))
        })
        .sum::<usize>() as i64;
    Ok(BookTextStats {
        han_character_count,
        chapter_count: detail.chapter_count,
        segment_count: detail.segment_count,
    })
}

pub async fn list_chapters(database: &Database, book_id: &str) -> AppResult<Vec<ChapterSummary>> {
    ensure_book_exists(database, book_id).await?;
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, String, String, i64)>(
        "SELECT c.id, c.book_id, c.title, c.order_index, c.created_at, c.updated_at, COUNT(s.id)
         FROM chapters c LEFT JOIN segments s ON s.chapter_id = c.id AND s.status <> 'superseded'
         WHERE c.book_id = ? GROUP BY c.id ORDER BY c.order_index",
    )
    .bind(book_id)
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, book_id, title, order_index, created_at, updated_at, segment_count)| {
                ChapterSummary {
                    chapter: Chapter {
                        id,
                        book_id,
                        title,
                        order_index,
                        created_at,
                        updated_at,
                    },
                    segment_count,
                }
            },
        )
        .collect())
}

pub async fn list_segments(
    database: &Database,
    chapter_id: &str,
    offset: i64,
    limit: i64,
) -> AppResult<PaginatedSegments> {
    if offset < 0 {
        return Err(AppError::new("INVALID_PAGINATION", "offset 不能小于 0"));
    }
    if limit <= 0 {
        return Err(AppError::new("INVALID_PAGINATION", "limit 必须大于 0"));
    }
    let limit = limit.min(MAX_SEGMENT_PAGE_SIZE);
    ensure_chapter_exists(database, chapter_id).await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM segments WHERE chapter_id = ? AND status <> 'superseded'",
    )
    .bind(chapter_id)
    .fetch_one(database.pool())
    .await?;
    let rows = sqlx::query_as::<_, (String, String, i64, String, Option<String>, i64, String, Option<String>, String, String)>(
        "SELECT id, chapter_id, order_index, original_text, reading_text, speak_enabled, status, current_audio_id, created_at, updated_at
         FROM segments WHERE chapter_id = ? AND status <> 'superseded' ORDER BY order_index LIMIT ? OFFSET ?")
        .bind(chapter_id).bind(limit).bind(offset).fetch_all(database.pool()).await?;
    let items = rows
        .into_iter()
        .map(
            |(
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
            )| Segment {
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
            },
        )
        .collect();
    Ok(PaginatedSegments {
        items,
        total,
        offset,
        limit,
    })
}

pub async fn get_segment(database: &Database, segment_id: &str) -> AppResult<Segment> {
    let row = sqlx::query_as::<_, (String, String, i64, String, Option<String>, i64, String, Option<String>, String, String)>(
        "SELECT id, chapter_id, order_index, original_text, reading_text, speak_enabled, status, current_audio_id, created_at, updated_at FROM segments WHERE id = ? AND status <> 'superseded'")
        .bind(segment_id).fetch_optional(database.pool()).await?;
    row.map(
        |(
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
        )| Segment {
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
        },
    )
    .ok_or_else(|| AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"))
}

pub async fn get_book_id_for_segment(database: &Database, segment_id: &str) -> AppResult<String> {
    sqlx::query_scalar(
        "SELECT c.book_id FROM segments s JOIN chapters c ON c.id = s.chapter_id WHERE s.id = ?",
    )
    .bind(segment_id)
    .fetch_optional(database.pool())
    .await?
    .ok_or_else(|| AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"))
}

pub(crate) async fn get_book_id_for_segment_tx(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    segment_id: &str,
) -> AppResult<String> {
    sqlx::query_scalar(
        "SELECT c.book_id FROM segments s JOIN chapters c ON c.id = s.chapter_id WHERE s.id = ?",
    )
    .bind(segment_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| AppError::new("DB_ERROR", error.to_string()))?
    .ok_or_else(|| AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"))
}

pub async fn delete_book(database: &Database, book_id: &str) -> AppResult<()> {
    let result: SqliteQueryResult = sqlx::query("DELETE FROM books WHERE id = ?")
        .bind(book_id)
        .execute(database.pool())
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::new("BOOK_NOT_FOUND", "Book 不存在"));
    }
    Ok(())
}

async fn ensure_book_exists(database: &Database, book_id: &str) -> AppResult<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM books WHERE id = ?")
        .bind(book_id)
        .fetch_optional(database.pool())
        .await?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(AppError::new("BOOK_NOT_FOUND", "Book 不存在"))
    }
}

async fn ensure_chapter_exists(database: &Database, chapter_id: &str) -> AppResult<()> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM chapters WHERE id = ?")
        .bind(chapter_id)
        .fetch_optional(database.pool())
        .await?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(AppError::new("CHAPTER_NOT_FOUND", "Chapter 不存在"))
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
    use super::{
        delete_book, get_book_text_stats, import_txt_book, list_books, list_chapters,
        list_segments, persist_book,
    };
    use crate::db::Database;
    use unicode_segmentation::UnicodeSegmentation;
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path =
            std::env::temp_dir().join(format!("ancient-medical-books-{}.sqlite", Uuid::now_v7()));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    #[test]
    fn transaction_rolls_back_on_segment_failure() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let result = persist_book(
                &database,
                "测试书",
                "test.txt",
                &["甲".to_string(), "乙".to_string()],
                Some(1),
            )
            .await;
            assert!(result.is_err());
            assert!(list_books(&database).await.expect("list books").is_empty());
        });
    }

    #[test]
    fn book_chapter_and_paginated_segments_are_persisted() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let result = persist_book(
                &database,
                "测试书",
                "test.txt",
                &["甲".to_string(), "乙".to_string(), "丙".to_string()],
                None,
            )
            .await
            .expect("persist book");
            let books = list_books(&database).await.expect("list books");
            assert_eq!(books[0].segment_count, 3);
            let chapters = list_chapters(&database, &result.book.book.id)
                .await
                .expect("list chapters");
            let page = list_segments(&database, &chapters[0].chapter.id, 1, 1)
                .await
                .expect("list segments");
            assert_eq!(page.total, 3);
            assert_eq!(page.items[0].original_text, "乙");
        });
    }

    #[test]
    fn deleting_book_cascades_to_chapters_and_segments() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let result = persist_book(&database, "待删除", "delete.txt", &["甲".to_string()], None)
                .await
                .expect("persist book");
            delete_book(&database, &result.book.book.id)
                .await
                .expect("delete book");
            assert!(list_books(&database).await.expect("list books").is_empty());
            assert_eq!(
                list_chapters(&database, &result.book.book.id)
                    .await
                    .expect_err("book should be gone")
                    .code,
                "BOOK_NOT_FOUND"
            );
        });
    }

    #[test]
    fn imports_sample_txt_into_book_chapter_and_segments() {
        let database = temp_db();
        let path = format!(
            "{}/../tests/fixtures/sample_medical_classic.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        tauri::async_runtime::block_on(async {
            let result = import_txt_book(&database, &path, None)
                .await
                .expect("sample import should succeed");
            let chapters = list_chapters(&database, &result.book.book.id)
                .await
                .expect("chapters should exist");
            let page = list_segments(&database, &chapters[0].chapter.id, 0, 100)
                .await
                .expect("segments should exist");
            let character_count = UnicodeSegmentation::graphemes(
                std::fs::read_to_string(&path)
                    .expect("sample should read")
                    .trim(),
                true,
            )
            .count();
            let shortest = page
                .items
                .iter()
                .map(|segment| {
                    UnicodeSegmentation::graphemes(segment.original_text.as_str(), true).count()
                })
                .min()
                .unwrap_or(0);
            let longest = page
                .items
                .iter()
                .map(|segment| {
                    UnicodeSegmentation::graphemes(segment.original_text.as_str(), true).count()
                })
                .max()
                .unwrap_or(0);
            println!("imported sample: characters={character_count}, segments={}, shortest={shortest}, longest={longest}", page.items.len());
            assert_eq!(result.chapter.title.as_deref(), Some("正文"));
            assert_eq!(page.total, result.segment_count);
        });
    }

    #[test]
    fn imports_realbook_into_ordered_chapters_without_speaking_headings() {
        let database = temp_db();
        let path = format!(
            "{}/../realbooks/huangdi_neijing_test_v01.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        tauri::async_runtime::block_on(async {
            let result = import_txt_book(&database, &path, None)
                .await
                .expect("realbook import should succeed");
            let chapters = list_chapters(&database, &result.book.book.id)
                .await
                .expect("realbook chapters should exist");
            let titles = chapters
                .iter()
                .map(|summary| summary.chapter.title.as_deref().unwrap_or(""))
                .collect::<Vec<_>>();
            assert_eq!(
                titles,
                vec![
                    "上古天真论篇第一",
                    "四气调神大论篇第二",
                    "生气通天论篇第三",
                    "金匮真言论篇第四",
                ]
            );
            assert_eq!(
                chapters
                    .iter()
                    .map(|summary| summary.segment_count)
                    .collect::<Vec<_>>(),
                vec![24, 17, 26, 18]
            );
            assert_eq!(result.chapters.len(), 4);
            assert_eq!(result.segment_count, 85);
            let first_page = list_segments(&database, &chapters[0].chapter.id, 0, 100)
                .await
                .expect("first chapter segments should exist");
            assert!(first_page
                .items
                .iter()
                .all(|segment| !segment.original_text.contains("上古天真论篇第一")));
            assert!(first_page
                .items
                .first()
                .is_some_and(|segment| segment.original_text.starts_with("昔在黄帝")));
        });
    }

    #[test]
    fn empty_txt_does_not_create_a_book() {
        let database = temp_db();
        let path = std::env::temp_dir().join(format!("ancient-empty-{}.txt", Uuid::now_v7()));
        std::fs::write(&path, "\n  \r\n").expect("fixture should write");
        tauri::async_runtime::block_on(async {
            let error = import_txt_book(&database, path.to_str().expect("temp path"), None)
                .await
                .expect_err("empty import should fail");
            assert_eq!(error.code, "EMPTY_TEXT");
            assert!(list_books(&database).await.expect("list books").is_empty());
        });
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_books_over_the_han_character_limit_without_residual_rows() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let error = persist_book(
                &database,
                "超限书",
                "too-large.txt",
                &["甲".repeat(5_001)],
                None,
            )
            .await
            .expect_err("book over the limit should fail");
            assert_eq!(error.code, "PROJECT_TEXT_LIMIT_EXCEEDED");
            assert!(list_books(&database).await.expect("list books").is_empty());
        });
    }

    #[test]
    fn reports_han_count_for_existing_book_data() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let result = persist_book(
                &database,
                "统计书",
                "stats.txt",
                &["腧穴𠀀A".to_string(), "脏腑".to_string()],
                None,
            )
            .await
            .expect("persist book");
            let stats = get_book_text_stats(&database, &result.book.book.id)
                .await
                .expect("book stats");
            assert_eq!(stats.han_character_count, 5);
            assert_eq!(stats.segment_count, 2);
        });
    }
}
