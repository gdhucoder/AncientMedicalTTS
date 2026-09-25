# v0.1.0 发布检查清单

## 版本与范围

- [x] 源码候选版本为 `0.1.0-rc1`。
- [x] Feature Freeze 已记录，未把 v0.2 功能混入本次发布。
- [ ] RC1 在 Windows 10/11 真机完成安装、运行和卸载验证。
- [ ] RC1 在 macOS Intel 真机完成运行验证；Apple Silicon 已验证。

## 代码与测试

- [x] `pnpm test`
- [x] `pnpm build`
- [x] `cargo check --manifest-path src-tauri/Cargo.toml`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] Python 3.12 Worker 单元测试
- [x] M9 三套古籍基线报告已生成
- [ ] 各目标平台 clean build 与安装包验证
- [ ] 发布前再次执行带真实 FFmpeg 的集成测试

## 安全与数据

- [x] `.env`、SecretId、SecretKey、SQLite、日志、音频和构建产物被排除在公开提交之外。
- [x] Secret 不进入前端持久化、Worker 日志或公开 Pages。
- [x] 已有数据库迁移前创建 `app.sqlite.backup`。
- [ ] 发布包在无 Python/uv/PATH 的干净环境完成 Worker `system.ping` 与发音分析（Apple Silicon 已完成，其他平台待验）。

## 组件与安装包

- [x] 开发环境 FFmpeg 版本、来源和许可证注意事项已记录。
- [x] Apple Silicon debug App 的 bundled Worker、FFmpeg 和签名校验已完成。
- [ ] Windows x64 FFmpeg/Worker 准备、签名和安装包验证。
- [ ] macOS Intel FFmpeg/Worker 准备、签名和安装包验证。
- [ ] Developer ID 签名与 notarization（如果作为正式分发渠道）。

## 公开发布

- [x] 本地 Git 初始化并提交发布资料。
- [x] GitHub 公开仓库推送。
- [x] GitHub Pages workflow 已配置。
- [ ] Pages 首次部署成功并检查公开页面不含敏感信息。
- [ ] RC1 验证通过后再打最终 `v0.1.0` tag。
