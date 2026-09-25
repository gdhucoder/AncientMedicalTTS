# 更新记录

## [0.1.0-rc1] - 2026-09-25

这是 AncientMedicalTTS v0.1.0 的候选版本，进入功能冻结后的发布验证阶段。

### 已包含

- 本地古籍 TXT 导入、篇章识别、段落导航和 5000 汉字限制。
- 基于 Grapheme Token 的发音分析、人工确认、忽略、规则复用和注音显示。
- `original_text` 不可变的朗读文本编辑、按光标分段、前后段合并和参与朗读开关。
- 腾讯云 TTS 单段/全文生成、取消/继续、单段试听和 16000Hz WAV。
- 按当前音频版本顺序导出完整 WAV 或 96 kbps MP3。
- API 调用用量统计、应用内凭据文件、Python 3.12 PyInstaller Worker 和内置 FFmpeg 运行链路。
- Rust、Python、前端和 M9 真实古籍基线测试资料。

### 本次发布收尾

- 增加发布差距清单、Feature Freeze、快速开始、已知限制和 v0.2 backlog。
- 增加 GitHub Pages 公开说明页；页面不包含 Secret、数据库、音频或构建产物。
- SQLite 在已有数据库迁移前创建 `app.sqlite.backup` 一致性快照。

### 尚未宣称完成

- Windows 10/11 与 macOS Intel 的真机安装、签名和运行尚未在当前开发机验证。
- 腾讯云真实请求需要用户在本地配置自己的凭据；公开仓库不提供凭据。
