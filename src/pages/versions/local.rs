//! 本地实例弹窗和轻提示；后台任务生命周期保存在应用层，不依赖这些视图。

use super::VersionMessage;
use crate::core::local_instances::{LocalError, scan::ScanProgress};
use crate::lang::lang::current_language;
use crate::lang::{t, text};
use crate::theme::button_style;
use astra_ui::{ButtonVariant, INK_MUTED, fonts, icons};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, stack};
use iced::{Alignment, Element, Fill};
use lucide_icons::Icon;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 关闭弹窗只隐藏视图；主动取消整轮任务由应用层单独处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanPhase {
    #[default]
    Idle,
    Running,
    Completed,
    CompletedPartial,
    Failed,
    Cancelled,
}

impl ScanPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "尚未开始扫描。",
            Self::Running => "扫描进度",
            Self::Completed => "扫描完成",
            Self::CompletedPartial => "扫描已完成，但结果不完整。",
            Self::Failed => "扫描已终止，结果可能不完整。",
            Self::Cancelled => "扫描已取消。",
        }
    }
    pub fn active(self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScanState {
    pub started_at: Option<Instant>,
    /// 旧版只保留最近五条扫描路径作为进度提示。
    pub recent_paths: VecDeque<String>,
    pub cancel_confirm_visible: bool,
    pub show_details: bool,
    pub auto_hide_at: Option<Instant>,
    pub visible: bool,
    pub phase: ScanPhase,
    pub progress: ScanProgress,
    pub added: u64,
    pub duplicates: u64,
    pub logs: VecDeque<String>,
    pub error: Option<LocalError>,
}

impl ScanState {
    pub fn status_label(&self) -> &'static str {
        self.phase.label()
    }

    pub fn observe_path(&mut self, path: String) {
        if path.is_empty() {
            return;
        }
        self.progress.path = path.clone();
        if self.recent_paths.back() == Some(&path) {
            return;
        }
        while self.recent_paths.len() >= 5 {
            self.recent_paths.pop_front();
        }
        self.recent_paths.push_back(path);
    }
}

/// 与旧版一样保留路径首尾；控制字符仅在展示时转义，实际文件路径不改变。
pub fn truncate_path(path: &str, max: usize) -> String {
    let display = path
        .replace('\n', "↵")
        .replace('\r', "␍")
        .replace('\t', "⇥");
    let count = display.chars().count();
    if count <= max {
        return display;
    }
    let head = max / 3;
    let tail = max.saturating_sub(head + 3);
    let start: String = display.chars().take(head).collect();
    let end: String = display.chars().skip(count - tail).collect();
    format!("{start}...{end}")
}

/// 详细日志单独限流，成功收起进度提示后仍可重新查看。
pub fn append_log(logs: &mut VecDeque<String>, line: String) {
    for line in line.lines() {
        while logs.len() >= 600 {
            logs.pop_front();
        }
        logs.push_back(line.chars().take(2000).collect());
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalInstallState {
    pub visible: bool,
    pub running: bool,
    pub path: Option<String>,
    pub logs: VecDeque<String>,
    pub error: Option<LocalError>,
}

#[derive(Debug, Clone)]
pub struct ToastNotice {
    pub message: &'static str,
    pub detail: String,
    pub danger: bool,
    pub until: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct LocalUiState {
    pub loading: bool,
    pub import_pending: bool,
    pub animation_frame: u16,
    pub scan: ScanState,
    pub install: LocalInstallState,
    pub toast: Option<ToastNotice>,
}

impl LocalUiState {
    pub fn notify(&mut self, message: &'static str, detail: impl ToString, danger: bool) {
        let detail = detail.to_string();
        let mut summary: String = detail.chars().take(240).collect();
        if detail.chars().count() > 240 {
            summary.push('…');
        }
        self.toast = Some(ToastNotice {
            message,
            detail: summary,
            danger,
            until: Instant::now() + Duration::from_secs(5),
        });
    }
    pub fn report(&mut self, error: &LocalError) {
        self.notify(error.message, &error.detail, true);
    }
    pub fn tick(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 20;
        if self
            .scan
            .auto_hide_at
            .is_some_and(|at| Instant::now() >= at)
        {
            self.scan.visible = false;
            self.scan.auto_hide_at = None;
        }
        if self.scan.phase.active()
            && let Some(started) = self.scan.started_at
        {
            self.scan.progress.elapsed_seconds = started.elapsed().as_secs();
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| Instant::now() >= toast.until)
        {
            self.toast = None;
        }
    }
    pub fn close_scan(&mut self) {
        self.scan.visible = false;
        self.scan.cancel_confirm_visible = false;
    }
}

fn modal<'a>(
    content: Element<'a, VersionMessage>,
    close: VersionMessage,
) -> Element<'a, VersionMessage> {
    sized_modal(content, close, 720.0)
}

fn sized_modal<'a>(
    content: Element<'a, VersionMessage>,
    close: VersionMessage,
    width: f32,
) -> Element<'a, VersionMessage> {
    stack![
        button(space::Space::new())
            .on_press(close)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(super::install_backdrop_style),
        container(
            mouse_area(
                container(content)
                    .width(width)
                    .padding(22)
                    .style(super::install_modal_style)
            )
            .on_press(VersionMessage::LocalModalInteract)
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(24),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}

fn log_view(logs: &VecDeque<String>, height: f32) -> Element<'_, VersionMessage> {
    scrollable(
        column(logs.iter().map(|line| text(line).size(11).into()))
            .spacing(3)
            .width(Fill),
    )
    .height(height)
    .into()
}

fn error_view(error: Option<&LocalError>) -> Element<'_, VersionMessage> {
    match error {
        Some(error) => column![
            text(error.message).size(12).color(astra_ui::DANGER),
            scrollable(text(&error.detail).size(11)).height(45)
        ]
        .spacing(4)
        .into(),
        None => space::vertical().height(0).into(),
    }
}

pub fn modal_view(state: &LocalUiState) -> Option<Element<'_, VersionMessage>> {
    if state.install.visible {
        let task = &state.install;
        let mut footer = row![space::horizontal()].spacing(8);
        if !task.running {
            if task.error.is_some()
                && let Some(path) = &task.path
            {
                footer = footer.push(
                    button(text("重试安装"))
                        .on_press(VersionMessage::InstallLocalDependencies(path.clone()))
                        .style(button_style(ButtonVariant::Primary)),
                );
            }
            footer = footer.push(
                button(text("关闭"))
                    .on_press(VersionMessage::CloseLocalInstall)
                    .style(button_style(ButtonVariant::Secondary)),
            );
        }
        let body = column![
            text("安装本地实例依赖").size(19).font(fonts::MEDIUM),
            scrollable(
                text(task.path.as_deref().unwrap_or(""))
                    .size(12)
                    .color(INK_MUTED)
            )
            .height(36),
            text(if task.running {
                "正在安装并检查运行依赖…"
            } else if task.error.is_some() {
                "安装依赖失败，请查看日志后重试。"
            } else {
                "运行依赖已安装完成，请手动切换版本。"
            })
            .size(13),
            log_view(&task.logs, 190.0),
            error_view(task.error.as_ref()),
            footer,
        ]
        .spacing(14)
        .into();
        return Some(modal(body, VersionMessage::LocalModalInteract));
    }
    let scan = &state.scan;
    if scan.cancel_confirm_visible && scan.phase.active() {
        let body = column![
            row![
                text("警告").size(17).font(fonts::MEDIUM),
                space::horizontal(),
                button(icons::icon(Icon::X, 15, INK_MUTED))
                    .on_press(VersionMessage::KeepScanning)
                    .style(button_style(ButtonVariant::Ghost))
            ]
            .align_y(Alignment::Center),
            text("确定要取消当前的扫描吗？已经扫描到的实例会保留。").size(13),
            row![
                button(text("确定"))
                    .on_press(VersionMessage::CancelScan)
                    .style(button_style(ButtonVariant::Primary)),
                button(text("取消"))
                    .on_press(VersionMessage::KeepScanning)
                    .style(button_style(ButtonVariant::Secondary)),
            ]
            .spacing(10),
        ]
        .spacing(14)
        .into();
        return Some(sized_modal(body, VersionMessage::KeepScanning, 380.0));
    }
    if !scan.visible {
        return None;
    }
    let indicator: Element<'_, VersionMessage> = if scan.phase.active() {
        astra_ui::ProgressCircle::new(0.0)
            .is_indeterminate(true)
            .size(astra_ui::ProgressCircleSize::Small)
            .animation_phase(f32::from(state.animation_frame) / 20.0)
            .into()
    } else {
        icons::icon(
            if scan.phase == ScanPhase::Completed {
                Icon::CircleCheck
            } else {
                Icon::Info
            },
            18,
            INK_MUTED,
        )
        .into()
    };
    let mut body = column![
        row![
            text("扫描本地实例").size(18).font(fonts::MEDIUM),
            space::horizontal(),
            button(icons::icon(Icon::X, 16, INK_MUTED))
                .on_press(VersionMessage::CloseScanLog)
                .style(button_style(ButtonVariant::Ghost))
        ]
        .align_y(Alignment::Center),
        row![indicator, text(scan.status_label()).size(13)]
            .spacing(8)
            .align_y(Alignment::Center),
        text("在用户主目录中查找酒馆实例。")
            .size(11)
            .color(INK_MUTED),
    ]
    .spacing(12);
    if !scan.recent_paths.is_empty() {
        let recent = column(scan.recent_paths.iter().map(|path| {
            iced::widget::tooltip(
                text(truncate_path(path, 60)).size(11).color(INK_MUTED),
                container(text(path).size(10))
                    .padding(8)
                    .max_width(420)
                    .style(super::install_modal_style),
                iced::widget::tooltip::Position::Bottom,
            )
            .into()
        }))
        .spacing(5);
        body = body.push(recent);
    }
    body = body.push(
        row![
            text("发现实例").size(11),
            text(scan.added + scan.duplicates).size(11),
            text("新增实例").size(11),
            text(scan.added).size(11),
            text("重复实例").size(11),
            text(scan.duplicates).size(11),
        ]
        .spacing(8),
    );
    if let Some(error) = &scan.error {
        body = body.push(error_view(Some(error)));
    }
    if scan.show_details {
        body = body.push(log_view(&scan.logs, 160.0));
    }
    let action = if scan.phase.active() {
        button(text("取消扫描"))
            .on_press(VersionMessage::RequestCancelScan)
            .style(button_style(ButtonVariant::Secondary))
    } else {
        button(text("重新扫描"))
            .on_press(VersionMessage::ScanLocal)
            .style(button_style(ButtonVariant::Primary))
    };
    body = body.push(
        row![
            button(text(if scan.show_details {
                "收起详细日志"
            } else {
                "查看详细日志"
            }))
            .on_press(VersionMessage::ToggleScanDetails)
            .style(button_style(ButtonVariant::Ghost)),
            space::horizontal(),
            action,
            button(text("关闭"))
                .on_press(VersionMessage::CloseScanLog)
                .style(button_style(ButtonVariant::Ghost)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    );
    Some(sized_modal(
        body.into(),
        VersionMessage::CloseScanLog,
        480.0,
    ))
}

pub fn toast_view(state: &LocalUiState) -> Option<Element<'_, VersionMessage>> {
    let toast = state.toast.as_ref()?;
    Some(
        container(astra_ui::toast(
            t(toast.message, current_language()),
            &toast.detail,
            if toast.danger {
                astra_ui::ToastVariant::Danger
            } else {
                astra_ui::ToastVariant::Success
            },
            None,
            VersionMessage::DismissLocalToast,
            VersionMessage::LocalModalInteract,
        ))
        .max_width(680)
        .padding(16)
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closing_progress_keeps_scanning_and_recent_paths_are_bounded() {
        let mut state = LocalUiState::default();
        state.scan.phase = ScanPhase::Running;
        state.scan.visible = true;
        for i in 0..8 {
            state.scan.observe_path(format!("/fixture/{i}"));
        }
        assert_eq!(state.scan.recent_paths.len(), 5);
        assert_eq!(state.scan.recent_paths.front().unwrap(), "/fixture/3");
        state.close_scan();
        assert_eq!(state.scan.phase, ScanPhase::Running);
        assert!(!state.scan.visible);
    }
    #[test]
    fn success_auto_hides_but_does_not_discard_logs() {
        let mut state = LocalUiState::default();
        state.scan.visible = true;
        state.scan.phase = ScanPhase::Completed;
        state.scan.auto_hide_at = Some(Instant::now() - Duration::from_secs(1));
        append_log(&mut state.scan.logs, "/fixture".into());
        state.tick();
        assert!(!state.scan.visible);
        assert_eq!(state.scan.logs.len(), 1);
    }
    #[test]
    fn path_labels_are_compact_without_breaking_unicode() {
        let path = format!("/用户/{}\n/end", "很长的目录".repeat(30));
        let label = truncate_path(&path, 60);
        assert!(label.chars().count() <= 60);
        assert!(label.starts_with("/用户/"));
        assert!(label.ends_with("/end"));
        assert!(!label.contains('\n'));
    }
    #[test]
    fn detailed_log_memory_is_bounded() {
        let mut logs = VecDeque::new();
        for _ in 0..800 {
            append_log(&mut logs, "line".into());
        }
        assert_eq!(logs.len(), 600);
    }
}
