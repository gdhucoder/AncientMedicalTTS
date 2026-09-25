use crate::error::{AppError, AppResult};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
};
use tauri::{AppHandle, Manager};

pub(crate) struct FfmpegInfo {
    pub(crate) path: PathBuf,
    pub(crate) version: String,
}

pub(crate) fn check_available(app: &AppHandle, require_mp3: bool) -> AppResult<FfmpegInfo> {
    let path = resolve_path(app)?;
    let version_output = run_probe(&path, &["-version"])?;
    if !version_output.status.success() {
        return Err(AppError::new(
            "FFMPEG_NOT_AVAILABLE",
            format!("FFmpeg 启动失败: {}", stderr_summary(&version_output)),
        ));
    }
    let version = String::from_utf8_lossy(&version_output.stdout)
        .lines()
        .next()
        .unwrap_or("ffmpeg")
        .trim()
        .to_string();
    let encoder_output = run_probe(&path, &["-encoders"])?;
    let encoder_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&encoder_output.stdout),
        String::from_utf8_lossy(&encoder_output.stderr)
    );
    let mp3_encoder_available = encoder_output.status.success()
        && encoder_text.lines().any(|line| line.contains("libmp3lame"));
    if require_mp3 && !mp3_encoder_available {
        return Err(AppError::new(
            "MP3_ENCODER_NOT_AVAILABLE",
            "当前 FFmpeg 未包含 libmp3lame MP3 编码器",
        ));
    }
    Ok(FfmpegInfo { path, version })
}

pub(crate) fn spawn(path: &Path, args: &[String]) -> AppResult<Child> {
    Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::new("FFMPEG_NOT_AVAILABLE", format!("无法启动 FFmpeg: {error}")))
}

fn run_probe(path: &Path, args: &[&str]) -> AppResult<Output> {
    Command::new(path).args(args).output().map_err(|error| {
        AppError::new(
            "FFMPEG_NOT_AVAILABLE",
            format!("无法执行 FFmpeg 检查: {error}"),
        )
    })
}

fn resolve_path(app: &AppHandle) -> AppResult<PathBuf> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| AppError::new("FFMPEG_NOT_AVAILABLE", error.to_string()))?;
    let target_name = format!("ffmpeg-{}", target_triple());
    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let mut bundled_candidates = vec![
        resource_dir.join(&target_name),
        resource_dir.join(executable_name),
    ];
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            bundled_candidates.push(parent.join(&target_name));
            bundled_candidates.push(parent.join(executable_name));
        }
    }
    if let Some(path) = bundled_candidates.into_iter().find(|path| path.is_file()) {
        return Ok(path);
    }

    #[cfg(debug_assertions)]
    {
        if let Ok(explicit_path) = std::env::var("ANCIENT_MEDICAL_TTS_FFMPEG") {
            let path = PathBuf::from(explicit_path);
            if path.is_file() {
                return Ok(path);
            }
        }
        if let Some(path) = find_on_path() {
            return Ok(path);
        }
    }

    Err(AppError::new(
        "FFMPEG_NOT_AVAILABLE",
        format!("应用内置 FFmpeg 不存在: {target_name}"),
    ))
}

#[cfg(debug_assertions)]
fn find_on_path() -> Option<PathBuf> {
    let executable_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|directory| {
        let path = directory.join(executable_name);
        path.is_file().then_some(path)
    })
}

fn target_triple() -> &'static str {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else {
        "unsupported-target"
    }
}

fn stderr_summary(output: &Output) -> String {
    let text = String::from_utf8_lossy(&output.stderr);
    let mut summary = text.trim().to_string();
    if summary.len() > 8 * 1024 {
        summary = summary[summary.len() - 8 * 1024..].to_string();
    }
    summary
}

#[cfg(test)]
mod tests {
    #[test]
    fn target_triple_is_supported_on_the_current_build_target() {
        assert_ne!(super::target_triple(), "unsupported-target");
    }
}
