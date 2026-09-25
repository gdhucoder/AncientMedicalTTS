# AncientMedicalTTS《伤寒论》发音评测 holdout

## Executive Summary

本报告评估实际生产 pronunciation analyzer 0.2.1。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：TXT 共 1735 个汉字；production snapshot 实际包含 1735 个汉字。Gold 对齐 7 个 Chapter、51 个 Segment；production 原始结果为 1 个 Chapter、48 个 Segment。
- Hard Gold：68 个 occurrence（43 个条目）。
- Detection Recall：18 / 68 = 26.5%。
- 焦点读音评估：15 / 18 default exact = 83.3%；candidate coverage = 100.0%。
- 漏报 50；wrong default 3；candidate missing 0；extra predictions 7（每千汉字 4.0）。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/shanghanlun_holdout_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/shanghanlun_gold_v01.json
- review_required：3 个条目、3 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。
- Chapter alignment：{'applied': True, 'source_chapters': 1, 'projected_chapters': 7, 'titles': ['辨太阳病脉证并治（上）', '辨太阳病脉证并治（中）', '辨阳明病脉证并治', '辨少阳病脉证并治', '辨太阴病脉证并治', '辨少阴病脉证并治', '辨厥阴病脉证并治'], 'prediction_crossings': 0}
- Git commit：unavailable: workspace is not a git worktree
- Worker Python：3.12.13；pypinyin：0.55.0；benchmark runner：0.3.0-holdout。
- Production resources：见 metrics.json 中的 resource_manifest SHA-256。

## Current Production Analyzer Behavior

当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v0.2.x 使用版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。

## Detection

整体：18 / 68 = 26.5%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_term | 0 | 1 | 0.0% |
| medical_polyphone | 14 | 23 | 60.9% |
| medical_term | 1 | 32 | 3.1% |
| rare_or_classical | 0 | 9 | 0.0% |
| semantic_polyphone | 3 | 3 | 100.0% |

## Prediction Sources

| source | all predictions | extra predictions |
|---|---:|---:|
| context_rule_v2 | 14 | 2 |
| high_risk_polyphone | 9 | 3 |
| rare_character | 3 | 2 |

## Pronunciation

焦点级评估：15 / 18 default exact；18 / 18 candidate contains Gold。完整 match_text 可比较的 occurrence：12，其中 full-span default exact 12。

## Most Important Misses


## Focus Term Checks

| term | Gold status | occurrences | detected | default observations | candidate observations |
|---|---|---:|---:|---|---|
| 恶寒 | gold | 15 | 0 | — | — |
| 中风 | gold | 6 | 0 | — | — |
| 脉数急 | gold | 1 | 0 | — | — |
| 瘈疭 | gold | 1 | 0 | — | — |
| 阳数七 | gold | 1 | 0 | — | — |
| 阴数六 | gold | 1 | 0 | — | — |
| 啬啬 | gold | 1 | 0 | — | — |
| 淅淅 | gold | 1 | 0 | — | — |
| 翕翕 | gold | 1 | 0 | — | — |
| 厚朴 | gold | 2 | 0 | — | — |
| 厥逆 | gold | 1 | 0 | — | — |
| 肉瞤 | review_required | 1 | 0 | — | — |
| 目瞑 | gold | 1 | 0 | — | — |
| 衄 | gold | 3 | 3 | nv4 | nv4 |
| 不更衣 | gold | 1 | 0 | — | — |
| 哕 | gold | 2 | 0 | — | — |
| 濈然 | not_listed | 1 | 0 | — | — |
| 少阳 | gold | 6 | 6 | shao4 yang2 | shao3 yang2 | shao4 yang2 |
| 谵语 | gold | 1 | 0 | — | — |
| 胁下硬满 | gold | 1 | 0 | — | — |
| 太阴 | gold | 4 | 0 | — | — |
| 少阴 | gold | 8 | 8 | shao4 yin1 | shao3 yin1 | shao4 yin1 |
| 脉细沉数 | gold | 1 | 0 | — | — |
| 厥阴 | gold | 3 | 0 | — | — |
| 吐蛔 | gold | 1 | 0 | — | — |
| 黄芩 | not_listed | 2 | 0 | — | — |
| 除中 | gold | 2 | 0 | — | — |
| 索饼 | gold | 1 | 0 | — | — |
| 几几 | review_required | 1 | 0 | — | — |
| 少腹 | not_listed | 1 | 0 | — | — |

Extra risk_type 分布：context_pronunciation=2, polyphone=3, rare_character=2。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G001 | 辨太阳病脉证并治（上） | 恶寒 / 恶 | wu4 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G002 | 辨太阳病脉证并治（上） | 中风 / 中 | zhong4 feng1 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G002 | 辨太阳病脉证并治（上） | 中风 / 中 | zhong4 feng1 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G004 | 辨太阳病脉证并治（上） | 失溲 / 溲 | shi1 sou1 | rare_or_classical | rare_character_not_detected |
| SHL-G005 | 辨太阳病脉证并治（上） | 惊痫 / 痫 | jing1 xian2 | medical_term | medical_term_dictionary_missing |
| SHL-G006 | 辨太阳病脉证并治（上） | 瘈疭 / 瘈疭 | chi4 zong4 | medical_term | medical_term_dictionary_missing |
| SHL-G009 | 辨太阳病脉证并治（上） | 啬啬 / 啬啬 | se4 se4 | rare_or_classical | rare_character_not_detected |
| SHL-G010 | 辨太阳病脉证并治（上） | 淅淅 / 淅淅 | xi1 xi1 | rare_or_classical | rare_character_not_detected |
| SHL-G011 | 辨太阳病脉证并治（上） | 翕翕 / 翕翕 | xi1 xi1 | rare_or_classical | rare_character_not_detected |
| SHL-G013 | 辨太阳病脉证并治（上） | 葛根汤 / 葛 | ge2 gen1 tang1 | medical_term | medical_term_dictionary_missing |
| SHL-G014 | 辨太阳病脉证并治（上） | 温针 / 针 | wen1 zhen1 | medical_term | medical_term_dictionary_missing |
| SHL-G015 | 辨太阳病脉证并治（上） | 厚朴 / 朴 | hou4 po4 | medical_term | medical_term_dictionary_missing |
| SHL-G016 | 辨太阳病脉证并治（中） | 厥逆 / 厥 | jue2 ni4 | medical_term | medical_term_dictionary_missing |
| SHL-G017 | 辨太阳病脉证并治（中） | 筋惕 / 惕 | jin1 ti4 | medical_term | medical_term_dictionary_missing |
| SHL-G019 | 辨太阳病脉证并治（中） | 目瞑 / 瞑 | mu4 ming2 | rare_or_classical | rare_character_not_detected |

## Wrong Defaults

| ID | occurrence | focus | Gold focus | system default | candidates |
|---|---:|---|---|---|---|
| SHL-G003 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| SHL-G021 | 1 | 更 | geng1 | geng4 | geng1 | geng4 |
| SHL-G035 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |

## Extra Predictions

共 7 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。已对全部条目按通用审计规则分类：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 7}。分类只用于复核，不改变硬指标；完整结果见 extra_predictions.csv，下面展示前 7 条。

| chapter | segment | surface | default | risk | source | classification | reason | context |
|---|---:|---|---|---|---|---|---|---|
| 辨太阳病脉证并治（上） | 0 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 

太阳之为病，脉浮，头项强痛而恶寒。太阳病，发热，汗出，恶风，脉缓者，名为中风。太阳病，或已发热， |
| 辨太阳病脉证并治（上） | 3 | 少阳 | shao4 yang2 | context_pronunciation | context_rule_v2 | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | 之，脉若静者，为不传。颇欲吐，若躁烦，脉数急者，为传也。伤寒二三日，阳明少阳证不见者，为不传也。

太阳病，发热而渴，不恶寒者，为温病。若发汗已，身 |
| 辨太阳病脉证并治（上） | 12 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之后，其气上冲者，可与 |
| 辨太阳病脉证并治（中） | 3 | 少阴 | shao4 yin1 | context_pronunciation | context_rule_v2 | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | 之；服之则厥逆，筋惕肉瞤，此为逆也。伤寒脉浮缓，身不疼但重，乍有轻时，无少阴证者，大青龙汤发之。伤寒表不解，心下有水气，干呕，发热而咳，或渴，或利， |
| 辨太阳病脉证并治（中） | 8 | 衄 | nv4 | rare_character | rare_character | useful | 显式 rare_character 信号，即使不在当前 Gold 中也值得人工复核 | ，八九日不解，表证仍在，此当发其汗。服药已微除，其人发烦目瞑，剧者必衄，衄乃解。所以然者，阳气重故也，麻黄汤主之。太阳病，脉浮紧，发热，身无汗，自 |
| 辨太阳病脉证并治（中） | 9 | 衄 | nv4 | rare_character | rare_character | useful | 显式 rare_character 信号，即使不在当前 Gold 中也值得人工复核 | 乃解。所以然者，阳气重故也，麻黄汤主之。太阳病，脉浮紧，发热，身无汗，自衄者愈。二阳并病，太阳初得病时，发其汗，汗先出不彻，因转属阳明，续自微汗出 |
| 辨太阴病脉证并治 | 1 | 长 | zhang3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | ，自利益甚，时腹自痛。若下之，必胸下结硬。太阴中风，四肢烦疼，阳微阴涩而长者，为欲愈。

太阴病欲解时，从亥至丑上。自利不渴者，属太阴，以其脏有寒 |

## review_required Observation

| ID | match | focus | Gold | detected | default | candidates |
|---|---|---|---|---|---|---|
| SHL-G012 | 项背强几几 | 几几 |  | False |  |  |
| SHL-G018 | 肉瞤 | 瞤 |  | False |  |  |
| SHL-G044 | 少腹满 | 少 |  | True | shao3 | shao3 | shao4 |

## Textual Issues

- 正文 / 辨太阳病脉证并治（上）, 辨太阳病脉证并治（中）, 辨阳明病脉证并治, 辨少阳病脉证并治, 辨太阴病脉证并治, 辨少阴病脉证并治, 辨厥阴病脉证并治：当前 production Segmenter 将这些未带序号的篇章标题合并为 1 个 Chapter；本报告使用只读标题投影将其对齐为 7 个 Gold Chapter。 建议：后续单独修复通用篇章标题识别后重新跑纯 production hold-out；本轮不修改 Segmenter。

## Architecture Findings

漏报原因分类（工程归因）：{'medical_term_dictionary_missing': 40, 'rare_character_not_detected': 9, 'pypinyin_single_reading': 1}。Gold occurrence 跨 Segment 边界 0 个；本数据集 supplementary-plane CJK 字符问题记录为 False。这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，本报告只记录词典/语境覆盖、文本底本和误报治理建议，不在 hold-out 数据上实施优化，也不把本次结果替换为 TTS 音频结论。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
