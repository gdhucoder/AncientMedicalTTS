# v0.1.0-rc1 验证记录

验证日期：2026-09-25  
源码提交：`852b9c73c7061986641bb013b8ffe3d6eed76344`  
分析器：`0.3.0`  · Python：`3.12.13`  · pypinyin：`0.55.0`

## M9 Gold/hold-out 基线

本次只重跑既有 Gold 与 context rules，没有修改 Gold、词典或 analyzer 参数。原始 JSON/CSV 报告位于 `reports/benchmark/release_rc1/`。

| 数据集 | 汉字数 | Gold hard occurrences | Recall | Default Accuracy | Candidate Coverage | Missed | Extra |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 《黄帝内经·素问》 | 3315 | 102 | 99.02% | 100% | 100% | 1 | 74 |
| 《伤寒论》 hold-out | 1735 | 68 | 91.18% | 100% | 100% | 6 | 69 |
| 《金匮要略》 hold-out | 2040 | 53 | 94.34% | 100% | 100% | 3 | 35 |

三套数据集的 Gold 评测均通过，未出现 wrong default 或 candidate missing。Extra predictions 继续按已有 M9 逻辑保留为可解释的复核候选；本次发布收尾没有为了降低 Extra 数量增加特例，也没有把 Gold 反向写入词典。

## 应用与运行时

- Apple Silicon macOS：已完成本机 debug App 构建、安装包内 PyInstaller Worker `system.ping`/发音分析、FFmpeg 8.1 + `libmp3lame` 和 app 签名校验。
- Worker 构建：使用 Python 3.12.x 虚拟环境，生产 App 不依赖目标机 Python、uv 或 PATH。
- Windows 10/11：当前开发机无法真机验证，状态为未测试。
- macOS Intel：当前没有硬件运行验证，状态为构建/发布准备待验证。
- 腾讯云真实请求：没有在公开发布流程中调用；用户应在本地配置凭据后完成试听、单段生成、全文生成和导出 smoke test。

## 发布结论

当前可以发布公开源码仓库、GitHub Pages 和 `v0.1.0-rc1` 候选版本。最终 `v0.1.0` 仍需完成 Windows 10/11 安装运行、macOS Intel 运行（如要支持该架构）、对应平台 Worker/FFmpeg 构建签名，以及至少一套真实凭据下的完整音频试听验收。
