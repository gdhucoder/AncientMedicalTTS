# AncientMedicalTTS 第二组 Hold-out 测试集 v0.1

## 文件

### 《伤寒论序》
- `shanghanlun_xu_test_v01.txt`
- `shanghanlun_xu_gold_v01.csv`
- `shanghanlun_xu_gold_v01.json`

### 《郑伯克段于鄢》
- `zhengbo_keduan_test_v01.txt`
- `zhengbo_keduan_gold_v01.csv`
- `zhengbo_keduan_gold_v01.json`

## 来源

用户指定的拼音参考页：

- 《伤寒论序》：https://www.cngwzj.com/pygushi/LiangHan/87734/
- 《郑伯克段于鄢》：https://www.cngwzj.com/pygushi/XianQin/67779/

注意：用户给出的第一个链接实际标题为《伤寒论序》，不是《伤寒论》全书，因此本 benchmark 对象为《伤寒论序》。

## 数据口径

这两组数据用于**第二套 hold-out benchmark**，不要在跑基线前导入：

- built-in medical lexicon
- context rules
- Book Rule
- Global Rule

Gold 只选择高风险、诊断价值较高的字词/语境，不复制网页的整篇逐字注音。

`gold_pinyin` 使用 AncientMedicalTTS 内部格式：

`ASCII Hanyu Pinyin + tone number`

例如：

- `wu4`
- `zhai4`
- `sheng4`
- `fu3 shu4`

## 数量

《伤寒论序》：
- 汉字数：609
- hard gold：35
- review_required：0

《郑伯克段于鄢》：
- 汉字数：550
- hard gold：29
- review_required：4

## 判分

只对：

`status = gold`

做自动硬判分。

`review_required` 只观察系统行为，不计入 Recall / Default Accuracy。

建议继续沿用《素问》 benchmark 指标：

1. Detection Recall
2. Default Pronunciation Accuracy
3. Candidate Coverage
4. Miss Count
5. Wrong Default Count
6. Candidate Missing Count
7. Extra Predictions

## 特别注意

《郑伯克段于鄢》页面存在少数读音/版本讨论，例如：

- `无使滋蔓`中的“无”
- `出奔共`中的“共”
- `繄我独无`中的“繄”
- `不言出奔，难之也`中的“难”

这些已标为 `review_required`，不要作为 analyzer 错误硬判。

## 建议测试顺序

1. 使用 TXT 正常导入 AncientMedicalTTS。
2. 禁止预先导入 Gold。
3. 运行 production analyzer v0.2。
4. 导出 predictions。
5. 再与 JSON Gold 对比。
6. 分别报告两篇结果，再汇总。

这样可以检验 M8 在**中医古籍**和**非医学先秦古文**上的泛化能力。
