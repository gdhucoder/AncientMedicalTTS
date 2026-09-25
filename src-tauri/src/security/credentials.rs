use crate::{
    error::{AppError, AppResult},
    models::CredentialStatus,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const SECRET_ID_ENV: &str = "ANCIENT_TTS_TENCENT_SECRET_ID";
pub const SECRET_KEY_ENV: &str = "ANCIENT_TTS_TENCENT_SECRET_KEY";
const CREDENTIALS_FILE_NAME: &str = "tencent_credentials.json";

#[derive(Debug, Clone)]
pub struct TencentCredentials {
    pub secret_id: String,
    pub secret_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredCredentials {
    #[serde(rename = "ANCIENT_TTS_TENCENT_SECRET_ID", default)]
    secret_id: Option<String>,
    #[serde(rename = "ANCIENT_TTS_TENCENT_SECRET_KEY", default)]
    secret_key: Option<String>,
}

pub fn credentials_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CREDENTIALS_FILE_NAME)
}

fn read_stored_credentials(data_dir: &Path) -> AppResult<Option<StoredCredentials>> {
    let path = credentials_path(data_dir);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AppError::new(
                "CREDENTIAL_STORE_ERROR",
                format!("读取凭据文件失败: {error}"),
            ));
        }
    };

    serde_json::from_str(&contents).map(Some).map_err(|_| {
        AppError::new(
            "CREDENTIAL_STORE_ERROR",
            "凭据文件格式无效，请在设置中重新保存腾讯云凭据",
        )
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn write_private_file(path: &Path, contents: &str) -> AppResult<()> {
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, contents).map_err(|error| {
        AppError::new(
            "CREDENTIAL_STORE_ERROR",
            format!("写入凭据文件失败: {error}"),
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
            AppError::new(
                "CREDENTIAL_STORE_ERROR",
                format!("设置凭据文件权限失败: {error}"),
            )
        })?;
    }

    let rename_result: std::io::Result<()> = {
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path)
        } else {
            Ok(())
        }

        #[cfg(not(windows))]
        Ok(())
    };
    if let Err(error) = rename_result {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::new(
            "CREDENTIAL_STORE_ERROR",
            format!("更新凭据文件失败: {error}"),
        ));
    }

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::new(
            "CREDENTIAL_STORE_ERROR",
            format!("保存凭据文件失败: {error}"),
        ));
    }
    Ok(())
}

pub fn status(data_dir: &Path) -> AppResult<CredentialStatus> {
    let stored = read_stored_credentials(data_dir)?;
    Ok(CredentialStatus {
        secret_id_configured: stored
            .as_ref()
            .and_then(|credentials| non_empty(credentials.secret_id.clone()))
            .is_some(),
        secret_key_configured: stored
            .as_ref()
            .and_then(|credentials| non_empty(credentials.secret_key.clone()))
            .is_some(),
    })
}

pub fn load_tencent_credentials(data_dir: &Path) -> AppResult<Option<TencentCredentials>> {
    let stored = read_stored_credentials(data_dir)?;
    let secret_id = stored
        .as_ref()
        .and_then(|credentials| non_empty(credentials.secret_id.clone()));
    let secret_key = stored
        .as_ref()
        .and_then(|credentials| non_empty(credentials.secret_key.clone()));
    match (secret_id, secret_key) {
        (Some(secret_id), Some(secret_key)) => Ok(Some(TencentCredentials {
            secret_id,
            secret_key,
        })),
        (None, None) => Ok(None),
        _ => Err(AppError::new(
            "TTS_CREDENTIALS_INCOMPLETE",
            "腾讯云 SecretId 与 SecretKey 必须同时配置",
        )),
    }
}

pub fn save_tencent_credentials(
    data_dir: &Path,
    secret_id: &str,
    secret_key: &str,
) -> AppResult<()> {
    let secret_id = secret_id.trim();
    let secret_key = secret_key.trim();
    if secret_id.is_empty() || secret_key.is_empty() {
        return Err(AppError::new(
            "TTS_CREDENTIALS_INVALID",
            "SecretId 和 SecretKey 不能为空",
        ));
    }

    fs::create_dir_all(data_dir).map_err(|error| {
        AppError::new(
            "CREDENTIAL_STORE_ERROR",
            format!("创建应用数据目录失败: {error}"),
        )
    })?;
    let contents = serde_json::to_string_pretty(&StoredCredentials {
        secret_id: Some(secret_id.to_string()),
        secret_key: Some(secret_key.to_string()),
    })
    .map_err(|error| AppError::new("CREDENTIAL_STORE_ERROR", error.to_string()))?;
    write_private_file(&credentials_path(data_dir), &contents)
}

pub fn delete_tencent_credentials(data_dir: &Path) -> AppResult<()> {
    match fs::remove_file(credentials_path(data_dir)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::new(
            "CREDENTIAL_STORE_ERROR",
            format!("删除凭据文件失败: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temporary_data_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ancient-medical-tts-credentials-{}",
            Uuid::now_v7()
        ))
    }

    #[test]
    fn credentials_round_trip_uses_application_data_file() {
        let data_dir = temporary_data_dir();
        save_tencent_credentials(&data_dir, "id-value", "key-value").expect("save credentials");

        assert_eq!(
            credentials_path(&data_dir),
            data_dir.join(CREDENTIALS_FILE_NAME)
        );
        let credential_status = status(&data_dir).expect("credential status");
        assert!(credential_status.secret_id_configured);
        assert!(credential_status.secret_key_configured);
        let credentials = load_tencent_credentials(&data_dir)
            .expect("load credentials")
            .expect("credentials should exist");
        assert_eq!(credentials.secret_id, "id-value");
        assert_eq!(credentials.secret_key, "key-value");

        delete_tencent_credentials(&data_dir).expect("delete credentials");
        assert!(load_tencent_credentials(&data_dir)
            .expect("load after delete")
            .is_none());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn incomplete_credentials_are_reported() {
        let data_dir = temporary_data_dir();
        save_tencent_credentials(&data_dir, "id-value", "key-value").expect("save credentials");
        fs::write(
            credentials_path(&data_dir),
            format!(r#"{{"{SECRET_ID_ENV}":"id-value"}}"#),
        )
        .expect("write incomplete credentials");

        let error =
            load_tencent_credentials(&data_dir).expect_err("credentials should be incomplete");
        assert_eq!(error.code, "TTS_CREDENTIALS_INCOMPLETE");
        assert!(
            status(&data_dir)
                .expect("credential status")
                .secret_id_configured
        );
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn malformed_credentials_do_not_panic() {
        let data_dir = temporary_data_dir();
        fs::create_dir_all(&data_dir).expect("create data dir");
        fs::write(credentials_path(&data_dir), "not-json").expect("write malformed credentials");

        let error = status(&data_dir).expect_err("malformed credentials should fail safely");
        assert_eq!(error.code, "CREDENTIAL_STORE_ERROR");
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn blank_credentials_are_rejected() {
        let data_dir = temporary_data_dir();
        let error = save_tencent_credentials(&data_dir, " ", "key-value")
            .expect_err("blank secret id should be rejected");
        assert_eq!(error.code, "TTS_CREDENTIALS_INVALID");
        let _ = fs::remove_dir_all(data_dir);
    }
}
