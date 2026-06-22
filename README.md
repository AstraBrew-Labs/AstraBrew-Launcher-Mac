<img src="https://raw.githubusercontent.com/al01cn/sillyTavern-launcher/GUI/src/assets/images/banner.png" style="width: 100%; height: 100%;" />

# 星酿启动器 (AstraBrew Launcher)


<div style="text-align: center;">

星酿启动器 (AstraBrew Launcher) 是一款专为 MacOS 平台打造的高性能应用程序启动器。它基于 Rust 和 egui 开发，旨在为用户提供快速、轻量、多功能的启动和管理体验。


[![Releases](https://img.shields.io/github/v/release/AstraBrew-Labs/AstraBrew-Launcher-Mac?label=版本)](https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases)
[![Rust](https://img.shields.io/badge/Rust-latest-CE422B?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![egui](https://img.shields.io/github/v/release/emilk/egui?label=egui)](https://github.com/emilk/egui)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

</div>

## 📖 项目介绍

星酿启动器提供了一站式的环境配置、一键启动、版本管理以及扩展和资源管理功能。其特点包括：
- **实时响应的界面**：基于 egui 构建的即时模式 GUI，流畅且资源占用低。
- **国际化支持**：目前支持中文 (zh_CN) 和英文 (en_US)。
- **主题切换**：支持深色和浅色模式的动态切换。
- **多屏自适应**：自动处理 16:9 和 4:3 屏幕比例，适配高分辨率显示器及多屏幕切换。
- **环境隔离**：支持内置和系统级别的 Git、Node.js 环境切换，内置包管理和加速代理配置。

## 🛠 技术栈

- **Rust (2024 Edition)**: 核心开发语言，提供内存安全和极致性能。
- **egui (0.33)**: 简单易用、响应快速的即时模式 (Immediate mode) GUI 框架。
- **eframe (0.33)**: 官方的 egui 原生应用集成框架。
- **egui_phosphor (0.11)**: 提供丰富的界面图标支持。
- **Serde**: 高效的序列化与反序列化库（用于配置文件管理）。
- **serde_json**: 用于 JSON 格式数据的序列化和反序列化。
- **serde_yaml**: 用于 YAML 格式数据的序列化和反序列化。
- **reqwest**: 强大的 HTTP 客户端库，用于网络请求和数据
- **objc2** : Rust 与 macOS 原生 API 的桥接库，用于调用 WKWebView 和其他系统功能。
- **rfd**: 跨平台的文件对话框库，用于文件选择和保存操作。
- **jwalk: 高性能的并行文件系统遍历库，用于快速扫描和管理文件资源。
- **block2**: 用于在 Rust 中实现阻塞操作的库，适用于需要等待的任务。
- **zip: 用于处理 ZIP 文件的库，支持压缩和解压缩操作。
- **qrcode: 用于生成二维码的库，支持多种二维码格式和自定义样式。
- **smappservice-rs**: 用于管理 macOS 自启动服务的库，支持启用、禁用和检查自启动状态。
- **cargo-packager**: 用于打包和发布 Rust 应用程序的库，支持生成 macOS 可执行文件和安装包。
- **cargo-packager-updater**: 用于自动更新和版本管理的库，支持从 GitHub 仓库获取最新版本并进行更新。

## 🚀 安装依赖与运行项目

### 前置要求

在开始之前，请确保您的系统已经安装了以下工具：
- [Rust & Cargo](https://www.rust-lang.org/tools/install) (建议使用最新的 stable 版本)
- 仅支持 MacOS 平台。

### 运行项目

1. 克隆或下载本项目到本地。
2. 进入项目根目录：
   ```bash
   cd astrabrew-launcher-mac
   ```
3. 使用 Cargo 检查或编译项目：
   ```bash
   cargo check
   ```
4. 运行项目（调试模式）：
   ```bash
   cargo run
   ```
   > **注意**：开发过程中如果只需检查代码规范和编译错误，请优先使用 `cargo check` 以提高效率。

## 📂 项目结构

```text
astrabrew-launcher-mac/
├── assets/                  # 静态资源文件
│   └── fonts/               # 字体文件（如 MiSans-Regular.ttf）
├── data/                    # 本地数据及配置存储
│   ├── settings.json        # 应用程序配置文件（实时保存）
│   └── ...                  # 库、日志和核心运行文件目录
├── src/                     # 源代码目录
│   ├── core/                # 核心逻辑模块（环境配置等）
│   ├── lang/                # 国际化语言模块（en.rs, zh.rs, lang.rs）
│   ├── pages/               # 界面视图模块（如 settings.rs 等）
│   ├── ui/                  # 自定义 UI 组件（如分段控制器）
│   ├── utils.rs             # 通用工具函数
│   └── main.rs              # 应用程序主入口
├── Cargo.toml               # Rust 项目配置和依赖声明
└── README.md                # 项目说明文档
```

## 📂 软件目录结构

```text
 ~/Library/Application Support/AstraBrew Launcher/    ← 根目录 (root)
 ├── data/                   ← 软件数据目录
 │   ├── default/            ← 默认数据目录
 │   │   └── sillytavern/        ← 默认酒馆数据目录
 │   │       └── config.yaml     ← 默认酒馆配置文件
 │   │       └── settings.json   ← 默认酒馆WebUI配置文件
 │   ├── sillytavern/        ← 全局酒馆数据目录
 │   │   └── data/           ← 全局酒馆数据目录
 │   │       ├── config.yaml ← 全局酒馆配置文件
 │   │       └── default-user/
 │   │           └── settings.json ← 全局酒馆WebUI设置
 │   └── local_instances.json ← 本地实例列表
 ├── sillytavern/            ← 酒馆核心文件目录 (ST installation) (对应软件里的`在线下载`实例)
 └── config.json             ← 启动器配置文件

 ~/Library/Logs/AstraBrew Launcher/      ← 日志目录 (logs)

 ~/Library/Caches/AstraBrew Launcher/    ← 缓存目录 (caches)

 /tmp/AstraBrew Launcher/                ← 临时目录 (temp)
```

## 📝 代码规范与注释规范

### AI编程
- 可使用仓库里的`MEMORY.md`喂给AI，辅助开发。

### 代码规范
- **命名规范**：
  - 变量和函数使用 `snake_case`。
  - 结构体、枚举和特征使用 `PascalCase`。
  - 常量和静态变量使用 `SCREAMING_SNAKE_CASE`。
- **模块化**：页面和功能模块需分文件编写，禁止所有逻辑堆砌在主函数或单个文件内。
- **错误处理**：尽量使用 `Result` 和 `Option` 进行错误处理，避免直接使用 `unwrap()` 或 `panic!()` 导致程序崩溃。

### 代码注释规范
- **强制使用中文**进行代码注释。
- **函数和结构体说明**：在复杂的函数和结构体定义前使用 `///` 进行文档注释，解释其用途和参数含义。
- **逻辑块注释**：在复杂的业务逻辑块上方使用 `//` 进行单行注释，说明该段代码的意图。
- 避免冗余和废话注释（如 `// 定义变量` 等无意义的说明）。

## 🤝 贡献指南

我们欢迎并感谢任何形式的贡献！
1. Fork 本仓库。
2. 创建您的特性分支 (`git checkout -b feature/AmazingFeature`)。
3. 提交您的更改 (`git commit -m 'Add some AmazingFeature'`)。
4. 推送到分支 (`git push origin feature/AmazingFeature`)。
5. 开启一个 Pull Request。

在提交代码前，请务必运行 `cargo check` 和 `cargo fmt` 以确保代码符合规范且无编译错误。

## 📄 代码许可

本项目采用 [MIT License](LICENSE) 协议进行开源，允许自由使用、修改和分发，但请保留原作者的版权声明。字体等第三方资源版权归其原作者所有。
