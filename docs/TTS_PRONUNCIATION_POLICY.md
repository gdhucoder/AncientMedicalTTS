# TTS 发音一致性策略

本文记录 v0.1.0 的三类读音语义，避免阅读界面看到的注音与真正送入腾讯云 TTS 的强制读音混为一谈。

## 三类读音

### 参考注音

参考注音来自 Python analyzer、pypinyin、医学/古籍词典和上下文分析。它用于 Reader 的 `risky` / `all` 注音显示、候选读音和复核提示，数据库仍保存 ASCII 数字声调格式，例如 `e4`、`shu4 xue2`。

参考注音不是用户确认，不会因为出现在页面上就自动进入 TTS。

## 严格按页面注音生成

Reader 的“严格按注音生成”是一个显式的 TTS 模式，不改变上述默认边界。启动该模式时，Rust 再次调用与页面“全文”注音相同的 `pronunciation.display_pinyin`，然后把每个汉字转换为一个独立的 `<phoneme alphabet="py" ph="...">字</phoneme>`。Annotation 中已经显示的人工确认、规则读音会优先作为页面当前读音；其余汉字使用同一次页面全文注音结果。

该模式的 `AudioVersion.pronunciation_mode` 为 `display`，普通重新生成和原有全文生成的模式为 `locked`。两类音频版本不会互相复用。严格模式仍要求 Segment 已完成分析且没有待确认项，并且不修改 `segment_annotations` 或 TTS 文本；它只是把页面注音作为本次合成的明确 SSML 输入。

### 已锁定读音

只有以下三类数据可以进入 TTS 的 SSML `phoneme`：

1. `segment_annotations.review_status = confirmed`、`source = manual`、存在 `target_pinyin` 的当前 Segment 人工确认；
2. `segment_annotations.review_status = confirmed`、存在 `source_rule_id`，且该规则仍存在、启用并且 scope 为 `book` 或 `global` 的规则 Annotation。
3. `segment_annotations.review_status = confirmed`、`source = imported_authoritative` 且存在 `target_pinyin` 的已注音古籍导入标注。

Rust 的 `build_effective_forced_pronunciations` 统一计算这份策略。优先级为本处 manual confirmed > Book Rule > Global Rule > Imported Authoritative > TTS 默认发音。自动分析的 `needs_review`、`ignored`、`source=pypinyin`、词典命中和上下文建议都只属于参考层。

已注音古籍导入有两种模式：`reference` 只写入 `source=imported_reference`，不强制 TTS；`authoritative` 或用户在导入确认页显式信任后写入 `source=imported_authoritative`，可以在不运行 analyzer 的情况下直接按导入注音生成。后续人工确认会把该处改成 `source=manual`，保留人工覆盖来源；Book/Global Rule 也可以覆盖导入基线。

## 强制策略的优先级

策略使用 Grapheme Token 的 `[start_token, end_token)` 范围，不使用 JavaScript 或 Python 字符 offset。重叠候选按以下规则确定：

1. 起始 token 更靠前；
2. 同一起点优先更长范围；
3. 同长度时人工确认优先于 Book Rule，Book Rule 优先于 Global Rule；
4. 最终只保留不重叠的范围。

因此，页面显示某字的 analyzer 建议，并不意味着它会覆盖已确认人工读音；规则也不会覆盖脱离规则的 Segment-specific manual override。

## SSML 消费路径

```text
Segment effective text
  ↓
Rust effective forced pronunciation policy
  ↓
JSONL tts.synthesize pronunciations
  ↓
Python build_ssml
  ↓
Tencent TextToVoice
```

默认没有强制读音时，Worker 将文本原样发送；存在强制读音时，才生成 `<phoneme alphabet="py" ph="...">...</phoneme>`。严格按页面注音模式则会为全文每个汉字生成覆盖。TTS 输入仍使用 Segment 的 `effective_text = reading_text ?? original_text`，注音只控制发音，不改变文本。

导入注音模式按连续已锁定 token、以标点为边界合并 phrase-level phoneme，不为每个字单独创建 SSML 节点；缺失注音的 Segment 只有在用户明确选择混合生成时才以普通文本交给腾讯云。

单 Segment 生成和全文批量生成都复用 Rust AudioService，因此使用同一套策略。导出只读取当前 AudioVersion，不重新分析也不重新生成语音。

## 腾讯云实际发音

TTS 请求启用腾讯 SDK 的 `EnableSubtitle`。当前 SDK 返回的 `Subtitles` 元素包含：

- `Text`
- `Phoneme`
- `BeginTime`
- `EndTime`

Worker 将这些字段规范化为：

```json
{
  "realized_pronunciation": [
    {"text": "恶", "phoneme": "wu4", "begin_ms": 210, "end_ms": 430}
  ]
}
```

该数据只保存到产生它的 `audio_versions.provider_metadata_json`，前端通过当前选择的 AudioVersion 展示为“TTS 实际发音”。它不会回写 `segment_annotations.target_pinyin`、pronunciation rule 或 canonical pinyin。供应商没有返回字幕时音频仍然成功，实际发音显示“未记录”。旧版本没有该元数据也显示“未记录”。

供应商实际读音可能包含语流音变或与强制拼音不同；这不表示数据库中的人工确认被修改。tone sandhi 只能作为该 AudioVersion 的实际结果观察，不能写回规则。

## `恶寒` 验收语义

- analyzer 仅给出 `恶 = e4`、状态为 `needs_review`：页面显示参考注音，SSML 不包含 `ph="e4"`；腾讯返回的 `wu4` 只会保存为该音频版本的实际发音。
- 用户确认 `恶 = wu4`：SSML 包含 `ph="wu4"`。
- 用户确认 `恶 = e4`：SSML 包含 `ph="e4"`。
- Book Rule `恶寒 → e4 han2`：按规则生成整词强制范围并进入 SSML。

这样“参考读音”“将要送入 TTS 的锁定读音”和“腾讯最终实现的读音”在 UI、数据库和运行路径上各自有明确边界。
