# 项目长期备忘（astrabrew-launcher-mac）

## 技术栈（重要：本项目是 iced，非 egui/eframe）
- 框架：**iced 0.14.0**（即时模式 GUI），**非** egui/eframe。egui/glow 是 Windows 版（AstraBrew-Launcher-Win）的技术栈，勿混淆。
- 图标：`lucide-icons 1.31.0`（feature `iced`）。
- 组件库：`astra_ui`（GitHub `AstraBrew-Labs/iced-astraui`，branch `beta`，crate 名 `iced-astraui`）。
- 字体：astra_ui 内置 HarmonyOS Sans（六档字重），默认 `astra_ui::fonts::REGULAR`。

## 开发规范（ai.md 硬性要求）
- **仅用 `cargo check`** 验证，禁用 `cargo run` 与测试运行。
- 严格模式：error 与 warning 全部清零。
- 代码注释**必须中文**；`///` 文档注释 + 逻辑块 `//` 注释。
- 仅 macOS 平台；模块化分文件（页面/导航栏禁止堆在单文件）。
- 命名：`snake_case` / `PascalCase` / `SCREAMING_SNAKE_CASE`。
- 窗口不可最大化；16:9 默认 1280×720（最小 800×600）；4:3 默认 800×600（最大 1200×800）。

## astra_ui API 备忘
- crate 根 `pub use components::*`，组件/颜色常量均可 `astra_ui::` 直接导入。
- 主题 `app_theme()`（当前仅浅色）；背景样式 `canvas`（浅灰）、`sidebar`（白底）、`card`、`flat_card`、`code_block`、`tint`。
- 按钮样式 `button_style(ButtonVariant::Primary/Outline/...)`、`nav_button(active)`。
- `Separator::new()` 默认横向 1px，可 `.orientation(Horizontal/Vertical)`、`.variant(Default/Secondary/Tertiary)`、`.thickness()`。
- 字体 `fonts::{REGULAR,MEDIUM,BOLD,BLACK,...}`；图标 `icons::icon(glyph, size, color)`。
- 颜色令牌：`BLUE_600/CYAN_500/INK/INK_MUTED/INK_SUBTLE/SURFACE/SURFACE_ALT/CANVAS/LINE/SUCCESS/WARNING/DANGER/WHITE/NAVY_*`，圆角 `RADIUS_FIELD`(12)/`RADIUS_CONTROL`(24)/`RADIUS_PANEL`(24)。

## 关键经验（Rust/iced）
- 无借用入参、返回 `Element` 的函数须用 `Element<'static, Message>`（不能用 `'_`，会 E0106）；内容均为 `'static` 字符串 + owned `Message`。
- iced `Button`/`button` 默认**不居中内容**（`layout::padded`），自定义按钮内容居中须用 `container(...).width(Fill).height(Fill).align_x/align_y(Center)` 包裹。
- `Column.align_x(Center)` 仅在列为 `Fill`（或固定）宽度时生效；Shrink 列上无效。
- `Task<Option<T>>::and_then(f)` 是特化方法：自动解包 Option，闭包收 `T`，仅 Some 时执行（None 返回空任务），勿再手动 match。

## 窗口配置（iced::window，0.14）
- `window::Settings` 无 `maximizable` 字段；iced 把 `resizable` **同时**映射为 macOS 绿色缩放按钮（`WindowButtons::MAXIMIZE`）与拖拽缩放。
- **禁用绿色缩放按钮 + 禁止独占全屏（最终方案）**：用原生 ObjC。新增 `src/platform.rs` 的 `disable_zoom_button_and_fullscreen()`，经 `objc2_foundation::run_on_main` 保证主线程，遍历 `NSApp.windows()` 对每个窗口 `standardWindowButton(NSWindowZoomButton).setEnabled(false)` + `setCollectionBehavior(behavior - FullScreenPrimary)`。在首开 `MonitorMeasured` 时调用。依赖（与 winit 同版本去重）：`objc2 0.5.2` + `objc2-foundation 0.2.2`(NSArray/NSEnumerator/NSThread/dispatch) + `objc2-app-kit 0.2.2`(NSApplication/NSWindow/NSButton/NSControl)。
- `window::monitor_size(id) -> Task<Option<Size>>`：窗口当前显示器的**逻辑**分辨率。
- `window::events() -> Subscription<(Id, Event)>`（`Opened`/`Rescaled`/`Moved`/`Resized`），配 `Subscription::filter_map`；`window::latest()/resize/set_min_size/set_max_size`。
- 本项目窗口档位：宽屏（宽高比≥1.5，含 16:9/16:10）默认 1280×720、min 800×600、max 1280×720；4:3 默认 800×600、max 1200×800。首开 resize 到默认尺寸；Rescaled（跨屏）只更新 min/max。

## 文件结构
- `src/main.rs`：iced 入口（字体注册、窗口 Settings：默认 1280×720 / min 800×600 / max 1280×720 / Centered / resizable=true / 不最大化）；注册 `mod platform`（cfg macos）。
- `src/app.rs`：应用根，`Screen`（Init/Main）路由 + `Page` 状态 + 初始化流程 + 窗口尺寸档位（`window_profile`）与多屏校准；首开调用 `platform::disable_zoom_button_and_fullscreen()`。
- `src/platform.rs`：macOS 原生窗口定制（禁用绿色缩放按钮、移除独占全屏），基于 objc2。
- `src/sidebar.rs`：左侧导航栏（Logo 占位 + 导航组 + 分割线 + 弹性留白），`SIDEBAR_WIDTH=96`、正方形按钮 `NAV_ITEM_SIZE=72`。
- `src/pages.rs`：`Page` 枚举（title/icon）+ 页面占位视图。
- `src/utils.rs`、`src/core/`、`src/lang/`：待实现（占位）。
