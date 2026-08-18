//! macOS 平台相关的原生窗口定制。
//!
//! iced 未暴露「禁用绿色缩放按钮」与「禁止独占全屏」的能力，
//! 这里通过 Objective-C 运行时（objc2）直接操作 NSWindow 实现。

use objc2_app_kit::{NSApplication, NSWindowButton, NSWindowCollectionBehavior};
use objc2_foundation::run_on_main;

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
