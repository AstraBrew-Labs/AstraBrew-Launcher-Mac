//! 配置同步的展示状态与遮罩；沿用当前 Astra UI，不引入旧版页面布局。

use super::{TavernAction, TavernMessage};
use crate::core::tavern_config::{ConfigError, schema};
use crate::lang::{raw, text};
use crate::theme::button_style;
use astra_ui::{BLUE_600, ButtonVariant, DANGER, INK_MUTED, SUCCESS, WARNING};
use iced::widget::{button, column, container, mouse_area, row, scrollable, space, stack};
use iced::{Alignment, Color, Element, Fill};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    NoTarget,
    Loading,
    Ready,
    Pending,
    Saving,
    Saved,
    Validation,
    SaveFailed,
    Conflict,
    Missing,
    Invalid,
    ReadFailed,
    Generating,
    Importing,
}
impl Status {
    pub fn label_key(self) -> &'static str {
        match self {
            Self::NoTarget => "tavern.sync.status.no_target",
            Self::Loading => "tavern.sync.status.loading",
            Self::Ready => "tavern.sync.status.ready",
            Self::Pending => "tavern.sync.status.pending",
            Self::Saving => "tavern.sync.status.saving",
            Self::Saved => "tavern.sync.status.saved",
            Self::Validation => "tavern.sync.status.validation",
            Self::SaveFailed => "tavern.sync.status.save_failed",
            Self::Conflict => "tavern.sync.status.conflict",
            Self::Missing => "tavern.sync.status.missing",
            Self::Invalid => "tavern.sync.status.invalid",
            Self::ReadFailed => "tavern.sync.status.read_failed",
            Self::Generating => "tavern.sync.status.generating",
            Self::Importing => "tavern.sync.status.importing",
        }
    }
    pub fn color(self) -> Color {
        match self {
            Self::Ready | Self::Saved => SUCCESS,
            Self::SaveFailed | Self::Invalid | Self::ReadFailed => DANGER,
            Self::Validation | Self::Conflict | Self::Missing => WARNING,
            _ => BLUE_600,
        }
    }
}
#[derive(Debug, Clone)]
pub struct ImportPrompt {
    pub source: PathBuf,
    pub target: PathBuf,
    pub changed: bool,
}
#[derive(Debug, Clone, Default)]
pub struct SyncView {
    pub status: Status,
    pub target: Option<PathBuf>,
    pub error: Option<ConfigError>,
    pub import: Option<ImportPrompt>,
    pub conflicts: Vec<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub close_prompt: bool,
    pub pending_targets: Vec<PathBuf>,
    /// 当前服务模式下由系统托管、在界面中只读的地址。
    pub fixed_whitelist: Vec<String>,
}

pub fn validated_input<'a>(
    input: Element<'a, TavernMessage>,
    key: &str,
    value: &serde_json::Value,
) -> Element<'a, TavernMessage> {
    let error = schema::field(key).and_then(|field| schema::encode(field, value).err());
    if let Some(error) = error {
        column![input, text(error).size(12).color(DANGER)]
            .spacing(4)
            .into()
    } else {
        input
    }
}

fn alert<'a>(content: Element<'a, TavernMessage>) -> Element<'a, TavernMessage> {
    stack![
        button(space::Space::new())
            .on_press(TavernMessage::ConfigOverlayInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(|_, _| button::Style {
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.58
                ))),
                ..button::Style::default()
            }),
        container(
            mouse_area(
                container(content)
                    .width(Fill).max_width(510)
                    .padding(24)
                    .style(super::config_card_style)
            )
            .on_press(TavernMessage::ConfigOverlayInteract)
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .padding(20),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}
fn action(
    label: &'static str,
    message: TavernMessage,
) -> iced::widget::Button<'static, TavernMessage> {
    button(text(label).size(14))
        .padding([9, 14])
        .on_press(message)
        .style(button_style(ButtonVariant::Primary))
}

pub fn with_overlay<'a>(
    base: Element<'a, TavernMessage>,
    view: &'a SyncView,
) -> Element<'a, TavernMessage> {
    let mut body = column![].spacing(14);
    if let Some(import) = &view.import {
        body = body.push(text("tavern.action.import_config_file").size(20).font(crate::core::typography::medium()));
        if import.changed {
            body = body.push(
                text("tavern.sync.import.changed")
                    .size(14)
                    .color(WARNING),
            );
        }
        body = body
            .push(text("tavern.sync.import.description").size(15));
        body = body.push(text("tavern.sync.import.source").size(13).color(INK_MUTED));
        body = body.push(scrollable(raw(import.source.display().to_string()).size(13)).height(36));
        body = body.push(text("tavern.sync.import.target").size(13).color(INK_MUTED));
        body = body.push(scrollable(raw(import.target.display().to_string()).size(13)).height(36));
        body = body.push(
            text("tavern.sync.import.note")
                .size(13)
                .color(INK_MUTED),
        );
        body = body.push(
            row![
                action("tavern.sync.import.confirm", TavernMessage::ConfirmImport),
                action("tavern.sync.import.cancel", TavernMessage::CancelImport)
                    .style(button_style(ButtonVariant::Secondary))
            ]
            .spacing(10),
        );
    } else if !view.conflicts.is_empty() {
        body = body.push(text("tavern.sync.status.conflict").size(20).font(crate::core::typography::medium()));
        body = body.push(text("tavern.sync.conflict.description").size(14));
        body = body.push(
            scrollable(
                column(view.conflicts.iter().map(|key| {
                    raw(schema::field(key).map(|f| f.path).unwrap_or(key))
                        .size(14)
                        .into()
                }))
                .spacing(5),
            )
            .height(100),
        );
        body = body.push(
            row![
                action("tavern.sync.conflict.use_disk", TavernMessage::UseDiskValues),
                action("tavern.sync.conflict.keep_draft", TavernMessage::KeepDraftValues)
            ]
            .spacing(10),
        );
    } else if matches!(
        view.status,
        Status::NoTarget
            | Status::Loading
            | Status::Missing
            | Status::Invalid
            | Status::ReadFailed
            | Status::Generating
            | Status::Importing
    ) {
        body = body.push(text(view.status.label_key()).size(20).font(crate::core::typography::medium()));
        if let Some(path) = &view.target {
            body = body.push(scrollable(raw(path.display().to_string()).size(13).color(INK_MUTED)).height(44));
        }
        match view.status {
            Status::NoTarget => {
                body = body.push(text("tavern.sync.no_target.hint").size(14));
                body = body.push(action("tavern.sync.go_to_versions", TavernMessage::GoToVersions));
            }
            Status::Missing => {
                body = body.push(
                    text("tavern.sync.missing.hint").size(14),
                );
                body = body.push(action("tavern.sync.generate", TavernMessage::GenerateConfig));
            }
            Status::Invalid | Status::ReadFailed => {
                body = body.push(text("tavern.sync.invalid.hint").size(14));
                body = body.push(
                    row![
                        action("tavern.sync.retry", TavernMessage::RetryConfig),
                        action(
                            "tavern.action.open_config_file",
                            TavernMessage::Action(TavernAction::OpenConfigFile)
                        )
                    ]
                    .spacing(8),
                );
            }
            Status::Generating | Status::Importing => {
                let progress = view
                    .total
                    .filter(|total| *total > 0)
                    .map(|total| view.downloaded as f32 * 100.0 / total as f32);
                body = body.push(
                    astra_ui::ProgressBar::new(progress.unwrap_or(0.0))
                        .is_indeterminate(progress.is_none())
                        .show_value(progress.is_some()),
                );
                body = body
                    .push(text("tavern.sync.processing.hint").size(14));
            }
            _ => {}
        }
    } else {
        return base;
    }
    if let Some(error) = &view.error {
        body = body.push(text(error.message).size(14).color(DANGER));
        body = body.push(scrollable(raw(&error.detail).size(13)).height(45));
    }
    stack![base, alert(body.into())]
        .width(Fill)
        .height(Fill)
        .into()
}

pub fn close_overlay(view: &SyncView) -> Element<'_, TavernMessage> {
    alert(
        column![
            text("tavern.sync.close.title").size(20).font(crate::core::typography::medium()),
            text("tavern.sync.close.description")
                .size(15),
            scrollable(
                column(
                    view.pending_targets
                        .iter()
                        .map(|path| raw(path.display().to_string()).size(13).into())
                )
                .spacing(5)
            )
            .height(60),
            row![
                action("tavern.sync.close.continue_editing", TavernMessage::ContinueEditing),
                action("tavern.sync.close.discard", TavernMessage::DiscardAndClose)
                    .style(button_style(ButtonVariant::DangerSoft))
            ]
            .spacing(10),
        ]
        .spacing(16)
        .into(),
    )
}
