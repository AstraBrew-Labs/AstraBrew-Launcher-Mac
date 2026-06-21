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
//!
//! ## Delegate 实现
//! - **WKNavigationDelegate**：拦截外部链接在默认浏览器打开；检测不可显示的 MIME 类型触发下载
//! - **WKUIDelegate**：处理 `<input type="file">` 文件选择对话框
//! - **WKScriptMessageHandler**：接收 JS 发送的 blob 导出数据（`window.webkit.messageHandlers.fileDownloader`），
//!   解码 base64 后自动保存到配置的导出目录

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::LazyLock;

use block2::DynBlock;
use objc2::define_class;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSModalResponseOK, NSOpenPanel, NSWindow, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSData, NSDataBase64DecodingOptions, NSDictionary, NSObject,
    NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKFrameInfo, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
    WKNavigationResponse, WKNavigationResponsePolicy, WKNavigationType, WKOpenPanelParameters,
    WKScriptMessage, WKScriptMessageHandler, WKUIDelegate, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration,
};

/// blob: URL 下载的目标目录，由 DesktopWebView::open 设置
static EXPORT_PATH: LazyLock<Mutex<String>> = LazyLock::new(|| {
    Mutex::new(
        std::env::var("HOME")
            .map(|h| format!("{}/Downloads", h))
            .unwrap_or_default(),
    )
});

/// blob 下载结果通知队列，由 main.rs 每帧轮询并弹出 Toast
pub static DOWNLOAD_NOTIFICATIONS: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

// ============================================================================
// WKNavigationDelegate — 外部链接 & 下载处理
// ============================================================================

define_class!(
    /// 自定义 NavigationDelegate：
    /// - 外部链接 / target="_blank" → 在默认浏览器中打开
    /// - 不可显示的 MIME 类型 → 在默认浏览器中下载
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct WebViewNavDelegate;

    impl WebViewNavDelegate {
        /// 决策导航动作：区分内部/外部链接
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decide_policy_for_navigation_action(
            &self,
            web_view: &WKWebView,
            navigation_action: &WKNavigationAction,
            decision_handler: &DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            unsafe {
                let nav_type = navigation_action.navigationType();
                let request = navigation_action.request();
                let target_frame = navigation_action.targetFrame();

                // 判断是否需要在默认浏览器打开
                let should_open_externally = if nav_type == WKNavigationType::LinkActivated {
                    // targetFrame 为 nil 表示 target="_blank" / 新窗口
                    if target_frame.is_none() {
                        true
                    } else {
                        // 比较当前页面 host 与目标 URL host，不同则视为外部链接
                        let request_url = request.URL();
                        match (web_view.URL(), &request_url) {
                            (Some(cur), Some(req)) => {
                                let cur_host = cur.host();
                                let req_host = req.host();
                                cur_host != req_host
                                    || cur_host.is_none()
                                    || req_host.is_none()
                            }
                            _ => false,
                        }
                    }
                } else {
                    false
                };

                if should_open_externally {
                    if let Some(url) = request.URL() {
                        let workspace = NSWorkspace::sharedWorkspace();
                        workspace.openURL(&url);
                    }
                    decision_handler.call((WKNavigationActionPolicy::Cancel,));
                } else {
                    decision_handler.call((WKNavigationActionPolicy::Allow,));
                }
            }
        }

        /// 决策导航响应：检测不可显示的 MIME 类型 → 触发下载
        #[unsafe(method(webView:decidePolicyForNavigationResponse:decisionHandler:))]
        fn decide_policy_for_navigation_response(
            &self,
            _web_view: &WKWebView,
            navigation_response: &WKNavigationResponse,
            decision_handler: &DynBlock<dyn Fn(WKNavigationResponsePolicy)>,
        ) {
            unsafe {
                if navigation_response.canShowMIMEType() {
                    decision_handler.call((WKNavigationResponsePolicy::Allow,));
                } else {
                    // WKWebView 无法显示此 MIME 类型 → 在默认浏览器中打开以下载
                    let response = navigation_response.response();
                    if let Some(url) = response.URL() {
                        let workspace = NSWorkspace::sharedWorkspace();
                        workspace.openURL(&url);
                    }
                    decision_handler.call((WKNavigationResponsePolicy::Cancel,));
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for WebViewNavDelegate {}
    unsafe impl WKNavigationDelegate for WebViewNavDelegate {}
);

impl WebViewNavDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        // SAFETY: alloc returns +1 retain count. For a delegate without
        // custom ivars, NSObject::init is a no-op. We skip calling it
        // to avoid an objc2 0.5/0.6 version conflict in msg_send!.
        // Both Allocated<T> and Retained<T> are #[repr(transparent)]
        // over a single pointer, so transmute is safe here.
        unsafe { core::mem::transmute::<objc2::rc::Allocated<Self>, Retained<Self>>(this) }
    }
}

// ============================================================================
// WKUIDelegate — 文件上传对话框
// ============================================================================

define_class!(
    /// 自定义 UIDelegate：处理 `<input type="file">` 文件选择
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct WebViewUIDelegate;

    impl WebViewUIDelegate {
        /// 显示文件选择面板（文件导入）
        #[unsafe(method(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:))]
        fn run_open_panel(
            &self,
            _web_view: &WKWebView,
            parameters: &WKOpenPanelParameters,
            _frame: &WKFrameInfo,
            completion_handler: &DynBlock<dyn Fn(*mut NSArray<NSURL>)>,
        ) {
            unsafe {
                let mtm = MainThreadMarker::new()
                    .expect("UIDelegate::runOpenPanel must be on main thread");

                let panel = NSOpenPanel::openPanel(mtm);

                // 根据网页表单参数配置面板
                panel.setCanChooseFiles(true);
                panel.setAllowsMultipleSelection(parameters.allowsMultipleSelection());
                panel.setCanChooseDirectories(parameters.allowsDirectories());

                let result = panel.runModal();

                if result == NSModalResponseOK {
                    let urls = panel.URLs();
                    // 将所有权转移给 WebKit
                    completion_handler.call((Retained::into_raw(urls),));
                } else {
                    completion_handler.call((ptr::null_mut(),));
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for WebViewUIDelegate {}
    unsafe impl WKUIDelegate for WebViewUIDelegate {}
);

impl WebViewUIDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        // 同 WebViewNavDelegate::new 的理由
        unsafe { core::mem::transmute::<objc2::rc::Allocated<Self>, Retained<Self>>(this) }
    }
}

// ============================================================================
// WKScriptMessageHandler — blob 导出文件下载
// ============================================================================

define_class!(
    /// 接收 JS 通过 `webkit.messageHandlers.fileDownloader.postMessage(...)` 发送的 blob 数据
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct FileDownloadHandler;

    // 注意：WKScriptMessageHandler 的 userContentController:didReceiveScriptMessage: 是
    // required 方法，必须定义在 `unsafe impl WKScriptMessageHandler` 块内，否则 objc2
    // define_class! 宏在 debug 构建下会 panic（协议必需方法未在协议块中注册）。
    unsafe impl WKScriptMessageHandler for FileDownloadHandler {
        #[allow(non_snake_case)]
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn userContentController_didReceiveScriptMessage(
            &self,
            _user_content_controller: &WKUserContentController,
            message: &WKScriptMessage,
        ) {
            unsafe {
                let body = message.body();
                // JS postMessage({filename, base64}) → NSDictionary<NSString, NSString>
                let dict: &NSDictionary<NSString, NSString> =
                    &*(&*body as *const AnyObject
                        as *const NSDictionary<NSString, NSString>);

                let filename = dict
                    .objectForKey(&NSString::from_str("filename"))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "download".to_string());

                let b64_str = dict
                    .objectForKey(&NSString::from_str("base64"))
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if b64_str.is_empty() {
                    DOWNLOAD_NOTIFICATIONS
                        .lock()
                        .unwrap()
                        .push("导出失败：数据为空".into());
                    return;
                }

                // Base64 → NSData
                let b64_ns = NSString::from_str(&b64_str);
                let data = NSData::initWithBase64EncodedString_options(
                    NSData::alloc(),
                    &b64_ns,
                    NSDataBase64DecodingOptions(0),
                );
                let data = match data {
                    Some(d) => d,
                    None => {
                        DOWNLOAD_NOTIFICATIONS
                            .lock()
                            .unwrap()
                            .push("导出失败：文件数据损坏".into());
                        return;
                    }
                };

                // 保存到导出目录
                let downloads = EXPORT_PATH.lock().unwrap().clone();

                // 处理文件名冲突
                use std::path::Path;
                let mut save_path = format!("{}/{}", downloads, filename);
                if Path::new(&save_path).exists() {
                    let stem = Path::new(&filename)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&filename);
                    let ext = Path::new(&filename)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    let mut counter: u32 = 1;
                    loop {
                        let candidate = if ext.is_empty() {
                            format!("{}/{}_{}", downloads, stem, counter)
                        } else {
                            format!("{}/{}_{}.{}", downloads, stem, counter, ext)
                        };
                        if !Path::new(&candidate).exists() {
                            save_path = candidate;
                            break;
                        }
                        counter += 1;
                    }
                }

                let _ = std::fs::create_dir_all(&downloads);
                let path_ns = NSString::from_str(&save_path);
                if data.writeToFile_atomically(&path_ns, true) {
                    let display_name = save_path
                        .rsplit_once('/')
                        .map(|(_, name)| name)
                        .unwrap_or(&save_path);
                    DOWNLOAD_NOTIFICATIONS
                        .lock()
                        .unwrap()
                        .push(format!("已导出: {}", display_name));
                } else {
                    DOWNLOAD_NOTIFICATIONS
                        .lock()
                        .unwrap()
                        .push("导出失败：无法写入文件".into());
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for FileDownloadHandler {}
);

impl FileDownloadHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        unsafe {
            core::mem::transmute::<objc2::rc::Allocated<Self>, Retained<Self>>(this)
        }
    }
}

// ============================================================================
// DesktopWebView
// ============================================================================

pub struct DesktopWebView {
    window: Retained<NSWindow>,
    /// 保持强引用，因为 WKWebView 对 delegate 是 weak 引用
    _nav_delegate: Retained<WebViewNavDelegate>,
    _ui_delegate: Retained<WebViewUIDelegate>,
    _download_handler: Retained<FileDownloadHandler>,
    running: Arc<AtomicBool>,
}

impl DesktopWebView {
    /// 在主线程上创建 NSWindow + WKWebView
    ///
    /// - `url`: 酒馆访问地址（如 http://127.0.0.1:8000）
    /// - `title`: 窗口标题（如 "SillyTavern - v1.12.0"）
    /// - `export_path`: 酒馆页面导出文件的保存目录
    ///
    /// 调用者必须确保在主线程上调用此方法。
    pub fn open(url: &str, title: &str, export_path: String) -> Result<Self, String> {
        // 更新 blob 下载目标目录
        *EXPORT_PATH.lock().unwrap() = export_path;

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

        // ---- 创建 Delegate 对象（保持强引用） ----
        let nav_delegate = WebViewNavDelegate::new(mtm);
        let ui_delegate = WebViewUIDelegate::new(mtm);
        let download_handler = FileDownloadHandler::new(mtm);

        // ---- 创建 WKWebView ----
        let config = unsafe { WKWebViewConfiguration::new(mtm) };

        // 注册 JS → Native 通信桥梁
        unsafe {
            let controller = config.userContentController();
            controller.addScriptMessageHandler_name(
                &ProtocolObject::from_ref(&*download_handler),
                &NSString::from_str("fileDownloader"),
            );
        }

        // 注入脚本：拦截 <a href="blob:..."> 点击，fetch 转 base64 后通过 messageHandler 发送给原生层
        let blob_patch_js = concat!(
            "window.addEventListener('click',function(e){",
            "var a=e.target.closest('a');",
            "if(a&&a.href&&a.href.startsWith('blob:')){",
            "e.preventDefault();",
            "var u=a.href;",
            "var f=a.download||'download';",
            "fetch(u).then(function(r){return r.blob()}).then(function(b){",
            "var rd=new FileReader();",
            "rd.onloadend=function(){",
            "window.webkit.messageHandlers.fileDownloader.postMessage({",
            "filename:f,",
            "base64:rd.result.split(',')[1]",
            "})};",
            "rd.readAsDataURL(b)",
            "})",
            "}},true)",
        );
        let user_script = unsafe {
            WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                WKUserScript::alloc(mtm),
                &NSString::from_str(blob_patch_js),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            )
        };
        unsafe {
            let controller = config.userContentController();
            controller.addUserScript(&user_script);
        }

        let webview = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1280.0, 720.0)),
                &config,
            )
        };

        // 设置 delegates（从 Retained 创建 ProtocolObject 引用）
        unsafe {
            webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*nav_delegate)));
            webview.setUIDelegate(Some(ProtocolObject::from_ref(&*ui_delegate)));
        }

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
            _nav_delegate: nav_delegate,
            _ui_delegate: ui_delegate,
            _download_handler: download_handler,
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
