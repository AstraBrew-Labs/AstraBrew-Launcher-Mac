//! 网络相关功能：系统代理读取、GitHub 多地址连通性与下载测试。

use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GITHUB_CLONE_URL: &str = "https://github.com/SillyTavern/SillyTavern.git";
const GITHUB_DOWNLOAD_URL: &str =
    "https://github.com/SillyTavern/SillyTavern/archive/refs/tags/1.18.0.tar.gz";
const TEST_ROOT_DIR: &str = "AstraBrew Launcher";

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

fn build_client(proxy_mode: &str, proxy_host: &str) -> Result<reqwest::blocking::Client, String> {
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
    run_github_test_with_cancel(
        proxy_mode,
        proxy_host,
        accelerate_url,
        include_api,
        sender,
        Arc::new(AtomicBool::new(false)),
    );
}

/// 支持取消信号的 GitHub 测试入口。
pub fn run_github_test_with_cancel(
    proxy_mode: &str,
    proxy_host: &str,
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
        let item = test_http_endpoint(&client, &key, &name, &url, accelerate, &sender, &cancel);
        emit(&sender, GithubTestEvent::ItemFinished(item.clone()));
        results.push(item);
    }

    let clone_item = test_git_clone(proxy_mode, proxy_host, accelerate, &sender, &cancel);
    emit(&sender, GithubTestEvent::ItemFinished(clone_item.clone()));
    results.push(clone_item);

    let download_item = test_download(
        &client, proxy_mode, proxy_host, accelerate, &sender, &cancel,
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
    let request_url = accelerated_url(url, accelerate_url);
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

    let url = accelerated_url(GITHUB_CLONE_URL, accelerate_url);
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

    let url = accelerated_url(GITHUB_DOWNLOAD_URL, accelerate_url);
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
