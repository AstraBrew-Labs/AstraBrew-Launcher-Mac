//! 启动器自动更新模块 —— 从发布源检测并安装新版本。
//!
//! 两个发布源按顺序回退，任一源不可用都不会打断检测：
//!
//! | 源 | 仓库地址 | 清单定位方式 |
//! |---|---|---|
//! | 镜像 | <https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac> | GitCode OpenAPI v5 |
//! | 直连 | <https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac> | `releases/latest/download` |
//!
//! GitCode 不支持 GitHub 的 `releases/latest/download/<file>` 短链（该路径只会返回站点的
//! HTML 外壳），必须先用 API 查出最新 Release 与资产地址，再按 tag 拼出真实下载地址。
//!
//! 清单由本模块自己解析，只把「下载 + 验签 + 替换 .app」交给 cargo-packager-updater：
//!
//! - 它的解析契约比实际发布物更严（平台条目必须同时有 `url` / `signature` / `format`），
//!   而 macOS 的安装分支根本不读 `format`，为一个不影响行为的字段让整个更新不可用并不合理；
//! - 它按运行时自检的 `macos-<arch>` 去匹配清单里的键，但 cargo-packager 写出的是
//!   `darwin-universal`，两边对不上会直接 `TargetNotFound`；
//! - 自己解析后，清单只取一次，也顺带能用平台 API 给出的资产地址校正清单里写错的 tag。
//!
//! 无论走哪个源，下载到的包都用清单里的 `signature` 与内嵌公钥做 minisign 校验，
//! 因此换源只改变传输链路，不改变信任链。

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use cargo_packager_updater::{Config, Update, UpdateFormat};
use serde::Deserialize;

// ─── 发布源 ──────────────────────────────────────────────────────────────────

/// 仓库路径（GitHub 与 GitCode 上的路径一致）。
const REPO: &str = "AstraBrew-Labs/AstraBrew-Launcher-Mac";

/// 镜像源站点地址（GitCode）。
const MIRROR_SITE: &str = "https://gitcode.com";

/// 镜像源 OpenAPI v5 基址。
const MIRROR_API: &str = "https://api.gitcode.com/api/v5/repos";

/// 直连源站点地址（GitHub）。
const DIRECT_SITE: &str = "https://github.com";

/// 更新签名公钥；与打包私钥配对，私钥不入库（见 `keys/update_key.pem`）。
///
/// 由 `cargo packager signer generate` 在本仓库生成（与旧版仓库的密钥无关），
/// 每次重新生成密钥时都要同步这里。
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM4NzlCMTdDQ0Q1OTFDQkYKUldTL0hGbk5mTEY1eUJaTExLaHhRNzJSUTc1b29OVk1JbkdqeElZcjNvVlc5Q0c4elVETG9HTFUK";

/// 更新清单文件名，由 cargo-packager 在发布时生成。
const MANIFEST: &str = "latest.json";

/// 单次清单请求的超时；发布源不通时不应让检查一直挂着。
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// 安装包下载的超时；镜像带宽波动较大，留出足够余量。
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// 可用更新源，数组顺序即尝试优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSource {
    /// 镜像：GitCode。
    Mirror,
    /// 直连：GitHub。
    Direct,
}

impl UpdateSource {
    /// 全部更新源，按尝试优先级排列。
    pub const ALL: [Self; 2] = [Self::Mirror, Self::Direct];
}

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

/// 更新失败的原因。
///
/// 核心层不拼用户可见的句子：前几种情况由界面按键取文案，
/// [`UpdateFailure::Detail`] 承载外部库给出的运行时详情。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateFailure {
    /// 清单下载不到（源不可达或尚未发布）
    ManifestUnreachable,
    /// 清单结构不符合预期（非法 JSON、缺版本号、缺 `url` / `signature`）
    ManifestInvalid,
    /// 清单里没有适用于本机的平台条目
    PlatformUnsupported,
    /// 判断不出 `.app` 的安装位置
    InstallLocationUnknown,
    /// 所有更新源都不可用
    SourcesUnreachable,
    /// 更新信息已过期（检测与安装之间发布内容发生变化）
    Expired,
    /// 外部库报出的错误详情
    Detail(String),
}

impl UpdateFailure {
    /// 该原因对应的文案键；[`UpdateFailure::Detail`] 需要配合模板使用，返回 `None`。
    pub const fn message_key(&self) -> Option<&'static str> {
        match self {
            Self::ManifestUnreachable => Some("settings.update.error.manifest_unreachable"),
            Self::ManifestInvalid => Some("settings.update.error.manifest_invalid"),
            Self::PlatformUnsupported => Some("settings.update.error.platform_unsupported"),
            Self::InstallLocationUnknown => Some("settings.update.error.install_location"),
            Self::SourcesUnreachable => Some("settings.update.error.sources_unreachable"),
            Self::Expired => Some("settings.update.error.expired"),
            Self::Detail(_) => None,
        }
    }

    /// 外部错误详情。
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Detail(detail) => Some(detail),
            _ => None,
        }
    }
}

/// 更新检测 / 下载安装的状态推进。
#[derive(Debug, Clone)]
pub enum UpdateStatus {
    /// 正在检查
    Checking,
    /// 已是最新版本
    UpToDate,
    /// 发现新版本（版本号、更新说明、命中的更新源）
    UpdateAvailable {
        version: String,
        notes: Option<String>,
        source: UpdateSource,
    },
    /// 正在下载安装
    Downloading,
    /// 安装完成（需重启生效）
    Installed,
    /// 出错
    Error(UpdateFailure),
}

/// 已解析的发布源：清单地址 + 该源上的资产地址表。
struct ResolvedSource {
    /// `latest.json` 的下载地址。
    manifest: String,
    /// 资产文件名 → 权威下载地址；直连源无法列出资产时为空。
    assets: Vec<(String, String)>,
}

/// 清单解析结果。
#[derive(Debug)]
struct Manifest {
    /// 清单声明的版本号。
    version: cargo_packager_updater::semver::Version,
    /// 发行说明。
    notes: Option<String>,
    /// 本机可用的平台条目。
    platform: ManifestPlatform,
}

/// 清单里本机对应的平台条目。
#[derive(Debug)]
struct ManifestPlatform {
    /// 安装包地址。
    url: String,
    /// 安装包的 minisign 签名。
    signature: String,
}

/// GitCode Release 的 API 表示。
///
/// 字段与 GitHub 基本一致，差异有两点：GitCode 用 `release_status: "latest"` 显式标记最新
/// 发布（GitHub 没有该字段），且 `assets[].type` 区分源码包（`source`）与上传附件（`attach`）。
#[derive(Debug, Deserialize)]
struct GitCodeRelease {
    tag_name: String,
    #[serde(default)]
    release_status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_assets")]
    assets: Vec<GitCodeAsset>,
}

#[derive(Debug, Deserialize)]
struct GitCodeAsset {
    name: String,
    browser_download_url: String,
}

/// 把缺省与 `null` 都当成空列表；无附件的发布可能返回 `null`，不能因此让整份列表解析失败。
fn deserialize_assets<'de, D>(deserializer: D) -> Result<Vec<GitCodeAsset>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<Vec<GitCodeAsset>>::deserialize(deserializer)?.unwrap_or_default())
}

impl GitCodeRelease {
    /// 是否为 GitCode 标记的最新发布。
    fn is_latest(&self) -> bool {
        self.release_status.as_deref() == Some("latest")
    }

    /// 取指定文件名的附件地址。
    fn asset_url(&self, name: &str) -> Option<&str> {
        self.assets
            .iter()
            .find(|asset| asset.name == name)
            .map(|asset| asset.browser_download_url.as_str())
    }

    /// 资产文件名 → 下载地址。
    fn asset_table(&self) -> Vec<(String, String)> {
        self.assets
            .iter()
            .map(|asset| (asset.name.clone(), asset.browser_download_url.clone()))
            .collect()
    }
}

// ─── 公开 API ─────────────────────────────────────────────────────────────────

/// 用户点击「检查更新」时调用。
///
/// 找到新版本后发送 [`UpdateStatus::UpdateAvailable`]，由界面弹出确认框；
/// 已是最新版本或全部源不可用则分别发送 [`UpdateStatus::UpToDate`] 与 [`UpdateStatus::Error`]。
pub fn check_update_manual() -> mpsc::Receiver<UpdateStatus> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(UpdateStatus::Checking);
        match try_check() {
            Ok(Some((version, notes, source))) => {
                let _ = tx.send(UpdateStatus::UpdateAvailable {
                    version,
                    notes,
                    source,
                });
            }
            Ok(None) => {
                let _ = tx.send(UpdateStatus::UpToDate);
            }
            Err(failure) => {
                let _ = tx.send(UpdateStatus::Error(failure));
            }
        }
    });
    rx
}

/// 用户在确认框中选择「立即更新」后调用。
///
/// 只带上命中的更新源，安装前会重新解析一次该源的地址：
/// 检测与安装之间可能已经发布了新版本，重新解析比沿用旧地址更可靠。
pub fn do_install(source: UpdateSource) -> mpsc::Receiver<UpdateStatus> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(UpdateStatus::Downloading);
        match download_and_install(source) {
            Ok(()) => {
                let _ = tx.send(UpdateStatus::Installed);
            }
            Err(failure) => {
                let _ = tx.send(UpdateStatus::Error(failure));
            }
        }
    });
    rx
}

// ─── 内部实现：检测 ───────────────────────────────────────────────────────────

/// 依次探测所有源。
///
/// 只有真正「发现新版本」才会提前结束；「已是最新」不会，因为镜像同步存在滞后 ——
/// 镜像答「没有更新」时仍要去直连确认一次，否则用户在镜像追平之前永远看不到新版本。
/// 多个源都报出更新时取版本号更高的那个。
fn try_check() -> Result<Option<(String, Option<String>, UpdateSource)>, UpdateFailure> {
    let current = current_version().map_err(UpdateFailure::Detail)?;
    let mut newest: Option<(String, Option<String>, UpdateSource)> = None;
    let mut reachable = false;
    let mut broken: Option<UpdateFailure> = None;

    for source in UpdateSource::ALL {
        match check_at(source, &current) {
            CheckResult::Update(remote, notes) => {
                reachable = true;
                match &newest {
                    Some((best, _, _)) if !is_newer(&remote, best) => {}
                    _ => newest = Some((remote, notes, source)),
                }
            }
            CheckResult::UpToDate => reachable = true,
            // 清单结构不对属于发布侧问题，与「网络不通」要分开报，否则会把人引到错误方向。
            CheckResult::Broken(failure) => broken = broken.or(Some(failure)),
            // 该源不可用（未发布、网络不通），换下一个源继续。
            CheckResult::Unreachable => continue,
        }
    }

    if let Some(found) = newest {
        return Ok(Some(found));
    }
    if reachable {
        return Ok(None);
    }
    Err(broken.unwrap_or(UpdateFailure::SourcesUnreachable))
}

/// 判断候选版本号是否高于当前结果。
///
/// 任一版本号无法解析时返回 `false`：宁可保留先拿到的结果，也不因为比较失败丢掉已发现的更新。
fn is_newer(candidate: &str, current: &str) -> bool {
    match (
        candidate.parse::<cargo_packager_updater::semver::Version>(),
        current.parse::<cargo_packager_updater::semver::Version>(),
    ) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => false,
    }
}

/// 单个更新源的探测结果。
enum CheckResult {
    /// 发现新版本（版本号、更新说明）
    Update(String, Option<String>),
    /// 已是最新版本
    UpToDate,
    /// 该源不可用
    Unreachable,
    /// 该源的清单存在但内容不可用
    Broken(UpdateFailure),
}

/// 通过单个源探测更新。
fn check_at(
    source: UpdateSource,
    current: &cargo_packager_updater::semver::Version,
) -> CheckResult {
    let Some(resolved) = resolve(source) else {
        return CheckResult::Unreachable;
    };

    match newer_than(&resolved, current) {
        Ok(Some(manifest)) => CheckResult::Update(manifest.version.to_string(), manifest.notes),
        Ok(None) => CheckResult::UpToDate,
        // 清单拿不到说明该源不可达，继续回退；其余失败按「发布侧问题」单独上报。
        Err(UpdateFailure::ManifestUnreachable) => CheckResult::Unreachable,
        Err(failure) => CheckResult::Broken(failure),
    }
}

// ─── 内部实现：安装 ───────────────────────────────────────────────────────────

/// 在指定源上执行下载与安装。
fn download_and_install(source: UpdateSource) -> Result<(), UpdateFailure> {
    let current = current_version().map_err(UpdateFailure::Detail)?;
    let resolved = resolve(source).ok_or(UpdateFailure::ManifestUnreachable)?;
    let manifest = newer_than(&resolved, &current)?.ok_or(UpdateFailure::Expired)?;
    let update = build_update(manifest, &resolved)?;

    update
        .download_and_install()
        .map_err(|error| UpdateFailure::Detail(error.to_string()))
}

/// 读取清单，并在其中的版本高于本机时返回解析结果。
fn newer_than(
    resolved: &ResolvedSource,
    current: &cargo_packager_updater::semver::Version,
) -> Result<Option<Manifest>, UpdateFailure> {
    let text = http_get(&resolved.manifest).ok_or(UpdateFailure::ManifestUnreachable)?;
    let manifest = parse_manifest(&text)?;
    Ok((manifest.version > *current).then_some(manifest))
}

/// 把清单解析结果组装成 cargo-packager-updater 可直接执行的更新描述。
///
/// 只有下载、验签与替换 `.app` 交给它；版本比较与平台选择已经在前面自己做完。
fn build_update(manifest: Manifest, resolved: &ResolvedSource) -> Result<Update, UpdateFailure> {
    let endpoint = resolved
        .manifest
        .parse()
        .map_err(|_| UpdateFailure::ManifestInvalid)?;
    // 清单里的地址可能带着与实际 Release 不一致的 tag，优先用源上真实的资产地址。
    let url = corrected_download_url(&manifest.platform.url, &resolved.assets)
        .unwrap_or(manifest.platform.url);
    let download_url = url
        .parse()
        .map_err(|_| UpdateFailure::ManifestInvalid)?;

    Ok(Update {
        config: Config {
            endpoints: vec![endpoint],
            pubkey: PUBKEY.into(),
            ..Default::default()
        },
        body: manifest.notes,
        current_version: env!("CARGO_PKG_VERSION").to_owned(),
        version: manifest.version.to_string(),
        // 发布时间只用于展示，本项目界面不读它，无需为此引入 `time` 依赖。
        date: None,
        target: std::env::consts::OS.to_owned(),
        extract_path: extract_path()?,
        download_url,
        signature: manifest.platform.signature,
        timeout: Some(DOWNLOAD_TIMEOUT),
        headers: Default::default(),
        // macOS 上 cargo-packager 的安装格式恒为 `.app`，下载与安装分支都不读该字段。
        format: UpdateFormat::App,
    })
}

/// 判断 `.app` 的安装位置。
///
/// 算法与 cargo-packager-updater 内部保持一致：可执行文件位于 `X.app/Contents/MacOS/` 时
/// 上溯两层得到 `X.app`，否则取其所在目录。
fn extract_path() -> Result<PathBuf, UpdateFailure> {
    let executable = std::env::current_exe().map_err(|_| UpdateFailure::InstallLocationUnknown)?;
    let directory = executable
        .parent()
        .ok_or(UpdateFailure::InstallLocationUnknown)?;
    if directory.to_string_lossy().contains("Contents/MacOS") {
        return directory
            .parent()
            .and_then(std::path::Path::parent)
            .map(PathBuf::from)
            .ok_or(UpdateFailure::InstallLocationUnknown);
    }
    Ok(directory.to_path_buf())
}

// ─── 内部实现：发布源解析 ─────────────────────────────────────────────────────

/// 把更新源解析成清单地址与资产地址表。
fn resolve(source: UpdateSource) -> Option<ResolvedSource> {
    match source {
        UpdateSource::Mirror => resolve_mirror(),
        UpdateSource::Direct => Some(resolve_direct()),
    }
}

/// 解析 GitCode 上的最新 Release。
///
/// GitCode 没有 `releases/latest/download/<file>` 短链，必须读 API 才能定位到具体 tag 下的资产。
fn resolve_mirror() -> Option<ResolvedSource> {
    let releases: Vec<GitCodeRelease> = serde_json::from_str(&http_get(&format!(
        "{MIRROR_API}/{REPO}/releases"
    ))?)
    .ok()?;
    let release = pick_latest(&releases)?;

    // 附件地址由 API 给出，是权威值；万一附件列表里没有清单，再按 tag 拼标准地址兜底。
    let manifest = release
        .asset_url(MANIFEST)
        .map(str::to_owned)
        .unwrap_or_else(|| mirror_download_url(&release.tag_name, MANIFEST));

    Some(ResolvedSource {
        manifest,
        assets: release.asset_table(),
    })
}

/// 解析 GitHub 上的清单地址。
///
/// GitHub 原生支持 `releases/latest/download/<file>`，因此不需要调用 API，
/// 也就不受 `api.github.com` 的匿名限流影响；代价是拿不到资产列表，无法校正清单里的下载地址。
fn resolve_direct() -> ResolvedSource {
    ResolvedSource {
        manifest: format!("{DIRECT_SITE}/{REPO}/releases/latest/download/{MANIFEST}"),
        assets: Vec::new(),
    }
}

/// 选取要使用的 Release。
///
/// 优先采纳 GitCode 用 `release_status` 显式标记的最新发布；标记缺失时退回列表首个
/// 带清单附件的条目（GitCode 与 GitHub 一样按发布时间倒序返回）。
fn pick_latest(releases: &[GitCodeRelease]) -> Option<&GitCodeRelease> {
    releases
        .iter()
        .find(|release| release.is_latest() && release.asset_url(MANIFEST).is_some())
        .or_else(|| {
            releases
                .iter()
                .find(|release| release.asset_url(MANIFEST).is_some())
        })
}

/// 按 tag 拼出 GitCode 的标准资产下载地址。
fn mirror_download_url(tag: &str, name: &str) -> String {
    format!("{MIRROR_SITE}/{REPO}/releases/download/{tag}/{name}")
}

/// 用源上真实存在的资产地址校正清单里的下载地址。
///
/// 清单由发布流水线生成，其中的平台地址由 tag 与文件名拼成；一旦 tag 写法与实际 Release
/// 不一致（例如清单写 `v0.0.1` 而 Release tag 是 `beta-v0.0.1`），下载必定 404。
/// 此时按文件名在资产表里找到的地址就是可用的。资产表为空（直连源）或文件名对不上时返回 `None`。
fn corrected_download_url(manifest_url: &str, assets: &[(String, String)]) -> Option<String> {
    let name = manifest_url.rsplit('/').next()?;
    assets
        .iter()
        .find(|(asset, _)| asset == name)
        .map(|(_, url)| url.clone())
}

// ─── 内部实现：清单解析 ───────────────────────────────────────────────────────

/// 解析清单。
///
/// 只读取真正会用到的字段：版本号、发行说明与平台条目。
/// **不校验 `format`** —— 它是 cargo-packager-updater 反序列化的必填项，但 macOS 的安装
/// 分支并不读取它；为一个不影响行为的字段拒绝整份清单，会让更新功能无故失效。
fn parse_manifest(text: &str) -> Result<Manifest, UpdateFailure> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| UpdateFailure::ManifestInvalid)?;

    let version = value
        .get("version")
        .and_then(|value| value.as_str())
        .ok_or(UpdateFailure::ManifestInvalid)?
        // 版本号可能带 `v` 前缀，与 cargo-packager-updater 的解析保持一致。
        .trim_start_matches('v')
        .parse()
        .map_err(|_| UpdateFailure::ManifestInvalid)?;
    let notes = value
        .get("notes")
        .and_then(|value| value.as_str())
        .map(str::to_owned);

    Ok(Manifest {
        version,
        notes,
        platform: platform_entry(&value)?,
    })
}

/// 取出本机可用的平台条目。
///
/// 清单有两种形态：`platforms` 映射（按 `<os>-<arch>` 分区）与扁平形态（`url` / `signature`
/// 直接挂在顶层）。前者按本机架构选键，后者直接采用。
fn platform_entry(value: &serde_json::Value) -> Result<ManifestPlatform, UpdateFailure> {
    let Some(platforms) = value.get("platforms").and_then(|value| value.as_object()) else {
        return entry_fields(value).ok_or(UpdateFailure::ManifestInvalid);
    };
    let key = pick_platform_key(platforms).ok_or(UpdateFailure::PlatformUnsupported)?;
    platforms
        .get(&key)
        .and_then(entry_fields)
        .ok_or(UpdateFailure::ManifestInvalid)
}

/// 从平台条目里取 `url` 与 `signature`。
fn entry_fields(value: &serde_json::Value) -> Option<ManifestPlatform> {
    Some(ManifestPlatform {
        url: value.get("url")?.as_str()?.to_owned(),
        signature: value.get("signature")?.as_str()?.to_owned(),
    })
}

/// 在清单的 `platforms` 里选出本机可用的键。
///
/// 优先本机架构（包更小），其次通用包，最后是唯一的 macOS 条目 —— 它可能是另一架构的包，
/// 但 Apple Silicon 上可经 Rosetta 运行，总比判定「无可用更新」要好。清单里没有 macOS 条目
/// 时返回 `None`，避免把 Linux / Windows 的包当成 macOS 更新下下来。
fn pick_platform_key(platforms: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let macos: Vec<&String> = platforms
        .keys()
        .filter(|key| key.starts_with("darwin") || key.starts_with("macos"))
        .collect();
    let arch_suffix = format!("-{}", std::env::consts::ARCH);

    macos
        .iter()
        .find(|key| key.ends_with(&arch_suffix))
        .or_else(|| macos.iter().find(|key| key.ends_with("-universal")))
        // `then` 是惰性的：macos 为空时不能求值 `macos[0]`。
        .or_else(|| (macos.len() == 1).then(|| &macos[0]))
        .map(|key| (*key).clone())
}

// ─── 内部实现：基础工具 ───────────────────────────────────────────────────────

/// 发起一次简单的 GET 并取回文本；失败时返回 `None`，由调用方按「源不可用」处理。
fn http_get(url: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(CHECK_TIMEOUT)
        .user_agent(concat!("AstraBrew-Launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let response = client.get(url).send().ok()?.error_for_status().ok()?;
    // 非文本响应（例如 GitCode 的 HTML 外壳）会走到这里，交给调用方判为清单不可用。
    response.text().ok()
}

/// 读取当前编译版本号。
fn current_version() -> Result<cargo_packager_updater::semver::Version, String> {
    env!("CARGO_PKG_VERSION")
        .parse()
        .map_err(|_| "当前版本号格式不正确，无法检查更新。".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GitCode `GET /repos/{owner}/{repo}/releases` 的真实响应（截取，字段名原样保留）。
    const GITCODE_RELEASES: &str = r#"[
      {
        "tag_name": "beta-v0.0.1",
        "target_commitish": "09bf076625366c5e0b516ae7e8d208e226b625a1",
        "prerelease": false,
        "name": "beta-v0.0.1",
        "body": "Full Changelog",
        "created_at": "2026-09-19T16:39:45+08:00",
        "assets": [
          {
            "browser_download_url": "https://raw.gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/archive/refs/heads/beta-v0.0.1.zip",
            "name": "beta-v0.0.1.zip",
            "type": "source"
          },
          {
            "browser_download_url": "https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/beta-v0.0.1/AstraBrew-Launcher_universal.app.tar.gz",
            "name": "AstraBrew-Launcher_universal.app.tar.gz",
            "type": "attach"
          },
          {
            "browser_download_url": "https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/beta-v0.0.1/latest.json",
            "name": "latest.json",
            "type": "attach"
          }
        ],
        "release_status": "latest"
      }
    ]"#;

    /// 仓库里当前**实际发布**的 `latest.json`：平台条目缺 `format`，且下载地址的 tag 写错了。
    const PUBLISHED_MANIFEST: &str = r#"{
      "version":"0.0.1",
      "notes":"新版本发布",
      "pub_date":"2026-06-24T11:49:52Z",
      "platforms":{
        "darwin-universal":{
          "signature":"dW50cnVzdGVkIGNvbW1lbnQ6",
          "url":"https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/v0.0.1/AstraBrew-Launcher_universal.app.tar.gz"
        }
      }
    }"#;

    fn releases() -> Vec<GitCodeRelease> {
        serde_json::from_str(GITCODE_RELEASES).expect("GitCode 响应样例必须可解析")
    }

    fn platforms(json: &str) -> serde_json::Map<String, serde_json::Value> {
        serde_json::from_str(json).expect("platforms 样例必须可解析")
    }

    fn version(text: &str) -> cargo_packager_updater::semver::Version {
        text.parse().expect("样例版本号必须可解析")
    }

    #[test]
    fn published_manifest_is_accepted() {
        // 回归测试：线上清单缺 `format`，而该字段在 macOS 的安装分支里根本不会被读取，
        // 不能因为它拒绝整份清单。平台键 `darwin-universal` 也必须能选中。
        let manifest = parse_manifest(PUBLISHED_MANIFEST).expect("线上清单必须可用");
        assert_eq!(manifest.version, version("0.0.1"));
        assert_eq!(manifest.notes.as_deref(), Some("新版本发布"));
        assert_eq!(
            manifest.platform.url,
            "https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/v0.0.1/AstraBrew-Launcher_universal.app.tar.gz"
        );
        assert_eq!(manifest.platform.signature, "dW50cnVzdGVkIGNvbW1lbnQ6");
    }

    #[test]
    fn version_may_carry_a_v_prefix() {
        let json = r#"{"version":"v1.2.3","platforms":{"darwin-universal":{"url":"u","signature":"s"}}}"#;
        assert_eq!(
            parse_manifest(json).expect("带 v 前缀的版本号应可解析").version,
            version("1.2.3")
        );
    }

    #[test]
    fn flat_manifest_is_accepted() {
        let flat = r#"{"version":"1.0.0","url":"https://example.com/app.tar.gz","signature":"sig"}"#;
        let manifest = parse_manifest(flat).expect("扁平清单必须可用");
        assert_eq!(manifest.platform.url, "https://example.com/app.tar.gz");
    }

    #[test]
    fn manifest_without_signature_is_rejected() {
        let json = r#"{"version":"1.0.0","platforms":{"darwin-universal":{"url":"u"}}}"#;
        assert_eq!(
            parse_manifest(json).unwrap_err(),
            UpdateFailure::ManifestInvalid
        );
    }

    #[test]
    fn manifest_without_version_is_rejected() {
        let json = r#"{"platforms":{"darwin-universal":{"url":"u","signature":"s"}}}"#;
        assert_eq!(
            parse_manifest(json).unwrap_err(),
            UpdateFailure::ManifestInvalid
        );
    }

    #[test]
    fn invalid_json_is_reported_as_manifest_invalid() {
        // 源站返回 HTML 外壳（GitCode 的 `releases/latest/download` 就是这样）时走到这里。
        assert_eq!(
            parse_manifest("<!DOCTYPE html><html></html>").unwrap_err(),
            UpdateFailure::ManifestInvalid
        );
    }

    #[test]
    fn manifest_without_macos_platform_is_rejected() {
        let json = r#"{"version":"1.0.0","platforms":{"linux-x86_64":{"url":"u","signature":"s"}}}"#;
        assert_eq!(
            parse_manifest(json).unwrap_err(),
            UpdateFailure::PlatformUnsupported
        );
    }

    #[test]
    fn arch_specific_key_wins_over_universal() {
        let arch = std::env::consts::ARCH;
        let json = format!(
            r#"{{"darwin-universal":{{"url":"universal","signature":"s"}},"darwin-{arch}":{{"url":"arch","signature":"s"}}}}"#
        );
        assert_eq!(
            parse_manifest(&json).expect("应可解析").platform.url,
            "arch"
        );
    }

    #[test]
    fn sole_foreign_macos_key_is_accepted_as_rosetta_fallback() {
        // 只有另一架构的 macOS 包时仍然采用（Apple Silicon 可经 Rosetta 运行），
        // 但绝不能把 Linux / Windows 的包当成 macOS 更新。
        let json = r#"{"darwin-x86_64":{"url":"u","signature":"s"},"linux-aarch64":{"url":"u","signature":"s"}}"#;
        assert_eq!(
            pick_platform_key(&platforms(json)),
            Some("darwin-x86_64".to_owned())
        );
    }

    #[test]
    fn manifest_download_url_is_corrected_by_asset_table() {
        // 清单里写的是 `v0.0.1`，实际 Release tag 是 `beta-v0.0.1`；靠资产表纠正。
        let list = releases();
        let release = pick_latest(&list).expect("应能选出最新发布");
        let corrected = corrected_download_url(
            "https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/v0.0.1/AstraBrew-Launcher_universal.app.tar.gz",
            &release.asset_table(),
        );
        assert_eq!(
            corrected.as_deref(),
            Some("https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/beta-v0.0.1/AstraBrew-Launcher_universal.app.tar.gz")
        );
    }

    #[test]
    fn correction_is_skipped_without_matching_asset() {
        // 直连源没有资产表；文件名对不上时也不应改动清单地址。
        let manifest_url = "https://github.com/example/releases/download/v1/app.tar.gz";
        assert_eq!(corrected_download_url(manifest_url, &[]), None);
        assert_eq!(
            corrected_download_url(manifest_url, &[("other.tar.gz".to_owned(), "u".to_owned())]),
            None
        );
    }

    #[test]
    fn latest_release_is_picked_by_status_flag() {
        let list = releases();
        let release = pick_latest(&list).expect("应能选出最新发布");
        assert_eq!(release.tag_name, "beta-v0.0.1");
        assert!(release.is_latest());
    }

    #[test]
    fn release_assets_tolerate_missing_or_null_list() {
        let json = r#"[{"tag_name":"v1"},{"tag_name":"v2","assets":null}]"#;
        let list: Vec<GitCodeRelease> = serde_json::from_str(json).expect("缺省与 null 都应可解析");
        assert_eq!(list.len(), 2);
        assert!(list.iter().all(|release| release.assets.is_empty()));
    }

    #[test]
    fn manifest_endpoint_follows_the_real_tag() {
        // GitCode 的 `releases/latest/download` 不可用，必须落到具体 tag 上。
        let list = releases();
        let release = pick_latest(&list).expect("应能选出最新发布");
        assert_eq!(
            release.asset_url(MANIFEST),
            Some("https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/beta-v0.0.1/latest.json")
        );
        assert_eq!(
            mirror_download_url(&release.tag_name, MANIFEST),
            "https://gitcode.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/download/beta-v0.0.1/latest.json"
        );
    }

    #[test]
    fn release_without_manifest_is_skipped() {
        let mut list = releases();
        list.push(GitCodeRelease {
            tag_name: "v9.9.9".to_owned(),
            release_status: Some("latest".to_owned()),
            assets: Vec::new(),
        });
        // 标记为 latest 却没有清单附件，应退回真正带清单的条目，而不是选中它。
        let release = pick_latest(&list).expect("应能选出最新发布");
        assert_eq!(release.tag_name, "beta-v0.0.1");
    }

    #[test]
    fn pick_latest_returns_none_for_empty_list() {
        assert!(pick_latest(&[]).is_none());
    }

    #[test]
    fn direct_source_uses_github_latest_shortcut() {
        assert_eq!(
            resolve_direct().manifest,
            "https://github.com/AstraBrew-Labs/AstraBrew-Launcher-Mac/releases/latest/download/latest.json"
        );
    }

    #[test]
    fn current_version_parses_package_version() {
        let version = current_version().expect("CARGO_PKG_VERSION 必须可解析");
        assert_eq!(version.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn newer_version_wins_between_sources() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn unparsable_version_never_replaces_the_incumbent() {
        assert!(!is_newer("beta-v0.0.1", "0.2.0"));
        assert!(!is_newer("0.3.0", "not-a-version"));
    }

    #[test]
    fn extract_path_uses_the_executable_directory_when_unpackaged() {
        // 测试二进制不在 `.app` 里，取所在目录即可；打包后由 Contents/MacOS 上溯两层。
        let path = extract_path().expect("测试环境应能确定可执行文件目录");
        assert!(path.is_dir(), "{path:?} 应是已存在的目录");
    }

    #[test]
    fn only_detail_failures_need_a_template() {
        for failure in [
            UpdateFailure::ManifestUnreachable,
            UpdateFailure::ManifestInvalid,
            UpdateFailure::PlatformUnsupported,
            UpdateFailure::InstallLocationUnknown,
            UpdateFailure::SourcesUnreachable,
            UpdateFailure::Expired,
        ] {
            assert!(failure.message_key().is_some(), "{failure:?} 应有文案键");
            assert!(failure.detail().is_none(), "{failure:?} 不应携带详情");
        }
        let detail = UpdateFailure::Detail("boom".to_owned());
        assert!(detail.message_key().is_none());
        assert_eq!(detail.detail(), Some("boom"));
    }
}
