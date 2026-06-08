# MEMORY.md - 项目长期记忆

## 项目基础信息
- **项目名**：AstraBrew Launcher（星酿启动器）
- **技术栈**：Rust + egui/eframe 0.33，egui-phosphor 图标库
- **平台**：仅macOS
- **窗口规格**：默认 1280x720（16:9），最小 800x600，禁用最大化

## 关键目录/文件
- `src/main.rs`：主程序，MyApp 状态管理，eframe::App::update
- `src/pages/settings.rs`：设置页面 UI + SettingsState 数据结构
- `src/core/settings/`：各子系统核心逻辑（git/nodejs/pm2/github_proxy）
- `src/lang/zh.rs` + `src/lang/en.rs`：双语翻译文件
- `data/settings.json`：运行时持久化配置
- `data/github_proxy_cache.json`：GitHub 节点列表缓存（TTL 3天）

## 开发规范
- UI 设计：无滚动条完整显示（特殊场景用 max_height 限制的 ScrollArea）
- 工作流：先分析根因、参考现有模式、再编码
- 配置持久化：`SettingsState::save()` / `load()`（JSON）
- 异步通信：`std::sync::mpsc::channel` + `ctx.request_repaint()`
不要使用cargo run启动项目，也不要用于测试
只能使用cargo check检查项目
代码要严格模式，warning要修复，error要修复。
仅MacOS平台，不用考虑其他平台。

## GitHub 代理功能（2026-05-15）
- 接口：`https://api.akams.cn/github`（每10小时更新，50条节点）
- 返回字段：url / server / ip / location / latency / speed / tag
- 缓存：data/github_proxy_cache.json，3天 TTL
- 延迟测试：多线程 HEAD /favicon.ico，5s 超时
- UI：表格（单选框 / URL / 地区 / 实测延迟 / 速度），颜色编码
- `SettingsState` 新增字段：`github_proxy_enabled: bool` + `github_proxy_url: String`
- reqwest 需要 `json` feature

## 用户偏好
- 风格：干脆直接，不废话
- UI：明确数值约束，无滚动条（或 max_height 限制）
- 语言：中文沟通

## 酒馆配置页面（2026-06-03）
- 从 `.docs/Tavern.vue` 转换为 Rust egui 页面
- 9 个折叠分区：网络与访问、安全与白名单、SSL、CORS、代理与备份、缩略图、性能、日志、会话安全
- 数据结构：`src/core/settings/tavern.rs`（TavernConfig，含 20+ 子结构体）
- UI 页面：`src/pages/tavern_config.rs`（collapsible_section 自定义折叠卡片）
- 配置持久化：`data/tavern_config.json`，手动保存模式
- 状态管理：`TavernConfigUI` 存储于 `MyApp.tavern_config_ui`
- 翻译：中英双语 60+ 个 key（`tc_*` 前缀）
- **serde_yaml 0.9 不支持 `!tag:yaml.org,2002:null`** — 写 YAML null 用 `Value::Null`，不能用 tagged value，否则回读时解析失败 → 全默认值 → 保存覆盖原配置（数据丢失）
- YAML 写入后要确保 serde_yaml 能回读

## 控制台页面 + 进程管理（2026-06-03 初版 / 2026-06-08 进程对接）
- `src/pages/console.rs`：ConsoleState（status + logs + process + instance_path + data_mode）+ render 函数
- `src/core/tavern_process.rs`：TavernProcess — node server.js 子进程管理器
  - 启动：独立模式 `node server.js`，全局模式 `node server.js --configPath <path> --dataRoot <path>`
  - 停止：fork `kill <pid>`（SIGTERM），强制停止：`Child::kill()`（SIGKILL）
  - 日志：后台线程 BufReader + mpsc channel → 主线程轮询
  - Drop 时自动 kill 子进程（启动器关闭 → 酒馆也关闭）
- ConsoleState 方法：`start/stop/force_kill/restart/poll/sync_with_settings/has_instance`
- 状态栏（左右布局）+ 日志区（ScrollArea + stick_to_bottom）
- 按钮组：启动/重启/停止/强行停止，按状态联动启用/禁用，start 需 has_instance()
- 导航位于底部区域，在设置按钮上方（Page::Console）
- 翻译键 `console**` 前缀，中英双语 22 个 key
- main.rs 每帧同步 settings → console_state，每帧 poll()

## 一键启动主页（2026-06-04）
- `src/pages/home.rs`：主页渲染函数 `render(ui, current_page, console_state, lang, version_info)`
- 英雄按钮：停用态"一键启动"（绿）→ 跳转控制台 + 自动启动，运行态"立即停止"（红）→ 跳转控制台 + 优雅关闭
- 底部三列信息卡片：当前版本 / 启动模式 / 服务端口
- `ConsoleState::add_log` 改为 `pub` 供主页调用
- 翻译键 `home_*` 前缀，中英双语 18 个 key

## macOS 路径管理（2026-06-06 更新 — macOS 标准规范）
- `src/utils.rs`：`AppPaths` 结构体 + 全局单例 `app_paths()`
- **生产环境路径（macOS 规范）**：
  - `root`：`~/Library/Application Support/AstraBrew Launcher/`
  - `logs`：`~/Library/Logs/AstraBrew Launcher/`
  - `caches`：`~/Library/Caches/AstraBrew Launcher/`
  - `temp`：`/tmp/AstraBrew Launcher/`（程序结束可清理）
- **开发环境（ASTRA_DEV=1）**：所有路径归一到项目 `data/` 子目录
- **关键文件路径**：
  - 启动器配置：`root/config.json`（`settings_file()`）
  - 内置酒馆配置：`root/sillytavern/config.yaml`（`tavern_config_file()`）
  - 全局酒馆配置：`root/data/sillytavern/data/config.yaml`（`global_tavern_config_file()`）
  - 酒馆配置模板：`data/default/sillytavern/config.yaml`（`tavern_template_file()`）
  - 本地实例列表：`root/data/local_instances.json`（`instances_file()`）
  - GitHub 缓存：`caches/github_proxy_cache.json`
