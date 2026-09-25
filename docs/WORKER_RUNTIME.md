# Python Worker 运行时与发布约束

## 结论

AncientMedicalTTS 的 Python Worker 只有两种合法运行方式：

1. **正式发布**：启动随应用发布的 PyInstaller Worker 可执行文件。此时目标机器不需要安装 Python、uv 或任何 Python 依赖。
2. **源码开发/本机调试**：使用由 uv 创建、且明确基于 Python 3.12 的 `worker/.venv`。

生产环境禁止依赖系统 `python`、`python3`、Xcode 自带 Python、Homebrew Python 或用户的 PATH。

## 本次错误的根因

本次 macOS Application 中出现 `No module named 'pypinyin'` 和 `No module named 'tencentcloud'`，并不是 PyInstaller 选择了错误的 Python 版本。实际启动链路是：

1. App 包内没有 `ancient-tts-worker` 这个 PyInstaller 可执行文件，因此没有进入 bundled Worker 分支；
2. 从 Finder 启动的 GUI 进程没有继承终端的完整 PATH，Rust 找不到 `uv`；
3. 启动器进入历史兼容回退，执行 `python3 worker/main.py`；
4. macOS 本机的 `python3` 解析到了 Xcode 自带的 Python 3.9；
5. Python 3.9 环境没有项目的 Python 3.12 虚拟环境依赖，于是缺少 `pypinyin` 和腾讯云 SDK。

因此，问题本质是“没有使用 bundled Worker 时，源码 Worker 的解释器回退不受控”，而不是 PyInstaller 运行时在选择解释器。PyInstaller one-file Worker 本身是原生可执行文件，运行时不应再选择或依赖外部 Python。

## Python 版本硬约束

`worker/pyproject.toml` 当前声明：

```toml
requires-python = ">=3.12,<3.13"
```

这意味着：

- 源码 Worker 只能使用 Python **3.12.x**；
- `worker/.venv` 必须由 Python 3.12 创建；
- 依赖必须通过 `uv sync --project worker` 或 `uv run --project worker ...` 安装和运行；
- Python 3.9、3.10、3.11、3.13 以及未确认版本的 `python3` 均不得作为项目 Worker 解释器；
- 构建 PyInstaller Worker 的 Python 也必须是 Python 3.12.x；
- PyInstaller Worker 构建完成后，App 启动时不得再调用外部 Python。

开发机可以用以下命令确认解释器版本：

```bash
uv python pin 3.12
uv sync --project worker
uv run --project worker python --version
uv run --project worker python -c "import pypinyin, tencentcloud; print('worker dependencies ok')"
```

第三条命令输出的 Python 版本必须是 `3.12.x`。如果 `uv` 找不到或虚拟环境不存在，应明确报错，不能把错误静默转交给系统 `python3`。

## Worker 启动优先级

当前 Rust 启动器的实际顺序如下：

```text
ANCIENT_MEDICAL_TTS_WORKER        （显式开发覆盖）
        ↓ 不存在
App Resources/worker-runtime/ancient-tts-worker  （正式发布首选）
        ↓ 不存在
ANCIENT_MEDICAL_TTS_PYTHON        （显式源码调试覆盖）
        ↓ 未设置
worker/.venv/bin/python            （macOS/Linux 本机调试）
worker/.venv/Scripts/python.exe    （Windows 本机调试）
        ↓ 不存在
uv run --project worker python     （开发环境回退）
        ↓ 不可用
系统 python3                      （历史兼容回退，不得用于生产）
```

其中最后的系统 Python 回退只为旧开发环境保留。正式构建必须满足 App Resources 中存在目标平台的 PyInstaller Worker，并在构建验收中确认没有走到系统 Python 回退。

## PyInstaller 构建约束

PyInstaller Worker 必须：

- 使用 Python 3.12.x 构建；
- 固定包含 `pypinyin`、腾讯云 TTS SDK、`worker/dictionaries/` 和所有动态导入的 Worker 模块；
- 输出名称为 macOS/Linux `ancient-tts-worker`，Windows `ancient-tts-worker.exe`；
- 在 Tauri App 的 `Contents/Resources/worker-runtime/`（macOS）或对应 Resources 目录中可找到；
- 具有目标平台可执行权限；
- 能在没有 Python 和 uv 的干净环境中通过 `system.ping`；
- stdout 只能输出 JSON Lines，诊断信息只能输出 stderr；
- 不携带或写入 SecretId、SecretKey；凭据仍由 Rust 注入 Worker 环境变量。

当前仓库的 PyInstaller 构建入口是 `pnpm worker:build`，执行时会强制检查 Python 3.12.x、调用项目虚拟环境中的 PyInstaller、复制内置 dictionaries，并在生成后执行 bundled Worker 的 `system.ping` 自检：

```bash
uv run --project worker python --version
pnpm worker:build
```

构建输出为 `src-tauri/worker-runtime/ancient-tts-worker`（Windows 为 `.exe`）。生成的二进制被 `.gitignore` 排除，不提交到源码仓库；Tauri 打包前会通过 `beforeBuildCommand` 自动重新生成并将其复制到 App Resources。各目标平台必须在对应平台/架构的 Python 3.12 环境中分别构建，不能把 macOS ARM 产物当作 Windows 或 macOS Intel 产物。

不要使用系统 `pyinstaller`，也不要在 Python 3.9/3.10 环境中生成 Worker。构建后至少执行：

```bash
./src-tauri/worker-runtime/ancient-tts-worker <<'EOF'
{"id":"runtime-check","method":"system.ping","params":{}}
EOF
```

并检查响应中的 `ok=true`、`result.version` 和目标 App Resources 中的实际文件。

## 发布前检查清单

- [ ] `worker/pyproject.toml` 的 Python 范围未被放宽；
- [ ] 构建机 `uv run --project worker python --version` 为 3.12.x；
- [ ] PyInstaller 使用上述 Python 3.12.x 执行；
- [ ] PyInstaller Worker 已复制到 Tauri Resources；
- [ ] App 启动时优先命中 bundled Worker；
- [ ] 不依赖 GUI PATH 中的 `uv`；
- [ ] 在没有 Python/uv 的环境中完成 `system.ping`；
- [ ] 完成一次 `pronunciation.analyze`；
- [ ] 使用已确认发音完成一次 `tts.synthesize`；
- [ ] `worker.log` 没有 `No module named 'pypinyin'` 或 `No module named 'tencentcloud'`；
- [ ] Windows x64、macOS Apple Silicon、macOS Intel 分别使用对应的 Worker 构建产物。

## 当前仓库状态

当前仓库已经支持本机调试时优先使用 `worker/.venv`，并提供 `pnpm worker:build` 生成 PyInstaller Worker。Apple Silicon macOS 已验证生成的 Worker 可以在没有 Python、uv 和 PATH 依赖的环境中完成 `system.ping` 与发音分析。二进制仍不提交源码仓库；Windows x64、macOS Intel 的构建、签名和干净机器运行验证仍需在对应环境完成。
