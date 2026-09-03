//! 语言解析与翻译入口。

use std::cell::Cell;
use std::sync::OnceLock;

use crate::core::settings::DisplayLanguage;

/// 渲染时实际使用的语言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Chinese,
    English,
}

static SYSTEM_LANGUAGE: OnceLock<Language> = OnceLock::new();

thread_local! {
    /// iced 在同一 UI 线程构建并绘制元素；这里保存当前视图的只读渲染语言。
    static RENDER_LANGUAGE: Cell<Language> = const { Cell::new(Language::English) };
}

/// 将用户选择解析为实际语言。
pub fn effective_language(language: DisplayLanguage) -> Language {
    match language {
        DisplayLanguage::SimplifiedChinese => Language::Chinese,
        DisplayLanguage::English => Language::English,
        DisplayLanguage::System => *SYSTEM_LANGUAGE.get_or_init(detect_system_language),
    }
}

/// 在构建当前帧前设置渲染语言。
pub fn set_language(language: Language) {
    RENDER_LANGUAGE.set(language);
}

/// 当前帧使用的语言，供需要实现 `Display` 的控件选项读取。
pub fn current_language() -> Language {
    RENDER_LANGUAGE.get()
}

/// 将枚举等运行时 `Display` 文案转换为当前帧语言。
pub fn display_label(content: &str) -> String {
    match current_language() {
        Language::Chinese => content.to_owned(),
        Language::English => super::en::translate_owned(content),
    }
}

/// 根据 macOS 首选语言解析系统语言，无法识别时回退英文。
fn detect_system_language() -> Language {
    if let Ok(locale) = std::env::var("LANG")
        && locale.to_ascii_lowercase().starts_with("zh")
    {
        return Language::Chinese;
    }

    if let Ok(output) = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        && String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("zh")
    {
        return Language::Chinese;
    }

    Language::English
}

/// 翻译静态界面文案。
pub fn t(key: &'static str, language: Language) -> &'static str {
    match language {
        Language::Chinese => super::zh::translate(key),
        Language::English => super::en::translate(key),
    }
}

/// 构造会自动翻译内容的 iced 文本控件。
pub fn text<'a>(content: impl ToString) -> iced::widget::Text<'a> {
    let content = content.to_string();
    let translated = match current_language() {
        Language::Chinese => content,
        Language::English => super::en::translate_owned(&content),
    };
    iced::widget::text(translated)
}
