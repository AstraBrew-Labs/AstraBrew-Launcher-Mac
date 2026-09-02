//! 版本管理页面。
//!
//! 页面复刻旧版启动器的信息结构，并以独立状态承载 Tab、实例列表与操作反馈。
//! 当前服务层尚未接入，因此扫描、安装和切换操作先在本地状态中完成可视反馈。

use iced::widget::{button, column, container, row, scrollable, space, text, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{
    BLUE_600, ButtonVariant, DANGER, INK, INK_MUTED, INK_SUBTLE, LINE, SUCCESS, SURFACE, Separator,
    WARNING, WHITE, button_style, canvas, fonts, icons,
};

/// 版本页当前展示的实例类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionTab {
    #[default]
    Local,
    Online,
}

/// 本地发现或手动导入的酒馆实例。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInstance {
    pub version: String,
    pub path: String,
    pub dependencies_installed: bool,
}

/// 可从远端下载的酒馆发行版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineRelease {
    pub version: String,
    pub published_at: String,
    pub created_at: String,
    pub summary: String,
    pub installed: bool,
}

/// 版本管理页面的本地界面状态。
#[derive(Debug, Clone)]
pub struct VersionState {
    pub active_tab: VersionTab,
    pub current_version: Option<String>,
    pub current_path: Option<String>,
    pub latest_version: String,
    pub local_instances: Vec<LocalInstance>,
    pub online_releases: Vec<OnlineRelease>,
    pub last_sync: String,
    pub notice: Option<String>,
}

impl Default for VersionState {
    fn default() -> Self {
        Self {
            active_tab: VersionTab::Local,
            current_version: None,
            current_path: None,
            latest_version: "1.18.0".into(),
            local_instances: vec![
                LocalInstance {
                    version: "1.18.0".into(),
                    path: "/System/Volumes/Data/Users/al01/Tools/SillyTavern".into(),
                    dependencies_installed: false,
                },
                LocalInstance {
                    version: "1.18.0".into(),
                    path: "/System/Volumes/Data/Users/al01/SillyTavern/SillyTavern".into(),
                    dependencies_installed: false,
                },
                LocalInstance {
                    version: "1.18.0".into(),
                    path: "/Users/al01/Library/Application Support/AstraBrew Launcher/sillytavern"
                        .into(),
                    dependencies_installed: true,
                },
                LocalInstance {
                    version: "1.18.0".into(),
                    path: "/Users/al01/Tools/SillyTavern".into(),
                    dependencies_installed: false,
                },
                LocalInstance {
                    version: "1.18.0".into(),
                    path: "/Users/al01/SillyTavern/SillyTavern".into(),
                    dependencies_installed: false,
                },
            ],
            online_releases: vec![
                release(
                    "1.18.0",
                    "2026/05/03 23:55",
                    "2026/05/03 23:45",
                    "# SillyTavern 1.18.0\n稳定性改进与依赖更新。",
                ),
                release(
                    "1.17.0",
                    "2026/03/29 01:24",
                    "2026/03/29 01:22",
                    "# SillyTavern 1.17.0\n新增配置迁移与兼容性修复。",
                ),
                release(
                    "1.16.0",
                    "2026/02/14 23:47",
                    "2026/02/14 23:46",
                    "# SillyTavern 1.16.0\n优化启动流程与资源加载。",
                ),
                release(
                    "1.15.0",
                    "2025/12/21 20:18",
                    "2025/12/21 20:11",
                    "# SillyTavern 1.15.0\n常规功能更新与问题修复。",
                ),
            ],
            last_sync: "2026/08/30 21:37".into(),
            notice: None,
        }
    }
}

fn release(version: &str, published_at: &str, created_at: &str, summary: &str) -> OnlineRelease {
    OnlineRelease {
        version: version.into(),
        published_at: published_at.into(),
        created_at: created_at.into(),
        summary: summary.into(),
        installed: false,
    }
}

/// 版本页产生的用户操作。
#[derive(Debug, Clone)]
pub enum VersionMessage {
    SelectTab(VersionTab),
    ImportLocal,
    ScanLocal,
    OpenScanLog,
    InstallLocalDependencies(String),
    SwitchLocal(String),
    RemoveLocal(String),
    RefreshOnline,
    InstallOnline(String),
    SwitchOnline(String),
    DeleteOnline(String),
}

impl VersionState {
    /// 更新页面本地状态；真实服务接入后可在这些分支返回异步任务。
    pub fn update(&mut self, message: VersionMessage) {
        match message {
            VersionMessage::SelectTab(tab) => self.active_tab = tab,
            VersionMessage::ImportLocal => {
                self.notice = Some("已保留导入入口：请选择 SillyTavern 的 package.json。".into());
            }
            VersionMessage::ScanLocal => {
                self.notice = Some(format!(
                    "扫描完成，共发现 {} 个本地实例。",
                    self.local_instances.len()
                ));
            }
            VersionMessage::OpenScanLog => {
                self.notice = Some("扫描日志入口已保留，等待扫描服务接入。".into());
            }
            VersionMessage::InstallLocalDependencies(path) => {
                if let Some(instance) = self
                    .local_instances
                    .iter_mut()
                    .find(|instance| instance.path == path)
                {
                    instance.dependencies_installed = true;
                    self.notice = Some(format!("已为 v{} 标记依赖安装完成。", instance.version));
                }
            }
            VersionMessage::SwitchLocal(path) => {
                if let Some(instance) = self
                    .local_instances
                    .iter()
                    .find(|instance| instance.path == path)
                {
                    self.current_version = Some(instance.version.clone());
                    self.current_path = Some(instance.path.clone());
                    self.notice = Some(format!("已切换到本地实例 v{}。", instance.version));
                }
            }
            VersionMessage::RemoveLocal(path) => {
                let is_current = self.current_path.as_deref() == Some(path.as_str());
                if is_current {
                    self.notice = Some("当前正在使用的实例不能从列表中移除。".into());
                } else {
                    self.local_instances
                        .retain(|instance| instance.path != path);
                    self.notice = Some("已从本地实例列表移除。".into());
                }
            }
            VersionMessage::RefreshOnline => {
                self.last_sync = "2026/08/30 21:37".into();
                self.notice = Some("在线版本列表已刷新。".into());
            }
            VersionMessage::InstallOnline(version) => {
                if let Some(release) = self
                    .online_releases
                    .iter_mut()
                    .find(|release| release.version == version)
                {
                    release.installed = true;
                    self.notice = Some(format!("v{} 已加入本地安装列表。", release.version));
                }
            }
            VersionMessage::SwitchOnline(version) => {
                self.current_version = Some(version.clone());
                self.current_path = None;
                self.notice = Some(format!("已切换到在线安装版本 v{version}。"));
            }
            VersionMessage::DeleteOnline(version) => {
                let is_current = self.current_path.is_none()
                    && self.current_version.as_deref() == Some(version.as_str());
                if is_current {
                    self.notice = Some("当前正在使用的版本不能删除。".into());
                } else if let Some(release) = self
                    .online_releases
                    .iter_mut()
                    .find(|release| release.version == version)
                {
                    release.installed = false;
                    self.notice = Some(format!("已删除在线安装版本 v{}。", release.version));
                }
            }
        }
    }
}

/// 渲染版本管理页面。
pub fn versions_view(state: &VersionState) -> Element<'_, VersionMessage> {
    let summary = row![
        summary_card(
            "当前版本",
            state.current_version.as_deref().unwrap_or("未设置"),
            Icon::CircleCheck,
            BLUE_600,
        ),
        summary_card(
            "最新版本",
            &state.latest_version,
            Icon::Sparkles,
            Color::from_rgb8(142, 68, 220),
        ),
    ]
    .spacing(16)
    .width(Fill);

    let tabs = row![
        tab_button(
            "本地实例",
            VersionTab::Local,
            state.active_tab == VersionTab::Local,
        ),
        tab_button(
            "在线实例",
            VersionTab::Online,
            state.active_tab == VersionTab::Online,
        ),
        space::horizontal(),
        state
            .notice
            .as_deref()
            .map(notice_badge)
            .unwrap_or_else(|| space::horizontal().width(Length::Shrink).into()),
    ]
    .spacing(4)
    .align_y(Alignment::End)
    .width(Fill);

    let content = match state.active_tab {
        VersionTab::Local => local_panel(state),
        VersionTab::Online => online_panel(state),
    };

    container(
        container(
            column![summary, tabs, content]
                .spacing(18)
                .width(Fill)
                .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .max_width(940),
    )
    .width(Fill)
    .height(Fill)
    .padding([26, 30])
    .align_x(Alignment::Center)
    .style(canvas)
    .into()
}

fn summary_card<'a>(
    label: &'static str,
    value: &'a str,
    icon: Icon,
    accent: Color,
) -> Element<'a, VersionMessage> {
    container(
        row![
            column![
                text(label).size(11).font(fonts::MEDIUM).color(INK_SUBTLE),
                text(value).size(24).font(fonts::MEDIUM).color(INK),
            ]
            .spacing(5),
            space::horizontal(),
            container(icons::icon(icon, 24, accent))
                .width(52)
                .height(52)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_theme| accent_surface(accent)),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .height(104)
    .padding([18, 22])
    .style(summary_surface)
    .into()
}

fn tab_button(
    label: &'static str,
    tab: VersionTab,
    active: bool,
) -> Element<'static, VersionMessage> {
    let color = if active { BLUE_600 } else { INK_MUTED };
    button(
        column![
            container(text(label).size(13).font(fonts::MEDIUM).color(color))
                .height(32)
                .align_y(Alignment::Center),
            container(space::vertical())
                .width(Fill)
                .height(2)
                .style(move |_theme| tab_indicator(active)),
        ]
        .spacing(0)
        .align_x(Alignment::Center),
    )
    .on_press(VersionMessage::SelectTab(tab))
    .padding([0, 10])
    .style(tab_button_style)
    .into()
}

fn notice_badge(notice: &str) -> Element<'_, VersionMessage> {
    container(
        row![
            icons::icon(Icon::Info, 13, BLUE_600),
            text(notice).size(10).font(fonts::REGULAR).color(INK_MUTED),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(notice_surface)
    .into()
}

fn local_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    let header = panel_header(
        Icon::FolderSearch,
        "本地实例列表",
        Some(format!(
            "扫描完成，共发现 {} 个实例",
            state.local_instances.len()
        )),
        vec![
            icon_button(
                Icon::FolderPlus,
                "导入本地实例",
                VersionMessage::ImportLocal,
            ),
            icon_button(Icon::Search, "扫描本机", VersionMessage::ScanLocal),
            icon_button(Icon::FileText, "查看扫描日志", VersionMessage::OpenScanLog),
        ],
    );

    let list = if state.local_instances.is_empty() {
        container(
            column![
                icons::icon(Icon::FolderSearch, 36, INK_SUBTLE),
                text("尚未发现本地实例")
                    .size(13)
                    .font(fonts::MEDIUM)
                    .color(INK_MUTED),
                button("开始扫描")
                    .on_press(VersionMessage::ScanLocal)
                    .padding([8, 16])
                    .style(button_style(ButtonVariant::Primary)),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
        let rows = state.local_instances.iter().enumerate().fold(
            column![].width(Fill),
            |rows, (index, item)| {
                let rows = rows.push(local_instance_row(
                    item,
                    state.current_path.as_deref() == Some(item.path.as_str()),
                ));
                if index + 1 == state.local_instances.len() {
                    rows
                } else {
                    rows.push(Separator::new())
                }
            },
        );
        scrollable(rows).height(Fill).into()
    };

    panel(header, list)
}

fn local_instance_row<'a>(item: &'a LocalInstance, current: bool) -> Element<'a, VersionMessage> {
    let primary_action = if item.dependencies_installed {
        let label = if current {
            "当前使用"
        } else {
            "切换版本"
        };
        let mut action = button(
            row![
                icons::icon(
                    Icon::Power,
                    15,
                    if current { INK_SUBTLE } else { INK_MUTED }
                ),
                text(label).size(12).font(fonts::MEDIUM),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([9, 14])
        .style(button_style(ButtonVariant::Outline));
        if !current {
            action = action.on_press(VersionMessage::SwitchLocal(item.path.clone()));
        }
        action
    } else {
        button(
            row![
                icons::icon(Icon::Download, 15, WHITE),
                text("安装依赖").size(12).font(fonts::MEDIUM).color(WHITE),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::InstallLocalDependencies(item.path.clone()))
        .padding([9, 14])
        .style(warning_button_style)
    };

    let mut remove = button(
        row![
            icons::icon(Icon::Trash2, 15, DANGER),
            text("从列表中移除")
                .size(12)
                .font(fonts::MEDIUM)
                .color(DANGER),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([9, 14])
    .style(button_style(ButtonVariant::DangerSoft));
    if !current {
        remove = remove.on_press(VersionMessage::RemoveLocal(item.path.clone()));
    }

    container(
        row![
            container(icons::icon(Icon::Box, 20, Color::from_rgb8(88, 80, 236)))
                .width(40)
                .height(40)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(indigo_icon_surface),
            column![
                row![
                    text(format!("v{}", item.version))
                        .size(15)
                        .font(fonts::MEDIUM),
                    current_badge(current),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    icons::icon(Icon::MapPin, 12, INK_SUBTLE),
                    text(&item.path)
                        .size(10)
                        .font(fonts::REGULAR)
                        .color(INK_MUTED),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            ]
            .spacing(5)
            .width(Fill),
            row![primary_action, remove]
                .spacing(8)
                .align_y(Alignment::Center),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([16, 20])
    .into()
}

fn online_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    let header = panel_header(
        Icon::History,
        "版本列表",
        Some(format!("上次同步：{}", state.last_sync)),
        vec![icon_button(
            Icon::RefreshCw,
            "刷新在线版本",
            VersionMessage::RefreshOnline,
        )],
    );

    let rows = state.online_releases.iter().enumerate().fold(
        column![].width(Fill),
        |rows, (index, release)| {
            let rows = rows.push(online_release_row(
                release,
                release.version == state.latest_version,
                state.current_path.is_none()
                    && state.current_version.as_deref() == Some(release.version.as_str()),
            ));
            if index + 1 == state.online_releases.len() {
                rows
            } else {
                rows.push(Separator::new())
            }
        },
    );
    panel(header, scrollable(rows).height(Fill).into())
}

fn online_release_row<'a>(
    release: &'a OnlineRelease,
    latest: bool,
    current: bool,
) -> Element<'a, VersionMessage> {
    let actions: Element<'a, VersionMessage> = if release.installed {
        let label = if current {
            "当前使用"
        } else {
            "切换版本"
        };
        let mut switch = button(
            row![
                icons::icon(Icon::Power, 15, INK_MUTED),
                text(label).size(12).font(fonts::MEDIUM),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([9, 14])
        .style(button_style(ButtonVariant::Outline));
        if !current {
            switch = switch.on_press(VersionMessage::SwitchOnline(release.version.clone()));
        }

        let mut delete = button(
            row![
                icons::icon(Icon::Trash2, 15, DANGER),
                text("删除版本").size(12).font(fonts::MEDIUM).color(DANGER),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .padding([9, 14])
        .style(button_style(ButtonVariant::DangerSoft));
        if !current {
            delete = delete.on_press(VersionMessage::DeleteOnline(release.version.clone()));
        }
        row![switch, delete]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
    } else {
        button(
            row![
                icons::icon(Icon::Download, 15, WHITE),
                text("下载安装").size(12).font(fonts::MEDIUM).color(WHITE),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::InstallOnline(release.version.clone()))
        .padding([9, 15])
        .style(button_style(ButtonVariant::Primary))
        .into()
    };

    container(
        row![
            column![
                row![
                    text(&release.version).size(15).font(fonts::MEDIUM),
                    latest_badge(latest),
                    installed_badge(release.installed),
                    current_badge(current),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
                row![
                    row![
                        icons::icon(Icon::Calendar, 12, INK_SUBTLE),
                        text(format!("发布于 {}", release.published_at))
                            .size(10)
                            .font(fonts::REGULAR)
                            .color(INK_MUTED),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                    row![
                        icons::icon(Icon::Clock, 12, INK_SUBTLE),
                        text(format!("创建于 {}", release.created_at))
                            .size(10)
                            .font(fonts::REGULAR)
                            .color(INK_MUTED),
                    ]
                    .spacing(5)
                    .align_y(Alignment::Center),
                ]
                .spacing(14),
                text(&release.summary)
                    .size(11)
                    .font(fonts::REGULAR)
                    .color(INK_MUTED),
            ]
            .spacing(7)
            .width(Fill),
            actions,
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([16, 20])
    .into()
}

fn panel<'a>(
    header: Element<'a, VersionMessage>,
    content: Element<'a, VersionMessage>,
) -> Element<'a, VersionMessage> {
    container(
        column![header, Separator::new(), content]
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(panel_surface)
    .into()
}

fn panel_header<'a>(
    icon: Icon,
    title: &'static str,
    meta: Option<String>,
    actions: Vec<Element<'a, VersionMessage>>,
) -> Element<'a, VersionMessage> {
    let mut title_row = row![
        icons::icon(icon, 19, INK_MUTED),
        text(title).size(15).font(fonts::MEDIUM),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(meta) = meta {
        title_row = title_row.push(
            container(text(meta).size(9).font(fonts::REGULAR).color(INK_MUTED))
                .padding([5, 9])
                .style(meta_surface),
        );
    }

    container(
        title_row
            .push(space::horizontal())
            .extend(actions)
            .spacing(4),
    )
    .width(Fill)
    .height(58)
    .padding([0, 20])
    .align_y(Alignment::Center)
    .style(panel_header_surface)
    .into()
}

fn icon_button(
    icon: Icon,
    label: &'static str,
    message: VersionMessage,
) -> Element<'static, VersionMessage> {
    let action = button(
        container(icons::icon(icon, 16, INK_MUTED))
            .width(28)
            .height(28)
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

fn latest_badge(show: bool) -> Element<'static, VersionMessage> {
    optional_badge(show, "LATEST", Color::from_rgb8(142, 68, 220))
}

fn installed_badge(show: bool) -> Element<'static, VersionMessage> {
    optional_badge(show, "已安装", SUCCESS)
}

fn current_badge(show: bool) -> Element<'static, VersionMessage> {
    optional_badge(show, "当前", BLUE_600)
}

fn optional_badge(
    show: bool,
    label: &'static str,
    color: Color,
) -> Element<'static, VersionMessage> {
    if !show {
        return space::horizontal().width(0).into();
    }
    container(text(label).size(8).font(fonts::MEDIUM).color(color))
        .padding([4, 7])
        .style(move |_theme| badge_surface(color))
        .into()
}

fn summary_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 16.0.into(),
        },
        ..container::Style::default()
    }
}

fn accent_surface(accent: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            accent.r, accent.g, accent.b, 0.11,
        ))),
        border: Border {
            radius: 12.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn tab_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(Color::from_rgb8(238, 244, 250))),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tab_indicator(active: bool) -> container::Style {
    container::Style {
        background: active.then_some(Background::Color(BLUE_600)),
        border: Border {
            radius: 2.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn panel_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 16.0.into(),
        },
        ..container::Style::default()
    }
}

fn panel_header_surface(_theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: LINE,
            width: 0.0,
            radius: 16.0.into(),
        },
        ..container::Style::default()
    }
}

fn meta_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(247, 249, 252))),
        border: Border {
            color: LINE,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
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

fn notice_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(235, 245, 255))),
        border: Border {
            color: Color::from_rgb8(205, 227, 249),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

fn indigo_icon_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(238, 237, 255))),
        border: Border {
            radius: 10.0.into(),
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
            radius: 10.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn warning_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Color::from_rgb8(230, 145, 16)
    } else {
        WARNING
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: WHITE,
        border: Border {
            radius: 18.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{VersionMessage, VersionState, VersionTab};

    #[test]
    fn tab_can_switch_to_online_instances() {
        let mut state = VersionState::default();
        state.update(VersionMessage::SelectTab(VersionTab::Online));
        assert_eq!(state.active_tab, VersionTab::Online);
    }

    #[test]
    fn installing_online_release_updates_its_state() {
        let mut state = VersionState::default();
        state.update(VersionMessage::InstallOnline("1.18.0".into()));
        assert!(
            state
                .online_releases
                .iter()
                .find(|release| release.version == "1.18.0")
                .is_some_and(|release| release.installed)
        );
    }

    #[test]
    fn current_local_instance_cannot_be_removed() {
        let mut state = VersionState::default();
        let path = state.local_instances[0].path.clone();
        state.update(VersionMessage::InstallLocalDependencies(path.clone()));
        state.update(VersionMessage::SwitchLocal(path.clone()));
        state.update(VersionMessage::RemoveLocal(path.clone()));
        assert!(
            state
                .local_instances
                .iter()
                .any(|instance| instance.path == path)
        );
    }
}
