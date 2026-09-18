//! 启动器国际化模块。
//!
//! 对外只暴露「键」驱动的一套 API，界面代码不得直接书写具体文案：
//!
//! - [`t`] / [`tf`]：取当前语言的静态文案或带占位符的模板；
//! - [`t_in`] / [`tf_in`]：显式指定语言取文案；
//! - [`text`] / [`textf`]：直接构造 iced 文本控件；
//! - [`raw`]：构造不参与翻译的原始文本（版本号、路径等运行时数据）。
//!
//! 键到文案的映射由 [`zh`] 与 [`en`] 两张键值表提供，两表键集合必须一致。

pub(crate) mod en;
pub(crate) mod lang;
pub(crate) mod zh;

pub(crate) use lang::{
    Language, current_language, effective_language, raw, resolve, set_language, t, t_in, text,
    textf, tf,
};


/// 将 Git 克隆进度中的英文阶段名映射为当前语言文案。
///
/// Git 自身输出的阶段名不属于界面文案，因此在这里集中做「阶段名 → 键」的
/// 映射，认不出的阶段名原样返回。
pub(crate) fn github_clone_stage_label(stage: &str) -> String {
    let key = match stage {
        "Enumerating objects" => "git.stage.enumerating",
        "Counting objects" => "git.stage.counting",
        "Compressing objects" => "git.stage.compressing",
        "Receiving objects" => "git.stage.receiving",
        "Resolving deltas" => "git.stage.resolving",
        _ => return stage.to_owned(),
    };
    t(key).to_owned()
}

/// Git 克隆进度的占位文案。
pub(crate) fn github_clone_preparing_label() -> &'static str {
    t("git.clone.preparing")
}

/// Git 克隆进度中的“进行中”状态。
pub(crate) fn github_clone_in_progress_label() -> &'static str {
    t("git.clone.in_progress")
}

/// Git 克隆进度中的对象数量说明。
pub(crate) fn github_clone_objects_label(current: u64, total: u64) -> String {
    tf(
        "git.clone.objects",
        &[("current", &current.to_string()), ("total", &total.to_string())],
    )
}
