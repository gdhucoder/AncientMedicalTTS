# AncientMedicalTTS v0.1.0 发布差距清单

> 建立日期：2026-09-25  
> 当前阶段：发布收尾 / RC1 已发布  
> 说明：本文件记录发布前的真实状态，不把未验证的平台或能力描述为已完成。

## 当前已确认

- M1～M9 的主要应用代码、SQLite migration、前端测试、Rust 测试和 Python Worker 测试已经存在。
- 当前工作区已初始化 Git，`main` 已推送，RC1 提交和 tag 可追溯。
- 当前源码候选版本为 `0.1.0-rc1`；Worker/分析器版本独立为 `0.2.0`/`0.3.0`。
- Apple Silicon macOS 已完成一次本机构建验证：PyInstaller Worker 使用 Python 3.12.x 构建，App 内 Worker 可脱离外部 Python/uv/PATH 完成 `system.ping` 和发音分析；FFmpeg 8.1 与 `libmp3lame` 已验证。
- 常规 Rust、前端和 Python 测试在最近一次发布前检查中通过；FFmpeg 集成测试使用显式环境变量启用。
- `ANCIENT_TTS_TENCENT_SECRET_ID` 和 `ANCIENT_TTS_TENCENT_SECRET_KEY` 只存在于本地 `.env`/运行环境，不应进入 Git 或公开 Pages。
- 现有 M9 回归报告覆盖《黄帝内经·素问》《伤寒论》《金匮要略》；报告显示分析器版本 0.3.0、Python 3.12.13、pypinyin 0.55.0，并保留了 Gold 与 extra/missed 统计。
- RC1 重跑报告已写入 `reports/benchmark/release_rc1/`，记录了首个 Git commit 和 `0.1.0-rc1` 应用版本。

## 发布前必须补齐

| 项目 | 当前状态 | 收尾动作 |
| --- | --- | --- |
| 可追溯版本 | 已完成 | 本地 Git、`main` 分支、RC1 提交和 `v0.1.0-rc1` tag 已推送 |
| 发布文档 | 已补齐 | `CHANGELOG.md`、`docs/RELEASE_CHECKLIST.md`、`docs/QUICK_START.md`、`docs/KNOWN_LIMITATIONS.md`、`docs/V02_BACKLOG.md`、`docs/RC1_VALIDATION.md` |
| Feature Freeze | 已完成 | `docs/FEATURE_FREEZE.md` 已冻结 M9 之后的新功能 |
| 公开入口 | 已建立 | `site/` 静态页面和 GitHub Pages workflow 已推送，首次部署已成功 |
| 数据安全 | 已完成审计 | `.env`、Secret、应用数据库、音频和构建产物均未进入本地或远程公开提交 |
| 数据库迁移保护 | 已完成 | 迁移前对已有 SQLite 文件创建 `app.sqlite.backup` 一致性副本，并已有 Rust 测试 |
| 跨平台构建 | 未在本机完成 Windows 或 Intel macOS 实测 | 公开记录为未测试；对应平台仍需各自使用 Python 3.12 构建 Worker、准备 FFmpeg 并签名 |
| Windows 安装/运行 | 未测试 | 发布前必须在 Windows 10/11 至少一台真机验证；当前不能宣称通过 |
| macOS Intel | 无当前硬件实测 | 可做目标构建验证，但硬件运行状态保持 pending |
| 腾讯云真实业务 | 常规检查不调用真实接口 | 需要凭据的 TTS/试听/批量/导出验收由使用者在本地完成，不把 Secret 写进报告 |

## 当前发布判断

当前适合形成 **公开源码仓库 + GitHub Pages + `v0.1.0-rc1` 候选版本**。在 Windows 与 macOS Intel 的构建/运行验证完成前，不应把跨平台安装包称为最终 `v0.1.0`，也不应声称三平台均已验收。

本次收尾允许的改动范围：发布工程、文档、测试、错误处理、迁移备份、公开站点和明确的阻塞性修复。禁止借发布收尾继续增加新的阅读、分析、TTS 或导出业务功能。
