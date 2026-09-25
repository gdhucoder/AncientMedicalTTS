# AncientMedicalTTS 仲景系独立验证集 v0.1

本包包含两套**独立 hold-out**：

1. 《伤寒论》：1735 汉字，43 条 Hard Gold，3 条 review_required
2. 《金匮要略》：2040 汉字，45 条 Hard Gold，3 条 review_required

## 推荐验证顺序

先不要看 Gold：

```text
TXT
→ 当前 production analyzer
→ 保存 predictions
→ 再读取 Gold
→ benchmark
```

建议分别报告：

- Detection Recall
- Default Accuracy
- Candidate Coverage
- Miss
- Wrong Default
- Candidate Missing
- Extra Predictions

然后把两本书和此前《素问》结果并排比较。

## 重要

这两套文本用于工程验证，不是校勘定本。若后续根据其中漏报修改了 production analyzer，则该书不再是严格 hold-out，应该换新的古籍验证泛化。
