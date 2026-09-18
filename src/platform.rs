//! macOS 平台相关的原生窗口定制。
//!
//! iced 未暴露「禁用绿色缩放按钮」与「禁止独占全屏」的能力，
//! 这里通过 Objective-C 运行时（objc2）直接操作 NSWindow 实现。
//! 应用图标同理：macOS 的图标由 NSApplication 持有，winit 的窗口图标接口在本平台为空实现。

use iced::{Point, Size, window};
use objc2::ClassType;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSImage, NSScreen, NSWindowButton, NSWindowCollectionBehavior};
use objc2_foundation::{NSData, run_on_main};

/// 应用图标：`.icns` 内含 16→1024 的全部尺寸表示，Dock 任意缩放下都清晰。
const APP_ICON_ICNS: &[u8] = include_bytes!("../assets/icon/icon.icns");
/// 备用应用图标：`.icns` 解码失败时退回的 512×512 PNG。
const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon/icon_eframe.png");

/// 启动时使用的窗口位置与固定尺寸。
pub struct InitialWindowPlacement {
    pub position: window::Position,
    pub size: Size,
}

/// 根据当前显示器布局验证历史坐标，并选择对应的固定尺寸档位。
pub fn initial_window_placement(saved: Option<[f32; 2]>) -> InitialWindowPlacement {
    run_on_main(|mtm| {
        let screens = NSScreen::screens(mtm);
        let primary_height = NSScreen::mainScreen(mtm)
            .map(|screen| screen.frame().size.height as f32)
            .unwrap_or(0.0);
        let layouts = screens
            .iter()
            .map(|screen| {
                let frame = screen.frame();
                let visible = screen.visibleFrame();
                let logical = iced::Rectangle {
                    x: visible.origin.x as f32,
                    y: primary_height - (visible.origin.y + visible.size.height) as f32,
                    width: visible.size.width as f32,
                    height: visible.size.height as f32,
                };
                let monitor = Size::new(frame.size.width as f32, frame.size.height as f32);
                (logical, monitor)
            })
            .collect::<Vec<_>>();

        if let Some([x, y]) = saved {
            for (screen, monitor) in &layouts {
                let size = fixed_window_size(*monitor);
                let window_rect = iced::Rectangle::new(Point::new(x, y), size);
                if visible_intersection(window_rect, *screen) {
                    return InitialWindowPlacement {
                        position: window::Position::Specific(Point::new(x, y)),
                        size,
                    };
                }
            }
        }

        let (screen, monitor) = layouts.first().copied().unwrap_or((
            iced::Rectangle::new(Point::ORIGIN, Size::new(1920.0, 1080.0)),
            Size::new(1920.0, 1080.0),
        ));
        let size = fixed_window_size(monitor);
        let centered = Point::new(
            screen.x + ((screen.width - size.width) / 2.0).max(0.0),
            screen.y + ((screen.height - size.height) / 2.0).max(0.0),
        );
        InitialWindowPlacement {
            position: window::Position::Specific(centered),
            size,
        }
    })
}

fn fixed_window_size(monitor: Size) -> Size {
    if monitor.width / monitor.height.max(1.0) >= 1.5 {
        Size::new(1280.0, 720.0)
    } else {
        Size::new(1280.0, 800.0)
    }
}

fn visible_intersection(window: iced::Rectangle, screen: iced::Rectangle) -> bool {
    let left = window.x.max(screen.x);
    let right = (window.x + window.width).min(screen.x + screen.width);
    let top = window.y.max(screen.y);
    let bottom = (window.y + 32.0).min(screen.y + screen.height);
    right - left >= 64.0 && bottom - top >= 24.0
}

#[cfg(test)]
mod tests {
    use super::visible_intersection;
    use iced::{Point, Rectangle, Size};

    fn window(x: f32, y: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(1280.0, 720.0))
    }

    #[test]
    fn accepts_primary_and_negative_coordinate_displays() {
        let primary = Rectangle::new(Point::ORIGIN, Size::new(1920.0, 1040.0));
        let left = Rectangle::new(Point::new(-1920.0, 0.0), Size::new(1920.0, 1080.0));
        assert!(visible_intersection(window(200.0, 100.0), primary));
        assert!(visible_intersection(window(-1800.0, 80.0), left));
    }

    #[test]
    fn rejects_completely_offscreen_and_tiny_titlebar_overlap() {
        let screen = Rectangle::new(Point::ORIGIN, Size::new(1920.0, 1040.0));
        assert!(!visible_intersection(window(2500.0, 100.0), screen));
        assert!(!visible_intersection(window(1880.0, 100.0), screen));
        assert!(visible_intersection(window(1840.0, 100.0), screen));
    }
}

/// 将应用图标写入 Dock 与程序切换器（⌘Tab）。
///
/// macOS 的图标由应用（`NSApplication`）而非窗口（`NSWindow`）持有，
/// winit 的 `set_window_icon` 在本平台是空实现，因此 iced 侧的窗口图标配置不会生效，
/// 必须直接调用 `NSApplication.setApplicationIconImage:`。
///
/// 优先使用多尺寸 `.icns`，解码失败时退回 `.png`。通过 `run_on_main` 保证在主线程执行，
/// 可重复调用，幂等。
pub fn apply_application_icon() {
    run_on_main(|mtm| {
        let Some(image) = application_icon_image() else {
            return;
        };
        // SAFETY: 在主线程内构造并持有的 NSImage；该方法只更新 AppKit 持有的图标引用，
        // 图像对象的生命周期覆盖整个调用过程。
        unsafe {
            NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image));
        }
    });
}

/// 从内嵌资源解码应用图标：`.icns` 优先，失败时用 PNG 兜底。
fn application_icon_image() -> Option<Retained<NSImage>> {
    [APP_ICON_ICNS, APP_ICON_PNG]
        .into_iter()
        .find_map(|bytes| NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes)))
}

/// 禁用应用各窗口的绿色缩放按钮，并移除其「独占全屏」能力。
///
/// 具体做两件事：
/// - 将缩放按钮（红绿灯第三颗）置为不可用，使其无法缩放 / 放大；
/// - 从 `collectionBehavior` 中移除 `FullScreenPrimary`，使窗口无法进入独占全屏。
///
/// 通过 `run_on_main` 保证在主线程执行（AppKit 非线程安全）。可重复调用，幂等。
pub fn disable_zoom_button_and_fullscreen() {
    run_on_main(|mtm| {
        let app = NSApplication::sharedApplication(mtm);
        for window in app.windows().iter() {
            // 禁用绿色缩放按钮
            if let Some(button) = window.standardWindowButton(NSWindowButton::NSWindowZoomButton) {
                button.setEnabled(false);
            }
            // 移除「独占全屏」能力
            unsafe {
                let behavior = window.collectionBehavior();
                window.setCollectionBehavior(
                    behavior - NSWindowCollectionBehavior::FullScreenPrimary,
                );
            }
        }
    });
}
