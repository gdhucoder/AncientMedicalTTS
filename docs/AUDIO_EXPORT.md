# 完整音频导出

应用支持把一个 Book 当前的 Segment 音频按正文顺序导出为完整 WAV 或 MP3，也支持只导出段落导航中勾选的若干段。选段导出适合篇幅较长、希望分章节或分批保存音频的场景。

在左侧“章节与段落”列表勾选需要导出的段落，可以跨章节选择；“全选当前列表”只加入当前筛选结果中参与朗读的段落，“清空”取消选择。顶部按钮会在“导出整本”和“导出选中（N段）”之间切换。没有勾选时仍按整本导出。无论整本还是选段，Rust 都会重新按 `chapters.order_index ASC`、`segments.order_index ASC` 排序，列表勾选顺序不会影响音频顺序。

## 导出前预检

Rust `ExportService` 只读取选定范围内 `segments.current_audio_id`，按 `chapters.order_index ASC`、`segments.order_index ASC` 排序，不会选择最新版本或按文件名排序。预检会检查 Book、选定文本的 5000 汉字上限、Segment 状态、AudioVersion 所属关系、文件存在性、RIFF/WAVE header，以及 provider、voice、sample rate、codec、speed、volume 和实际 WAV 声道/位深等参数是否一致。

只有全部 Segment 为 `generated` 且所有有效音频参数一致时才允许导出。`ready` 会返回 `EXPORT_AUDIO_STALE`；任何待分析、待复核或缺失音频都会阻止导出。导出不会改变 pronunciation、Segment 或 AudioVersion 数据。

## FFmpeg 调用

React 通过 Tauri IPC 调用 Rust。Rust 使用应用内置的 Tauri external binary，不通过 shell，也不依赖用户的 PATH。开发 debug 模式允许使用 `ANCIENT_MEDICAL_TTS_FFMPEG` 或本机 PATH 进行调试；生产构建必须准备对应 target triple 的 bundled binary。

合并阶段使用 FFmpeg concat demuxer 和 `-c copy` 生成 `merged.wav`。因为预检已经验证输入 WAV 参数完全一致，当前不插入额外静音、不做 crossfade、不做 loudnorm，也不重新混音。MP3 阶段使用 `libmp3lame`、96 kbps，并沿用输入 sample rate 和声道数。

## 临时文件和取消

临时文件放在应用数据目录的 `cache/exports/{uuid}/`，包含 `concat.txt`、`merged.wav` 和可能的 `output.mp3`。成功或失败都会清理目录；取消时只终止 FFmpeg 子进程并清理临时文件，不影响原始 Segment WAV。

concat list 使用独立文件参数传给 FFmpeg，路径统一为 `/`，并对单引号使用 concat demuxer 的反斜杠转义；不会拼接 shell 命令。Windows 盘符、空格、中文和包含单引号的路径有单元测试覆盖。

## 错误

常见错误包括 `FFMPEG_NOT_AVAILABLE`、`MP3_ENCODER_NOT_AVAILABLE`、`EXPORT_AUDIO_STALE`、`EXPORT_AUDIO_SETTINGS_MISMATCH`、`AUDIO_FILE_MISSING`、`AUDIO_FORMAT_MISMATCH`、`EXPORT_ALREADY_RUNNING`、`BATCH_GENERATION_IN_PROGRESS` 和 `EXPORT_CANCELLED`。FFmpeg stderr 只保留最后约 8KB，避免把完整日志发送到前端。

## 构建

运行 `node scripts/prepare-ffmpeg.mjs` 会为当前平台复制并校验 sidecar；macOS 动态依赖会随 app resources 一起处理。生产构建默认要求 `vendor/ffmpeg/ffmpeg-{target-triple}` 已准备；仅本地调试时可设置 `ANCIENT_MEDICAL_TTS_ALLOW_SYSTEM_FFMPEG=1` 使用 PATH 中的 FFmpeg。完整来源、许可证和当前二进制限制见 [THIRD_PARTY.md](THIRD_PARTY.md)。
