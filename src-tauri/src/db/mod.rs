use crate::error::{AppError, AppResult};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    path: PathBuf,
}

impl Database {
    pub async fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio_fs::create_dir_all(parent)
                .await
                .map_err(AppError::from)?;
        }
        let database_exists = path
            .metadata()
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false);

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(|error| AppError::new("DB_ERROR", error.to_string()))?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        if database_exists {
            backup_database(&pool, &path).await?;
        }
        sqlx::migrate!()
            .run(&pool)
            .await
            .map_err(|error| AppError::new("DB_MIGRATION_ERROR", error.to_string()))?;

        Ok(Self { pool, path })
    }

    pub async fn health(&self) -> AppResult<()> {
        let _: i64 = sqlx::query_scalar("SELECT schema_version FROM app_meta LIMIT 1")
            .fetch_one(&self.pool)
            .await?;
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&self.pool)
            .await?;
        if foreign_keys != 1 {
            return Err(AppError::new(
                "DB_ERROR",
                "SQLite foreign_keys pragma 未开启",
            ));
        }
        let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&self.pool)
            .await?;
        if journal_mode.to_ascii_lowercase() != "wal" {
            return Err(AppError::new("DB_ERROR", "SQLite WAL journal 未开启"));
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

async fn backup_database(pool: &SqlitePool, path: &Path) -> AppResult<()> {
    let backup_path = path.with_file_name(format!(
        "{}.backup",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("app.sqlite")
    ));
    if backup_path.exists() {
        std::fs::remove_file(&backup_path)
            .map_err(|error| AppError::new("DB_BACKUP_ERROR", error.to_string()))?;
    }

    // VACUUM INTO creates a consistent SQLite snapshot, including data that is
    // currently in the WAL file. The destination path is escaped as a SQLite
    // string literal and is never passed through a shell.
    let escaped_path = backup_path.to_string_lossy().replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{escaped_path}'"))
        .execute(pool)
        .await
        .map_err(|error| AppError::new("DB_BACKUP_ERROR", error.to_string()))?;
    Ok(())
}

mod tokio_fs {
    pub async fn create_dir_all(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }
}

#[cfg(test)]
mod tests {
    use super::Database;
    use uuid::Uuid;

    #[test]
    fn sqlite_initializes_and_migrates() {
        let path = std::env::temp_dir().join(format!("ancient-medical-{}.sqlite", Uuid::now_v7()));
        let database = tauri::async_runtime::block_on(Database::open(&path))
            .expect("database should initialize");
        tauri::async_runtime::block_on(database.health()).expect("database should be healthy");
        assert!(database.path().exists());
        let schema_version: i64 = tauri::async_runtime::block_on(async {
            sqlx::query_scalar("SELECT schema_version FROM app_meta LIMIT 1")
                .fetch_one(database.pool())
                .await
                .expect("schema version should exist")
        });
        assert_eq!(schema_version, 15);
        let book_columns: Vec<String> = tauri::async_runtime::block_on(async {
            sqlx::query_scalar("SELECT name FROM pragma_table_info('books')")
                .fetch_all(database.pool())
                .await
                .expect("book columns should exist")
        });
        assert!(book_columns.contains(&"import_format".to_string()));
        assert!(book_columns.contains(&"import_pronunciation_mode".to_string()));
        let chapter_columns: Vec<String> = tauri::async_runtime::block_on(async {
            sqlx::query_scalar("SELECT name FROM pragma_table_info('chapters')")
                .fetch_all(database.pool())
                .await
                .expect("chapter columns should exist")
        });
        assert!(chapter_columns.contains(&"collection".to_string()));
        let segment_columns: Vec<String> = tauri::async_runtime::block_on(async {
            sqlx::query_scalar("SELECT name FROM pragma_table_info('segments')")
                .fetch_all(database.pool())
                .await
                .expect("segment columns should exist")
        });
        assert!(segment_columns.contains(&"corrected_text".to_string()));
        assert!(segment_columns.contains(&"translation".to_string()));
        let annotation_columns: Vec<String> = tauri::async_runtime::block_on(async {
            sqlx::query_scalar("SELECT name FROM pragma_table_info('segment_annotations')")
                .fetch_all(database.pool())
                .await
                .expect("annotation columns should exist")
        });
        assert!(annotation_columns.contains(&"rule_type".to_string()));
        assert!(annotation_columns.contains(&"confidence".to_string()));
        drop(database);
        let backup_path = path.with_file_name(format!(
            "{}.backup",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ));
        let reopened = tauri::async_runtime::block_on(Database::open(&path))
            .expect("database should reopen and back up");
        assert!(backup_path.exists());
        assert!(
            std::fs::metadata(&backup_path)
                .expect("backup metadata should exist")
                .len()
                > 0
        );
        drop(reopened);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup_path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }

    #[test]
    fn upgrades_schema_v14_and_preserves_existing_segment_text() {
        let path = std::env::temp_dir().join(format!(
            "ancient-medical-v14-upgrade-{}.sqlite",
            Uuid::now_v7()
        ));
        let database = tauri::async_runtime::block_on(Database::open(&path))
            .expect("database should initialize at the latest schema");
        tauri::async_runtime::block_on(async {
            sqlx::query(
                "INSERT INTO books (id, title, created_at, updated_at) VALUES ('book-v14', '迁移测试', 'now', 'now')",
            )
            .execute(database.pool())
            .await
            .expect("book should be inserted");
            sqlx::query(
                "INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES ('chapter-v14', 'book-v14', '正文', 0, 'now', 'now')",
            )
            .execute(database.pool())
            .await
            .expect("chapter should be inserted");
            sqlx::query(
                "INSERT INTO segments (id, chapter_id, order_index, original_text, corrected_text, created_at, updated_at) VALUES ('segment-v14', 'chapter-v14', 0, '原文', '校对原文', 'now', 'now')",
            )
            .execute(database.pool())
            .await
            .expect("segment should be inserted");

            // Recreate the v0.1.0-rc9 schema: migration 0014 is present, but
            // the newly added 0015 annotated-import columns are not.
            for statement in [
                "ALTER TABLE books DROP COLUMN import_format",
                "ALTER TABLE books DROP COLUMN import_format_version",
                "ALTER TABLE books DROP COLUMN import_pronunciation_mode",
                "ALTER TABLE books DROP COLUMN import_dataset_id",
                "ALTER TABLE books DROP COLUMN imported_at",
                "ALTER TABLE chapters DROP COLUMN collection",
                "ALTER TABLE chapters DROP COLUMN subtitle",
                "ALTER TABLE segments DROP COLUMN translation",
                "DELETE FROM _sqlx_migrations WHERE version = 15",
                "UPDATE app_meta SET schema_version = 14",
            ] {
                sqlx::query(statement)
                    .execute(database.pool())
                    .await
                    .expect("v14 schema should be recreated");
            }
        });
        drop(database);

        let upgraded = tauri::async_runtime::block_on(Database::open(&path))
            .expect("v14 database should upgrade to v15");
        let (schema_version, original_text, corrected_text): (i64, String, Option<String>) =
            tauri::async_runtime::block_on(async {
                sqlx::query_as(
                    "SELECT app_meta.schema_version, segments.original_text, segments.corrected_text
                     FROM app_meta CROSS JOIN segments WHERE segments.id = 'segment-v14'",
                )
                .fetch_one(upgraded.pool())
                .await
                .expect("existing segment should remain after migration")
            });
        assert_eq!(schema_version, 15);
        assert_eq!(original_text, "原文");
        assert_eq!(corrected_text.as_deref(), Some("校对原文"));
        let translation_column: Option<String> = tauri::async_runtime::block_on(async {
            sqlx::query_scalar(
                "SELECT name FROM pragma_table_info('segments') WHERE name = 'translation'",
            )
            .fetch_optional(upgraded.pool())
            .await
            .expect("translation column should be queryable")
        });
        assert_eq!(translation_column.as_deref(), Some("translation"));
        drop(upgraded);

        let backup_path = path.with_file_name(format!(
            "{}.backup",
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup_path);
        let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    }
}
