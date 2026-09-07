//! 配置同步的展示状态与遮罩；沿用当前 Astra UI，不引入旧版页面布局。

use super::{TavernAction, TavernMessage};
use crate::core::tavern_config::{ConfigError, schema};
use crate::lang::text;
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
    pub fn label(self) -> &'static str {
        match self {
            Self::NoTarget => "尚未选择实例",
            Self::Loading => "正在加载配置…",
            Self::Ready => "就绪",
            Self::Pending => "待保存",
            Self::Saving => "正在保存…",
            Self::Saved => "已保存",
            Self::Validation => "请检查输入",
            Self::SaveFailed => "保存失败",
            Self::Conflict => "配置存在冲突",
            Self::Missing => "配置文件不存在",
            Self::Invalid => "配置文件无效",
            Self::ReadFailed => "配置读取失败",
            Self::Generating => "正在生成配置…",
            Self::Importing => "正在准备导入…",
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
        body = body.push(text("导入配置文件").size(20).font(crate::core::typography::medium()));
        if import.changed {
            body = body.push(
                text("源文件或目标文件已变化，请再次确认。")
                    .size(14)
                    .color(WARNING),
            );
        }
        body = body
            .push(text("是否覆盖已有配置项？导入字段优先，缺失字段保留并由模板补全。").size(15));
        body = body.push(text("导入源文件").size(13).color(INK_MUTED));
        body = body.push(scrollable(text(import.source.display()).size(13)).height(36));
        body = body.push(text("目标配置文件").size(13).color(INK_MUTED));
        body = body.push(scrollable(text(import.target.display()).size(13)).height(36));
        body = body.push(
            text("确认后会先备份原配置，列表字段整体替换。")
                .size(13)
                .color(INK_MUTED),
        );
        body = body.push(
            row![
                action("确认覆盖并导入", TavernMessage::ConfirmImport),
                action("取消", TavernMessage::CancelImport)
                    .style(button_style(ButtonVariant::Secondary))
            ]
            .spacing(10),
        );
    } else if !view.conflicts.is_empty() {
        body = body.push(text("配置存在冲突").size(20).font(crate::core::typography::medium()));
        body = body.push(text("以下字段同时在界面和文件中修改，尚未覆盖任何一方。").size(14));
        body = body.push(
            scrollable(
                column(view.conflicts.iter().map(|key| {
                    text(schema::field(key).map(|f| f.path).unwrap_or(key))
                        .size(14)
                        .into()
                }))
                .spacing(5),
            )
            .height(100),
        );
        body = body.push(
            row![
                action("采用文件内容", TavernMessage::UseDiskValues),
                action("保留我的修改", TavernMessage::KeepDraftValues)
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
        body = body.push(text(view.status.label()).size(20).font(crate::core::typography::medium()));
        if let Some(path) = &view.target {
            body = body.push(scrollable(text(path.display()).size(13).color(INK_MUTED)).height(44));
        }
        match view.status {
            Status::NoTarget => {
                body = body.push(text("请先在版本管理中选择一个酒馆实例。").size(14));
                body = body.push(action("前往版本管理", TavernMessage::GoToVersions));
            }
            Status::Missing => {
                body = body.push(
                    text("目标配置文件不存在，请点击立即生成。生成前不会写入页面默认值。").size(14),
                );
                body = body.push(action("立即生成", TavernMessage::GenerateConfig));
            }
            Status::Invalid | Status::ReadFailed => {
                body = body.push(text("原文件不会被覆盖，请修复后重新加载。").size(14));
                body = body.push(
                    row![
                        action("重新加载", TavernMessage::RetryConfig),
                        action(
                            "打开配置文件",
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
                    .push(text("正在处理当前目标，请稍候。不会自动修改网络访问设置。").size(14));
            }
            _ => {}
        }
    } else {
        return base;
    }
    if let Some(error) = &view.error {
        body = body.push(text(error.message).size(14).color(DANGER));
        body = body.push(scrollable(text(&error.detail).size(13)).height(45));
    }
    stack![base, alert(body.into())]
        .width(Fill)
        .height(Fill)
        .into()
}

pub fn close_overlay(view: &SyncView) -> Element<'_, TavernMessage> {
    alert(
        column![
            text("仍有未保存的配置").size(20).font(crate::core::typography::medium()),
            text("存在无效输入、文件冲突或保存失败。继续编辑，或放弃尚未保存的修改并退出？")
                .size(15),
            scrollable(
                column(
                    view.pending_targets
                        .iter()
                        .map(|path| text(path.display()).size(13).into())
                )
                .spacing(5)
            )
            .height(60),
            row![
                action("继续编辑", TavernMessage::ContinueEditing),
                action("放弃并退出", TavernMessage::DiscardAndClose)
                    .style(button_style(ButtonVariant::DangerSoft))
            ]
            .spacing(10),
        ]
        .spacing(16)
        .into(),
    )
}
