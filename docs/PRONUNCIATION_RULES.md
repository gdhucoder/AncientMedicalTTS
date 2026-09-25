# Pronunciation Rules — Milestone 5

## Scope

当前只支持两种作用域：

- `book`：必须绑定 `book_id`，只作用于该 Book；
- `global`：`book_id` 必须为空，作用于所有 Book。

“仅本处”不创建规则，继续使用 `segment_annotations` 中 `source_rule_id = NULL` 的人工确认。

## Matching

规则匹配完全在 Rust `PronunciationRuleService` 中完成。Rust 先把 Segment 切成 Grapheme Token，再把 `pattern_text` 也切成 Grapheme Token，使用完整 token 序列进行 exact match。Annotation 范围使用 `[start_token, end_token)`，不使用 JavaScript 或 Unicode 字节 offset。

当前不支持模糊匹配、正则、同义词、简繁/异体转换、拼音匹配或上下文 target。规则 pattern 可以包含汉字和标点，但至少必须包含一个汉字；汉字 token 数量必须和 target pinyin 音节数一致。

## Priority

有效发音的固定优先级是：

1. 当前 Segment 的人工 confirmed；
2. 当前 Segment 的人工 ignored（阻止规则覆盖该位置）；
3. Book Rule；
4. Global Rule；
5. Python 自动分析的 `needs_review`；
6. TTS 默认发音。

规则之间使用 Longest Match First。同一位置长度相同则 Book Rule 优先；同 scope、同 pattern 不允许存在不同 target pinyin。

## Rule Annotation

命中规则后，Rust 创建或更新：

```text
review_status = confirmed
target_pinyin = rule.target_pinyin
source_rule_id = rule.id
```

规则 Annotation 与普通人工 Annotation 使用相同的 TTS 输入路径，Tencent Provider 不需要额外的规则逻辑。

## Manual Override

用户在规则 Annotation 的详情中再次确认读音，或者执行忽略/状态修改时，Rust 会把 `source_rule_id` 清空。该位置随后成为 Segment-specific manual override，后续重新应用规则不会覆盖它。

## Rule Lifecycle

- 创建：从 Annotation 详情选择“应用到本书”或“加入全局词典”，也可以在“发音词典”页面新增；创建后立即应用到作用范围内已有 Segment；
- 更新：修改 pattern 或 target 后重新应用作用范围，只更新仍绑定当前 rule 的 Annotation；
- 禁用/启用：使用 `enabled` 软状态。禁用会重新计算作用范围，必要时由 Global Rule 接管；全局规则变化会扫描当前所有 Book；
- 应用到 Book：只执行 Rust Rule Matcher，不调用 Python analyzer，不调用 Tencent TTS。

规则改变有效 confirmed pronunciation 时，Segment 若已有生成音频则状态变为 `ready`，但保留 `current_audio_id` 和旧 WAV。应用不会自动生成新音频，用户手动重新生成后才恢复 `generated`。
