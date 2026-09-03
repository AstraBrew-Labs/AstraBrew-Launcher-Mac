//! 启动器偏好设置的持久化。
//!
//! 本模块只维护当前版本已经接入的界面偏好，同时保留旧版配置文件中的
//! 其他字段，避免新旧版本交替使用时丢失尚未迁移的设置。

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

impl DisplayLanguage {
    pub const ALL: [Self; 3] = [Self::System, Self::SimplifiedChinese, Self::English];
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
            crate::lang::Language::English => {
                f.write_str(&crate::lang::en::translate_owned(label))
            }
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

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
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
            crate::lang::Language::English => {
                f.write_str(&crate::lang::en::translate_owned(label))
            }
        }
    }
}

/// 已接入持久化的用户偏好。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersistentPreferences {
    pub language: DisplayLanguage,
    pub theme: ThemeMode,
    pub remember_window_position: bool,
    pub window_position: Option<[f32; 2]>,
}

impl Default for PersistentPreferences {
    fn default() -> Self {
        Self {
            language: DisplayLanguage::System,
            theme: ThemeMode::System,
            remember_window_position: true,
            window_position: None,
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
        .and_then(Value::as_bool)
        .unwrap_or(defaults.remember_window_position);
    let window_position = document
        .get("window_position")
        .cloned()
        .and_then(|value| serde_json::from_value::<[f32; 2]>(value).ok())
        .filter(|[x, y]| x.is_finite() && y.is_finite());

    PersistentPreferences {
        language,
        theme,
        remember_window_position,
        window_position,
    }
}

fn default_settings_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
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
