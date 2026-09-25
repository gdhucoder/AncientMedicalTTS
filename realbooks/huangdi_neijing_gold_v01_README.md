# 《黄帝内经·素问》发音 Gold 测试集 v0.1

## 1. 用途

本数据集与 `huangdi_neijing_test_v01.txt` 配套，用于验证 AncientMedicalTTS 的：

- 高风险字词发现；
- 目标拼音标注；
- Book / Global Rule 应用；
- 腾讯 TTS phoneme override；
- 后续发音评测。

本数据集不是逐字全文注音，而是**高风险读音测试集**。

## 2. 判分口径

朗读目标固定为：

> 现代普通话朗读《黄帝内经·素问》，中医术语采用中医/篇章语境读法；不尝试复原上古音。

内部拼音格式：

`ASCII pinyin + tone number`

例如：

- `shu4 xue2`
- `jie1 nve4`
- `qiu2 nv4`
- `lv4`

其中 `v` 代表 `ü`。

## 3. 数据规模

- 条目总数：78
- 可直接硬判分 `gold`：74
- 需人工/校注决策 `review_required`：4

风险类型分布：

- `classical_pronunciation`: 1
- `classical_term`: 3
- `classical_usage`: 7
- `medical_polyphone`: 8
- `medical_term`: 22
- `polyphone`: 15
- `rare_character`: 4
- `rare_or_classical`: 9
- `semantic_polyphone`: 2
- `simplified_variant`: 2
- `source_conflict`: 1
- `text_variant`: 2
- `tongjia`: 2

## 4. 自动评测规则

### 4.1 硬 Gold

只对：

`status = gold`

做自动 Pass / Fail。

系统对 `match_text` 覆盖的完整汉字 span，其最终目标拼音应与 `gold_pinyin` 一致。

例如：

`痎疟 → jie1 nve4`

### 4.2 review_required

以下 v0.1 条目暂时**不计入准确率**：

- `精气溢写`
- `喘喝`
- `汨汨`
- `当写`

原因分别涉及通假字朗读政策、公开资料读音分歧或底本文字差异。

系统可以显示这些条目，但评测程序不得自动判错。

## 5. 当前文本中值得注意的底本问题

### 汨汨

当前测试文件为：

`汨汨乎不可止`

所对照底本/简体转写为：

`汩汩乎不可止`

`汩汩`规范读音为 `gu3 gu3`。

因此这里首先应视为**文本校勘问题**，不能仅凭 TTS 结果判断。

### 按𫏋

`𫏋`同`蹻`。

本篇“故冬不按蹻”语境中采用：

`an4 qiao1`

### 其音征

当前简体测试文本为：

`其音征`

这里语义对应五音“徵”，目标读音为：

`qi2 yin1 zhi3`

### 痤痱 / 皶

测试文本与所对照底本存在字形差异，但 v0.1 Gold 已按当前测试文本建立对应读音，并在 `note` 中说明。

## 6. 推荐指标

### Detection Recall

Gold 中应该被视为高风险的 occurrence，有多少被系统发现：

`detected_gold_occurrences / total_gold_occurrences`

### Pronunciation Accuracy

系统给出的最终 target pinyin 与 Gold 一致的比例：

`correct_gold_occurrences / evaluated_gold_occurrences`

### False Positive

这个指标不能只靠本 Gold 文件自动计算。

因为系统可能标出 Gold 集之外的合理风险字，需要人工复核系统额外标出的项目。

### TTS Actual Pronunciation

本 Gold 文件验证的是**文本侧目标拼音**。

它不能证明腾讯最终 WAV 一定真的读对。

TTS 实际发音仍应通过人工试听或后续音频质检评测。

## 7. 重要限制

Classica Sinica Medica 提供了与具体《素问》段落对齐的拼音、底本说明和文本异文，适合工程核对；其页面同时标注部分内容尚未经过最终审校。

因此本文件中的 `high_for_benchmark` 表示：

> 足以用于当前工程回归测试。

不表示：

> 已完成严格的古籍音韵学、校勘学最终定论。

若软件进入正式出版/商业朗读阶段，建议再由中医古籍专业人员对 Gold 集做一次人工签核。

## 8. 文件关系

- `huangdi_neijing_test_v01.txt`：待测正文
- `huangdi_neijing_gold_v01.csv`：人工查看、筛选、统计
- `huangdi_neijing_gold_v01.json`：程序评测使用
