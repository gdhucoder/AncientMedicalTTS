# AncientMedicalTTS M9 三数据集回归

三套数据从 M9 起均为 development/regression datasets。本报告不把它们作为最终泛化证明；新的独立 final hold-out 尚未创建 Gold。

## v0.2 → v0.3

| Dataset | Version | Recall | Default Accuracy | Candidate Coverage | Miss | Wrong Default | Candidate Missing | Extra | Extra / 1000 Han |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 素问 | v0.2.1 | 95.1% | 100.0% | 100.0% | 5 | 0 | 0 | 69 | 20.8 |
| 素问 | v0.3.0 | 99.0% | 100.0% | 100.0% | 1 | 0 | 0 | 74 | 22.3 |
| 伤寒论 | v0.2.1 | 26.5% | 83.3% | 100.0% | 50 | 3 | 0 | 7 | 4.0 |
| 伤寒论 | v0.3.0 | 91.2% | 100.0% | 100.0% | 6 | 0 | 0 | 69 | 39.8 |
| 金匮要略 | v0.2.1 | 11.3% | 0.0% | 100.0% | 47 | 6 | 0 | 12 | 5.9 |
| 金匮要略 | v0.3.0 | 94.3% | 100.0% | 100.0% | 3 | 0 | 0 | 35 | 17.2 |

## Source Contribution

| Dataset | Source | Predictions | Correct Gold Hits | Extra |
|---|---|---:|---:|---:|
| 素问 | context_exact | 50 | 41 | 9 |
| 素问 | context_semantic | 12 | 0 | 12 |
| 素问 | high_risk_polyphone | 38 | 0 | 38 |
| 素问 | medical_lexicon_v3 | 48 | 33 | 15 |
| 素问 | rare_classical_lexicon | 27 | 27 | 0 |
| 素问 | variant_mapping | 1 | 0 | 0 |
| 伤寒论 | context_exact | 14 | 12 | 2 |
| 伤寒论 | context_semantic | 6 | 5 | 1 |
| 伤寒论 | high_risk_polyphone | 4 | 0 | 3 |
| 伤寒论 | medical_lexicon_v3 | 94 | 33 | 61 |
| 伤寒论 | rare_classical_lexicon | 14 | 12 | 2 |
| 金匮要略 | context_semantic | 13 | 7 | 6 |
| 金匮要略 | high_risk_polyphone | 6 | 0 | 6 |
| 金匮要略 | medical_lexicon_v3 | 53 | 30 | 22 |
| 金匮要略 | rare_classical_lexicon | 14 | 13 | 1 |

## Remaining Top Misses

### 伤寒论

| Match / Focus | Risk | Missed | Reason |
|---|---|---:|---|
| 温针 / 针 | medical_term | 1 | medical_term_dictionary_missing |
| 悸而惊 / 悸 | medical_term | 1 | medical_term_dictionary_missing |
| 四逆辈 / 逆 | medical_term | 1 | medical_term_dictionary_missing |
| 其脏有寒 / 脏 | medical_polyphone | 1 | medical_term_dictionary_missing |
| 下焦 / 焦 | medical_term | 1 | medical_term_dictionary_missing |
| 乍有轻时 / 乍 | rare_or_classical | 1 | rare_character_not_detected |

### 金匮要略

| Match / Focus | Risk | Missed | Reason |
|---|---|---:|---|
| 蚀于喉 / 蚀 | medical_term | 1 | medical_term_dictionary_missing |
| 脓已成 / 脓 | medical_term | 1 | medical_term_dictionary_missing |
| 痹 / 痹 | medical_term | 1 | medical_term_dictionary_missing |

## Engineering Assessment

M9 Recall、Default Accuracy、Candidate Coverage 与素问不回退等核心参考线：达到。

- Extra / 1000 Han 的建议线（<30）：素问 达到，伤寒论 未达到，金匮要略 达到。伤寒论为 39.8；聚合审计显示主要来自太阳、阳明、恶寒和方剂名等可解释的 verified 医学词典命中，bug 分类为 0。本轮不通过删除通用领域词条或增加数据集特例来压低该数值。
- Ablation：本轮未把模块开关引入 production analyzer；为避免形成运行时可变语义，未执行 ablation。来源贡献表用于判断各模块收益。
- common-char rare 回归由 Python 自动测试覆盖：子、人、上、下、之、而、于、中、数、少不会因默认拼音链路异常成为 rare/unknown。
- 下一步应建立《难经》或《神农本草经》的独立 final hold-out，再决定是否开始更大范围真实 TTS 试听。
