use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct WorkerClient {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<String>,
    timeout: Duration,
}

impl WorkerClient {
    pub fn spawn_with_env(
        program: &Path,
        args: &[PathBuf],
        log_path: &Path,
        timeout: Duration,
        envs: &[(String, String)],
    ) -> AppResult<Self> {
        let mut command = Command::new(program);
        command
            .args(args)
            .envs(envs.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            AppError::new(
                "WORKER_START_ERROR",
                format!("无法启动 Python Worker: {error}"),
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::new("WORKER_START_ERROR", "Worker stdin 不可用"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::new("WORKER_START_ERROR", "Worker stdout 不可用"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::new("WORKER_START_ERROR", "Worker stderr 不可用"))?;
        let (sender, responses) = mpsc::channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(value) => {
                        if sender.send(value).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let log_path = log_path.to_path_buf();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                append_worker_log(&log_path, &line);
            }
        });

        Ok(Self {
            child,
            stdin,
            responses,
            timeout,
        })
    }

    pub fn default_timeout() -> Duration {
        DEFAULT_TIMEOUT
    }

    pub fn call(&mut self, method: &str, params: Value) -> AppResult<Value> {
        self.call_with_timeout(method, params, self.timeout)
    }

    pub fn call_with_timeout(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> AppResult<Value> {
        self.ensure_running()?;
        let request_id = uuid::Uuid::now_v7().to_string();
        let request = json!({ "id": request_id, "method": method, "params": params });
        serde_json::to_writer(&mut self.stdin, &request)
            .map_err(|error| AppError::new("WORKER_WRITE_ERROR", error.to_string()))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| AppError::new("WORKER_WRITE_ERROR", error.to_string()))?;
        self.stdin
            .flush()
            .map_err(|error| AppError::new("WORKER_WRITE_ERROR", error.to_string()))?;

        let line = match self.responses.recv_timeout(timeout) {
            Ok(line) => line,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(AppError::new(
                    "WORKER_TIMEOUT",
                    format!("Worker 在 {:?} 内未响应", timeout),
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AppError::new("WORKER_EXITED", "Worker 已退出"));
            }
        };
        let response: Value = serde_json::from_str(&line).map_err(|error| {
            AppError::new(
                "WORKER_INVALID_RESPONSE",
                format!("Worker 返回非法 JSON: {error}"),
            )
        })?;
        if response.get("id").and_then(Value::as_str) != Some(request_id.as_str()) {
            return Err(AppError::new(
                "WORKER_PROTOCOL_ERROR",
                "Worker response.id 与 request.id 不一致",
            ));
        }
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            let code = response
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("WORKER_ERROR");
            let message = response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("Worker 请求失败");
            return Err(AppError::new(code, message));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    pub fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    fn ensure_running(&mut self) -> AppResult<()> {
        if self.is_running() {
            Ok(())
        } else {
            Err(AppError::new("WORKER_NOT_RUNNING", "Python Worker 未运行"))
        }
    }
}

impl Drop for WorkerClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn append_worker_log(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{} {}", chrono_like_timestamp(), line);
    }
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::WorkerClient;
    use std::{path::PathBuf, time::Duration};

    fn python() -> PathBuf {
        PathBuf::from(std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string()))
    }
    fn worker_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../worker/main.py")
    }
    fn log_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ancient-{name}-worker.log"))
    }

    #[test]
    fn worker_ping_round_trip() {
        let mut worker = WorkerClient::spawn_with_env(
            &python(),
            &[worker_script()],
            &log_path("ping"),
            WorkerClient::default_timeout(),
            &[],
        )
        .expect("worker starts");
        let response = worker
            .call("system.ping", serde_json::json!({}))
            .expect("ping succeeds");
        assert_eq!(response["version"], "0.2.0");
    }

    #[test]
    fn timeout_is_reported() {
        let mut worker = WorkerClient::spawn_with_env(
            &python(),
            &[
                PathBuf::from("-c"),
                PathBuf::from("import time; time.sleep(1)"),
            ],
            &log_path("timeout"),
            Duration::from_millis(50),
            &[],
        )
        .expect("worker starts");
        let error = worker
            .call("system.ping", serde_json::json!({}))
            .expect_err("call should time out");
        assert_eq!(error.code, "WORKER_TIMEOUT");
    }

    #[test]
    fn invalid_json_is_an_error_not_a_panic() {
        let mut worker = WorkerClient::spawn_with_env(
            &python(),
            &[
                PathBuf::from("-c"),
                PathBuf::from("print('not-json', flush=True)"),
            ],
            &log_path("invalid"),
            WorkerClient::default_timeout(),
            &[],
        )
        .expect("worker starts");
        let error = worker
            .call("system.ping", serde_json::json!({}))
            .expect_err("invalid json should fail");
        assert_eq!(error.code, "WORKER_INVALID_RESPONSE");
    }
}
