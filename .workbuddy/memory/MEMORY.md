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
- **生成配置自动优化（2026-06-15）**：`optimize_after_generate()` 直接修改 YAML → port=11451 / listen=true / protocol.ipv6=true，小白友好

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

## 聊天记录管理页面（2026-06-11）
- `src/pages/resource_manage.rs` 聊天记录 Tab 完整实现
- 数据来源：`chats/{角色名}/*.jsonl`（独立/全局模式路径不同）
- 文件命名格式：`角色名 - YYYY-M-D @HHh MMm SSs SSSms.jsonl`
- `ChatFileInfo`：filename/filepath/display_time/sort_key
- `ChatGroup`：folder_name/files/expanded/page
- `ChatMessage`：name/is_user/send_date/content（解析自 jsonl 每行 JSON）
- UI：`CollapsingHeader` 可折叠面板（标题=角色文件夹名），面板内 3 列网格，10条/页，新→旧排序
- 点击文件日期 → 弹出聊天查看器（微信/QQ 风格气泡，用户右蓝/角色左灰，30条/页）
- 新增翻译：`ch_viewer_title`, `ch_viewer_messages`
- `CornerRadius` 字段类型是 `u8` 不是 `f32`
- **缓存失效（2026-06-15）**：`load_chats()` 通过 `cached_chats_path` + `cached_chats_mode` 检测路径/模式变化，变化时自动重置 `chats_loaded`。同样修复了 `load_presets()`
- **诊断信息（2026-06-15）**：`chat_scan_debug` 字段 + UI 显示扫描路径、发现文件夹数、跳过原因。扩展名比对改为 `eq_ignore_ascii_case`，目录遍历改为显式错误处理。

## 反向代理弹窗（2026-06-12）
- `src/pages/reverse_proxy_popup.rs`：反向代理设置弹窗
- 入口：设置页面 → 服务器模式开启 + 互联网模式 → "反向代理"行 + "管理"按钮
- 弹窗结构：
  - 顶部总开关 `reverse_proxy_enabled`
  - Tab 1「基本设置」：域名绑定/代理端口/目标地址
  - Tab 2「SSL 设置」：SSL 开关 + 强制 HTTPS 开关 + 左右分栏证书/私钥多行输入框
- 状态：`REVERSE_PROXY_POPUP` 全局静态 `LazyLock<Mutex<ReverseProxyPopupState>>`
- SettingsState 新增 8 个字段：`reverse_proxy_enabled/domain/port/target/ssl_enabled/ssl_force_https/ssl_cert/ssl_key`
- 翻译键 `rp_*` 前缀，中英双语 22 个 key
- 与 Github 测试弹窗相同的 borrow 模式：先提取值 → 渲染 → 后同步回锁

## PM2 管理模块（2026-06-13）
- `src/core/pm2.rs`：Pm2Manager 封装 PM2 CLI
  - 方法：start/stop/restart/delete/get_status/get_logs/clear_logs/update_config
  - Pm2Status 枚举：NotStarted/Online/Stopped/Launching/Stopping/Errored/Unknown
  - pm2 jlist JSON 简易解析（无 serde 依赖），pm2 logs --nostream --raw 获取日志
  - 支持 --node-args "--import interceptor.js" 传递 GitHub 拦截器
  - 支持 HTTP_PROXY 环境变量、--configPath/--dataRoot（全局模式）、--browserLaunchEnabled false（桌面模式）
- `src/pages/console.rs`：ConsoleState 新增 PM2 模式
  - 新增字段：pm2_manager/Pm2Manager、use_pm2/bool、pm2_log_offset/usize
  - `sync_with_settings` 新增 `allow_tavern_background` 参数，检测 PM2 可用性自动切换模式
  - 模式切换时：直接→PM2 会 kill 直接进程；PM2→直接会重置状态
  - start/stop/restart/force_kill 优先走 PM2（use_pm2=true 时）
  - poll_pm2：每帧获取 pm2 jlist 状态 + 增量拉取 pm2 logs
  - 清空按钮同时调用 pm2 flush + 重置偏移量
  - 状态栏 PM2 模式蓝色徽章（CLOUD 图标）
- `src/lang/zh.rs` + `en.rs`：新增 8 个 pm2_* 翻译键
- `src/core/mod.rs`：新增 `pub mod pm2`
- `src/core/tavern_process.rs`：normalize_proxy_url / prepare_interceptor 改为 pub
- `src/main.rs`：sync_with_settings 传入 `allow_tavern_background`

## 预设管理页面（2026-06-14）
- `src/pages/resource_manage.rs` 预设 Tab 完整实现
- 数据来源：`data/default-user/OpenAI Settings/*.json`（独立/全局模式路径不同）
- `PresetInfo`：filename/filepath/name/chat_completion_source/openai_model/claude_model/max_context_unlocked/openai_max_context/openai_max_tokens/stream_openai/prompts/has_spreset
- `PresetPrompt`：name/identifier/system_prompt/enabled/role/content/injection_position/injection_depth/injection_order/forbid_overrides/marker
- UI：参照世界书 3 列卡片网格（120px高），上部名称+SPreset红色tag，下部来源+提示词数量
- 详情弹窗：参照角色卡布局，上信息栏（5 个核心字段+模型），下条目区 2×2/页
- 条目卡片：启用状态/系统提示词tag/标记tag/名称/角色/identifier/forbid_overrides/注入位置/内容
- extensions.SPreset → 红色 "依赖酒馆助手" tag
- 翻译键 `rm_tab_presets` / `ps_*` 前缀，中英双语 29 个 key
