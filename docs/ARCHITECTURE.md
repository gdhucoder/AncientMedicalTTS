# AncientMedicalTTS 实际架构

AncientMedicalTTS 是本地运行的 Tauri 2 桌面应用。React/TypeScript 只负责页面和交互；Rust 是唯一的数据控制层，负责文件系统、SQLite、migration、Worker 生命周期、协议调用、分析结果校验、Annotation 写入、凭据读取和音频文件生命周期。Python Worker 只接收 Rust 提供的文本与 token，负责 pypinyin、内置词典分析和腾讯云 TTS SDK 调用。

```text
React + TypeScript
        │ Tauri IPC
        ▼
Rust Core
  ├── BookService: TXT 读取、规范化、导入
  ├── SegmentService: Grapheme Token 分词
  ├── PronunciationService: Worker 调用、结果校验、复核状态
  ├── PronunciationRuleService: 本书/全局规则、token 精确匹配和生命周期
  ├── SegmentEditService: reading_text、speak_enabled、token Split/Merge 和失效处理
  ├── SettingsService: TTS 参数和内置腾讯音色列表
  ├── AudioService: 临时 WAV、版本持久化、播放选择
  ├── BatchGenerationService: Book 预检、设置快照、串行生成、复用、取消和失败状态
  ├── ExportService: 导出预检、顺序整理、临时 concat list、输出验证和取消
  ├── FFmpeg abstraction: bundled sidecar、WAV concat、MP3 libmp3lame 编码
  ├── CredentialStore: Tauri 应用数据目录下的受控 JSON 文件
  ├── WorkerProcess: stdin/stdout JSONL、超时、退出检测
  └── sqlx SQLite + migrations
        │ JSONL stdin/stdout
        ▼
Python Worker
  ├── system.ping
  ├── pronunciation.analyze
  ├── tts.test_connection
  └── tts.synthesize
       ├── pypinyin==0.55.0
       ├── dictionaries/medical_lexicon_v3.json
       ├── dictionaries/classical_lexicon_v3.json
       ├── dictionaries/context_rules_v3.json
       ├── dictionaries/context_pronunciation_rules.json
       ├── dictionaries/high_risk_polyphones.json
       ├── dictionaries/known_rare_characters.json
       └── dictionaries/variant_characters.json
       └── Tencent Cloud TTS SDK 3.0.1307
```

## Worker 运行时约束

源码 Worker 的 Python 运行时固定为 Python 3.12.x，约束来自 `worker/pyproject.toml` 的 `requires-python = ">=3.12,<3.13"`。开发环境必须使用由 uv 创建的 `worker/.venv`；正式发布必须使用由 Python 3.12 构建并放入 Tauri Resources 的 PyInstaller Worker，不得依赖目标机器的 Python、uv 或 PATH。

需要特别区分：PyInstaller Worker 是原生可执行文件，启动时不会再选择 Python 解释器。若 App 没有找到 bundled Worker，且 Finder 启动时又找不到 `uv`，Rust 的历史兼容回退可能执行系统 `python3`。本机曾因此解析到 Xcode Python 3.9，导致 `pypinyin` 和 `tencentcloud` 缺失；这属于 Worker 启动回退问题，不是 PyInstaller 版本选择问题。启动优先级、构建要求和发布验收见 [WORKER_RUNTIME.md](WORKER_RUNTIME.md)。

## 主要流程

### TXT 导入

```text
Tauri Dialog
 ↓
Rust import_txt_book
 ↓
UTF-8 校验 + 最小化规范化
 ↓
Rust SegmentService / Grapheme Token 边界
 ↓
books + chapters + segments 事务
 ↓
React Books / Reader
```

当前 TXT 会按 Rust 保守识别的“第…篇/章/卷”等篇章标题创建多个 Chapter；没有明确标题时保持一个“正文” Chapter。原始文件路径不会作为后续依赖，只保存文件名。

### 发音分析

```text
Reader 点击“分析发音”
 ↓ Tauri IPC
Rust 读取 Segment 并生成 GraphemeToken[]
 ↓ JSONL pronunciation.analyze
Python 使用提供的 token 做异体分析视图、结构化词典最长匹配、pypinyin 候选和上下文选音
 ↓ JSONL response
Rust 校验 response、范围、surface_text、风险类型和拼音
 ↓ SQLite transaction
保留 manual confirmed/ignored，删除 needs_review，插入新的 needs_review
 ↓
Rust 读取 Book Rule + Global Rule，按 Grapheme Token 精确匹配
 ↓
删除规则覆盖的自动 needs_review，创建/更新 confirmed Rule Annotation
 ↓
返回 SegmentReader{segment,tokens,annotations,audio_versions}
```

Rust 生成的 token 是唯一索引基准。Python 不计算字符串偏移，也不假设 Python 字符索引等于 token 索引。Annotation 使用 `[start_token, end_token)` 范围；医学词可以覆盖多个 token。

## Annotation 与 Segment 状态

自动风险包括 `polyphone`、`rare_character`、`unknown_character`、`medical_term`、`classical_term`、`context_pronunciation`、`knowledge_conflict` 和 `textual_variant`。自动分析产生的 Annotation 总是 `needs_review`；没有 `auto` 状态。手工标注在 Rust 校验后直接成为 `confirmed`。每条自动 Annotation 还保存可解释的 `source`、`reason`、`rule_type` 和 `confidence`。

Analyzer v0.3 的知识层顺序是：analysis-only variant view → exact context → medical lexicon → semantic context → classical lexicon → high-risk/rare filter。完整医学词命中后 semantic rule 不再覆盖；若同一范围的医学词典与 semantic rule 给出不同读音，则产生 `knowledge_conflict` 并保留双方候选。常用字只要 pypinyin 可解析就不会因为内部缺省值成为 `rare_character`；真正无法解析的字符使用 `unknown_character`。详细数据与维护约束见 [PRONUNCIATION_KNOWLEDGE_LAYER_V3.md](PRONUNCIATION_KNOWLEDGE_LAYER_V3.md)。

Segment 状态由 Rust 事务内集中重算：没有 Annotation 时为 `analyzed`；存在任意 `needs_review` 时为 `needs_review`；其余 Annotation 全为 `confirmed` 或 `ignored` 时为 `ready`。未分析的 Segment 保持 `pending`。

重分析的保护规则是：删除当前 Segment 旧的 `needs_review`，保留 `source_rule_id IS NULL` 的 `confirmed` 和 `ignored`；新结果与受保护范围重叠时跳过；新结果之间重叠或协议数据非法时整次事务失败。规则应用不调用 Python。

Book 页面提供 `reanalyze_book_pronunciation`，由 Rust 顺序读取全书 Segment、逐次调用同一个 Worker、复用上述事务保护。它只更新发音分析结果，不调用 TTS，不建立 Job 表。

### 发音规则

```text
Annotation confirmed
 ↓ 用户选择本书 / 全局
PronunciationRuleService
 ↓
Book Rule + Global Rule → 最长匹配 → Book 优先
 ↓
segment_annotations(review_status=confirmed, source_rule_id=rule.id)
```

规则只支持 `book` 和 `global` scope，使用完整 pattern 的 Grapheme Token exact match。相同位置的人工 confirmed/ignored Annotation 优先；规则之间先选最长 pattern，再选 Book Rule。规则更新、启用和禁用都会重新应用作用范围，禁用 Book Rule 后可由 Global Rule 接管。用户在某个 Segment 再次确认规则标注时会清空 `source_rule_id`，形成只属于该 Segment 的 manual override。

规则或人工确认导致 Segment 的有效 confirmed pronunciation 改变时，Rust 把 `generated` 改为 `ready`，保留 `current_audio_id`，不自动调用腾讯云 TTS。用户可以继续播放旧版本，手动重新生成后恢复 `generated`；如果读音恢复到当前 AudioVersion 保存的 pronunciation signature，则无需重复生成即可恢复 `generated`。没有该快照的历史 AudioVersion 采用保守策略，仍要求重新生成。

## Manual Segment Editing

`segments.original_text` 是导入后的不可变原文。人工编辑只写入可选的 `reading_text`；所有朗读相关流程统一使用 `effective_text = reading_text ?? original_text`。`speak_enabled=0` 的活动 Segment 会保留在阅读列表中，但不会进入批量 TTS 或完整音频导出。

Split/Merge 不修改既有 Segment 的 `original_text`，而是在同一数据库 transaction 中创建新的活动 Segment，并将旧行标记为 `superseded`。旧行的 `segment_annotations` 会失效，旧 `audio_versions` 保留但不再是活动 Segment 的当前音频。活动查询、发音规则、批量 TTS 和导出均排除 `superseded`。

`reading_text` 修改、Split 和 Merge 会清理受影响活动 Segment 的 Annotation、`current_audio_id`，并将状态置为 `pending`；Reader 完成 mutation 后重新分析新 Segment。这里不做复杂的 Annotation token remapping。`speak_enabled` 只控制是否参与朗读，不改变 pronunciation 数据模型。

## TTS 与音频流程

```text
Reader 选择已分析且无待复核的 Segment
 ↓ Tauri IPC
Rust 读取 TtsSettings、Segment 和 confirmed Annotation
 ↓ 临时路径 + tts.synthesize JSONL
Python 校验 token/override，必要时生成 <speak><phoneme> SSML
 ↓ Tencent TextToVoice（官方 Python SDK）
Base64 WAV 写入 Rust 指定临时路径
 ↓ Rust 校验 RIFF/WAVE，改名为 vN.wav
SQLite transaction: audio_versions + current_audio_id/status=generated
 ↓ Tauri asset:// 播放
React <audio controls>
```

当前 TTS 固定为腾讯云、16000Hz、WAV、单 Segment。未命中 confirmed 发音覆盖时，Worker 将原文直接作为 `Text`，不会包裹 `<speak>`；存在覆盖时，才生成 XML 转义后的 SSML。TTS 请求超时为 60 秒，连接测试超时为 30 秒。

### TTS 设置与试听

设置页的合成参数使用前端本地状态编辑。音色、语速和音量改变后，试听卡立即显示当前选择；只有“保存设置”才写入 `app_settings`。试听通过明确的 `generate_tts_preview` IPC 进入 Rust，再复用同一个 `tts.synthesize` Worker 协议和腾讯云 Provider，不创建 Segment 或 AudioVersion。结果写入应用数据目录 `cache/tts-preview/`，新试听和应用启动时会清理旧试听文件。设置页的 API 用量来自 `api_usage_events`，记录单段生成、全文生成、试听和连接测试，不估算费用。

## 完整音频导出

```text
Book Segments
  ↓ Chapter.order_index + Segment.order_index
current_audio_id → AudioVersion → WAV header / 参数预检
  ↓
ExportService 临时 concat.txt
  ↓ Tauri bundled FFmpeg sidecar
merged.wav（concat demuxer，-c copy）
  ├── WAV：保存 merged.wav
  └── MP3：libmp3lame，96 kbps，保持输入 sample rate/声道
  ↓
用户在系统 Save Dialog 选择的路径
```

导出在内存中维护单实例状态，不增加数据库表。运行期间 Rust 锁定批量 TTS、Book 删除、发音修改、TTS 设置和音频版本修改；取消只终止 FFmpeg 子进程并清理 `cache/exports/{uuid}`，不修改 Segment WAV 或 AudioVersion。React 只能通过 `check_ffmpeg`、`get_book_export_preflight`、`export_book_audio`、`get_export_state` 和 `cancel_export` 这些明确 IPC 调用参与导出，不能执行系统命令。

## 全文批量语音

```text
Book 页面
 ↓ get_book_generation_preflight
Rust 检查 5000 汉字、Segment 状态、150 字单句限制、凭据和设置
 ↓ 用户确认
读取一次 TtsSettings 快照
 ↓ 串行按 Chapter/Segment 顺序
最新 AudioVersion 参数完全匹配且文件存在 → skip
否则 → 复用 AudioService 生成新 WAV 版本
 ↓
batch-generation-progress 事件 + get_batch_generation_state
```

批量生成不建立 jobs 表，不使用并发，不调用 FFmpeg，不改变 Tencent Provider。它通过 `AudioService::generate_segment_audio_with_settings` 复用单 Segment 的 SSML、Worker、WAV 校验、版本编号和数据库事务。设置、发音分析、规则、导入、删除和单 Segment TTS 在批量运行期间由 Rust Command 层锁定。取消只设置协作式标志，当前请求完成后停止；已生成版本继续保留。详见 [docs/BATCH_GENERATION.md](BATCH_GENERATION.md)。

凭据只由 Rust 读写 Tauri 应用数据目录下的 `tencent_credentials.json`，并在启动 Worker 时以环境变量注入 Worker 进程；Secret 不进入 SQLite、IPC 返回值或日志。Unix 系统上凭据文件权限限制为 `0600`。删除 Book 时，Rust 同时清理该 Book 的应用数据目录音频文件。

## 安全边界和未实现内容

Python Worker 不接收数据库路径，不访问 SQLite。当前没有 HTTP server、FastAPI、Flask、Electron、Docker、PostgreSQL、Redis、模糊/正则规则、LLM、Job Manager、自动通假判断或 OCR 导入。FFmpeg 只作为 Rust 控制的 bundled external binary 使用，React 不执行 shell。腾讯云只通过 Worker 官方 SDK 调用，Rust 保持数据库和文件系统控制权。规则可以批量应用到已有书籍；全文批量语音生成独立 WAV 后，ExportService 可以进行完整 WAV/MP3 导出，但仍不做 crossfade、额外静音、ASR、字幕或复杂 metadata。

Zustand 只保存页面导航 ID 和 UI 选择状态，业务数据通过 Rust IPC 获取。Reader 显示的候选和输入最终仍由 Rust 进行拼音格式及 token 数量校验。
