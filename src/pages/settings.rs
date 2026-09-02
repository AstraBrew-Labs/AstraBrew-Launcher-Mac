//! 单页设置视图：沿用旧版设置清单，使用 Astra UI 重新排版。

use std::fmt;

use iced::widget::{
    button, column, container, pick_list, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Theme};
use lucide_icons::Icon;

use astra_ui::{
    Alert, AlertKind, BLUE_600, ButtonVariant, Card, ChipVariant, INK, INK_MUTED, INK_SUBTLE,
    RADIUS_FIELD, Separator, SeparatorVariant, WARNING, button_style, chip, fonts, icons,
    pick_list_handle, pick_list_menu_style, pick_list_style, switch, text_input_style,
};

use crate::app::Message;

const SELECT_WIDTH: f32 = 190.0;
const INPUT_WIDTH: f32 = 250.0;

macro_rules! enum_text {
    ($ty:ident, $([$variant:ident, $label:literal]),+ $(,)?) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self { $(Self::$variant => $label,)+ })
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayLanguage {
    SimplifiedChinese,
    English,
    #[default]
    System,
}
impl DisplayLanguage {
    pub const ALL: [Self; 3] = [Self::System, Self::SimplifiedChinese, Self::English];
}
enum_text!(
    DisplayLanguage,
    [SimplifiedChinese, "简体中文"],
    [English, "English"],
    [System, "跟随系统"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}
impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];
}
enum_text!(
    ThemeMode,
    [Light, "浅色"],
    [Dark, "深色"],
    [System, "跟随系统"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CpuCores {
    #[default]
    Auto,
    Half,
    All,
}
impl CpuCores {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Half, Self::All];
}
enum_text!(
    CpuCores,
    [Auto, "自动"],
    [Half, "一半核心"],
    [All, "全部核心"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartMode {
    #[default]
    Normal,
    Desktop,
}
impl StartMode {
    pub const ALL: [Self; 2] = [Self::Normal, Self::Desktop];
}
enum_text!(StartMode, [Normal, "正常模式"], [Desktop, "桌面模式"]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuickStartMode {
    #[default]
    Normal,
    Desktop,
    Server,
}
impl QuickStartMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "普通模式",
            Self::Desktop => "桌面模式",
            Self::Server => "服务器模式",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TavernVersion {
    #[default]
    V1_15_0,
    V1_14_8,
    V1_13_5,
}
impl TavernVersion {
    pub const ALL: [Self; 3] = [Self::V1_15_0, Self::V1_14_8, Self::V1_13_5];

    pub const fn label(self) -> &'static str {
        match self {
            Self::V1_15_0 => "1.15.0",
            Self::V1_14_8 => "1.14.8",
            Self::V1_13_5 => "1.13.5",
        }
    }
}
impl fmt::Display for TavernVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServerServiceMode {
    #[default]
    Lan,
    Internet,
}
impl ServerServiceMode {
    pub const ALL: [Self; 2] = [Self::Lan, Self::Internet];
}
enum_text!(ServerServiceMode, [Lan, "局域网"], [Internet, "互联网"]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TavernDataMode {
    Global,
    #[default]
    Current,
}
impl TavernDataMode {
    pub const ALL: [Self; 2] = [Self::Global, Self::Current];
}
enum_text!(TavernDataMode, [Global, "全局数据"], [Current, "独立数据"]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpmRegistry {
    Official,
    #[default]
    Npmmirror,
    Tencent,
}
impl NpmRegistry {
    pub const ALL: [Self; 3] = [Self::Npmmirror, Self::Official, Self::Tencent];
}
enum_text!(
    NpmRegistry,
    [Official, "NPM 官方源"],
    [Npmmirror, "淘宝镜像源"],
    [Tencent, "腾讯云镜像"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProxyMode {
    #[default]
    None,
    System,
    Custom,
}
impl ProxyMode {
    pub const ALL: [Self; 3] = [Self::None, Self::System, Self::Custom];
}
enum_text!(
    ProxyMode,
    [None, "关闭"],
    [System, "跟随系统"],
    [Custom, "自定义代理"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    OpenLoginItemSettings,
    ChooseExportPath,
    ManageReverseProxy,
    ChooseGlobalDataPath,
    DetectAll,
    ManageHomebrew,
    ManageGit,
    ManageNode,
    ManageCaddy,
    ManagePm2,
    RefreshGithubNodes,
    TestGithub,
    CheckUpdate,
}
impl SettingsAction {
    pub const fn feedback(self) -> &'static str {
        match self {
            Self::OpenLoginItemSettings => "系统登录项设置入口待接入。",
            Self::ChooseExportPath => "酒馆导出目录选择器待接入。",
            Self::ManageReverseProxy => "反向代理配置面板待接入。",
            Self::ChooseGlobalDataPath => "全局数据目录选择器待接入。",
            Self::DetectAll => "环境依赖检测服务待接入。",
            Self::ManageHomebrew => "Homebrew 安装与更新服务待接入。",
            Self::ManageGit => "Git 安装与更新服务待接入。",
            Self::ManageNode => "Node.js 安装与更新服务待接入。",
            Self::ManageCaddy => "Caddy 安装与更新服务待接入。",
            Self::ManagePm2 => "PM2 安装与更新服务待接入。",
            Self::RefreshGithubNodes => "GitHub 加速节点获取与测速服务待接入。",
            Self::TestGithub => "GitHub 连通性测试服务待接入。",
            Self::CheckUpdate => "启动器更新检查服务待接入。",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub language: DisplayLanguage,
    pub theme: ThemeMode,
    pub remember_window_position: bool,
    pub auto_start: bool,
    pub cpu_cores: CpuCores,
    pub start_mode: StartMode,
    pub tavern_version: TavernVersion,
    pub auto_stop_tavern_on_window_close: bool,
    pub tavern_export_path: String,
    pub server_mode_enabled: bool,
    pub server_service_mode: ServerServiceMode,
    pub allow_tavern_background: bool,
    pub data_mode: TavernDataMode,
    pub global_data_path: String,
    pub show_startup_command: bool,
    pub npm_registry: NpmRegistry,
    pub github_proxy_enabled: bool,
    pub github_proxy_url: String,
    pub proxy_mode: ProxyMode,
    pub custom_proxy: String,
    pub last_action: Option<SettingsAction>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            language: DisplayLanguage::System,
            theme: ThemeMode::System,
            remember_window_position: true,
            auto_start: false,
            cpu_cores: CpuCores::Auto,
            start_mode: StartMode::Normal,
            tavern_version: TavernVersion::default(),
            auto_stop_tavern_on_window_close: true,
            tavern_export_path: "~/Downloads".into(),
            server_mode_enabled: false,
            server_service_mode: ServerServiceMode::Lan,
            allow_tavern_background: false,
            data_mode: TavernDataMode::Current,
            global_data_path: "~/Library/Application Support/AstraBrew/data".into(),
            show_startup_command: false,
            npm_registry: NpmRegistry::Npmmirror,
            github_proxy_enabled: false,
            github_proxy_url: "https://gh-proxy.org/".into(),
            proxy_mode: ProxyMode::None,
            custom_proxy: String::new(),
            last_action: None,
        }
    }
}

pub fn settings_view(state: &SettingsState) -> Element<'_, Message> {
    let header = row![
        column![
            text("设置").size(24).font(fonts::MEDIUM),
            text("管理启动器外观、酒馆运行方式、环境依赖与网络连接")
                .size(12)
                .font(fonts::REGULAR)
                .color(INK_MUTED)
        ]
        .spacing(4),
        space::horizontal(),
        button(
            row![
                icons::icon(Icon::RotateCcw, 15, INK_MUTED),
                text("恢复默认").size(12).font(fonts::MEDIUM)
            ]
            .spacing(7)
            .align_y(Alignment::Center)
        )
        .on_press(Message::SettingsRestoreDefaults)
        .height(36)
        .padding([8, 13])
        .style(button_style(ButtonVariant::Outline)),
    ]
    .align_y(Alignment::Center)
    .width(Fill);

    let mut sections = column![
        interface_settings(state),
        basic_settings(state),
        console_settings(state),
        environment_settings(state),
        github_settings(state),
        network_settings(state),
        software_settings(),
    ]
    .spacing(22)
    .width(Fill);
    if let Some(action) = state.last_action {
        sections = sections.push(
            Alert::new("功能入口已保留")
                .description(action.feedback())
                .kind(AlertKind::Info),
        );
    }

    container(
        column![
            header,
            scrollable(container(sections).padding([0, 24]))
                .width(Fill)
                .height(Fill)
        ]
        .spacing(20),
    )
    .width(Fill)
    .height(Fill)
    .padding([24, 28])
    .style(astra_ui::canvas)
    .into()
}

fn interface_settings(state: &SettingsState) -> Element<'_, Message> {
    section(
        Icon::Palette,
        "界面设置",
        "调整启动器的显示语言、主题与窗口行为。",
        section_rows(vec![
            setting_row(
                Icon::Languages,
                "语言",
                "选择显示语言或跟随系统。",
                select_control(
                    &DisplayLanguage::ALL,
                    state.language,
                    Message::SettingsLanguageSelected,
                ),
            ),
            setting_row(
                Icon::SunMoon,
                "主题",
                "选择浅色、深色或跟随系统外观。",
                select_control(&ThemeMode::ALL, state.theme, Message::SettingsThemeSelected),
            ),
            setting_row(
                Icon::PanelsTopLeft,
                "记住上次窗口位置",
                "启动时恢复上次窗口的位置和大小。",
                toggle_control(
                    state.remember_window_position,
                    Message::SettingsRememberWindowPosition,
                ),
            ),
        ]),
    )
}

fn basic_settings(state: &SettingsState) -> Element<'_, Message> {
    section(
        Icon::SlidersHorizontal,
        "基本设置",
        "配置启动行为、酒馆运行模式与数据存放方式。",
        section_rows(vec![
            setting_row(
                Icon::Power,
                "软件自启动",
                "开机时自动启动星酿启动器。",
                row![
                    toggle_control(state.auto_start, Message::SettingsAutoStart),
                    action_button(
                        "系统设置",
                        Icon::ExternalLink,
                        SettingsAction::OpenLoginItemSettings
                    )
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into(),
            ),
            setting_row(
                Icon::Cpu,
                "扫描占用核心数",
                "分配用于全盘扫描的 CPU 线程数。",
                select_control(
                    &CpuCores::ALL,
                    state.cpu_cores,
                    Message::SettingsCpuCoresSelected,
                ),
            ),
            setting_row(
                Icon::Play,
                "酒馆启动模式",
                "正常模式直接使用浏览器；桌面模式使用内置 WebView。",
                select_control(
                    &StartMode::ALL,
                    state.start_mode,
                    Message::SettingsStartModeSelected,
                ),
            ),
            setting_row(
                Icon::CircleStop,
                "关闭酒馆窗口自动停止服务",
                "桌面模式下关闭 WebView 窗口时自动停止酒馆服务。",
                toggle_control(
                    state.auto_stop_tavern_on_window_close,
                    Message::SettingsAutoStopTavern,
                ),
            ),
            setting_row(
                Icon::FolderOpen,
                "导出保存目录",
                "酒馆页面导出文件的默认保存位置。",
                path_control(&state.tavern_export_path, SettingsAction::ChooseExportPath),
            ),
            setting_row(
                Icon::Server,
                "启用服务器模式",
                "把此设备作为仅运行酒馆服务的服务器。",
                toggle_control(state.server_mode_enabled, Message::SettingsServerMode),
            ),
            setting_row(
                Icon::Globe,
                "酒馆服务模式",
                "选择只向局域网开放，或向互联网开放。",
                select_control(
                    &ServerServiceMode::ALL,
                    state.server_service_mode,
                    Message::SettingsServerServiceModeSelected,
                ),
            ),
            setting_row(
                Icon::CloudCog,
                "允许酒馆后台运行",
                "关闭启动器后继续运行酒馆服务，需要 PM2。",
                toggle_control(
                    state.allow_tavern_background,
                    Message::SettingsAllowTavernBackground,
                ),
            ),
            setting_row(
                Icon::Waypoints,
                "反向代理",
                "管理互联网服务模式使用的域名、端口与证书，需要 Caddy。",
                action_button(
                    "管理",
                    Icon::ChevronRight,
                    SettingsAction::ManageReverseProxy,
                ),
            ),
            setting_row(
                Icon::Database,
                "酒馆数据模式",
                "全局模式共用数据；独立模式让各酒馆使用自己的数据。",
                select_control(
                    &TavernDataMode::ALL,
                    state.data_mode,
                    Message::SettingsDataModeSelected,
                ),
            ),
            setting_row(
                Icon::FolderCog,
                "全局数据存放位置",
                "设置全局数据模式下酒馆配置与数据的存储目录。",
                path_control(
                    &state.global_data_path,
                    SettingsAction::ChooseGlobalDataPath,
                ),
            ),
        ]),
    )
}

fn console_settings(state: &SettingsState) -> Element<'_, Message> {
    section(
        Icon::SquareTerminal,
        "控制台设置",
        "控制酒馆启动日志中显示的信息。",
        section_rows(vec![setting_row(
            Icon::Terminal,
            "显示完整的启动命令",
            "启动酒馆时在控制台日志中显示完整命令。",
            toggle_control(
                state.show_startup_command,
                Message::SettingsShowStartupCommand,
            ),
        )]),
    )
}

fn environment_settings(state: &SettingsState) -> Element<'_, Message> {
    let overview = container(
        row![
            row![
                icons::icon(Icon::CircleAlert, 17, WARNING),
                column![
                    text("Homebrew、Git 与 Node.js 为必装依赖")
                        .size(12)
                        .font(fonts::MEDIUM),
                    text("Caddy 与 PM2 会在启用对应功能时使用")
                        .size(10)
                        .font(fonts::REGULAR)
                        .color(INK_MUTED)
                ]
                .spacing(2)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            space::horizontal(),
            action_button("检测全部", Icon::ScanSearch, SettingsAction::DetectAll),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([12, 14])
    .style(environment_overview_style);
    let rows = section_rows(vec![
        environment_row(
            Icon::PackageOpen,
            "Homebrew",
            "macOS 下的环境安装工具。",
            true,
            SettingsAction::ManageHomebrew,
        ),
        environment_row(
            Icon::GitBranch,
            "Git",
            "用于管理酒馆版本与下载酒馆。",
            true,
            SettingsAction::ManageGit,
        ),
        environment_row(
            Icon::CodeXml,
            "Node.js",
            "用于运行酒馆。",
            true,
            SettingsAction::ManageNode,
        ),
        setting_row(
            Icon::Globe,
            "NPM 源设置",
            "设置 NPM 下载软件包时使用的镜像源。",
            select_control(
                &NpmRegistry::ALL,
                state.npm_registry,
                Message::SettingsNpmRegistrySelected,
            ),
        ),
        environment_row(
            Icon::ShieldCheck,
            "Caddy",
            "用于给酒馆添加反向代理。",
            false,
            SettingsAction::ManageCaddy,
        ),
        environment_row(
            Icon::CloudCog,
            "PM2",
            "让酒馆脱离启动器在后台运行。",
            false,
            SettingsAction::ManagePm2,
        ),
    ]);
    section(
        Icon::PackageOpen,
        "环境依赖",
        "检查、安装并管理酒馆运行所需的本机工具。",
        column![overview, rows].spacing(12).into(),
    )
}

fn github_settings(state: &SettingsState) -> Element<'_, Message> {
    section(
        Icon::GitFork,
        "GitHub 设置",
        "使用资源替换节点加速 GitHub 文件与仓库访问。",
        section_rows(vec![
            setting_row(
                Icon::Rocket,
                "GitHub 资源加速",
                "开启后在 GitHub 源地址前添加加速节点地址。",
                toggle_control(
                    state.github_proxy_enabled,
                    Message::SettingsGithubProxyEnabled,
                ),
            ),
            setting_row(
                Icon::Network,
                "加速节点地址",
                "输入当前使用的 GitHub 资源替换节点。",
                input_control(
                    "https://example.com/",
                    &state.github_proxy_url,
                    Message::SettingsGithubProxyUrlChanged,
                ),
            ),
            setting_row(
                Icon::RefreshCw,
                "替换节点列表",
                "获取可用节点并测试延迟与下载速度。",
                action_button(
                    "刷新节点",
                    Icon::RefreshCw,
                    SettingsAction::RefreshGithubNodes,
                ),
            ),
        ]),
    )
}

fn network_settings(state: &SettingsState) -> Element<'_, Message> {
    section(
        Icon::Cable,
        "网络设置",
        "设置应用程序网络代理并测试 GitHub 连通性。",
        section_rows(vec![
            setting_row(
                Icon::Shield,
                "代理设置",
                "选择直连、跟随系统代理或使用自定义代理。",
                select_control(
                    &ProxyMode::ALL,
                    state.proxy_mode,
                    Message::SettingsProxyModeSelected,
                ),
            ),
            setting_row(
                Icon::Link,
                "自定义代理地址",
                "输入 HTTP、HTTPS 或 SOCKS 代理地址。",
                input_control(
                    "http://127.0.0.1:7890",
                    &state.custom_proxy,
                    Message::SettingsCustomProxyChanged,
                ),
            ),
            setting_row(
                Icon::Activity,
                "GitHub 连接测试",
                "测试首页、仓库、API、文件访问与下载速度。",
                action_button("开始测试", Icon::Activity, SettingsAction::TestGithub),
            ),
        ]),
    )
}

fn software_settings() -> Element<'static, Message> {
    section(
        Icon::Info,
        "软件与更新",
        "查看当前版本并检查启动器更新。",
        section_rows(vec![
            setting_row(
                Icon::AppWindow,
                "AstraBrew Launcher",
                "当前版本 0.2.0",
                chip("当前版本", None, BLUE_600, ChipVariant::Flat),
            ),
            setting_row(
                Icon::Download,
                "检查更新",
                "检查是否有新的启动器版本可用。",
                action_button("检查更新", Icon::RefreshCw, SettingsAction::CheckUpdate),
            ),
        ]),
    )
}

fn section<'a>(
    icon: Icon,
    title: &'static str,
    description: &'static str,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    column![
        row![
            container(icons::icon(icon, 17, BLUE_600))
                .width(32)
                .height(32)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(setting_icon_style),
            column![
                text(title).size(17).font(fonts::MEDIUM),
                text(description)
                    .size(11)
                    .font(fonts::REGULAR)
                    .color(INK_MUTED)
            ]
            .spacing(3)
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        Card::new(content).width(Fill).padding(0)
    ]
    .spacing(11)
    .width(Fill)
    .into()
}

fn section_rows<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut content = column![];
    let count = rows.len();
    for (index, item) in rows.into_iter().enumerate() {
        content = content.push(item);
        if index + 1 < count {
            content = content.push(Separator::new().variant(SeparatorVariant::Tertiary));
        }
    }
    content.into()
}

fn setting_row<'a>(
    icon: Icon,
    title: &'static str,
    description: &'static str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        setting_icon(icon),
        column![
            text(title).size(13).font(fonts::MEDIUM).color(INK),
            text(description)
                .size(11)
                .font(fonts::REGULAR)
                .color(INK_MUTED)
        ]
        .spacing(3)
        .width(Fill),
        control
    ]
    .spacing(12)
    .padding([14, 16])
    .align_y(Alignment::Center)
    .width(Fill)
    .into()
}

fn environment_row(
    icon: Icon,
    title: &'static str,
    description: &'static str,
    required: bool,
    action: SettingsAction,
) -> Element<'static, Message> {
    let requirement = if required {
        chip("必装", None, WARNING, ChipVariant::Flat)
    } else {
        chip("可选", None, INK_SUBTLE, ChipVariant::Flat)
    };
    row![
        setting_icon(icon),
        column![
            row![
                text(title).size(13).font(fonts::MEDIUM).color(INK),
                requirement
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text(description)
                .size(11)
                .font(fonts::REGULAR)
                .color(INK_MUTED)
        ]
        .spacing(4)
        .width(Fill),
        row![
            icons::icon(Icon::Circle, 13, INK_SUBTLE),
            text("未检测")
                .size(11)
                .font(fonts::REGULAR)
                .color(INK_MUTED)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        action_button("管理", Icon::ChevronRight, action)
    ]
    .spacing(12)
    .padding([13, 16])
    .align_y(Alignment::Center)
    .width(Fill)
    .into()
}

fn setting_icon(icon: Icon) -> Element<'static, Message> {
    container(icons::icon(icon, 17, BLUE_600))
        .width(34)
        .height(34)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(setting_icon_style)
        .into()
}

fn select_control<'a, T: Copy + Eq + fmt::Display + 'a>(
    options: &'a [T],
    selected: T,
    on_selected: fn(T) -> Message,
) -> Element<'a, Message> {
    pick_list(options, Some(selected), on_selected)
        .width(SELECT_WIDTH)
        .padding([8, 11])
        .text_size(12)
        .font(fonts::REGULAR)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style)
        .into()
}

fn input_control<'a>(
    placeholder: &'static str,
    value: &'a str,
    on_input: fn(String) -> Message,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .width(INPUT_WIDTH)
        .padding([8, 11])
        .size(12)
        .style(text_input_style)
        .into()
}

fn toggle_control(is_toggled: bool, on_toggle: fn(bool) -> Message) -> Element<'static, Message> {
    switch(
        "",
        is_toggled,
        if is_toggled { 1.0 } else { 0.0 },
        on_toggle,
    )
}

fn path_control(path: &str, action: SettingsAction) -> Element<'_, Message> {
    row![
        container(text(path).size(10).font(fonts::REGULAR).color(INK_MUTED)).width(190),
        action_button("更改", Icon::FolderOpen, action)
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn action_button(
    label: &'static str,
    icon: Icon,
    action: SettingsAction,
) -> Element<'static, Message> {
    button(
        row![
            icons::icon(icon, 14, INK_MUTED),
            text(label).size(11).font(fonts::MEDIUM)
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SettingsAction(action))
    .height(34)
    .padding([7, 11])
    .style(button_style(ButtonVariant::Secondary))
    .into()
}

fn setting_icon_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.09,
        ))),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn environment_overview_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            WARNING.r, WARNING.g, WARNING.b, 0.08,
        ))),
        border: Border {
            color: Color::from_rgba(WARNING.r, WARNING.g, WARNING.b, 0.22),
            width: 1.0,
            radius: RADIUS_FIELD.into(),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{SettingsState, StartMode, TavernDataMode};
    #[test]
    fn defaults_match_old_launcher_preferences() {
        let settings = SettingsState::default();
        assert_eq!(settings.start_mode, StartMode::Normal);
        assert_eq!(settings.data_mode, TavernDataMode::Current);
        assert!(settings.remember_window_position);
        assert!(settings.auto_stop_tavern_on_window_close);
    }
}
