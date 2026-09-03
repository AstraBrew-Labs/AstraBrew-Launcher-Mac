//! AstraBrew Launcher 主入口。
//!
//! 负责初始化 iced 应用、注册字体与图标、配置窗口属性。
//! UI 组件统一来自 astra_ui（Astra UI）组件库。

mod app;
mod core;
mod lang;
mod pages;
#[cfg(target_os = "macos")]
mod platform;
mod sidebar;
mod theme;

use iced::window;
use lucide_icons::LUCIDE_FONT_BYTES;

fn main() -> iced::Result {
    let (settings_store, preferences) = core::settings::SettingsStore::load_default();
    let saved_position = preferences
        .remember_window_position
        .then_some(preferences.window_position)
        .flatten();
    let placement = platform::initial_window_placement(saved_position);
    let initial_store = settings_store.clone();

    let mut application = iced::application(
        move || app::Launcher::new(initial_store.clone(), preferences),
        app::Launcher::update,
        app::Launcher::view,
    )
    .title(app::Launcher::title)
    .subscription(app::Launcher::subscription)
    .theme(app::Launcher::theme)
    .font(LUCIDE_FONT_BYTES);

    // 注册 astra_ui 内置的 HarmonyOS Sans 字体（六档字重）
    for (_, bytes) in astra_ui::fonts::FONT_MAPPINGS {
        application = application.font(bytes);
    }

    application
        .default_font(astra_ui::fonts::REGULAR)
        .window(window::Settings {
            // 启动前已经按显示器比例选择固定尺寸，并验证历史窗口坐标。
            size: placement.size,
            min_size: Some(placement.size),
            max_size: Some(placement.size),
            position: placement.position,
            // 窗口尺寸固定，不以最大化 / 全屏方式启动。
            // 绿色缩放按钮与独占全屏在首开时由 platform::disable_zoom_button_and_fullscreen 关闭。
            resizable: false,
            maximized: false,
            fullscreen: false,
            minimizable: true,
            closeable: true,
            // 关闭事件交给应用保存窗口位置后再显式退出。
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .antialiasing(true)
        .run()
}
