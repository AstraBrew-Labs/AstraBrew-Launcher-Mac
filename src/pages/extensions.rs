//! 扩展管理页面。
//!
//! 复刻旧版启动器的页面信息结构：当前酒馆版本、扩展筛选、安装入口，
//! 以及每个扩展的启用、自动更新和目录操作。服务层尚未接入，页面先以
//! 本地状态承载交互反馈，后续扫描接口可直接替换默认数据来源。

use iced::widget::{button, column, container, row, scrollable, space, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{
    BLUE_600, ButtonVariant, DANGER, INK, INK_MUTED, INK_SUBTLE, SUCCESS, SURFACE_ALT, WARNING,
    WHITE, fonts, icons,
};

use super::versions::{VersionSource, VersionState};
use crate::lang::text;
use crate::theme::button_style;

const PURPLE: Color = Color::from_rgb8(142, 68, 220);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionScope {
    Global,
}

impl ExtensionScope {
    const fn label(self) -> &'static str {
        match self {
            Self::Global => "全局",
        }
    }

    const fn color(self) -> Color {
        match self {
            Self::Global => PURPLE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInfo {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub minimum_client_version: Option<String>,
    pub author: String,
    pub scope: ExtensionScope,
    pub enabled: bool,
    pub auto_update: Option<bool>,
    pub is_system: bool,
    pub has_homepage: bool,
}

impl ExtensionInfo {
    fn third_party(
        id: &str,
        display_name: &str,
        version: &str,
        minimum_client_version: Option<&str>,
        author: &str,
        auto_update: Option<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            version: version.into(),
            minimum_client_version: minimum_client_version.map(str::to_owned),
            author: author.into(),
            scope: ExtensionScope::Global,
            enabled: true,
            auto_update,
            is_system: false,
            has_homepage: true,
        }
    }

    fn system(id: &str, display_name: &str, version: &str, author: &str) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            version: version.into(),
            minimum_client_version: None,
            author: author.into(),
            scope: ExtensionScope::Global,
            enabled: true,
            auto_update: None,
            is_system: true,
            has_homepage: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExtensionsState {
    pub auto_repair_git: bool,
    pub show_system_extensions: bool,
    pub extensions: Vec<ExtensionInfo>,
    pub notice: Option<String>,
}

impl Default for ExtensionsState {
    fn default() -> Self {
        Self {
            auto_repair_git: true,
            show_system_extensions: false,
            extensions: vec![
                ExtensionInfo::third_party(
                    "st-input-helper",
                    "输入助手",
                    "1.3.1",
                    None,
                    "AI助手和Mooooooon",
                    None,
                ),
                ExtensionInfo::third_party(
                    "st-memory-enhancement",
                    "记忆增强表格",
                    "2.2.15",
                    None,
                    "muyoo",
                    None,
                ),
                ExtensionInfo::third_party(
                    "JS-Slash-Runner",
                    "酒馆助手",
                    "4.8.12",
                    Some("1.12.13"),
                    "KAKAA",
                    Some(true),
                ),
                ExtensionInfo::third_party(
                    "Extension-TopInfoBar",
                    "Chat Top Bar",
                    "1.0.0",
                    None,
                    "Cohee1207",
                    Some(true),
                ),
                ExtensionInfo::system("third-party", "第三方扩展加载器", "1.0.0", "SillyTavern"),
                ExtensionInfo::system("quick-reply", "快速回复", "3.1.0", "SillyTavern"),
            ],
            notice: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExtensionsMessage {
    ToggleAutoRepair(bool),
    ToggleShowSystem(bool),
    Refresh,
    Install,
    OpenExtensionRoot,
    OpenHomepage(String),
    OpenDirectory(String),
    ToggleEnabled(String, bool),
    ToggleAutoUpdate(String, bool),
    Delete(String),
}

impl ExtensionsState {
    pub fn update(&mut self, message: ExtensionsMessage) {
        match message {
            ExtensionsMessage::ToggleAutoRepair(enabled) => {
                self.auto_repair_git = enabled;
                self.notice = Some(
                    if enabled {
                        "已开启扩展 Git 自动修复。"
                    } else {
                        "已关闭扩展 Git 自动修复。"
                    }
                    .into(),
                );
            }
            ExtensionsMessage::ToggleShowSystem(enabled) => {
                self.show_system_extensions = enabled;
                self.notice = Some(
                    if enabled {
                        "已显示系统扩展。"
                    } else {
                        "已隐藏系统扩展。"
                    }
                    .into(),
                );
            }
            ExtensionsMessage::Refresh => {
                self.notice = Some(format!(
                    "扫描完成，共发现 {} 个扩展。",
                    self.visible_count()
                ));
            }
            ExtensionsMessage::Install => {
                self.notice = Some("安装扩展入口已准备，等待安装服务接入。".into());
            }
            ExtensionsMessage::OpenExtensionRoot => {
                self.notice = Some("扩展根目录入口已准备，等待系统目录服务接入。".into());
            }
            ExtensionsMessage::OpenHomepage(id) => {
                self.notice = Some(format!("{id} 的主页入口已准备。"));
            }
            ExtensionsMessage::OpenDirectory(id) => {
                self.notice = Some(format!("{id} 的目录入口已准备。"));
            }
            ExtensionsMessage::ToggleEnabled(id, enabled) => {
                if let Some(extension) = self
                    .extensions
                    .iter_mut()
                    .find(|extension| extension.id == id && !extension.is_system)
                {
                    extension.enabled = enabled;
                    self.notice = Some(format!(
                        "{}已{}。",
                        extension.display_name,
                        if enabled { "启用" } else { "停用" }
                    ));
                }
            }
            ExtensionsMessage::ToggleAutoUpdate(id, enabled) => {
                if let Some(extension) = self
                    .extensions
                    .iter_mut()
                    .find(|extension| extension.id == id)
                    && extension.auto_update.is_some()
                {
                    extension.auto_update = Some(enabled);
                    self.notice = Some(format!(
                        "{}的自动更新已{}。",
                        extension.display_name,
                        if enabled { "开启" } else { "关闭" }
                    ));
                }
            }
            ExtensionsMessage::Delete(id) => {
                let removed = self
                    .extensions
                    .iter()
                    .find(|extension| extension.id == id && !extension.is_system)
                    .map(|extension| extension.display_name.clone());
                if let Some(name) = removed {
                    self.extensions.retain(|extension| extension.id != id);
                    self.notice = Some(format!("已删除扩展 {name}。"));
                }
            }
        }
    }

    fn visible_count(&self) -> usize {
        self.extensions
            .iter()
            .filter(|extension| self.show_system_extensions || !extension.is_system)
            .count()
    }
}

pub fn extensions_view<'a>(
    state: &'a ExtensionsState,
    versions: &'a VersionState,
) -> Element<'a, ExtensionsMessage> {
    let header = row![
        column![
            text("扩展管理")
                .size(24)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
            text("管理酒馆已安装的第三方扩展")
                .size(12)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(4),
        space::horizontal(),
        container(
            row![
                text("自动修复")
                    .size(11)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
                compact_switch(
                    state.auto_repair_git,
                    BLUE_600,
                    ExtensionsMessage::ToggleAutoRepair(!state.auto_repair_git),
                ),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        )
        .height(36)
        .padding([0, 12])
        .align_y(Alignment::Center)
        .style(control_surface),
        action_button(
            "安装扩展",
            Icon::Download,
            ExtensionsMessage::Install,
            ButtonVariant::Primary,
        ),
        action_button(
            "打开扩展文件夹",
            Icon::FolderOpen,
            ExtensionsMessage::OpenExtensionRoot,
            ButtonVariant::Secondary,
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Fill);

    let version = selected_version_card(versions);
    let list = extensions_panel(state);

    container(
        container(
            column![header, version, list]
                .spacing(16)
                .width(Fill)
                .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .max_width(980),
    )
    .width(Fill)
    .height(Fill)
    .padding([26, 30])
    .align_x(Alignment::Center)
    .style(crate::theme::canvas_style)
    .into()
}

fn selected_version_card(versions: &VersionState) -> Element<'_, ExtensionsMessage> {
    let version = versions
        .current_version
        .as_deref()
        .unwrap_or(versions.latest_version.as_str());
    let path = versions
        .current_path
        .as_deref()
        .or_else(|| {
            versions
                .local_instances
                .iter()
                .find(|instance| {
                    instance.dependencies == crate::pages::versions::DependencyStatus::Ready
                })
                .map(|instance| instance.path.as_str())
        })
        .unwrap_or("AstraBrew Launcher 管理的在线实例");
    let source = match versions.current_source {
        Some(VersionSource::Online) => "在线下载",
        _ => "本地导入",
    };

    container(
        row![
            container(icons::icon(Icon::Puzzle, 19, BLUE_600))
                .width(40)
                .height(40)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(blue_icon_surface),
            column![
                text("当前选择的酒馆版本")
                    .size(14)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
                row![
                    text(format!("当前版本：{version}"))
                        .size(10)
                        .font(fonts::REGULAR)
                        .style(crate::theme::muted_text_style),
                    badge(source, WARNING),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    crate::theme::subtle_icon(Icon::Folder, 12),
                    text(path)
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::subtle_text_style),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            ]
            .spacing(4),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(82)
    .padding([12, 16])
    .style(panel_surface)
    .into()
}

fn extensions_panel(state: &ExtensionsState) -> Element<'_, ExtensionsMessage> {
    let visible: Vec<&ExtensionInfo> = state
        .extensions
        .iter()
        .filter(|extension| state.show_system_extensions || !extension.is_system)
        .collect();

    let header = container(
        row![
            crate::theme::muted_icon(Icon::Puzzle, 18),
            text("已安装扩展")
                .size(15)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
            container(
                text(format!("{} 项", visible.len()))
                    .size(9)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
            )
            .padding([4, 8])
            .style(meta_surface),
            space::horizontal(),
            state
                .notice
                .as_deref()
                .map(notice_badge)
                .unwrap_or_else(|| space::horizontal().width(Length::Shrink).into()),
            text("显示系统扩展")
                .size(11)
                .font(fonts::MEDIUM)
                .style(crate::theme::muted_text_style),
            compact_switch(
                state.show_system_extensions,
                BLUE_600,
                ExtensionsMessage::ToggleShowSystem(!state.show_system_extensions),
            ),
            icon_action(
                Icon::RefreshCw,
                "重新扫描扩展",
                ExtensionsMessage::Refresh,
                INK_MUTED,
            ),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(58)
    .padding([0, 20])
    .align_y(Alignment::Center);

    let list: Element<'_, ExtensionsMessage> = if visible.is_empty() {
        container(
            column![
                crate::theme::subtle_icon(Icon::Puzzle, 34),
                text("没有找到扩展")
                    .size(13)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
                text("安装扩展后，它们会显示在这里。")
                    .size(10)
                    .font(fonts::REGULAR)
                    .style(crate::theme::subtle_text_style),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
        let count = visible.len();
        let rows = visible.into_iter().enumerate().fold(
            column![].width(Fill),
            |rows, (index, extension)| {
                let rows = rows.push(extension_row(extension));
                if index + 1 < count {
                    rows.push(separator_line())
                } else {
                    rows
                }
            },
        );
        scrollable(rows).height(Fill).into()
    };

    container(
        column![header, separator_line(), list]
            .height(Fill)
            .width(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(panel_surface)
    .into()
}

fn extension_row(extension: &ExtensionInfo) -> Element<'_, ExtensionsMessage> {
    let content_color = if extension.enabled { INK } else { INK_SUBTLE };
    let id = extension.id.clone();

    let mut title = row![
        text(&extension.display_name)
            .size(15)
            .font(fonts::MEDIUM)
            .color(content_color),
        owned_badge(format!("v{}", extension.version), INK_MUTED),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(minimum) = extension.minimum_client_version.as_deref() {
        title = title.push(owned_badge(format!("ST ≥ {minimum}"), BLUE_600));
    }
    title = title.push(badge(extension.scope.label(), extension.scope.color()));
    if extension.is_system {
        title = title.push(icon_badge("系统", Icon::ShieldCheck, WARNING));
    }
    if !extension.enabled {
        title = title.push(badge("已停用", INK_SUBTLE));
    }

    let mut meta = row![
        crate::theme::subtle_icon(Icon::User, 12),
        text(&extension.author)
            .size(10)
            .font(fonts::REGULAR)
            .style(crate::theme::muted_text_style),
        text("|").size(10).style(crate::theme::subtle_text_style),
        crate::theme::subtle_icon(Icon::Folder, 12),
        text(&extension.id)
            .size(10)
            .font(fonts::REGULAR)
            .style(crate::theme::muted_text_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if extension.has_homepage {
        meta = meta.push(small_action(
            "访问主页",
            Icon::Globe,
            ExtensionsMessage::OpenHomepage(id.clone()),
        ));
    }
    meta = meta.push(small_action(
        "打开目录",
        Icon::FolderOpen,
        ExtensionsMessage::OpenDirectory(id.clone()),
    ));

    let primary_controls: Element<'_, ExtensionsMessage> = if extension.is_system {
        badge("随酒馆启用", WARNING)
    } else {
        row![
            text(if extension.enabled {
                "已启用"
            } else {
                "已停用"
            })
            .size(11)
            .font(fonts::MEDIUM)
            .color(if extension.enabled {
                INK_MUTED
            } else {
                INK_SUBTLE
            }),
            compact_switch(
                extension.enabled,
                BLUE_600,
                ExtensionsMessage::ToggleEnabled(id.clone(), !extension.enabled),
            ),
            icon_action(
                Icon::Trash2,
                "删除扩展",
                ExtensionsMessage::Delete(id.clone()),
                DANGER,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };

    let secondary_controls: Element<'_, ExtensionsMessage> = extension
        .auto_update
        .map(|enabled| {
            row![
                text("自动更新")
                    .size(11)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
                compact_switch(
                    enabled,
                    SUCCESS,
                    ExtensionsMessage::ToggleAutoUpdate(id, !enabled),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        })
        .unwrap_or_else(|| space::vertical().height(0).into());

    container(
        row![
            column![title, meta].spacing(8).width(Fill),
            column![primary_controls, secondary_controls]
                .spacing(8)
                .align_x(Alignment::End),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(92)
    .padding([14, 20])
    .into()
}

fn action_button(
    label: &'static str,
    icon: Icon,
    message: ExtensionsMessage,
    variant: ButtonVariant,
) -> Element<'static, ExtensionsMessage> {
    let color = if variant == ButtonVariant::Primary {
        WHITE
    } else {
        INK_MUTED
    };
    button(
        row![
            icons::icon(icon, 15, color),
            text(label).size(11).font(fonts::MEDIUM).color(color),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .on_press(message)
    .height(36)
    .padding([8, 13])
    .style(button_style(variant))
    .into()
}

fn small_action(
    label: &'static str,
    icon: Icon,
    message: ExtensionsMessage,
) -> Element<'static, ExtensionsMessage> {
    button(
        row![
            crate::theme::muted_icon(icon, 11),
            text(label)
                .size(9)
                .font(fonts::MEDIUM)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(message)
    .height(26)
    .padding([4, 8])
    .style(small_action_style)
    .into()
}

fn icon_action(
    icon: Icon,
    label: &'static str,
    message: ExtensionsMessage,
    color: Color,
) -> Element<'static, ExtensionsMessage> {
    let action = button(
        container(icons::icon(icon, 15, color))
            .width(26)
            .height(26)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press(message)
    .padding(0)
    .style(button_style(ButtonVariant::Ghost));

    tooltip(
        action,
        container(text(label).size(10).font(fonts::REGULAR).color(WHITE))
            .padding([6, 9])
            .style(tooltip_surface),
        tooltip::Position::Bottom,
    )
    .gap(5)
    .delay(iced::time::Duration::from_millis(350))
    .into()
}

fn compact_switch(
    enabled: bool,
    accent: Color,
    message: ExtensionsMessage,
) -> Element<'static, ExtensionsMessage> {
    let (leading, trailing) = if enabled { (18.0, 2.0) } else { (2.0, 18.0) };
    let track = if enabled { accent } else { SURFACE_ALT };

    button(
        container(
            row![
                space::horizontal().width(leading),
                container(space::horizontal())
                    .width(16)
                    .height(16)
                    .style(switch_thumb),
                space::horizontal().width(trailing),
            ]
            .align_y(Alignment::Center),
        )
        .width(36)
        .height(20)
        .align_y(Alignment::Center)
        .style(move |_theme| switch_track(track)),
    )
    .on_press(message)
    .padding(0)
    .height(20)
    .style(switch_button_style)
    .into()
}

fn badge<'a>(label: &'a str, color: Color) -> Element<'a, ExtensionsMessage> {
    container(text(label).size(8).font(fonts::MEDIUM).color(color))
        .padding([4, 7])
        .style(move |_theme| badge_surface(color))
        .into()
}

fn owned_badge(label: String, color: Color) -> Element<'static, ExtensionsMessage> {
    container(text(label).size(8).font(fonts::MEDIUM).color(color))
        .padding([4, 7])
        .style(move |_theme| badge_surface(color))
        .into()
}

fn icon_badge(
    label: &'static str,
    icon: Icon,
    color: Color,
) -> Element<'static, ExtensionsMessage> {
    container(
        row![
            icons::icon(icon, 10, color),
            text(label).size(8).font(fonts::MEDIUM).color(color),
        ]
        .spacing(3)
        .align_y(Alignment::Center),
    )
    .padding([4, 7])
    .style(move |_theme| badge_surface(color))
    .into()
}

fn notice_badge(notice: &str) -> Element<'_, ExtensionsMessage> {
    container(
        row![
            icons::icon(Icon::Info, 11, BLUE_600),
            text(notice)
                .size(9)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([5, 8])
    .style(notice_surface)
    .into()
}

fn separator_line<'a>() -> Element<'a, ExtensionsMessage> {
    container(space::vertical())
        .width(Fill)
        .height(1)
        .style(separator_surface)
        .into()
}

fn panel_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

fn control_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

fn blue_icon_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) {
                0.18
            } else {
                0.10
            },
        ))),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn badge_surface(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.10,
        ))),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn meta_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    }
}

fn notice_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) {
                0.14
            } else {
                0.08
            },
        ))),
        border: Border {
            color: Color::from_rgba(
                theme.palette().primary.r,
                theme.palette().primary.g,
                theme.palette().primary.b,
                if crate::theme::is_dark(theme) {
                    0.38
                } else {
                    0.18
                },
            ),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    }
}

fn small_action_style(theme: &Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        crate::theme::surface_alt(theme)
    } else {
        crate::theme::surface(theme)
    };
    button::Style {
        background: Some(Background::Color(background)),
        border: Border {
            radius: 7.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tooltip_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(38, 38, 42))),
        border: Border {
            radius: 7.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn separator_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::line(theme))),
        ..container::Style::default()
    }
}

fn switch_track(color: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            radius: 10.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn switch_thumb(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(WHITE)),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn switch_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style::default()
}

#[cfg(test)]
mod tests {
    use super::{ExtensionsMessage, ExtensionsState};

    #[test]
    fn system_extensions_are_hidden_by_default() {
        let state = ExtensionsState::default();
        assert_eq!(state.visible_count(), 4);
    }

    #[test]
    fn enabling_system_extensions_updates_the_filter() {
        let mut state = ExtensionsState::default();
        state.update(ExtensionsMessage::ToggleShowSystem(true));
        assert_eq!(state.visible_count(), 6);
    }

    #[test]
    fn extension_can_be_disabled_and_deleted() {
        let mut state = ExtensionsState::default();
        state.update(ExtensionsMessage::ToggleEnabled(
            "st-input-helper".into(),
            false,
        ));
        assert!(
            state
                .extensions
                .iter()
                .find(|extension| extension.id == "st-input-helper")
                .is_some_and(|extension| !extension.enabled)
        );
        state.update(ExtensionsMessage::Delete("st-input-helper".into()));
        assert!(
            state
                .extensions
                .iter()
                .all(|extension| extension.id != "st-input-helper")
        );
    }
}
