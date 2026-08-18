//! AstraBrew Launcher 主入口。
//!
//! 负责初始化 iced 应用、注册字体与图标、配置窗口属性。
//! UI 组件统一来自 astra_ui（Astra UI）组件库。

mod app;
mod pages;
#[cfg(target_os = "macos")]
mod platform;
mod sidebar;

use iced::{Size, window};
use lucide_icons::LUCIDE_FONT_BYTES;

fn main() -> iced::Result {
    let mut application = iced::application(
        app::Launcher::new,
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
            // 默认按 16:9 宽屏档位 1280×720；运行时根据实际显示器宽高比再校准
            size: Size::new(1280.0, 720.0),
            // 手动调整大小的下限 / 上限（上限即“不可最大化”到任意尺寸）
            min_size: Some(Size::new(800.0, 600.0)),
            max_size: Some(Size::new(1280.0, 720.0)),
            // 在（主）屏幕上居中显示；副屏断开时 macOS 会自动将窗口移回主屏
            position: window::Position::Centered,
            // 允许手动调整大小，但不以最大化 / 全屏方式启动。
            // 绿色缩放按钮与独占全屏在首开时由 platform::disable_zoom_button_and_fullscreen 关闭。
            resizable: true,
            maximized: false,
            fullscreen: false,
            minimizable: true,
            closeable: true,
            ..window::Settings::default()
        })
        .antialiasing(true)
        .run()
}
