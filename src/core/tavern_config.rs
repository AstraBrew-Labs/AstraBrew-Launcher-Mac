//! 酒馆 YAML 无损读写服务。文档节点只在执行服务的线程内使用，不跨线程共享。

pub(crate) mod schema;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use yaml_edit::{Mapping, YamlFile, YamlNode};

pub type Values = BTreeMap<String, Value>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub key: String,
    pub path: PathBuf,
    pub instance: PathBuf,
}
impl Context {
    /// 全局路径保持用户设置，独立路径只绑定当前真正选中的实例。
    pub fn resolve(
        instance: Option<&str>,
        source: &str,
        global: bool,
        global_path: &str,
    ) -> Option<Self> {
        let instance = instance.filter(|path| !path.is_empty()).map(expand_home)?;
        let path = if global {
            if global_path.trim().is_empty() {
                app_root().join("data/config.yaml")
            } else {
                expand_home(global_path).join("config.yaml")
            }
        } else {
            instance.join("config.yaml")
        };
        Some(Self {
            key: format!(
                "{global}|{source}|{}|{}",
                instance.display(),
                path.display()
            ),
            path,
            instance,
        })
    }
}
pub fn expand_home(path: &str) -> PathBuf {
    path.strip_prefix("~/")
        .and_then(|rest| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest)))
        .unwrap_or_else(|| PathBuf::from(path))
}
fn app_root() -> PathBuf {
    crate::core::network::sillytavern_install_dir()
        .parent()
        .unwrap_or(Path::new("."))
        .to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Missing,
    Invalid,
    Io,
    Changed,
    Template,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub kind: ErrorKind,
    pub message: &'static str,
    pub detail: String,
}
impl ConfigError {
    pub fn new(kind: ErrorKind, message: &'static str, detail: impl ToString) -> Self {
        Self {
            kind,
            message,
            detail: detail.to_string(),
        }
    }
    fn io(path: &Path, error: io::Error) -> Self {
        Self::new(
            if error.kind() == io::ErrorKind::NotFound {
                ErrorKind::Missing
            } else {
                ErrorKind::Io
            },
            "无法读写酒馆配置文件。",
            format!("{}: {error}", path.display()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub text: String,
    pub physical_path: PathBuf,
    pub values: Values,
    pub raw: BTreeMap<String, Option<Value>>,
}

fn parse(text: &str) -> Result<(YamlFile, Mapping), ConfigError> {
    let file = YamlFile::from_str(text).map_err(|_| {
        ConfigError::new(ErrorKind::Invalid, "配置 YAML 无效，请修复文件后重试。", "")
    })?;
    let docs: Vec<_> = file.documents().collect();
    if docs.len() != 1 {
        return Err(ConfigError::new(
            ErrorKind::Invalid,
            "配置必须包含一个 YAML 映射文档。",
            "",
        ));
    }
    let root = docs[0].as_mapping().ok_or_else(|| {
        ConfigError::new(ErrorKind::Invalid, "配置必须包含一个 YAML 映射文档。", "")
    })?;
    validate_keys(&YamlNode::Mapping(root.clone()))?;
    Ok((file, root))
}
fn validate_keys(node: &YamlNode) -> Result<(), ConfigError> {
    if let Some(map) = node.as_mapping() {
        let mut seen = HashSet::new();
        for (key, value) in map.iter() {
            let key = key
                .as_scalar()
                .ok_or_else(|| {
                    ConfigError::new(ErrorKind::Invalid, "配置键必须是文本且不能重复。", "")
                })?
                .as_string();
            if !seen.insert(key) {
                return Err(ConfigError::new(
                    ErrorKind::Invalid,
                    "配置键必须是文本且不能重复。",
                    "",
                ));
            }
            validate_keys(&value)?;
        }
    } else if let Some(sequence) = node.as_sequence() {
        for value in sequence.values() {
            validate_keys(&value)?;
        }
    }
    Ok(())
}

fn raw_value(node: &YamlNode) -> Result<Value, ConfigError> {
    let error = || ConfigError::new(ErrorKind::Invalid, "此配置字段的 YAML 类型不受支持。", "");
    match node {
        YamlNode::Scalar(scalar) => Ok(if scalar.is_null() {
            Value::Null
        } else if let Some(value) = scalar.as_i64() {
            value.into()
        } else if let Some(value) = scalar.as_bool() {
            value.into()
        } else {
            Value::String(scalar.as_string())
        }),
        YamlNode::Sequence(sequence) => sequence
            .values()
            .map(|item| raw_value(&item))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        YamlNode::TaggedNode(tag) if tag.tag().is_some_and(|tag| tag.contains("null")) => {
            Ok(Value::Null)
        }
        _ => Err(error()),
    }
}
fn get_at(root: &Mapping, path: &str) -> Result<Option<YamlNode>, ConfigError> {
    let mut current = YamlNode::Mapping(root.clone());
    for part in path.split('.') {
        let value = if let Some(map) = current.as_mapping() {
            map.get(part)
        } else if let Some(sequence) = current.as_sequence() {
            part.parse().ok().and_then(|index| sequence.get(index))
        } else {
            return Err(ConfigError::new(
                ErrorKind::Invalid,
                "配置字段的父级类型不正确。",
                path,
            ));
        };
        let Some(value) = value else {
            return Ok(None);
        };
        current = value;
    }
    Ok(Some(current))
}

pub fn snapshot(
    text: String,
    physical_path: PathBuf,
    defaults: &Values,
) -> Result<Snapshot, ConfigError> {
    let (_, root) = parse(&text)?;
    let mut values = defaults.clone();
    let mut raw = BTreeMap::new();
    for field in schema::FIELDS {
        let value = get_at(&root, field.path)?
            .map(|node| raw_value(&node))
            .transpose()
            .map_err(|mut error| {
                error.detail = field.path.into();
                error
            })?;
        if let Some(value) = &value {
            let ui = schema::decode(field, value)
                .map_err(|message| ConfigError::new(ErrorKind::Invalid, message, field.path))?;
            values.insert(field.key.into(), ui);
        }
        raw.insert(field.key.into(), value);
    }
    Ok(Snapshot {
        text,
        physical_path,
        values,
        raw,
    })
}
pub fn load(context: &Context, defaults: &Values) -> Result<Snapshot, ConfigError> {
    let path =
        fs::canonicalize(&context.path).map_err(|error| ConfigError::io(&context.path, error))?;
    let text = fs::read_to_string(&path).map_err(|error| ConfigError::io(&path, error))?;
    snapshot(text, path, defaults)
}

fn value_node(value: &Value) -> Result<YamlNode, ConfigError> {
    let text = format!("value: {}\n", value);
    let (_, mapping) = parse(&text)?;
    mapping
        .get("value")
        .ok_or_else(|| ConfigError::new(ErrorKind::Invalid, "无法构造 YAML 配置值。", ""))
}
fn set_at(
    root: &Mapping,
    field: &schema::Field,
    value: &Value,
    defaults: &Values,
) -> Result<(), ConfigError> {
    let parts: Vec<_> = field.path.split('.').collect();
    let mut current = YamlNode::Mapping(root.clone());
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        if let Some(mapping) = current.as_mapping() {
            if last {
                mapping.set(*part, value_node(value)?);
                return Ok(());
            }
            if mapping.get(*part).is_none() {
                if parts[i + 1].parse::<usize>().is_ok() {
                    let prefix = parts[..=i].join(".");
                    let mut values = Vec::new();
                    for index in 0..2 {
                        let sibling = schema::FIELDS
                            .iter()
                            .find(|f| f.path == format!("{prefix}.{index}"));
                        let value = sibling
                            .and_then(|f| {
                                defaults.get(f.key).and_then(|v| schema::encode(f, v).ok())
                            })
                            .unwrap_or(Value::Null);
                        values.push(value);
                    }
                    mapping.set(*part, value_node(&Value::Array(values))?);
                } else {
                    mapping.set(*part, Mapping::new());
                }
            }
            current = mapping.get(*part).ok_or_else(|| {
                ConfigError::new(ErrorKind::Invalid, "无法构造 YAML 配置值。", field.path)
            })?;
        } else if let Some(sequence) = current.as_sequence() {
            let index: usize = part.parse().map_err(|_| {
                ConfigError::new(ErrorKind::Invalid, "配置字段的父级类型不正确。", field.path)
            })?;
            if !last {
                return Err(ConfigError::new(
                    ErrorKind::Invalid,
                    "配置字段的父级类型不正确。",
                    field.path,
                ));
            }
            while sequence.len() <= index {
                sequence.push(value_node(&Value::Null)?);
            }
            sequence.set(index, value_node(value)?);
            return Ok(());
        } else {
            return Err(ConfigError::new(
                ErrorKind::Invalid,
                "配置字段的父级类型不正确。",
                field.path,
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Patch {
    pub key: String,
    pub expected: Option<Value>,
    pub value: Value,
    pub revision: u64,
}
#[derive(Debug, Clone)]
pub struct SaveResult {
    pub snapshot: Snapshot,
    pub applied: Vec<(String, u64)>,
    pub conflicts: Vec<(String, u64)>,
}
/// 在最新磁盘文档上逐字段三方比较；冲突字段不写，其他有效字段继续保存。
pub fn save(
    context: &Context,
    base_path: &Path,
    patches: &[Patch],
    defaults: &Values,
) -> Result<SaveResult, ConfigError> {
    let disk = load(context, defaults)?;
    if disk.physical_path != base_path {
        return Err(ConfigError::new(
            ErrorKind::Changed,
            "配置目标已变化，请重新加载。",
            context.path.display(),
        ));
    }
    let (file, root) = parse(&disk.text)?;
    let mut applied = Vec::new();
    let mut conflicts = Vec::new();
    for patch in patches {
        let field = schema::field(&patch.key)
            .ok_or_else(|| ConfigError::new(ErrorKind::Invalid, "未知配置字段。", &patch.key))?;
        let current = disk.raw.get(&patch.key).cloned().flatten();
        if current != patch.expected && current.as_ref() != Some(&patch.value) {
            conflicts.push((patch.key.clone(), patch.revision));
            continue;
        }
        if current.as_ref() != Some(&patch.value) {
            set_at(&root, field, &patch.value, defaults)?;
        }
        applied.push((patch.key.clone(), patch.revision));
    }
    let text = file.to_string();
    let updated = snapshot(text.clone(), disk.physical_path.clone(), defaults)?;
    if text != disk.text {
        replace(context, &disk, &text)?;
    }
    Ok(SaveResult {
        snapshot: updated,
        applied,
        conflicts,
    })
}

static NONCE: AtomicU64 = AtomicU64::new(0);
fn temporary(path: &Path, suffix: &str) -> PathBuf {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(
        ".config.yaml.{suffix}.{}.{time}.{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ))
}
fn check_unchanged(context: &Context, expected: &Snapshot) -> Result<(), ConfigError> {
    let path =
        fs::canonicalize(&context.path).map_err(|error| ConfigError::io(&context.path, error))?;
    if path != expected.physical_path
        || fs::read_to_string(&path).map_err(|error| ConfigError::io(&path, error))?
            != expected.text
    {
        return Err(ConfigError::new(
            ErrorKind::Changed,
            "配置文件已被外部修改。",
            context.path.display(),
        ));
    }
    Ok(())
}
fn replace(context: &Context, expected: &Snapshot, text: &str) -> Result<(), ConfigError> {
    check_unchanged(context, expected)?;
    let permissions = fs::metadata(&expected.physical_path)
        .map_err(|e| ConfigError::io(&context.path, e))?
        .permissions();
    let temp = temporary(&expected.physical_path, "saving");
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| ConfigError::io(&temp, e))?;
        file.write_all(text.as_bytes())
            .map_err(|e| ConfigError::io(&temp, e))?;
        file.set_permissions(permissions)
            .map_err(|e| ConfigError::io(&temp, e))?;
        file.sync_all().map_err(|e| ConfigError::io(&temp, e))?;
        check_unchanged(context, expected)?;
        fs::rename(&temp, &expected.physical_path).map_err(|e| ConfigError::io(&context.path, e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[derive(Debug, Clone)]
pub struct NetworkOptions {
    pub proxy_mode: String,
    pub proxy_host: String,
    pub github_proxy: Option<String>,
}
pub fn template(
    context: &Context,
    options: &NetworkOptions,
    progress: &mut impl FnMut(u64, Option<u64>),
    defaults: &Values,
) -> Result<String, ConfigError> {
    let cache = app_root().join("default/config.yaml");
    let candidates = [
        context.instance.join("default/config.yaml"),
        cache.clone(),
        app_root().join("data/default/sillytavern/config.yaml"),
    ];
    for path in candidates {
        match fs::read_to_string(&path) {
            Ok(text) => {
                snapshot(text.clone(), path, defaults)?;
                return Ok(text);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(ConfigError::io(&path, error)),
        }
    }
    let url = "https://raw.githubusercontent.com/SillyTavern/SillyTavern/refs/heads/release/default/config.yaml";
    let mut urls = Vec::new();
    if let Some(proxy) = &options.github_proxy {
        urls.push(format!("{}/{url}", proxy.trim_end_matches('/')));
    }
    urls.push(url.into());
    let client = crate::core::network::build_client(&options.proxy_mode, &options.proxy_host)
        .map_err(|_| ConfigError::new(ErrorKind::Template, "无法创建配置模板下载请求。", ""))?;
    for url in urls {
        let result = (|| -> Result<String, ConfigError> {
            let mut response = client
                .get(url)
                .send()
                .and_then(reqwest::blocking::Response::error_for_status)
                .map_err(|_| ConfigError::new(ErrorKind::Template, "下载配置模板失败。", ""))?;
            let total = response.content_length();
            let mut bytes = Vec::new();
            let mut buffer = [0; 8192];
            loop {
                let n = response
                    .read(&mut buffer)
                    .map_err(|_| ConfigError::new(ErrorKind::Template, "下载配置模板失败。", ""))?;
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err(ConfigError::new(
                        ErrorKind::Template,
                        "配置模板下载内容异常。",
                        "",
                    ));
                }
                progress(bytes.len() as u64, total);
            }
            let text = String::from_utf8(bytes)
                .map_err(|_| ConfigError::new(ErrorKind::Template, "配置模板下载内容异常。", ""))?;
            snapshot(text.clone(), cache.clone(), defaults)?;
            Ok(text)
        })();
        if let Ok(text) = result {
            // 模板缓存失败不覆盖目标文件；下次仍可重新下载。
            let _ = create_new(&cache, &text);
            return Ok(text);
        }
    }
    Err(ConfigError::new(
        ErrorKind::Template,
        "所有配置模板下载地址均失败。",
        "",
    ))
}

/// 先写临时文件，再用硬链接进行“不覆盖”发布，防止生成期间外部已创建配置。
fn create_new(path: &Path, text: &str) -> Result<(), ConfigError> {
    let parent = path
        .parent()
        .ok_or_else(|| ConfigError::new(ErrorKind::Io, "配置目标路径无效。", ""))?;
    fs::create_dir_all(parent).map_err(|e| ConfigError::io(parent, e))?;
    let temp = temporary(path, "creating");
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| ConfigError::io(&temp, e))?;
        file.write_all(text.as_bytes())
            .map_err(|e| ConfigError::io(&temp, e))?;
        file.sync_all().map_err(|e| ConfigError::io(&temp, e))?;
        fs::hard_link(&temp, path).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                ConfigError::new(
                    ErrorKind::Changed,
                    "配置文件已存在，已停止覆盖。",
                    path.display(),
                )
            } else {
                ConfigError::io(path, error)
            }
        })
    })();
    let _ = fs::remove_file(&temp);
    result
}
pub fn generate(
    context: &Context,
    options: &NetworkOptions,
    defaults: &Values,
    progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<Snapshot, ConfigError> {
    match load(context, defaults) {
        Ok(snapshot) => return Ok(snapshot),
        Err(error) if error.kind == ErrorKind::Missing => {}
        Err(error) => return Err(error),
    }
    let text = template(context, options, progress, defaults)?;
    match create_new(&context.path, &text) {
        Ok(()) => load(context, defaults),
        Err(error) if error.kind == ErrorKind::Changed => load(context, defaults),
        Err(error) => Err(error),
    }
}

/// 递归合并映射；用现有文档作底稿，保留未改动的注释和排版。
fn merge_maps(target: &Mapping, source: &Mapping, overwrite: bool) {
    for (key, value) in source.iter() {
        if let Some(key) = key.as_scalar() {
            let key = key.as_string();
            if let (Some(existing), Some(source)) = (target.get_mapping(&key), value.as_mapping()) {
                merge_maps(&existing, source, overwrite);
            } else if let Some(existing) = target.get(&key) {
                let equal = raw_value(&existing)
                    .ok()
                    .zip(raw_value(&value).ok())
                    .is_some_and(|(a, b)| a == b);
                if overwrite && !equal {
                    target.set(key, value);
                }
            } else {
                target.set(key, value);
            }
        }
    }
}
pub fn merge_text(
    template: &str,
    current: &str,
    imported: &str,
    defaults: &Values,
) -> Result<String, ConfigError> {
    let (file, root) = parse(current)?;
    let (_, default_map) = parse(template)?;
    let (_, incoming) = parse(imported)?;
    for text in [template, current, imported] {
        snapshot(text.into(), PathBuf::new(), defaults)?;
    }
    merge_maps(&root, &default_map, false);
    merge_maps(&root, &incoming, true);
    let text = file.to_string();
    snapshot(text.clone(), PathBuf::new(), defaults)?;
    Ok(text)
}
#[derive(Debug, Clone)]
pub struct ImportPreview {
    pub context: Context,
    pub source: PathBuf,
    pub source_text: String,
    pub target: Snapshot,
    pub template_text: String,
    pub merged_text: String,
}
pub fn prepare_import(
    context: &Context,
    source: &Path,
    options: &NetworkOptions,
    defaults: &Values,
    progress: &mut impl FnMut(u64, Option<u64>),
) -> Result<ImportPreview, ConfigError> {
    let target = load(context, defaults)?;
    let source_text = fs::read_to_string(source).map_err(|e| ConfigError::io(source, e))?;
    snapshot(source_text.clone(), source.to_owned(), defaults)?;
    let template_text = template(context, options, progress, defaults)?;
    let merged_text = merge_text(&template_text, &target.text, &source_text, defaults)?;
    Ok(ImportPreview {
        context: context.clone(),
        source: source.to_owned(),
        source_text,
        target,
        template_text,
        merged_text,
    })
}
#[derive(Debug, Clone)]
pub enum ImportResult {
    Saved(Snapshot, PathBuf),
    Reconfirm(ImportPreview),
}
pub fn import(preview: &ImportPreview, defaults: &Values) -> Result<ImportResult, ConfigError> {
    let target = load(&preview.context, defaults)?;
    let source_text =
        fs::read_to_string(&preview.source).map_err(|e| ConfigError::io(&preview.source, e))?;
    if target.text != preview.target.text
        || target.physical_path != preview.target.physical_path
        || source_text != preview.source_text
    {
        let merged_text = merge_text(&preview.template_text, &target.text, &source_text, defaults)?;
        return Ok(ImportResult::Reconfirm(ImportPreview {
            target,
            source_text,
            merged_text,
            ..preview.clone()
        }));
    }
    let backup = temporary(&target.physical_path, "backup");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&backup)
        .map_err(|e| ConfigError::io(&backup, e))?;
    file.write_all(target.text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| ConfigError::io(&backup, e))?;
    check_unchanged(&preview.context, &target)?;
    if fs::read_to_string(&preview.source).map_err(|e| ConfigError::io(&preview.source, e))?
        != source_text
    {
        return Err(ConfigError::new(
            ErrorKind::Changed,
            "导入源文件已变化，请重新确认。",
            preview.source.display(),
        ));
    }
    replace(&preview.context, &target, &preview.merged_text)?;
    Ok(ImportResult::Saved(
        load(&preview.context, defaults)?,
        backup,
    ))
}

pub fn reveal(context: &Context) -> Result<(), ConfigError> {
    let status = std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(&context.path)
        .status()
        .map_err(|e| ConfigError::io(&context.path, e))?;
    if !status.success() {
        return Err(ConfigError::new(
            ErrorKind::Io,
            "无法在 Finder 中定位配置文件。",
            context.path.display(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    struct Fixture {
        root: PathBuf,
        context: Context,
    }
    impl Fixture {
        fn new() -> Self {
            let root = temporary(&std::env::temp_dir().join("config.yaml"), "test");
            fs::create_dir_all(root.join("default")).unwrap();
            let context = Context {
                key: root.display().to_string(),
                path: root.join("config.yaml"),
                instance: root.clone(),
            };
            Self { root, context }
        }
        fn write(&self, text: &str) {
            fs::write(&self.context.path, text).unwrap();
        }
        fn defaults(&self) -> Values {
            crate::pages::tavern::TavernState::default().values()
        }
        fn load(&self) -> Snapshot {
            load(&self.context, &self.defaults()).unwrap()
        }
        fn patch(&self, key: &str, value: Value) -> Patch {
            Patch {
                key: key.into(),
                value,
                expected: self.load().raw[key].clone(),
                revision: 1,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn no_network() -> NetworkOptions {
        NetworkOptions {
            proxy_mode: "none".into(),
            proxy_host: String::new(),
            github_proxy: None,
        }
    }

    #[test]
    fn every_ui_field_has_a_unique_legacy_mapping_and_round_trips() {
        let fixture = Fixture::new();
        let defaults = fixture.defaults();
        assert_eq!(defaults.len(), schema::FIELDS.len());
        let mut keys = HashSet::new();
        let mut paths = HashSet::new();
        let (file, root) = parse("# configuration\n{}\n").unwrap();
        for field in schema::FIELDS {
            assert!(keys.insert(field.key));
            assert!(paths.insert(field.path));
            let raw = schema::encode(field, &defaults[field.key]).unwrap();
            set_at(&root, field, &raw, &defaults).unwrap();
        }
        let loaded = snapshot(file.to_string(), fixture.context.path.clone(), &defaults).unwrap();
        assert_eq!(loaded.values, defaults);
        assert_eq!(loaded.raw["cors_enabled"], Some(json!(true)));
        assert!(file.to_string().contains("cors:"));
        assert!(!file.to_string().contains("corsProxy:"));
    }

    #[test]
    fn scalar_patch_preserves_unknown_fields_comments_and_permissions() {
        let fixture = Fixture::new();
        fixture.write("# leading comment\nport: 8000 # port comment\n# unknown block\ncustomExtension:\n  token: 'quoted text' # keep me\nlisten: false\n");
        fs::set_permissions(&fixture.context.path, fs::Permissions::from_mode(0o640)).unwrap();
        let before = fixture.load();
        let patch = fixture.patch("port", json!(9000));
        let saved = save(
            &fixture.context,
            &before.physical_path,
            &[patch],
            &fixture.defaults(),
        )
        .unwrap();
        assert!(saved.snapshot.text.contains("# leading comment"));
        assert!(saved.snapshot.text.contains("# port comment"));
        assert!(
            saved
                .snapshot
                .text
                .contains("customExtension:\n  token: 'quoted text' # keep me")
        );
        assert_eq!(saved.snapshot.values["port"], "9000");
        assert_eq!(
            fs::metadata(&fixture.context.path).unwrap().mode() & 0o777,
            0o640
        );
        assert_eq!(saved.applied, vec![("port".into(), 1)]);
    }

    #[test]
    fn nested_scalar_and_dimension_patch_leave_siblings_untouched() {
        let fixture = Fixture::new();
        fixture.write("protocol:\n  ipv4: true # keep ipv4\n  ipv6: false\nthumbnails:\n  dimensions:\n    bg: [160, 90]\n    avatar: [96, 144]\n");
        let before = fixture.load();
        let patches = [
            fixture.patch("protocol_ipv6", json!(true)),
            fixture.patch("background_width", json!(320)),
        ];
        let saved = save(
            &fixture.context,
            &before.physical_path,
            &patches,
            &fixture.defaults(),
        )
        .unwrap();
        assert!(saved.snapshot.text.contains("# keep ipv4"));
        assert_eq!(saved.snapshot.values["background_width"], "320");
        assert_eq!(saved.snapshot.values["background_height"], "90");
        assert_eq!(saved.snapshot.values["avatar_width"], "96");
    }

    #[test]
    fn external_changes_merge_other_fields_and_conflict_on_same_field() {
        let fixture = Fixture::new();
        fixture.write("port: 8000\nlisten: false\n");
        let before = fixture.load();
        let patches = [
            fixture.patch("port", json!(9000)),
            fixture.patch("listen", json!(true)),
        ];
        fixture.write("port: 9100\nlisten: false\nexternal: preserved\n");
        let saved = save(
            &fixture.context,
            &before.physical_path,
            &patches,
            &fixture.defaults(),
        )
        .unwrap();
        assert_eq!(saved.conflicts, vec![("port".into(), 1)]);
        assert_eq!(saved.snapshot.values["port"], "9100");
        assert_eq!(saved.snapshot.values["listen"], true);
        assert!(saved.snapshot.text.contains("external: preserved"));
    }

    #[test]
    fn missing_invalid_and_multiple_documents_never_become_defaults_on_disk() {
        let fixture = Fixture::new();
        assert_eq!(
            load(&fixture.context, &fixture.defaults())
                .unwrap_err()
                .kind,
            ErrorKind::Missing
        );
        for text in [
            "- not-a-map\n",
            "port: invalid\n",
            "port: [\n",
            "---\nport: 8000\n---\nport: 9000\n",
            "port: 8000\nport: 9000\n",
            "protocol: true\n",
        ] {
            fixture.write(text);
            assert!(
                load(&fixture.context, &fixture.defaults()).is_err(),
                "{text}"
            );
            assert_eq!(fs::read_to_string(&fixture.context.path).unwrap(), text);
        }
    }

    #[test]
    fn explicit_null_empty_lists_and_unknown_enum_are_preserved() {
        let fixture = Fixture::new();
        fixture.write("cors:\n  maxAge: null\n  allowedHeaders: []\nbrowserLaunch:\n  browser: custom-browser\n");
        let before = fixture.load();
        assert_eq!(before.values["cors_max_age"], "");
        assert_eq!(before.values["browser_type"], "Unknown");
        let patch = fixture.patch("listen", json!(true));
        let after = save(
            &fixture.context,
            &before.physical_path,
            &[patch],
            &fixture.defaults(),
        )
        .unwrap();
        assert!(after.snapshot.text.contains("custom-browser"));
        assert_eq!(after.snapshot.raw["cors_max_age"], Some(Value::Null));
    }

    #[test]
    fn import_merges_new_defaults_and_old_fields_with_atomic_list_replacement() {
        let fixture = Fixture::new();
        let template = "port: 8000\nlisten: false\nprotocol:\n  ipv4: true\n  ipv6: false\nfutureOption: 123\nwhitelist: ['127.0.0.1']\n";
        let current = "# preserve current\nport: 9000\ncustom: {keep: true}\nprotocol:\n  ipv4: false # don't touch\nwhitelist: ['127.0.0.1', '::1']\n";
        let imported = "port: 9100\nprotocol:\n  ipv6: true\nwhitelist: ['::1']\nother: incoming\n";
        let merged = merge_text(template, current, imported, &fixture.defaults()).unwrap();
        let result = snapshot(
            merged.clone(),
            fixture.context.path.clone(),
            &fixture.defaults(),
        )
        .unwrap();
        assert_eq!(result.values["port"], "9100");
        assert_eq!(result.values["listen"], false);
        assert_eq!(result.values["protocol_ipv4"], false);
        assert_eq!(result.values["protocol_ipv6"], true);
        assert_eq!(result.values["whitelist"], json!(["::1"]));
        assert!(merged.contains("# preserve current"));
        assert!(merged.contains("# don't touch"));
        assert!(merged.contains("futureOption"));
        assert!(merged.contains("custom:"));
        assert!(merged.contains("other:"));
    }

    #[test]
    fn import_backs_up_and_reconfirms_changed_source_or_target() {
        let fixture = Fixture::new();
        fixture.write("port: 8000\n");
        fs::write(
            fixture.root.join("default/config.yaml"),
            "port: 8000\nlisten: false\n",
        )
        .unwrap();
        let source = fixture.root.join("import.yml");
        fs::write(&source, "port: 9000\n").unwrap();
        let preview = prepare_import(
            &fixture.context,
            &source,
            &no_network(),
            &fixture.defaults(),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(fixture.load().values["port"], "8000");
        fixture.write("port: 8100\nlisten: true\n");
        let ImportResult::Reconfirm(preview) = import(&preview, &fixture.defaults()).unwrap()
        else {
            panic!("requires reconfirmation");
        };
        assert_eq!(fixture.load().values["port"], "8100");
        fs::write(&source, "port: 9200\n").unwrap();
        let ImportResult::Reconfirm(preview) = import(&preview, &fixture.defaults()).unwrap()
        else {
            panic!("requires reconfirmation");
        };
        let ImportResult::Saved(saved, backup) = import(&preview, &fixture.defaults()).unwrap()
        else {
            panic!("expected saved");
        };
        assert_eq!(saved.values["port"], "9200");
        assert_eq!(saved.values["listen"], true);
        assert_eq!(
            fs::read_to_string(backup).unwrap(),
            "port: 8100\nlisten: true\n"
        );
    }

    #[test]
    fn generation_keeps_template_network_values_and_never_overwrites_existing_file() {
        let fixture = Fixture::new();
        fs::write(
            fixture.root.join("default/config.yaml"),
            "# template\nport: 8000\nlisten: false\nprotocol:\n  ipv6: false\n",
        )
        .unwrap();
        let generated = generate(
            &fixture.context,
            &no_network(),
            &fixture.defaults(),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(generated.values["port"], "8000");
        assert_eq!(generated.values["listen"], false);
        assert_eq!(generated.values["protocol_ipv6"], false);
        assert!(create_new(&fixture.context.path, "port: 9999\n").is_err());
        assert_eq!(fixture.load().values["port"], "8000");
    }

    #[test]
    fn symlink_retarget_and_corrupt_template_stop_without_writing() {
        let fixture = Fixture::new();
        fixture.write("port: 8000\n");
        let other = fixture.root.join("other.yaml");
        fs::write(&other, "port: 9100\n").unwrap();
        let before = fixture.load();
        fs::remove_file(&fixture.context.path).unwrap();
        std::os::unix::fs::symlink(&other, &fixture.context.path).unwrap();
        let patch = Patch {
            key: "port".into(),
            value: json!(9000),
            expected: Some(json!(8000)),
            revision: 1,
        };
        assert_eq!(
            save(
                &fixture.context,
                &before.physical_path,
                &[patch],
                &fixture.defaults()
            )
            .unwrap_err()
            .kind,
            ErrorKind::Changed
        );
        assert_eq!(fs::read_to_string(other).unwrap(), "port: 9100\n");
        fs::remove_file(&fixture.context.path).unwrap();
        fs::write(fixture.root.join("default/config.yaml"), "- invalid\n").unwrap();
        assert!(
            generate(
                &fixture.context,
                &no_network(),
                &fixture.defaults(),
                &mut |_, _| {}
            )
            .is_err()
        );
        assert!(!fixture.context.path.exists());
    }

    #[test]
    fn target_resolution_requires_selection_and_respects_data_mode() {
        assert!(Context::resolve(None, "online", true, "/custom").is_none());
        let local = Context::resolve(Some("/fixture/local"), "local", false, "/ignored").unwrap();
        assert_eq!(local.path, Path::new("/fixture/local/config.yaml"));
        let global = Context::resolve(Some("/fixture/local"), "local", true, "/custom").unwrap();
        assert_eq!(global.path, Path::new("/custom/config.yaml"));
        assert_ne!(local.key, global.key);
    }
}
