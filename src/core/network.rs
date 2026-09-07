//! 网络相关功能：系统代理读取、GitHub 多地址连通性与下载测试。

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GITHUB_CLONE_URL: &str = "https://github.com/SillyTavern/SillyTavern.git";
const GITHUB_DOWNLOAD_URL: &str =
    "https://github.com/SillyTavern/SillyTavern/archive/refs/tags/1.18.0.tar.gz";
const TEST_ROOT_DIR: &str = "AstraBrew Launcher";

/// 酒馆核心下载渠道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DownloadChannel {
    #[default]
    Auto,
    Mirror1,
    Mirror2,
    Mirror3,
    Official,
}

impl DownloadChannel {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Mirror1 => "mirror1",
            Self::Mirror2 => "mirror2",
            Self::Mirror3 => "mirror3",
            Self::Official => "official",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "自动",
            Self::Mirror1 => "镜像 1",
            Self::Mirror2 => "镜像 2",
            Self::Mirror3 => "镜像 3",
            Self::Official => "官方",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Auto => "自动选择速度最快且可用的下载渠道。",
            Self::Mirror1 => "官方仓库的镜像，国内速度比较快，但版本同步会晚一些。",
            Self::Mirror2 => "官方仓库的备用镜像，国内速度较快，但版本同步会晚一些。",
            Self::Mirror3 => "官方仓库的备用镜像，国内速度较快，但版本同步会慢很多。",
            Self::Official => "官方仓库直连，国内速度较慢，但版本更新最快。",
        }
    }

    pub const fn repository_url(self) -> &'static str {
        match self {
            Self::Auto => "https://github.com/sillyTavern/SillyTavern",
            Self::Mirror1 => "https://gitee.com/AstraBrew-Labs/SillyTavern",
            Self::Mirror2 => "https://gitcode.com/GitHub_Trending/si/SillyTavern",
            Self::Mirror3 => "https://cnb.cool/AstraBrew-Labs/SillyTavern",
            Self::Official => "https://github.com/sillyTavern/SillyTavern",
        }
    }

    pub const fn clone_url(self) -> &'static str {
        match self {
            Self::Auto => Self::Official.clone_url(),
            Self::Mirror1 => "https://gitee.com/AstraBrew-Labs/SillyTavern.git",
            Self::Mirror2 => "https://gitcode.com/GitHub_Trending/si/SillyTavern.git",
            Self::Mirror3 => "https://cnb.cool/AstraBrew-Labs/SillyTavern.git",
            Self::Official => GITHUB_CLONE_URL,
        }
    }

    pub const fn fixed_channels() -> [Self; 4] {
        [Self::Mirror1, Self::Mirror2, Self::Mirror3, Self::Official]
    }

    pub fn from_key(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "mirror1" | "mirror_1" | "镜像1" | "镜像 1" => Self::Mirror1,
            "mirror2" | "mirror_2" | "镜像2" | "镜像 2" => Self::Mirror2,
            "mirror3" | "mirror_3" | "镜像3" | "镜像 3" => Self::Mirror3,
            "official" | "官方" => Self::Official,
            _ => Self::Auto,
        }
    }

    pub fn display_label(self, resolved: Option<Self>) -> String {
        match (self, resolved) {
            (Self::Auto, Some(channel)) if channel != Self::Auto => format!(
                "{}（{}）",
                crate::lang::display_label(self.label()),
                crate::lang::display_label(channel.label())
            ),
            _ => crate::lang::display_label(self.label()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DownloadChannelTestResult {
    pub channel: DownloadChannel,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DownloadChannelTestEvent {
    ChannelStarted {
        channel: DownloadChannel,
    },
    CloneProgress {
        channel: DownloadChannel,
        stage: String,
        current: Option<u64>,
        total: Option<u64>,
        percentage: Option<f32>,
    },
    ChannelFinished(DownloadChannelTestResult),
    Completed {
        selected: DownloadChannel,
        results: Vec<DownloadChannelTestResult>,
        all_failed: bool,
    },
}

/// 自动下载渠道测速结果的缓存，保存在 macOS 的 Caches 目录而不是设置目录。
#[derive(Debug, Clone)]
pub struct DownloadChannelCache {
    pub resolved_channel: DownloadChannel,
    pub tested_at: u64,
    pub results: Vec<DownloadChannelTestResult>,
}

impl DownloadChannelCache {
    pub fn is_valid_at(&self, now: u64) -> bool {
        self.resolved_channel != DownloadChannel::Auto
            && now.saturating_sub(self.tested_at) < 7 * 24 * 60 * 60
    }
}

fn cache_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// 自动下载渠道缓存文件：`~/Library/Caches/AstraBrew Launcher/download_channel_cache.json`。
pub fn download_channel_cache_path() -> PathBuf {
    cache_home_dir()
        .join("Library")
        .join("Caches")
        .join(TEST_ROOT_DIR)
        .join("download_channel_cache.json")
}

fn unix_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

/// 读取自动下载渠道缓存；文件不存在、损坏或字段不完整时返回 None。
pub fn load_download_channel_cache() -> Option<DownloadChannelCache> {
    let contents = fs::read_to_string(download_channel_cache_path()).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    let object = value.as_object()?;
    let resolved_channel = object
        .get("resolved_channel")
        .and_then(serde_json::Value::as_str)
        .map(DownloadChannel::from_key)?;
    let tested_at = object
        .get("tested_at")
        .and_then(serde_json::Value::as_u64)?;
    let results = object
        .get("results")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    Some(DownloadChannelTestResult {
                        channel: DownloadChannel::from_key(item.get("channel")?.as_str()?),
                        success: item.get("success")?.as_bool()?,
                        latency_ms: item.get("latency_ms").and_then(serde_json::Value::as_u64),
                        error: item
                            .get("error")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(DownloadChannelCache {
        resolved_channel,
        tested_at,
        results,
    })
}

/// 保存自动下载渠道测速结果到 Caches 目录，不写入 settings.json。
pub fn save_download_channel_cache(
    selected: DownloadChannel,
    results: &[DownloadChannelTestResult],
) -> io::Result<DownloadChannelCache> {
    let tested_at = unix_seconds().unwrap_or_default();
    let value = serde_json::json!({
        "resolved_channel": selected.key(),
        "tested_at": tested_at,
        "results": results.iter().map(|result| serde_json::json!({
            "channel": result.channel.key(),
            "success": result.success,
            "latency_ms": result.latency_ms,
            "error": result.error,
        })).collect::<Vec<_>>(),
    });
    let path = download_channel_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&value).map_err(io::Error::other)?,
    )?;
    fs::rename(&temporary, &path)?;
    Ok(DownloadChannelCache {
        resolved_channel: selected,
        tested_at,
        results: results.to_vec(),
    })
}

/// 通过 `scutil --proxy` 读取 macOS 系统代理设置。
///
/// 优先 HTTPS 代理，回退 HTTP 代理；如果系统代理明确未启用，返回
/// `Some(("".to_owned(), false))`，读取失败时回退环境变量。
pub fn read_system_proxy() -> Option<(String, bool)> {
    if let Some(output) = Command::new("scutil").arg("--proxy").output().ok()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let text_lower = text.to_lowercase();
        if !text_lower.contains("not configured") && !text_lower.contains("no such") {
            let parsed = parse_system_proxy_output(&text);
            if let Some(proxy) = parsed.active_proxy {
                return Some((proxy, true));
            }
            if let Some(server) = environment_proxy() {
                return Some((server, true));
            }
            if parsed.configured {
                return Some((String::new(), false));
            }
        }
    }

    environment_proxy().map(|server| (server, true))
}

fn environment_proxy() -> Option<String> {
    ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
        .into_iter()
        .find_map(|key| {
            let value = std::env::var(key).ok()?;
            (!value.trim().is_empty()).then_some(value)
        })
}

struct ParsedSystemProxy {
    active_proxy: Option<String>,
    configured: bool,
}

fn parse_system_proxy_output(text: &str) -> ParsedSystemProxy {
    let mut https_enable = false;
    let mut https_proxy = String::new();
    let mut https_port: u16 = 0;
    let mut http_enable = false;
    let mut http_proxy = String::new();
    let mut http_port: u16 = 0;
    let mut configured = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("HTTPSEnable : ") {
            configured = true;
            https_enable = val.trim() == "1";
        } else if let Some(val) = trimmed.strip_prefix("HTTPSProxy : ") {
            configured = true;
            https_proxy = val.trim().to_owned();
        } else if let Some(val) = trimmed.strip_prefix("HTTPSPort : ") {
            configured = true;
            https_port = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = trimmed.strip_prefix("HTTPEnable : ") {
            configured = true;
            http_enable = val.trim() == "1";
        } else if let Some(val) = trimmed.strip_prefix("HTTPProxy : ") {
            configured = true;
            http_proxy = val.trim().to_owned();
        } else if let Some(val) = trimmed.strip_prefix("HTTPPort : ") {
            configured = true;
            http_port = val.trim().parse().unwrap_or(0);
        }
    }

    let active_proxy = if https_enable && !https_proxy.is_empty() {
        Some(if https_port > 0 {
            format!("{https_proxy}:{https_port}")
        } else {
            https_proxy
        })
    } else if http_enable && !http_proxy.is_empty() {
        Some(if http_port > 0 {
            format!("{http_proxy}:{http_port}")
        } else {
            http_proxy
        })
    } else {
        None
    };

    ParsedSystemProxy {
        active_proxy,
        configured,
    }
}

fn normalize_proxy_url(proxy: &str) -> Result<String, &'static str> {
    let proxy = proxy.trim();
    if proxy.is_empty() {
        return Err("自定义代理地址不能为空。");
    }
    if proxy.starts_with("http://")
        || proxy.starts_with("https://")
        || proxy.starts_with("socks5://")
    {
        Ok(proxy.to_owned())
    } else {
        Ok(format!("http://{proxy}"))
    }
}

fn normalize_system_proxy_url(proxy: &str) -> Option<String> {
    if proxy.contains('=') {
        let entries = proxy.split(';').filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((key, value))
        });
        let mut http = None;
        for (key, value) in entries {
            if key == "https" {
                return normalize_proxy_url(value).ok();
            }
            if key == "http" {
                http = Some(value);
            }
        }
        return normalize_proxy_url(http?).ok();
    }
    normalize_proxy_url(proxy).ok()
}

fn selected_proxy_url(proxy_mode: &str, proxy_host: &str) -> Result<Option<String>, String> {
    match proxy_mode {
        "custom" => normalize_proxy_url(proxy_host)
            .map(Some)
            .map_err(str::to_owned),
        "system" => Ok(read_system_proxy()
            .filter(|(_, enabled)| *enabled)
            .and_then(|(server, _)| normalize_system_proxy_url(&server))),
        _ => Ok(None),
    }
}

pub(crate) fn build_client(
    proxy_mode: &str,
    proxy_host: &str,
) -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .user_agent("AstraBrew-Launcher-macOS");
    if let Some(proxy_url) = selected_proxy_url(proxy_mode, proxy_host)? {
        let proxy = reqwest::Proxy::all(&proxy_url)
            .map_err(|error| format!("代理地址格式无效：{error}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|error| format!("构建网络客户端失败：{error}"))
}

/// 单个 GitHub 测试结果。
#[derive(Debug, Clone)]
pub struct GithubMultiTestItem {
    pub key: String,
    pub name: String,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

/// 测试线程发给 iced UI 的实时事件。
#[derive(Debug, Clone)]
pub enum GithubTestEvent {
    ItemStarted {
        key: String,
        name: String,
    },
    CloneProgress {
        stage: String,
        current: Option<u64>,
        total: Option<u64>,
        percentage: Option<f32>,
    },
    DownloadProgress {
        total_bytes: Option<u64>,
        downloaded_bytes: u64,
        bytes_per_second: u64,
        percentage: Option<f32>,
    },
    ItemFinished(GithubMultiTestItem),
    Completed(Vec<GithubMultiTestItem>),
}

fn emit(sender: &Option<Sender<GithubTestEvent>>, event: GithubTestEvent) {
    if let Some(sender) = sender {
        let _ = sender.send(event);
    }
}

fn test_urls(include_api: bool) -> Vec<(String, String, String)> {
    let mut urls = vec![
        (
            "raw".to_owned(),
            "文件访问".to_owned(),
            "https://raw.githubusercontent.com/SillyTavern/SillyTavern/release/start.sh".to_owned(),
        ),
        (
            "repo".to_owned(),
            "仓库访问".to_owned(),
            "https://github.com/SillyTavern/SillyTavern".to_owned(),
        ),
        (
            "homepage".to_owned(),
            "首页访问".to_owned(),
            "https://www.github.com".to_owned(),
        ),
    ];
    if include_api {
        urls.push((
            "api".to_owned(),
            "API 访问".to_owned(),
            "https://api.github.com/repos/SillyTavern/SillyTavern/releases".to_owned(),
        ));
    }
    urls
}

fn accelerated_url(url: &str, accelerate_url: Option<&str>) -> String {
    accelerate_url
        .map(|accelerate| format!("{}/{}", accelerate.trim_end_matches('/'), url))
        .unwrap_or_else(|| url.to_owned())
}

fn channelize_url(url: &str, channel: DownloadChannel) -> String {
    let github_repo = "https://github.com/SillyTavern/SillyTavern";
    let lower = url.to_ascii_lowercase();
    let github_prefix = github_repo.to_ascii_lowercase();
    if lower.starts_with(&github_prefix) {
        let suffix = &url[github_repo.len()..];
        format!("{}{}", channel.repository_url(), suffix)
    } else {
        url.to_owned()
    }
}

fn download_url(channel: DownloadChannel) -> &'static str {
    match channel {
        DownloadChannel::Mirror1 => {
            "https://gitee.com/AstraBrew-Labs/SillyTavern/archive/refs/tags/1.18.0.tar.gz"
        }
        DownloadChannel::Mirror2 => {
            "https://gitcode.com/GitHub_Trending/si/SillyTavern/archive/refs/tags/1.18.0.tar.gz"
        }
        DownloadChannel::Mirror3 => {
            "https://cnb.cool/AstraBrew-Labs/SillyTavern/archive/refs/tags/1.18.0.tar.gz"
        }
        DownloadChannel::Auto | DownloadChannel::Official => GITHUB_DOWNLOAD_URL,
    }
}

fn failed_results(error: &str, include_api: bool) -> Vec<GithubMultiTestItem> {
    let mut items = test_urls(include_api)
        .into_iter()
        .map(|(key, name, _)| GithubMultiTestItem {
            key,
            name,
            success: false,
            latency_ms: None,
            error: Some(error.to_owned()),
            warning: None,
        })
        .collect::<Vec<_>>();
    items.push(GithubMultiTestItem {
        key: "clone".to_owned(),
        name: "仓库克隆".to_owned(),
        success: false,
        latency_ms: None,
        error: Some(error.to_owned()),
        warning: None,
    });
    items.push(GithubMultiTestItem {
        key: "speed".to_owned(),
        name: "下载速度".to_owned(),
        success: false,
        latency_ms: None,
        error: Some(error.to_owned()),
        warning: None,
    });
    items
}

/// 同步执行完整测试并返回最终结果。主要用于测试和非 UI 调用。
#[allow(dead_code)]
pub fn test_github_multi(
    proxy_mode: &str,
    proxy_host: &str,
    _proxy_port: u16,
    accelerate_url: Option<String>,
    include_api: bool,
) -> Vec<GithubMultiTestItem> {
    let (sender, receiver) = std::sync::mpsc::channel();
    run_github_test(
        proxy_mode,
        proxy_host,
        accelerate_url,
        include_api,
        Some(sender),
    );
    receiver
        .into_iter()
        .find_map(|event| match event {
            GithubTestEvent::Completed(results) => Some(results),
            _ => None,
        })
        .unwrap_or_else(|| failed_results("测试未能完成。", include_api))
}

/// 在后台线程中执行完整 GitHub 测试，并通过事件发送实时进度。
pub fn run_github_test(
    proxy_mode: &str,
    proxy_host: &str,
    accelerate_url: Option<String>,
    include_api: bool,
    sender: Option<Sender<GithubTestEvent>>,
) {
    run_github_test_with_cancel_for_channel(
        proxy_mode,
        proxy_host,
        DownloadChannel::Official,
        accelerate_url,
        include_api,
        sender,
        Arc::new(AtomicBool::new(false)),
    );
}

/// 支持取消信号的 GitHub 测试入口。
#[allow(dead_code)]
pub fn run_github_test_with_cancel(
    proxy_mode: &str,
    proxy_host: &str,
    accelerate_url: Option<String>,
    include_api: bool,
    sender: Option<Sender<GithubTestEvent>>,
    cancel: Arc<AtomicBool>,
) {
    run_github_test_with_cancel_for_channel(
        proxy_mode,
        proxy_host,
        DownloadChannel::Official,
        accelerate_url,
        include_api,
        sender,
        cancel,
    );
}

/// 使用指定酒馆下载渠道执行完整 GitHub 连通性测试。
pub fn run_github_test_with_cancel_for_channel(
    proxy_mode: &str,
    proxy_host: &str,
    channel: DownloadChannel,
    accelerate_url: Option<String>,
    include_api: bool,
    sender: Option<Sender<GithubTestEvent>>,
    cancel: Arc<AtomicBool>,
) {
    let mut results = Vec::new();
    let accelerate = accelerate_url.as_deref();
    let client = match build_client(proxy_mode, proxy_host) {
        Ok(client) => client,
        Err(error) => {
            let results = failed_results(&error, include_api);
            for item in &results {
                emit(&sender, GithubTestEvent::ItemFinished(item.clone()));
            }
            emit(&sender, GithubTestEvent::Completed(results));
            return;
        }
    };

    for (key, name, url) in test_urls(include_api) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let item = test_http_endpoint(
            &client, &key, &name, &url, channel, accelerate, &sender, &cancel,
        );
        emit(&sender, GithubTestEvent::ItemFinished(item.clone()));
        results.push(item);
    }

    let clone_item = test_git_clone(
        proxy_mode, proxy_host, channel, accelerate, &sender, &cancel,
    );
    emit(&sender, GithubTestEvent::ItemFinished(clone_item.clone()));
    results.push(clone_item);

    let download_item = test_download(
        &client, proxy_mode, proxy_host, channel, accelerate, &sender, &cancel,
    );
    emit(
        &sender,
        GithubTestEvent::ItemFinished(download_item.clone()),
    );
    results.push(download_item);

    emit(&sender, GithubTestEvent::Completed(results));
}

fn test_http_endpoint(
    client: &reqwest::blocking::Client,
    key: &str,
    name: &str,
    url: &str,
    channel: DownloadChannel,
    accelerate_url: Option<&str>,
    sender: &Option<Sender<GithubTestEvent>>,
    cancel: &Arc<AtomicBool>,
) -> GithubMultiTestItem {
    emit(
        sender,
        GithubTestEvent::ItemStarted {
            key: key.to_owned(),
            name: name.to_owned(),
        },
    );
    if cancel.load(Ordering::Relaxed) {
        return cancelled_item(key, name);
    }
    let request_url = accelerated_url(&channelize_url(url, channel), accelerate_url);
    let start = Instant::now();
    match client.get(request_url).send() {
        Ok(mut response) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = response.status();
            let mut success = status.is_success();
            let mut warning = None;
            let mut error = None;

            if !success {
                if let Some(accelerate_url) = accelerate_url {
                    let status_code = status.as_u16();
                    if status_code == 403 || status_code == 404 {
                        success = true;
                        warning = Some(format!("加速地址可用，但该资源无法加速 ({status_code})"));
                    } else {
                        let mut body = String::new();
                        let _ = response.read_to_string(&mut body);
                        let lower = body.to_lowercase();
                        if lower.contains("invalid input") || lower.contains("无效输入") {
                            success = true;
                            warning = Some("加速地址可用，但该资源无法加速".to_owned());
                        } else {
                            error = Some(format!("HTTP {status}（加速地址：{accelerate_url}）"));
                        }
                    }
                } else {
                    error = Some(format!("HTTP {status}"));
                }
            }

            GithubMultiTestItem {
                key: key.to_owned(),
                name: name.to_owned(),
                success,
                latency_ms: Some(latency),
                error,
                warning,
            }
        }
        Err(error) => GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: None,
            error: Some(format!("连接失败：{error}")),
            warning: None,
        },
    }
}

fn resolve_command(name: &str) -> String {
    ["/opt/homebrew/bin", "/usr/local/bin"]
        .into_iter()
        .map(|base| format!("{base}/{name}"))
        .find(|path| std::path::Path::new(path).is_file())
        .unwrap_or_else(|| name.to_owned())
}

fn configure_git_proxy(command: &mut Command, proxy_mode: &str, proxy_host: &str) {
    match selected_proxy_url(proxy_mode, proxy_host).ok().flatten() {
        Some(proxy) => {
            command.arg("-c").arg(format!("http.proxy={proxy}"));
            command.arg("-c").arg(format!("https.proxy={proxy}"));
        }
        None => {
            command.arg("-c").arg("http.proxy=");
            command.arg("-c").arg("https.proxy=");
        }
    }
}

fn unique_test_root() -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let suffix = format!("{}-{timestamp}", std::process::id());
    std::path::PathBuf::from("/tmp")
        .join(TEST_ROOT_DIR)
        .join(suffix)
}

fn test_git_clone(
    proxy_mode: &str,
    proxy_host: &str,
    channel: DownloadChannel,
    accelerate_url: Option<&str>,
    sender: &Option<Sender<GithubTestEvent>>,
    cancel: &Arc<AtomicBool>,
) -> GithubMultiTestItem {
    let key = "clone";
    let name = "仓库克隆";
    emit(
        sender,
        GithubTestEvent::ItemStarted {
            key: key.to_owned(),
            name: name.to_owned(),
        },
    );

    if cancel.load(Ordering::Relaxed) {
        return cancelled_item(key, name);
    }
    let root = unique_test_root();
    let clone_path = root.join("SillyTavern");
    if let Err(error) = fs::create_dir_all(&root) {
        return GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: None,
            error: Some(format!("无法创建临时目录：{error}")),
            warning: None,
        };
    }

    let url = accelerated_url(channel.clone_url(), accelerate_url);
    let start = Instant::now();
    let mut command = Command::new(resolve_command("git"));
    configure_git_proxy(&mut command, proxy_mode, proxy_host);
    command
        .args(["clone", "--progress"])
        .arg(url)
        .arg(&clone_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_dir_all(&root);
            return GithubMultiTestItem {
                key: key.to_owned(),
                name: name.to_owned(),
                success: false,
                latency_ms: None,
                error: Some(format!("无法启动 git：{error}")),
                warning: None,
            };
        }
    };

    let mut last_message = None;
    let progress_receiver = child.stderr.take().map(spawn_progress_reader);
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir_all(&root);
            return cancelled_item(key, name);
        }

        if let Some(receiver) = &progress_receiver {
            while let Ok(line) = receiver.try_recv() {
                if let Some(progress) = parse_git_progress(&line) {
                    emit(
                        sender,
                        GithubTestEvent::CloneProgress {
                            stage: progress.stage,
                            current: progress.current,
                            total: progress.total,
                            percentage: progress.percentage,
                        },
                    );
                } else if !line.starts_with("warning:") && !line.is_empty() {
                    last_message = Some(line);
                }
            }
        }

        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                last_message = Some(format!("等待 git clone 状态失败：{error}"));
                break;
            }
        }
    }
    if let Some(receiver) = &progress_receiver {
        while let Ok(line) = receiver.try_recv() {
            if let Some(progress) = parse_git_progress(&line) {
                emit(
                    sender,
                    GithubTestEvent::CloneProgress {
                        stage: progress.stage,
                        current: progress.current,
                        total: progress.total,
                        percentage: progress.percentage,
                    },
                );
            } else if !line.starts_with("warning:") && !line.is_empty() {
                last_message = Some(line);
            }
        }
    }

    let status = child.wait();
    let elapsed = start.elapsed().as_millis() as u64;
    let result = match status {
        Ok(status) if status.success() => GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: true,
            latency_ms: Some(elapsed),
            error: None,
            warning: None,
        },
        Ok(status) => GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: Some(elapsed),
            error: Some(last_message.unwrap_or_else(|| {
                format!("git clone 失败（退出码：{}）", status.code().unwrap_or(-1))
            })),
            warning: None,
        },
        Err(error) => GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: Some(elapsed),
            error: Some(format!("等待 git clone 结束失败：{error}")),
            warning: None,
        },
    };
    let _ = fs::remove_dir_all(&root);
    result
}

struct GitProgress {
    stage: String,
    current: Option<u64>,
    total: Option<u64>,
    percentage: Option<f32>,
}

fn parse_git_progress(line: &str) -> Option<GitProgress> {
    let cleaned_line = strip_git_ansi(line);
    let line = cleaned_line.trim().trim_start_matches("remote: ").trim();
    let (stage, detail) = line.split_once(':')?;
    let stage = stage.trim();
    if !matches!(
        stage,
        "Enumerating objects"
            | "Counting objects"
            | "Compressing objects"
            | "Receiving objects"
            | "Resolving deltas"
    ) {
        return None;
    }

    let percentage = detail.split_once('%').and_then(|(value, _)| {
        value
            .split_whitespace()
            .last()
            .and_then(|value| value.parse::<f32>().ok())
    });
    let (current, total) = detail
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .and_then(|(counts, _)| counts.split_once('/'))
        .map(|(current, total)| {
            (
                current.trim().parse::<u64>().ok(),
                total.trim().parse::<u64>().ok(),
            )
        })
        .unwrap_or((None, None));
    let percentage = percentage.or_else(|| {
        current.zip(total).and_then(|(current, total)| {
            (total > 0).then_some(current as f32 / total as f32 * 100.0)
        })
    });

    Some(GitProgress {
        stage: stage.to_owned(),
        current,
        total,
        percentage,
    })
}

fn strip_git_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(next) = chars.next() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn spawn_progress_reader(mut stderr: impl Read + Send + 'static) -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let mut pending = String::new();
        loop {
            let read = match stderr.read(&mut buffer) {
                Ok(read) => read,
                Err(_) => break,
            };
            if read == 0 {
                break;
            }
            pending.push_str(&String::from_utf8_lossy(&buffer[..read]));
            while let Some(index) = pending.find(['\r', '\n']) {
                let line = pending[..index].to_owned();
                pending.drain(..=index);
                let _ = sender.send(line);
            }
        }
        if !pending.is_empty() {
            let _ = sender.send(pending);
        }
    });
    receiver
}

fn emit_download_channel(
    sender: &Option<Sender<DownloadChannelTestEvent>>,
    event: DownloadChannelTestEvent,
) {
    if let Some(sender) = sender {
        let _ = sender.send(event);
    }
}

/// 对全部固定酒馆下载渠道执行轻量 Git 克隆测速。
pub fn run_download_channel_test(
    proxy_mode: &str,
    proxy_host: &str,
    sender: Option<Sender<DownloadChannelTestEvent>>,
    cancel: Arc<AtomicBool>,
) {
    let mut results = Vec::new();
    for channel in DownloadChannel::fixed_channels() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        emit_download_channel(
            &sender,
            DownloadChannelTestEvent::ChannelStarted { channel },
        );
        let result = test_download_channel_clone(channel, proxy_mode, proxy_host, &sender, &cancel);
        emit_download_channel(
            &sender,
            DownloadChannelTestEvent::ChannelFinished(result.clone()),
        );
        results.push(result);
    }

    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let selected = fastest_download_channel(&results).unwrap_or(DownloadChannel::Official);
    let all_failed = results.iter().all(|result| !result.success);
    emit_download_channel(
        &sender,
        DownloadChannelTestEvent::Completed {
            selected,
            results,
            all_failed,
        },
    );
}

fn fastest_download_channel(results: &[DownloadChannelTestResult]) -> Option<DownloadChannel> {
    results
        .iter()
        .filter(|result| result.success)
        .filter_map(|result| result.latency_ms.map(|latency| (result.channel, latency)))
        .min_by_key(|(_, latency)| *latency)
        .map(|(channel, _)| channel)
}

fn test_download_channel_clone(
    channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
    sender: &Option<Sender<DownloadChannelTestEvent>>,
    cancel: &Arc<AtomicBool>,
) -> DownloadChannelTestResult {
    let root = unique_test_root();
    let clone_path = root.join(channel.key());
    if let Err(error) = fs::create_dir_all(&root) {
        return DownloadChannelTestResult {
            channel,
            success: false,
            latency_ms: None,
            error: Some(format!("无法创建临时目录：{error}")),
        };
    }

    let start = Instant::now();
    let mut command = Command::new(resolve_command("git"));
    configure_git_proxy(&mut command, proxy_mode, proxy_host);
    command
        .args(["clone", "--progress", "--depth=1", "--no-tags"])
        .arg(channel.clone_url())
        .arg(&clone_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_dir_all(&root);
            return DownloadChannelTestResult {
                channel,
                success: false,
                latency_ms: None,
                error: Some(format!("无法启动 git：{error}")),
            };
        }
    };

    let progress_receiver = child.stderr.take().map(spawn_progress_reader);
    let mut last_message = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir_all(&root);
            return DownloadChannelTestResult {
                channel,
                success: false,
                latency_ms: None,
                error: Some("测速已取消".to_owned()),
            };
        }
        if let Some(receiver) = &progress_receiver {
            while let Ok(line) = receiver.try_recv() {
                if let Some(progress) = parse_git_progress(&line) {
                    emit_download_channel(
                        sender,
                        DownloadChannelTestEvent::CloneProgress {
                            channel,
                            stage: progress.stage,
                            current: progress.current,
                            total: progress.total,
                            percentage: progress.percentage,
                        },
                    );
                } else if !line.starts_with("warning:") && !line.trim().is_empty() {
                    last_message = Some(line);
                }
            }
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                last_message = Some(format!("等待 git clone 状态失败：{error}"));
                break;
            }
        }
    }

    if let Some(receiver) = &progress_receiver {
        while let Ok(line) = receiver.try_recv() {
            if let Some(progress) = parse_git_progress(&line) {
                emit_download_channel(
                    sender,
                    DownloadChannelTestEvent::CloneProgress {
                        channel,
                        stage: progress.stage,
                        current: progress.current,
                        total: progress.total,
                        percentage: progress.percentage,
                    },
                );
            } else if !line.starts_with("warning:") && !line.trim().is_empty() {
                last_message = Some(line);
            }
        }
    }

    let status = child.wait();
    let elapsed = start.elapsed().as_millis() as u64;
    let result = match status {
        Ok(status) if status.success() => DownloadChannelTestResult {
            channel,
            success: true,
            latency_ms: Some(elapsed),
            error: None,
        },
        Ok(status) => DownloadChannelTestResult {
            channel,
            success: false,
            latency_ms: Some(elapsed),
            error: Some(last_message.unwrap_or_else(|| {
                format!("git clone 失败（退出码：{}）", status.code().unwrap_or(-1))
            })),
        },
        Err(error) => DownloadChannelTestResult {
            channel,
            success: false,
            latency_ms: Some(elapsed),
            error: Some(format!("等待 git clone 结束失败：{error}")),
        },
    };
    let _ = fs::remove_dir_all(&root);
    result
}

fn cancelled_item(key: &str, name: &str) -> GithubMultiTestItem {
    GithubMultiTestItem {
        key: key.to_owned(),
        name: name.to_owned(),
        success: false,
        latency_ms: None,
        error: Some("测试已取消".to_owned()),
        warning: None,
    }
}

fn test_download(
    client: &reqwest::blocking::Client,
    _proxy_mode: &str,
    _proxy_host: &str,
    channel: DownloadChannel,
    accelerate_url: Option<&str>,
    sender: &Option<Sender<GithubTestEvent>>,
    cancel: &Arc<AtomicBool>,
) -> GithubMultiTestItem {
    let key = "speed";
    let name = "下载速度";
    emit(
        sender,
        GithubTestEvent::ItemStarted {
            key: key.to_owned(),
            name: name.to_owned(),
        },
    );

    if cancel.load(Ordering::Relaxed) {
        return cancelled_item(key, name);
    }
    let root = unique_test_root();
    let path = root.join("SillyTavern-1.18.0.tar.gz");
    if let Err(error) = fs::create_dir_all(&root) {
        return GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: None,
            error: Some(format!("无法创建下载临时目录：{error}")),
            warning: None,
        };
    }

    let url = accelerated_url(download_url(channel), accelerate_url);
    let start = Instant::now();
    let response = client.get(url).send();
    let result = match response {
        Ok(mut response) if response.status().is_success() => {
            let total_bytes = response.content_length();
            let mut file = match File::create(&path) {
                Ok(file) => file,
                Err(error) => {
                    let _ = fs::remove_dir_all(&root);
                    return GithubMultiTestItem {
                        key: key.to_owned(),
                        name: name.to_owned(),
                        success: false,
                        latency_ms: None,
                        error: Some(format!("无法创建下载文件：{error}")),
                        warning: None,
                    };
                }
            };
            let mut downloaded_bytes = 0u64;
            let mut last_emit = Instant::now();
            let mut last_emit_bytes = 0u64;
            let mut buffer = [0u8; 32 * 1024];
            let mut read_error = None;

            emit(
                sender,
                GithubTestEvent::DownloadProgress {
                    total_bytes,
                    downloaded_bytes: 0,
                    bytes_per_second: 0,
                    percentage: Some(0.0).filter(|_| total_bytes.is_some()),
                },
            );

            loop {
                if cancel.load(Ordering::Relaxed) {
                    let _ = fs::remove_dir_all(&root);
                    return cancelled_item(key, name);
                }
                match response.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        if let Err(error) = file.write_all(&buffer[..read]) {
                            read_error = Some(format!("写入下载文件失败：{error}"));
                            break;
                        }
                        downloaded_bytes += read as u64;
                        let elapsed = last_emit.elapsed();
                        if elapsed >= Duration::from_millis(100) {
                            let speed = ((downloaded_bytes - last_emit_bytes) as f64
                                / elapsed.as_secs_f64())
                                as u64;
                            emit(
                                sender,
                                GithubTestEvent::DownloadProgress {
                                    total_bytes,
                                    downloaded_bytes,
                                    bytes_per_second: speed,
                                    percentage: total_bytes.map(|total| {
                                        (downloaded_bytes as f32 / total.max(1) as f32 * 100.0)
                                            .min(100.0)
                                    }),
                                },
                            );
                            last_emit = Instant::now();
                            last_emit_bytes = downloaded_bytes;
                        }
                    }
                    Err(error) => {
                        read_error = Some(format!("下载文件失败：{error}"));
                        break;
                    }
                }
            }

            let elapsed = start.elapsed();
            let average_speed = (downloaded_bytes as f64 / elapsed.as_secs_f64().max(0.001)) as u64;
            emit(
                sender,
                GithubTestEvent::DownloadProgress {
                    total_bytes,
                    downloaded_bytes,
                    bytes_per_second: average_speed,
                    percentage: total_bytes.map(|total| {
                        (downloaded_bytes as f32 / total.max(1) as f32 * 100.0).min(100.0)
                    }),
                },
            );

            if let Some(error) = read_error {
                GithubMultiTestItem {
                    key: key.to_owned(),
                    name: name.to_owned(),
                    success: false,
                    latency_ms: Some(elapsed.as_millis() as u64),
                    error: Some(error),
                    warning: None,
                }
            } else {
                GithubMultiTestItem {
                    key: key.to_owned(),
                    name: name.to_owned(),
                    success: true,
                    latency_ms: Some(elapsed.as_millis() as u64),
                    error: None,
                    warning: Some(speed_message(average_speed)),
                }
            }
        }
        Ok(mut response) => {
            let status = response.status();
            let mut success = false;
            let mut warning = None;
            let mut error = Some(format!("HTTP {status}"));
            if accelerate_url.is_some() && (status.as_u16() == 403 || status.as_u16() == 404) {
                success = true;
                warning = Some(format!(
                    "加速地址可用，但该资源无法加速 ({})",
                    status.as_u16()
                ));
                error = None;
            } else {
                let mut body = String::new();
                let _ = response.read_to_string(&mut body);
            }
            GithubMultiTestItem {
                key: key.to_owned(),
                name: name.to_owned(),
                success,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                error,
                warning,
            }
        }
        Err(error) => GithubMultiTestItem {
            key: key.to_owned(),
            name: name.to_owned(),
            success: false,
            latency_ms: None,
            error: Some(format!("测速失败：{error}")),
            warning: None,
        },
    };
    let _ = fs::remove_dir_all(&root);
    result
}

fn speed_message(bytes_per_second: u64) -> String {
    let mbps = bytes_per_second as f64 / 1_048_576.0;
    if mbps < 1.0 {
        format!("速度较慢 ({:.1} KB/s)", bytes_per_second as f64 / 1024.0)
    } else if mbps < 4.0 {
        format!("速度正常 ({mbps:.2} MB/s)")
    } else if mbps < 10.0 {
        format!("速度很快 ({mbps:.2} MB/s)")
    } else {
        format!("速度极快 ({mbps:.2} MB/s)")
    }
}

/// 构造旧版风格的超时结果。
pub fn timeout_results() -> Vec<GithubMultiTestItem> {
    failed_results("连接超时", true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_system_proxy_before_http_proxy() {
        let parsed = parse_system_proxy_output(
            "HTTPSEnable : 1\nHTTPSProxy : secure.proxy\nHTTPSPort : 8443\nHTTPEnable : 1\nHTTPProxy : plain.proxy\nHTTPPort : 8080\n",
        );
        assert_eq!(parsed.active_proxy.as_deref(), Some("secure.proxy:8443"));
        assert!(parsed.configured);
    }

    #[test]
    fn download_channels_have_expected_urls_and_keys() {
        assert_eq!(
            DownloadChannel::from_key("mirror_1"),
            DownloadChannel::Mirror1
        );
        assert_eq!(
            DownloadChannel::from_key("镜像 2"),
            DownloadChannel::Mirror2
        );
        assert_eq!(
            DownloadChannel::from_key("mirror3"),
            DownloadChannel::Mirror3
        );
        assert_eq!(DownloadChannel::from_key("官方"), DownloadChannel::Official);
        assert_eq!(DownloadChannel::from_key("unknown"), DownloadChannel::Auto);
        assert_eq!(
            DownloadChannel::Mirror1.clone_url(),
            "https://gitee.com/AstraBrew-Labs/SillyTavern.git"
        );
        assert_eq!(
            DownloadChannel::Mirror2.clone_url(),
            "https://gitcode.com/GitHub_Trending/si/SillyTavern.git"
        );
        assert_eq!(
            DownloadChannel::Mirror3.clone_url(),
            "https://cnb.cool/AstraBrew-Labs/SillyTavern.git"
        );
        assert_eq!(DownloadChannel::Official.clone_url(), GITHUB_CLONE_URL);
        assert_eq!(
            download_url(DownloadChannel::Mirror3),
            "https://cnb.cool/AstraBrew-Labs/SillyTavern/archive/refs/tags/1.18.0.tar.gz"
        );
    }

    #[test]
    fn fastest_channel_ignores_failed_results() {
        let results = vec![
            DownloadChannelTestResult {
                channel: DownloadChannel::Mirror1,
                success: false,
                latency_ms: Some(10),
                error: Some("failed".into()),
            },
            DownloadChannelTestResult {
                channel: DownloadChannel::Mirror2,
                success: true,
                latency_ms: Some(80),
                error: None,
            },
            DownloadChannelTestResult {
                channel: DownloadChannel::Official,
                success: true,
                latency_ms: Some(120),
                error: None,
            },
        ];
        assert_eq!(
            fastest_download_channel(&results),
            Some(DownloadChannel::Mirror2)
        );
    }

    #[test]
    fn all_failed_channel_selection_falls_back_to_official() {
        let results = vec![
            DownloadChannelTestResult {
                channel: DownloadChannel::Mirror1,
                success: false,
                latency_ms: Some(10),
                error: None,
            },
            DownloadChannelTestResult {
                channel: DownloadChannel::Mirror2,
                success: false,
                latency_ms: None,
                error: None,
            },
            DownloadChannelTestResult {
                channel: DownloadChannel::Official,
                success: false,
                latency_ms: Some(30),
                error: None,
            },
        ];
        assert_eq!(fastest_download_channel(&results), None);
    }

    #[test]
    fn test_urls_use_the_requested_clone_and_archive_endpoints() {
        assert_eq!(
            GITHUB_CLONE_URL,
            "https://github.com/SillyTavern/SillyTavern.git"
        );
        assert_eq!(
            GITHUB_DOWNLOAD_URL,
            "https://github.com/SillyTavern/SillyTavern/archive/refs/tags/1.18.0.tar.gz"
        );
    }

    #[test]
    fn parses_git_progress_stage_without_percentage() {
        let progress = parse_git_progress("Enumerating objects: 123, done.").expect("progress");
        assert_eq!(progress.stage, "Enumerating objects");
        assert_eq!(progress.percentage, None);
    }

    #[test]
    fn parses_git_progress_with_counts_and_percentage() {
        let progress = parse_git_progress("Receiving objects: 42% (42/100), 1.2 MiB | 2.3 MiB/s")
            .expect("progress");
        assert_eq!(progress.stage, "Receiving objects");
        assert_eq!(progress.current, Some(42));
        assert_eq!(progress.total, Some(100));
        assert_eq!(progress.percentage, Some(42.0));
    }

    #[test]
    fn normalizes_proxy_urls_and_prefers_https_in_proxy_lists() {
        assert_eq!(
            normalize_proxy_url("127.0.0.1:7890").unwrap(),
            "http://127.0.0.1:7890"
        );
        assert_eq!(
            normalize_system_proxy_url("http=plain.proxy:8080;https=secure.proxy:8443"),
            Some("http://secure.proxy:8443".to_owned())
        );
    }

    #[test]
    fn timeout_results_include_clone_and_download_checks() {
        let results = timeout_results();
        assert_eq!(results.len(), 6);
        assert_eq!(results[4].key, "clone");
        assert_eq!(results[5].key, "speed");
    }
}

/// 在线酒馆稳定发行版本。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct SillyTavernRelease {
    /// 展示用的版本号，例如 `1.18.0`。
    pub version: String,
    /// GitHub 的原始 tag，用于 checkout。
    pub tag_name: String,
    /// GitHub Release 的发布时间。
    pub published_at: String,
    /// GitHub Release 的创建时间。
    pub created_at: String,
    /// GitHub Release body，保持 Markdown 原文。
    pub body: String,
    /// 当前有效下载渠道是否已经同步该 tag。
    pub mirror_available: bool,
}

/// staging 分支的最新状态。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct SillyTavernStaging {
    pub branch: String,
    pub commit_sha: String,
    pub committed_at: String,
    pub message: String,
    pub mirror_available: bool,
}

/// 在线版本目录读取结果。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SillyTavernCatalog {
    pub branch: String,
    pub releases: Vec<SillyTavernRelease>,
    pub staging: Option<SillyTavernStaging>,
    /// 本次用于判断镜像标签的实际下载渠道。
    pub resolved_channel: DownloadChannel,
    /// 版本数据实际写入缓存的时间戳，用于避免页面进入时显示“刚刚更新”。
    pub cached_at: u64,
    /// 是否因为网络请求失败而复用了过期缓存。
    pub used_stale_cache: bool,
}

/// 规范目录中当前安装的酒馆 Git 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct InstalledSillyTavern {
    pub tag_name: Option<String>,
    pub branch: Option<String>,
    pub head: String,
}

/// 在线酒馆安装过程发送给界面的事件。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SillyTavernInstallEvent {
    Log(String),
    DownloadComplete,
    InstallStarted,
    Cancelled,
    Completed(Result<(), String>),
}

/// 在线酒馆安装目标，可以是稳定版 tag 或 staging 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SillyTavernInstallTarget {
    Tag(String),
    Branch(String),
}

const SILLYTAVERN_API_MIRROR: &str =
    "https://gh-proxy.org/https://api.github.com/repos/SillyTavern/SillyTavern";
const SILLYTAVERN_API_DIRECT: &str = "https://api.github.com/repos/SillyTavern/SillyTavern";
const SILLYTAVERN_CACHE_NAME: &str = "sillytavern_versions_cache.json";
const SILLYTAVERN_CACHE_TTL: u64 = 7 * 24 * 60 * 60;

/// 在线酒馆安装目录：`~/Library/Application Support/AstraBrew Launcher/sillytavern`。
#[allow(dead_code)]
pub fn sillytavern_install_dir() -> PathBuf {
    crate::utils::app_paths().sillytavern_dir()
}

/// 在线版本缓存文件路径。
#[allow(dead_code)]
pub fn sillytavern_versions_cache_path() -> PathBuf {
    cache_home_dir()
        .join("Library")
        .join("Caches")
        .join(TEST_ROOT_DIR)
        .join(SILLYTAVERN_CACHE_NAME)
}

/// 读取规范在线酒馆目录当前精确检出的 Git tag。
#[allow(dead_code)]
pub fn installed_sillytavern_tag() -> Option<String> {
    installed_sillytavern_state().and_then(|state| state.tag_name)
}

/// 读取规范在线酒馆目录的 tag、分支和 HEAD，供重启时恢复 UI 状态。
#[allow(dead_code)]
pub fn installed_sillytavern_state() -> Option<InstalledSillyTavern> {
    let target = sillytavern_install_dir();
    let target = target.to_str()?;
    if !sillytavern_install_dir().is_dir() {
        return None;
    }
    let head = git_output(&["-C", target, "rev-parse", "HEAD"])?;
    let tag_name = git_output(&["-C", target, "describe", "--tags", "--exact-match", "HEAD"]);
    let branch =
        git_output(&["-C", target, "branch", "--show-current"]).filter(|branch| !branch.is_empty());
    Some(InstalledSillyTavern {
        tag_name,
        branch,
        head,
    })
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new(resolve_command("git"))
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

#[derive(Debug, Clone)]
struct SillyTavernVersionCache {
    release_cached_at: u64,
    staging_cached_at: u64,
    releases: Vec<SillyTavernRelease>,
    /// 旧版缓存可能没有 body，缺少 body 时必须重新请求一次 Release API。
    release_body_complete: bool,
    staging: Option<SillyTavernStaging>,
}

impl SillyTavernVersionCache {
    fn cached_at_for(&self, branch: &str) -> u64 {
        if branch == "staging" {
            self.staging_cached_at
        } else {
            self.release_cached_at
        }
    }

    fn is_fresh_at(&self, branch: &str, now: u64) -> bool {
        let cached_at = if branch == "staging" {
            self.staging_cached_at
        } else {
            self.release_cached_at
        };
        cached_at != 0
            && now.saturating_sub(cached_at) < SILLYTAVERN_CACHE_TTL
            && (branch == "staging" || self.release_body_complete)
    }
}

fn load_sillytavern_versions_cache() -> Option<SillyTavernVersionCache> {
    let contents = fs::read_to_string(sillytavern_versions_cache_path()).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&contents).ok()?;
    let object = value.as_object()?;
    let legacy_cached_at = object
        .get("cached_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let release_cached_at = object
        .get("release_cached_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(legacy_cached_at);
    let staging_cached_at = object
        .get("staging_cached_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let mut release_body_complete = true;
    let releases = object
        .get("releases")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let item = item.as_object()?;
                    if !item.contains_key("body") {
                        release_body_complete = false;
                    }
                    Some(SillyTavernRelease {
                        version: item.get("version")?.as_str()?.to_owned(),
                        tag_name: item.get("tag_name")?.as_str()?.to_owned(),
                        published_at: item
                            .get("published_at")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        created_at: item
                            .get("created_at")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        body: item
                            .get("body")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                        mirror_available: item
                            .get("mirror_available")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let staging = object.get("staging").and_then(|value| {
        let item = value.as_object()?;
        Some(SillyTavernStaging {
            branch: item.get("branch")?.as_str()?.to_owned(),
            commit_sha: item.get("commit_sha")?.as_str()?.to_owned(),
            committed_at: item.get("committed_at")?.as_str()?.to_owned(),
            message: item.get("message")?.as_str()?.to_owned(),
            mirror_available: item
                .get("mirror_available")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        })
    });
    if releases.is_empty() && staging.is_none() {
        return None;
    }
    Some(SillyTavernVersionCache {
        release_cached_at,
        staging_cached_at,
        releases,
        release_body_complete,
        staging,
    })
}

fn save_sillytavern_versions_cache(
    branch: &str,
    channel: DownloadChannel,
    releases: &[SillyTavernRelease],
    staging: Option<&SillyTavernStaging>,
) -> io::Result<()> {
    let old = load_sillytavern_versions_cache();
    let now = unix_seconds().unwrap_or_default();
    let release_cached_at = if branch == "release" {
        now
    } else {
        old.as_ref()
            .map(|cache| cache.release_cached_at)
            .unwrap_or(0)
    };
    let staging_cached_at = if branch == "staging" {
        now
    } else {
        old.as_ref()
            .map(|cache| cache.staging_cached_at)
            .unwrap_or(0)
    };
    let cached_releases = if branch == "release" {
        releases.to_vec()
    } else {
        old.as_ref()
            .map(|cache| cache.releases.clone())
            .unwrap_or_default()
    };
    let cached_staging = if branch == "staging" {
        staging.cloned()
    } else {
        old.as_ref().and_then(|cache| cache.staging.clone())
    };
    let path = sillytavern_versions_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let value = serde_json::json!({
        "release_cached_at": release_cached_at,
        "staging_cached_at": staging_cached_at,
        "mirror_channel": channel.key(),
        "releases": cached_releases.iter().map(|release| serde_json::json!({
            "version": release.version,
            "tag_name": release.tag_name,
            "published_at": release.published_at,
            "created_at": release.created_at,
            "body": release.body,
            "mirror_available": release.mirror_available,
        })).collect::<Vec<_>>(),
        "staging": cached_staging.map(|item| serde_json::json!({
            "branch": item.branch,
            "commit_sha": item.commit_sha,
            "committed_at": item.committed_at,
            "message": item.message,
            "mirror_available": item.mirror_available,
        })),
    });
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&value).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}

/// 按分支读取在线版本。有效缓存不会触发网络请求，过期后镜像失败则回退直连和旧缓存。
#[allow(dead_code)]
pub fn fetch_sillytavern_catalog(
    branch: &str,
    selected_channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> Result<SillyTavernCatalog, String> {
    let now = unix_seconds().unwrap_or_default();
    let channel = resolve_download_channel(selected_channel);
    let cached = load_sillytavern_versions_cache();
    if let Some(cache) = cached.as_ref()
        && cache.is_fresh_at(branch, now)
    {
        return Ok(catalog_from_cache(cache, branch, channel));
    }

    let client = build_client(proxy_mode, proxy_host);
    let result = client.and_then(|client| {
        if branch == "staging" {
            fetch_sillytavern_staging(&client, SILLYTAVERN_API_MIRROR).or_else(|mirror_error| {
                fetch_sillytavern_staging(&client, SILLYTAVERN_API_DIRECT).map_err(|direct_error| {
                    format!("镜像请求失败：{mirror_error}；直连请求失败：{direct_error}")
                })
            })
        } else {
            fetch_sillytavern_releases(&client, SILLYTAVERN_API_MIRROR)
                .map(FetchedCatalog::Release)
                .or_else(|mirror_error| {
                    fetch_sillytavern_releases(&client, SILLYTAVERN_API_DIRECT)
                        .map(FetchedCatalog::Release)
                        .map_err(|direct_error| {
                            format!("镜像请求失败：{mirror_error}；直连请求失败：{direct_error}")
                        })
                })
        }
    });

    match result {
        Ok(FetchedCatalog::Release(mut releases)) => {
            releases.truncate(10);
            releases = apply_mirror_availability(releases, channel, proxy_mode, proxy_host);
            save_sillytavern_versions_cache(
                "release",
                channel,
                &releases,
                cached.as_ref().and_then(|item| item.staging.as_ref()),
            )
            .map_err(|error| format!("无法保存酒馆版本缓存：{error}"))?;
            Ok(SillyTavernCatalog {
                branch: "release".to_owned(),
                releases,
                staging: cached.and_then(|item| item.staging),
                resolved_channel: channel,
                cached_at: unix_seconds().unwrap_or_default(),
                used_stale_cache: false,
            })
        }
        Ok(FetchedCatalog::Staging(staging)) => {
            let staging =
                apply_staging_mirror_availability(staging, channel, proxy_mode, proxy_host);
            save_sillytavern_versions_cache("staging", channel, &[], Some(&staging))
                .map_err(|error| format!("无法保存酒馆版本缓存：{error}"))?;
            Ok(SillyTavernCatalog {
                branch: "staging".to_owned(),
                releases: Vec::new(),
                staging: Some(staging),
                resolved_channel: channel,
                cached_at: unix_seconds().unwrap_or_default(),
                used_stale_cache: false,
            })
        }
        Err(error) => cached
            .as_ref()
            .map(|cache| catalog_from_cache(cache, branch, channel))
            .map(|mut catalog| {
                catalog.used_stale_cache = true;
                catalog
            })
            .ok_or(error),
    }
}

enum FetchedCatalog {
    Release(Vec<SillyTavernRelease>),
    Staging(SillyTavernStaging),
}

fn catalog_from_cache(
    cache: &SillyTavernVersionCache,
    branch: &str,
    channel: DownloadChannel,
) -> SillyTavernCatalog {
    if branch == "staging" {
        SillyTavernCatalog {
            branch: "staging".to_owned(),
            releases: Vec::new(),
            staging: cache.staging.clone(),
            resolved_channel: channel,
            cached_at: cache.cached_at_for(branch),
            used_stale_cache: false,
        }
    } else {
        SillyTavernCatalog {
            branch: "release".to_owned(),
            releases: cache.releases.iter().take(10).cloned().collect(),
            staging: cache.staging.clone(),
            resolved_channel: channel,
            cached_at: cache.cached_at_for(branch),
            used_stale_cache: false,
        }
    }
}

fn fetch_sillytavern_releases(
    client: &reqwest::blocking::Client,
    api_base: &str,
) -> Result<Vec<SillyTavernRelease>, String> {
    let mut releases = Vec::new();
    let mut page = 1_u32;
    loop {
        let url = format!("{api_base}/releases?per_page=100&page={page}");
        let response = client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "AstraBrew-Launcher")
            .send()
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        let body = response.text().map_err(|error| error.to_string())?;
        let items = serde_json::from_str::<serde_json::Value>(&body)
            .map_err(|error| format!("发布接口返回格式无效：{error}"))?;
        let Some(items) = items.as_array() else {
            return Err("发布接口返回格式无效".to_owned());
        };
        if items.is_empty() {
            break;
        }
        for item in items {
            let Some(item) = item.as_object() else {
                continue;
            };
            if item
                .get("draft")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
                || item
                    .get("prerelease")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            {
                continue;
            }
            let Some(tag_name) = item.get("tag_name").and_then(serde_json::Value::as_str) else {
                continue;
            };
            releases.push(SillyTavernRelease {
                version: normalize_release_version(tag_name),
                tag_name: tag_name.to_owned(),
                published_at: item
                    .get("published_at")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                created_at: item
                    .get("created_at")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                body: item
                    .get("body")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                mirror_available: false,
            });
        }
        if items.len() < 100 {
            break;
        }
        page = page
            .checked_add(1)
            .ok_or_else(|| "发布版本分页溢出".to_owned())?;
    }
    releases.sort_by(compare_releases_desc);
    releases.dedup_by(|left, right| left.tag_name == right.tag_name);
    if releases.is_empty() {
        return Err("未获取到稳定发行版本".to_owned());
    }
    Ok(releases)
}

fn fetch_sillytavern_staging(
    client: &reqwest::blocking::Client,
    api_base: &str,
) -> Result<FetchedCatalog, String> {
    let response = client
        .get(format!("{api_base}/branches/staging"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "AstraBrew-Launcher")
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.text().map_err(|error| error.to_string())?;
    let value = serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|error| format!("分支接口返回格式无效：{error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "分支接口返回格式无效".to_owned())?;
    let commit = object
        .get("commit")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "分支接口缺少 commit".to_owned())?;
    let commit_detail = commit
        .get("commit")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "分支接口缺少提交详情".to_owned())?;
    let author = commit_detail
        .get("author")
        .and_then(serde_json::Value::as_object);
    Ok(FetchedCatalog::Staging(SillyTavernStaging {
        branch: "staging".to_owned(),
        commit_sha: commit
            .get("sha")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        committed_at: author
            .and_then(|item| item.get("date"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        message: commit_detail
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned(),
        mirror_available: false,
    }))
}

fn normalize_release_version(tag_name: &str) -> String {
    tag_name.trim().trim_start_matches(['v', 'V']).to_owned()
}

fn compare_releases_desc(
    left: &SillyTavernRelease,
    right: &SillyTavernRelease,
) -> std::cmp::Ordering {
    right
        .published_at
        .cmp(&left.published_at)
        .then_with(|| {
            parse_release_version(&right.version).cmp(&parse_release_version(&left.version))
        })
        .then_with(|| right.tag_name.cmp(&left.tag_name))
}

fn parse_release_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_text = parts.next()?;
    let patch_end = patch_text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(patch_text.len());
    let patch = patch_text[..patch_end].parse().ok()?;
    Some((major, minor, patch))
}

fn apply_mirror_availability(
    mut releases: Vec<SillyTavernRelease>,
    channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> Vec<SillyTavernRelease> {
    if channel == DownloadChannel::Official {
        return releases;
    }
    let tags = list_remote_tags(channel, proxy_mode, proxy_host).unwrap_or_default();
    for release in &mut releases {
        release.mirror_available = tags.iter().any(|tag| tag == &release.tag_name);
    }
    releases
}

fn apply_staging_mirror_availability(
    mut staging: SillyTavernStaging,
    channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> SillyTavernStaging {
    if channel != DownloadChannel::Official {
        staging.mirror_available = list_remote_branches(channel, proxy_mode, proxy_host)
            .is_ok_and(|branches| branches.iter().any(|branch| branch == "staging"));
    }
    staging
}

/// Auto 优先使用现有测速缓存；没有有效测速结果时回退官方直连。
#[allow(dead_code)]
pub fn resolve_download_channel(channel: DownloadChannel) -> DownloadChannel {
    if channel != DownloadChannel::Auto {
        return channel;
    }
    let now = unix_seconds().unwrap_or_default();
    load_download_channel_cache()
        .filter(|cache| cache.is_valid_at(now))
        .map(|cache| cache.resolved_channel)
        .filter(|channel| *channel != DownloadChannel::Auto)
        .unwrap_or(DownloadChannel::Official)
}

#[allow(dead_code)]
pub fn list_sillytavern_remote_tags(
    selected_channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> Result<Vec<String>, String> {
    list_remote_tags(
        resolve_download_channel(selected_channel),
        proxy_mode,
        proxy_host,
    )
}

fn list_remote_tags(
    channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> Result<Vec<String>, String> {
    if channel == DownloadChannel::Official {
        return Ok(Vec::new());
    }
    let mut command = Command::new(resolve_command("git"));
    configure_git_proxy(&mut command, proxy_mode, proxy_host);
    let output = command
        .args(["ls-remote", "--tags", channel.clone_url()])
        .output()
        .map_err(|error| format!("无法读取镜像标签：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once("refs/tags/").map(|(_, tag)| tag))
        .map(|tag| tag.trim_end_matches("^{}").to_owned())
        .collect())
}

fn list_remote_branches(
    channel: DownloadChannel,
    proxy_mode: &str,
    proxy_host: &str,
) -> Result<Vec<String>, String> {
    if channel == DownloadChannel::Official {
        return Ok(vec!["staging".to_owned()]);
    }
    let mut command = Command::new(resolve_command("git"));
    configure_git_proxy(&mut command, proxy_mode, proxy_host);
    let output = command
        .args(["ls-remote", "--heads", channel.clone_url()])
        .output()
        .map_err(|error| format!("无法读取镜像分支：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            line.split_once("refs/heads/")
                .map(|(_, branch)| branch.to_owned())
        })
        .collect())
}

#[allow(dead_code)]
pub fn resolve_sillytavern_install_channel(
    selected_channel: DownloadChannel,
    tag_name: &str,
    proxy_mode: &str,
    proxy_host: &str,
) -> DownloadChannel {
    let channel = resolve_download_channel(selected_channel);
    if channel == DownloadChannel::Official {
        return DownloadChannel::Official;
    }
    match list_remote_tags(channel, proxy_mode, proxy_host) {
        Ok(tags) if tags.iter().any(|tag| tag == tag_name) => channel,
        _ => DownloadChannel::Official,
    }
}

fn resolve_sillytavern_branch_channel(
    selected_channel: DownloadChannel,
    branch: &str,
    proxy_mode: &str,
    proxy_host: &str,
) -> DownloadChannel {
    let channel = resolve_download_channel(selected_channel);
    if channel == DownloadChannel::Official {
        return DownloadChannel::Official;
    }
    match list_remote_branches(channel, proxy_mode, proxy_host) {
        Ok(branches) if branches.iter().any(|item| item == branch) => channel,
        _ => DownloadChannel::Official,
    }
}

/// 安装或更新稳定 tag / staging 分支，并将 git/npm 的完整输出发送给界面。
#[allow(dead_code)]
pub fn run_sillytavern_install(
    target_ref: SillyTavernInstallTarget,
    target: PathBuf,
    source_channel: DownloadChannel,
    npm_registry: String,
    proxy_mode: String,
    proxy_host: String,
    sender: Sender<SillyTavernInstallEvent>,
) {
    run_sillytavern_install_with_cancel(
        target_ref,
        target,
        source_channel,
        npm_registry,
        proxy_mode,
        proxy_host,
        sender,
        Arc::new(AtomicBool::new(false)),
    );
}

#[allow(dead_code)]
pub fn run_sillytavern_install_with_cancel(
    target_ref: SillyTavernInstallTarget,
    target: PathBuf,
    source_channel: DownloadChannel,
    npm_registry: String,
    proxy_mode: String,
    proxy_host: String,
    sender: Sender<SillyTavernInstallEvent>,
    cancel: Arc<AtomicBool>,
) {
    let result = (|| -> Result<(), String> {
        let (ref_name, is_branch) = match &target_ref {
            SillyTavernInstallTarget::Tag(tag) => (tag.clone(), false),
            SillyTavernInstallTarget::Branch(branch) => (branch.clone(), true),
        };
        let configured_channel = resolve_download_channel(source_channel);
        let preferred_channel = if is_branch {
            resolve_sillytavern_branch_channel(source_channel, &ref_name, &proxy_mode, &proxy_host)
        } else {
            resolve_sillytavern_install_channel(source_channel, &ref_name, &proxy_mode, &proxy_host)
        };
        let mut channels = vec![preferred_channel];
        if preferred_channel != DownloadChannel::Official {
            channels.push(DownloadChannel::Official);
        }

        let target_existed = target.exists();
        let mut sync_error = None;
        for (index, channel) in channels.into_iter().enumerate() {
            if index > 0 {
                send_install_log(
                    &sender,
                    "镜像下载开发版失败，正在回退到官方 GitHub 源。".to_owned(),
                );
            } else if channel == DownloadChannel::Official
                && configured_channel != DownloadChannel::Official
            {
                send_install_log(
                    &sender,
                    "镜像没有 staging 开发版分支，正在使用官方 GitHub 源。".to_owned(),
                );
            }

            match install_git_ref(
                &ref_name,
                is_branch,
                channel,
                &target,
                target_existed,
                &proxy_mode,
                &proxy_host,
                &sender,
                &cancel,
            ) {
                Ok(()) => {
                    sync_error = None;
                    break;
                }
                Err(error) => {
                    send_install_log(&sender, format!("{channel:?} 源下载失败：{error}"));
                    sync_error = Some(error);
                    if !target_existed && target.exists() {
                        // 新目录克隆失败时清理残留目录，确保官方源可以重新 clone。
                        let _ = fs::remove_dir_all(&target);
                    }
                }
            }
        }
        if let Some(error) = sync_error {
            return Err(error);
        }

        ensure_not_cancelled(&cancel)?;
        let _ = sender.send(SillyTavernInstallEvent::DownloadComplete);
        wait_before_npm_install(&cancel)?;
        let _ = sender.send(SillyTavernInstallEvent::InstallStarted);
        // node@24 是 keg-only formula，必须使用统一命令环境为 npm 的 shebang 注入 Node PATH。
        let mut npm = crate::core::settings::env_detect::cmd("npm");
        npm.current_dir(&target).arg("install");
        if !npm_registry.trim().is_empty() {
            npm.env("npm_config_registry", npm_registry);
        }
        configure_npm_proxy(&mut npm, &proxy_mode, &proxy_host);
        run_install_command(npm, &sender, &cancel)
    })();
    match &result {
        Err(error) if error == "安装已取消" => {
            send_install_log(&sender, "安装已取消。".to_owned());
            let _ = sender.send(SillyTavernInstallEvent::Cancelled);
        }
        Err(error) => send_install_log(&sender, format!("安装失败：{error}")),
        Ok(()) => {}
    }
    let _ = sender.send(SillyTavernInstallEvent::Completed(result));
}

/// 使用指定 Git 源同步一个 tag 或分支。调用方负责在失败后切换备用源。
fn install_git_ref(
    ref_name: &str,
    is_branch: bool,
    channel: DownloadChannel,
    target: &PathBuf,
    target_existed: bool,
    proxy_mode: &str,
    proxy_host: &str,
    sender: &Sender<SillyTavernInstallEvent>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let source_url = channel.clone_url();
    let target_text = target.to_string_lossy().to_string();
    if target_existed {
        send_install_log(sender, format!("复用现有酒馆目录：{}", target.display()));
        if !target.join(".git").is_dir() {
            return Err("规范目录已存在但不是 Git 仓库，无法安全更新。".to_owned());
        }
        let mut set_remote = Command::new(resolve_command("git"));
        configure_git_proxy(&mut set_remote, proxy_mode, proxy_host);
        set_remote.args([
            "-C",
            &target_text,
            "remote",
            "set-url",
            "origin",
            source_url,
        ]);
        run_install_command(set_remote, sender, cancel)?;

        let mut fetch = Command::new(resolve_command("git"));
        configure_git_proxy(&mut fetch, proxy_mode, proxy_host);
        if is_branch {
            // 显式写入远程跟踪分支；仅执行 `fetch origin staging` 只会更新 FETCH_HEAD，
            // 后续 checkout origin/staging 会因此找不到提交。
            let remote_ref = format!("+{ref_name}:refs/remotes/origin/{ref_name}");
            fetch.args(["-C", &target_text, "fetch", "origin", &remote_ref]);
        } else {
            fetch.args(["-C", &target_text, "fetch", "--tags", "--force", "origin"]);
        }
        run_install_command(fetch, sender, cancel)?;

        let mut checkout = Command::new(resolve_command("git"));
        configure_git_proxy(&mut checkout, proxy_mode, proxy_host);
        if is_branch {
            let remote_ref = format!("origin/{ref_name}");
            checkout.args(["-C", &target_text, "checkout", "-B", ref_name, &remote_ref]);
        } else {
            checkout.args([
                "-C",
                &target_text,
                "checkout",
                "--detach",
                "--force",
                ref_name,
            ]);
        }
        run_install_command(checkout, sender, cancel)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("无法创建安装目录：{error}"))?;
        }
        let mut clone = Command::new(resolve_command("git"));
        configure_git_proxy(&mut clone, proxy_mode, proxy_host);
        clone
            .args([
                "clone",
                "--progress",
                "--branch",
                ref_name,
                "--depth",
                "1",
                source_url,
            ])
            .arg(target);
        run_install_command(clone, sender, cancel)
    }
}

pub(crate) fn configure_npm_proxy(command: &mut Command, proxy_mode: &str, proxy_host: &str) {
    if let Ok(Some(proxy)) = selected_proxy_url(proxy_mode, proxy_host) {
        command.env("HTTPS_PROXY", &proxy).env("HTTP_PROXY", proxy);
    }
}

fn ensure_not_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("安装已取消".to_owned())
    } else {
        Ok(())
    }
}

fn wait_before_npm_install(cancel: &AtomicBool) -> Result<(), String> {
    for _ in 0..30 {
        ensure_not_cancelled(cancel)?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn send_install_log(sender: &Sender<SillyTavernInstallEvent>, line: String) {
    let _ = sender.send(SillyTavernInstallEvent::Log(line));
}

fn run_install_command(
    command: Command,
    sender: &Sender<SillyTavernInstallEvent>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    run_logged_command(command, cancel, |line| send_install_log(sender, line))
}

/// 本地与在线安装共用的进程执行器；仅上层决定如何显示日志及安装结果。
pub(crate) fn run_logged_command(
    mut command: Command,
    cancel: &AtomicBool,
    mut log: impl FnMut(String),
) -> Result<(), String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let program = command.get_program().to_string_lossy();
    log(format!("正在执行 {program}…"));
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动命令：{error}"))?;
    let (line_sender, line_receiver) = mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        forward_process_stream(stdout, line_sender.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        forward_process_stream(stderr, line_sender.clone());
    }
    drop(line_sender);
    loop {
        while let Ok(line) = line_receiver.try_recv() {
            log(line);
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("安装已取消".to_owned());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                while let Ok(line) = line_receiver.recv_timeout(Duration::from_millis(100)) {
                    log(line);
                }
                if status.success() {
                    return Ok(());
                }
                return Err(format!("命令退出码：{}", status.code().unwrap_or(-1)));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Err(format!("等待命令结束失败：{error}")),
        }
    }
}

fn forward_process_stream<R>(stream: R, sender: Sender<String>)
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in io::BufReader::new(stream).lines() {
            match line {
                Ok(line) => {
                    let _ = sender.send(line);
                }
                Err(error) => {
                    let _ = sender.send(format!("读取命令日志失败：{error}"));
                    break;
                }
            }
        }
    });
}
