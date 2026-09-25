# Manual Segment Editing

## 文本边界

导入原文保存在 `segments.original_text`，人工编辑不会覆盖它。用户编辑的可选 `segments.reading_text` 是当前朗读文本；当该字段为 `NULL` 时：

```text
effective_text = reading_text ?? original_text
```

TTS 请求、发音分析、Reader token 和完整音频导出都使用 effective text。发音 Annotation 数据模型不因为文本编辑而改变。

## 支持的操作

- 在 Current Segment 中编辑朗读文本，可增加或删除标点、空格；
- 点击 Grapheme Token 选择光标位置并 Split；
- 与上一 Segment 合并；
- 与下一 Segment 合并；
- 恢复 `reading_text` 为原文；
- 设置 Segment 是否参与朗读。

## 事务和历史数据

`reading_text` 修改、Split、Merge 都由 Rust 在 SQLite transaction 中执行。修改有效文本时会删除受影响 Segment 的发音 Annotation，清空 `current_audio_id`，并将状态置为 `pending`。旧 `AudioVersion` 不删除，因此历史数据仍可保留旧语音，但它不再是当前有效音频。

Split/Merge 不修改已经存在的 `original_text`。旧 Segment 标记为 `superseded`，新的活动 Segment 使用拆分或合并后的原文创建；活动列表、发音规则、批量生成和完整导出都排除 superseded 行。

Split/Merge 完成后 Reader 自动对新 Segment 重新执行发音分析。当前版本不做 Annotation 的复杂 token remapping，因此操作后以新文本重新分析为准。

`speak_enabled=false` 的 Segment 仍可在 Reader 中查看和编辑，但不会进入批量 TTS，也不会出现在完整音频导出的拼接列表中。重新开启朗读后，仍需按当前状态确认是否需要生成新的语音。
