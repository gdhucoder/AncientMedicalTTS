# 第三方组件：FFmpeg

## 当前开发验证

本次 Milestone 7 在 Apple Silicon macOS 开发机上检测到 Homebrew FFmpeg **8.1**，并通过 `ffmpeg -encoders` 验证包含 `libmp3lame`。该 binary 使用 Homebrew 动态库；`scripts/prepare-ffmpeg.mjs` 在 macOS 构建时会复制其 Homebrew 依赖到 app 的 `Resources/ffmpeg-runtime`，改写为 app 相对路径并进行开发用 ad-hoc 签名，因此本机 debug `.app` 不再依赖 `/opt/homebrew`。这不是静态 FFmpeg 构建，正式发布仍应使用经过审计的目标平台构建并由发布签名流程重新签名。

FFmpeg 官方源码和发布入口为 [ffmpeg.org](https://ffmpeg.org/)。本项目的目标 sidecar 文件名为：

- `ffmpeg-aarch64-apple-darwin`
- `ffmpeg-x86_64-apple-darwin`
- `ffmpeg-x86_64-pc-windows-msvc.exe`

## 许可证

FFmpeg 的许可证取决于实际构建启用的组件和配置。当前开发机 Homebrew 构建显示为 GPL 构建并包含 `libmp3lame`；本项目不把该许可证结论泛化到所有未来 binary。`libmp3lame` 本身也有独立许可证义务。

正式发布前必须针对每个平台实际使用的 binary 记录：FFmpeg 版本、源码或可信构建来源、完整 configure/build flags、启用的编码器、许可证文本及随安装包提供的 notices。若改用 LGPL 配置，必须重新验证 `libmp3lame` 是否可用以及对应的再分发义务。

## GitHub Actions 自动构建

`.github/workflows/build.yml` 会在以下 GitHub-hosted runner 上分别构建：

- `macos-15`：Apple Silicon arm64；
- `macos-15-intel`：Intel x86_64；
- `windows-2025`：Windows x64。

构建时 macOS runner 通过 Homebrew 安装 FFmpeg，Windows runner 通过 Chocolatey 安装 FFmpeg；随后由 `prepare-ffmpeg.mjs` 校验版本输出和 `libmp3lame`，再复制到 Tauri sidecar 位置。这样不需要人工把 binary 放进源码仓库，但这些 runner 包管理器提供的 binary 仍必须在正式长期分发前记录具体版本、来源、构建配置和许可证。Workflow 的产物会保存 14 天；推送 `v*` tag 时会自动上传到对应 GitHub Release。

这套自动构建不把 FFmpeg 当作系统依赖发布：系统 FFmpeg 只在 runner 上作为构建输入，最终 App 内仍携带自己的 FFmpeg sidecar。正式 Developer ID 签名/notarization 仍需要在发布 job 中接入签名凭据；当前 workflow 只做 macOS ad-hoc 签名。

## 准备方式

`scripts/prepare-ffmpeg.mjs` 是可重复的文件校验和复制入口。正式构建应把经过审计的目标 binary 放在：

```text
vendor/ffmpeg/ffmpeg-aarch64-apple-darwin
vendor/ffmpeg/ffmpeg-x86_64-apple-darwin
vendor/ffmpeg/ffmpeg-x86_64-pc-windows-msvc.exe
```

Tauri 配置通过 `bundle.externalBin` 发布 `binaries/ffmpeg`，并通过 `bundle.resources` 发布 macOS 动态依赖；Tauri 会根据 target triple 选择对应 sidecar。当前仓库没有提交大型 FFmpeg binary，也没有声称 Windows x64 或 macOS Intel binary 已在本机完成构建；缺少 vendor binary 时，生产构建会明确失败。正式 Developer ID signing/notarization 时，`ffmpeg` 和 `ffmpeg-runtime` 中的所有 Mach-O 文件必须随 app 一起重新签名。
