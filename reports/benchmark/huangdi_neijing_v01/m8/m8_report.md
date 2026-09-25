# AncientMedicalTTS《黄帝内经·素问》发音评测 m8

## Executive Summary

本报告评估实际生产 pronunciation analyzer v0.2。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。

- 正文：3273 个汉字；4 个 Chapter；85 个 Segment。
- Hard Gold：102 个 occurrence（74 个条目）。
- Detection Recall：97 / 102 = 95.1%。
- 焦点读音评估：97 / 97 default exact = 100.0%；candidate coverage = 100.0%。
- 漏报 5；wrong default 0；candidate missing 0；extra predictions 79。

## v0.1 → v0.2 对比

| 指标 | v0.1 baseline | v0.2 M8 |
|---|---:|---:|
| detection recall | 64.7% | 95.1% |
| default accuracy | 53.0% | 100.0% |
| candidate coverage | 100.0% | 100.0% |
| missed | 36 | 5 |
| wrong default | 31 | 0 |
| candidate missing | 0 | 0 |
| extra predictions | 1696 | 79 |

M8 验收目标：detection recall ≥ 85%、default accuracy ≥ 80%、candidate coverage ≥ 98%、candidate missing = 0；本次全部达到。extra predictions 由 1696 降至 79，仍需结合真实复核继续治理，但没有把 Gold 写入生产路径。

## Dataset

- Text：/Users/gdhu/projects/ancientbookreading/realbooks/huangdi_neijing_test_v01.txt
- Gold：/Users/gdhu/projects/ancientbookreading/realbooks/huangdi_neijing_gold_v01.json
- review_required：4 个条目、4 个 occurrence；不计入硬指标。
- Segment boundary crossing：0；context 未找到：0。

## Current Production Analyzer Behavior

当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v0.2 增加版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。

## Detection

整体：97 / 102 = 95.1%。

| risk_type | detected | total | recall |
|---|---:|---:|---:|
| classical_pronunciation | 1 | 1 | 100.0% |
| classical_term | 3 | 3 | 100.0% |
| classical_usage | 5 | 7 | 71.4% |
| medical_polyphone | 14 | 14 | 100.0% |
| medical_term | 33 | 33 | 100.0% |
| polyphone | 21 | 21 | 100.0% |
| rare_character | 5 | 5 | 100.0% |
| rare_or_classical | 7 | 9 | 77.8% |
| semantic_polyphone | 6 | 6 | 100.0% |
| simplified_variant | 2 | 2 | 100.0% |
| text_variant | 0 | 1 | 0.0% |

## Pronunciation

焦点级评估：97 / 97 default exact；97 / 97 candidate contains Gold。完整 match_text 可比较的 occurrence：94，其中 full-span default exact 94。

## Most Important Misses


Extra risk_type 分布：context_pronunciation=9, medical_term=12, polyphone=48, rare_character=10。
| ID | chapter | match / focus | Gold | risk | possible reason |
|---|---|---|---|---|---|
| HNSW-G022 | 四气调神大论篇第二 | 菀稿 / 菀稿 | wan3 gao3 | text_variant | text_variant |
| HNSW-G033 | 生气通天论篇第三 | 燔炭 / 燔 | fan2 tan4 | rare_or_classical | rare_character_not_detected |
| HNSW-G049 | 生气通天论篇第三 | 魄汗 / 魄 | po4 han4 | classical_usage | classical_usage_not_detected |
| HNSW-G052 | 生气通天论篇第三 | 苛毒 / 苛 | ke1 du2 | classical_usage | classical_usage_not_detected |
| HNSW-G073 | 金匮真言论篇第四 | 稷 / 稷 | ji4 | rare_or_classical | rare_character_not_detected |

## Wrong Defaults

没有被评估 occurrence 出现 default mismatch。

## Extra Predictions

共 79 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。以下是前 20 条抽样；likely_useful/unclear 是工程抽样分类，不改变硬指标。

| chapter | segment | surface | default | risk | source | classification | context |
|---|---:|---|---|---|---|---|---|
| 上古天真论篇第一 | 0 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | 昔在黄帝，生而神灵，弱而能言，幼而徇齐，长而敦敏，成而登天。乃问于天师曰：“余闻上古之人，春秋皆度百岁，而动作不衰 |
| 上古天真论篇第一 | 6 | 少 | shao3 | polyphone | high_risk_polyphone | likely_useful | ：虚邪贼风，避之有时；恬淡虚无，真气从之；精神内守，病安从来？是以志闲而少欲，心安而不惧，形劳而不倦，气从以顺，各从其欲，皆得所愿。故美其食，任其 |
| 上古天真论篇第一 | 9 | 子 |  | rare_character | pypinyin | likely_useful | 所以能年皆度百岁，而动作不衰者，以其德全不危也。”

帝曰：“人年老而无子者，材力尽邪？将天数然也？”

岐伯曰：“女子七岁，肾气盛，齿更发长。二 |
| 上古天真论篇第一 | 9 | 数 | shu4 | polyphone | high_risk_polyphone | likely_useful | 动作不衰者，以其德全不危也。”

帝曰：“人年老而无子者，材力尽邪？将天数然也？”

岐伯曰：“女子七岁，肾气盛，齿更发长。二七而天癸至，任脉通， |
| 上古天真论篇第一 | 9 | 子 |  | rare_character | pypinyin | likely_useful | 。”

帝曰：“人年老而无子者，材力尽邪？将天数然也？”

岐伯曰：“女子七岁，肾气盛，齿更发长。二七而天癸至，任脉通，太冲脉盛，月事以时下，故有 |
| 上古天真论篇第一 | 9 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | 而无子者，材力尽邪？将天数然也？”

岐伯曰：“女子七岁，肾气盛，齿更发长。二七而天癸至，任脉通，太冲脉盛，月事以时下，故有子；
三七，肾气平均， |
| 上古天真论篇第一 | 10 | 子 |  | rare_character | pypinyin | likely_useful | 七岁，肾气盛，齿更发长。二七而天癸至，任脉通，太冲脉盛，月事以时下，故有子；
三七，肾气平均，故真牙生而长极；
四七，筋骨坚，发长极，身体盛壮；
 |
| 上古天真论篇第一 | 10 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | 癸至，任脉通，太冲脉盛，月事以时下，故有子；
三七，肾气平均，故真牙生而长极；
四七，筋骨坚，发长极，身体盛壮；
五七，阳明脉衰，面始焦，发始堕； |
| 上古天真论篇第一 | 10 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | 月事以时下，故有子；
三七，肾气平均，故真牙生而长极；
四七，筋骨坚，发长极，身体盛壮；
五七，阳明脉衰，面始焦，发始堕；六七，三阳脉衰于上，面皆 |
| 上古天真论篇第一 | 11 | 少 | shao3 | polyphone | high_risk_polyphone | likely_useful | 焦，发始堕；六七，三阳脉衰于上，面皆焦，发始白；
七七，任脉虚，太冲脉衰少，天癸竭，地道不通，故形坏而无子也。

丈夫八岁，肾气实，发长齿更；二八 |
| 上古天真论篇第一 | 11 | 子 |  | rare_character | pypinyin | likely_useful | 面皆焦，发始白；
七七，任脉虚，太冲脉衰少，天癸竭，地道不通，故形坏而无子也。

丈夫八岁，肾气实，发长齿更；二八，肾气盛，天癸至，精气溢写，阴阳 |
| 上古天真论篇第一 | 11 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | ，太冲脉衰少，天癸竭，地道不通，故形坏而无子也。

丈夫八岁，肾气实，发长齿更；二八，肾气盛，天癸至，精气溢写，阴阳和，故能有子；三八，肾气平均， |
| 上古天真论篇第一 | 12 | 子 |  | rare_character | pypinyin | likely_useful | 夫八岁，肾气实，发长齿更；二八，肾气盛，天癸至，精气溢写，阴阳和，故能有子；三八，肾气平均，筋骨劲强，故真牙生而长极；
四八，筋骨隆盛，肌肉满壮； |
| 上古天真论篇第一 | 13 | 强 | qiang2 | polyphone | high_risk_polyphone | likely_useful | 二八，肾气盛，天癸至，精气溢写，阴阳和，故能有子；三八，肾气平均，筋骨劲强，故真牙生而长极；
四八，筋骨隆盛，肌肉满壮；
五八，肾气衰，发堕齿槁； |
| 上古天真论篇第一 | 13 | 长 | zhang3 | polyphone | high_risk_polyphone | likely_useful | 天癸至，精气溢写，阴阳和，故能有子；三八，肾气平均，筋骨劲强，故真牙生而长极；
四八，筋骨隆盛，肌肉满壮；
五八，肾气衰，发堕齿槁；
六八，阳气衰 |
| 上古天真论篇第一 | 14 | 少 | shao3 | polyphone | high_risk_polyphone | likely_useful | 
六八，阳气衰竭于上，面焦，发鬓颁白；七八，肝气衰，筋不能动，天癸竭，精少，肾藏衰，形体皆极；
八八，则齿发去。肾者主水，受五藏六府之精而藏之，故 |
| 上古天真论篇第一 | 15 | 藏 | cang2 | polyphone | high_risk_polyphone | likely_useful | 竭，精少，肾藏衰，形体皆极；
八八，则齿发去。肾者主水，受五藏六府之精而藏之，故五藏盛乃能写。今五藏皆衰，筋骨懈惰，天癸尽矣。故发鬓白，身体重，行 |
| 上古天真论篇第一 | 15 | 子 |  | rare_character | pypinyin | likely_useful | 乃能写。今五藏皆衰，筋骨懈惰，天癸尽矣。故发鬓白，身体重，行步不正，而无子耳。”

帝曰：“有其年已老而有子者，何也？”

岐伯曰：“此其天寿过度 |
| 上古天真论篇第一 | 15 | 子 |  | rare_character | pypinyin | likely_useful | 尽矣。故发鬓白，身体重，行步不正，而无子耳。”

帝曰：“有其年已老而有子者，何也？”

岐伯曰：“此其天寿过度，气脉常通，而肾气有馀也。此虽有子 |
| 上古天真论篇第一 | 17 | 子 |  | rare_character | pypinyin | likely_useful | 子者，何也？”

岐伯曰：“此其天寿过度，气脉常通，而肾气有馀也。此虽有子，男不过尽八八，女不过尽七七，而天地之精气皆竭矣。”

帝曰：“夫道者年 |

## review_required Observation

| ID | match | focus | Gold | detected | default | candidates |
|---|---|---|---|---|---|---|
| HNSW-G013 | 精气溢写 | 写 |  | False |  |  |
| HNSW-G032 | 喘喝 | 喝 |  | False |  |  |
| HNSW-G037 | 汨汨 | 汨汨 |  | True | mi4 mi4 | gu3 gu3 | mi4 mi4 |
| HNSW-G054 | 当写 | 写 |  | False |  |  |

## Special Checks

| item | occurrences | detected | default/candidate observation |
|---|---:|---:|---|
| 五藏 | 3 | 3 | zang4 (cang2 | zang1 | zang4); zang4 (cang2 | zang1 | zang4); zang4 (cang2 | zang1 | zang4) |
| 亟夺 | 1 | 1 | ji2 (ji2 | qi4) |
| 俞气 | 1 | 1 | shu4 (shu4 | yu2) |
| 其音征 | 1 | 1 | zhi3 (zheng1 | zhi3) |
| 其音角 | 1 | 1 | jue2 (gu3 | jiao3 | jue2 | lu4) |
| 数犯 | 1 | 1 | shuo4 (shu3 | shu4 | shuo4) |
| 数至 | 1 | 1 | shuo4 (shu3 | shu4 | shuo4) |
| 痎疟 | 1 | 1 | jie1 nve4 (jie1 nve4 | jie1 yao4) |
| 穴俞 | 1 | 1 | shu4 (shu4 | yu2) |
| 緛短 | 2 | 2 | ruan3 (ruan3 | ruan4); ruan3 (ruan3 | ruan4) |
| 肾藏 | 1 | 1 | zang4 (cang2 | zang1 | zang4) |
| 起亟 | 1 | 1 | qi4 (ji2 | qi4) |
| 闭藏 | 1 | 1 | cang2 (cang2 | zang1 | zang4) |
| 飧泄 | 1 | 1 | sun1 xie4 (sun1 xie4 | sun1 yi4) |
| 鼽衄 | 2 | 2 | qiu2 nv4 (qiu2 nv4); qiu2 nv4 (qiu2 nv4) |
| 𫏋 | 1 | 1 | qiao1 (jue1 | qiao1) |

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

漏报原因分类（工程归因）：{'text_variant': 1, 'rare_character_not_detected': 2, 'classical_usage_not_detected': 2}。本轮未发现 Gold occurrence 跨 Segment 边界，也未发现 supplementary-plane 字符 𫏋 的 token 定位错位；这些结论仅针对本数据集。

## Recommendation for Next Step

1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。
2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。
3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。

基于本次文本侧结果，M8 更适合先做“古籍/中医发音词典增强 + 可解释上下文候选”，暂不以 TTS 音频质检作为本 benchmark 的结论。以上建议只记录，不在本任务中实施。

## Reproducibility

本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。
