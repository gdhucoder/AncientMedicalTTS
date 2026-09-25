# AncientMedicalTTS《金匮要略》发音评测 m9

## Executive Summary

本报告评估实际生产 pronunciation analyzer 0.3.0。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：TXT 共 2040 个汉字；production snapshot 实际包含 2029 个汉字。Gold 对齐 5 个 Chapter、52 个 Segment；production 原始结果为 2 个 Chapter、52 个 Segment。
- Hard Gold：53 个 occurrence（45 个条目）。
- Detection Recall：50 / 53 = 94.3%。
- 焦点读音评估：50 / 50 default exact = 100.0%；candidate coverage = 100.0%。
- 漏报 3；wrong default 0；candidate missing 0；extra predictions 35（每千汉字 17.2）。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/jinguiyaolue_holdout_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/jinguiyaolue_gold_v01.json
- review_required：3 个条目、3 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。
- Chapter alignment：{'applied': True, 'source_chapters': 2, 'projected_chapters': 5, 'titles': ['脏腑经络先后病脉证第一', '痉湿暍病脉证并治第二', '百合狐惑阴阳毒病脉证治第三', '疟病脉证并治第四', '中风历节病脉证并治第五'], 'prediction_crossings': 0}
- Git commit：unavailable: workspace is not a git worktree
- Worker Python：3.12.13；pypinyin：0.55.0；benchmark runner：0.4.0-m9。
- Production resources：见 metrics.json 中的 resource_manifest SHA-256。

## Current Production Analyzer Behavior

当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v0.3.0 使用版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。

## Detection

整体：50 / 53 = 94.3%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_usage | 3 | 3 | 100.0% |
| medical_polyphone | 10 | 10 | 100.0% |
| medical_term | 24 | 27 | 88.9% |
| rare_character | 4 | 4 | 100.0% |
| rare_or_classical | 7 | 7 | 100.0% |
| semantic_polyphone | 2 | 2 | 100.0% |

## Prediction Sources

| source | all predictions | correct Gold hits | extra predictions |
|---|---:|---:|---:|
| context_semantic | 13 | 7 | 6 |
| high_risk_polyphone | 6 | 0 | 6 |
| medical_lexicon_v3 | 53 | 30 | 22 |
| rare_classical_lexicon | 14 | 13 | 1 |

## Pronunciation

焦点级评估：50 / 50 default exact；50 / 50 candidate contains Gold。完整 match_text 可比较的 occurrence：38，其中 full-span default exact 38。

## Most Important Misses


## Focus Term Checks

| term | Gold status | occurrences | detected | default observations | candidate observations |
|---|---|---:|---:|---|---|
| 脏腑 | gold | 3 | 3 | zang4 fu3 | zang1 fu3 | zang4 fu3 |
| 中人多死 | gold | 1 | 0 | — | — |
| 疢难 | gold | 1 | 1 | chen4 nan4 | chen4 nan2 | chen4 nan4 |
| 干忤 | gold | 1 | 1 | gan1 wu3 | an4 wu3 | an4 wu4 | gan1 wu3 | gan1 wu4 | gan4 wu3 | gan4 wu4 |
| 腠理 | gold | 1 | 1 | cou4 li3 | cou4 li3 |
| 喑喑 | gold | 1 | 1 | yin1 yin1 | yin1 yin1 | yin1 yin3 | yin1 yin4 | yin3 yin1 | yin3 yin3 | yin3 yin4 | yin4 yin1 | yin4 yin3 | yin4 yin4 |
| 啾啾 | gold | 1 | 1 | jiu1 jiu1 | jiu1 jiu1 |
| 肺痿 | gold | 1 | 1 | fei4 wei3 | fei4 wei3 | pei4 wei3 |
| 微数 | gold | 2 | 0 | — | — |
| 刚痉 | gold | 2 | 2 | gang1 jing4 | gang1 jing4 |
| 柔痉 | gold | 1 | 1 | rou2 jing4 | rou2 jing4 |
| 卒口噤 | gold | 1 | 1 | cu4 kou3 jin4 | cu4 kou3 jin4 | cui4 kou3 jin4 | zu2 kou3 jin4 |
| 栝蒌 | gold | 2 | 2 | gua1 lou2 | gua1 lou2 | kuo4 lou2 | tian3 lou2 |
| 挛 | gold | 1 | 1 | luan2 | luan2 |
| 齘齿 | gold | 1 | 1 | xie4 chi3 | xie4 chi3 |
| 湿痹 | gold | 2 | 2 | shi1 bi4 | shi1 bi4 |
| 哕 | gold | 1 | 1 | yue3 | hui4 | yue3 |
| 中暍 | not_listed | 1 | 1 | zhong4 ye1 | zhong1 ye1 | zhong4 ye1 |
| 芤 | gold | 1 | 1 | kou1 | kou1 |
| 数下之 | gold | 1 | 0 | — | — |
| 其脉微数 | gold | 1 | 0 | — | — |
| 淅然 | gold | 1 | 1 | xi1 ran2 | xi1 ran2 |
| 头眩 | gold | 1 | 1 | tou2 xuan4 | tou2 huan4 | tou2 juan4 | tou2 xuan4 |
| 代赭 | not_listed | 1 | 1 | dai4 zhe3 | dai4 zhe3 |
| 狐惑 | gold | 1 | 1 | hu2 huo4 | hu2 huo4 |
| 眦 | gold | 1 | 1 | zi4 | zi4 |
| 鳖甲 | not_listed | 3 | 3 | bie1 jia3 | bie1 jia3 |
| 疟脉 | gold | 1 | 1 | nve4 mai4 | nve4 mai4 | nve4 mo4 | yao4 mai4 | yao4 mo4 |
| 弦数者 | gold | 2 | 0 | — | — |
| 不差 | gold | 3 | 3 | bu4 chai4 | bu2 cha1 | bu2 cha4 | bu2 chai1 | bu2 chai4 | bu2 ci1 | bu2 cuo1 | bu2 jie1 | bu4 cha1 | bu4 cha4 | bu4 chai1 | bu4 chai4 | bu4 ci1 | bu4 cuo1 | bu4 jie1 | fou1 cha1 | fou1 cha4 | fou1 chai1 | fou1 chai4 | fou1 ci1 | fou1 cuo1 | fou1 jie1 | fou3 cha1 | fou3 cha4 | fou3 chai1 | fou3 chai4 | fou3 ci1 | fou3 cuo1 | fou3 jie1 | fu1 cha1 | fu1 cha4 | fu1 chai1 | fu1 chai4 | fu1 ci1 | fu1 cuo1 | fu1 jie1 |
| 癥瘕 | gold | 1 | 1 | zheng1 jia3 | zheng1 jia3 | zheng1 xia1 |
| 疟母 | gold | 1 | 1 | nve4 mu3 | nve4 mu2 | nve4 mu3 | nve4 wu2 | nve4 wu3 | yao4 mu2 | yao4 mu3 | yao4 wu2 | yao4 wu3 |
| 消铄 | gold | 1 | 1 | xiao1 shuo4 | xiao1 shuo4 |
| 牡疟 | gold | 1 | 1 | mu3 nve4 | mu3 nve4 | mu3 yao4 |
| 中风 | gold | 2 | 2 | zhong4 feng1 | zhong1 feng1 | zhong4 feng1 |
| 脉微而数 | gold | 1 | 0 | — | — |
| 㖞 | gold | 1 | 1 | wai1 | wai1 |
| 瘾疹 | gold | 1 | 1 | yin3 zhen3 | yin3 chen4 | yin3 zhen3 |

Extra risk_type 分布：classical_term=1, context_pronunciation=6, medical_term=22, polyphone=6。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| JGYL-G029 | 百合狐惑阴阳毒病脉证治第三 | 蚀于喉 / 蚀 | shi2 yu2 hou2 | medical_term | medical_term_dictionary_missing |
| JGYL-G032 | 百合狐惑阴阳毒病脉证治第三 | 脓已成 / 脓 | nong2 yi3 cheng2 | medical_term | medical_term_dictionary_missing |
| JGYL-G044 | 中风历节病脉证并治第五 | 痹 / 痹 | bi4 | medical_term | medical_term_dictionary_missing |

## Wrong Defaults

没有被评估 occurrence 出现 default mismatch。

## Extra Predictions

共 35 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。已对全部条目按通用审计规则分类：{'bug': 0, 'unclear': 0, 'unnecessary': 0, 'useful': 35}。分类只用于复核，不改变硬指标；完整结果见 extra_predictions.csv，下面展示前 20 条。

| chapter | segment | surface | default | risk | source | classification | reason | context |
|---|---:|---|---|---|---|---|---|---|
| 脏腑经络先后病脉证第一 | 4 | 长 | zhang3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 肝气盛。故实脾，则肝自愈，此治肝补脾之要妙也。

夫人禀五常，因风气而生长。风气虽能生万物，亦能害万物，如水能浮舟，亦能覆舟。若五脏元真通畅，人即 |
| 脏腑经络先后病脉证第一 | 7 | 九窍 | jiu3 qiao4 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 多死。千般疢难，不越三条：一者，经络受邪，入脏腑，为内所因也；二者，四肢九窍，血脉相传，壅塞不通，为外皮肤所中也；三者，房室、金刃、虫兽所伤。以此详 |
| 脏腑经络先后病脉证第一 | 8 | 九窍 | jiu3 qiao4 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 经络，未流传脏腑，即医治之；四肢才觉重滞，即导引、吐纳、针灸、膏摩，勿令九窍闭塞；更能无犯王法、禽兽灾伤；房室勿令竭乏；服食节其冷热苦酸辛甘，不遗形 |
| 脏腑经络先后病脉证第一 | 8 | 更 | geng4 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 传脏腑，即医治之；四肢才觉重滞，即导引、吐纳、针灸、膏摩，勿令九窍闭塞；更能无犯王法、禽兽灾伤；房室勿令竭乏；服食节其冷热苦酸辛甘，不遗形体有衰， |
| 脏腑经络先后病脉证第一 | 11 | 长 | zhang3 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 人语声寂然喜惊呼者，骨节间病；语声喑喑然不彻者，心膈间病；语声啾啾然细而长者，头中病。一作痛。

师曰：息摇肩者，心中坚；息引胸中上气者，咳；息张 |
| 脏腑经络先后病脉证第一 | 12 | 中 | zhong1 | context_pronunciation | context_semantic | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | 者，心膈间病；语声啾啾然细而长者，头中病。一作痛。

师曰：息摇肩者，心中坚；息引胸中上气者，咳；息张口短气者，肺痿唾沫。师曰：吸而微数，其病在中 |
| 脏腑经络先后病脉证第一 | 13 | 中 | zhong1 | context_pronunciation | context_semantic | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | 中坚；息引胸中上气者，咳；息张口短气者，肺痿唾沫。师曰：吸而微数，其病在中焦，实也，当下之即愈；虚者不治。在上焦者，其吸促；在下焦者，其吸远，此皆 |
| 痉湿暍病脉证并治第二 | 0 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 太阳病，发热无汗，反恶寒者，名曰刚痉。

太阳病，发热汗出，而不恶寒，名曰柔 |
| 痉湿暍病脉证并治第二 | 0 | 恶寒 | wu4 han2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 太阳病，发热无汗，反恶寒者，名曰刚痉。

太阳病，发热汗出，而不恶寒，名曰柔痉。太阳病，发热，脉 |
| 痉湿暍病脉证并治第二 | 0 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 太阳病，发热无汗，反恶寒者，名曰刚痉。

太阳病，发热汗出，而不恶寒，名曰柔痉。太阳病，发热，脉沉而细者，名曰痉，为难 |
| 痉湿暍病脉证并治第二 | 0 | 恶寒 | wu4 han2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 太阳病，发热无汗，反恶寒者，名曰刚痉。

太阳病，发热汗出，而不恶寒，名曰柔痉。太阳病，发热，脉沉而细者，名曰痉，为难治。

太阳病，发汗太 |
| 痉湿暍病脉证并治第二 | 1 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 发热无汗，反恶寒者，名曰刚痉。

太阳病，发热汗出，而不恶寒，名曰柔痉。太阳病，发热，脉沉而细者，名曰痉，为难治。

太阳病，发汗太多，因致痉。

 |
| 痉湿暍病脉证并治第二 | 1 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 汗出，而不恶寒，名曰柔痉。太阳病，发热，脉沉而细者，名曰痉，为难治。

太阳病，发汗太多，因致痉。

夫风病，下之则痉，复发汗，必拘急。

疮家虽身 |
| 痉湿暍病脉证并治第二 | 2 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 复发汗，必拘急。

疮家虽身疼痛，不可发汗，汗出则痉。病者身热足寒，颈项强急，恶寒，时头热，面赤，目赤，独头动摇，卒口噤，背反张者，痉病也。若发其 |
| 痉湿暍病脉证并治第二 | 2 | 恶寒 | wu4 han2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ，必拘急。

疮家虽身疼痛，不可发汗，汗出则痉。病者身热足寒，颈项强急，恶寒，时头热，面赤，目赤，独头动摇，卒口噤，背反张者，痉病也。若发其汗者，寒 |
| 痉湿暍病脉证并治第二 | 3 | 恶寒 | wu4 han2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ，独头动摇，卒口噤，背反张者，痉病也。若发其汗者，寒湿相得，其表益虚，即恶寒甚。发其汗已，其脉如蛇。

暴腹胀大者，为欲解。脉如故，反伏弦者，痉。
 |
| 痉湿暍病脉证并治第二 | 4 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | ，反伏弦者，痉。

夫痉脉，按之紧如弦，直上下行。痉病有灸疮，难治。

太阳病，其证备，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无 |
| 痉湿暍病脉证并治第二 | 4 | 强 | qiang2 | polyphone | high_risk_polyphone | useful | 命中版本化高风险多音字表，属于可解释的复核提示 | 夫痉脉，按之紧如弦，直上下行。痉病有灸疮，难治。

太阳病，其证备，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无汗而小便反少，气上 |
| 痉湿暍病脉证并治第二 | 5 | 太阳 | tai4 yang2 | medical_term | medical_lexicon_v3 | useful | 显式 medical_term 信号，即使不在当前 Gold 中也值得人工复核 | 。

太阳病，其证备，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无汗而小便反少，气上冲胸，口噤不得语，欲作刚痉，葛根汤主之。痉为病， |
| 痉湿暍病脉证并治第二 | 5 | 少 | shao3 | context_pronunciation | context_semantic | useful | 显式 context_pronunciation 信号，即使不在当前 Gold 中也值得人工复核 | ，身体强，几几然，脉反沉迟，此为痉，栝蒌桂枝汤主之。太阳病，无汗而小便反少，气上冲胸，口噤不得语，欲作刚痉，葛根汤主之。痉为病，胸满口噤，卧不着席 |

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

漏报原因分类（工程归因）：{'medical_term_dictionary_missing': 3}。Gold occurrence 跨 Segment 边界 0 个；本数据集 supplementary-plane CJK 字符问题记录为 False。这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，本报告只记录词典/语境覆盖、文本底本和误报治理建议，不在 hold-out 数据上实施优化，也不把本次结果替换为 TTS 音频结论。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
