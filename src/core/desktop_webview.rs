//! 桌面模式 WebView 管理器
//!
//! 当启动模式为"桌面模式"时，酒馆启动成功后自动创建原生 WebView 窗口，
//! 以类似桌面应用的方式展示酒馆页面。
//!
//! 设计要点：
//! - macOS 要求 UI 必须在主线程创建。iced 的 NSApp 已在主线程运行，
//!   所以直接用 objc2 创建 NSWindow + WKWebView，参与现有运行循环。
//! - 通过 `isVisible` 轮询检测窗口关闭（由 iced 定时消息轮询，主线程安全）。
//! - Drop 时自动关闭窗口。
//!
//! ## Delegate 实现
//! - **WKNavigationDelegate**：拦截外部链接在默认浏览器打开；检测不可显示的 MIME 类型触发下载
//! - **WKUIDelegate**：处理 `<input type="file">` 文件选择对话框
//! - **WKScriptMessageHandler**：接收 JS 发送的 blob 导出数据（`window.webkit.messageHandlers.fileDownloader`），
//!   解码 base64 后自动保存到配置的导出目录

use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::sync::LazyLock;

use block2::{DynBlock, RcBlock};
use objc2_06::define_class;
use objc2_06::rc::Retained;
use objc2_06::runtime::{AnyObject, ProtocolObject};
use objc2_06::{AnyThread, DefinedClass, MainThreadOnly, msg_send};
use objc2_app_kit_06::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSModalResponseOK, NSOpenPanel,
    NSView, NSWindow, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation_06::{
    MainThreadMarker, NSArray, NSData, NSDataBase64DecodingOptions, NSDictionary, NSError,
    NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_uniform_type_identifiers_06::UTType;
use objc2_web_kit::{
    WKFrameInfo, WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
    WKNavigationResponse, WKNavigationResponsePolicy, WKNavigationType, WKOpenPanelParameters,
    WKScriptMessage, WKScriptMessageHandler, WKUIDelegate, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration,
};

/// WebView 导出文件的规范保存目录。
static EXPORT_PATH: LazyLock<Mutex<PathBuf>> =
    LazyLock::new(|| Mutex::new(default_download_directory()));

/// WebView 下载结果，由启动器根界面显示全局消息。
#[derive(Debug, Clone)]
pub enum WebViewDownloadEvent {
    Saved(PathBuf),
    Failed(String),
}

static DOWNLOAD_NOTIFICATIONS: LazyLock<Mutex<Vec<WebViewDownloadEvent>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// 一次性取出所有下载结果，避免重复显示全局通知。
pub fn drain_download_notifications() -> Vec<WebViewDownloadEvent> {
    std::mem::take(&mut *DOWNLOAD_NOTIFICATIONS.lock().unwrap())
}

/// 最近一次点击的 `<input type="file">` 的 accept 属性，由 JS 注入脚本通过
/// `fileInputTracker` messageHandler 同步发送，供 `run_open_panel` 设置 NSOpenPanel.allowedFileTypes。
///
/// 时序保证：JS click 事件 capture 阶段调用 postMessage → WebKit dispatch_async(主线程)
/// → WebKit 在 click 事件结束后 dispatch_async(主线程) 调用 runOpenPanel。
/// 两次 dispatch_async 按入队顺序执行，故 accept 先于 runOpenPanel 写入。
static LAST_FILE_ACCEPT: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new(String::new()));

// ============================================================================
// WKNavigationDelegate — 外部链接 & 下载处理
// ============================================================================

/// 原生 WebView 导航状态，由 iced 主线程定时消费。
#[derive(Debug, Clone)]
pub enum WebViewEvent {
    Loading,
    Ready(String),
    Failed(String),
    ContentProcessTerminated,
}

#[derive(serde::Deserialize)]
struct WebViewPageProbe {
    href: String,
    title: String,
    html_length: usize,
    body_children: usize,
    body_width: f64,
    body_height: f64,
}

#[derive(Clone)]
struct WebViewNavDelegateIvars {
    events: Sender<WebViewEvent>,
}

define_class!(
    /// 自定义 NavigationDelegate：
    /// - 外部链接 / target="_blank" → 在默认浏览器中打开
    /// - 不可显示的 MIME 类型 → 在默认浏览器中下载
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = WebViewNavDelegateIvars]
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
        #[unsafe(method(webView:didStartProvisionalNavigation:))]
        fn did_start_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
        ) {
            let _ = self.ivars().events.send(WebViewEvent::Loading);
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish_navigation(
            &self,
            web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
        ) {
            // didFinish 也会为初始 about:blank 触发；必须验证真实 DOM 后才能判定加载成功。
            let events = self.ivars().events.clone();
            let completion = RcBlock::new(move |result: *mut AnyObject, error: *mut NSError| {
                if !error.is_null() {
                    let description = unsafe { (&*error).localizedDescription().to_string() };
                    let _ = events.send(WebViewEvent::Failed(format!(
                        "页面状态检查失败：{description}"
                    )));
                    return;
                }
                if result.is_null() {
                    let _ = events.send(WebViewEvent::Failed(
                        "页面状态检查没有返回结果。".to_owned(),
                    ));
                    return;
                }
                let object = unsafe { &*result };
                let Some(value) = object.downcast_ref::<NSString>() else {
                    let _ = events.send(WebViewEvent::Failed(
                        "页面状态检查返回了未知格式。".to_owned(),
                    ));
                    return;
                };
                match serde_json::from_str::<WebViewPageProbe>(&value.to_string()) {
                    Ok(probe)
                        if (probe.href.starts_with("http://")
                            || probe.href.starts_with("https://"))
                            && !probe.title.is_empty()
                            && probe.html_length > 100
                            && probe.body_children > 0
                            && probe.body_width > 0.0
                            && probe.body_height > 0.0 =>
                    {
                        let _ = events.send(WebViewEvent::Ready(probe.href));
                    }
                    Ok(probe) => {
                        let _ = events.send(WebViewEvent::Failed(format!(
                            "页面未完成渲染：url={} title={} html={} children={} size={}x{}",
                            probe.href,
                            probe.title,
                            probe.html_length,
                            probe.body_children,
                            probe.body_width,
                            probe.body_height
                        )));
                    }
                    Err(error) => {
                        let _ = events.send(WebViewEvent::Failed(format!(
                            "无法解析页面状态：{error}"
                        )));
                    }
                }
            });
            let script = NSString::from_str(concat!(
                "JSON.stringify({href:location.href,title:document.title||'',",
                "html_length:document.documentElement?document.documentElement.innerHTML.length:0,",
                "body_children:document.body?document.body.childElementCount:0,",
                "body_width:document.body?document.body.getBoundingClientRect().width:0,",
                "body_height:document.body?document.body.getBoundingClientRect().height:0})"
            ));
            unsafe {
                web_view.evaluateJavaScript_completionHandler(&script, Some(&completion));
            }
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_provisional_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            let _ = self.ivars().events.send(WebViewEvent::Failed(
                error.localizedDescription().to_string(),
            ));
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &NSError,
        ) {
            let _ = self.ivars().events.send(WebViewEvent::Failed(
                error.localizedDescription().to_string(),
            ));
        }

        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn content_process_terminated(&self, _web_view: &WKWebView) {
            let _ = self
                .ivars()
                .events
                .send(WebViewEvent::ContentProcessTerminated);
        }
    }

    unsafe impl NSObjectProtocol for WebViewNavDelegate {}
    unsafe impl WKNavigationDelegate for WebViewNavDelegate {}
);

impl WebViewNavDelegate {
    fn new(mtm: MainThreadMarker, events: Sender<WebViewEvent>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(WebViewNavDelegateIvars { events });
        // SAFETY: NSObject 的 init 签名正确，实例变量已经完成初始化。
        unsafe { msg_send![super(this), init] }
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
        ///
        /// WKOpenPanelParameters 不暴露 HTML `<input accept>` 属性（WebKit API 限制），
        /// 因此通过 JS 注入脚本在 input 点击时通过 `fileInputTracker` messageHandler
        /// 预先把 accept 发送给原生层，这里读取并设置 NSOpenPanel.allowedFileTypes。
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

                // 读取 JS 预先发送的 accept，设置文件类型过滤
                // 仅取扩展名形式（如 .json .png），MIME 类型 / 通配符交给 JS change 校验处理
                let accept = LAST_FILE_ACCEPT.lock().unwrap().clone();
                if !accept.is_empty() {
                    let uttypes: Vec<Retained<UTType>> = accept
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| s.starts_with('.') && s.len() > 1)
                        .filter_map(|s| {
                            UTType::typeWithFilenameExtension(&NSString::from_str(&s[1..]))
                        })
                        .collect();
                    if !uttypes.is_empty() {
                        let ns_types: Retained<NSArray<UTType>> = uttypes.into_iter().collect();
                        panel.setAllowedContentTypes(&ns_types);
                    }
                }

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
        unsafe { core::mem::transmute::<objc2_06::rc::Allocated<Self>, Retained<Self>>(this) }
    }
}

// ============================================================================
// WKScriptMessageHandler — blob 导出文件下载
// ============================================================================

define_class!(
    /// 接收 JS 通过 `webkit.messageHandlers.*.postMessage(...)` 发送的消息
    ///
    /// 当前注册两个 name：
    /// - `fileDownloader`：接收 {filename, base64} 字典，base64 解码后写入导出目录
    /// - `fileInputTracker`：接收 accept 字符串，记录到 `LAST_FILE_ACCEPT` 供 NSOpenPanel 过滤
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
                let name = message.name().to_string();
                match name.as_str() {
                    "fileDownloader" => {
                        handle_file_download(message);
                    }
                    "fileInputTracker" => {
                        handle_file_input_accept(message);
                    }
                    _ => {}
                }
            }
        }
    }

    unsafe impl NSObjectProtocol for FileDownloadHandler {}
);

/// 处理 blob 导出下载：JS postMessage({filename, base64}) → 写入文件
///
/// 注意：这是自由函数而非 FileDownloadHandler 的方法，因为 objc2 define_class! 的
/// `impl Type` 块内方法会被当作 ObjC 方法处理（需要 &self 参数）。
unsafe fn handle_file_download(message: &WKScriptMessage) {
    unsafe {
        let body = message.body();
        // JS postMessage({filename, base64}) → NSDictionary<NSString, NSString>
        let dict: &NSDictionary<NSString, NSString> =
            &*(&*body as *const AnyObject
                as *const NSDictionary<NSString, NSString>);

        let requested_name = dict
            .objectForKey(&NSString::from_str("filename"))
            .map(|name| name.to_string())
            .unwrap_or_else(|| "download".to_owned());
        // 网页提供的文件名不得包含目录，避免覆盖下载目录之外的文件。
        let filename = Path::new(&requested_name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("download")
            .to_owned();

        let base64 = dict
            .objectForKey(&NSString::from_str("base64"))
            .map(|value| value.to_string())
            .unwrap_or_default();
        if base64.is_empty() {
            push_download_event(WebViewDownloadEvent::Failed(
                "下载数据为空。".to_owned(),
            ));
            return;
        }

        let data = NSData::initWithBase64EncodedString_options(
            NSData::alloc(),
            &NSString::from_str(&base64),
            NSDataBase64DecodingOptions(0),
        );
        let Some(data) = data else {
            push_download_event(WebViewDownloadEvent::Failed(
                "下载文件数据损坏。".to_owned(),
            ));
            return;
        };

        let directory = EXPORT_PATH.lock().unwrap().clone();
        if let Err(error) = std::fs::create_dir_all(&directory) {
            push_download_event(WebViewDownloadEvent::Failed(format!(
                "无法创建下载目录 {}：{error}",
                directory.display()
            )));
            return;
        }

        let save_path = available_download_path(&directory, &filename);
        if data.writeToFile_atomically(&NSString::from_str(&save_path.to_string_lossy()), true) {
            push_download_event(WebViewDownloadEvent::Saved(save_path));
        } else {
            push_download_event(WebViewDownloadEvent::Failed(format!(
                "无法写入下载文件：{}",
                save_path.display()
            )));
        }
    }
}

fn push_download_event(event: WebViewDownloadEvent) {
    DOWNLOAD_NOTIFICATIONS.lock().unwrap().push(event);
}

fn default_download_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Downloads")
}

/// 将 `~/Downloads` 等设置转换为真实绝对路径；空值回退系统下载目录。
fn resolve_download_directory(path: &str) -> PathBuf {
    let path = path.trim();
    if path.is_empty() || path == "~" {
        return default_download_directory();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(rest);
    }
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(path)
    }
}

fn available_download_path(directory: &Path, filename: &str) -> PathBuf {
    let requested = directory.join(filename);
    if !requested.exists() {
        return requested;
    }
    let path = Path::new(filename);
    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("download");
    let extension = path.extension().and_then(|value| value.to_str());
    for counter in 1_u32.. {
        let candidate = match extension {
            Some(extension) if !extension.is_empty() => {
                directory.join(format!("{stem}_{counter}.{extension}"))
            }
            _ => directory.join(format!("{stem}_{counter}")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("文件名递增查找必定能够找到可用路径")
}

/// 处理 `<input type="file">` 的 accept 属性：JS postMessage(acceptString)
/// → 记录到 LAST_FILE_ACCEPT，供 run_open_panel 设置 NSOpenPanel.allowedFileTypes
unsafe fn handle_file_input_accept(message: &WKScriptMessage) {
    unsafe {
        let body = message.body();
        // JS postMessage(string) → NSString
        let ns_str: &NSString = &*(&*body as *const AnyObject as *const NSString);
        let accept = ns_str.to_string();
        *LAST_FILE_ACCEPT.lock().unwrap() = accept;
    }
}

impl FileDownloadHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        unsafe {
            core::mem::transmute::<objc2_06::rc::Allocated<Self>, Retained<Self>>(this)
        }
    }
}

// ============================================================================
// DesktopWebView
// ============================================================================

pub struct DesktopWebView {
    window: Retained<NSWindow>,
    /// 显式持有 WebView 的父视图，确保 WebKit 正确进入 AppKit 视图层级。
    _parent_view: Retained<NSView>,
    /// 显式持有 WKWebView，避免只依赖 NSWindow 的间接引用。
    webview: Retained<WKWebView>,
    events: Receiver<WebViewEvent>,
    original_url: String,
    /// 保留当前导航对象，直到下一次加载替换它。
    navigation: Option<Retained<WKNavigation>>,
    /// 保持强引用，因为 WKWebView 对 delegate 是 weak 引用
    _nav_delegate: Retained<WebViewNavDelegate>,
    _ui_delegate: Retained<WebViewUIDelegate>,
    _download_handler: Retained<FileDownloadHandler>,
    running: Arc<AtomicBool>,
}

impl DesktopWebView {
    /// 更新导出文件保存目录。
    ///
    /// 设置页修改 `tavern_export_path` 后每帧调用此方法同步到 WebView，
    /// 这样无需重新打开 WebView 即可让新路径生效。
    pub fn set_export_path(path: &str) {
        *EXPORT_PATH.lock().unwrap() = resolve_download_directory(path);
    }

    /// 在主线程上创建 NSWindow + WKWebView
    ///
    /// - `url`: 酒馆访问地址（如 http://127.0.0.1:8000）
    /// - `title`: 窗口标题（如 "SillyTavern - v1.12.0"）
    /// - `export_path`: 酒馆页面导出文件的保存目录
    ///
    /// 调用者必须确保在主线程上调用此方法。
    pub fn open(url: &str, title: &str, export_path: String) -> Result<Self, String> {
        // 更新 blob 下载目标目录
        Self::set_export_path(&export_path);

        let mtm =
            MainThreadMarker::new().ok_or("桌面模式 WebView 必须在主线程创建")?;
        validate_webview_url(url)?;
        let (event_tx, event_rx) = mpsc::channel();

        // ---- 创建 NSWindow ----
        // WebView 是酒馆的独立内容窗口，沿用旧版行为允许缩放和绿色按钮最大化；
        // 启动器主窗口的固定尺寸约束不应用到该窗口。
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1280.0, 720.0));
        let minimum_size = NSSize::new(800.0, 500.0);

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
        window.setContentMinSize(minimum_size);
        window.center();

        // 关键：用户关闭窗口时不自动释放，由我们的 Retained 管理生命周期
        unsafe { window.setReleasedWhenClosed(false) };

        // ---- 创建 Delegate 对象（保持强引用） ----
        let nav_delegate = WebViewNavDelegate::new(mtm, event_tx);
        let ui_delegate = WebViewUIDelegate::new(mtm);
        let download_handler = FileDownloadHandler::new(mtm);

        // ---- 创建 WKWebView ----
        let config = unsafe { WKWebViewConfiguration::new(mtm) };

        // 注册 JS → Native 通信桥梁
        // - fileDownloader：接收 blob 导出的 {filename, base64}
        // - fileInputTracker：接收 <input type="file"> 的 accept 属性，供 NSOpenPanel 过滤
        unsafe {
            let controller = config.userContentController();
            controller.addScriptMessageHandler_name(
                &ProtocolObject::from_ref(&*download_handler),
                &NSString::from_str("fileDownloader"),
            );
            controller.addScriptMessageHandler_name(
                &ProtocolObject::from_ref(&*download_handler),
                &NSString::from_str("fileInputTracker"),
            );
        }

        // 注入脚本：全面拦截文件下载行为，覆盖 FileSaver.js / 程序触发 a.click() / window.open 等
        //
        // 背景：原方案只拦截用户真实点击 <a href="blob:">，但预设/世界书等导出走 FileSaver.js
        // 等库，通常是程序触发 a.click() 或使用 data: URL，导致拦截失败。本脚本覆写以下入口：
        //   1. 用户真实点击 <a> (capture 阶段)
        //   2. HTMLAnchorElement.prototype.click (FileSaver.js 等库入口)
        //   3. window.open(blob:|data:) (部分库的备选路径)
        // 同时支持 blob: 和 data: 两种 URL scheme。
        let blob_patch_js = concat!(
            "(function(){",
            "function dl(u,f){",
            "fetch(u).then(function(r){return r.blob()}).then(function(b){",
            "var rd=new FileReader();",
            "rd.onloadend=function(){",
            "window.webkit.messageHandlers.fileDownloader.postMessage({",
            "filename:f||'download',",
            "base64:rd.result.split(',')[1]",
            "})};",
            "rd.readAsDataURL(b)",
            "}).catch(function(e){console.error('export err:',e)})",
            "}",
            "function isDl(u){return u&&(u.indexOf('blob:')===0||u.indexOf('data:')===0)}",
            "window.addEventListener('click',function(e){",
            "var a=e.target.closest&&e.target.closest('a');",
            "if(a&&a.href&&isDl(a.href)){",
            "e.preventDefault();e.stopPropagation();",
            "dl(a.href,a.download||'download')",
            "}",
            "},true);",
            "var oc=HTMLAnchorElement.prototype.click;",
            "HTMLAnchorElement.prototype.click=function(){",
            "if(this.href&&isDl(this.href)){",
            "dl(this.href,this.download||'download');return",
            "}",
            "return oc.apply(this,arguments)",
            "};",
            "var oo=window.open;",
            "window.open=function(u){",
            "if(u&&isDl(u)){dl(u,'download');return null}",
            "return oo.apply(window,arguments)",
            "}",
            "})()"
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

        // 注入脚本：恢复 `<input type="file" accept="...">` 的文件类型过滤
        //
        // 背景：自定义 WKUIDelegate::runOpenPanel 创建新的 NSOpenPanel 时，WebKit 不会
        // 自动应用 HTML accept 属性（WKOpenPanelParameters 不暴露该信息）。本脚本：
        //   1. capture 阶段监听 input click，识别导入类型，通过 fileInputTracker
        //      messageHandler 同步发送给原生层（WebKit dispatch_async 保证先于 runOpenPanel）
        //   2. change 事件校验作为后备：若 NSOpenPanel 过滤失效，在文件选中后再次校验，
        //      不匹配则清空 input.value 并提示
        //
        // 手动指定类型规则（不依赖酒馆 DOM 结构）：
        //   - 角色卡导入：accept 含 png / image → 强制 .png,.json
        //   - 世界书/预设导入：accept 含 json → 强制 .json
        //   - 其他：用原 accept
        let file_input_filter_js = concat!(
            "(function(){",
            // 类型识别：根据 input 的 accept 属性归类
            "function pickType(input){",
            "var acc=(input.getAttribute('accept')||'').toLowerCase();",
            // 角色卡：通常 accept="image/png,.png,application/json,.json" 或 .json
            // 但有的角色卡 import 按钮 accept 只写 .json，需结合上下文判断
            // 这里用 accept 内容做硬规则
            "if(acc.indexOf('png')>=0||acc.indexOf('image/')>=0){return '.png,.json'}",
            "if(acc.indexOf('json')>=0){return '.json'}",
            // 兜底：用原 accept
            "return acc",
            "}",
            // 1. 点击 input[type=file] 时，把识别出的类型发送给原生层
            "document.addEventListener('click',function(e){",
            "var t=e.target;",
            "if(!t||t.tagName!=='INPUT'||(t.type||'').toLowerCase()!=='file')return;",
            "var acc=pickType(t);",
            "try{window.webkit.messageHandlers.fileInputTracker.postMessage(acc)}catch(err){}",
            "},true);",
            // 2. change 事件校验（后备，与 pickType 规则保持一致）
            "document.addEventListener('change',function(e){",
            "var t=e.target;",
            "if(!t||t.tagName!=='INPUT'||(t.type||'').toLowerCase()!=='file')return;",
            "if(!t.files||!t.files.length)return;",
            "var acc=pickType(t);",
            "if(!acc)return;",
            "var exts=[],any=false;",
            "acc.split(',').forEach(function(p){",
            "p=p.trim().toLowerCase();",
            "if(!p)return;",
            "if(p.charAt(0)==='.'){exts.push(p.slice(1))}",
            "else if(p==='*/*'||p==='*'||p.indexOf('/*')>=0){any=true}",
            "});",
            "if(any)return;",
            "if(!exts.length)return;",
            "var bad=[];",
            "for(var i=0;i<t.files.length;i++){",
            "var f=t.files[i];",
            "var n=(f.name||'').toLowerCase();",
            "var ok=exts.some(function(x){return n.lastIndexOf('.'+x)===n.length-x.length-1});",
            "if(!ok){bad.push(f.name)}",
            "}",
            "if(bad.length){",
            "t.value='';",
            "alert('以下文件类型不被允许：\\n'+bad.join('\\n')+'\\n\\n允许的类型：'+acc)",
            "}",
            "},true)",
            "})()"
        );
        let file_filter_script = unsafe {
            WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                WKUserScript::alloc(mtm),
                &NSString::from_str(file_input_filter_js),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            )
        };
        unsafe {
            let controller = config.userContentController();
            controller.addUserScript(&file_filter_script);
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
            webview.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            webview.setWantsLayer(true);
        }

        // WKWebView 不直接作为 NSWindow.contentView，而是挂载到普通 NSView。
        // 该结构与 WebKit 桌面应用的标准做法一致，可确保内容进程真正发起网络请求和绘制。
        let parent_view = NSView::initWithFrame(NSView::alloc(mtm), rect);
        parent_view.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        parent_view.setWantsLayer(true);
        parent_view.addSubview(&webview);
        window.setContentView(Some(&parent_view));
        window.makeFirstResponder(Some(&webview));
        window.makeKeyAndOrderFront(None);

        let application = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        application.activateIgnoringOtherApps(true);

        let navigation = match load_webview_url(&webview, url) {
            Ok(navigation) => navigation,
            Err(error) => {
                window.close();
                return Err(error);
            }
        };

        Ok(Self {
            window,
            _parent_view: parent_view,
            webview,
            events: event_rx,
            original_url: url.to_owned(),
            navigation: Some(navigation),
            _nav_delegate: nav_delegate,
            _ui_delegate: ui_delegate,
            _download_handler: download_handler,
            running: Arc::new(AtomicBool::new(true)),
        })
    }

    /// 拉取导航代理产生的状态事件，不阻塞 iced 主线程。
    pub fn drain_events(&self) -> Vec<WebViewEvent> {
        self.events.try_iter().collect()
    }

    /// 重新加载页面；需要时把 localhost 回退为 IPv4 回环地址。
    pub fn reload(&mut self, use_loopback_fallback: bool) -> Result<(), String> {
        let target = if use_loopback_fallback {
            loopback_fallback_url(&self.original_url)
        } else {
            self.original_url.clone()
        };
        self.navigation = Some(load_webview_url(&self.webview, &target)?);
        Ok(())
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
    /// 由 iced 定时消息轮询，主线程安全。
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

fn validate_webview_url(url: &str) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("WebView 地址协议无效：{url}"));
    }
    NSURL::URLWithString(&NSString::from_str(url))
        .map(|_| ())
        .ok_or_else(|| format!("WebView 地址无效：{url}"))
}

fn load_webview_url(
    webview: &WKWebView,
    url: &str,
) -> Result<Retained<WKNavigation>, String> {
    let nsurl = NSURL::URLWithString(&NSString::from_str(url))
        .ok_or_else(|| format!("WebView 地址无效：{url}"))?;
    let request = NSURLRequest::requestWithURL(&nsurl);
    unsafe { webview.loadRequest(&request) }
        .ok_or_else(|| format!("WebView 无法创建导航请求：{url}"))
}

fn loopback_fallback_url(url: &str) -> String {
    url.replacen("://localhost", "://127.0.0.1", 1)
}

impl Drop for DesktopWebView {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_download_directory, loopback_fallback_url, resolve_download_directory,
        validate_webview_url,
    };

    #[test]
    fn validates_http_urls_and_rejects_other_schemes() {
        assert!(validate_webview_url("http://localhost:8000/").is_ok());
        assert!(validate_webview_url("https://127.0.0.1:8000/").is_ok());
        assert!(validate_webview_url("file:///tmp/index.html").is_err());
    }

    #[test]
    fn default_download_setting_expands_to_home_downloads() {
        assert_eq!(resolve_download_directory("~/Downloads"), default_download_directory());
        assert_eq!(resolve_download_directory(""), default_download_directory());
    }

    #[test]
    fn localhost_retry_uses_ipv4_loopback() {
        assert_eq!(
            loopback_fallback_url("http://localhost:11451/"),
            "http://127.0.0.1:11451/"
        );
        assert_eq!(
            loopback_fallback_url("http://192.168.1.2:11451/"),
            "http://192.168.1.2:11451/"
        );
    }
}
