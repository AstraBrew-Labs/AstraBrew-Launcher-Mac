//! 启动器字体目录、动态字体加载与界面缩放规范。
//!
//! macOS 字体只在启动时扫描元数据；真正的字体文件按用户选择加载，避免把
//! 所有系统字体长期放入内存。渲染字体通过线程局部状态传递给各页面，与语言
//! 模块采用相同方式，保证构建同一帧时使用一致的字体族。

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use fontdb::Source;
use iced::Font;
use iced::font::{Family, Stretch, Style, Weight};

/// settings.json 中表示内置默认字体的稳定值。
pub(crate) const DEFAULT_FONT_KEY: &str = "default";
/// 用户可选的最小界面缩放。
pub(crate) const MIN_UI_SCALE: f32 = 0.90;
/// 用户可选的最大界面缩放。
pub(crate) const MAX_UI_SCALE: f32 = 1.50;
/// 界面缩放的调节步长。
pub(crate) const UI_SCALE_STEP: f32 = 0.05;
/// 新安装及旧配置缺少字段时使用的默认缩放。
pub(crate) const DEFAULT_UI_SCALE: f32 = 1.10;

/// 可在字体搜索框中展示的一项字体族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FontChoice {
    key: &'static str,
    family: Option<&'static str>,
}

impl FontChoice {
    /// 内置 HarmonyOS Sans 选项。
    pub(crate) const fn default_choice() -> Self {
        Self {
            key: DEFAULT_FONT_KEY,
            family: None,
        }
    }

    /// settings.json 中使用的稳定键。
    pub(crate) const fn key(self) -> &'static str {
        self.key
    }

    /// 传给 iced 字体选择器的真实字体族名。
    pub(crate) const fn family(self) -> Option<&'static str> {
        self.family
    }
}

impl Default for FontChoice {
    fn default() -> Self {
        Self::default_choice()
    }
}

impl fmt::Display for FontChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(family) = self.family {
            f.write_str(family)
        } else {
            f.write_str(crate::lang::t(
                "settings.interface.font.default",
                crate::lang::lang::current_language(),
            ))
        }
    }
}

/// macOS 可见字体族目录。
#[derive(Debug, Clone)]
pub(crate) struct SystemFontCatalog {
    choices: Arc<Vec<FontChoice>>,
    sources: Arc<HashMap<&'static str, Vec<PathBuf>>>,
}

impl Default for SystemFontCatalog {
    fn default() -> Self {
        Self {
            choices: Arc::new(vec![FontChoice::default_choice()]),
            sources: Arc::new(HashMap::new()),
        }
    }
}

impl SystemFontCatalog {
    /// 扫描 macOS 系统、网络和当前用户字体目录。
    pub(crate) fn discover() -> Self {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();

        let mut families: BTreeMap<String, (String, BTreeSet<PathBuf>)> = BTreeMap::new();
        for face in database.faces() {
            if face.style != fontdb::Style::Normal {
                continue;
            }
            let Some((family, _)) = face.families.first() else {
                continue;
            };
            let family = family.trim();
            if !is_visible_family(family) {
                continue;
            }

            let path = match &face.source {
                Source::File(path) | Source::SharedFile(path, _) => Some(path.clone()),
                Source::Binary(_) => None,
            };
            if let Some(path) = path {
                let normalized = family.to_lowercase();
                families
                    .entry(normalized)
                    .or_insert_with(|| (family.to_owned(), BTreeSet::new()))
                    .1
                    .insert(path);
            }
        }

        let mut entries = families
            .into_values()
            .filter(|(_, paths)| !paths.is_empty())
            .collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| {
            left.to_lowercase()
                .cmp(&right.to_lowercase())
                .then_with(|| left.cmp(right))
        });

        let mut choices = Vec::with_capacity(entries.len() + 1);
        choices.push(FontChoice::default_choice());
        let mut sources = HashMap::with_capacity(entries.len());
        for (family, paths) in entries {
            // 字体目录在进程生命周期内固定，泄漏少量名称可满足 iced 对静态族名的要求。
            let family: &'static str = Box::leak(family.into_boxed_str());
            choices.push(FontChoice {
                key: family,
                family: Some(family),
            });
            sources.insert(family, paths.into_iter().collect());
        }

        Self {
            choices: Arc::new(choices),
            sources: Arc::new(sources),
        }
    }

    /// 返回可供搜索框使用的全部选项。
    pub(crate) fn choices(&self) -> Vec<FontChoice> {
        self.choices.as_ref().clone()
    }

    /// 将保存值解析为当前机器真实存在的字体，缺失时回退默认字体。
    pub(crate) fn resolve(&self, key: &str) -> FontChoice {
        if key.is_empty() || key == DEFAULT_FONT_KEY {
            return FontChoice::default_choice();
        }
        self.choices
            .iter()
            .copied()
            .find(|choice| choice.key.eq_ignore_ascii_case(key))
            .unwrap_or_default()
    }

    /// 读取字体族关联的去重字体文件，供 iced 注册所有可用字重。
    pub(crate) fn load_family_bytes(&self, choice: FontChoice) -> Result<Vec<Vec<u8>>, String> {
        let Some(family) = choice.family else {
            return Ok(Vec::new());
        };
        let Some(paths) = self.sources.get(family) else {
            return Err(format!("找不到字体族“{family}”的字体文件。"));
        };

        paths
            .iter()
            .map(|path| {
                std::fs::read(path)
                    .map_err(|error| format!("无法读取字体 {}：{error}", path.display()))
            })
            .collect()
    }
}

/// 判断字体族是否适合出现在用户选择器中。
fn is_visible_family(family: &str) -> bool {
    let family = family.trim();
    !family.is_empty() && !family.starts_with('.') && !family.eq_ignore_ascii_case("HarmonyOS Sans")
}

thread_local! {
    /// 当前帧普通界面文字使用的字体族。
    static RENDER_FAMILY: Cell<Family> = const {
        Cell::new(Family::Name("HarmonyOS Sans"))
    };
    /// 当前帧用户选择的额外界面缩放。
    static RENDER_SCALE: Cell<f32> = const { Cell::new(DEFAULT_UI_SCALE) };
}

/// 在构建当前帧前设置普通界面字体。
pub(crate) fn set_render_font(choice: FontChoice) {
    RENDER_FAMILY.set(match choice.family() {
        Some(family) => Family::Name(family),
        None => Family::Name("HarmonyOS Sans"),
    });
}

/// 在构建当前帧前记录界面缩放，供响应式组件选择排版。
pub(crate) fn set_render_scale(scale: f32) {
    RENDER_SCALE.set(normalize_ui_scale(scale));
}

/// 当前帧使用的用户界面缩放。
pub(crate) fn current_ui_scale() -> f32 {
    RENDER_SCALE.get()
}

fn with_weight(weight: Weight) -> Font {
    RENDER_FAMILY.with(|family| Font {
        family: family.get(),
        weight,
        stretch: Stretch::Normal,
        style: Style::Normal,
    })
}

/// 普通字重。
pub(crate) fn regular() -> Font {
    with_weight(Weight::Normal)
}

/// 中等字重。
pub(crate) fn medium() -> Font {
    with_weight(Weight::Medium)
}

/// 粗体字重。
pub(crate) fn bold() -> Font {
    with_weight(Weight::Bold)
}

/// 将任意设置值限制到合法范围并吸附到 5% 档位。
pub(crate) fn normalize_ui_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return DEFAULT_UI_SCALE;
    }
    let clamped = value.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
    let steps = ((clamped - MIN_UI_SCALE) / UI_SCALE_STEP).round();
    (MIN_UI_SCALE + steps * UI_SCALE_STEP).clamp(MIN_UI_SCALE, MAX_UI_SCALE)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_UI_SCALE, MAX_UI_SCALE, MIN_UI_SCALE, is_visible_family, normalize_ui_scale,
    };

    #[test]
    fn scale_is_clamped_and_snapped() {
        assert_eq!(normalize_ui_scale(0.1), MIN_UI_SCALE);
        assert_eq!(normalize_ui_scale(2.0), MAX_UI_SCALE);
        assert!((normalize_ui_scale(1.13) - 1.15).abs() < f32::EPSILON);
        assert_eq!(normalize_ui_scale(f32::NAN), DEFAULT_UI_SCALE);
    }

    #[test]
    fn hidden_and_empty_font_families_are_filtered() {
        assert!(!is_visible_family(""));
        assert!(!is_visible_family("  "));
        assert!(!is_visible_family(".AppleSystemUIFont"));
        assert!(!is_visible_family("HarmonyOS Sans"));
        assert!(is_visible_family("PingFang SC"));
    }
}
