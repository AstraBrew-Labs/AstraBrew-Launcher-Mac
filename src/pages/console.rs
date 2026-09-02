//! SillyTavern 控制台页面。
//!
//! 复刻旧版控制台的三段式布局：顶部状态与操作栏、日志流以及局域网/公网
//! 连接信息。当前项目的进程服务尚未接入，因此交互先由本地状态承载；消息
//! 边界保持独立，接入真实服务时无需改动视图结构。

use iced::widget::{button, column, container, row, scrollable, space, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Fill, Font, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{
    BLUE_600, ButtonVariant, DANGER, INK, INK_MUTED, INK_SUBTLE, LINE, SUCCESS, SURFACE,
    SURFACE_ALT, button_style, fonts, icons,
};

use crate::app::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleStatus {
    NotStarted,
    Starting,
    Running,
    Stopped,
    Failed,
}

impl ConsoleStatus {
    fn label(self) -> &'static str {
        match self {
            Self::NotStarted => "未启动",
            Self::Starting => "启动中",
            Self::Running => "运行中",
            Self::Stopped => "已停止",
            Self::Failed => "启动失败",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::NotStarted => INK_MUTED,
            Self::Starting => BLUE_600,
            Self::Running => SUCCESS,
            Self::Stopped => WARNING,
            Self::Failed => DANGER,
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::NotStarted | Self::Stopped => Icon::Square,
            Self::Starting => Icon::Loader,
            Self::Running => Icon::CircleCheck,
            Self::Failed => Icon::CircleX,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Info,
    Success,
    Error,
    Output,
    System,
}

impl LogKind {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Error => "error",
            Self::Output => "output",
            Self::System => "system",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Info => BLUE_600,
            Self::Success => SUCCESS,
            Self::Error => DANGER,
            Self::Output => INK_MUTED,
            Self::System => Color::from_rgb8(142, 68, 220),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsoleLog {
    pub time: String,
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkMode {
    Lan,
    Internet,
}

impl NetworkMode {
    fn label(self) -> &'static str {
        match self {
            Self::Lan => "局域网连接",
            Self::Internet => "公网连接",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Lan => SUCCESS,
            Self::Internet => DANGER,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsoleState {
    pub status: ConsoleStatus,
    pub logs: Vec<ConsoleLog>,
    pub auto_scroll: bool,
    pub show_network_dialog: bool,
    pub network_mode: Option<NetworkMode>,
    pub network_port: u16,
    pub server_url: Option<String>,
    pub process_pid: Option<u32>,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self {
            status: ConsoleStatus::NotStarted,
            logs: Vec::new(),
            auto_scroll: true,
            show_network_dialog: false,
            network_mode: None,
            network_port: 8000,
            server_url: None,
            process_pid: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConsoleMessage {
    Start,
    Stop,
    ClearLogs,
    FollowLogs,
    ToggleNetworkDialog,
    OpenServer,
}

impl ConsoleState {
    pub fn update(&mut self, message: ConsoleMessage) {
        match message {
            ConsoleMessage::Start => self.start(),
            ConsoleMessage::Stop => self.stop(),
            ConsoleMessage::ClearLogs => self.logs.clear(),
            ConsoleMessage::FollowLogs => self.auto_scroll = true,
            ConsoleMessage::ToggleNetworkDialog => {
                self.show_network_dialog = !self.show_network_dialog
            }
            ConsoleMessage::OpenServer => {
                if let Some(url) = self.server_url.as_deref() {
                    let result = if cfg!(target_os = "macos") {
                        std::process::Command::new("open").arg(url).spawn()
                    } else if cfg!(target_os = "windows") {
                        std::process::Command::new("cmd")
                            .args(["/C", "start", url])
                            .spawn()
                    } else {
                        std::process::Command::new("xdg-open").arg(url).spawn()
                    };
                    if result.is_ok() {
                        self.push(LogKind::Info, "已在默认浏览器中打开酒馆。")
                    } else {
                        self.push(LogKind::Error, "无法打开默认浏览器，请手动访问服务地址。")
                    }
                }
            }
        }
    }

    fn start(&mut self) {
        if matches!(
            self.status,
            ConsoleStatus::Starting | ConsoleStatus::Running
        ) {
            return;
        }
        self.status = ConsoleStatus::Starting;
        self.process_pid = Some(48217);
        self.server_url = Some("http://127.0.0.1:8000".into());
        self.push(LogKind::System, "正在准备 SillyTavern 运行环境…");
        self.push(LogKind::Info, "Node.js 22.14.0 · 端口 8000");
        self.push(
            LogKind::Output,
            "SillyTavern listening on http://127.0.0.1:8000",
        );
        self.status = ConsoleStatus::Running;
        self.push(LogKind::Success, "SillyTavern 已启动，等待连接。 ");
    }

    fn stop(&mut self) {
        if matches!(
            self.status,
            ConsoleStatus::NotStarted | ConsoleStatus::Stopped
        ) {
            return;
        }
        self.status = ConsoleStatus::Stopped;
        self.push(LogKind::Info, "正在停止 SillyTavern…");
        self.push(LogKind::Success, "服务已停止。");
        self.process_pid = None;
        self.network_mode = None;
        self.show_network_dialog = false;
    }

    fn push(&mut self, kind: LogKind, text: &str) {
        let seconds = 8 * 3600 + 42 * 60 + self.logs.len() as u32 + 1;
        let time = format!(
            "{:02}:{:02}:{:02}",
            (seconds / 3600) % 24,
            (seconds / 60) % 60,
            seconds % 60
        );
        self.logs.push(ConsoleLog {
            time,
            kind,
            text: text.into(),
        });
    }
}

pub fn console_view(state: &ConsoleState) -> Element<'_, Message> {
    let header = header_view(state);
    let logs = logs_view(state);
    let body = column![header, logs].spacing(0).height(Fill).width(Fill);

    if state.show_network_dialog {
        stack![body, network_dialog(state)].into()
    } else {
        body.into()
    }
}

fn header_view(state: &ConsoleState) -> Element<'_, Message> {
    let status = status_badge(state.status);
    let mut left = row![
        icons::icon(Icon::SquareTerminal, 20, INK_MUTED),
        text("服务控制台").size(15).font(fonts::MEDIUM),
        status
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    if let Some(pid) = state.process_pid {
        left = left.push(separator()).push(
            text(format!("PID: {pid}"))
                .size(11)
                .font(Font::MONOSPACE)
                .color(INK_MUTED),
        );
    }

    if state.status == ConsoleStatus::Running {
        if let Some(mode) = state.network_mode {
            left = left.push(separator()).push(
                button(
                    text(mode.label())
                        .size(11)
                        .font(fonts::MEDIUM)
                        .color(mode.color()),
                )
                .padding([6, 10])
                .style(soft_button(mode.color()))
                .on_press(Message::Console(ConsoleMessage::ToggleNetworkDialog)),
            );
        } else if state.server_url.is_some() {
            left = left.push(separator()).push(
                button(
                    row![
                        text("访问酒馆").size(11).font(fonts::MEDIUM).color(SUCCESS),
                        icons::icon(Icon::ExternalLink, 13, SUCCESS)
                    ]
                    .spacing(5),
                )
                .padding([6, 10])
                .style(soft_button(SUCCESS))
                .on_press(Message::Console(ConsoleMessage::OpenServer)),
            );
        }
    }

    let clear = button(icons::icon(Icon::Trash2, 16, INK_MUTED))
        .padding(8)
        .style(icon_button_style())
        .on_press(Message::Console(ConsoleMessage::ClearLogs));
    let follow = (!state.auto_scroll).then(|| {
        button(
            row![
                icons::icon(Icon::ArrowDownToLine, 14, BLUE_600),
                text("跟随日志")
                    .size(11)
                    .font(fonts::MEDIUM)
                    .color(BLUE_600)
            ]
            .spacing(6),
        )
        .padding([6, 10])
        .style(soft_button(BLUE_600))
        .on_press(Message::Console(ConsoleMessage::FollowLogs))
    });
    let stop = button(
        row![
            icons::icon(Icon::Square, 13, DANGER),
            text("停止").size(11).font(fonts::MEDIUM).color(DANGER)
        ]
        .spacing(6),
    )
    .padding([6, 10])
    .style(soft_button(DANGER))
    .on_press_maybe(
        (state.status != ConsoleStatus::NotStarted
            && state.status != ConsoleStatus::Stopped
            && state.status != ConsoleStatus::Failed)
            .then_some(Message::Console(ConsoleMessage::Stop)),
    );
    let start = button(
        row![
            icons::icon(Icon::Play, 13, SUCCESS),
            text("启动").size(11).font(fonts::MEDIUM).color(SUCCESS)
        ]
        .spacing(6),
    )
    .padding([6, 10])
    .style(soft_button(SUCCESS))
    .on_press_maybe(
        (state.status != ConsoleStatus::Starting && state.status != ConsoleStatus::Running)
            .then_some(Message::Console(ConsoleMessage::Start)),
    );

    let mut actions = row![clear, separator()]
        .spacing(8)
        .align_y(Alignment::Center);
    if let Some(follow) = follow {
        actions = actions.push(follow).push(separator());
    }
    actions = actions.push(stop).push(start);

    container(row![left, space::horizontal(), actions].align_y(Alignment::Center))
        .padding([12, 18])
        .width(Fill)
        .style(header_surface)
        .into()
}

fn logs_view(state: &ConsoleState) -> Element<'_, Message> {
    let content: Element<'_, Message> = if state.logs.is_empty() {
        container(
            column![
                icons::icon(Icon::SquareTerminal, 44, INK_SUBTLE),
                text("控制台已就绪，等待启动服务")
                    .size(13)
                    .font(fonts::REGULAR)
                    .color(INK_MUTED)
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
        let rows = state.logs.iter().map(|log| {
            row![
                text(format!("[{}]", log.time))
                    .size(11)
                    .font(Font::MONOSPACE)
                    .color(INK_SUBTLE)
                    .width(Length::Fixed(72.0)),
                text(log.kind.label().to_uppercase())
                    .size(10)
                    .font(fonts::BOLD)
                    .color(log.kind.color())
                    .width(Length::Fixed(66.0)),
                text(&log.text)
                    .size(12)
                    .font(Font::MONOSPACE)
                    .color(INK)
                    .width(Fill)
            ]
            .spacing(10)
            .align_y(Alignment::Start)
            .into()
        });
        scrollable(column(rows).spacing(5).padding([18, 20]))
            .direction(scrollable::Direction::Vertical(Default::default()))
            .height(Fill)
            .width(Fill)
            .into()
    };

    container(content)
        .height(Fill)
        .width(Fill)
        .style(log_surface)
        .into()
}

fn network_dialog(state: &ConsoleState) -> Element<'_, Message> {
    let mode = state.network_mode.unwrap_or(NetworkMode::Lan);
    let url = match mode {
        NetworkMode::Lan => format!("http://192.168.1.24:{}", state.network_port),
        NetworkMode::Internet => "https://example.tavern.link".into(),
    };
    let panel = column![
        row![
            text(mode.label()).size(17).font(fonts::MEDIUM),
            space::horizontal(),
            button(icons::icon(Icon::X, 16, INK_MUTED))
                .padding(6)
                .style(icon_button_style())
                .on_press(Message::Console(ConsoleMessage::ToggleNetworkDialog))
        ]
        .align_y(Alignment::Center),
        text("将此地址复制到同一网络中的设备，即可访问当前酒馆实例。")
            .size(12)
            .font(fonts::REGULAR)
            .color(INK_MUTED),
        container(
            row![
                text(url).size(13).font(Font::MONOSPACE).color(INK),
                space::horizontal(),
                button(icons::icon(Icon::ExternalLink, 15, BLUE_600))
                    .padding(7)
                    .style(icon_button_style())
                    .on_press(Message::Console(ConsoleMessage::OpenServer))
            ]
            .align_y(Alignment::Center)
        )
        .padding([12, 14])
        .width(Fill)
        .style(dialog_code_surface),
        row![
            space::horizontal(),
            button(text("关闭").size(12).font(fonts::MEDIUM))
                .padding([8, 16])
                .style(button_style(ButtonVariant::Secondary))
                .on_press(Message::Console(ConsoleMessage::ToggleNetworkDialog))
        ],
    ]
    .spacing(14)
    .width(430);
    container(
        container(panel)
            .padding(22)
            .width(470)
            .style(dialog_surface),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(backdrop_surface)
    .into()
}

fn status_badge(status: ConsoleStatus) -> Element<'static, Message> {
    container(
        row![
            icons::icon(status.icon(), 13, status.color()),
            text(status.label())
                .size(11)
                .font(fonts::MEDIUM)
                .color(status.color())
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(move |_theme| soft_surface(status.color()))
    .into()
}

fn separator() -> Element<'static, Message> {
    container(space::vertical().height(16))
        .width(1)
        .style(separator_surface)
        .into()
}

fn header_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb8(250, 251, 253))),
        border: Border {
            color: LINE,
            width: 1.0,
            ..Border::default()
        },
        ..Default::default()
    }
}
fn log_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb8(247, 249, 252))),
        ..Default::default()
    }
}
fn separator_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(LINE)),
        ..Default::default()
    }
}
fn soft_surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.11,
        ))),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}
fn soft_button(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(Color::from_rgba(
                color.r,
                color.g,
                color.b,
                if hovered { 0.18 } else { 0.11 },
            ))),
            border: Border {
                color: Color::from_rgba(color.r, color.g, color.b, 0.26),
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        }
    }
}
fn icon_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    |_theme, status| button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(SURFACE_ALT)),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..Default::default()
    }
}
fn dialog_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    }
}
fn dialog_code_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb8(244, 247, 250))),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}
fn backdrop_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(
            15.0 / 255.0,
            17.0 / 255.0,
            26.0 / 255.0,
            0.45,
        ))),
        ..Default::default()
    }
}

const WARNING: Color = Color::from_rgb8(245, 165, 36);

#[cfg(test)]
mod tests {
    use super::{ConsoleMessage, ConsoleState, ConsoleStatus};

    #[test]
    fn start_and_stop_update_status_and_logs() {
        let mut state = ConsoleState::default();
        state.update(ConsoleMessage::Start);
        assert_eq!(state.status, ConsoleStatus::Running);
        assert_eq!(state.process_pid, Some(48217));
        assert_eq!(state.logs.len(), 4);

        state.update(ConsoleMessage::Stop);
        assert_eq!(state.status, ConsoleStatus::Stopped);
        assert!(state.process_pid.is_none());
        assert_eq!(state.logs.len(), 6);
    }

    #[test]
    fn clearing_logs_keeps_process_state() {
        let mut state = ConsoleState::default();
        state.update(ConsoleMessage::Start);
        state.update(ConsoleMessage::ClearLogs);
        assert!(state.logs.is_empty());
        assert_eq!(state.status, ConsoleStatus::Running);
    }
}
