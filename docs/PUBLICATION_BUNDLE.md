# Publication Bundle v1

AncientMedicalTTS 的“移动端发布包”是制作端交给未来 Web/PWA/iOS/Android 阅读端的只读成品接口。它不包含 SQLite、规则历史、日志、本机路径或腾讯云凭证；本版本不包含播放器代码，也不负责上传和云同步。

## 目录

```text
黄帝内经·素问/
├── manifest.json
├── book.json
├── chapters/
│   ├── chapter-001.json
│   └── chapter-002.json
└── audio/
    ├── chapter-001.mp3
    └── chapter-002.mp3
```

用户在 Book 页面“更多…”中选择“导出移动端发布包”，再选择目标目录。应用会在目标目录下创建经过文件名清理的书籍目录；同名目录不会被静默覆盖。

## 版本与清单

`manifest.json` 固定使用：

```json
{
  "format": "ancient-medical-publication-bundle",
  "format_version": "1.0",
  "generator": { "name": "AncientMedicalTTS", "version": "0.1.0" },
  "generated_at": "2026-09-28T00:00:00Z",
  "book": "book.json"
}
```

阅读端必须先检查 `format` 和 `format_version`，不兼容时不要尝试猜测字段含义。

## Book 与 Chapter

`book.json` 只输出数据库中已有的作者、朝代和版本信息；缺失值为 `null`。语言固定为 `zh-CN`，朗读模式固定为 `modern_standard_mandarin`。章节引用包含：

- `id`：数据库 Chapter UUID；
- `order`：按数据库 `Chapter.order_index` 排序后的 1-based 顺序；
- `title`：数据库标题，缺失时使用“第 N 章”；
- `content`：章节 JSON 的相对路径；
- `audio`：章节 MP3 的相对路径。

每个 Chapter 生成一个 MP3。音频源是该章节内 `speak_enabled=true` 的当前 `current_audio_id` WAV，严格按 `Segment.order_index` 排序；不朗读 Segment 仍写入章节正文，但不会写入音频和时间轴。

## Segment 与时间轴

章节 JSON 的 Segment 使用当前有效朗读文本：

```text
reading_text ?? original_text
```

示例：

```json
{
  "id": "segment-uuid",
  "order": 3,
  "text": "乃问于天师曰……",
  "translation": "于是向天师询问……",
  "speak_enabled": true,
  "start_ms": 6850,
  "end_ms": 15260,
  "duration_ms": 8410,
  "pronunciation": null
}
```

时间轴来自实际当前 WAV 的 PCM sample count，不使用文本长度估算。先将章节 WAV 用 FFmpeg concat demuxer 合并为连续 WAV，再只编码一次 MP3（96 kbps、当前音频采样率和声道数）。因此 `start_ms` 是移动端对章节 MP3 执行 `audio.currentTime = start_ms / 1000` 的目标位置。

没有额外插入静音、crossfade 或音量处理。MP3 实际时长通过内置 FFmpeg 读取并与最后一个 Segment 的 `end_ms` 对比，允许差异不超过 100ms；超出即阻止发布。

`speak_enabled=false` 的 Segment 的 `start_ms`、`end_ms`、`duration_ms` 必须为 `null`。点击这类文字只能阅读，不能定位到章节音频。

## 发音信息

v1 不以拼音阻塞出版。发布包会优先输出当前有效的参考/已锁定 token 读音；如果当前 AudioVersion 保存了腾讯返回的实际发音，也会一并写入：

```json
{
  "pronunciation": {
    "tokens": [
      { "text": "恶", "reference_pinyin": "e4", "confirmed_pinyin": "wu4", "forced": true }
    ],
    "tts_actual": [
      { "text": "恶", "pinyin": "wu4", "begin_ms": 210, "end_ms": 430 }
    ]
  }
}
```

`reference_pinyin`、`confirmed_pinyin` 和 `tts_actual.pinyin` 继续使用 ASCII 数字声调。参考注音、已锁定读音和腾讯实际发音是三种不同数据，不在导出时互相覆盖。

## 预检与只读保证

发布前必须满足：Book/Chapter 存在；所有可朗读 Segment 为 `generated`；有 `current_audio_id` 和对应 AudioVersion；WAV 文件存在且为有效 PCM WAV；章节内语音设置和 WAV 参数一致；没有 stale 音频。预检失败时不会生成或修改任何数据库数据。

发布过程只写应用缓存目录和用户选择的目标目录，不新增 AudioVersion、不改变 Segment 状态、不修改发音规则。失败会清理缓存目录；已有同名目标目录不会被覆盖。

## 校验器

可用无第三方依赖的校验器检查发布包：

```bash
python3 tools/validate_publication_bundle.py /path/to/黄帝内经-素问
```

它会检查版本、章节和 Segment 唯一性、顺序、相对路径、音频存在性、不朗读段落的空时间轴、时间轴不重叠、末尾时长容差以及凭证字段泄露。

完整字段说明见 [publication-bundle-v1.schema.json](publication-bundle-v1.schema.json)。
