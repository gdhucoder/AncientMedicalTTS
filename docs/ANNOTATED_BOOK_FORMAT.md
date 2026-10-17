# Ancient Annotated Book Format v1.0

Ancient Annotated Book Format 是 AncientMedicalTTS 的通用已注音古籍输入格式，格式名为 `ancient-annotated-book`，当前版本为 `1.0`。它与 TXT 导入并存：TXT 仍然走自动分析和人工校音；已注音数据先验证，再按用户选择作为参考或最终发音导入。

## 目录

```text
book.zip
└── book/
    ├── manifest.json
    └── chapters/
        ├── 001.json
        └── 002.json
```

也支持一个内嵌 `chapters[].segments` 的单 JSON 文件。ZIP 中的路径必须是相对路径，不能包含绝对路径、`..` 或符号链接。

## manifest.json

```json
{
  "format": "ancient-annotated-book",
  "format_version": "1.0",
  "book": {
    "id": "shanghanlun-selected",
    "title": "伤寒论精选",
    "author": null,
    "collection": "",
    "language": "zh-CN",
    "edition": null
  },
  "pronunciation": { "mode": "authoritative" },
  "chapters": [
    {
      "id": "chapter-001",
      "order": 1,
      "collection": "正文",
      "title": "辨太阳病脉证并治",
      "subtitle": null,
      "file": "chapters/001.json"
    }
  ],
  "sources": {
    "original": null,
    "pronunciation": null,
    "translation": null
  }
}
```

`reference` 表示参考注音，默认不会强制送入 TTS；`authoritative` 表示提供方确认的最终读音。即便文件是 `reference`，导入确认时也可以显式选择“将文件中的注音作为最终发音”，本次导入会保存为 authoritative。

## Chapter 与 Segment

```json
{
  "id": "chapter-001",
  "order": 1,
  "title": "辨太阳病脉证并治",
  "segments": [
    {
      "id": "chapter-001-seg-001",
      "order": 1,
      "text": "恶寒。",
      "pinyin_numeric": "wu4 han2。",
      "pinyin_tone_marks": "wù hán。",
      "tokens": [
        { "text": "恶", "pinyin_numeric": "wu4" },
        { "text": "寒", "pinyin_numeric": "han2" },
        { "text": "。", "pinyin_numeric": null }
      ],
      "translation": "怕冷。"
    }
  ]
}
```

`tokens` 是最可靠的形式。`concat(tokens[].text)` 必须严格等于 `text`，每个 token 使用 Rust Grapheme Token 规则对应一个 grapheme，标点和空格的 pinyin 必须为 `null`。没有 tokens 时，导入器才会按汉字 grapheme 与拼音音节做严格一一对齐；数量不一致会返回 `ALIGNMENT_ERROR`，不会猜测。

拼音的内部 canonical 形式是 ASCII 字母加数字声调，例如 `wu4`、`nv3`、`nve4`、`lv4`。也接受带声调符号的输入并转换为 canonical 形式，例如 `wù`、`nǚ`、`nüè`、`lǜ`。

优先级固定为：`tokens[].pinyin_numeric` > `segment.pinyin_numeric` > `segment.pinyin_tone_marks`。译文会保存为 Segment 内容，但不会参加发音分析、字符统计或 TTS。

## 导入后的来源和 TTS

- 参考注音保存为 `source=imported_reference`，只作为页面参考；
- 最终注音保存为 `source=imported_authoritative`，生成 TTS 时作为强制读音；
- 人工确认和 Book/Global Rule 仍可覆盖导入基线；
- 最终 TTS 优先级为：Manual Confirmed > Book Rule > Global Rule > Imported Authoritative > TTS Default；
- Analyzer 不会覆盖 `imported_authoritative`，但仍可提供建议；
- 对连续、未跨标点的强制读音，SSML 会合并为 phrase-level `<phoneme>`，标点在 phoneme 外部。

authoritative 数据支持严格模式和混合模式。严格模式要求所有需要朗读的汉字都有最终强制读音；混合模式必须由用户明确选择，缺失位置会显示数量并交给 TTS 默认判音。导入完成且覆盖完整时，Reader 顶部的主操作会显示“按导入注音生成”，直接复用现有串行批量生成；严格入口也保留在“更多…”菜单中。批量生成时，`imported_authoritative` 会经过现有 `AudioService` 转为 phrase-level SSML `<phoneme>`，而不是只用于页面显示。

## 当前应用限制

格式本身不限制字数。AncientMedicalTTS 当前制作端仍按项目限制最多 5000 个汉字，超过时导入预检阻止并提示这是应用限制，未来提升制作端上限不需要修改格式版本。

## Legacy 兼容

现有包含 `dataset`、`version`、`chapters` 的 annotated dataset 会在 `LegacyAnnotatedDatasetAdapter` 逻辑中转换为同一内部格式。旧格式默认是 `reference`，用户必须显式信任后才会强制进入 TTS。

另外，阅读端精选内容包 `ancient-medical-reader-selected-content` v1（manifest 使用 `lessons`，lesson 使用 `tokens[].pinyin`）也通过同一兼容边界导入。它默认仍是 `reference`，因此不会因为文件带有拼音就静默改变 TTS；导入确认页勾选“将文件中的注音作为最终发音”后，才会按 `imported_authoritative` 进入强制发音策略。旧包中未标声调的 ASCII 音节按轻声 `5` 兼容，正式 `ancient-annotated-book` v1 仍要求明确声调。

## 正式制作约束（适用于后续新书）

上面的格式说明是正式规范。后续制作新书时，优先遵守本节的“必须”约束；旧版兼容格式只用于导入已有数据，不作为新项目模板。

### A. 必须使用的正式根字段

新书的 `manifest.json` 必须包含：

```json
{
  "format": "ancient-annotated-book",
  "format_version": "1.0",
  "book": {
    "id": "稳定且唯一的书籍 ID",
    "title": "书名",
    "language": "zh-CN"
  },
  "pronunciation": {
    "mode": "reference"
  },
  "chapters": []
}
```

以下字段可以省略或填 `null`，不能为了完整而虚构：

```text
author
collection
edition
subtitle
sources
```

### B. 每个章节的最小要求

manifest 中每个章节必须声明：

```json
{
  "id": "chapter-001",
  "order": 1,
  "title": "篇名",
  "file": "chapters/001.json"
}
```

章节文件必须包含：

```json
{
  "id": "chapter-001",
  "order": 1,
  "title": "篇名",
  "segments": []
}
```

要求：

- 章节 `id` 全书不重复；
- 章节 `order` 不重复；
- manifest 和章节文件中的 `id`、`order` 必须一致；
- `file` 指向的文件必须存在；
- 建议 `order` 从 1 开始连续编号；
- 章节文件建议一章一个 JSON，不要把整本书全部写在一个巨大文件中。

### C. 每个 Segment 的最小要求

```json
{
  "id": "chapter-001-seg-001",
  "order": 1,
  "text": "恶寒。",
  "tokens": [
    { "text": "恶", "pinyin_numeric": "wu4" },
    { "text": "寒", "pinyin_numeric": "han2" },
    { "text": "。", "pinyin_numeric": null }
  ]
}
```

要求：

- Segment `id` 全书不重复；
- 同一章节内 `order` 不重复；
- `text` 不能为空；
- `concat(tokens[].text)` 必须逐字等于 `text`；
- 标点和空格的拼音必须为 `null`；
- 汉字拼音使用 ASCII 数字声调，例如 `wu4`、`han2`、`nv3`、`nve4`、`lv4`；
- 不要把现代汉语译文混入 `text`；
- `translation` 可以保存译文，但不会送入 TTS。

### D. 推荐始终提供 tokens

虽然 `tokens` 是可选字段，但正式书籍强烈建议每个 Segment 都提供完整 Token：

```text
正文 → 逐字 Token → 逐字拼音
```

这样可以避免：

- 多音字无法定位；
- 生僻字或扩展区汉字被错误切分；
- 标点导致的拼音错位；
- 整段拼音音节数量不一致。

没有 `tokens` 时，系统只能尝试按汉字 Grapheme 和拼音音节严格对齐。数量不一致会产生 `ALIGNMENT_ERROR`，不会猜测。

### E. reference 和 authoritative 的选择

建议按以下标准选择：

```text
尚未逐字复核       → reference
已经人工确认读音   → authoritative
```

`reference` 不会默认强制进入 TTS；用户可以在导入确认页显式信任该文件。

`authoritative` 会作为导入发音基线，但人工校音、Book Rule 和 Global Rule 仍然可以覆盖它。

### F. 当前应用限制和 ZIP 安全限制

当前制作端限制：

```text
单个 Book 最多 5000 个汉字
```

这是应用版本限制，不是格式版本限制。

当前 ZIP 安全限制：

```text
文件数 ≤ 512
压缩数据 ≤ 100 MiB
解压数据 ≤ 200 MiB
```

同时禁止：

- 绝对路径；
- `..` 路径穿越；
- Windows 盘符路径；
- 符号链接；
- 非 UTF-8 JSON；
- API Key、Secret、数据库、日志和本机路径。

## 新书交付检查清单

每整理一本新书，打包前按此清单检查：

### 文件结构

- [ ] ZIP 内存在 `manifest.json`；
- [ ] `format` 为 `ancient-annotated-book`；
- [ ] `format_version` 为字符串 `1.0`；
- [ ] 所有 `chapters[].file` 都能找到对应文件；
- [ ] 路径全部是相对路径；
- [ ] 没有 `..`、绝对路径和符号链接；
- [ ] 没有把音频缓存、数据库和密钥放进 ZIP。

### 书籍和章节

- [ ] `book.id` 稳定且唯一；
- [ ] `book.title` 已填写；
- [ ] 作者、朝代、版本不确定时使用 `null`；
- [ ] 章节 ID 和 order 不重复；
- [ ] 章节顺序与正文顺序一致；
- [ ] 章节标题使用 `title`；
- [ ] 章节正文位于 `segments`。

### 正文和拼音

- [ ] 每个 Segment 有唯一 ID 和 order；
- [ ] `text` 不为空；
- [ ] Token 拼接结果与正文完全一致；
- [ ] 每个 Token 是一个 Unicode Grapheme；
- [ ] 标点和空格没有拼音；
- [ ] 拼音使用数字声调；
- [ ] `nü/lü/nüe/lüe` 已规范成 `nv/lv/nve/lve`；
- [ ] 多音字已经人工复核；
- [ ] 需要强制朗读的汉字都有拼音。

### 译文和来源

- [ ] 译文只填写在 `translation`；
- [ ] 译文没有混入 TTS 文本；
- [ ] 原文、拼音和译文来源已记录；
- [ ] 没有虚构作者、朝代、版本等元数据。

## 推荐的后续工作流

```text
整理原文
    ↓
按章节拆分
    ↓
按 Segment 拆分
    ↓
逐字生成或人工整理 tokens
    ↓
人工检查多音字和古音
    ↓
设置 reference / authoritative
    ↓
建立 manifest.json
    ↓
压缩为 ZIP
    ↓
导入应用并查看预检结果
    ↓
确认注音模式
    ↓
批量生成 TTS
    ↓
抽查音频
    ↓
导出 Publication Bundle
```

## 模板和 Schema

新书可以直接复制以下模板：

- [JSON Schema](../schemas/annotated-book-v1.schema.json)
- [manifest 模板](../examples/annotated-book-template/manifest.json)
- [章节模板](../examples/annotated-book-template/chapters/001.json)

正式书籍只需要按照模板增加章节和 Segment，不要改变字段含义。
