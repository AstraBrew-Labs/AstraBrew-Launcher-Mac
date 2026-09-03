//! 主界面各功能页面的路由枚举与视图分发。
//!
//! 设置页和酒馆配置页已经接入真实视图，其余页面暂时提供居中的占位内容。

use iced::widget::{button, column, container, image, pick_list, row, scrollable, space};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{BLUE_600, ButtonVariant, SUCCESS, ToggleButtonGroupItem, WHITE, fonts, icons, pick_list_handle};

use crate::app::Message;
use crate::lang::text;
use crate::theme::{button_style, pick_list_menu_style, pick_list_style};
pub(crate) mod console;
pub(crate) mod extensions;
pub(crate) mod resource_manage;
pub(crate) mod settings;
pub(crate) mod tavern;
pub(crate) mod versions;

use self::console::{ConsoleState, console_view};
use self::extensions::{ExtensionsState, extensions_view};
use self::resource_manage::{ResourceManageState, resource_manage_view};
use self::settings::{
    QuickStartMode, ServerServiceMode, SettingsState, TavernVersion, settings_view,
};
use self::tavern::{BrowserType, TavernState, tavern_view};
use self::versions::{VersionState, versions_view};

const HOME_HERO_HEIGHT: f32 = 240.0;

/// 主界面导航页面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// 主页
    Home,
    /// 酒馆配置
    TavernConfig,
    /// 版本管理
    Version,
    /// 扩展管理
    Extensions,
    /// 资源管理
    Resources,
    /// 控制台
    Console,
    /// 设置
    Settings,
}

impl Page {
    /// 页面在导航栏中的中文标题。
    pub const fn title(self) -> &'static str {
        match self {
            Page::Home => "主页",
            Page::TavernConfig => "酒馆配置",
            Page::Version => "版本管理",
            Page::Extensions => "扩展管理",
            Page::Resources => "资源管理",
            Page::Console => "控制台",
            Page::Settings => "设置",
        }
    }

    /// 页面在导航栏中对应的 Lucide 图标。
    pub const fn icon(self) -> Icon {
        match self {
            Page::Home => Icon::House,
            Page::TavernConfig => Icon::SlidersHorizontal,
            Page::Version => Icon::GitBranch,
            Page::Extensions => Icon::Puzzle,
            Page::Resources => Icon::FolderOpen,
            Page::Console => Icon::SquareTerminal,
            Page::Settings => Icon::Settings,
        }
    }
}

/// 渲染指定页面的内容。
///
/// 设置页和酒馆配置页分发到真实视图，其余页面居中显示开发中说明。
pub fn page_view<'a>(
    page: Page,
    settings: &'a SettingsState,
    tavern: &'a TavernState,
    versions: &'a VersionState,
    extensions: &'a ExtensionsState,
    resources: &'a ResourceManageState,
    launch_requested: bool,
    console: &'a ConsoleState,
) -> Element<'a, Message> {
    match page {
        Page::Home => home_view(settings, tavern, launch_requested),
        Page::Settings => settings_view(settings),
        Page::TavernConfig => tavern_view(tavern).map(Message::Tavern),
        Page::Version => versions_view(versions).map(Message::Version),
        Page::Extensions => extensions_view(extensions, versions).map(Message::Extensions),
        Page::Resources => resource_manage_view(resources).map(Message::Resources),
        Page::Console => console_view(console),
    }
}

/// 主页：用启动入口、环境摘要和运行配置把常用操作集中在首屏。
fn home_view<'a>(
    state: &'a SettingsState,
    tavern: &'a TavernState,
    launch_requested: bool,
) -> Element<'a, Message> {
    let launch_label = if launch_requested {
        "已发送启动请求"
    } else {
        "一键启动"
    };
    let launch_color = if launch_requested { SUCCESS } else { WHITE };

    let hero = crate::theme::card(
        image("assets/imgs/og.png")
            .width(Fill)
            .height(Length::Fixed(HOME_HERO_HEIGHT))
            .content_fit(ContentFit::Cover),
        Fill,
        0,
    );

    let environment = crate::theme::card(
        column![
            row![
                column![
                    text("运行环境").size(16).font(fonts::MEDIUM),
                    text("启动器检测到的本机依赖与当前配置")
                        .size(11)
                        .font(fonts::REGULAR)
                        .style(crate::theme::muted_text_style),
                ]
                .spacing(4),
                space::horizontal(),
                status_badge("环境正常"),
            ]
            .align_y(Alignment::Center),
            container(
                row![
                    info_item(Icon::FlaskConical, "Homebrew", "4.5.1"),
                    info_item(Icon::GitBranch, "Git", "2.49.0"),
                    info_item(Icon::Hexagon, "Node.js", "22.14.0"),
                    info_item(Icon::Beer, "酒馆版本", state.tavern_version.label()),
                    info_item(Icon::Rocket, "启动模式", current_quick_mode(state).label()),
                ]
                .spacing(10),
            )
            .width(Fill)
            .padding([14, 16])
            .style(info_surface),
        ]
        .spacing(16),
        Fill,
        20,
    );

    let version_select = column![
        text("酒馆版本")
            .size(11)
            .font(fonts::MEDIUM)
            .style(crate::theme::muted_text_style),
        pick_list(
            TavernVersion::ALL,
            Some(state.tavern_version),
            Message::HomeTavernVersionSelected,
        )
        .width(170)
        .padding([8, 11])
        .text_size(12)
        .font(fonts::REGULAR)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style),
    ]
    .spacing(6);

    let selected_mode = current_quick_mode(state);
    let mode_items = [
        (QuickStartMode::Normal, Icon::Play),
        (QuickStartMode::Desktop, Icon::AppWindow),
        (QuickStartMode::Server, Icon::Server),
    ]
    .into_iter()
    .map(|(mode, icon)| {
        ToggleButtonGroupItem::new(Some(mode.label()), Some(icon), mode == selected_mode)
    })
    .collect();
    let mode_select = column![
        text("启动模式")
            .size(11)
            .font(fonts::MEDIUM)
            .style(crate::theme::muted_text_style),
        themed_segmented_group(mode_items, |index| {
            Message::HomeStartModeSelected(quick_mode_from_index(index))
        }),
    ]
    .spacing(6);

    let browser_select: Option<Element<'_, Message>> = (selected_mode == QuickStartMode::Normal)
        .then(|| {
            let browser_items = [
                (BrowserType::System, Icon::Globe),
                (BrowserType::Chrome, Icon::Monitor),
                (BrowserType::Firefox, Icon::Compass),
                (BrowserType::Edge, Icon::PanelsTopLeft),
                (BrowserType::Safari, Icon::Compass),
            ]
            .into_iter()
            .map(|(browser, icon)| {
                ToggleButtonGroupItem::new(
                    Some(browser.label()),
                    Some(icon),
                    browser == tavern.browser_type(),
                )
            })
            .collect();

            column![
                text("浏览器").size(11).font(fonts::MEDIUM).style(crate::theme::muted_text_style),
                themed_segmented_group(browser_items, |index| {
                    Message::HomeBrowserSelected(browser_type_from_index(index))
                }),
            ]
            .spacing(6)
            .into()
        });

    let service_mode_select: Option<Element<'_, Message>> = state.server_mode_enabled.then(|| {
        let items = [
            (ServerServiceMode::Lan, "局域网", Icon::Wifi),
            (ServerServiceMode::Internet, "互联网", Icon::Globe),
        ]
        .into_iter()
        .map(|(mode, label, icon)| {
            ToggleButtonGroupItem::new(Some(label), Some(icon), mode == state.server_service_mode)
        })
        .collect();

        column![
            text("服务模式")
                .size(11)
                .font(fonts::MEDIUM)
                .style(crate::theme::muted_text_style),
            themed_segmented_group(items, |index| {
                Message::SettingsServerServiceModeSelected(server_service_mode_from_index(index))
            }),
        ]
        .spacing(6)
        .into()
    });

    let launch_button = button(
        row![
            icons::icon(Icon::Play, 17, launch_color),
            text(launch_label)
                .size(13)
                .font(fonts::MEDIUM)
                .color(launch_color),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .on_press(Message::LaunchTavern)
    .height(42)
    .padding([10, 18])
    .style(button_style(ButtonVariant::Primary));

    let launch_controls = row![version_select, mode_select]
        .spacing(16)
        .align_y(Alignment::Center);
    let launch_controls = if let Some(browser_select) = browser_select {
        launch_controls.push(browser_select)
    } else {
        launch_controls
    };
    let launch_controls = if let Some(service_mode_select) = service_mode_select {
        launch_controls.push(service_mode_select)
    } else {
        launch_controls
    };
    let launch_panel = crate::theme::card(
        launch_controls
            .push(space::horizontal())
            .push(launch_button),
        Fill,
        18,
    );

    container(
        column![
            scrollable(
                column![
                    column![
                        text("主页").size(25).font(fonts::MEDIUM),
                        text("AstraBrew Launcher")
                            .size(12)
                            .font(fonts::REGULAR)
                            .style(crate::theme::muted_text_style),
                    ]
                    .spacing(4),
                    hero,
                    environment,
                ]
                .spacing(18)
                .width(Fill),
            )
            .width(Fill)
            .height(Fill),
            launch_panel,
        ]
        .spacing(14)
        .width(Fill)
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .padding([26, 30])
    .style(crate::theme::canvas_style)
    .into()
}

fn current_quick_mode(state: &SettingsState) -> QuickStartMode {
    if state.server_mode_enabled {
        QuickStartMode::Server
    } else if state.start_mode == settings::StartMode::Desktop {
        QuickStartMode::Desktop
    } else {
        QuickStartMode::Normal
    }
}

/// 主页使用的主题感知分段选择器，避免 Astra UI 默认的固定浅色背景。
fn themed_segmented_group<'a>(
    items: Vec<ToggleButtonGroupItem<'a>>,
    on_toggle: impl Fn(usize) -> Message + Clone + 'a,
) -> Element<'a, Message> {
    let item_count = items.len();
    let controls = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let selected = item.selected;
            let label = item.label.unwrap_or_default();
            let icon = item.icon;
            let mut content = row![].spacing(6).align_y(Alignment::Center);
            if let Some(icon) = icon {
                let icon_text: iced::widget::Text<'a> = icon.into();
                content = content.push(
                    icon_text
                        .size(15)
                        .style(move |theme| iced::widget::text::Style {
                            color: Some(if selected {
                                WHITE
                            } else {
                                crate::theme::text_muted(theme)
                            }),
                        }),
                );
            }
            content = content.push(text(label).size(11).font(fonts::MEDIUM));
            button(container(content).align_x(Alignment::Center).align_y(Alignment::Center))
                .height(34)
                .padding([0, 11])
                .on_press(on_toggle.clone()(index))
                .style(move |theme, status| {
                    segmented_button_style(theme, selected, status, index, item_count)
                })
                .into()
        })
        .collect::<Vec<_>>();

    container(row(controls).spacing(0))
        .style(crate::theme::segmented_group_style)
        .into()
}

fn segmented_button_style(
    theme: &Theme,
    selected: bool,
    status: button::Status,
    index: usize,
    item_count: usize,
) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    let background = if selected {
        theme.palette().primary
    } else if hovered {
        crate::theme::surface(theme)
    } else {
        crate::theme::surface_alt(theme)
    };
    let radius = if item_count <= 1 {
        iced::border::Radius::from(10.0)
    } else if index == 0 {
        iced::border::Radius::default().left(10.0)
    } else if index + 1 == item_count {
        iced::border::Radius::default().right(10.0)
    } else {
        iced::border::Radius::default()
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if selected {
            WHITE
        } else {
            crate::theme::text(theme)
        },
        border: Border {
            radius,
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn quick_mode_from_index(index: usize) -> QuickStartMode {
    match index {
        1 => QuickStartMode::Desktop,
        2 => QuickStartMode::Server,
        _ => QuickStartMode::Normal,
    }
}

fn server_service_mode_from_index(index: usize) -> ServerServiceMode {
    match index {
        1 => ServerServiceMode::Internet,
        _ => ServerServiceMode::Lan,
    }
}

fn browser_type_from_index(index: usize) -> BrowserType {
    match index {
        1 => BrowserType::Chrome,
        2 => BrowserType::Firefox,
        3 => BrowserType::Edge,
        4 => BrowserType::Safari,
        _ => BrowserType::System,
    }
}

fn info_item(icon: Icon, label: &'static str, value: &'static str) -> Element<'static, Message> {
    container(
        row![
            container(icons::icon(icon, 16, BLUE_600))
                .width(28)
                .height(28)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(icon_surface),
            column![
                text(label).size(11).font(fonts::REGULAR).style(crate::theme::muted_text_style),
                text(value).size(13).font(fonts::MEDIUM),
            ]
            .spacing(2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .into()
}

fn status_badge(label: &'static str) -> Element<'static, Message> {
    container(
        row![
            icons::icon(Icon::CircleCheck, 14, SUCCESS),
            text(label).size(11).font(fonts::MEDIUM).color(SUCCESS),
        ]
        .spacing(5)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(status_surface)
    .into()
}

fn info_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(if crate::theme::is_dark(theme) {
            crate::theme::surface_alt(theme)
        } else {
            Color::from_rgb8(246, 250, 255)
        })),
        border: Border {
            radius: 12.0.into(),
            color: if crate::theme::is_dark(theme) {
                crate::theme::line(theme)
            } else {
                Color::from_rgb8(224, 235, 248)
            },
            width: 1.0,
        },
        ..iced::widget::container::Style::default()
    }
}

fn icon_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) { 0.18 } else { 0.10 },
        ))),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    }
}

fn status_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().success.r,
            theme.palette().success.g,
            theme.palette().success.b,
            if crate::theme::is_dark(theme) { 0.18 } else { 0.12 },
        ))),
        border: Border {
            radius: 20.0.into(),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    }
}
