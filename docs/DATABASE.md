# 数据库与迁移

Rust 通过 `sqlx` 独占 SQLite 连接。Python Worker 不接收数据库路径，也没有 SQLite 依赖。数据库位于 Tauri 应用数据目录的 `app.sqlite`。连接初始化开启 WAL 和外键约束，并通过 `sqlx::migrate!` 执行 SQL migration。

## Migrations

- `0001_init.sql`：创建 `app_meta`，保存 `schema_version` 和 `app_version`；
- `0002_books_chapters_segments.sql`：创建 Book、Chapter、Segment 文本域；
- `0003_pronunciation_annotations.sql`：创建 `segment_annotations` 和按 Segment 的索引。
- `0004_tts_audio.sql`：创建 `app_settings`、`audio_versions` 和按 Segment 的音频版本索引。
- `0005_pronunciation_rules.sql`：创建 `pronunciation_rules`，并为 `segment_annotations` 增加 `source_rule_id`。
- `0006_pronunciation_analyzer_v2.sql`：为 `segment_annotations` 增加分析来源 `source` 及索引。
- `0007_reader_display_settings.sql`：保存 Reader 字号和注音显示模式。
- `0008_manual_segment_editing.sql`：为 Segment 增加可选 `reading_text` 和 `speak_enabled`。
- `0009_pronunciation_knowledge_v3.sql`：为 `segment_annotations` 增加 `rule_type` 和 `confidence` 解释元数据。
- `0010_audio_pronunciation_signature.sql`：为 `audio_versions` 增加生成时 confirmed pronunciation 快照。
- `0011_api_usage_events.sql`：记录腾讯云 TTS 的请求操作、成功/失败和字符用量，用于本地统计。

已经执行的 migration 不应原地修改；新增结构使用新的 SQL 文件。

## 迁移前备份

应用打开已有数据库时，Rust 会在执行 `sqlx::migrate!` 前通过 SQLite `VACUUM INTO` 创建一致性快照：

```text
<应用数据目录>/app.sqlite.backup
```

这样可以同时包含 WAL 中尚未合并到主文件的数据。备份文件仅用于本机恢复，不上传、不写入日志，也不替代用户定期复制整个应用数据目录。首次创建数据库时不会生成空备份；备份失败会以 `DB_BACKUP_ERROR` 阻止迁移，避免在无法保留回滚点时继续变更数据库。

## Text-domain tables

```text
books
  │ 1:N
  ▼
chapters
  │ 1:N
  ▼
segments
  ├── segment_annotations
  └── audio_versions
```

`pronunciation_rules.book_id` 为空表示全局规则；非空表示本书规则。规则表通过 `scope` 区分 `global` 和 `book`，实际写入由 Rust 服务校验。

`books.source_file` 只保存原始文件名。TXT 内容保存于 `segments.original_text`，该字段在导入后不可被人工编辑覆盖。`segments.reading_text` 为空时，effective text 为 `original_text`；非空时，朗读、发音分析和导出使用 `reading_text`。`speak_enabled=0` 表示保留在阅读列表但不参与 TTS/导出。Segment 初始状态为 `pending`，分析后由 Rust 根据 Annotation 状态重算为 `analyzed`、`needs_review` 或 `ready`。

Split/Merge 使用 `status='superseded'` 标记被替代的旧 Segment，而不是物理删除或覆盖旧行。这样既保留原始文本和历史 AudioVersion，又能让活动查询只看到最新 Segment。`reading_text` 修改、Split 和 Merge 都在 SQLite transaction 中完成，并清除受影响 Segment 的 Annotation 和 `current_audio_id`；旧 AudioVersion 不删除。

## `segments` 编辑字段

| 字段 | 用途 |
| --- | --- |
| `original_text` | 导入/分段产生的不可变原文；人工编辑不直接覆盖 |
| `reading_text` | 可选的人工朗读文本，允许标点和空格；为空时回退原文 |
| `speak_enabled` | `1` 参与 TTS 和完整导出，`0` 仅保留阅读 |
| `status='superseded'` | Split/Merge 后的历史 Segment，不再参与活动查询 |

新导入 Book 的 CJK 汉字总数由 Rust 在写入事务前检查，最多 5000 个，计数包含补充平面 CJK。这个限制不新增表字段；`get_book_text_stats` 会从现有 Segment 文本重新统计，因此旧数据仍可被识别为超限。批量生成不会为此修改或删除旧 Book。

## `segment_annotations`

| 字段 | 用途 |
| --- | --- |
| `id` | UUID v7 业务主键 |
| `segment_id` | 所属 Segment，外键级联删除 |
| `start_token` / `end_token` | `[start, end)` Grapheme Token 范围 |
| `surface_text` | 范围内 token 拼接后的原文 |
| `default_pinyin` | 分析器默认 ASCII 数字拼音，可为空 |
| `target_pinyin` | 人工确认或手工标注的目标拼音，可为空 |
| `candidates_json` | 候选拼音数组 JSON |
| `risk_type` | `polyphone`、`rare_character`、`unknown_character`、`medical_term`、`classical_term`、`context_pronunciation`、`knowledge_conflict`、`textual_variant` 或 `manual` |
| `reason` | 面向复核者的原因说明 |
| `review_status` | `needs_review`、`confirmed`、`ignored` |
| `analyzer_version` | 自动分析器版本，手工标注可为空 |
| `source` | 自动分析来源：`medical_lexicon_v3`、`rare_classical_lexicon`、`context_exact`、`context_semantic`、`high_risk_polyphone`、`variant_mapping`、`knowledge_conflict` 或 `pypinyin`；手工标注为 `manual` |
| `rule_type` | 可解释规则类别，例如 `pulse_context`、`formula`、`classical_term` 或 `manual` |
| `confidence` | 离散置信级别：`verified`、`high`、`medium`、`low`；不表示概率 |
| `source_rule_id` | 规则生成的 confirmed Annotation 所绑定的规则；人工确认/覆盖时为空 |
| `created_at` / `updated_at` | RFC 3339 时间 |

`idx_annotations_segment(segment_id, start_token)` 用于 Reader 查询。自动分析结果不会直接写入 confirmed。重分析在一个事务内删除旧的 `needs_review`，保留 `confirmed`/`ignored`，并跳过与受保护范围重叠的新结果。

## `pronunciation_rules`

| 字段 | 用途 |
| --- | --- |
| `id` | UUID v7 业务主键 |
| `scope` | `book` 或 `global` |
| `book_id` | 本书规则的 Book 外键；全局规则为 NULL |
| `pattern_text` | 要匹配的完整词语，使用 Grapheme Token exact match |
| `target_pinyin` | ASCII 数字声调拼音 |
| `rule_type` / `source` | 规则类型和来源说明 |
| `verified` / `enabled` | 当前验证和启用状态 |
| `created_at` / `updated_at` | RFC 3339 时间 |

相同 scope 和 pattern 不允许存在不同目标拼音。规则应用次数通过 `segment_annotations.source_rule_id` 的 SQL count 计算，不额外维护计数器。

## 事务和校验

TXT 导入、分析写入、复核状态变化和手工标注都由 Rust 控制。Rust 在写入前校验 Annotation 范围与 `surface_text` 一致、风险类型在允许集合内、拼音格式正确且拼音音节数量与范围内汉字 token 数量一致。任何校验或写入失败都会回滚当前事务。

## `app_settings`

`app_settings` 只保存非敏感的 TTS 参数：`tts.provider`、`tts.tencent.voice_type`、`tts.tencent.speed`、`tts.tencent.volume` 和 `tts.tencent.sample_rate`。当前固定为 `provider=tencent`、`sample_rate=16000`、`codec=wav`；SecretId/SecretKey 永不写入此表。

## `audio_versions`

| 字段 | 用途 |
| --- | --- |
| `id` | UUID v7 语音版本主键 |
| `segment_id` | 所属 Segment，外键级联删除 |
| `version_no` | Segment 内从 1 开始的递增版本号 |
| `provider` / `voice_type` | 生成服务和音色 |
| `sample_rate` / `codec` | 当前为 16000 / wav |
| `speed` / `volume` | 生成时使用的 TTS 参数 |
| `ssml` | 有 confirmed 发音覆盖时保存的实际 SSML；无覆盖为空 |
| `pronunciation_signature` | 生成时按 Grapheme Token 范围和 target pinyin 排序序列化的发音快照；旧版本可能为空 |
| `audio_path` | 应用数据目录中的 WAV 绝对路径 |
| `provider_request_id` / `provider_session_id` | 腾讯云返回的请求和会话标识 |
| `duration_ms` | 供应商返回的时长；当前可为空 |
| `created_at` | 生成时间 |

`segments.current_audio_id` 指向当前选择的版本。生成语音时 Rust 先写临时 WAV 并校验 RIFF/WAVE，再在数据库事务中插入版本、更新 current_audio_id 和 Segment 状态。切换旧版本只更新 current_audio_id，不删除版本文件。

当确认发音发生变化时，Rust 会把当前 Segment 标记为 `ready`；如果新的有效发音恰好恢复到当前 AudioVersion 的 `pronunciation_signature`，则恢复为 `generated`。没有快照的历史 AudioVersion 采用保守策略，仍需要重新生成。

批量复用查询 `audio_versions` 中每个 Segment 的最大 `version_no`，而不是 `current_audio_id`。只有最新版本的 provider、voice_type、sample_rate、codec、speed、volume 与批量开始时的设置快照相同且 `audio_path` 文件存在时才跳过。批量过程不新增持久化状态表。

完整音频导出只读取 `segments.current_audio_id` 对应的 `audio_versions`，并验证 WAV header 和所有音频参数；它不新增 `exports` 表，也不改变 pronunciation、Segment 或 AudioVersion 数据。导出的 MP3/WAV 是用户选择路径上的派生文件，临时 concat 文件位于应用数据目录的 cache 下并在结束后清理。

## `api_usage_events`

该表只保存本应用发起的 TTS 使用记录，不保存 Secret、不计算费用。`operation` 当前包括 `segment_generation`、`batch_generation`、`voice_preview` 和 `connection_test`；`input_units` 对应请求字符数，失败请求也保留失败记录，但统计页面只累计成功请求的字符数。`model` 保存腾讯音色编号，页面按今天、近 7 天、近 30 天或全部时间统计请求/成功/失败/成功字符，并展示成功字符最多的三个音色。
