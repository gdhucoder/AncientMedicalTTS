# AncientMedicalTTS《黄帝内经·素问》发音基线评测 v0.1

## Executive Summary

本报告只评估文本侧 pronunciation analyzer。没有导入 Gold 规则、没有修改 medical_terms/pypinyin/analyzer，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：3273 个汉字；4 个 Chapter；85 个 Segment。
- Hard Gold：102 个 occurrence（74 个条目）。
- Detection Recall：66 / 102 = 64.7%。
- 焦点读音评估：35 / 66 default exact = 53.0%；candidate coverage = 100.0%。
- 漏报 36；wrong default 31；candidate missing 0；extra predictions 1696。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/huangdi_neijing_test_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/huangdi_neijing_gold_v01.json
- review_required：4 个条目、4 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。

## Current Production Analyzer Behavior

当前实际代码是：先用 pypinyin TONE3 生成 default；加载 worker/dictionaries/medical_terms.json 并按词长降序做精确 token 匹配，命中后生成 medical_term 并覆盖其 token；其余汉字只有在 pypinyin 返回多个候选时生成 polyphone，无法得到有效读音时生成 rare_character；普通单读音汉字不会生成 Annotation。

## Detection

整体：66 / 102 = 64.7%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_pronunciation | 1 | 1 | 100.0% |
| classical_term | 0 | 3 | 0.0% |
| classical_usage | 5 | 7 | 71.4% |
| medical_polyphone | 14 | 14 | 100.0% |
| medical_term | 9 | 33 | 27.3% |
| polyphone | 21 | 21 | 100.0% |
| rare_character | 3 | 5 | 60.0% |
| rare_or_classical | 6 | 9 | 66.7% |
| semantic_polyphone | 6 | 6 | 100.0% |
| simplified_variant | 1 | 2 | 50.0% |
| text_variant | 0 | 1 | 0.0% |

## Pronunciation

焦点级评估：35 / 66 default exact；66 / 66 candidate contains Gold。完整 match_text 可比较的 occurrence：31，其中 full-span default exact 26。

## Most Important Misses


Extra risk_type 分布：medical_term=2, polyphone=1684, rare_character=10。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| HNSW-G001 | 上古天真论篇第一 | 徇齐 / 徇齐 | xun4 qi2 | rare_or_classical | rare_character_not_detected |
| HNSW-G004 | 上古天真论篇第一 | 天癸 / 天癸 | tian1 gui3 | medical_term | medical_term_dictionary_missing |
| HNSW-G004 | 上古天真论篇第一 | 天癸 / 天癸 | tian1 gui3 | medical_term | medical_term_dictionary_missing |
| HNSW-G004 | 上古天真论篇第一 | 天癸 / 天癸 | tian1 gui3 | medical_term | medical_term_dictionary_missing |
| HNSW-G004 | 上古天真论篇第一 | 天癸 / 天癸 | tian1 gui3 | medical_term | medical_term_dictionary_missing |
| HNSW-G004 | 上古天真论篇第一 | 天癸 / 天癸 | tian1 gui3 | medical_term | medical_term_dictionary_missing |
| HNSW-G007 | 上古天真论篇第一 | 颁白 / 颁 | ban1 bai2 | classical_usage | classical_usage_not_detected |
| HNSW-G010 | 上古天真论篇第一 | 六府 / 府 | liu4 fu3 | medical_term | medical_term_dictionary_missing |
| HNSW-G012 | 上古天真论篇第一 | 恚嗔 / 恚嗔 | hui4 chen1 | rare_or_classical | rare_character_not_detected |
| HNSW-G014 | 四气调神大论篇第二 | 发陈 / 发陈 | fa1 chen2 | classical_usage | classical_usage_not_detected |
| HNSW-G016 | 四气调神大论篇第二 | 痎疟 / 痎疟 | jie1 nve4 | medical_term | medical_term_dictionary_missing |
| HNSW-G017 | 四气调神大论篇第二 | 飧泄 / 飧泄 | sun1 xie4 | medical_term | medical_term_dictionary_missing |
| HNSW-G019 | 四气调神大论篇第二 | 地坼 / 坼 | di4 che4 | rare_or_classical | rare_character_not_detected |
| HNSW-G021 | 四气调神大论篇第二 | 痿厥 / 痿厥 | wei3 jue2 | medical_term | medical_term_dictionary_missing |
| HNSW-G022 | 四气调神大论篇第二 | 菀稿 / 菀稿 | wan3 gao3 | text_variant | text_variant |
| HNSW-G028 | 生气通天论篇第三 | 九窍 / 窍 | jiu3 qiao4 | medical_term | medical_term_dictionary_missing |
| HNSW-G028 | 生气通天论篇第三 | 九窍 / 窍 | jiu3 qiao4 | medical_term | medical_term_dictionary_missing |
| HNSW-G028 | 生气通天论篇第三 | 九窍 / 窍 | jiu3 qiao4 | medical_term | medical_term_dictionary_missing |
| HNSW-G031 | 生气通天论篇第三 | 运枢 / 枢 | yun4 shu1 | medical_term | medical_term_dictionary_missing |
| HNSW-G036 | 生气通天论篇第三 | 煎厥 / 煎厥 | jian1 jue2 | medical_term | medical_term_dictionary_missing |

## Wrong Defaults

| ID | occurrence | focus | Gold focus | system default | candidates |
|---|---:|---|---|---|---|
| HNSW-G003 | 1 | 更 | geng1 | geng4 | geng1 | geng4 |
| HNSW-G003 | 2 | 更 | geng1 | geng4 | geng1 | geng4 |
| HNSW-G008 | 1 | 藏 | zang4 | cang2 | cang2 | zang1 | zang4 |
| HNSW-G009 | 1 | 藏 | zang4 | cang2 | cang2 | zang1 | zang4 |
| HNSW-G009 | 2 | 藏 | zang4 | cang2 | cang2 | zang1 | zang4 |
| HNSW-G009 | 3 | 藏 | zang4 | cang2 | cang2 | zang1 | zang4 |
| HNSW-G015 | 1 | 蕃 | fan2 | fan1 | bo1 | fan1 | fan2 | pi2 |
| HNSW-G023 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| HNSW-G024 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| HNSW-G025 | 1 | 少 | shao4 | shao3 | shao3 | shao4 |
| HNSW-G026 | 1 | 少 | shao4 | shao3 | shao3 | shao4 |
| HNSW-G029 | 1 | 数 | shuo4 | shu4 | shu3 | shu4 | shuo4 |
| HNSW-G035 | 1 | 长 | chang2 | zhang3 | chang2 | zhang3 |
| HNSW-G035 | 2 | 长 | chang2 | zhang3 | chang2 | zhang3 |
| HNSW-G038 | 1 | 菀 | yun4 | wan3 | wan3 | yu4 | yun4 |
| HNSW-G039 | 1 | 薄 | bo2 | bao2 | bao2 | bo2 | bo4 | bu4 |
| HNSW-G047 | 1 | 俞 | shu4 | yu2 | shu4 | yu2 |
| HNSW-G050 | 1 | 俞 | shu4 | yu2 | shu4 | yu2 |
| HNSW-G053 | 1 | 畜 | xu4 | chu4 | chu4 | xu4 |
| HNSW-G055 | 1 | 亟 | qi4 | ji2 | ji2 | qi4 |

## Extra Predictions

共 1696 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。以下是前 20 条抽样；likely_useful/unclear 是工程抽样分类，不改变硬指标。

| chapter | segment | surface | default | risk | classification | context |
|---|---:|---|---|---|---|---|
| 上古天真论篇第一 | 0 | 昔 | xi1 | polyphone | unclear | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰： |
| 上古天真论篇第一 | 0 | 而 | er2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之 |
| 上古天真论篇第一 | 0 | 神 | shen2 | polyphone | unclear | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人 |
| 上古天真论篇第一 | 0 | 而 | er2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆 |
| 上古天真论篇第一 | 0 | 能 | neng2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度 |
| 上古天真论篇第一 | 0 | 言 | yan2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百 |
| 上古天真论篇第一 | 0 | 幼 | you4 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁， |
| 上古天真论篇第一 | 0 | 而 | er2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而 |
| 上古天真论篇第一 | 0 | 齐 | qi2 | polyphone | unclear | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作 |
| 上古天真论篇第一 | 0 | 长 | zhang3 | polyphone | likely_useful | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰 |
| 上古天真论篇第一 | 0 | 而 | er2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰； |
| 上古天真论篇第一 | 0 | 敦 | dun1 | polyphone | unclear | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今 |
| 上古天真论篇第一 | 0 | 而 | er2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人， |
| 上古天真论篇第一 | 0 | 登 | deng1 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年 |
| 上古天真论篇第一 | 1 | 乃 | nai3 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而 |
| 上古天真论篇第一 | 1 | 于 | yu2 | polyphone | likely_unnecessary | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而动作 |
| 上古天真论篇第一 | 1 | 余 | yu2 | polyphone | likely_unnecessary | 黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而动作皆衰者，时世 |
| 上古天真论篇第一 | 1 | 上 | shang4 | polyphone | likely_unnecessary | ，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而动作皆衰者，时世异耶 |
| 上古天真论篇第一 | 1 | 古 | gu3 | polyphone | likely_unnecessary | 生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而动作皆衰者，时世异耶？ |
| 上古天真论篇第一 | 1 | 之 | zhi1 | polyphone | likely_unnecessary | 而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰；今时之人，年半百而动作皆衰者，时世异耶？人 |

## review_required Observation

| ID | match | focus | Gold | detected | default | candidates |
|---|---|---|---|---|---|---|
| HNSW-G013 | 精气溢写 | 写 |  | True | xie3 | xie3 | xie4 |
| HNSW-G032 | 喘喝 | 喝 |  | True | he1 | he1 | he4 | kai4 | ye4 |
| HNSW-G037 | 汨汨 | 汨汨 |  | False |  |  |
| HNSW-G054 | 当写 | 写 |  | True | xie3 | xie3 | xie4 |

## Special Checks

| item | occurrences | detected | default/candidate observation |
|---|---:|---:|---|
| 五藏 | 3 | 3 | cang2 (cang2 | zang1 | zang4); cang2 (cang2 | zang1 | zang4); cang2 (cang2 | zang1 | zang4) |
| 亟夺 | 1 | 1 | ji2 (ji2 | qi4) |
| 俞气 | 1 | 1 | yu2 (shu4 | yu2) |
| 其音征 | 1 | 0 | 未标注 (—) |
| 其音角 | 1 | 1 | jiao3 (gu3 | jiao3 | jue2 | lu4) |
| 数犯 | 1 | 1 | shu4 (shu3 | shu4 | shuo4) |
| 数至 | 1 | 1 | shu4 (shu3 | shu4 | shuo4) |
| 痎疟 | 1 | 0 | 未标注 (—) |
| 穴俞 | 1 | 1 | yu2 (shu4 | yu2) |
| 緛短 | 2 | 2 | ruan3 (ruan3 | ruan4); ruan3 (ruan3 | ruan4) |
| 肾藏 | 1 | 1 | cang2 (cang2 | zang1 | zang4) |
| 起亟 | 1 | 1 | ji2 (ji2 | qi4) |
| 闭藏 | 1 | 1 | cang2 (cang2 | zang1 | zang4) |
| 飧泄 | 1 | 0 | 未标注 (—) |
| 鼽衄 | 2 | 0 | 未标注 (—); 未标注 (—) |
| 𫏋 | 1 | 1 | jue1 (jue1 | qiao1) |

## Textual Issues

- 汨汨 / 汩汩：当前字形与对照底本不同，review_required 不应作为 analyzer 错误计分。 建议：先由人工确定 v0.2 底本，再决定目标读音。
- 精气溢写 / 精气溢写/精气溢泻：通假字按本字 xie3 还是通假义 xie4 尚未固定，当前 Gold 明确不计硬分。 建议：先确定项目通假字朗读政策，再纳入硬 Gold。
- 喘喝 / 喘喝：公开资料对该语境读 he1/he4 存在分歧，当前 Gold 明确不计硬分。 建议：由人工选定权威来源后再确定目标读音。
- 当写 / 当写/当泻：本字与通假义两种朗读政策不同，当前 Gold 明确不计硬分。 建议：与精气溢写使用同一通假字政策。
- 菀稿 / 菀稾/菀槁：字形变体不改变本 Gold 目标读音，但可能影响精确术语词典命中。 建议：后续词典记录字形变体，不改本次输入。
- 其音征 / 其音徵：当前字形可能被普通字典读作 zheng1，Gold 语义读作 zhi3。 建议：保留原文，同时把异体/简化映射作为后续词典数据。
- 按𫏋 / 按蹻：补充平面 CJK 字符需要正确 token 化，不能按 BMP 假设处理。 建议：保持当前文本，继续用 Grapheme Token 定位。
- 痤痱 / 痤疿：两种字形在本 Gold 中采用同一读音，但词典匹配可能受字形影响。 建议：后续词典记录字形变体，不修改本次输入。
- 皶 / 齇：当前字形的 pypinyin/词典覆盖可能与常用底本字不同。 建议：后续补充异体字关联，但先保持 Gold 与文本独立。

## Architecture Findings

漏报原因分类（工程归因）：{'rare_character_not_detected': 5, 'medical_term_dictionary_missing': 24, 'classical_usage_not_detected': 2, 'text_variant': 2, 'pypinyin_single_reading': 3}。本轮未发现 Gold occurrence 跨 Segment 边界，也未发现 supplementary-plane 字符 𫏋 的 token 定位错位；这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，M8 更适合先做“古籍/中医发音词典增强 + 可解释上下文候选”，暂不以 TTS 音频质检作为本 benchmark 的结论。以上建议只记录，不在本任务中实施。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
