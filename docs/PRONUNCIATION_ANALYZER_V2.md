# Pronunciation Analyzer v0.2.1

Milestone 8 的分析器只处理 Rust 传入的 Segment 文本和 Grapheme Token，不访问 SQLite、不读取 Gold JSON，也不调用 TTS。

## 处理顺序

1. Rust 生成并传入 `[start_token, end_token)` Grapheme Token。
2. Python 用 pypinyin TONE3 生成基础 default/candidates。
3. 以最长匹配顺序检查 `medical_terms_v2.json` 和 `context_pronunciation_rules.json`。
4. 对未被词组覆盖的 Token 检查分析期异体映射、高风险多音字和已知生僻字。
5. 普通 pypinyin 单读音或普通多音字不自动生成 Annotation。
6. 返回 `source`、`risk_type`、`reason` 和候选读音，Rust 再做范围、surface 和拼音音节数校验。

Rust 落库时仍由既有优先级保护人工确认/忽略和 Book/Global Rule；Python 不知道这些数据库状态。

## 来源和风险

| source | risk_type | 含义 |
| --- | --- | --- |
| `medical_lexicon` | `medical_term` | 命中版本化医学/古籍术语，默认仍为 `needs_review` |
| `context_rule_v2` | `context_pronunciation` | 命中完整短语的确定性上下文读音，默认仍为 `needs_review` |
| `high_risk_polyphone` | `polyphone` | 只对高风险多音字表中的字保留人工复核 |
| `rare_character` | `rare_character` | 已知生僻/古籍字，即使 pypinyin 有读音也提示复核 |
| `variant_mapping` | `textual_variant` | 只做分析匹配或文本异文提示，不改变 surface text |
| `pypinyin` | `rare_character` | pypinyin 无有效拼音时的兜底错误提示 |

上下文结果会把上下文读音放在 default，同时保留 pypinyin 候选。医学词典和上下文规则命中同一范围但读音不一致时，不静默选择：结果 default 为空、候选取并集，并在 Worker stderr 和响应 `warnings` 中记录 `ANALYZER_KNOWLEDGE_CONFLICT`。

## 资源文件

- `medical_terms.json`：v0.1 旧资源，保留用于历史追溯；生产 v0.2 使用 `medical_terms_v2.json`。
- `medical_terms_v2.json`：独立于用户 Pronunciation Rule 的程序资源，按完整词语和最长匹配使用。
- `context_pronunciation_rules.json`：完整词/短语级上下文规则，不支持正则、模糊匹配或 target 子位置。
- `high_risk_polyphones.json`：有限的高风险多音字清单；不把所有 pypinyin 候选变成告警。
- `known_rare_characters.json`：已知生僻/古籍字清单。
- `variant_characters.json`：surface → canonical 的分析期映射。原文、Token surface 和数据库文本不会被改写；`汨汨/汩汩` 只作为异文复核提示。

## 版本和复核

Worker `system.ping` 返回 `0.2.0`；本次 M8.1 的 `pronunciation.analyze` 返回 analyzer version `0.2.1`。自动分析 Annotation 永远为 `needs_review`，高置信词典也不会自动 confirmed。重新分析只刷新旧的 needs_review，既有人工确认、忽略和规则覆盖由 Rust 保留。pypinyin 返回无声调值时，分析器会从同一字符的 heteronym 候选中选择第一个有效 tone-number 读音，避免把正常字符错误标记成 `rare_character`。

## Benchmark 结果

评测脚本在 Rust production test 中导入真实 TXT、调用真实 Worker，并在生产分析完成后才由独立 Python evaluator 读取 Gold。M8 输出位于 `reports/benchmark/huangdi_neijing_v01/m8/`，不会覆盖 v1 baseline。

M8.1 在同一 3273 汉字、4 Chapter、85 Segment 上重跑：检测召回 95.1%（97/102），default 准确率 100%（97/97），candidate coverage 100%（97/97），extra predictions 从 79 降至 69。上一轮 79 条 extra 全量按通用审计规则分类为 useful 69、bug 10；10 条 bug 均为“候选存在但默认拼音为空”的 pypinyin 归一化问题，修复后不再产生。剩余漏报和评测明细已列在 M8.1 输出目录，不在生产代码中为 Gold 条目增加特殊分支。
