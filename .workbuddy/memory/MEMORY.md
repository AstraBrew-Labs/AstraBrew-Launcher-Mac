# MEMORY.md - 项目长期记忆

## 项目基础信息
- **项目名**：AstraBrew Launcher（星酿启动器）
- **技术栈**：Rust + egui/eframe 0.33，egui-phosphor 图标库
- **平台**：仅 macOS（不考虑其他平台）
- **窗口规格**：默认 1280x720（16:9），最小 800x600，禁用最大化

## 开发规范（强制）
- **不要用 cargo run 启动项目**，只能用 `cargo check` 检查
- 严格模式：warning 和 error 都必须修复
- UI 设计：无滚动条完整显示（特殊场景用 max_height 限制的 ScrollArea）
- 工作流：先分析根因、参考现有模式、再编码
- 配置持久化：`SettingsState::save()` / `load()`（JSON）
- 异步通信：`std::sync::mpsc::channel` + `ctx.request_repaint()`

## 用户偏好
- 风格：干脆直接，不废话
- 语言：中文沟通

## 关键目录/文件
- `src/main.rs`：主程序，MyApp 状态管理，eframe::App::update
- `src/pages/settings.rs`：设置页面 UI + SettingsState 数据结构
- `src/pages/console.rs`：控制台 + ConsoleState（status/logs/process/instance_path）
- `src/pages/home.rs`：一键启动主页
- `src/pages/reverse_proxy_popup.rs`：反向代理弹窗（参考样板，弹窗模式）
- `src/core/settings/`：各子系统核心逻辑（git/nodejs/pm2/github_proxy）
- `src/core/tavern_process.rs`：TavernProcess — node server.js 子进程管理
- `src/core/pm2.rs`：Pm2Manager — PM2 CLI 封装
- `src/core/desktop_webview.rs`：macOS WKWebView 封装
- `src/utils.rs`：AppPaths 路径管理（macOS 规范）
- `src/lang/zh.rs` + `src/lang/en.rs`：双语翻译文件
- `data/settings.json`：运行时持久化配置

## 设置页面选项变更（2026-06-23）
- 已移除："启动后自动启动酒馆" 选项（`auto_start_tavern`）
- 已移除：相关翻译字符串、`main.rs` 启动逻辑、`console.rs` 引用
- 替换：`auto_start_tavern_skipped` → `tavern_port_in_use`（通用提示）
- `main.rs`：`startup_check_done` 字段已清除

## 版本管理 - 自动扫描权限（2026-06-23）
- 点击"自动扫描"按钮时，若无完全磁盘访问权限（FDA），弹出模态对话框
- 对话框：说明需要 FDA 权限、提供"前往设置"和"仍要继续"按钮
- `VersionManageState` 新增：`show_fda_dialog`、`pending_scan_start`
- 扫描线程在帧末统一启动（避免借用冲突）
- 翻译新增：`fda_access_dialog_desc`、`continue_anyway`

## 软件自启动功能完善（2026-06-23）
- `auto_launch.rs` 新增 `get_auto_launch_status()` 返回 enabled/disabled/requires_approval
- 设置 UI：自启动开关下方显示状态文字（绿色/警告色）
- 状态为 requires_approval 时显示"打开系统设置"按钮
- `Cargo.toml`：`smappservice-rs` 版本号从 `0.2` 修正为 `0.1`
- 翻译新增：`auto_start_enabled`、`auto_start_disabled`、`auto_start_requires_approval`、`open_system_settings`

## macOS 路径管理（src/utils.rs AppPaths）
- `root`：`~/Library/Application Support/AstraBrew Launcher/`
- `logs`：`~/Library/Logs/AstraBrew Launcher/`
- `caches`：`~/Library/Caches/AstraBrew Launcher/`
- `temp`：`/tmp/AstraBrew Launcher/`
- 开发环境（ASTRA_DEV=1）：归一到项目 `data/` 子目录

## 弹窗模式参考（reverse_proxy_popup.rs）
- 全局静态 `LazyLock<Mutex<PopupState>>`
- borrow 模式：先从锁提取值 → 渲染 → 后同步回锁
- egui::Window + Area 渲染

## 访问酒馆弹窗（access_tavern_popup.rs，2026-06-22）
- 服务器模式下"访问酒馆"按钮打开弹窗（非浏览器）
- 流程：打开→loading→后台检测 IPv4/IPv6→布局切换→生成二维码(后台线程+loading遮罩)
- 布局：单地址上下 / 双地址左右对称 / 都失败错误+重试
- IP 检测在 `network.rs`：局域网解析 ifconfig；公网 reqwest+local_address 强制 v4/v6，优先 ip.sb
- ConsoleState 新增 `server_service_mode` 字段（sync_with_settings 参数）
- 异步：mpsc channel + request_repaint；poll_messages 先收集到 Vec 再处理避免借用冲突
- qrcode crate 0.14：`QrCode::new(url).to_colors()` → Vec<Color>，Color::Dark 判断深色

## 控制台/进程管理要点
- ConsoleState：start/stop/force_kill/restart/poll/sync_with_settings/has_instance
- 服务器模式相关：`server_mode`（bool）+ 互联网/局域网模式
- 翻译键 `console_*` / `home_*` / `pm2_*` 前缀

## 桌面模式 WebView 关键依赖与陷阱
- `objc2 0.6`, `objc2-app-kit 0.3`, `objc2-foundation 0.3`, `objc2-web-kit 0.3`, `block2 0.6`
- NSData::alloc() 需要 `use objc2::AnyThread;`
- RcBlock → &DynBlock 转换用 `&*handler`（Deref）
- **objc2 define_class! 协议必需方法**：required 方法（无 `#[optional]`）必须定义在 `unsafe impl Protocol for Type` 块内，否则 debug panic
- **define_class! 限制**：`impl Type { }` 块内方法被当 ObjC 方法（需 `&self`），普通关联函数必须放 `define_class!` 外部
- NSArray 构造：无 `from_vec`，用 `RetainedFromIterator` 的 `collect()`
- NSOpenPanel 文件类型过滤：`setAllowedContentTypes(&NSArray<UTType>)`，需 `objc2-uniform-type-identifiers` feature

## 酒馆配置页面
- `src/core/settings/tavern.rs`：TavernConfig + 20+ 子结构体
- `src/pages/tavern_config.rs`：9 个折叠分区，collapsible_section
- 配置：`data/tavern_config.json`，手动保存
- serde_yaml 0.9 不支持 `!tag:yaml.org,2002:null`，写 null 用 `Value::Null`
- `optimize_after_generate()`：生成配置后改 port=11451 / listen=true / protocol.ipv6=true

## 资源管理页面（src/pages/resource_manage.rs）
- 聊天记录 Tab：`chats/{角色名}/*.jsonl`，CollapsingHeader + 3 列网格 10条/页
- 预设 Tab：`OpenAI Settings/*.json`，3 列卡片网格 + 详情弹窗
- 角色卡/世界书 Tab：类似布局
- 缓存失效：`cached_chats_path` / `cached_chats_mode` 检测路径/模式变化
- `CornerRadius` 字段类型是 `u8` 不是 `f32`

## 打包与自动更新（2026-06-23）
- 打包工具：`cargo-packager` CLI（非 cargo-bundle），配置在 Cargo.toml `[package.metadata.packager]`（camelCase 键）
- 打包命令：`cargo packager --release --private-key keys/update_key.pem`
- 图标：`icons/icon.png` → `icons/icon.icns`（通过 sips + iconutil 转换 10 种尺寸）
- 更新签名密钥：`keys/update_key.pem`（私钥，.gitignore）/ `keys/update_key.pem.pub`（公钥）

### 自动更新模块（src/core/updater.rs）
- 依赖 `cargo-packager-updater = "0.2"`（Cargo.toml）
- 代理回退顺序：gh-proxy.org → ghfast.top → gt.astrabrew.cn → github.com（直连兜底）
- 用户触发：设置 → 关于软件 → "检查更新"按钮 → 弹窗确认 → 下载安装
- 触发机制：`SettingsState.check_update_trigger`/`do_update_trigger` → `main.rs` 处理 → `MyApp.updater_rx` 轮询
- `UpdateStatus` 枚举：Checking/UpToDate/UpdateAvailable{version,notes,endpoint}/Downloading/Installed/Error
- `start_check()` 保留供未来自动检测（当前 #[allow(dead_code)]）
- 发布时需上传 `latest.json`（格式：{version, notes, pub_date, url, signature, format}）+ .app.tar.gz + .sig 到 GitHub Releases
