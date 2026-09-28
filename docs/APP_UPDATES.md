# 应用在线更新

AncientMedicalTTS 使用 Tauri 官方 updater 插件从 GitHub Release 获取更新。应用启动后会延迟检查一次；本地会记住最近一次检查时间，24 小时内不会重复自动检查。用户也可以在顶部点击“检查更新”。

## 更新安全

更新包必须使用 Tauri signer 签名。应用内只保存公钥，私钥不能提交 Git，也不能写入 `.env`、前端代码或安装包。当前仓库的 GitHub Actions 使用以下 GitHub Actions Secrets：

- `TAURI_SIGNING_PRIVATE_KEY`：完整私钥内容；
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：如果私钥设置了密码则填写，否则留空。

发布新版本前，必须确认上述私钥仍然可用。丢失私钥后，已经安装旧版本的用户无法验证后续更新，只能重新下载安装包。

## 发布流程

1. 更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中的版本；
2. 创建并推送版本 tag，例如 `v0.1.1`；
3. GitHub Actions 在 macOS Apple Silicon、macOS Intel 和 Windows x64 runner 上构建安装包；
4. 带有签名的 macOS `.app.tar.gz`、Windows NSIS `setup.exe` 更新包和对应 `.sig` 会上传到 GitHub Release；同一个 `setup.exe` 也可作为手动安装包发布；
5. 发布任务生成 `latest.json`，并通过 GitHub Contents API 同步到 `main` 分支的 `site/latest.json`；应用从以下固定地址检查：

   `https://raw.githubusercontent.com/gdhucoder/AncientMedicalTTS/main/site/latest.json`

没有完整签名产物时，工作流不会生成 `latest.json`，应用不会安装不完整的更新。

## 用户侧行为

发现新版本后，应用只显示提示，由用户选择“立即更新”或“稍后”。更新下载和安装期间会显示进度。Windows 安装器会按 Tauri updater 的规则退出并完成安装；macOS 安装完成后应用会重新启动。

开发环境直接运行 `pnpm tauri:dev` 时不会生成更新包。只有构建环境提供 `TAURI_SIGNING_PRIVATE_KEY` 时，`scripts/tauri-cli.mjs` 才会打开 `createUpdaterArtifacts` 并生成更新产物。
