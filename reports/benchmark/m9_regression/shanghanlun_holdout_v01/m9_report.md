# AncientMedicalTTS《伤寒论》发音评测 m9

## Executive Summary

本报告评估实际生产 pronunciation analyzer 0.3.0。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：TXT 共 1735 个汉字；production snapshot 实际包含 1735 个汉字。Gold 对齐 7 个 Chapter、51 个 Segment；production 原始结果为 1 个 Chapter、48 个 Segment。
- Hard Gold：68 个 occurrence（43 个条目）。
- Detection Recall：62 / 68 = 91.2%。
- 焦点读音评估：62 / 62 default exact = 100.0%；candidate coverage = 100.0%。
- 漏报 6；wrong default 0；candidate missing 0；extra predictions 69（每千汉字 39.8）。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/shanghanlun_holdout_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/shanghanlun_gold_v01.json
- review_required：3 个条目、3 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。
- Chapter alignment：{'applied': True, 'source_chapters': 1, 'projected_chapters': 7, 'titles': ['辨太阳病脉证并治（上）', '辨太阳病脉证并治（中）', '辨阳明病脉证并治', '辨少阳病脉证并治', '辨太阴病脉证并治', '辨少阴病脉证并治', '辨厥阴病脉证并治'], 'prediction_crossings': 0}
- Git commit：unavailable: workspace is not a git worktree
- Worker Python：3.12.13；pypinyin：0.55.0；benchmark runner：0.4.0-m9。
- Production resources：见 metrics.json 中的 resource_manifest SHA-256。

## Current Production Analyzer Behavior

当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v0.3.0 使用版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。

## Detection

整体：62 / 68 = 91.2%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_term | 1 | 1 | 100.0% |
| medical_polyphone | 22 | 23 | 95.7% |
| medical_term | 28 | 32 | 87.5% |
| rare_or_classical | 8 | 9 | 88.9% |
| semantic_polyphone | 3 | 3 | 100.0% |

## Prediction Sources

| source | all predictions | correct Gold hits | extra predictions |
|---|---:|---:|---:|
| context_exact | 14 | 12 | 2 |
| context_semantic | 6 | 5 | 1 |
| high_risk_polyphone | 4 | 0 | 3 |
| medical_lexicon_v3 | 94 | 33 | 61 |
| rare_classical_lexicon | 14 | 12 | 2 |

## Pronunciation

焦点级评估：62 / 62 default exact；62 / 62 candidate contains Gold。完整 match_text 可比较的 occurrence：53，其中 full-span default exact 53。

## Most Important Misses


## Focus Term Checks

| term | Gold status | occurrences | detected | default observations | candidate observations |
|---|---|---:|---:|---|---|
| 恶寒 | gold | 15 | 15 | wu4 han2 | e3 han2 | e4 han2 | wu1 han2 | wu4 han2 |
| 中风 | gold | 6 | 6 | zhong4 feng1 | zhong1 feng1 | zhong4 feng1 |
| 脉数急 | gold | 1 | 0 | — | — |
| 瘈疭 | gold | 1 | 1 | chi4 zong4 | chi4 zong4 | zhi4 zong4 |
| 阳数七 | gold | 1 | 0 | — | — |
| 阴数六 | gold | 1 | 0 | — | — |
| 啬啬 | gold | 1 | 1 | se4 se4 | se4 se4 |
| 淅淅 | gold | 1 | 1 | xi1 xi1 | xi1 xi1 |
| 翕翕 | gold | 1 | 1 | xi1 xi1 | xi1 xi1 |
| 厚朴 | gold | 2 | 2 | hou4 po4 | hou4 piao2 | hou4 po1 | hou4 po4 | hou4 pu1 | hou4 pu3 |
| 厥逆 | gold | 1 | 1 | jue2 ni4 | jue2 ni4 |
| 肉瞤 | review_required | 1 | 0 | — | — |
| 目瞑 | gold | 1 | 1 | mu4 ming2 | mu4 meng2 | mu4 mian2 | mu4 ming2 |
| 衄 | gold | 3 | 3 | nv4 | nv4 |
| 不更衣 | gold | 1 | 0 | — | — |
| 哕 | gold | 2 | 2 | yue3 | hui4 | yue3 |
| 濈然 | not_listed | 1 | 1 | ji2 ran2 | ji2 ran2 | sha4 ran2 |
| 少阳 | gold | 6 | 6 | shao4 yang2 | shao3 yang2 | shao4 yang2 |
| 谵语 | gold | 1 | 1 | zhan1 yu3 | zhan1 yu3 | zhan1 yu4 |
| 胁下硬满 | gold | 1 | 1 | xie2 xia4 ying4 man3 | xie2 xia4 geng3 man3 | xie2 xia4 ying4 man3 |
| 太阴 | gold | 4 | 4 | tai4 yin1 | ta1 yin1 | tai4 yin1 |
| 少阴 | gold | 8 | 8 | shao4 yin1 | shao3 yin1 | shao4 yin1 |
| 脉细沉数 | gold | 1 | 0 | — | — |
| 厥阴 | gold | 3 | 3 | jue2 yin1 | jue2 yin1 |
| 吐蛔 | gold | 1 | 1 | tu3 hui2 | tu3 hui2 | tu4 hui2 |
| 黄芩 | not_listed | 2 | 2 | huang2 qin2 | huang2 qin2 | huang2 yin2 |
| 除中 | gold | 2 | 2 | chu2 zhong1 | chu2 zhong1 | chu2 zhong4 | shu1 zhong1 | shu1 zhong4 | zhu4 zhong1 | zhu4 zhong4 |
| 索饼 | gold | 1 | 1 | suo3 bing3 | suo3 bing3 |
| 几几 | review_required | 1 | 0 | — | — |
| 少腹 | not_listed | 1 | 0 | — | — |

Extra risk_type 分布：classical_term=2, context_pronunciation=3, medical_term=61, polyphone=3。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| SHL-G014 | 辨太阳病脉证并治（上） | 温针 / 针 | wen1 zhen1 | medical_term | medical_term_dictionary_missing |
| SHL-G026 | 辨少阳病脉证并治 | 悸而惊 / 悸 | ji4 er2 jing1 | medical_term | medical_term_dictionary_missing |
| SHL-G030 | 辨太阴病脉证并治 | 四逆辈 / 逆 | si4 ni4 bei4 | medical_term | medical_term_dictionary_missing |
| SHL-G031 | 辨太阴病脉证并治 | 其脏有寒 / 脏 | qi2 zang4 you3 han2 | medical_polyphone | medical_term_dictionary_missing |
| SHL-G034 | 辨少阴病脉证并治 | 下焦 / 焦 | xia4 jiao1 | medical_term | medical_term_dictionary_missing |
| SHL-G043 | 辨太阳病脉证并治（中） | 乍有轻时 / 乍 | zha4 you3 qing1 shi2 | rare_or_classical | rare_character_not_detected |

## Wrong Defaults

没有被评估 occurrence 出现 default mismatch。

## Extra Predictions

共 69 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。已对全部条目按通用审计规则分类：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 69}。分类只用于复核，不改变硬指标；完整结果见 extra_predictions.csv，下面展示前 20 条。

| chapter | segment | surface | default | risk | source | classification | reason | context |
|---|---:|---|---|---|---|---|---|---|
| 辨太阳病脉证并治（上） | 0 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 

太阳之为病，脉浮，头项强痛而恶寒。太阳病，发热，汗出，恶风，脉缓者，名为中风 |
| 辨太阳病脉证并治（上） | 0 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 

太阳之为病，脉浮，头项强痛而恶寒。太阳病，发热，汗出，恶风，脉缓者，名为中风。太阳病，或已发热， |
| 辨太阳病脉证并治（上） | 1 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 

太阳之为病，脉浮，头项强痛而恶寒。太阳病，发热，汗出，恶风，脉缓者，名为中风。太阳病，或已发热，或未发热，必恶 |
| 辨太阳病脉证并治（上） | 2 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 为病，脉浮，头项强痛而恶寒。太阳病，发热，汗出，恶风，脉缓者，名为中风。太阳病，或已发热，或未发热，必恶寒，体痛，呕逆，脉阴阳俱紧者，名曰伤寒。

 |
| 辨太阳病脉证并治（上） | 2 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 热，或未发热，必恶寒，体痛，呕逆，脉阴阳俱紧者，名曰伤寒。

伤寒一日，太阳受之，脉若静者，为不传。颇欲吐，若躁烦，脉数急者，为传也。伤寒二三日，阳 |
| 辨太阳病脉证并治（上） | 3 | 阳明 | yang2 ming2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 阳受之，脉若静者，为不传。颇欲吐，若躁烦，脉数急者，为传也。伤寒二三日，阳明少阳证不见者，为不传也。

太阳病，发热而渴，不恶寒者，为温病。若发汗已 |
| 辨太阳病脉证并治（上） | 3 | 少阳 | shao4 yang2 | context_pronunciation | context_exact | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | 之，脉若静者，为不传。颇欲吐，若躁烦，脉数急者，为传也。伤寒二三日，阳明少阳证不见者，为不传也。

太阳病，发热而渴，不恶寒者，为温病。若发汗已，身 |
| 辨太阳病脉证并治（上） | 3 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ，若躁烦，脉数急者，为传也。伤寒二三日，阳明少阳证不见者，为不传也。

太阳病，发热而渴，不恶寒者，为温病。若发汗已，身灼热者，名曰风温。风温为病， |
| 辨太阳病脉证并治（上） | 7 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 无热恶寒者，发于阴也。发于阳七日愈，发于阴六日愈，以阳数七，阴数六故也。太阳病，头痛至七日以上自愈者，以行其经尽故也。若欲作再经者，针足阳明，使经不 |
| 辨太阳病脉证并治（上） | 7 | 阳明 | yang2 ming2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 六故也。太阳病，头痛至七日以上自愈者，以行其经尽故也。若欲作再经者，针足阳明，使经不传则愈。

太阳病欲解时，从巳至未上。

风家，表解而不了了者， |
| 辨太阳病脉证并治（上） | 7 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 日以上自愈者，以行其经尽故也。若欲作再经者，针足阳明，使经不传则愈。

太阳病欲解时，从巳至未上。

风家，表解而不了了者，十二日愈。病人身大热，反 |
| 辨太阳病脉证并治（上） | 9 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ，热在皮肤，寒在骨髓也；身大寒，反不欲近衣者，寒在皮肤，热在骨髓也。

太阳中风，阳浮而阴弱。阳浮者，热自发；阴弱者，汗自出。啬啬恶寒，淅淅恶风，翕 |
| 辨太阳病脉证并治（上） | 10 | 桂枝汤 | gui4 zhi1 tang1 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 浮者，热自发；阴弱者，汗自出。啬啬恶寒，淅淅恶风，翕翕发热，鼻鸣干呕者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者， |
| 辨太阳病脉证并治（上） | 11 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ；阴弱者，汗自出。啬啬恶寒，淅淅恶风，翕翕发热，鼻鸣干呕者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风 |
| 辨太阳病脉证并治（上） | 11 | 桂枝汤 | gui4 zhi1 tang1 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 恶风，翕翕发热，鼻鸣干呕者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之 |
| 辨太阳病脉证并治（上） | 12 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 热，鼻鸣干呕者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之后，其气上 |
| 辨太阳病脉证并治（上） | 12 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 者，桂枝汤主之。太阳病，头痛，发热，汗出，恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之后，其气上冲者，可与 |
| 辨太阳病脉证并治（上） | 13 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 恶风，桂枝汤主之。太阳病，项背强几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之后，其气上冲者，可与桂枝汤，方用前法；若不上冲者，不得与之。太阳 |
| 辨太阳病脉证并治（上） | 13 | 桂枝汤 | gui4 zhi1 tang1 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 几几者，反汗出恶风者，桂枝加葛根汤主之。太阳病，下之后，其气上冲者，可与桂枝汤，方用前法；若不上冲者，不得与之。太阳病三日，已发汗，若吐、若下、若温针 |
| 辨太阳病脉证并治（上） | 14 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 太阳病，下之后，其气上冲者，可与桂枝汤，方用前法；若不上冲者，不得与之。太阳病三日，已发汗，若吐、若下、若温针，仍不解者，此为坏病，桂枝不中与之也。 |

## review_required Observation

| ID | match | focus | Gold | detected | default | candidates |
|---|---|---|---|---|---|---|
| SHL-G012 | 项背强几几 | 几几 |  | False |  |  |
| SHL-G018 | 肉瞤 | 瞤 |  | False |  |  |
| SHL-G044 | 少腹满 | 少 |  | True | shao3 | shao3 | shao4 |

## Textual Issues

- 正文 / 辨太阳病脉证并治（上）, 辨太阳病脉证并治（中）, 辨阳明病脉证并治, 辨少阳病脉证并治, 辨太阴病脉证并治, 辨少阴病脉证并治, 辨厥阴病脉证并治：当前 production Segmenter 将这些未带序号的篇章标题合并为 1 个 Chapter；本报告使用只读标题投影将其对齐为 7 个 Gold Chapter。 建议：后续单独修复通用篇章标题识别后重新跑纯 production hold-out；本轮不修改 Segmenter。

## Architecture Findings

漏报原因分类（工程归因）：{'medical_term_dictionary_missing': 5, 'rare_character_not_detected': 1}。Gold occurrence 跨 Segment 边界 0 个；本数据集 supplementary-plane CJK 字符问题记录为 False。这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，本报告只记录词典/语境覆盖、文本底本和误报治理建议，不在 hold-out 数据上实施优化，也不把本次结果替换为 TTS 音频结论。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
