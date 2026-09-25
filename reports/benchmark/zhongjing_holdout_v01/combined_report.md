# AncientMedicalTTS 仲景系 Hold-out 泛化评测

本报告只汇总当前 production pronunciation analyzer 的独立文本评测。两套 hold-out 均先通过 Rust → Grapheme Token → Python Worker 生成 raw prediction snapshot，再由独立 evaluator 读取 Gold；本轮没有修改 analyzer、Gold、词典、context rules、Book Rule 或 Global Rule，也没有调用 TTS/ASR。

## Core Metrics

| Dataset | Recall | Default Accuracy | Candidate Coverage | Miss | Wrong Default | Candidate Missing | Extra | Extra / 1000 Han |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 素问 M8.1 | 95.1% | 100.0% | 100.0% | 5 | 0 | 0 | 69 | 20.8 |
| 伤寒论 hold-out | 26.5% | 83.3% | 100.0% | 50 | 3 | 0 | 7 | 4.0 |
| 金匮要略 hold-out | 11.3% | 0.0% | 100.0% | 47 | 6 | 0 | 12 | 5.9 |

## Generalization Gap

| Hold-out | Recall gap vs 素问 | Default gap vs 素问 | Candidate gap vs 素问 |
|---|---:|---:|---:|
| 伤寒论 | -68.6pp | -16.7pp | +0.0pp |
| 金匮要略 | -83.8pp | -100.0pp | +0.0pp |

## Source and Risk Observations

### 伤寒论 hold-out

- Prediction sources：{'context_rule_v2': 14, 'high_risk_polyphone': 9, 'rare_character': 3}。
- Extra sources：{'context_rule_v2': 2, 'high_risk_polyphone': 3, 'rare_character': 2}；extra risk：{'context_pronunciation': 2, 'polyphone': 3, 'rare_character': 2}。
- Extra audit：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 7}。
- Gold risk recall：{'classical_term': {'total': 1, 'detected': 0, 'recall': 0.0}, 'medical_polyphone': {'total': 23, 'detected': 14, 'recall': 0.6086956521739131}, 'medical_term': {'total': 32, 'detected': 1, 'recall': 0.03125}, 'rare_or_classical': {'total': 9, 'detected': 0, 'recall': 0.0}, 'semantic_polyphone': {'total': 3, 'detected': 3, 'recall': 1.0}}。

### 金匮要略 hold-out

- Prediction sources：{'high_risk_polyphone': 16, 'medical_lexicon': 2}。
- Extra sources：{'high_risk_polyphone': 10, 'medical_lexicon': 2}；extra risk：{'medical_term': 2, 'polyphone': 10}。
- Extra audit：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 12}。
- Gold risk recall：{'classical_usage': {'total': 3, 'detected': 0, 'recall': 0.0}, 'medical_polyphone': {'total': 10, 'detected': 5, 'recall': 0.5}, 'medical_term': {'total': 27, 'detected': 0, 'recall': 0.0}, 'rare_character': {'total': 4, 'detected': 0, 'recall': 0.0}, 'rare_or_classical': {'total': 7, 'detected': 0, 'recall': 0.0}, 'semantic_polyphone': {'total': 2, 'detected': 1, 'recall': 0.5}}。

## Hold-out Findings

至少一套 hold-out 未达到工程参考线，当前 analyzer 仍存在泛化风险；建议先做通用词典/语境覆盖和误报治理，再进入大规模 TTS 试听。

- Rare-character bug candidate：查看各书 `extra_predictions.csv` 中 `review_classification=bug`；本次评测不自动修改 analyzer。
- Unicode/Grapheme/Segment：以各书 `metrics.json` 的 `gold_occurrences_crossing_segment_boundary` 和 `textual_issues.csv` 为准。
- Gold 不是 exhaustively annotated，因此 Extra Prediction 仅作为治理信号，不能直接等同 false positive。

## Top Misses

### 伤寒论 hold-out

按 Gold entry 去重；括号内为该 entry 漏掉的 occurrence 数。

| ID | match / focus | Gold | risk | missed occurrences | reason |
|---|---|---|---:|---:|---|
| SHL-G001 | 恶寒 / 恶 | wu4 han2 | medical_polyphone | 6 | medical_term_dictionary_missing |
| SHL-G002 | 中风 / 中 | zhong4 feng1 | medical_polyphone | 2 | medical_term_dictionary_missing |
| SHL-G004 | 失溲 / 溲 | shi1 sou1 | rare_or_classical | 1 | rare_character_not_detected |
| SHL-G005 | 惊痫 / 痫 | jing1 xian2 | medical_term | 1 | medical_term_dictionary_missing |
| SHL-G006 | 瘈疭 / 瘈疭 | chi4 zong4 | medical_term | 1 | medical_term_dictionary_missing |
| SHL-G009 | 啬啬 / 啬啬 | se4 se4 | rare_or_classical | 1 | rare_character_not_detected |
| SHL-G010 | 淅淅 / 淅淅 | xi1 xi1 | rare_or_classical | 1 | rare_character_not_detected |
| SHL-G011 | 翕翕 / 翕翕 | xi1 xi1 | rare_or_classical | 1 | rare_character_not_detected |
| SHL-G013 | 葛根汤 / 葛 | ge2 gen1 tang1 | medical_term | 1 | medical_term_dictionary_missing |
| SHL-G014 | 温针 / 针 | wen1 zhen1 | medical_term | 1 | medical_term_dictionary_missing |

### 金匮要略 hold-out

按 Gold entry 去重；括号内为该 entry 漏掉的 occurrence 数。

| ID | match / focus | Gold | risk | missed occurrences | reason |
|---|---|---|---:|---:|---|
| JGYL-G001 | 脏腑 / 脏 | zang4 fu3 | medical_polyphone | 3 | medical_term_dictionary_missing |
| JGYL-G002 | 中人多死 / 中 | zhong4 ren2 duo1 si3 | semantic_polyphone | 1 | classical_usage_not_detected |
| JGYL-G003 | 疢难 / 疢 | chen4 nan4 | rare_or_classical | 1 | rare_character_notz_detected |
| JGYL-G004 | 干忤 / 忤 | gan1 wu3 | rare_or_classical | 1 | rare_character_not_detected |
| JGYL-G005 | 腠理 / 腠 | cou4 li3 | medical_term | 1 | medical_term_dictionary_missing |
| JGYL-G006 | 喑喑然 / 喑喑 | yin1 yin1 ran2 | rare_or_classical | 1 | rare_character_not_detected |
| JGYL-G007 | 啾啾然 / 啾啾 | jiu1 jiu1 ran2 | rare_or_classical | 1 | rare_character_not_detected |
| JGYL-G008 | 肺痿 / 痿 | fei4 wei3 | medical_term | 1 | medical_term_dictionary_missing |
| JGYL-G010 | 寸口脉 / 寸口 | cun4 kou3 mai4 | medical_term | 1 | medical_term_dictionary_missing |
| JGYL-G011 | 刚痉 / 痉 | gang1 jing4 | medical_term | 2 | medical_term_dictionary_missing |

## Wrong Defaults

- 伤寒论 hold-out：数（2 次）Gold=shuo4 default=shu4 candidates=shu3 | shu4 | shuo4; 更（1 次）Gold=geng1 default=geng4 candidates=geng1 | geng4
- 金匮要略 hold-out：数（6 次）Gold=shuo4 default=shu4 candidates=shu3 | shu4 | shuo4

## Recommendation

1. 当前不建议进入以 analyzer 正确性为前提的全书真实 TTS 验收：两套 hold-out 的 Recall 均低于 75%，且金匮要略 Default Accuracy 为 0%；可以做极小范围人工听音 smoke test，但不能把它当作 production readiness 结论。不要把 hold-out Gold 反向写入词典。
2. 若后续继续优化，优先做通用中医术语/方名覆盖和跨语境 polyphone 规则设计，并用第三套未参与调优的古籍复验。
3. 对 hold-out 中的 rare/classical、文本底本和争议读音维持人工复核队列，不用 benchmark-specific special case 压低 Miss 或 Extra。

## Reproducibility

本次 runner：0.3.0-holdout；Git SHA：unavailable: workspace is not a git worktree。每本书的 raw predictions、metrics、miss、wrong、extra、review 和 textual issues 均保存在各自目录。
