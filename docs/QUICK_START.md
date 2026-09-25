# 快速开始

## 使用已发布应用

1. 启动应用并进入“我的古籍”。
2. 导入 UTF-8 编码的 `.txt` 文件；单本最多 5000 个 CJK 汉字。
3. 打开古籍，先分析发音，再处理右侧待确认读音。
4. 在“TTS 设置”中保存腾讯云 SecretId/SecretKey 和音色。凭据保存在本机应用数据目录，不写入 SQLite、日志或前端存储。
5. 对确认完成的段落生成语音；全部段落完成后可导出完整 WAV 或 96 kbps MP3。

## 源码开发

环境要求：Node.js 20+、pnpm 10+、Rust stable、Tauri 2 构建依赖、Python 3.12.x 和 uv。

```bash
pnpm install
uv sync --project worker
pnpm dev
```

如需启动完整 Tauri 开发窗口：

```bash
pnpm tauri:dev
```

Python Worker 源码调试必须使用项目虚拟环境：

```bash
uv run --project worker python --version
uv run --project worker python worker/main.py
```

输出必须是 Python `3.12.x`。不要用系统 `python3`、Xcode Python、Homebrew Python 或 Python 3.13 构建 Worker。正式应用使用随包发布的 PyInstaller Worker，目标机器不需要安装 Python 或 uv。详见 [WORKER_RUNTIME.md](WORKER_RUNTIME.md)。

## 检查

```bash
pnpm test
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
uv run --project worker python -m unittest discover -s worker/tests -p 'test_*.py'
```

真实腾讯云测试需要显式提供环境变量，切勿把值写进命令历史、日志或 Git：

```bash
RUN_TENCENT_TTS_INTEGRATION=1 \\
ANCIENT_TTS_TENCENT_SECRET_ID=... \\
ANCIENT_TTS_TENCENT_SECRET_KEY=... \\
uv run --project worker python -m unittest worker/tests/test_tencent_integration.py -v
```
