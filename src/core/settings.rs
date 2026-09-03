//! 启动器偏好设置的持久化。
//!
//! 本模块只维护当前版本已经接入的界面偏好，同时保留旧版配置文件中的
//! 其他字段，避免新旧版本交替使用时丢失尚未迁移的设置。

pub(crate) mod env_detect;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 启动器显示语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DisplayLanguage {
    #[serde(rename = "Chinese", alias = "SimplifiedChinese")]
    SimplifiedChinese,
    English,
    #[default]
    System,
}

impl fmt::Display for DisplayLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SimplifiedChinese => "简体中文",
            Self::English => "English",
            Self::System => "跟随系统",
        };
        match crate::lang::lang::current_language() {
            crate::lang::Language::Chinese => f.write_str(label),
            crate::lang::Language::English => f.write_str(&crate::lang::en::translate_owned(label)),
        }
    }
}

/// 启动器界面主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Light => "浅色",
            Self::Dark => "深色",
            Self::System => "跟随系统",
        };
        match crate::lang::lang::current_language() {
            crate::lang::Language::Chinese => f.write_str(label),
            crate::lang::Language::English => f.write_str(&crate::lang::en::translate_owned(label)),
        }
    }
}

/// 已接入持久化的用户偏好。
#[derive(Debug, Clone, PartialEq)]
pub struct PersistentPreferences {
    pub language: DisplayLanguage,
    pub theme: ThemeMode,
    pub remember_window_position: bool,
    pub window_position: Option<[f32; 2]>,
    /// 网络代理模式：none、system 或 custom。
    pub proxy_mode: String,
    /// 自定义代理地址，即使当前未选择自定义模式也保留。
    pub custom_proxy: String,
    /// 旧版 githubProxy 配置。
    pub github_proxy_enabled: bool,
    pub github_proxy_url: String,
    /// 旧版 npmRegistry 配置。
    pub npm_registry: String,
    /// 是否启用软件自启动。
    pub auto_start: bool,
    /// 酒馆数据模式：global 或 current。
    pub data_mode: String,
    /// 全局数据存放位置。
    pub global_data_path: String,
    /// 导出保存目录。
    pub tavern_export_path: String,
    /// 酒馆启动模式：normal 或 desktop。
    pub start_mode: String,
    /// 是否启用服务器模式。
    pub server_mode_enabled: bool,
}

impl Default for PersistentPreferences {
    fn default() -> Self {
        Self {
            language: DisplayLanguage::System,
            theme: ThemeMode::System,
            remember_window_position: true,
            window_position: None,
            proxy_mode: "system".to_owned(),
            custom_proxy: String::new(),
            github_proxy_enabled: false,
            github_proxy_url: "https://ghfast.top/".to_owned(),
            npm_registry: "https://registry.npmmirror.com/".to_owned(),
            auto_start: false,
            data_mode: "current".to_owned(),
            global_data_path: "~/Library/Application Support/AstraBrew/data".to_owned(),
            tavern_export_path: "~/Downloads".to_owned(),
            start_mode: "normal".to_owned(),
            server_mode_enabled: false,
        }
    }
}

/// 保留原始 JSON 的设置存储器。
#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
    document: Map<String, Value>,
}

impl SettingsStore {
    /// 从默认的 macOS Application Support 目录加载配置。
    pub fn load_default() -> (Self, PersistentPreferences) {
        Self::load(default_settings_path())
    }

    /// 从指定路径加载配置，便于测试时隔离真实用户数据。
    pub fn load(path: impl Into<PathBuf>) -> (Self, PersistentPreferences) {
        let path = path.into();
        let document = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        let preferences = preferences_from_document(&document);

        (Self { path, document }, preferences)
    }

    /// 将偏好合并回旧版配置并进行原子替换。
    pub fn save(&mut self, preferences: PersistentPreferences) -> io::Result<()> {
        // 保持旧版 settings.json 的字段结构，方便老用户直接升级。
        self.document.insert(
            "language".into(),
            serde_json::to_value(preferences.language).map_err(io::Error::other)?,
        );
        self.document.insert(
            "theme".into(),
            serde_json::to_value(preferences.theme).map_err(io::Error::other)?,
        );
        self.document.insert(
            "remember_window_pos".into(),
            Value::Bool(preferences.remember_window_position),
        );
        self.document.remove("remember_window_position");
        self.document.insert(
            "window_position".into(),
            serde_json::to_value(preferences.window_position).map_err(io::Error::other)?,
        );
        self.document.insert(
            "github_proxy_enabled".into(),
            Value::Bool(preferences.github_proxy_enabled),
        );
        self.document.insert(
            "github_proxy_url".into(),
            Value::String(preferences.github_proxy_url),
        );
        self.document.insert(
            "npm_registry".into(),
            Value::String(preferences.npm_registry),
        );
        self.document
            .insert("auto_start".into(), Value::Bool(preferences.auto_start));
        self.document.insert(
            "data_mode".into(),
            Value::String(normalize_data_mode(&preferences.data_mode).to_owned()),
        );
        self.document.insert(
            "global_data_path".into(),
            Value::String(preferences.global_data_path),
        );
        self.document.insert(
            "tavern_export_path".into(),
            Value::String(preferences.tavern_export_path),
        );
        self.document.insert(
            "start_mode".into(),
            Value::String(legacy_start_mode(&preferences.start_mode).to_owned()),
        );
        self.document.insert(
            "server_mode_enabled".into(),
            Value::Bool(preferences.server_mode_enabled),
        );
        self.document.insert(
            "proxy_type".into(),
            Value::String(legacy_proxy_type(&preferences.proxy_mode).to_owned()),
        );
        self.document.insert(
            "custom_proxy".into(),
            Value::String(preferences.custom_proxy),
        );

        // 清理过渡期写入的其他键，避免与旧版结构混杂。
        for key in [
            "lang",
            "rememberWindowPosition",
            "windowPosition",
            "githubProxy",
            "npmRegistry",
            "networkProxy",
            "proxy_mode",
            "proxyMode",
            "customProxy",
            "dataMode",
            "globalDataPath",
            "tavernExportPath",
            "startMode",
            "serverModeEnabled",
        ] {
            self.document.remove(key);
        }

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = serde_json::to_vec_pretty(&self.document).map_err(io::Error::other)?;
        let temporary = temporary_path(&self.path);
        fs::write(&temporary, contents)?;
        fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}

fn preferences_from_document(document: &Map<String, Value>) -> PersistentPreferences {
    let defaults = PersistentPreferences::default();
    let language = document
        .get("language")
        .or_else(|| document.get("lang"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or(defaults.language);
    let theme = document
        .get("theme")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or(defaults.theme);
    let remember_window_position = document
        .get("remember_window_pos")
        .or_else(|| document.get("remember_window_position"))
        .or_else(|| document.get("rememberWindowPosition"))
        .and_then(Value::as_bool)
        .unwrap_or(defaults.remember_window_position);
    let window_position = document
        .get("window_position")
        .or_else(|| document.get("windowPosition"))
        .and_then(parse_window_position)
        .filter(|[x, y]| x.is_finite() && y.is_finite());
    let proxy_mode = document
        .get("proxy_type")
        .or_else(|| document.get("proxy_mode"))
        .or_else(|| {
            document
                .get("networkProxy")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("mode"))
        })
        .and_then(Value::as_str)
        .map(normalize_proxy_type)
        .unwrap_or_else(|| defaults.proxy_mode.clone());
    let custom_proxy = document
        .get("custom_proxy")
        .or_else(|| document.get("customProxy"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            document
                .get("networkProxy")
                .and_then(Value::as_object)
                .and_then(|obj| {
                    let host = obj.get("host").and_then(Value::as_str)?;
                    let port = obj.get("port").and_then(Value::as_u64).unwrap_or(7890);
                    Some(format!("{host}:{port}"))
                })
        })
        .unwrap_or_else(|| defaults.custom_proxy.clone());
    let github_proxy_enabled = document
        .get("github_proxy_enabled")
        .or_else(|| {
            document
                .get("githubProxy")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("enable"))
        })
        .and_then(Value::as_bool)
        .unwrap_or(defaults.github_proxy_enabled);
    let github_proxy_url = document
        .get("github_proxy_url")
        .or_else(|| {
            document
                .get("githubProxy")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("url"))
        })
        .and_then(Value::as_str)
        .filter(|url| !url.trim().is_empty())
        .unwrap_or(&defaults.github_proxy_url)
        .to_owned();
    let npm_registry = document
        .get("npm_registry")
        .or_else(|| document.get("npmRegistry"))
        .and_then(Value::as_str)
        .filter(|url| !url.trim().is_empty())
        .unwrap_or(&defaults.npm_registry)
        .to_owned();
    let auto_start = document
        .get("auto_start")
        .or_else(|| document.get("autoStart"))
        .and_then(Value::as_bool)
        .unwrap_or(defaults.auto_start);
    let data_mode = document
        .get("data_mode")
        .or_else(|| document.get("dataMode"))
        .and_then(Value::as_str)
        .map(normalize_data_mode)
        .unwrap_or_else(|| defaults.data_mode.clone());
    let global_data_path = document
        .get("global_data_path")
        .or_else(|| document.get("globalDataPath"))
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(&defaults.global_data_path)
        .to_owned();
    let tavern_export_path = document
        .get("tavern_export_path")
        .or_else(|| document.get("tavernExportPath"))
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(&defaults.tavern_export_path)
        .to_owned();
    let start_mode = document
        .get("start_mode")
        .or_else(|| document.get("startMode"))
        .or_else(|| document.get("launchMode"))
        .and_then(Value::as_str)
        .map(normalize_start_mode)
        .unwrap_or_else(|| defaults.start_mode.clone());
    let server_mode_enabled = document
        .get("server_mode_enabled")
        .or_else(|| document.get("serverModeEnabled"))
        .and_then(Value::as_bool)
        .unwrap_or(defaults.server_mode_enabled);

    PersistentPreferences {
        language,
        theme,
        remember_window_position,
        window_position,
        proxy_mode,
        custom_proxy,
        github_proxy_enabled,
        github_proxy_url,
        npm_registry,
        auto_start,
        data_mode,
        global_data_path,
        tavern_export_path,
        start_mode,
        server_mode_enabled,
    }
}

fn parse_window_position(value: &Value) -> Option<[f32; 2]> {
    if let Some(position) = value.as_object() {
        let x = position.get("x").and_then(Value::as_f64)? as f32;
        let y = position.get("y").and_then(Value::as_f64)? as f32;
        return Some([x, y]);
    }
    serde_json::from_value::<[f32; 2]>(value.clone()).ok()
}

fn normalize_proxy_type(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" | "direct" | "off" | "关闭" | "直连" => "none".to_owned(),
        "custom" | "自定义" | "自定义代理" => "custom".to_owned(),
        _ => "system".to_owned(),
    }
}

fn normalize_data_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "global" => "global".to_owned(),
        _ => "current".to_owned(),
    }
}

fn normalize_start_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "desktop" => "desktop".to_owned(),
        _ => "normal".to_owned(),
    }
}

fn legacy_start_mode(value: &str) -> &'static str {
    match value {
        "desktop" => "Desktop",
        _ => "Normal",
    }
}

fn legacy_proxy_type(value: &str) -> &'static str {
    match value {
        "none" => "None",
        "custom" => "Custom",
        _ => "System",
    }
}

fn default_settings_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Library")
        .join("Application Support")
        .join("AstraBrew Launcher")
        .join("settings.json")
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!(".{name}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::{DisplayLanguage, PersistentPreferences, SettingsStore, ThemeMode};
    use std::fs;

    fn test_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "astrabrew-settings-{name}-{}.json",
            std::process::id()
        ))
    }

    #[test]
    fn reads_legacy_names_and_preserves_unknown_fields() {
        let path = test_path("legacy");
        fs::write(
            &path,
            r#"{"language":"Chinese","theme":"Dark","remember_window_pos":false,"window_position":[-120.0,48.0],"proxy_type":"Custom"}"#,
        )
        .expect("write fixture");

        let (mut store, preferences) = SettingsStore::load(&path);
        assert_eq!(preferences.language, DisplayLanguage::SimplifiedChinese);
        assert_eq!(preferences.theme, ThemeMode::Dark);
        assert!(!preferences.remember_window_position);

        store
            .save(PersistentPreferences {
                language: DisplayLanguage::English,
                ..preferences
            })
            .expect("save settings");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read settings"))
                .expect("parse settings");
        assert_eq!(value["proxy_type"], "Custom");
        assert_eq!(value["language"], "English");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn persists_proxy_mode_and_custom_proxy() {
        let path = test_path("proxy");
        let (mut store, _) = SettingsStore::load(&path);
        store
            .save(PersistentPreferences {
                proxy_mode: "system".to_owned(),
                custom_proxy: "127.0.0.1:7890".to_owned(),
                ..PersistentPreferences::default()
            })
            .expect("save proxy settings");

        let (_, preferences) = SettingsStore::load(&path);
        assert_eq!(preferences.proxy_mode, "system");
        assert_eq!(preferences.custom_proxy, "127.0.0.1:7890");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read settings"))
                .expect("parse settings");
        assert_eq!(value["proxy_mode"], "system");
        assert_eq!(value["custom_proxy"], "127.0.0.1:7890");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_proxy_mode_defaults_to_follow_system() {
        let path = test_path("proxy-default");
        fs::write(&path, r#"{"language":"English"}"#).expect("write fixture");
        let (_, preferences) = SettingsStore::load(&path);
        assert_eq!(preferences.proxy_mode, "system");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn accepts_current_aliases_and_recovers_from_invalid_json() {
        let alias_path = test_path("aliases");
        fs::write(
            &alias_path,
            r#"{"language":"SimplifiedChinese","remember_window_position":false}"#,
        )
        .expect("write fixture");
        let (_, preferences) = SettingsStore::load(&alias_path);
        assert_eq!(preferences.language, DisplayLanguage::SimplifiedChinese);
        assert!(!preferences.remember_window_position);
        let _ = fs::remove_file(alias_path);

        let invalid_path = test_path("invalid");
        fs::write(&invalid_path, "not json").expect("write fixture");
        let (_, invalid) = SettingsStore::load(&invalid_path);
        assert_eq!(invalid, PersistentPreferences::default());
        let _ = fs::remove_file(invalid_path);
    }
}
