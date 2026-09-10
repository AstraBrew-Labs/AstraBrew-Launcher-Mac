//! 启动器国际化模块。

pub(crate) mod en;
pub(crate) mod lang;
pub(crate) mod zh;

pub(crate) use lang::{
    current_language, display_label, effective_language, set_language, t, text, Language,
};

/// 将 Git 克隆阶段文案转成当前语言。
pub(crate) fn github_clone_stage_label(stage: &str) -> String {
    match lang::current_language() {
        Language::Chinese => match stage {
            "Enumerating objects" => "正在枚举对象".to_owned(),
            "Counting objects" => "正在计数对象".to_owned(),
            "Compressing objects" => "正在压缩对象".to_owned(),
            "Receiving objects" => "正在接收对象".to_owned(),
            "Resolving deltas" => "正在解析差异".to_owned(),
            _ => stage.to_owned(),
        },
        Language::English => stage.to_owned(),
    }
}

/// Git 克隆进度的占位文案。
pub(crate) fn github_clone_preparing_label() -> &'static str {
    match lang::current_language() {
        Language::Chinese => "准备克隆",
        Language::English => "Preparing clone",
    }
}

/// Git 克隆进度中的“进行中”状态。
pub(crate) fn github_clone_in_progress_label() -> &'static str {
    match lang::current_language() {
        Language::Chinese => "进行中",
        Language::English => "In progress",
    }
}

/// Git 克隆进度中的对象数量说明。
pub(crate) fn github_clone_objects_label(current: u64, total: u64) -> String {
    match lang::current_language() {
        Language::Chinese => format!("{current} / {total} 个对象"),
        Language::English => format!("{current} / {total} objects"),
    }
}
