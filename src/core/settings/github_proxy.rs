use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ─── 接口数据结构 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyNode {
    pub url: String,
    pub server: String,
    pub ip: String,
    pub location: String,
    pub latency: u64, // 接口返回的 latency（ms），仅供参考
    pub speed: f64,   // KB/s
    pub tag: String,
}

// ─── 缓存结构 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct CacheFile {
    /// Unix 时间戳（秒）——写入时间
    cached_at: u64,
    nodes: Vec<ProxyNode>,
}

const CACHE_TTL_SECS: u64 = 3 * 24 * 60 * 60; // 3 天
const API_URL: &str = "https://api.akams.cn/github";

fn cache_path() -> PathBuf {
    crate::utils::app_paths().caches.join("github_proxy_cache.json")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 从磁盘读取缓存，若未过期则返回节点列表
fn load_cache() -> Option<Vec<ProxyNode>> {
    let path = cache_path();
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let cache: CacheFile = serde_json::from_str(&content).ok()?;
    if now_secs().saturating_sub(cache.cached_at) < CACHE_TTL_SECS {
        Some(cache.nodes)
    } else {
        None
    }
}

/// 从磁盘读取缓存（无视 TTL，过期也返回）
fn load_cache_any() -> Option<Vec<ProxyNode>> {
    let path = cache_path();
    if !path.exists() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let cache: CacheFile = serde_json::from_str(&content).ok()?;
    Some(cache.nodes)
}

/// 将节点列表写入磁盘缓存
fn save_cache(nodes: &[ProxyNode]) {
    let cache = CacheFile {
        cached_at: now_secs(),
        nodes: nodes.to_vec(),
    };
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&cache) {
        let _ = fs::write(path, content);
    }
}

/// 从接口获取节点列表（阻塞）
/// 使用 serde_json::Value 手动解析，对字段缺失/类型不匹配更宽容
fn fetch_nodes_from_api() -> Result<Vec<ProxyNode>, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?
        .get(API_URL)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;

    let text = response
        .text()
        .map_err(|e| format!("读取响应失败: {e}"))?;

    // 空响应
    if text.trim().is_empty() {
        return Err("接口返回空响应".to_string());
    }

    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        // 截取前 300 字符便于调试
        let preview = &text[..text.len().min(300)];
        format!("解析响应失败 ({e})\n响应预览: {preview}")
    })?;

    // 检查 code 字段
    let code = json
        .get("code")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if code != 200 {
        let msg = json
            .get("msg")
            .and_then(|v| v.as_str())
            .unwrap_or("未知错误");
        return Err(format!("接口错误: {msg}"));
    }

    // 手动解析 data 数组，每个字段独立处理
    let nodes: Vec<ProxyNode> = json
        .get("data")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|item| ProxyNode {
                    url: item
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    server: item
                        .get("server")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    ip: item
                        .get("ip")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    location: item
                        .get("location")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    latency: item
                        .get("latency")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0),
                    speed: item
                        .get("speed")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    tag: item
                        .get("tag")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    if nodes.is_empty() {
        return Err("接口返回的节点列表为空".to_string());
    }

    Ok(nodes)
}

// ─── 延迟测试 ─────────────────────────────────────────────────────────────────

/// 测试单个节点 URL 的实测延迟（ms），失败则返回 None
/// 通过对 {url}/favicon.ico 发起 HEAD 请求来测量
pub fn measure_latency(url: &str) -> Option<u64> {
    let test_url = format!("{}/favicon.ico", url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let start = Instant::now();
    let resp = client.head(&test_url).send();
    let elapsed = start.elapsed().as_millis() as u64;
    match resp {
        Ok(_) => Some(elapsed),
        Err(_) => None,
    }
}

// ─── 公开 API ─────────────────────────────────────────────────────────────────

/// 异步状态机，供 UI 层轮询
#[derive(Debug, Clone, PartialEq)]
pub enum NodeLoadState {
    Idle,
    Loading,
    Done(Vec<NodeEntry>),
    /// 数据可用，但使用了过期缓存（附带原因说明）
    DoneWithWarning(Vec<NodeEntry>, String),
    Error(String),
}

/// 带实测延迟的节点条目
#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub url: String,
    pub server: String,
    pub ip: String,
    pub location: String,
    pub speed: f64,       // KB/s
    pub api_latency: u64,  // 接口返回的参考延迟（ms）
    pub tag: String,
    /// 实测延迟（ms），None = 测试中或失败
    pub measured_ms: Arc<Mutex<Option<Option<u64>>>>,
}

impl PartialEq for NodeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

/// 后台线程：获取节点列表 + 并发测速
///
/// 回退规则：
/// 1. 始终优先调 API 获取最新数据
/// 2. API 失败 → 有缓存（含过期）用缓存
/// 3. 无缓存 + API 失败 → 用硬编码回退列表
pub fn start_fetch_and_test(tx: std::sync::mpsc::Sender<NodeLoadMsg>, force_refresh: bool) {
    std::thread::spawn(move || {
        // 预先读缓存作为降级备选（不管 TTL）
        let cached_any = load_cache_any();
        // 有效缓存标记（用于区分降级提示文案）
        let cache_is_valid = if !force_refresh {
            load_cache().is_some()
        } else {
            false
        };

        match fetch_nodes_from_api() {
            Ok(nodes) => {
                // 规则 3：API 成功 → 更新缓存 + 使用最新数据
                save_cache(&nodes);
                let _entries = build_and_test_nodes(&nodes, &tx);
                let _ = tx.send(NodeLoadMsg::Done);
            }
            Err(api_err) => {
                // 规则 1 & 2：API 失败 → 降级
                if let Some(cached) = cached_any {
                    let msg = if cache_is_valid {
                        format!("接口获取失败（{api_err}），已使用本地缓存")
                    } else {
                        format!("接口获取失败（{api_err}），已降级使用过期缓存")
                    };
                    let _entries = build_and_test_nodes(&cached, &tx);
                    let _ = tx.send(NodeLoadMsg::DoneWithWarning(msg));
                    let _ = tx.send(NodeLoadMsg::Done);
                } else {
                    let fallback = fallback_nodes();
                    let _entries = build_and_test_nodes(&fallback, &tx);
                    let _ = tx.send(NodeLoadMsg::DoneWithWarning(format!(
                        "接口获取失败（{api_err}），已使用默认节点列表（含 2 个可用节点）"
                    )));
                    let _ = tx.send(NodeLoadMsg::Done);
                }
            }
        }
    });
}

/// 将 ProxyNode 列表转为 NodeEntry 列表并启动并发测速
fn build_and_test_nodes(
    nodes: &[ProxyNode],
    tx: &std::sync::mpsc::Sender<NodeLoadMsg>,
) -> Vec<NodeEntry> {
    let entries: Vec<NodeEntry> = nodes
        .iter()
        .map(|n| NodeEntry {
            url: n.url.clone(),
            server: n.server.clone(),
            ip: n.ip.clone(),
            location: n.location.clone(),
            speed: n.speed,
            api_latency: n.latency,
            tag: n.tag.clone(),
            measured_ms: Arc::new(Mutex::new(None)),
        })
        .collect();

    let _ = tx.send(NodeLoadMsg::Nodes(entries.clone()));

    let handles: Vec<_> = entries
        .iter()
        .map(|entry| {
            let url = entry.url.clone();
            let slot = Arc::clone(&entry.measured_ms);
            let tx2 = tx.clone();
            std::thread::spawn(move || {
                let result = measure_latency(&url);
                if let Ok(mut guard) = slot.lock() {
                    *guard = Some(result);
                }
                let _ = tx2.send(NodeLoadMsg::LatencyUpdate);
            })
        })
        .collect();

    for h in handles {
        let _ = h.join();
    }

    entries
}

/// 硬编码回退节点列表 — API 不可用且无缓存时的最后防线
fn fallback_nodes() -> Vec<ProxyNode> {
    vec![
        ProxyNode {
            url: "https://ghfast.top/".to_string(),
            server: "ghfast.top".to_string(),
            ip: String::new(),
            location: "默认节点".to_string(),
            latency: 0,
            speed: 0.0,
            tag: "默认".to_string(),
        },
        ProxyNode {
            url: "https://github-proxy.memory-echoes.cn/".to_string(),
            server: "memory-echoes.cn".to_string(),
            ip: String::new(),
            location: "默认节点".to_string(),
            latency: 0,
            speed: 0.0,
            tag: "备用".to_string(),
        },
    ]
}

/// 通道消息
#[derive(Debug)]
pub enum NodeLoadMsg {
    Nodes(Vec<NodeEntry>),
    LatencyUpdate,
    Done,
    /// 完成但使用了降级数据（过期缓存等），附带原因
    DoneWithWarning(String),
    #[allow(dead_code)]
    Error(String),
}
