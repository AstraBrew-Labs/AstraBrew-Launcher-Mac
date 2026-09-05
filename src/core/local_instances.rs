//! 本地实例的身份识别与原子存储；不在这里操作界面或启动安装。

pub(crate) mod dependencies;
pub(crate) mod find_scan;
pub(crate) mod scan;

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 运行依赖必须经过实际检测，不能从 node_modules 是否存在推断完整性。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DependencyStatus {
    #[default]
    Checking,
    Ready,
    Incomplete,
    Failed(String),
    Installing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInstance {
    pub version: String,
    pub path: String,
    pub dependencies: DependencyStatus,
    /// 后台读取的文件系统身份，UI 去重不再同步访问磁盘。
    pub identity: Option<(u64, u64)>,
}

/// 后台任务使用错误类别，不以界面语言判断取消或权限错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalErrorKind {
    Service,
    Io,
    PermissionDenied,
    Cancelled,
    InvalidInstance,
    OnlineInstance,
}

/// 可翻译的标题、原始诊断及目标路径分开保存。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalError {
    pub message: &'static str,
    pub detail: String,
    pub kind: LocalErrorKind,
    pub path: Option<PathBuf>,
}

impl LocalError {
    pub fn new(message: &'static str, detail: impl ToString) -> Self {
        Self {
            message,
            detail: detail.to_string(),
            kind: LocalErrorKind::Service,
            path: None,
        }
    }

    pub fn with_kind(mut self, kind: LocalErrorKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn cancelled() -> Self {
        Self::new("扫描已取消。", "").with_kind(LocalErrorKind::Cancelled)
    }

    pub fn io(message: &'static str, path: &Path, error: io::Error) -> Self {
        Self {
            message,
            detail: format!("{}: {error}", path.display()),
            kind: if error.kind() == io::ErrorKind::PermissionDenied {
                LocalErrorKind::PermissionDenied
            } else {
                LocalErrorKind::Io
            },
            path: Some(path.to_owned()),
        }
    }
}

pub fn online_dir() -> PathBuf {
    crate::core::network::sillytavern_install_dir()
}

pub fn store_path() -> PathBuf {
    online_dir()
        .parent()
        .unwrap_or(Path::new("."))
        .join("data/local_instances.json")
}

/// 保留无法访问的路径用于展示，但不靠字符串小写化合并大小写敏感卷上的目录。
pub fn normalized_path(path: &Path) -> PathBuf {
    let expanded = if let Ok(rest) = path.strip_prefix("~") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(rest)
    } else {
        path.to_owned()
    };
    if let Ok(canonical) = fs::canonicalize(&expanded) {
        return canonical;
    }
    let mut result = PathBuf::new();
    for part in expanded.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    result
}

pub fn directory_identity(path: &Path) -> io::Result<(u64, u64)> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a directory",
        ));
    }
    Ok((metadata.dev(), metadata.ino()))
}

pub fn same_directory(left: &Path, right: &Path) -> bool {
    match (directory_identity(left), directory_identity(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => normalized_path(left) == normalized_path(right),
    }
}

/// 逐层比较身份，覆盖 /Users 与 /System/Volumes/Data/Users 等路径别名。
pub fn inside_online(path: &Path, online: &Path) -> bool {
    let path = normalized_path(path);
    path.ancestors()
        .any(|ancestor| same_directory(ancestor, online))
}

pub fn inspect_package(package: &Path, online: &Path) -> Result<LocalInstance, LocalError> {
    let invalid = || {
        LocalError::new("选择的不是酒馆实例，请重新选择。", "")
            .with_kind(LocalErrorKind::InvalidInstance)
    };
    if package.file_name().and_then(|name| name.to_str()) != Some("package.json") {
        return Err(invalid());
    }
    let original_root = package.parent().ok_or_else(invalid)?;
    // 文件本身也可能是符号链接，必须以真实清单所在目录识别实例。
    let resolved = fs::canonicalize(package).unwrap_or_else(|_| package.to_owned());
    let root = resolved.parent().ok_or_else(invalid)?;
    // 优先识别在线目录，即使其 package.json 正处于下载或更新阶段。
    if inside_online(root, online) || inside_online(original_root, online) {
        return Err(
            LocalError::new("请不要添加在线实例。", "").with_kind(LocalErrorKind::OnlineInstance)
        );
    }
    let bytes =
        fs::read(package).map_err(|error| LocalError::io("无法读取实例文件。", package, error))?;
    let document: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if !document
        .get("name")
        .and_then(|value| value.as_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("sillytavern"))
    {
        return Err(invalid());
    }
    Ok(LocalInstance {
        version: document
            .get("version")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("未知版本")
            .to_owned(),
        path: normalized_path(root).to_string_lossy().into_owned(),
        dependencies: DependencyStatus::Checking,
        identity: directory_identity(root).ok(),
    })
}

/// 旧版字段允许反序列化；依赖检测结果不写入磁盘，避免重启后误信缓存。
#[derive(Debug, Serialize, Deserialize)]
struct StoredInstance {
    #[serde(default)]
    version: String,
    path: String,
    #[serde(default, skip_serializing)]
    is_online: bool,
}

pub fn load(path: &Path, online: &Path) -> Result<Vec<LocalInstance>, LocalError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(LocalError::new("无法加载本地实例列表。", error)),
    };
    let stored: Vec<StoredInstance> = serde_json::from_slice(&bytes)
        .map_err(|error| LocalError::new("本地实例列表损坏，已停止覆盖原文件。", error))?;
    let mut instances: Vec<LocalInstance> = Vec::new();
    for item in stored {
        if item.path.trim().is_empty() || !normalized_path(Path::new(&item.path)).is_absolute() {
            return Err(LocalError::new(
                "本地实例列表损坏，已停止覆盖原文件。",
                &item.path,
            ));
        }
        let path = normalized_path(Path::new(&item.path));
        if item.is_online
            || inside_online(&path, online)
            || instances
                .iter()
                .any(|other| same_directory(Path::new(&other.path), &path))
        {
            continue;
        }
        let instance = match inspect_package(&path.join("package.json"), online) {
            Ok(instance) => instance,
            Err(error) => LocalInstance {
                version: if item.version.is_empty() {
                    "未知版本".into()
                } else {
                    item.version
                },
                path: path.to_string_lossy().into_owned(),
                dependencies: DependencyStatus::Failed(error.detail),
                identity: directory_identity(&path).ok(),
            },
        };
        instances.push(instance);
    }
    Ok(instances)
}

pub fn save(path: &Path, instances: &[LocalInstance]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let stored: Vec<StoredInstance> = instances
        .iter()
        .map(|item| StoredInstance {
            version: item.version.clone(),
            path: item.path.clone(),
            is_online: false,
        })
        .collect();
    let bytes = serde_json::to_vec_pretty(&stored).map_err(io::Error::other)?;
    // 使用同目录临时文件加 rename，避免异常退出留下半份 JSON。
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let temp = parent.join(format!(
        ".local_instances.{}.{nonce}.tmp",
        std::process::id()
    ));
    let mut created = false;
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        created = true;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if result.is_err() && created {
        // 只清理本轮成功创建的临时文件。
        // 同一进程的存储调用由应用层串行化。
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    pub(super) struct Fixture(pub PathBuf);
    impl Fixture {
        pub(super) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "astra-local-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub(super) fn package(&self, value: &str) -> PathBuf {
            let path = self.0.join("package.json");
            fs::write(&path, value).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn package_validation_and_online_aliases() {
        let fixture = Fixture::new();
        let online = fixture.0.join("online");
        for name in ["sillytavern", "SillyTavern", "SILLYTAVERN"] {
            let path = fixture.package(&format!(r#"{{"name":"{name}"}}"#));
            assert_eq!(inspect_package(&path, &online).unwrap().version, "未知版本");
        }
        for content in [
            "{}",
            "{",
            r#"{"name":42}"#,
            r#"{"name":" sillytavern"}"#,
            r#"{"name":"other"}"#,
        ] {
            assert!(inspect_package(&fixture.package(content), &online).is_err());
        }
        let path = fixture.package(r#"{"name":"sillytavern"}"#);
        assert!(inspect_package(&path, &fixture.0).is_err());
        let alias = fixture.0.join("alias");
        std::os::unix::fs::symlink(&fixture.0, &alias).unwrap();
        assert!(same_directory(&alias, &fixture.0));
        assert!(inspect_package(&alias.join("package.json"), &fixture.0).is_err());
        assert!(inspect_package(&fixture.0.join("other.json"), &online).is_err());
    }
    #[test]
    fn legacy_store_retains_unavailable_and_deduplicates() {
        let fixture = Fixture::new();
        fixture.package(r#"{"name":"sillytavern","version":"1"}"#);
        let path = fixture.0.join("instances.json");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!([
                {"path":fixture.0,"version":"0","is_current":true},
                {"path":fixture.0,"version":"0"},
                {"path":fixture.0.join("missing"),"version":"2"}
            ]))
            .unwrap(),
        )
        .unwrap();
        let instances = load(&path, &fixture.0.join("online")).unwrap();
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].version, "1");
        assert!(matches!(
            instances[1].dependencies,
            DependencyStatus::Failed(_)
        ));
        save(&path, &instances).unwrap();
        assert_eq!(load(&path, &fixture.0.join("online")).unwrap().len(), 2);
        fs::write(&path, "{").unwrap();
        assert!(load(&path, &fixture.0.join("online")).is_err());
    }
    #[test]
    fn failed_atomic_save_preserves_existing_file() {
        let fixture = Fixture::new();
        let path = fixture.0.join("existing");
        fs::write(&path, "original").unwrap();
        assert!(save(&path.join("instances.json"), &[]).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "original");
    }

    #[test]
    fn manifest_symlink_to_online_is_rejected() {
        let fixture = Fixture::new();
        let online = fixture.0.join("online");
        let local = fixture.0.join("local");
        fs::create_dir(&online).unwrap();
        fs::create_dir(&local).unwrap();
        fs::write(online.join("package.json"), r#"{"name":"sillytavern"}"#).unwrap();
        std::os::unix::fs::symlink(online.join("package.json"), local.join("package.json"))
            .unwrap();
        assert_eq!(
            inspect_package(&local.join("package.json"), &online)
                .unwrap_err()
                .message,
            "请不要添加在线实例。"
        );
    }
    #[test]
    fn permission_errors_carry_machine_readable_kind_and_path() {
        for code in [1, 13] {
            let error = LocalError::io(
                "任意语言的错误标题",
                Path::new("/fixture/denied"),
                io::Error::from_raw_os_error(code),
            );
            assert_eq!(error.kind, LocalErrorKind::PermissionDenied);
            assert_eq!(error.path, Some(PathBuf::from("/fixture/denied")));
        }
        assert_eq!(LocalError::cancelled().kind, LocalErrorKind::Cancelled);
    }
}
