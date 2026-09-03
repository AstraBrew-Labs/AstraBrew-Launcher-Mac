//! 启动器国际化模块。

pub(crate) mod en;
pub(crate) mod lang;
pub(crate) mod zh;

pub(crate) use lang::{Language, display_label, effective_language, set_language, t, text};
