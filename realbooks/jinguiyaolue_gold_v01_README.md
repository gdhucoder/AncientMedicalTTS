# 金匮要略 pronunciation hold-out v0.1

## 用途

这套数据用于 AncientMedicalTTS 的**独立泛化验证**，不要作为 production analyzer 的词典来源。

推荐顺序：

1. 先只导入 `jinguiyaolue_holdout_v01.txt`
2. 跑当前 production analyzer
3. 保存 prediction
4. 最后才读取 Gold
5. 用 benchmark runner 计算 Detection Recall / Default Accuracy / Candidate Coverage / Extra Predictions

## 数据规模

- 汉字数（含标题/篇名）：2040
- 篇章：5
- Gold 条目：48
- Hard Gold：45
- review_required：3

## 评测口径

- 目标：现代普通话朗读
- 中医术语：按现代中医惯用读法
- 不复原上古音/中古音
- 拼音内部格式：`ASCII + tone number`
- `v` 表示 `ü`

## review_required

这些条目不要计入硬准确率：

- `身体强，几几然`：“几几”读法存在长期争议，不纳入硬判分。
- `声喝`：“喝”在该古籍病证语境的朗读需按权威校注固定，暂不硬判。
- `瘅疟`：现代资料存在 dàn/dān 标注不一致；建议项目确定权威来源后再硬判。

## 文本说明

正文参照公开的《金匮要略》通行文本整理，用于软件工程验证。章节选择、现代标点和空白经过整理，因此它是**工程测试节选**，不是严格校勘本。

维基文库的相关页面本身也提示部分文本仍可能存在校订/简繁转换问题；正式出版或商业朗读前，应再和权威纸本/专业校勘本核对。

## 防止 Gold 泄漏

不要：

- 把 Gold 自动导入 `medical_terms`
- 把 Gold 自动导入 Context Rules
- 创建 Book/Global Rule 后再报告 baseline
- 根据本 hold-out 的漏报逐条硬编码再在同一数据上报告“泛化提升”

如果未来用本集调过 analyzer，本集应降级为 development set，并换第三套古籍做新的 hold-out。

## 推荐指标

- Detection Recall
- Default Pronunciation Accuracy
- Candidate Coverage
- Miss Count
- Wrong Default Count
- Extra Predictions

Gold 之外的 prediction 只能称为 `extra predictions`，不能自动视为 false positive。

## 文件

- `jinguiyaolue_holdout_v01.txt`
- `jinguiyaolue_gold_v01.csv`
- `jinguiyaolue_gold_v01.json`
- `jinguiyaolue_gold_v01_README.md`
