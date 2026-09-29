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
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    command
        .spawn()
        .map_err(|error| AppError::new("FFMPEG_NOT_AVAILABLE", format!("无法启动 FFmpeg: {error}")))
}

pub(crate) fn run_output(path: &Path, args: &[String]) -> AppResult<Output> {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    command.output().map_err(|error| {
        AppError::new(
            "FFMPEG_NOT_AVAILABLE",
            format!("无法启动应用内置 FFmpeg: {error}"),
        )
    })
}

pub(crate) fn build_concat_list(paths: &[PathBuf]) -> AppResult<String> {
    if paths.is_empty() {
        return Err(AppError::new("AUDIO_FILE_MISSING", "没有可拼接的音频片段"));
    }
    let mut result = String::from("ffconcat version 1.0\n");
    for path in paths {
        let path = path.to_string_lossy().replace('\\', "/");
        if path.contains('\n') || path.contains('\r') {
            return Err(AppError::new(
                "INVALID_EXPORT_PATH",
                "音频路径包含换行符，无法安全写入 concat list",
            ));
        }
        let escaped = path.replace('\'', "'\\''");
        result.push_str("file '");
        result.push_str(&escaped);
        result.push_str("'\n");
    }
    Ok(result)
}

pub(crate) fn concat_wav_args(concat_path: &Path, output_path: &Path) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-f".to_string(),
        "concat".to_string(),
        "-safe".to_string(),
        "0".to_string(),
        "-i".to_string(),
        concat_path.to_string_lossy().to_string(),
        "-c".to_string(),
        "copy".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
}

pub(crate) fn encode_mp3_args(
    input_path: &Path,
    output_path: &Path,
    sample_rate: i64,
    channels: i64,
    title: &str,
) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-vn".to_string(),
        "-codec:a".to_string(),
        "libmp3lame".to_string(),
        "-b:a".to_string(),
        "96k".to_string(),
        "-ar".to_string(),
        sample_rate.to_string(),
        "-ac".to_string(),
        channels.to_string(),
        "-metadata".to_string(),
        format!("title={title}"),
        output_path.to_string_lossy().to_string(),
    ]
}

pub(crate) fn parse_duration_ms(stderr: &[u8]) -> Option<i64> {
    let text = String::from_utf8_lossy(stderr);
    let marker = "Duration: ";
    let start = text.find(marker)? + marker.len();
    let value = text[start..].split([',', '\n', '\r']).next()?.trim();
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<i64>().ok()?;
    let minutes = parts.next()?.parse::<i64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if hours < 0 || minutes < 0 || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Some(((hours * 3600 + minutes * 60) as f64 * 1000.0 + seconds * 1000.0).round() as i64)
}

pub(crate) fn probe_duration_ms(path: &Path, ffmpeg_path: &Path) -> AppResult<i64> {
    let args = vec![
        "-hide_banner".to_string(),
        "-i".to_string(),
        path.to_string_lossy().to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];
    let output = run_output(ffmpeg_path, &args)?;
    parse_duration_ms(&output.stderr).ok_or_else(|| {
        AppError::new(
            "PUBLICATION_AUDIO_DURATION_UNAVAILABLE",
            "FFmpeg 未返回可解析的章节音频时长",
        )
    })
}

fn run_probe(path: &Path, args: &[&str]) -> AppResult<Output> {
    let mut command = Command::new(path);
    command.args(args);
    hide_console_window(&mut command);
    command.output().map_err(|error| {
        AppError::new(
            "FFMPEG_NOT_AVAILABLE",
            format!("无法执行 FFmpeg 检查: {error}"),
        )
    })
}

fn hide_console_window(command: &mut Command) {
    #[cfg(not(windows))]
    let _ = command;

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
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

    #[test]
    fn concat_list_escapes_paths_and_duration_parser_is_millisecond_based() {
        let paths = vec![std::path::PathBuf::from(
            "C:\\Users\\测试\\Sean's Book\\001.wav",
        )];
        assert_eq!(
            super::build_concat_list(&paths).expect("concat list"),
            "ffconcat version 1.0\nfile 'C:/Users/测试/Sean'\\''s Book/001.wav'\n"
        );
        assert_eq!(
            super::parse_duration_ms(b"Duration: 01:02:03.450, start: 0.000000"),
            Some(3_723_450)
        );
    }
}
