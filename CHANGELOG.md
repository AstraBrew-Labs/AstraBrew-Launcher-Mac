# 更新日志

本文件记录 AstraBrew Launcher MacOS 平台 的所有版本更新内容。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范。

---

## [0.2.1] - 2026-09-19

### Added
- 启动器自动更新：从 GitCode 镜像（优先）与 GitHub 直连（兜底）检测并安装新版本；minisign 验签、下载地址按发布资产自动纠偏、多源回退、失败原因精确归因。
- 设置页「软件与更新」区块：展示当前版本，提供「检查更新」入口与更新确认弹窗（含发行说明）。
- 构建发布流水线：`build.sh`（本地）与 `.github/workflows/release.yml`（CI），产出 universal 应用包、DMG、签名更新包与 `latest.json`。
- 测试版标记：构建渠道为 beta 时界面左上角显示「测试版」，由 Cargo.toml 的 `[package.metadata.astrabrew] beta` 开关控制。
- 版本管理、扩展管理、资源管理、本地实例管理、环境依赖检测、动态字体加载与界面缩放等核心模块。

### Changed
- 设置页版本号不再硬编码，改为读取编译期版本号。

### Fixed
- 修正更新清单平台键（`darwin-universal`）与更新器自检键（`macos-<arch>`）不匹配导致无法检测的问题。

## [0.2.0] - 2026-08-18

### Changed
- 完成 iced + astra_ui 界面架构迁移，补齐核心页面与模块。

## [0.1.0] - 2026-08-17

### Changed
- 从 egui 迁移到 iced + astra_ui，重建导航、主题与字体系统。

## [0.0.1] - 2026-06-23

- 初始版本
- 添加了 README.md 文件，并完善了项目结构。
- 第一个测试版，包含基础的设置页面、控制台、主页。
- 反向代理还处于初始阶段，但功能尚未完善。