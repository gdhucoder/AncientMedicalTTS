# 当前已知限制

- 当前开发机实际验证的是 Apple Silicon macOS；Windows 10/11 和 macOS Intel 仍需对应环境验证。公开发布前不能把构建成功写成真机通过。
- 正式跨平台包必须分别准备目标平台的 Python 3.12 PyInstaller Worker 和 FFmpeg sidecar；源码仓库不提交这些大型二进制文件。
- Worker 生产运行依赖随 App 发布的 PyInstaller 可执行文件；如果开发环境没有构建 Worker，源码回退只适用于本机调试，不能作为生产发布方案。
- 真实 Tencent TTS、完整古籍批量生成和试听需要用户自己的凭据、网络和配额，常规自动测试不会调用云服务。
- 5000 汉字上限、串行批量生成和本地单实例导出是 v0.1.0 的固定边界。
- 当前不提供通假字自动推断、LLM、OCR、字幕、ASR、crossfade、响度处理、M4B、云同步或多人词典。
- 音频导出不主动插入额外句间静音；真实文本的停顿自然度仍需要人工试听。
- SQLite 迁移前会生成 `app.sqlite.backup`，但用户仍应在升级前备份整个应用数据目录。
