# Annotated Book Import v1：现状与差距分析

## 结论

当前 AncientMedicalTTS 已经具备可复用的核心能力：Rust Grapheme Token、Segment/Chapter/Book 持久化、发音 Annotation、Book/Global Rule 优先级、SSML `<phoneme>`、腾讯 TTS、批量生成，以及 Publication Bundle 导出。新增功能不需要另起一套 TTS 或阅读端架构。

需要补齐的是一条“解析并规范化已注音数据”的导入管线，以及把导入注音纳入现有 pronunciation policy 和发布包。

## 现有能力

| 领域 | 当前能力 | 对本需求的复用方式 |
| --- | --- | --- |
| TXT 导入 | `book_service::import_txt_book`、Rust 分章/分段、5000 汉字限制 | 保持不变；Annotated Import 使用独立入口 |
| 文本定位 | `unicode_segmentation` Grapheme Token | 用于 token/text 严格对齐和 Annotation token 范围 |
| 数据库 | Book、Chapter、Segment、segment_annotations、AudioVersion | 增加导入元数据、Chapter 元数据、Segment translation |
| 发音 | Manual、Book Rule、Global Rule、Analyzer 结果 | 增加 imported_reference / imported_authoritative 来源和优先级 |
| TTS | locked/display 模式、腾讯 Provider、批量生成、SSML range phoneme | imported authoritative 复用 locked/批量链路，不新建 TTS 引擎 |
| 发布包 | 每章 MP3、Segment timeline、relative path validator | 增加 translation、当前有效发音 metadata |
| 前端 | Library 导入区、Reader Inspector、全文生成入口 | 增加导入预检确认、注音模式和直接生成入口 |

## 需要新增或调整

### 数据库 migration

新增 migration，不修改已执行 migration：

- `books.import_format`
- `books.import_format_version`
- `books.import_pronunciation_mode`
- `books.import_dataset_id`
- `books.imported_at`
- `chapters.collection`
- `chapters.subtitle`
- `segments.translation`

不新增独立的“导入注音表”。导入注音落在现有 `segment_annotations`，通过 `source` 区分 `imported_reference` 与 `imported_authoritative`，保留现有 Manual/Rule provenance。

### 统一导入管线

新增 `annotated_import_service`：

1. 读取单 JSON 或安全 ZIP；
2. 兼容正式 `ancient-annotated-book` v1 和现有 legacy annotated dataset；
3. 转换到内部统一 DTO；
4. 用 Rust Grapheme Token 验证 token/text；
5. 验证 numeric/tone-mark pinyin、覆盖率和当前 5000 汉字制作端限制；
6. 先返回 Preflight，不通过验证不写数据库；
7. 事务写入 Book/Chapter/Segment/Annotation。

Legacy 只在 adapter 中判断，后续业务只接收规范化 DTO。

### 发音优先级

最终有效 TTS 发音顺序调整为：

`Manual Confirmed > Book Rule > Global Rule > Imported Authoritative > TTS Default`

`Imported Reference` 只用于页面参考，不进入强制 TTS。已有人工确认和规则不被导入基线覆盖；重新分析不覆盖 imported authoritative。

### SSML

现有 Worker 已支持按 token range 生成 `<phoneme>`。Rust policy 层需要把连续、同来源、未跨标点/空白的 imported/其它有效 forced span 合并为 phrase-level range，再交给现有 SSML builder；不把每个字拆成一个 phoneme。

### Strict / Mixed

沿用现有 Batch Generation、Cancel、Resume、Retry。新增导入注音 preflight：

- Strict：所有需要朗读的汉字都有 authoritative pinyin 才允许；
- Mixed：明确允许缺失位置由 TTS 默认判音，并显示缺失数量；
- Reference：默认不能 force，必须由用户显式信任文件注音。

### Publication Bundle

现有导出只需扩展：

- Segment `translation`；
- 当前 effective pronunciation token metadata，而不是简单回传原始导入 pinyin；
- 继续保持 relative path、无 secret、无本机路径。

## 不做的事

- 不重写 Python Analyzer；
- 不修改 M9 词典和 context rules；
- 不增加 LLM、ASR、自动翻译；
- 不开发 Web/PWA/iOS/Android Reader；
- 不把 legacy 判断散落在 TTS、Publication 或 UI 业务层；
- 不把原始 ZIP 放入 SQLite。

## 风险与控制

| 风险 | 控制 |
| --- | --- |
| ZIP Slip / 恶意压缩包 | 拒绝绝对路径、`..`、符号链接；限制文件数、压缩和解压大小 |
| token 错位 | 全部使用 Rust Grapheme Token；`concat(tokens.text) == segment.text` |
| tone mark 转换误判 | 只接受可确定转换；失败返回明确错误，不猜读音 |
| imported 被规则覆盖错误 | policy 按来源优先级逐 token 选择；规则不删除 imported baseline |
| 重新分析丢失导入数据 | 保留 imported annotations；authoritative 永不被 analyzer 覆盖 |
| 旧格式污染新代码 | `LegacyAnnotatedDatasetAdapter -> AnnotatedBookV1` |
| 当前项目限制误写进格式 | 5000 汉字只在应用 Preflight 阻止，格式/schema 不写死 |

## 实施顺序

1. Migration、模型和规范化导入 DTO；
2. JSON/ZIP/legacy 解析、验证、事务导入命令；
3. pronunciation policy、规则冲突和 TTS strict/mixed；
4. Publication Bundle translation/pronunciation 扩展；
5. 前端导入确认和直接生成入口；
6. 文档、模板、fixtures、Rust/Python/frontend 测试；
7. 使用真实《黄帝内经精选》数据包做导入、TTS request 和 Bundle 验证。
