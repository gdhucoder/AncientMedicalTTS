# Batch Generation — Milestone 6

## 范围

Milestone 6 只负责一个 Book 的全文独立 WAV 生成。每个 Segment 仍然通过现有 `AudioService` 和 Python Worker 的 `tts.synthesize` 生成；不做 FFmpeg、MP3、拼接、章节导出、并发或 jobs 表。

新导入的单个 Book 最多包含 5000 个 Unicode CJK 汉字。计数包括 CJK Unified Ideographs、CJK Extension A～I 和兼容表意文字；补充平面字符按一个汉字计数。超过限制的导入返回 `PROJECT_TEXT_LIMIT_EXCEEDED`，事务不会留下 Book。历史数据中的超限 Book 不会被删除，可以继续读取和单 Segment 生成，但全文预检会阻止批量生成。

腾讯云当前 provider 的单 Segment 限制是 150 个 Unicode 字符。批量预检会以 `SEGMENT_TEXT_TOO_LONG` 标出超限 Segment，不会在批量层重新切分。

## 预检

`get_book_generation_preflight(book_id)` 由 Rust 读取 Book 统计、Segment 状态、TTS 设置和凭据状态，返回汉字数、Segment 总数、`pending` 和 `needs_review` 数量、可复用版本数量、需要生成数量和 blocker 列表。

预检阻止以下情况：文本超过 5000 汉字、Segment 尚未分析、仍有待复核发音、Segment 超过 150 字符、凭据未配置、TTS 设置无效。

## 快照与复用

启动批量生成时读取一次 `TtsSettings` 快照，整个批量过程使用同一份 provider、voice_type、sample_rate、codec、speed、volume。运行期间设置页面被锁定。

只有以下条件全部满足时才跳过 Segment：

1. Segment 状态为 `generated`；
2. `audio_versions` 中 `version_no` 最大的版本存在；
3. provider、voice_type、sample_rate、codec、speed、volume 与快照完全相同；
4. 该版本的 WAV 文件仍然存在。

比较使用最新版本，不使用 `segments.current_audio_id`，因为用户可能正在播放旧版本。发音规则改变导致有效发音变化时，旧音频保留但 Segment 变为 `ready`，因此会重新生成新版本。

## 顺序、取消和错误

同一应用同时只允许一个批量生成。生成顺序是 Chapter order、Segment order，串行度为 1。取消是协作式的：当前 Worker 请求完成后停止，不杀 Worker，不删除已生成结果。重新开始时沿用上述复用规则，等价于从未完成处继续。

可重试错误最多执行“首次尝试 + 两次重试”，退避 500ms、1500ms：`TTS_TIMEOUT`、`TTS_RATE_LIMITED`、`TTS_PROVIDER_ERROR`。凭据、权限、服务未开通、配额、无效音色、无效 SSML、文本过长和数据库/文件系统/Worker 致命错误不按普通 Segment 错误重试。普通 Segment 错误记录到内存失败列表并继续；致命错误停止批量。完成状态仍可能带有失败列表。

## IPC 与事件

命令：

- `get_book_generation_preflight`
- `start_book_audio_generation`
- `cancel_book_audio_generation`
- `get_batch_generation_state`

`batch-generation-progress` 事件传输当前进度，但事件不是事实来源。页面打开或重新进入时调用 `get_batch_generation_state` 恢复状态。状态包含 `generated`、`skipped`、`failed`、当前 Segment、失败列表和致命错误。

应用退出时不保留批量运行状态，不创建持久化 Job；已经落盘的 AudioVersion 和 WAV 文件保留。

