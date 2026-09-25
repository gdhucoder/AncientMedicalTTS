# AncientMedicalTTS《金匮要略》发音评测 holdout

## Executive Summary

本报告评估实际生产 pronunciation analyzer 0.2.1。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：TXT 共 2040 个汉字；production snapshot 实际包含 2029 个汉字。Gold 对齐 5 个 Chapter、52 个 Segment；production 原始结果为 2 个 Chapter、52 个 Segment。
- Hard Gold：53 个 occurrence（45 个条目）。
- Detection Recall：6 / 53 = 11.3%。
- 焦点读音评估：0 / 6 default exact = 0.0%；candidate coverage = 100.0%。
- 漏报 47；wrong default 6；candidate missing 0；extra predictions 12（每千汉字 5.9）。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/jinguiyaolue_holdout_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/jinguiyaolue_gold_v01.json
- review_required：3 个条目、3 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。
- Chapter alignment：{'applied': True, 'source_chapters': 2, 'projected_chapters': 5, 'titles': ['脏腑经络先后病脉证第一', '痉湿暍病脉证并治第二', '百合狐惑阴阳毒病脉证治第三', '疟病脉证并治第四', '中风历节病脉证并治第五'], 'prediction_crossings': 0}
- Git commit：unavailable: workspace is not a git worktree
- Worker Python：3.12.13；pypinyin：0.55.0；benchmark runner：0.3.0-holdout。
- Production resources：见 metrics.json 中的 resource_manifest SHA-256。

## Current Production Analyzer Behavior

当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v0.2.x 使用版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。

## Detection

整体：6 / 53 = 11.3%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_usage | 0 | 3 | 0.0% |
| medical_polyphone | 5 | 10 | 50.0% |
| medical_term | 0 | 27 | 0.0% |
| rare_character | 0 | 4 | 0.0% |
| rare_or_classical | 0 | 7 | 0.0% |
| semantic_polyphone | 1 | 2 | 50.0% |

## Prediction Sources

| source | all predictions | extra predictions |
|---|---:|---:|
| high_risk_polyphone | 16 | 10 |
| medical_lexicon | 2 | 2 |

## Pronunciation

焦点级评估：0 / 6 default exact；6 / 6 candidate contains Gold。完整 match_text 可比较的 occurrence：0，其中 full-span default exact 0。

## Most Important Misses


## Focus Term Checks

| term | Gold status | occurrences | detected | default observations | candidate observations |
|---|---|---:|---:|---|---|
| 脏腑 | gold | 3 | 0 | — | — |
| 中人多死 | gold | 1 | 0 | — | — |
| 疢难 | gold | 1 | 0 | — | — |
| 干忤 | gold | 1 | 0 | — | — |
| 腠理 | gold | 1 | 0 | — | — |
| 喑喑 | gold | 1 | 0 | — | — |
| 啾啾 | gold | 1 | 0 | — | — |
| 肺痿 | gold | 1 | 0 | — | — |
| 微数 | gold | 2 | 0 | — | — |
| 刚痉 | gold | 2 | 0 | — | — |
| 柔痉 | gold | 1 | 0 | — | — |
| 卒口噤 | gold | 1 | 0 | — | — |
| 栝蒌 | gold | 2 | 0 | — | — |
| 挛 | gold | 1 | 0 | — | — |
| 齘齿 | gold | 1 | 0 | — | — |
| 湿痹 | gold | 2 | 0 | — | — |
| 哕 | gold | 1 | 0 | — | — |
| 中暍 | not_listed | 1 | 0 | — | — |
| 芤 | gold | 1 | 0 | — | — |
| 数下之 | gold | 1 | 0 | — | — |
| 其脉微数 | gold | 1 | 0 | — | — |
| 淅然 | gold | 1 | 0 | — | — |
| 头眩 | gold | 1 | 0 | — | — |
| 代赭 | not_listed | 1 | 0 | — | — |
| 狐惑 | gold | 1 | 0 | — | — |
| 眦 | gold | 1 | 0 | — | — |
| 鳖甲 | not_listed | 3 | 0 | — | — |
| 疟脉 | gold | 1 | 0 | — | — |
| 弦数者 | gold | 2 | 0 | — | — |
| 不差 | gold | 3 | 0 | — | — |
| 癥瘕 | gold | 1 | 0 | — | — |
| 疟母 | gold | 1 | 0 | — | — |
| 消铄 | gold | 1 | 0 | — | — |
| 牡疟 | gold | 1 | 0 | — | — |
| 中风 | gold | 2 | 0 | — | — |
| 脉微而数 | gold | 1 | 0 | — | — |
| 㖞 | gold | 1 | 0 | — | — |
| 瘾疹 | gold | 1 | 0 | — | — |

Extra risk_type 分布：medical_term=2, polyphone=10。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| JGYL-G001 | 脏腑经络先后病脉证第一 | 脏腑 / 脏 | zang4 fu3 | medical_polyphone | medical_term_dictionary_missing |
| JGYL-G001 | 脏腑经络先后病脉证第一 | 脏腑 / 脏 | zang4 fu3 | medical_polyphone | medical_term_dictionary_missing |
| JGYL-G001 | 脏腑经络先后病脉证第一 | 脏腑 / 脏 | zang4 fu3 | medical_polyphone | medical_term_dictionary_missing |
| JGYL-G002 | 脏腑经络先后病脉证第一 | 中人多死 / 中 | zhong4 ren2 duo1 si3 | semantic_polyphone | classical_usage_not_detected |
| JGYL-G003 | 脏腑经络先后病脉证第一 | 疢难 / 疢 | chen4 nan4 | rare_or_classical | rare_character_not_detected |
| JGYL-G004 | 脏腑经络先后病脉证第一 | 干忤 / 忤 | gan1 wu3 | rare_or_classical | rare_character_not_detected |
| JGYL-G005 | 脏腑经络先后病脉证第一 | 腠理 / 腠 | cou4 li3 | medical_term | medical_term_dictionary_missing |
| JGYL-G006 | 脏腑经络先后病脉证第一 | 喑喑然 / 喑喑 | yin1 yin1 ran2 | rare_or_classical | rare_character_not_detected |
| JGYL-G007 | 脏腑经络先后病脉证第一 | 啾啾然 / 啾啾 | jiu1 jiu1 ran2 | rare_or_classical | rare_character_not_detected |
| JGYL-G008 | 脏腑经络先后病脉证第一 | 肺痿 / 痿 | fei4 wei3 | medical_term | medical_term_dictionary_missing |
| JGYL-G010 | 脏腑经络先后病脉证第一 | 寸口脉 / 寸口 | cun4 kou3 mai4 | medical_term | medical_term_dictionary_missing |
| JGYL-G011 | 痉湿暍病脉证并治第二 | 刚痉 / 痉 | gang1 jing4 | medical_term | medical_term_dictionary_missing |
| JGYL-G011 | 痉湿暍病脉证并治第二 | 刚痉 / 痉 | gang1 jing4 | medical_term | medical_term_dictionary_missing |
| JGYL-G012 | 痉湿暍病脉证并治第二 | 柔痉 / 痉 | rou2 jing4 | medical_term | medical_term_dictionary_missing |
| JGYL-G013 | 痉湿暍病脉证并治第二 | 卒口噤 / 卒 | cu4 kou3 jin4 | classical_usage | classical_usage_not_detected |
| JGYL-G015 | 痉湿暍病脉证并治第二 | 栝蒌桂枝汤 / 栝蒌 | gua1 lou2 gui4 zhi1 tang1 | medical_term | medical_term_dictionary_missing |
| JGYL-G016 | 痉湿暍病脉证并治第二 | 脚挛急 / 挛 | jiao3 luan2 ji2 | medical_term | medical_term_dictionary_missing |
| JGYL-G017 | 痉湿暍病脉证并治第二 | 齘齿 / 齘 | xie4 chi3 | rare_character | rare_character_not_detected |
| JGYL-G018 | 痉湿暍病脉证并治第二 | 湿痹 / 痹 | shi1 bi4 | medical_term | medical_term_dictionary_missing |
| JGYL-G018 | 痉湿暍病脉证并治第二 | 湿痹 / 痹 | shi1 bi4 | medical_term | medical_term_dictionary_missing |

## Wrong Defaults

| ID | occurrence | focus | Gold focus | system default | candidates |
|---|---:|---|---|---|---|
| JGYL-G009 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| JGYL-G022 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| JGYL-G023 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| JGYL-G035 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| JGYL-G035 | 2 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| JGYL-G045 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |

## Extra Predictions

共 12 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。已对全部条目按通用审计规则分类：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 12}。分类只用于复核，不改变硬指标；完整结果见 extra_predictions.csv，下面展示前 12 条。

| chapter | segment | surface | default | risk | source | classification | reason | context |
|---|---:|---|---|---|---|---|---|---|
| 脏腑经络先后病脉证第一 | 4 | 长 | zhang3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 肝气盛。故实脾，则肝自愈，此治肝补脾之要妙也。

夫人禀五常，因风气而生长。风气虽能生万物，亦能害万物，如水能浮舟，亦能覆舟。若五脏元真通畅，人即 |
| 脏腑经络先后病脉证第一 | 7 | 九窍 | jiu3 qiao4 | medical_term | medical_lexicon | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 多死。千般疢难，不越三条：一者，经络受邪，入脏腑，为内所因也；二者，四肢九窍，血脉相传，壅塞不通，为外皮肤所中也；三者，房室、金刃、虫兽所伤。以此详 |
| 脏腑经络先后病脉证第一 | 8 | 九窍 | jiu3 qiao4 | medical_term | medical_lexicon | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 经络，未流传脏腑，即医治之；四肢才觉重滞，即导引、吐纳、针灸、膏摩，勿令九窍闭塞；更能无犯王法、禽兽灾伤；房室勿令竭乏；服食节其冷热苦酸辛甘，不遗形 |
| 脏腑经络先后病脉证第一 | 8 | 更 | geng4 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 传脏腑，即医治之；四肢才觉重滞，即导引、吐纳、针灸、膏摩，勿令九窍闭塞；更能无犯王法、禽兽灾伤；房室勿令竭乏；服食节其冷热苦酸辛甘，不遗形体有衰， |
| 脏腑经络先后病脉证第一 | 11 | 长 | zhang3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 人语声寂然喜惊呼者，骨节间病；语声喑喑然不彻者，心膈间病；语声啾啾然细而长者，头中病。一作痛。

师曰：息摇肩者，心中坚；息引胸中上气者，咳；息张 |
| 痉湿暍病脉证并治第二 | 2 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 复发汗，必拘急。

疮家虽身疼痛，不可发汗，汗出则痉。病者身热足寒，颈项强急，恶寒，时头热，面赤，目赤，独头动摇，卒口噤，背反张者，痉病也。若发其 |
| 痉湿暍病脉证并治第二 | 4 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 夫痉脉，按之紧如弦，直上下行。痉病有灸疮，难治。

太阳病，其证备，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无汗而小便反少，气上 |
| 痉湿暍病脉证并治第二 | 5 | 少 | shao3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | ，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无汗而小便反少，气上冲胸，口噤不得语，欲作刚痉，葛根汤主之。痉为病，胸满口噤，卧不着席 |
| 痉湿暍病脉证并治第二 | 10 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 其小便。湿家之为病，一身尽疼，发热，身色如熏黄也。湿家，其人但头汗出，背强，欲得被覆向火。若下之早则哕，或胸满，小便不利，舌上如胎者，以丹田有热， |
| 百合狐惑阴阳毒病脉证治第三 | 8 | 数 | shu4 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | ，甘草泻心汤主之；蚀于下部则咽干，苦参汤洗之；蚀于肛者，雄黄熏之。病者脉数，无热微烦，默默但欲卧，汗出，初得之三四日，目赤如鸠眼；七八日，目四眦黑 |
| 疟病脉证并治第四 | 3 | 少 | shao3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 结为癥瘕，名曰疟母，急治之，宜鳖甲煎丸。师曰：阴气孤绝，阳气独发，则热而少气烦冤，手足热而欲呕，名曰瘅疟。若但热不寒者，邪气内藏于心，外舍分肉之间 |
| 疟病脉证并治第四 | 4 | 藏 | cang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | ，阳气独发，则热而少气烦冤，手足热而欲呕，名曰瘅疟。若但热不寒者，邪气内藏于心，外舍分肉之间，令人消铄脱肉。温疟者，其脉如平，身无寒但热，骨节疼烦 |

## review_required Observation

| ID | match | focus | Gold | detected | default | candidates |
|---|---|---|---|---|---|---|
| JGYL-G014 | 身体强，几几然 | 几几 |  | False |  |  |
| JGYL-G030 | 声喝 | 喝 |  | False |  |  |
| JGYL-G039 | 瘅疟 | 瘅疟 |  | False |  |  |

## Textual Issues

- 正文 / 脏腑经络先后病脉证第一, 痉湿暍病脉证并治第二, 百合狐惑阴阳毒病脉证治第三, 疟病脉证并治第四, 中风历节病脉证并治第五：当前 production Segmenter 将这些未带序号的篇章标题合并为 2 个 Chapter；本报告使用只读标题投影将其对齐为 5 个 Gold Chapter。 建议：后续单独修复通用篇章标题识别后重新跑纯 production hold-out；本轮不修改 Segmenter。
-  / jinguiyaolue_gold_v01_README.md：总 manifest 引用了该 README，但当前工作区未找到文件；不影响 JSON Gold 计算。 建议：补齐数据包 README 后再归档 hold-out 结果。

## Architecture Findings

漏报原因分类（工程归因）：{'medical_term_dictionary_missing': 32, 'classical_usage_not_detected': 4, 'rare_character_not_detected': 11}。Gold occurrence 跨 Segment 边界 0 个；本数据集 supplementary-plane CJK 字符问题记录为 False。这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，本报告只记录词典/语境覆盖、文本底本和误报治理建议，不在 hold-out 数据上实施优化，也不把本次结果替换为 TTS 音频结论。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
