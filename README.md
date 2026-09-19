<img src="https://raw.githubusercontent.com/al01cn/sillyTavern-launcher/GUI/src/assets/images/banner.png" style="width: 100%; height: 100%;" />

# 星酿启动器 (AstraBrew Launcher) · MacOS版本


<div style="text-align: center;" align="center">

星酿启动器 (AstraBrew Launcher) 原为 [酒馆启动器GUI (SillyTavern Launcher GUI)](https://github.com/al01cn/sillyTavern-launcher)，是一款专为小白打造的简单易用的[酒馆(SillyTavern)](https://github.com/sillyTavern/SillyTavern)启动器。基于 Rust 和 iced 开发，旨在为用户提供易用、快速、轻量、多功能的启动和管理体验。

当前仓库单独管理 MacOS版本 的启动器。MacOS 用户可以通过星酿启动器轻松配置和管理酒馆实例，享受一键启动、版本管理、环境配置等功能。我们专注于提供流畅的用户界面和稳定的性能，让每位用户都能轻松上手并愉快使用。

[![Releases](https://img.shields.io/github/v/release/AstraBrew-Labs/AstraBrew-Launcher-Mac?label=版本)](./releases)
[![Rust](https://img.shields.io/badge/Rust-latest-CE422B?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![iced](https://img.shields.io/github/v/release/iced-rs/iced?label=iced)](https://github.com/iced-rs/iced)
[![License](https://img.shields.io/badge/License-MIT-green?style=flat-square)](LICENSE)

[官网](https://launcher.astrabrew.cn) | [Windows版](../../AstraBrew-Launcher-Win/)

</div>

## 📖 项目介绍

星酿启动器提供了一站式的环境配置、一键启动、版本管理以及扩展和资源管理功能。其特点包括：
- **实时响应的界面**：基于 iced 构建的即时模式 GUI，流畅且资源占用低。
- **国际化支持**：目前支持中文 (zh_CN) 和英文 (en_US)。
- **主题切换**：支持深色和浅色模式的动态切换。
- **多屏自适应**：自动处理 16:9 和 4:3 屏幕比例，适配高分辨率显示器及多屏幕切换。
- **环境隔离**：支持内置和系统级别的 Git、Node.js 环境切换，内置包管理和加速代理配置。

## 🛠 技术栈

- **Rust (2024 Edition)**: 核心开发语言，提供内存安全和极致性能。
- **iced (0.14.0)**: 简单易用、响应快速的即时模式 (Immediate mode) GUI 框架。
- **lucide-icons (1.31.0)** : 官方图标库，提供图标的矢量 SVG 格式。

## 运行项目

## 运行

### 普通用户

普通用户请可以直接到[发布页(Releases)](/releases)下载最新版本的DMG安装包，在安装界面把 AstraBrew Launcher.app 拖到 Applications 文件夹中，即可在App里找到星酿启动器 (AstraBrew Launcher) 点击启动。

### 开发者

##  开发环境

本项目使用 Rust 语言进行开发，因此需要安装 Rust 和 Cargo。建议使用最新的 stable 版本，以确保兼容性和性能。

### 前置要求

在开始之前，请确保您的系统已经安装了以下工具：
- [Rust & Cargo](https://www.rust-lang.org/tools/install) (建议使用最新的 stable 版本)
- 因为当前仓库是MacOS的版本，所有开发都按照 MacOS 的规范进行开发。仅支持 MacOS 平台。
- 请勿将其他平台的依赖或配置引入本项目，以避免不必要的兼容性问题。

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
├── src/                     # 源代码目录
│   ├── core/                # 核心逻辑模块（环境配置等）
│   ├── lang/                # 国际化语言模块（en.rs, zh.rs, lang.rs）
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
- 可使用仓库里的`AGENTS.md`喂给AI，辅助开发。

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
