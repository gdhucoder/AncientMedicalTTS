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
        assert_eq!(schema_version, 11);
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
}
