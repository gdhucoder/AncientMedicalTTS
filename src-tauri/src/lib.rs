mod audio;
mod commands;
mod db;
mod error;
mod models;
mod security;
mod services;
pub(crate) mod worker;

use crate::{
    db::Database,
    error::{AppError, AppResult},
    worker::WorkerClient,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Manager, Runtime};

pub struct AppState {
    pub(crate) db: Mutex<Option<Database>>,
    pub(crate) worker: Mutex<Option<WorkerClient>>,
    pub(crate) worker_config: Mutex<Option<WorkerLaunchConfig>>,
    pub(crate) tts_busy: Mutex<bool>,
    pub(crate) data_dir: Mutex<Option<PathBuf>>,
    pub(crate) batch: Arc<services::batch_generation_service::BatchGenerationController>,
    pub(crate) export: Arc<services::export_service::ExportController>,
}

#[derive(Clone)]
pub(crate) struct WorkerLaunchConfig {
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<PathBuf>,
    pub(crate) log_path: PathBuf,
    pub(crate) timeout: Duration,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            db: Mutex::new(None),
            worker: Mutex::new(None),
            worker_config: Mutex::new(None),
            tts_busy: Mutex::new(false),
            data_dir: Mutex::new(None),
            batch: Arc::new(services::batch_generation_service::BatchGenerationController::new()),
            export: Arc::new(services::export_service::ExportController::new()),
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            initialize_app(app).map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::ping_worker,
            commands::import_txt_book,
            commands::list_books,
            commands::get_book,
            commands::list_chapters,
            commands::list_segments,
            commands::get_segment,
            commands::update_segment_reading_text,
            commands::restore_segment_reading_text,
            commands::set_segment_speak_enabled,
            commands::split_segment,
            commands::merge_segment_with_previous,
            commands::merge_segment_with_next,
            commands::delete_book,
            commands::analyze_segment_pronunciation,
            commands::reanalyze_book_pronunciation,
            commands::get_segment_reader,
            commands::list_segment_annotations,
            commands::confirm_annotation,
            commands::ignore_annotation,
            commands::reset_annotation,
            commands::create_manual_annotation,
            commands::create_pronunciation_rule,
            commands::create_rule_from_annotation,
            commands::update_pronunciation_rule,
            commands::enable_pronunciation_rule,
            commands::disable_pronunciation_rule,
            commands::get_pronunciation_rule,
            commands::list_book_pronunciation_rules,
            commands::list_global_pronunciation_rules,
            commands::apply_pronunciation_rules_to_book,
            commands::get_book_generation_preflight,
            commands::start_book_audio_generation,
            commands::cancel_book_audio_generation,
            commands::get_batch_generation_state,
            commands::check_ffmpeg,
            commands::get_book_export_preflight,
            commands::export_book_audio,
            commands::cancel_export,
            commands::get_export_state,
            commands::get_tts_settings,
            commands::save_tts_settings,
            commands::get_reader_display_settings,
            commands::save_reader_display_settings,
            commands::get_tencent_voices,
            commands::get_tts_credential_status,
            commands::save_tencent_credentials,
            commands::delete_tencent_credentials,
            commands::test_tts_connection,
            commands::get_api_usage_summary,
            commands::generate_tts_preview,
            commands::get_segment_display_pinyin,
            commands::generate_segment_audio,
            commands::select_audio_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running AncientMedicalTTS");
}

fn initialize_app<R: Runtime>(app: &tauri::App<R>) -> AppResult<()> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| AppError::new("FILE_IO_ERROR", error.to_string()))?;
    std::fs::create_dir_all(data_dir.join("logs"))?;
    if let Err(error) = services::preview_service::cleanup_preview_cache(&data_dir) {
        append_app_log(
            &data_dir.join("logs/app.log"),
            "WARN",
            &format!("试听缓存清理失败: {error}"),
        );
    }
    append_app_log(
        &data_dir.join("logs/app.log"),
        "INFO",
        "application initializing",
    );

    let database_path = data_dir.join("app.sqlite");
    let database = tauri::async_runtime::block_on(Database::open(&database_path))?;
    append_app_log(
        &data_dir.join("logs/app.log"),
        "INFO",
        &format!("database ready: {}", database_path.display()),
    );

    let (worker, worker_config) = spawn_worker(app, &data_dir)?;
    let state = app.state::<AppState>();
    *state
        .db
        .lock()
        .map_err(|_| AppError::new("DB_ERROR", "数据库状态锁不可用"))? = Some(database);
    *state
        .worker
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "Worker 状态锁不可用"))? = Some(worker);
    *state
        .worker_config
        .lock()
        .map_err(|_| AppError::new("WORKER_ERROR", "Worker 配置锁不可用"))? = Some(worker_config);
    *state
        .data_dir
        .lock()
        .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))? =
        Some(data_dir.clone());
    append_app_log(
        &data_dir.join("logs/app.log"),
        "INFO",
        "worker process started",
    );
    Ok(())
}

fn spawn_worker<R: Runtime>(
    app: &tauri::App<R>,
    data_dir: &Path,
) -> AppResult<(WorkerClient, WorkerLaunchConfig)> {
    let log_path = data_dir.join("logs/worker.log");
    if let Ok(explicit_worker) = std::env::var("ANCIENT_MEDICAL_TTS_WORKER") {
        let config = WorkerLaunchConfig {
            program: PathBuf::from(explicit_worker),
            args: Vec::new(),
            log_path: log_path.clone(),
            timeout: WorkerClient::default_timeout(),
        };
        return spawn_config(config, data_dir);
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| AppError::new("WORKER_START_ERROR", error.to_string()))?;
    let packaged_worker_name = if cfg!(windows) {
        "ancient-tts-worker.exe"
    } else {
        "ancient-tts-worker"
    };
    let packaged_worker = [
        resource_dir.join(packaged_worker_name),
        resource_dir
            .join("worker-runtime")
            .join(packaged_worker_name),
    ]
    .into_iter()
    .find(|path| path.is_file());
    if let Some(packaged_worker) = packaged_worker {
        let config = WorkerLaunchConfig {
            program: packaged_worker,
            args: Vec::new(),
            log_path: log_path.clone(),
            timeout: WorkerClient::default_timeout(),
        };
        return spawn_config(config, data_dir);
    }

    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../worker/main.py");
    let worker_project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../worker");
    if let Ok(python) = std::env::var("ANCIENT_MEDICAL_TTS_PYTHON") {
        let config = WorkerLaunchConfig {
            program: PathBuf::from(python),
            args: vec![script],
            log_path: log_path.clone(),
            timeout: Duration::from_secs(5),
        };
        return spawn_config(config, data_dir);
    }

    // Prefer the project's managed environment when running a local build.
    // The system Python fallback is intentionally kept for older development
    // setups, but it commonly lacks pypinyin and the Tencent SDK.
    let local_python = if cfg!(windows) {
        worker_project
            .join(".venv")
            .join("Scripts")
            .join("python.exe")
    } else {
        worker_project.join(".venv").join("bin").join("python")
    };
    if local_python.is_file() {
        let config = WorkerLaunchConfig {
            program: local_python,
            args: vec![script.clone()],
            log_path: log_path.clone(),
            timeout: Duration::from_secs(5),
        };
        return spawn_config(config, data_dir);
    }

    if std::process::Command::new("uv")
        .arg("--version")
        .output()
        .is_ok()
    {
        let config = WorkerLaunchConfig {
            program: PathBuf::from("uv"),
            args: vec![
                PathBuf::from("run"),
                PathBuf::from("--project"),
                worker_project,
                PathBuf::from("python"),
                script,
            ],
            log_path: log_path.clone(),
            timeout: Duration::from_secs(5),
        };
        return spawn_config(config, data_dir);
    }
    spawn_config(
        WorkerLaunchConfig {
            program: PathBuf::from("python3"),
            args: vec![script],
            log_path,
            timeout: Duration::from_secs(5),
        },
        data_dir,
    )
}

fn spawn_config(
    config: WorkerLaunchConfig,
    data_dir: &Path,
) -> AppResult<(WorkerClient, WorkerLaunchConfig)> {
    let credentials = security::credentials::load_tencent_credentials(data_dir)?;
    let mut envs = Vec::new();
    if let Some(credentials) = credentials {
        envs.push((
            security::credentials::SECRET_ID_ENV.to_string(),
            credentials.secret_id,
        ));
        envs.push((
            security::credentials::SECRET_KEY_ENV.to_string(),
            credentials.secret_key,
        ));
    }
    let worker = WorkerClient::spawn_with_env(
        &config.program,
        &config.args,
        &config.log_path,
        config.timeout,
        &envs,
    )?;
    Ok((worker, config))
}

impl AppState {
    pub(crate) fn restart_worker(&self) -> AppResult<()> {
        let config = self
            .worker_config
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "Worker 配置锁不可用"))?
            .clone()
            .ok_or_else(|| AppError::new("WORKER_NOT_RUNNING", "Worker 配置尚未初始化"))?;
        let data_dir = self
            .data_dir
            .lock()
            .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
            .clone()
            .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用数据目录尚未初始化"))?;
        let (mut worker, _) = spawn_config(config, &data_dir)?;
        worker.call("system.ping", serde_json::json!({}))?;
        *self
            .worker
            .lock()
            .map_err(|_| AppError::new("WORKER_ERROR", "Worker 状态锁不可用"))? = Some(worker);
        Ok(())
    }

    pub(crate) fn data_directory(&self) -> AppResult<PathBuf> {
        self.data_dir
            .lock()
            .map_err(|_| AppError::new("FILE_IO_ERROR", "应用目录状态锁不可用"))?
            .clone()
            .ok_or_else(|| AppError::new("FILE_IO_ERROR", "应用数据目录尚未初始化"))
    }
}

pub(crate) fn append_app_log(path: &Path, level: &str, message: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{level} {message}");
    }
}
