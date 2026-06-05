//! 网络相关功能：macOS 系统代理读取、Github 多节点连接测试
//! 跨平台兼容（优先 macOS）

use std::io::Read;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

// ─── macOS 系统代理读取 ──────────────────────────────────────────────────────

/// 通过 `scutil --proxy` 读取 macOS 系统代理设置
///
/// 优先 HTTPS 代理，回退到 HTTP 代理。
/// 返回 `Some((代理地址, 是否启用))` 或 `None`（无代理/读取失败）。
pub fn read_system_proxy() -> Option<(String, bool)> {
    let output = Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let text_lower = text.to_lowercase();

    // 检查是否有错误输出（scutil 把错误信息也打到 stdout）
    if text_lower.contains("not configured") || text_lower.contains("no such") {
        return None;
    }

    // 解析 HTTPSProxy / HTTPProxy
    let mut https_enable = false;
    let mut https_proxy = String::new();
    let mut https_port: u16 = 0;
    let mut http_enable = false;
    let mut http_proxy = String::new();
    let mut http_port: u16 = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("HTTPSEnable : ") {
            https_enable = val.trim() == "1";
        } else if let Some(val) = trimmed.strip_prefix("HTTPSProxy : ") {
            https_proxy = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("HTTPSPort : ") {
            https_port = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = trimmed.strip_prefix("HTTPEnable : ") {
            http_enable = val.trim() == "1";
        } else if let Some(val) = trimmed.strip_prefix("HTTPProxy : ") {
            http_proxy = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("HTTPPort : ") {
            http_port = val.trim().parse().unwrap_or(0);
        }
    }

    // 优先 HTTPS 代理
    if https_enable && !https_proxy.is_empty() {
        let addr = if https_port > 0 {
            format!("{}:{}", https_proxy, https_port)
        } else {
            https_proxy
        };
        return Some((addr, true));
    }

    // 回退 HTTP 代理
    if http_enable && !http_proxy.is_empty() {
        let addr = if http_port > 0 {
            format!("{}:{}", http_proxy, http_port)
        } else {
            http_proxy
        };
        return Some((addr, true));
    }

    // 再检查环境变量
    if let Ok(server) = std::env::var("HTTPS_PROXY") {
        return Some((server, true));
    }
    if let Ok(server) = std::env::var("https_proxy") {
        return Some((server, true));
    }
    if let Ok(server) = std::env::var("HTTP_PROXY") {
        return Some((server, true));
    }
    if let Ok(server) = std::env::var("http_proxy") {
        return Some((server, true));
    }

    None
}

// ─── GitHub 多链接测试 ─────────────────────────────────────────────────────────

/// 多链接测试结果项
#[derive(Debug, Clone)]
pub struct GithubMultiTestItem {
    #[allow(dead_code)]
    pub key: String,
    pub name: String,
    pub success: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

/// 测试多个 GitHub 相关链接
/// proxy_mode: "none" | "system" | "custom"
/// accelerate_url: 加速地址前缀（可选）
/// include_api: 是否包含 api.github.com 测试
pub fn test_github_multi(
    proxy_mode: &str,
    proxy_host: &str,
    _proxy_port: u16,
    accelerate_url: Option<String>,
    include_api: bool,
) -> Vec<GithubMultiTestItem> {
    // 定义测试链接列表
    let mut test_urls: Vec<(String, String, String)> = vec![
        (
            "raw".to_string(),
            "文件访问".to_string(),
            "https://raw.githubusercontent.com/SillyTavern/SillyTavern/release/start.sh".to_string(),
        ),
        (
            "repo".to_string(),
            "仓库访问".to_string(),
            "https://github.com/SillyTavern/SillyTavern".to_string(),
        ),
        (
            "homepage".to_string(),
            "首页访问".to_string(),
            "https://www.github.com".to_string(),
        ),
    ];

    if include_api {
        test_urls.push((
            "api".to_string(),
            "API 访问".to_string(),
            "https://api.github.com/repos/SillyTavern/SillyTavern/releases".to_string(),
        ));
    }

    // 应用加速地址前缀
    if let Some(ref accel) = accelerate_url {
        let accel_base = accel.trim_end_matches('/');
        for url_item in &mut test_urls {
            url_item.2 = format!("{}/{}", accel_base, url_item.2);
        }
    }

    // 构建 reqwest client
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("AstraBrew-Launcher-macOS");

    match proxy_mode {
        "custom" => {
            let mut proxy_url = proxy_host.to_string();
            if !proxy_url.starts_with("http://")
                && !proxy_url.starts_with("https://")
                && !proxy_url.starts_with("socks5://")
            {
                proxy_url = format!("http://{}", proxy_url);
            }
            if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                builder = builder.proxy(proxy);
            }
        }
        "system" => {
            if let Some((server, true)) = read_system_proxy() {
                let proxy_addr = if server.contains('=') {
                    server
                        .split(';')
                        .find_map(|part| {
                            let kv: Vec<&str> = part.splitn(2, '=').collect();
                            if kv.len() == 2 && (kv[0] == "https" || kv[0] == "http") {
                                Some(kv[1].to_string())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| {
                            server
                                .split(';')
                                .next()
                                .and_then(|p| p.splitn(2, '=').nth(1))
                                .unwrap_or(&server)
                                .to_string()
                        })
                } else {
                    server.clone()
                };

                let mut proxy_url = proxy_addr;
                if !proxy_url.starts_with("http://")
                    && !proxy_url.starts_with("https://")
                    && !proxy_url.starts_with("socks5://")
                {
                    proxy_url = format!("http://{}", proxy_url);
                }

                if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
                    builder = builder.proxy(proxy);
                }
            }
        }
        _ => {
            builder = builder.no_proxy();
        }
    }

    let client = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            return test_urls
                .into_iter()
                .map(|(key, name, _)| GithubMultiTestItem {
                    key,
                    name,
                    success: false,
                    latency_ms: None,
                    error: Some(format!("构建客户端失败: {}", e)),
                    warning: None,
                })
                .collect();
        }
    };

    let mut results = Vec::new();
    for (key, name, url) in test_urls {
        let start = Instant::now();
        match client.get(&url).send() {
            Ok(mut resp) => {
                let latency = start.elapsed().as_millis() as u64;
                let status = resp.status();

                let mut success = status.is_success();
                let mut warning = None;
                let mut error = None;

                if !success {
                    if accelerate_url.is_some() {
                        let status_u16 = status.as_u16();
                        if status_u16 == 403 {
                            success = true;
                            warning = Some("加速地址可用，但该资源无法加速 (403)".to_string());
                        } else if status_u16 == 404 {
                            success = true;
                            warning = Some("加速地址可用，但该资源无法加速 (404)".to_string());
                        } else {
                            let mut body = String::new();
                            let _ = resp.read_to_string(&mut body);
                            let lower = body.to_lowercase();
                            if lower.contains("invalid input") || lower.contains("无效输入") {
                                success = true;
                                warning = Some("加速地址可用，但该资源无法加速".to_string());
                            } else {
                                error = Some(format!("HTTP {}", status));
                            }
                        }
                    } else {
                        error = Some(format!("HTTP {}", status));
                    }
                }

                results.push(GithubMultiTestItem {
                    key,
                    name,
                    success,
                    latency_ms: Some(latency),
                    error,
                    warning,
                });
            }
            Err(e) => {
                results.push(GithubMultiTestItem {
                    key,
                    name,
                    success: false,
                    latency_ms: None,
                    error: Some(format!("连接失败: {}", e)),
                    warning: None,
                });
            }
        }
    }

    // 下载速度测试
    let mut speed_test_url =
        "https://github.com/al01cn/sillyTavern-launcher/releases/download/v0.1.5/SillyTavern.Launcher.GUI_x64.app.tar.gz"
            .to_string();
    if let Some(ref accel) = accelerate_url {
        let accel_base = accel.trim_end_matches('/');
        speed_test_url = format!("{}/{}", accel_base, speed_test_url);
    }

    let speed_start = Instant::now();
    let global_timeout = Duration::from_secs(60);

    match client.get(&speed_test_url).send() {
        Ok(mut resp) => {
            let status = resp.status();
            if status.is_success() {
                let mut downloaded = 0u64;
                let mut buffer = [0u8; 8192];
                const MAX_TEST_BYTES: u64 = 4 * 1024 * 1024; // 最多下载 4MB

                while let Ok(n) = resp.read(&mut buffer) {
                    if n == 0 {
                        break;
                    }
                    downloaded += n as u64;
                    if downloaded >= MAX_TEST_BYTES {
                        break;
                    }
                    if speed_start.elapsed() > global_timeout {
                        break;
                    }
                }

                let elapsed = speed_start.elapsed();
                let speed_mbps =
                    (downloaded as f64 / 1_048_576.0) / elapsed.as_secs_f64().max(0.001);

                let speed_msg = if speed_mbps < 1.0 {
                    format!("速度太慢 ({:.1} KB/s)", speed_mbps * 1024.0)
                } else if speed_mbps < 4.0 {
                    format!("速度正常 ({:.2} MB/s)", speed_mbps)
                } else if speed_mbps < 10.0 {
                    format!("速度很快 ({:.2} MB/s)", speed_mbps)
                } else {
                    format!("速度极快 ({:.2} MB/s)", speed_mbps)
                };

                results.push(GithubMultiTestItem {
                    key: "speed".to_string(),
                    name: "下载速度".to_string(),
                    success: true,
                    latency_ms: None,
                    error: None,
                    warning: Some(speed_msg),
                });
            } else {
                let mut success = false;
                let mut warning = None;
                let mut error = None;

                if accelerate_url.is_some() {
                    let status_u16 = status.as_u16();
                    if status_u16 == 403 {
                        success = true;
                        warning = Some("加速地址可用，但该资源无法加速 (403)".to_string());
                    } else if status_u16 == 404 {
                        success = true;
                        warning = Some("加速地址可用，但该资源无法加速 (404)".to_string());
                    } else {
                        let mut body = String::new();
                        let _ = resp.read_to_string(&mut body);
                        let lower = body.to_lowercase();
                        if lower.contains("invalid input") || lower.contains("无效输入") {
                            success = true;
                            warning = Some("加速地址可用，但该资源无法加速".to_string());
                        } else {
                            error = Some(format!("HTTP {}", status));
                        }
                    }
                } else {
                    error = Some(format!("HTTP {}", status));
                }

                results.push(GithubMultiTestItem {
                    key: "speed".to_string(),
                    name: "下载速度".to_string(),
                    success,
                    latency_ms: None,
                    error,
                    warning,
                });
            }
        }
        Err(e) => {
            results.push(GithubMultiTestItem {
                key: "speed".to_string(),
                name: "下载速度".to_string(),
                success: false,
                latency_ms: None,
                error: Some(format!("测速失败: {}", e)),
                warning: None,
            });
        }
    }

    results
}

// ─── 多链接测试全局状态 ──────────────────────────────────────────────────────

struct GithubMultiTestState {
    in_progress: bool,
    test_id: u64,
    results: Option<Vec<GithubMultiTestItem>>,
    start_time: Option<Instant>,
}

static GITHUB_MULTI_TEST_STATE: LazyLock<Mutex<GithubMultiTestState>> = LazyLock::new(|| {
    Mutex::new(GithubMultiTestState {
        in_progress: false,
        test_id: 0,
        results: None,
        start_time: None,
    })
});

/// 取消多链接测试
pub fn cancel_github_multi_test() {
    let mut state = GITHUB_MULTI_TEST_STATE.lock().unwrap();
    state.in_progress = false;
    state.results = None;
    state.start_time = None;
    state.test_id = state.test_id.wrapping_add(1);
}

/// 检查多链接测试是否正在运行
pub fn is_github_multi_test_in_progress() -> bool {
    let mut state = GITHUB_MULTI_TEST_STATE.lock().unwrap();
    if state.in_progress {
        if let Some(st) = state.start_time {
            if st.elapsed() > Duration::from_secs(60) {
                // 超时处理：强行结束测试状态并填入超时结果
                state.in_progress = false;
                let dummy_results = vec![
                    GithubMultiTestItem {
                        key: "raw".to_string(),
                        name: "文件访问".to_string(),
                        success: false,
                        latency_ms: None,
                        error: Some("连接超时".to_string()),
                        warning: None,
                    },
                    GithubMultiTestItem {
                        key: "repo".to_string(),
                        name: "仓库访问".to_string(),
                        success: false,
                        latency_ms: None,
                        error: Some("连接超时".to_string()),
                        warning: None,
                    },
                    GithubMultiTestItem {
                        key: "homepage".to_string(),
                        name: "首页访问".to_string(),
                        success: false,
                        latency_ms: None,
                        error: Some("连接超时".to_string()),
                        warning: None,
                    },
                    GithubMultiTestItem {
                        key: "api".to_string(),
                        name: "API 访问".to_string(),
                        success: false,
                        latency_ms: None,
                        error: Some("连接超时".to_string()),
                        warning: None,
                    },
                    GithubMultiTestItem {
                        key: "speed".to_string(),
                        name: "下载速度".to_string(),
                        success: false,
                        latency_ms: None,
                        error: Some("连接超时".to_string()),
                        warning: None,
                    },
                ];
                state.results = Some(dummy_results);
            }
        }
    }
    state.in_progress
}

/// 启动 Github 多链接测试（后台线程）
pub fn start_github_multi_test(
    proxy_mode: &str,
    proxy_host: &str,
    proxy_port: u16,
    accelerate_url: Option<String>,
    include_api: bool,
) {
    let mut state = GITHUB_MULTI_TEST_STATE.lock().unwrap();
    if state.in_progress {
        return;
    }
    state.in_progress = true;
    state.test_id = state.test_id.wrapping_add(1);
    let current_test_id = state.test_id;
    state.results = None;
    state.start_time = Some(Instant::now());
    drop(state);

    let proxy_mode = proxy_mode.to_string();
    let proxy_host = proxy_host.to_string();

    std::thread::spawn(move || {
        let results =
            test_github_multi(&proxy_mode, &proxy_host, proxy_port, accelerate_url, include_api);
        let mut state = GITHUB_MULTI_TEST_STATE.lock().unwrap();
        if state.in_progress && state.test_id == current_test_id {
            state.in_progress = false;
            state.results = Some(results);
        }
    });
}

/// 获取多链接测试结果（调用后清空）
pub fn get_github_multi_test_result() -> Option<Vec<GithubMultiTestItem>> {
    let mut state = GITHUB_MULTI_TEST_STATE.lock().unwrap();
    state.results.take()
}
