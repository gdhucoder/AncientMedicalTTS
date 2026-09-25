# AncientMedicalTTS

AncientMedicalTTS 是一个面向 Windows 10/11 和 macOS 的跨平台桌面应用，用于阅读中医古籍、复核发音并生成语音。当前源码候选版本为 **v0.1.0-rc1**，已进入功能冻结后的发布验证阶段。项目主页：[GitHub Pages](https://gdhucoder.github.io/AncientMedicalTTS/)，源码：[GitHub 仓库](https://github.com/gdhucoder/AncientMedicalTTS)。

当前版本已实现：UTF-8 TXT 导入、SQLite 本地阅读、Rust Grapheme Token 分词、Python 发音分析器 v0.3、结构化中医与古籍发音知识层、可解释语义上下文选音、Annotation 复核、腾讯云 TTS、SSML 发音覆盖、WAV 音频版本、单 Segment 播放、本书/全局精确发音规则、单 Book 串行全文 WAV 生成，以及按正文顺序导出完整 WAV/MP3。

## 当前已实现

- Tauri 2 + React + TypeScript + Vite + pnpm；
- Rust 作为 SQLite、文件系统、Worker 生命周期和数据校验的唯一控制层；
- SQLite WAL、外键约束和基于 SQL 文件的 sqlx migration；
- UTF-8 TXT 选择、最小化文本规范化和 Book → Chapter → Segment 持久化；
- 导入 TXT 时对保守识别的“第…篇/章/卷”等篇章标题自动拆分 Chapter；无明确标题的文本保持单个“正文” Chapter；
- 使用 Rust `unicode-segmentation` 生成稳定的 Grapheme Token；
- Python Worker 通过 stdin/stdout JSON Lines 提供 `system.ping` 和 `pronunciation.analyze`；
- `pypinyin==0.55.0` 的 ASCII 数字声调拼音分析；
- `polyphone`、`rare_character`、`unknown_character`、`medical_term`、`classical_term`、`context_pronunciation`、`knowledge_conflict`、`textual_variant` 自动风险，并记录 `source/reason/rule_type/confidence`；
- Python analyzer v0.3.0：结构化医学词典、独立古籍词典、完整词最长匹配、exact 与 semantic context 选音、高风险多音字表、已知生僻字表和仅用于分析的异体映射；pypinyin 继续作为候选提供者，普通多音字不再全部告警；
- Book 级“重新分析全书发音”命令，串行调用 Worker，保留既有人工确认/忽略和规则覆盖；
- `needs_review`、`confirmed`、`ignored` 三种复核状态，以及 Segment 状态重算；
- Reader 中的 token 标注展示、候选选择、目标拼音输入、确认/忽略/重置和普通汉字手工标注；
- TTS 设置页面：应用数据目录凭据状态、腾讯云音色、Speed、Volume、Test Connection；
- TTS 设置页面提供四张卡片：凭据、合成参数、本地 API 用量统计和音色试听；试听直接使用当前未保存的音色/语速/音量，保存设置后才影响正式生成；
- Rust 组装已确认发音覆盖，Worker 生成腾讯云 TTS 请求和可选 SSML；
- 16000Hz WAV 单 Segment 生成、版本保存、版本切换和 Tauri asset 协议播放；
- 发音词典：本书规则和全局规则、Grapheme Token 精确匹配、最长匹配、Book 优先级、人工覆盖保护、规则更新/启用/禁用；
- 规则生成的 confirmed Annotation、`source_rule_id` 来源追踪，以及发音改变后的音频 stale 提示；
- 单个新导入 Book 最多 5000 个 Unicode CJK 汉字（含 supplementary-plane CJK）；旧 Book 可继续读取和单句生成，但超限时批量预检会阻止全文生成；
- Book 全文语音预检、当前 TTS 设置快照、串行生成、最新 AudioVersion 参数复用、进度事件、协作式停止、失败列表和可重启续跑；
- 完整音频导出：Rust ExportService 按 Chapter/Segment 顺序读取 `current_audio_id`，通过内置 FFmpeg concat demuxer 生成 WAV，并使用 `libmp3lame` 以 96 kbps 生成 MP3；
- 导出预检、WAV header/参数一致性校验、系统保存对话框、取消、临时目录清理和跨平台路径 escaping；
- Manual Segment Editing：保留不可变 `original_text`，支持 `reading_text`、参与朗读开关、按 Grapheme Token 分段、前后 Segment 合并和恢复原文；编辑后自动重新分析，旧 AudioVersion 保留但不再作为当前音频；
- Worker 异常、超时、非法 JSON、协议 ID 不匹配和 Rust 拼音/范围校验的基础错误处理。

## 当前未实现

通假字自动推断、LLM、Job Manager、云端同步、PDF/DOCX/OCR 导入尚未实现。异体映射当前只用于分析匹配和复核提示，不修改原始文本。当前导出不做额外静音、crossfade、响度处理、字幕、M4B 或复杂 metadata。

## 发布文档

- [快速开始](docs/QUICK_START.md)
- [发布检查清单](docs/RELEASE_CHECKLIST.md)
- [当前已知限制](docs/KNOWN_LIMITATIONS.md)
- [Python Worker 运行时约束](docs/WORKER_RUNTIME.md)
- [完整架构](docs/ARCHITECTURE.md)
- [更新记录](CHANGELOG.md)

## GitHub Actions 自动构建

推送到 `main`、创建 Pull Request 或手动运行“构建桌面安装包”Workflow，会分别构建 macOS Apple Silicon、macOS Intel 和 Windows x64。推送版本 tag（例如 `v0.1.1` 或 `v0.1.1-rc1`）后，Workflow 会在三种 runner 全部成功后自动创建/更新 GitHub Release 并上传安装包。

```bash
git tag v0.1.1
git push origin v0.1.1
```

构建使用 runner 上的 Python 3.12、uv、PyInstaller 和 FFmpeg，最终应用内仍携带 Worker/FFmpeg，不要求用户安装这些依赖。FFmpeg 的来源、许可证和当前自动构建边界见 [docs/THIRD_PARTY.md](docs/THIRD_PARTY.md)。

## 环境要求

- Node.js 20+；
- pnpm 10+；
- Rust stable，以及 Tauri 2 对应的 Windows/macOS 构建依赖；
- Python 3.12.x（Worker 源码运行时的硬约束，不能使用系统 Python 3.9/3.10/3.11/3.13）；
- uv。

Worker 的 Python 依赖固定在 `worker/pyproject.toml`，包括 `pypinyin==0.55.0` 和腾讯云官方 TTS SDK `3.0.1307`。SQLite 不由 Python 访问。

## 安装与开发启动

```bash
pnpm install
pnpm tauri:dev
```

打开“我的古籍”，导入 UTF-8 `.txt` 后选择 Segment。点击“分析发音”即可调用 Worker；正文中的黄色标注表示待复核，绿色表示已确认，灰色表示已忽略。确认读音后，可在 Annotation 详情选择“应用到本书”或“加入全局词典”；也可以从“发音词典”页面新增、编辑、启用、禁用规则。规则只做完整文本的精确匹配，使用 Grapheme Token 定位。

在 Current Segment 的“朗读文本与分段”区域编辑 `reading_text`。原始导入文本始终保留在 `original_text`，朗读、发音分析和导出使用 `reading_text ?? original_text`。点击正文 token 可选择分段光标；也可以与上一/下一 Segment 合并，或关闭当前 Segment 的朗读。编辑、Split、Merge 会清除受影响 Segment 的当前分析和音频引用，并自动重新分析；旧语音版本不会删除，需重新生成才能成为当前有效音频。

在“TTS 设置”中保存腾讯云 SecretId/SecretKey 后，凭据会写入 Tauri 应用数据目录下的 `tencent_credentials.json`，不写入 SQLite、日志或前端持久化。Unix 系统上文件权限限制为当前用户可读写。选择音色并保存设置后，在已分析且没有待复核 Annotation 的 Segment 中点击“生成语音”。生成的 WAV 保存在应用数据目录，并通过 Tauri asset 协议播放。没有腾讯云凭据时，应用仍可启动、导入和完成发音分析，但 TTS 请求会返回凭据缺失错误。

设置页的“音色试听”最多接受 200 字，试听音频仅写入 `cache/tts-preview/`，不会生成 Segment 音频版本；“API 调用用量”只统计本应用记录的请求、成功/失败和成功字符数，不提供费用估算。

打开 Book 后，Book 语音区域可以先执行批量预检。预检通过并确认后，应用会读取一次当前 TTS 设置，按 Segment 顺序串行生成。状态为 `generated` 且最新 `AudioVersion` 的 provider、voice、sample rate、codec、speed、volume 与快照完全一致并且文件仍存在时会跳过；否则生成新版本。停止只在当前 Segment 完成后生效，已有结果会保留；再次点击生成会复用可复用版本并继续未完成部分。批量运行期间会锁定 TTS 设置、发音分析、发音词典、单句生成、导入和删除 Book。

当全文 Segment 都为 `generated` 且音频参数一致时，可在 Book 页面点击“导出完整音频”。预检会阻止 stale、待复核、缺失文件或参数不一致的文档。导出格式默认 MP3（96 kbps），也可选择无损 WAV；用户通过系统保存对话框选择目标路径。正式准备好对应平台的 bundled sidecar 后，用户不需要安装 FFmpeg 或配置 PATH。请按 [docs/THIRD_PARTY.md](docs/THIRD_PARTY.md) 准备 audited sidecar；本地调试可使用 `ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG=1`。

Rust 开发命令会优先使用项目的 Python 3.12 虚拟环境启动 Worker；也可以直接启动 Worker：

```bash
uv sync --project worker
uv run --project worker python worker/main.py
pnpm worker:build
```

从 Finder 启动的 macOS App 不保证继承终端 PATH，因此不能依赖 GUI 环境中的 `uv` 或系统 `python3`。正式发布必须把 Python 3.12 环境构建出的 PyInstaller Worker 放入 App Resources；PyInstaller Worker 是正式运行时，目标机器不需要安装 Python。源码调试时使用 `worker/.venv`，不要使用未确认版本的 `python3`。

直接启动时，Worker 会等待 stdin 中的 JSONL 请求，协议数据只写入 stdout，诊断日志写入 stderr。

如需运行真实腾讯云集成测试，必须显式提供测试环境变量；测试会调用连接检查，并生成多组覆盖不同发音 override 的临时 WAV：

```bash
RUN_TENCENT_TTS_INTEGRATION=1 \\
ANCIENT_TTS_TENCENT_SECRET_ID=... \\
ANCIENT_TTS_TENCENT_SECRET_KEY=... \\
uv run --project worker python -m unittest worker/tests/test_tencent_integration.py -v
```

## 分词和分析边界

导入阶段的分段与 token 生成完全由 Rust 完成。Worker 接收 Rust 已生成的 token 列表，不计算字符偏移，也不把 Python 字符串索引当作 token 索引。Python v0.3 使用 pypinyin 作为候选提供者，并通过 `worker/dictionaries/` 下版本化的医学词典、古籍词典、exact/semantic context、高风险多音字、生僻字和异体资源生成可解释 Annotation。完整医学词优先于 semantic context；两者冲突时返回 `knowledge_conflict`，不静默覆盖。

自动分析不会写入 confirmed。重分析只删除旧的 `needs_review`，保留 `confirmed` 和 `ignored`，并跳过与受保护标注重叠的新结果。分析结果的 `source` 用于区分医学词典、古籍词典、exact/semantic context、高风险多音字、生僻字、异体映射和 pypinyin；`rule_type` 与 `confidence` 只提供解释，不改变现有复核语义。规则生成的 confirmed Annotation 仍由 Rust 写入 `source_rule_id`。

## 测试和检查

```bash
pnpm test
pnpm build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
uv run --project worker python -m unittest discover -s worker/tests -p 'test_*.py'
uv run --project worker python tools/validate_pronunciation_lexicon.py
RUN_FFMPEG_INTEGRATION=1 ANCIENT_MEDICAL_TTS_FFMPEG=/path/to/ffmpeg cargo test --manifest-path src-tauri/Cargo.toml services::export_service::tests::ffmpeg_integration_concat_and_mp3_are_opt_in -- --exact
ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG=1 ANCIENT_MEDICAL_TTS_ADHOC_SIGN=1 CI=true pnpm tauri build --debug --bundles app

# 真实古籍发音基线（只读评测，不写入 Gold 规则）
python3 tools/benchmark/run_pronunciation_benchmark.py
python3 tools/benchmark/run_pronunciation_benchmark.py --output reports/benchmark/huangdi_neijing_v01/m8 --label m8
python3 -m tools.benchmark.test_run_pronunciation_benchmark

# M9 三开发/回归数据集（不会覆盖旧报告）
PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset huangdi_neijing_v01 --label m9 --output reports/benchmark/m9_regression/huangdi_neijing_v01
PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset shanghanlun_holdout_v01 --label m9 --output reports/benchmark/m9_regression/shanghanlun_holdout_v01
PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset jinguiyaolue_holdout_v01 --label m9 --output reports/benchmark/m9_regression/jinguiyaolue_holdout_v01
PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --combine-m9
```

Rust 测试覆盖 SQLite 初始化、migration、TXT 导入上限、历史 Book 统计、Grapheme Token、分析写入、重分析保护、状态变化、手工标注、音频版本、Worker 通信、批量单实例/预检/重试分类、导出预检/排序/路径 escaping，以及规则的精确匹配、最长匹配、作用域优先级、人工覆盖、更新、禁用回退和 stale。设置 `RUN_FFMPEG_INTEGRATION=1` 后，额外测试 3 段 WAV concat 和 `libmp3lame` MP3 编码；普通 `cargo test` 不依赖 FFmpeg。Python 测试覆盖 ping、未知方法、非法 JSON、医学/古籍词典最长匹配、方剂名、semantic context、knowledge conflict、common-char-not-rare、异体 surface 保留、SSML、WAV 写入和 supplementary-plane 汉字；前端测试覆盖 token 标记范围、分析来源/风险/置信标签、手工标注和导出文件名/阶段。

## 当前已知限制

本机 macOS Apple Silicon 已完成 Python 3.12 构建的 PyInstaller Worker、无外部 Python/uv/PATH 的 `system.ping` 与发音分析验证。Worker 二进制和 FFmpeg binary 都不提交源码仓库；跨机器发布前必须在对应环境准备 Windows x64、macOS ARM 和 macOS Intel 的 Worker/FFmpeg 产物，并完成正式签名。Worker 的解释器选择、PyInstaller 构建和发布检查见 [docs/WORKER_RUNTIME.md](docs/WORKER_RUNTIME.md)。批量生成当前按 Book 顺序逐 Segment 扫描且并发度为 1，导出同样只允许一个本地操作。真实 Tencent 批量验收和完整古籍试听需要已有凭据及成品音频，因此常规测试不默认执行。

## 运行时文件

应用把 `app.sqlite`、`logs/app.log`、`logs/worker.log` 和 `projects/{book_id}/audio/{segment_id}/vN.wav` 写入 Tauri 选择的平台应用数据目录。sqlx migration history 由数据库维护，已经执行的 migration 不应原地修改。

详见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、[docs/WORKER_RUNTIME.md](docs/WORKER_RUNTIME.md)、[docs/DATABASE.md](docs/DATABASE.md)、[docs/MANUAL_SEGMENT_EDITING.md](docs/MANUAL_SEGMENT_EDITING.md)、[docs/PRONUNCIATION_KNOWLEDGE_LAYER_V3.md](docs/PRONUNCIATION_KNOWLEDGE_LAYER_V3.md)、[docs/PRONUNCIATION_ANALYZER_V2.md](docs/PRONUNCIATION_ANALYZER_V2.md)、[docs/PRONUNCIATION_RULES.md](docs/PRONUNCIATION_RULES.md)、[docs/BATCH_GENERATION.md](docs/BATCH_GENERATION.md)、[docs/AUDIO_EXPORT.md](docs/AUDIO_EXPORT.md)、[docs/THIRD_PARTY.md](docs/THIRD_PARTY.md) 和 [docs/PROTOCOL.md](docs/PROTOCOL.md)。
