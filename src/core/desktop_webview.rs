//! 桌面模式 WebView 管理器
//!
//! 当启动模式为"桌面模式"时，酒馆启动成功后自动创建原生 WebView 窗口，
//! 以类似桌面应用的方式展示酒馆页面。
//!
//! 设计要点：
//! - macOS 要求 UI 必须在主线程创建。eframe 的 NSApp 已在主线程运行，
//!   所以直接用 objc2 创建 NSWindow + WKWebView，参与现有运行循环。
//! - 通过 `isVisible` 轮询检测窗口关闭（每帧在 egui update 中调用，主线程安全）。
//! - Drop 时自动关闭窗口。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use objc2::rc::Retained;
use objc2::MainThreadOnly;
use objc2_app_kit::{NSBackingStoreType, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest};
use objc2_web_kit::{WKWebView, WKWebViewConfiguration};

pub struct DesktopWebView {
    window: Retained<NSWindow>,
    running: Arc<AtomicBool>,
}

impl DesktopWebView {
    /// 在主线程上创建 NSWindow + WKWebView
    ///
    /// - `url`: 酒馆访问地址（如 http://127.0.0.1:8000）
    /// - `title`: 窗口标题（如 "SillyTavern - v1.12.0"）
    ///
    /// 调用者必须确保在主线程上调用此方法。
    pub fn open(url: &str, title: &str) -> Result<Self, String> {
        let mtm =
            MainThreadMarker::new().ok_or("桌面模式 WebView 必须在主线程创建")?;

        // ---- 创建 NSWindow ----
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1280.0, 720.0));
        let min_size = NSSize::new(800.0, 500.0);

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setTitle(&NSString::from_str(title));
        window.setContentMinSize(min_size);
        window.center();

        // 关键：用户关闭窗口时不自动释放，由我们的 Retained 管理生命周期
        unsafe { window.setReleasedWhenClosed(false) };

        // ---- 创建 WKWebView ----
        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        let webview = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1280.0, 720.0)),
                &config,
            )
        };

        // 加载 URL
        let ns_url = NSString::from_str(url);
        if let Some(nsurl) = NSURL::URLWithString(&ns_url) {
            let request = NSURLRequest::requestWithURL(&nsurl);
            unsafe { webview.loadRequest(&request) };
        }

        // WebView 填入窗口
        window.setContentView(Some(&webview));

        window.makeKeyAndOrderFront(None);

        Ok(Self {
            window,
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// 主动关闭 WebView 窗口（仅当窗口仍可见时）
    pub fn close(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // 窗口已被用户关闭时不再重复 close，否则 segfault
        if self.window.isVisible() {
            self.window.close();
        }
    }

    /// 将 WebView 窗口唤回前台（避免重复打开新窗口）
    pub fn bring_to_front(&self) {
        self.window.makeKeyAndOrderFront(None);
    }

    /// 检查 WebView 窗口是否已被关闭（用户点击关闭按钮 或 程序主动关闭）
    ///
    /// 每帧在 egui update 中调用，主线程安全。
    /// 返回 `true` 表示窗口已关闭。
    pub fn is_closed(&self) -> bool {
        // 主动关闭时 running=false，用户关闭时 isVisible=false
        !self.running.load(Ordering::SeqCst) || !self.window.isVisible()
    }

    /// WebView 是否仍在运行
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for DesktopWebView {
    fn drop(&mut self) {
        self.close();
    }
}
