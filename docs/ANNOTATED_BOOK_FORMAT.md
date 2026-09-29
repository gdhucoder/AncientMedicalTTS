# Ancient Annotated Book Format v1

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

authoritative 数据支持严格模式和混合模式。严格模式要求所有需要朗读的汉字都有最终强制读音；混合模式必须由用户明确选择，缺失位置会显示数量并交给 TTS 默认判音。

## 当前应用限制

格式本身不限制字数。AncientMedicalTTS 当前制作端仍按项目限制最多 5000 个汉字，超过时导入预检阻止并提示这是应用限制，未来提升制作端上限不需要修改格式版本。

## Legacy 兼容

现有包含 `dataset`、`version`、`chapters` 的 annotated dataset 会在 `LegacyAnnotatedDatasetAdapter` 逻辑中转换为同一内部格式。旧格式默认是 `reference`，用户必须显式信任后才会强制进入 TTS。

另外，阅读端精选内容包 `ancient-medical-reader-selected-content` v1（manifest 使用 `lessons`，lesson 使用 `tokens[].pinyin`）也通过同一兼容边界导入。它默认仍是 `reference`，因此不会因为文件带有拼音就静默改变 TTS；导入确认页勾选“将文件中的注音作为最终发音”后，才会按 `imported_authoritative` 进入强制发音策略。旧包中未标声调的 ASCII 音节按轻声 `5` 兼容，正式 `ancient-annotated-book` v1 仍要求明确声调。
