//! 单页设置视图：沿用旧版设置清单，使用 Astra UI 重新排版。

use std::fmt;

use iced::widget::{
    button, column, combo_box, container, mouse_area, row, rule, scrollable, slider, space, stack,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Font, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{
    AlertKind, BLUE_600, ButtonVariant, INK_MUTED, ProgressBar, ProgressBarColor,
    ProgressBarSize, ProgressCircle, ProgressCircleColor, ProgressCircleSize,
    ToggleButtonGroupItem, icons,
};

use super::{themed_segmented_group, themed_segmented_group_enabled};
use crate::app::Message;
use crate::core::network::{DownloadChannel, DownloadChannelTestResult};
use crate::core::typography::{
    DEFAULT_UI_SCALE, FontChoice, MAX_UI_SCALE, MIN_UI_SCALE, normalize_ui_scale,
};
pub use crate::core::settings::{DisplayLanguage, ThemeMode};
use crate::lang::lang::current_language;
use crate::lang::{t, text};
use crate::theme::{button_style, pick_list_menu_style, slider_style, text_input_style};

const INPUT_WIDTH: f32 = 250.0;

/// 设置页主滚动区域使用稳定标识，便于从全局安装引导定位到环境依赖区域。
pub(crate) fn settings_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("settings-main-scroll")
}

macro_rules! enum_text {
    ($ty:ident, $([$variant:ident, $label:literal]),+ $(,)?) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&crate::lang::display_label(match self { $(Self::$variant => $label,)+ }))
            }
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CpuCores {
    #[default]
    Auto,
    Half,
    All,
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
pub enum ServerServiceMode {
    #[default]
    Lan,
    Internet,
}
impl ServerServiceMode {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Lan => "lan",
            Self::Internet => "internet",
        }
    }

    pub fn from_key(value: &str) -> Self {
        if value.eq_ignore_ascii_case("internet") {
            Self::Internet
        } else {
            Self::Lan
        }
    }
}
enum_text!(ServerServiceMode, [Lan, "局域网"], [Internet, "互联网"]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TavernDataMode {
    Global,
    #[default]
    Current,
}
enum_text!(TavernDataMode, [Global, "全局数据"], [Current, "独立数据"]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NpmRegistry {
    Official,
    #[default]
    Npmmirror,
    Tencent,
    HuaweiCloud,
}
impl NpmRegistry {
    pub fn from_url(url: &str) -> Self {
        match url.trim_end_matches('/') {
            "https://registry.npmjs.org" => Self::Official,
            "https://registry.npmmirror.com" => Self::Npmmirror,
            "https://mirrors.cloud.tencent.com/npm" => Self::Tencent,
            "https://repo.huaweicloud.com/repository/npm" => Self::HuaweiCloud,
            _ => Self::Npmmirror,
        }
    }

    pub const fn url(self) -> &'static str {
        match self {
            Self::Official => "https://registry.npmjs.org/",
            Self::Npmmirror => "https://registry.npmmirror.com/",
            Self::Tencent => "https://mirrors.cloud.tencent.com/npm/",
            Self::HuaweiCloud => "https://repo.huaweicloud.com/repository/npm/",
        }
    }
}
enum_text!(
    NpmRegistry,
    [Official, "NPM 官方源"],
    [Npmmirror, "npmmirror"],
    [Tencent, "腾讯云镜像"],
    [HuaweiCloud, "华为云镜像"]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProxyMode {
    #[default]
    None,
    System,
    Custom,
}
impl fmt::Display for ProxyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::None => "代理关闭",
            Self::System => "跟随系统",
            Self::Custom => "自定义代理",
        };
        f.write_str(&crate::lang::display_label(label))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    OpenLoginItemSettings,
    ChooseExportPath,
    ChooseGlobalDataPath,
    RefreshDownloadChannel,
    TestGithub,
    CheckUpdate,
}
impl SettingsAction {
    pub const fn feedback(self) -> &'static str {
        match self {
            Self::OpenLoginItemSettings => "已打开系统登录项设置。",
            Self::ChooseExportPath => "已更新酒馆资源保存位置。",
            Self::ChooseGlobalDataPath => "已更新全局数据存放位置。",
            Self::RefreshDownloadChannel => "下载渠道测速已刷新。",
            Self::TestGithub => "GitHub 连通性测试服务待接入。",
            Self::CheckUpdate => "启动器更新检查服务待接入。",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentDependency {
    Homebrew,
    Git,
    NodeJs,
    Caddy,
    Pm2,
}

impl EnvironmentDependency {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::Git => "Git",
            Self::NodeJs => "Node.js",
            Self::Caddy => "Caddy",
            Self::Pm2 => "PM2",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvironmentVersions {
    pub homebrew: Option<String>,
    pub git: Option<String>,
    pub nodejs: Option<String>,
    pub caddy: Option<String>,
    pub pm2: Option<String>,
}

impl EnvironmentVersions {
    #[cfg(not(test))]
    pub fn detect_all() -> Self {
        use crate::core::settings::env_detect;
        Self {
            homebrew: env_detect::detect_homebrew(),
            git: env_detect::detect_git(),
            nodejs: env_detect::detect_nodejs(),
            caddy: env_detect::detect_caddy(),
            pm2: env_detect::detect_pm2(),
        }
    }

    pub fn set(&mut self, dependency: EnvironmentDependency, version: String) {
        let slot = match dependency {
            EnvironmentDependency::Homebrew => &mut self.homebrew,
            EnvironmentDependency::Git => &mut self.git,
            EnvironmentDependency::NodeJs => &mut self.nodejs,
            EnvironmentDependency::Caddy => &mut self.caddy,
            EnvironmentDependency::Pm2 => &mut self.pm2,
        };
        *slot = Some(version);
    }
}

#[derive(Debug, Clone, Default)]
pub struct EnvironmentTaskState {
    pub dependency: Option<EnvironmentDependency>,
    pub show: bool,
    pub log: String,
    pub running: bool,
    pub done_at: Option<std::time::Instant>,
    pub started_at: Option<std::time::Instant>,
    pub timed_out: bool,
    pub failed: bool,
    /// 安装日志默认收起，用户需要时再展开查看完整详情。
    pub show_details: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemProxyStatus {
    Enabled,
    Disabled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default)]
pub struct GithubTestState {
    pub show: bool,
    pub running: bool,
    pub timed_out: bool,
    pub results: Option<Vec<crate::core::network::GithubMultiTestItem>>,
    pub error: Option<String>,
    pub mode_label: String,
    pub proxy_address: Option<String>,
    pub accelerate_url: Option<String>,
    pub started_at: Option<std::time::Instant>,
    pub current_key: Option<String>,
    pub current_name: Option<String>,
    pub clone_stage: Option<String>,
    pub clone_current: Option<u64>,
    pub clone_total: Option<u64>,
    pub clone_percentage: Option<f32>,
    pub download_total_bytes: Option<u64>,
    pub download_downloaded_bytes: u64,
    pub download_bytes_per_second: u64,
    pub download_percentage: Option<f32>,
    pub live_items: Vec<GithubLiveItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubLiveItemStatus {
    Running,
    Finished,
}

#[derive(Debug, Clone)]
pub struct GithubLiveItem {
    pub key: String,
    pub name: String,
    pub status: GithubLiveItemStatus,
    pub result: Option<crate::core::network::GithubMultiTestItem>,
}

#[derive(Debug, Clone, Default)]
pub struct DownloadChannelTestState {
    pub show: bool,
    pub running: bool,
    pub timed_out: bool,
    pub all_failed: bool,
    pub started_at: Option<std::time::Instant>,
    pub done_at: Option<std::time::Instant>,
    pub current_channel: Option<DownloadChannel>,
    pub clone_stage: Option<String>,
    pub clone_current: Option<u64>,
    pub clone_total: Option<u64>,
    pub clone_percentage: Option<f32>,
    pub results: Vec<DownloadChannelTestResult>,
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub language: DisplayLanguage,
    pub theme: ThemeMode,
    /// 普通界面文字、控件和间距的缩放比例。
    pub ui_scale: f32,
    /// 当前已经成功应用的字体设置键。
    pub font_family: String,
    /// 可搜索字体选择器的本地状态。
    pub font_choices: combo_box::State<FontChoice>,
    /// 字体选择器当前展示的选项。
    pub selected_font: FontChoice,
    /// 新字体文件是否正在交给渲染器加载。
    pub font_loading: bool,
    /// 字体发现或动态加载失败时展示的错误。
    pub appearance_error: Option<String>,
    pub remember_window_position: bool,
    pub auto_start: bool,
    pub cpu_cores: CpuCores,
    pub start_mode: StartMode,
    pub auto_stop_tavern_on_window_close: bool,
    pub tavern_export_path: String,
    pub server_mode_enabled: bool,
    pub server_service_mode: ServerServiceMode,
    pub allow_tavern_background: bool,
    pub data_mode: TavernDataMode,
    pub global_data_path: String,
    pub show_startup_command: bool,
    pub npm_registry: NpmRegistry,
    /// 兼容旧版配置的隐藏字段，新版下载设置不再展示它们。
    pub github_proxy_enabled: bool,
    pub github_proxy_url: String,
    pub download_channel: DownloadChannel,
    pub download_resolved_channel: Option<DownloadChannel>,
    pub download_channel_last_tested: Option<u64>,
    pub download_channel_test: DownloadChannelTestState,
    pub proxy_mode: ProxyMode,
    pub custom_proxy: String,
    pub system_proxy_status: SystemProxyStatus,
    pub environment: EnvironmentVersions,
    pub environment_task: EnvironmentTaskState,
    pub github_test: GithubTestState,
    pub last_action: Option<SettingsAction>,
    /// 最近一次偏好设置保存失败的错误信息。
    pub save_error: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            language: DisplayLanguage::System,
            theme: ThemeMode::System,
            ui_scale: DEFAULT_UI_SCALE,
            font_family: crate::core::typography::DEFAULT_FONT_KEY.to_owned(),
            font_choices: combo_box::State::with_selection(
                vec![FontChoice::default_choice()],
                Some(&FontChoice::default_choice()),
            ),
            selected_font: FontChoice::default_choice(),
            font_loading: false,
            appearance_error: None,
            remember_window_position: true,
            auto_start: false,
            cpu_cores: CpuCores::Auto,
            start_mode: StartMode::Normal,
            auto_stop_tavern_on_window_close: true,
            tavern_export_path: "~/Downloads".into(),
            server_mode_enabled: false,
            server_service_mode: ServerServiceMode::Lan,
            allow_tavern_background: false,
            data_mode: TavernDataMode::Current,
            global_data_path: crate::core::settings::DEFAULT_GLOBAL_DATA_PATH.into(),
            show_startup_command: false,
            npm_registry: NpmRegistry::Npmmirror,
            github_proxy_enabled: false,
            github_proxy_url: "https://gh-proxy.org/".into(),
            download_channel: DownloadChannel::Auto,
            download_resolved_channel: None,
            download_channel_last_tested: None,
            download_channel_test: DownloadChannelTestState::default(),
            proxy_mode: ProxyMode::System,
            custom_proxy: String::new(),
            system_proxy_status: SystemProxyStatus::Unknown,
            environment: EnvironmentVersions::default(),
            environment_task: EnvironmentTaskState::default(),
            github_test: GithubTestState::default(),
            last_action: None,
            save_error: None,
        }
    }
}

impl SettingsState {
    /// 使用当前机器字体目录配置可搜索选项并解析保存值。
    pub(crate) fn configure_fonts(
        &mut self,
        catalog: &crate::core::typography::SystemFontCatalog,
    ) -> FontChoice {
        let selected = catalog.resolve(&self.font_family);
        self.font_family = selected.key().to_owned();
        self.selected_font = selected;
        self.font_choices = combo_box::State::with_selection(
            catalog.choices(),
            Some(&selected),
        );
        selected
    }

    /// 让字体选择器回到已经生效的字体。
    pub(crate) fn select_font(&mut self, selected: FontChoice) {
        self.selected_font = selected;
        self.font_choices = combo_box::State::with_selection(
            self.font_choices.options().to_vec(),
            Some(&selected),
        );
    }

    /// 判断自动下载渠道缓存是否仍在七天有效期内。
    pub fn download_channel_cache_valid(&self) -> bool {
        let Some(resolved) = self.download_resolved_channel else {
            return false;
        };
        if resolved == DownloadChannel::Auto {
            return false;
        }
        let Some(tested_at) = self.download_channel_last_tested else {
            return false;
        };
        let Some(now) = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs())
        else {
            return false;
        };
        now.saturating_sub(tested_at) < 7 * 24 * 60 * 60
    }

    /// 将磁盘偏好应用到设置页状态.
    pub fn apply_persistent_preferences(
        &mut self,
        preferences: &crate::core::settings::PersistentPreferences,
    ) {
        self.language = preferences.language;
        self.theme = preferences.theme;
        self.ui_scale = normalize_ui_scale(preferences.ui_scale);
        self.font_family = preferences.font_family.clone();
        self.remember_window_position = preferences.remember_window_position;
        self.data_mode = match preferences.data_mode.as_str() {
            "global" => TavernDataMode::Global,
            _ => TavernDataMode::Current,
        };
        self.global_data_path = preferences.global_data_path.clone();
        self.tavern_export_path = preferences.tavern_export_path.clone();
        self.start_mode = match preferences.start_mode.as_str() {
            "desktop" => StartMode::Desktop,
            _ => StartMode::Normal,
        };
        self.server_mode_enabled = preferences.server_mode_enabled;
        self.server_service_mode = ServerServiceMode::from_key(&preferences.server_service_mode);
        self.auto_stop_tavern_on_window_close = preferences.auto_stop_tavern_on_window_close;
        self.allow_tavern_background = preferences.allow_tavern_background;
        self.show_startup_command = preferences.show_startup_command;
        if self.server_mode_enabled {
            self.start_mode = StartMode::Normal;
        }
        self.proxy_mode = match preferences.proxy_mode.as_str() {
            "none" => ProxyMode::None,
            "custom" => ProxyMode::Custom,
            _ => ProxyMode::System,
        };
        self.custom_proxy = preferences.custom_proxy.clone();
        self.download_channel = DownloadChannel::from_key(&preferences.download_channel);
        self.npm_registry = NpmRegistry::from_url(&preferences.npm_registry);
        self.github_proxy_enabled = preferences.github_proxy_enabled;
        self.github_proxy_url = preferences.github_proxy_url.clone();
    }
}

pub fn settings_view(state: &SettingsState, mode_controls_locked: bool) -> Element<'_, Message> {
    let header = row![
        column![
            text("设置").size(24).font(crate::core::typography::medium()),
            text("管理启动器外观、酒馆运行方式、环境依赖与网络连接")
                .size(12)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style)
        ]
        .spacing(4),
        space::horizontal(),
        button(
            row![
                crate::theme::muted_icon(Icon::RotateCcw, 15),
                text("恢复默认").size(12).font(crate::core::typography::medium())
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
        basic_settings(state, mode_controls_locked),
        console_settings(state),
        environment_settings(state),
        download_settings(state),
        network_settings(state),
        software_settings(),
    ]
    .spacing(22)
    .width(Fill);
    if let Some(error) = &state.appearance_error {
        sections = sections.push(crate::theme::alert(
            t("settings.interface.font.error_title", current_language()),
            error,
            AlertKind::Danger,
        ));
    }
    if let Some(error) = &state.save_error {
        sections = sections.push(crate::theme::alert(
            "设置未能保存",
            error,
            AlertKind::Danger,
        ));
    }

    let mut page: Element<'_, Message> = container(
        column![
            header,
            scrollable(container(sections).padding([0, 24]))
                .id(settings_scroll_id())
                .width(Fill)
                .height(Fill)
        ]
        .spacing(20),
    )
    .width(Fill)
    .height(Fill)
    .padding([24, 28])
    .style(crate::theme::canvas_style)
    .into();

    if state.environment_task.show {
        page = stack![page, environment_task_modal(&state.environment_task)]
            .width(Fill)
            .height(Fill)
            .into();
    }
    if state.github_test.show {
        page = stack![page, github_test_modal(&state.github_test)]
            .width(Fill)
            .height(Fill)
            .into();
    }
    if state.download_channel_test.show {
        page = stack![page, download_channel_test_modal(state)]
            .width(Fill)
            .height(Fill)
            .into();
    }
    page
}

/// 本地实例缺少 Node.js 时显示的全局安装引导。
///
/// 安装按钮由应用根层处理，以便同时完成页面导航和复用设置页的安装动作。
pub(crate) fn nodejs_required_modal() -> Element<'static, Message> {
    let language = current_language();
    let panel = mouse_area(
        container(
            column![
                row![
                    container(icons::icon(
                        Icon::CodeXml,
                        22,
                        iced::Color::from_rgb8(88, 80, 236),
                    ))
                    .width(42)
                    .height(42)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(environment_log_style),
                    column![
                        text(t("environment.nodejs_required.title", language))
                            .size(18)
                            .font(crate::core::typography::medium()),
                        text(t("environment.nodejs_required.description", language))
                            .size(12)
                            .font(crate::core::typography::regular())
                            .style(crate::theme::muted_text_style),
                    ]
                    .spacing(5)
                    .width(Fill),
                ]
                .spacing(14)
                .align_y(Alignment::Center),
                crate::theme::alert(
                    t("environment.nodejs_required.title", language),
                    t("environment.nodejs_required.action_hint", language),
                    AlertKind::Info,
                ),
                row![
                    space::horizontal(),
                    button(
                        text(t("environment.nodejs_required.later", language))
                            .size(12)
                            .font(crate::core::typography::medium())
                    )
                    .on_press(Message::DismissNodeJsRequired)
                    .height(36)
                    .padding([8, 14])
                    .style(button_style(ButtonVariant::Secondary)),
                    button(
                        row![
                            icons::icon(Icon::Download, 15, iced::Color::WHITE),
                            text(t("environment.nodejs_required.install", language))
                                .size(12)
                                .font(crate::core::typography::medium())
                                .color(iced::Color::WHITE),
                        ]
                        .spacing(7)
                        .align_y(Alignment::Center)
                    )
                    .on_press(Message::InstallRequiredNodeJs)
                    .height(36)
                    .padding([8, 15])
                    .style(button_style(ButtonVariant::Primary)),
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .width(Fill),
            ]
            .spacing(18),
        )
        .width(Fill).max_width(500)
        .padding(22)
        .style(environment_modal_style),
    )
    .on_press(Message::NodeJsRequiredInteract);

    stack![
        button(space::Space::new())
            .on_press(Message::NodeJsRequiredInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(environment_backdrop_style),
        container(panel)
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

fn download_channel_test_modal(state: &SettingsState) -> Element<'_, Message> {
    let test = &state.download_channel_test;
    let phase = test
        .started_at
        .map(|started| (started.elapsed().as_secs_f32() * 0.9).fract())
        .unwrap_or(0.0);
    let channels = DownloadChannel::fixed_channels();
    let rows = channels
        .into_iter()
        .map(|channel| {
            let result = test.results.iter().find(|result| result.channel == channel);
            let is_current = test.current_channel == Some(channel) && result.is_none();
            let (icon, detail, indicator): (
                Element<'static, Message>,
                Element<'static, Message>,
                Element<'static, Message>,
            ) = if let Some(result) = result {
                let detail = if result.success {
                    result
                        .latency_ms
                        .map(|latency| format!("测速成功 · {latency} ms"))
                        .unwrap_or_else(|| "测速成功".to_owned())
                } else {
                    result
                        .error
                        .clone()
                        .unwrap_or_else(|| "测速失败".to_owned())
                };
                let icon = if result.success {
                    icons::icon(Icon::CircleCheck, 16, iced::Color::from_rgb8(23, 201, 100))
                } else {
                    icons::icon(Icon::CircleX, 16, iced::Color::from_rgb8(255, 56, 60))
                };
                (
                    icon,
                    text(detail)
                        .size(10)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style)
                        .into(),
                    text("").into(),
                )
            } else if is_current {
                let stage = test
                    .clone_stage
                    .as_deref()
                    .map(crate::lang::github_clone_stage_label)
                    .unwrap_or_else(|| "准备克隆".to_owned());
                let percent = test
                    .clone_percentage
                    .map(|value| format!("{value:.0}%"))
                    .unwrap_or_else(|| "进行中".to_owned());
                let detail = format!("{stage} {percent}");
                let counts = match (test.clone_current, test.clone_total) {
                    (Some(current), Some(total)) => Some(format!("{current} / {total} 个对象")),
                    _ => None,
                };
                let detail = if let Some(counts) = counts {
                    format!("{detail} · {counts}")
                } else {
                    detail
                };
                let indicator: Element<'static, Message> = match test.clone_percentage {
                    Some(value) => ProgressCircle::new(value)
                        .color(ProgressCircleColor::Accent)
                        .into(),
                    None => ProgressCircle::new(0.0)
                        .is_indeterminate(true)
                        .animation_phase(phase)
                        .color(ProgressCircleColor::Accent)
                        .into(),
                };
                (
                    crate::theme::subtle_icon(Icon::Circle, 13),
                    text(detail)
                        .size(10)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style)
                        .into(),
                    indicator,
                )
            } else {
                (
                    crate::theme::subtle_icon(Icon::Circle, 13),
                    text("等待测试…").into(),
                    text("").into(),
                )
            };
            row![
                icon,
                column![text(channel.label()).size(12).font(crate::core::typography::medium()), detail]
                    .spacing(3)
                    .width(Fill),
                indicator,
            ]
            .spacing(10)
            .padding([9, 10])
            .align_y(Alignment::Center)
            .into()
        })
        .collect::<Vec<_>>();

    let status: Element<'_, Message> = if test.running {
        row![
            ProgressCircle::new(0.0)
                .is_indeterminate(true)
                .animation_phase(phase)
                .color(ProgressCircleColor::Accent),
            text("正在测速下载渠道…").size(12).font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else if test.timed_out {
        row![
            icons::icon(Icon::ClockAlert, 16, iced::Color::from_rgb8(245, 165, 36)),
            text("渠道测速超时，请稍后重试。")
                .size(12)
                .font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else if test.all_failed {
        row![
            icons::icon(Icon::CircleX, 16, iced::Color::from_rgb8(255, 56, 60)),
            text("所有渠道测速失败，已回退到官方渠道。")
                .size(12)
                .font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            icons::icon(Icon::CircleCheck, 16, iced::Color::from_rgb8(23, 201, 100)),
            text(format!(
                "最快渠道：{}",
                state
                    .download_resolved_channel
                    .or(test.current_channel)
                    .unwrap_or(DownloadChannel::Official)
                    .label()
            ))
            .size(12)
            .font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };

    let footer = row![
        status,
        space::horizontal(),
        button(
            text(if test.running { "取消" } else { "关闭" })
                .size(12)
                .font(crate::core::typography::medium())
        )
        .on_press(Message::DownloadChannelTestClose)
        .height(34)
        .padding([7, 14])
        .style(button_style(ButtonVariant::Secondary)),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Fill);

    let panel = mouse_area(
        container(
            column![
                text("酒馆下载渠道测速").size(18).font(crate::core::typography::medium()),
                rule::horizontal(1.0).style(crate::theme::separator_style),
                scrollable(
                    container(column(rows).spacing(8))
                        .padding(12)
                        .style(environment_log_style)
                )
                .height(if crate::core::typography::current_ui_scale() >= 1.35 {
                    210
                } else {
                    300
                }),
                rule::horizontal(1.0).style(crate::theme::separator_style),
                footer,
            ]
            .spacing(16),
        )
        .width(Fill).max_width(620)
        .padding(20)
        .style(environment_modal_style),
    )
    .on_press(Message::DownloadChannelTestInteract);

    stack![
        button(space::Space::new())
            .on_press(Message::DownloadChannelTestInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(environment_backdrop_style),
        container(panel)
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

fn environment_task_modal(task: &EnvironmentTaskState) -> Element<'_, Message> {
    let dependency = task.dependency.unwrap_or(EnvironmentDependency::Git);
    let language = current_language();
    let elapsed = task
        .started_at
        .map(|started| started.elapsed().as_secs())
        .unwrap_or_default();
    let phase = task
        .started_at
        .map(|started| (started.elapsed().as_secs_f32() * 0.9).fract())
        .unwrap_or(0.0);

    let (status_key, description_key, progress, progress_color) = if task.running {
        (
            "environment.install.running",
            "environment.install.executing",
            0.0,
            ProgressBarColor::Accent,
        )
    } else if task.timed_out {
        (
            "environment.install.timed_out",
            "environment.install.timeout_description",
            100.0,
            ProgressBarColor::Warning,
        )
    } else if task.failed {
        (
            "environment.install.failed",
            "environment.install.not_completed",
            100.0,
            ProgressBarColor::Danger,
        )
    } else {
        (
            "environment.install.success",
            "environment.install.ready",
            100.0,
            ProgressBarColor::Success,
        )
    };

    let status_icon: Element<'_, Message> = if task.running {
        container(
            ProgressCircle::new(0.0)
                .is_indeterminate(true)
                .animation_phase(phase)
                .size(ProgressCircleSize::Small)
                .color(ProgressCircleColor::Accent),
        )
        .width(28)
        .height(28)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(environment_task_icon_style)
        .into()
    } else {
        let (icon, color) = if task.timed_out {
            (Icon::ClockAlert, iced::Color::from_rgb8(245, 165, 36))
        } else if task.failed {
            (Icon::CircleX, iced::Color::from_rgb8(255, 56, 60))
        } else {
            (Icon::CircleCheck, iced::Color::from_rgb8(23, 201, 100))
        };
        container(icons::icon(icon, 17, color))
            .width(28)
            .height(28)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(environment_task_icon_style)
            .into()
    };

    let status_title = format!("{} {}", t(status_key, language), dependency.name());
    let elapsed_label = format!(
        "{} {}s",
        t("environment.install.elapsed", language),
        elapsed
    );
    let latest_line = task
        .log
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| truncate_environment_log(line, 78))
        .unwrap_or_else(|| t("environment.install.waiting", language).to_owned());

    let action_label = if task.running {
        t("environment.install.cancel", language)
    } else {
        t("environment.install.close", language)
    };
    let details_label = if task.show_details {
        t("environment.install.hide_details", language)
    } else {
        t("environment.install.show_details", language)
    };

    let header = row![
        status_icon,
        column![
            text(status_title).size(14).font(crate::core::typography::medium()),
            text(format!(
                "{}  ·  {}",
                t(description_key, language),
                elapsed_label
            ))
            .size(10)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style),
        ]
        .spacing(2)
        .width(Fill),
        button(text(action_label).size(10).font(crate::core::typography::medium()))
            .on_press(Message::EnvironmentTaskClose)
            .height(28)
            .padding([5, 10])
            .style(button_style(ButtonVariant::Secondary)),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Fill);

    let summary = row![
        icons::icon(
            if task.failed || task.timed_out {
                Icon::CircleAlert
            } else {
                Icon::CircleCheck
            },
            13,
            if task.failed {
                iced::Color::from_rgb8(255, 56, 60)
            } else if task.timed_out {
                iced::Color::from_rgb8(245, 165, 36)
            } else {
                INK_MUTED
            },
        ),
        text(latest_line)
            .size(10)
            .font(Font::MONOSPACE)
            .style(crate::theme::muted_text_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let details_toggle = button(
        row![
            crate::theme::muted_icon(
                if task.show_details {
                    Icon::ChevronUp
                } else {
                    Icon::ChevronDown
                },
                14,
            ),
            text(details_label).size(10).font(crate::core::typography::medium()),
            space::horizontal(),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
        .width(Fill),
    )
    .on_press(Message::EnvironmentTaskToggleDetails)
    .width(Fill)
    .padding([6, 0])
    .style(button_style(ButtonVariant::Ghost));

    let mut content = column![
        header,
        ProgressBar::new(progress)
            .show_value(false)
            // brew/npm 不提供可信的整体百分比，安装期间统一使用不确定进度。
            .is_indeterminate(task.running)
            .animation_phase(phase)
            .size(ProgressBarSize::Small)
            .color(progress_color),
    ]
    .spacing(8)
    .width(Fill);

    if task.running {
        content = content.push(
            text(t("environment.install.progress_unknown", language))
                .size(9)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
        );
    }
    content = content.push(summary).push(details_toggle);

    if task.show_details {
        let log = if task.log.is_empty() {
            t("environment.install.waiting", language)
        } else {
            task.log.as_str()
        };
        content = content.push(
            scrollable(
                container(
                    text(log)
                        .size(10)
                        .font(Font::MONOSPACE)
                        .style(crate::theme::text_style),
                )
                .width(Fill)
                .padding(12)
                .style(environment_log_style),
            )
            .height(if crate::core::typography::current_ui_scale() >= 1.35 {
                120
            } else {
                180
            }),
        );
    }

    let panel = mouse_area(
        container(content)
            .width(Fill).max_width(500)
            .padding(12)
            .style(environment_modal_style),
    )
    .on_press(Message::EnvironmentModalInteract);

    stack![
        button(space::Space::new())
            .on_press(Message::EnvironmentModalInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(environment_backdrop_style),
        container(panel)
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

/// 安装摘要只展示最近一行，并在固定宽度内保留省略号。
fn truncate_environment_log(line: &str, max_chars: usize) -> String {
    let clean = line.trim();
    if clean.chars().count() <= max_chars {
        return clean.to_owned();
    }
    let mut result: String = clean.chars().take(max_chars.saturating_sub(1)).collect();
    result.push('…');
    result
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
                segmented_control(
                    &[
                        (DisplayLanguage::System, "跟随系统", Icon::Monitor),
                        (
                            DisplayLanguage::SimplifiedChinese,
                            "简体中文",
                            Icon::Languages,
                        ),
                        (DisplayLanguage::English, "English", Icon::Languages),
                    ],
                    state.language,
                    Message::SettingsLanguageSelected,
                ),
            ),
            setting_row(
                Icon::SunMoon,
                "主题",
                "选择浅色、深色或跟随系统外观。",
                segmented_control(
                    &[
                        (ThemeMode::System, "跟随系统", Icon::Monitor),
                        (ThemeMode::Light, "浅色", Icon::Sun),
                        (ThemeMode::Dark, "深色", Icon::Moon),
                    ],
                    state.theme,
                    Message::SettingsThemeSelected,
                ),
            ),
            setting_row(
                Icon::Scaling,
                t("settings.interface.scale.title", current_language()),
                t("settings.interface.scale.description", current_language()),
                ui_scale_control(state.ui_scale),
            ),
            setting_row(
                Icon::Type,
                t("settings.interface.font.title", current_language()),
                t("settings.interface.font.description", current_language()),
                font_control(state),
            ),
            setting_row(
                Icon::PanelsTopLeft,
                "记住上次窗口位置",
                "启动时恢复上次窗口的位置。",
                toggle_control(
                    state.remember_window_position,
                    Message::SettingsRememberWindowPosition,
                ),
            ),
        ]),
    )
}

fn basic_settings(state: &SettingsState, mode_controls_locked: bool) -> Element<'_, Message> {
    let launch_mode_control: Element<'_, Message> = if state.server_mode_enabled {
        readonly_text("服务器模式")
    } else {
        start_mode_control(state.start_mode, !mode_controls_locked)
    };

    let mut rows = vec![
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
            segmented_control(
                &[
                    (CpuCores::Auto, "自动", Icon::Gauge),
                    (CpuCores::Half, "一半核心", Icon::CircleGauge),
                    (CpuCores::All, "全部核心", Icon::Cpu),
                ],
                state.cpu_cores,
                Message::SettingsCpuCoresSelected,
            ),
        ),
        setting_row(
            Icon::Play,
            "酒馆启动模式",
            "正常模式直接使用浏览器；桌面模式使用内置 WebView；服务器模式下固定为服务器模式。",
            launch_mode_control,
        ),
    ];

    // WebView 相关设置仅在桌面模式下显示。
    if !state.server_mode_enabled && state.start_mode == StartMode::Desktop {
        rows.extend([
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
                "酒馆资源保存",
                "桌面模式下酒馆页面导出资源的默认保存位置。",
                path_control(&state.tavern_export_path, SettingsAction::ChooseExportPath),
            ),
        ]);
    }

    rows.push(setting_row(
        Icon::Server,
        "启用服务器模式",
        "把此设备作为仅运行酒馆服务的服务器。",
        if mode_controls_locked {
            readonly_text(if state.server_mode_enabled { "已启用" } else { "未启用" })
        } else {
            toggle_control(state.server_mode_enabled, Message::SettingsServerMode)
        },
    ));

    // 酒馆服务模式仅在服务器模式启用时显示。
    if state.server_mode_enabled {
        rows.push(setting_row(
            Icon::Globe,
            "酒馆服务模式",
            "选择只向局域网开放，或向互联网开放。",
            segmented_control_enabled(
                &[
                    (ServerServiceMode::Lan, "局域网", Icon::Wifi),
                    (ServerServiceMode::Internet, "互联网", Icon::Earth),
                ],
                state.server_service_mode,
                !mode_controls_locked,
                Message::SettingsServerServiceModeSelected,
            ),
        ));
    }

    if state.server_mode_enabled {
        let pm2_available = state.environment.pm2.is_some();
        rows.push(setting_row(
            Icon::CloudCog,
            "允许酒馆后台运行",
            "关闭启动器后继续运行酒馆服务，需要 PM2。",
            if mode_controls_locked || !pm2_available {
                readonly_text(if state.allow_tavern_background { "已启用" } else { "未启用" })
            } else {
                toggle_control(
                    state.allow_tavern_background,
                    Message::SettingsAllowTavernBackground,
                )
            },
        ));
        if state.server_service_mode == ServerServiceMode::Internet {
            rows.push(setting_row(
                Icon::Waypoints,
                "反向代理",
                "互联网服务模式使用的域名、端口与证书功能。",
                readonly_text("待开发"),
            ));
        }
    }

    rows.extend([
        setting_row(
            Icon::Database,
            "酒馆数据模式",
            "全局模式共用数据；独立模式让各酒馆使用自己的数据。",
            segmented_control(
                &[
                    (TavernDataMode::Global, "全局数据", Icon::Database),
                    (TavernDataMode::Current, "独立数据", Icon::HardDrive),
                ],
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
    ]);

    section(
        Icon::SlidersHorizontal,
        "基本设置",
        "配置启动行为、酒馆运行模式与数据存放方式。",
        section_rows(rows),
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
    use crate::core::settings::env_detect;

    let brew_installed = state.environment.homebrew.is_some();
    let nodejs_installed = state.environment.nodejs.is_some();
    let homebrew_outdated = state
        .environment
        .homebrew
        .as_deref()
        .is_some_and(env_detect::is_homebrew_outdated);
    let nodejs_outdated = state
        .environment
        .nodejs
        .as_deref()
        .is_some_and(env_detect::is_nodejs_outdated);

    let homebrew_title = dependency_title("Homebrew", homebrew_outdated);
    let nodejs_title = dependency_title("Node.js", nodejs_outdated);
    let caddy_description = if state.server_mode_enabled {
        "用于给酒馆添加反向代理（必装）。"
    } else {
        "用于给酒馆添加反向代理（可选）。"
    };
    let pm2_description = if state.server_mode_enabled {
        "让酒馆脱离启动器在后台运行（必装，需要 Node.js）。"
    } else {
        "让酒馆脱离启动器在后台运行（可选，需要 Node.js）。"
    };

    let rows = section_rows(vec![
        environment_dependency_row(
            Icon::Beer,
            homebrew_title,
            "macOS 的软件包管理器（必装）。",
            environment_version_or_action(
                EnvironmentDependency::Homebrew,
                state.environment.homebrew.clone(),
                false,
                true,
            ),
        ),
        environment_dependency_row(
            Icon::GitBranch,
            "Git".to_owned(),
            "用于管理酒馆版本与下载酒馆（必装）。",
            environment_version_or_action(
                EnvironmentDependency::Git,
                state.environment.git.clone(),
                false,
                brew_installed,
            ),
        ),
        environment_dependency_row(
            Icon::CodeXml,
            nodejs_title,
            "用于运行酒馆（必装）。",
            environment_version_or_action(
                EnvironmentDependency::NodeJs,
                state.environment.nodejs.clone(),
                nodejs_outdated,
                brew_installed,
            ),
        ),
        npm_registry_setting(state),
        environment_dependency_row(
            Icon::ShieldCheck,
            "Caddy".to_owned(),
            caddy_description,
            environment_version_or_action(
                EnvironmentDependency::Caddy,
                state.environment.caddy.clone(),
                false,
                brew_installed,
            ),
        ),
        environment_dependency_row(
            Icon::CloudDownload,
            "PM2".to_owned(),
            pm2_description,
            environment_version_or_action(
                EnvironmentDependency::Pm2,
                state.environment.pm2.clone(),
                false,
                nodejs_installed,
            ),
        ),
    ]);

    section(
        Icon::PackageOpen,
        "环境依赖",
        "检查、安装并管理酒馆运行所需的本机工具。",
        rows,
    )
}

fn npm_registry_setting(state: &SettingsState) -> Element<'_, Message> {
    row![
        setting_icon(Icon::Globe),
        column![
            text("NPM 源设置")
                .size(13)
                .font(crate::core::typography::medium())
                .style(crate::theme::text_style),
            text("设置 NPM 下载软件包时使用的镜像源。")
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
            row![
                text("源 URL：")
                    .size(10)
                    .font(crate::core::typography::regular())
                    .style(crate::theme::muted_text_style),
                text(state.npm_registry.url())
                    .size(10)
                    .font(crate::core::typography::regular())
                    .style(crate::theme::text_style),
            ]
            .spacing(3)
            .align_y(Alignment::Center),
        ]
        .spacing(4)
        .width(Fill),
        segmented_control(
            &[
                (NpmRegistry::Npmmirror, "npmmirror", Icon::Package),
                (NpmRegistry::HuaweiCloud, "华为云", Icon::Cloud),
                (NpmRegistry::Tencent, "腾讯云", Icon::CloudCog),
                (NpmRegistry::Official, "官方源", Icon::Boxes),
            ],
            state.npm_registry,
            Message::SettingsNpmRegistrySelected,
        ),
    ]
    .spacing(12)
    .padding([13, 16])
    .align_y(Alignment::Center)
    .width(Fill)
    .into()
}

fn dependency_title(name: &str, outdated: bool) -> String {
    if outdated {
        format!("{name}  ⚠ 版本过低")
    } else {
        name.to_owned()
    }
}

fn environment_version_or_action(
    dependency: EnvironmentDependency,
    version: Option<String>,
    outdated: bool,
    enabled: bool,
) -> Element<'static, Message> {
    match version {
        Some(_) if outdated => environment_action_button("更新", dependency, enabled),
        Some(version) => text(version)
            .size(13)
            .font(crate::core::typography::medium())
            .style(crate::theme::text_style)
            .into(),
        None => environment_action_button("安装", dependency, enabled),
    }
}

fn environment_action_button(
    label: &'static str,
    dependency: EnvironmentDependency,
    enabled: bool,
) -> Element<'static, Message> {
    let content = row![
        crate::theme::muted_icon(
            if label == "更新" {
                Icon::ArrowUp
            } else {
                Icon::Download
            },
            14,
        ),
        text(label).size(11).font(crate::core::typography::medium()),
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    let mut control = button(content)
        .height(34)
        .padding([7, 11])
        .style(button_style(ButtonVariant::Secondary));
    if enabled {
        control = control.on_press(Message::EnvironmentInstall(dependency));
    }
    control.into()
}

fn download_settings(state: &SettingsState) -> Element<'_, Message> {
    let selected_label = state
        .download_channel
        .display_label(state.download_resolved_channel);
    let descriptions = column(
        DownloadChannel::fixed_channels()
            .into_iter()
            .map(|channel| {
                row![
                    text(format!("{}：", channel.label()))
                        .size(10)
                        .font(crate::core::typography::medium())
                        .style(crate::theme::muted_text_style),
                    text(channel.description())
                        .size(10)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style),
                ]
                .spacing(3)
                .into()
            })
            .collect::<Vec<_>>(),
    )
    .spacing(3);

    let channel_values = [
        DownloadChannel::Auto,
        DownloadChannel::Mirror1,
        DownloadChannel::Mirror2,
        DownloadChannel::Mirror3,
        DownloadChannel::Official,
    ];
    let channel_icons = [
        Icon::Gauge,
        Icon::Cloud,
        Icon::CloudCog,
        Icon::CloudDownload,
        Icon::GitBranch,
    ];
    let channel_items = channel_values
        .into_iter()
        .zip(channel_icons)
        .map(|(channel, icon)| {
            ToggleButtonGroupItem::new(
                Some(download_channel_toggle_label(
                    channel,
                    state.download_resolved_channel,
                    state.language,
                )),
                Some(icon),
                channel == state.download_channel,
            )
        })
        .collect();
    let channel_control = themed_segmented_group(channel_items, move |index| {
        Message::SettingsDownloadChannelSelected(channel_values[index])
    });

    section(
        Icon::CloudDownload,
        "下载设置",
        "选择酒馆核心下载、安装与更新时使用的仓库渠道。",
        section_rows(vec![
            row![
                setting_icon(Icon::Download),
                column![
                    text("酒馆下载渠道")
                        .size(13)
                        .font(crate::core::typography::medium())
                        .style(crate::theme::text_style),
                    text(selected_label)
                        .size(11)
                        .font(crate::core::typography::medium())
                        .style(crate::theme::muted_text_style),
                    descriptions,
                ]
                .spacing(4)
                .width(Fill),
                channel_control,
            ]
            .spacing(12)
            .padding([13, 16])
            .align_y(Alignment::Center)
            .width(Fill)
            .into(),
            setting_row(
                Icon::RefreshCw,
                "自动测速缓存",
                "测速结果缓存 7 天；缓存有效期内不会重复测速。",
                row![
                    text(if state.download_channel_test.running {
                        "测速中…"
                    } else if state.download_channel == DownloadChannel::Auto
                        && state.download_channel_cache_valid()
                    {
                        "缓存有效"
                    } else {
                        "尚未测速"
                    })
                    .size(11)
                    .font(crate::core::typography::medium())
                    .style(crate::theme::muted_text_style),
                    action_button(
                        "重新测速",
                        Icon::RefreshCw,
                        SettingsAction::RefreshDownloadChannel,
                    ),
                ]
                .spacing(10)
                .align_y(Alignment::Center)
                .into(),
            ),
        ]),
    )
}

fn network_settings(state: &SettingsState) -> Element<'_, Message> {
    let mut rows = vec![proxy_setting(state)];
    if state.proxy_mode == ProxyMode::Custom {
        rows.push(setting_row(
            Icon::Link,
            "自定义代理地址",
            "输入 HTTP、HTTPS 或 SOCKS 代理地址。",
            input_control(
                "http://127.0.0.1:7890",
                &state.custom_proxy,
                Message::SettingsCustomProxyChanged,
            ),
        ));
    }
    rows.push(github_test_setting(state));

    section(
        Icon::Cable,
        "网络设置",
        "设置应用程序网络代理并测试 GitHub 连通性。",
        section_rows(rows),
    )
}

fn proxy_setting(state: &SettingsState) -> Element<'_, Message> {
    let mut description = column![
        text("选择直连、跟随系统代理或使用自定义代理。")
            .size(11)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style),
    ]
    .spacing(4)
    .width(Fill);

    if state.proxy_mode == ProxyMode::System {
        let status = match state.system_proxy_status {
            SystemProxyStatus::Enabled => "系统代理：已启用",
            SystemProxyStatus::Disabled => "系统代理：未启用",
            SystemProxyStatus::Unknown => "系统代理：未知",
        };
        description = description.push(
            row![
                crate::theme::subtle_icon(Icon::Info, 13),
                text(status)
                    .size(10)
                    .font(crate::core::typography::regular())
                    .style(crate::theme::muted_text_style),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
        );
    }

    row![
        setting_icon(Icon::Shield),
        description,
        segmented_control(
            &[
                (ProxyMode::None, "直连", Icon::Cable),
                (ProxyMode::System, "跟随系统", Icon::Monitor),
                (ProxyMode::Custom, "自定义", Icon::Link),
            ],
            state.proxy_mode,
            Message::SettingsProxyModeSelected,
        ),
    ]
    .spacing(12)
    .padding([14, 16])
    .align_y(Alignment::Center)
    .width(Fill)
    .into()
}

fn github_test_setting(state: &SettingsState) -> Element<'_, Message> {
    let control: Element<'_, Message> = if state.github_test.running {
        let phase = state
            .github_test
            .started_at
            .map(|started| (started.elapsed().as_secs_f32() * 0.9).fract())
            .unwrap_or(0.0);
        button(
            row![
                ProgressCircle::new(0.0)
                    .is_indeterminate(true)
                    .animation_phase(phase)
                    .color(ProgressCircleColor::Accent),
                text("测试中…").size(11).font(crate::core::typography::medium()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .height(34)
        .padding([7, 11])
        .style(button_style(ButtonVariant::Secondary))
        .into()
    } else {
        action_button("开始测试", Icon::Activity, SettingsAction::TestGithub)
    };

    setting_row(
        Icon::Plug,
        "GitHub 连接测试",
        "测试首页、仓库、API、文件访问与下载速度。",
        control,
    )
}

fn github_test_modal(state: &GithubTestState) -> Element<'_, Message> {
    let phase = state
        .started_at
        .map(|started| (started.elapsed().as_secs_f32() * 0.9).fract())
        .unwrap_or(0.0);
    let mode = if state.mode_label.is_empty() {
        "直连"
    } else {
        state.mode_label.as_str()
    };
    let mut details = column![
        row![
            text("测试模式").size(11).font(crate::core::typography::medium()),
            space::horizontal(),
            text(mode).size(11).font(crate::core::typography::regular()),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(7)
    .width(Fill);
    if let Some(proxy) = &state.proxy_address {
        details = details.push(
            row![
                text("代理地址").size(11).font(crate::core::typography::medium()),
                space::horizontal(),
                text(proxy).size(10).font(crate::core::typography::regular()),
            ]
            .align_y(Alignment::Center),
        );
    }
    if let Some(accelerate) = &state.accelerate_url {
        details = details.push(
            row![
                text("加速地址").size(11).font(crate::core::typography::medium()),
                space::horizontal(),
                text(accelerate).size(10).font(crate::core::typography::regular()),
            ]
            .align_y(Alignment::Center),
        );
    }

    let body: Element<'_, Message> = if state.running {
        let rows = state
            .live_items
            .iter()
            .map(|item| live_github_result_row(item, state, phase))
            .collect::<Vec<_>>();
        column(rows).spacing(8).into()
    } else if let Some(results) = &state.results {
        column(
            results
                .iter()
                .map(|item| github_result_row(item, Some(state)))
                .collect::<Vec<_>>(),
        )
        .spacing(8)
        .into()
    } else {
        crate::theme::alert(
            "测试失败",
            state
                .error
                .as_deref()
                .unwrap_or("GitHub 测试进程未能完成。"),
            AlertKind::Danger,
        )
    };

    let status: Element<'_, Message> = if state.running {
        row![
            ProgressCircle::new(0.0)
                .is_indeterminate(true)
                .animation_phase(phase)
                .color(ProgressCircleColor::Accent),
            text("正在测试 GitHub 连接…").size(12).font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else if state.timed_out {
        row![
            icons::icon(Icon::ClockAlert, 16, iced::Color::from_rgb8(245, 165, 36)),
            text("测试超时，请检查网络或代理设置后重试。")
                .size(12)
                .font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else if state.error.is_some() {
        row![
            icons::icon(Icon::CircleX, 16, iced::Color::from_rgb8(255, 56, 60)),
            text("测试失败，请查看详情后重试。")
                .size(12)
                .font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            icons::icon(Icon::CircleCheck, 16, iced::Color::from_rgb8(23, 201, 100)),
            text("测试完成").size(12).font(crate::core::typography::medium()),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };

    let footer = row![
        status,
        space::horizontal(),
        button(text("关闭").size(12).font(crate::core::typography::medium()))
            .on_press(Message::GithubTestClose)
            .height(34)
            .padding([7, 14])
            .style(button_style(ButtonVariant::Secondary)),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Fill);

    let panel = mouse_area(
        container(
            column![
                text("GitHub 连接测试").size(18).font(crate::core::typography::medium()),
                rule::horizontal(1.0).style(crate::theme::separator_style),
                details,
                rule::horizontal(1.0).style(crate::theme::separator_style),
                scrollable(
                    container(body)
                        .width(Fill)
                        .padding(12)
                        .style(environment_log_style)
                )
                .height(if crate::core::typography::current_ui_scale() >= 1.35 {
                    210
                } else {
                    300
                }),
                rule::horizontal(1.0).style(crate::theme::separator_style),
                footer,
            ]
            .spacing(16),
        )
        .width(Fill).max_width(620)
        .padding(20)
        .style(environment_modal_style),
    )
    .on_press(Message::GithubTestInteract);

    stack![
        button(space::Space::new())
            .on_press(Message::GithubTestInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(environment_backdrop_style),
        container(panel)
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

fn live_github_result_row(
    item: &GithubLiveItem,
    state: &GithubTestState,
    phase: f32,
) -> Element<'static, Message> {
    if let Some(result) = &item.result {
        return github_result_row(result, Some(state));
    }

    let detail_text = |value: String| {
        text(value)
            .size(10)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style)
    };

    let (progress, details): (Element<'static, Message>, Element<'static, Message>) =
        if item.key == "clone" {
            let stage = state
                .clone_stage
                .as_deref()
                .map(crate::lang::github_clone_stage_label)
                .unwrap_or_else(|| crate::lang::github_clone_preparing_label().to_owned());
            let percentage = state
                .clone_percentage
                .map(|value| format!("{value:.0}%"))
                .unwrap_or_else(|| crate::lang::github_clone_in_progress_label().to_owned());
            let counts = match (state.clone_current, state.clone_total) {
                (Some(current), Some(total)) => {
                    Some(crate::lang::github_clone_objects_label(current, total))
                }
                _ => None,
            };
            let detail_lines = if let Some(counts) = counts {
                column![
                    detail_text(format!("{stage} {percentage}")),
                    detail_text(counts)
                ]
            } else {
                column![detail_text(format!("{stage} {percentage}"))]
            };
            let indicator: Element<'static, Message> = match state.clone_percentage {
                Some(value) => ProgressCircle::new(value)
                    .color(ProgressCircleColor::Accent)
                    .into(),
                None => ProgressCircle::new(0.0)
                    .is_indeterminate(true)
                    .animation_phase(phase)
                    .color(ProgressCircleColor::Accent)
                    .into(),
            };
            (
                indicator,
                detail_lines.spacing(2).align_x(Alignment::End).into(),
            )
        } else if item.key == "speed" {
            let downloaded = format_bytes(state.download_downloaded_bytes);
            let total = state
                .download_total_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "总大小未知".to_owned());
            let speed = format_speed(state.download_bytes_per_second);
            let percentage = state
                .download_percentage
                .map(|value| format!("{value:.1}%"))
                .unwrap_or_else(|| "—".to_owned());
            let indicator: Element<'static, Message> = match state.download_percentage {
                Some(value) => ProgressCircle::new(value)
                    .color(ProgressCircleColor::Accent)
                    .into(),
                None => ProgressCircle::new(0.0)
                    .is_indeterminate(true)
                    .animation_phase(phase)
                    .color(ProgressCircleColor::Accent)
                    .into(),
            };
            (
                indicator,
                column![
                    detail_text(format!("{downloaded} / {total}")),
                    detail_text(speed),
                    detail_text(percentage),
                ]
                .spacing(2)
                .align_x(Alignment::End)
                .into(),
            )
        } else {
            (
                ProgressCircle::new(0.0)
                    .is_indeterminate(true)
                    .animation_phase(phase)
                    .color(ProgressCircleColor::Accent)
                    .into(),
                detail_text("测试中…".to_owned()).into(),
            )
        };

    row![
        crate::theme::subtle_icon(Icon::Circle, 13),
        text(item.name.clone())
            .size(12)
            .font(crate::core::typography::medium())
            .width(Fill),
        row![progress, details]
            .spacing(8)
            .align_y(Alignment::Center)
            .width(Length::Shrink),
    ]
    .spacing(10)
    .padding([9, 10])
    .align_y(Alignment::Center)
    .into()
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.2} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn format_speed(bytes_per_second: u64) -> String {
    if bytes_per_second == 0 {
        "速度计算中".to_owned()
    } else {
        format!("{}/s", format_bytes(bytes_per_second))
    }
}

fn github_result_row(
    item: &crate::core::network::GithubMultiTestItem,
    state: Option<&GithubTestState>,
) -> Element<'static, Message> {
    let (icon, color) = if item.success {
        (Icon::CircleCheck, iced::Color::from_rgb8(23, 201, 100))
    } else {
        (Icon::CircleX, iced::Color::from_rgb8(255, 56, 60))
    };
    let detail = if item.key == "speed" && item.success {
        if let Some(state) = state {
            let total = state
                .download_total_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "总大小未知".to_owned());
            let summary = format!(
                "{} / {} · 平均 {}",
                format_bytes(state.download_downloaded_bytes),
                total,
                format_speed(state.download_bytes_per_second)
            );
            item.warning
                .as_ref()
                .map(|warning| format!("{summary} · {warning}"))
                .unwrap_or(summary)
        } else {
            item.warning
                .clone()
                .unwrap_or_else(|| "下载完成".to_owned())
        }
    } else {
        item.warning
            .clone()
            .or_else(|| item.error.clone())
            .unwrap_or_else(|| match (item.key.as_str(), state) {
                ("clone", Some(_)) if item.success => "克隆完成".to_owned(),
                _ => "连接成功".to_owned(),
            })
    };
    let latency = item
        .latency_ms
        .map(|value| format!("耗时 {value} ms"))
        .unwrap_or_default();

    row![
        icons::icon(icon, 16, color),
        column![
            text(item.name.clone()).size(12).font(crate::core::typography::medium()),
            text(detail)
                .size(10)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
        ]
        .spacing(3)
        .width(Fill),
        text(latency)
            .size(11)
            .font(crate::core::typography::medium())
            .style(crate::theme::muted_text_style),
    ]
    .spacing(10)
    .padding([9, 10])
    .align_y(Alignment::Center)
    .into()
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
                crate::theme::flat_chip("当前版本", BLUE_600),
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
                text(title).size(17).font(crate::core::typography::medium()),
                text(description)
                    .size(11)
                    .font(crate::core::typography::regular())
                    .style(crate::theme::muted_text_style)
            ]
            .spacing(3)
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        crate::theme::card(content, Fill, 0)
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
            content = content.push(rule::horizontal(1.0).style(crate::theme::separator_style));
        }
    }
    content.into()
}

fn setting_row<'a>(
    icon: Icon,
    title: &'a str,
    description: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let label = column![
        text(title)
            .size(13)
            .font(crate::core::typography::medium())
            .style(crate::theme::text_style),
        text(description)
            .size(11)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style)
    ]
    .spacing(3)
    .width(Fill);

    if crate::core::typography::current_ui_scale() >= 1.35 {
        column![
            row![setting_icon(icon), label]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Fill),
            container(control).width(Fill).padding(iced::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 46.0,
            }),
        ]
        .spacing(10)
        .padding([14, 16])
        .width(Fill)
        .into()
    } else {
        row![setting_icon(icon), label, control]
            .spacing(12)
            .padding([14, 16])
            .align_y(Alignment::Center)
            .width(Fill)
            .into()
    }
}

fn environment_dependency_row<'a>(
    icon: Icon,
    title: String,
    description: &'a str,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        setting_icon(icon),
        column![
            text(title)
                .size(13)
                .font(crate::core::typography::medium())
                .style(crate::theme::text_style),
            text(description)
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style)
        ]
        .spacing(4)
        .width(Fill),
        control,
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

fn start_mode_control(selected: StartMode, enabled: bool) -> Element<'static, Message> {
    let items = [
        (StartMode::Normal, "正常模式", Icon::Play),
        (StartMode::Desktop, "桌面模式", Icon::AppWindow),
    ]
    .into_iter()
    .map(|(mode, label, icon)| {
        ToggleButtonGroupItem::new(Some(label), Some(icon), mode == selected)
    })
    .collect();

    themed_segmented_group_enabled(items, enabled, |index| {
        Message::SettingsLaunchModeSelected(match index {
            1 => QuickStartMode::Desktop,
            _ => QuickStartMode::Normal,
        })
    })
}

fn download_channel_toggle_label(
    channel: DownloadChannel,
    resolved: Option<DownloadChannel>,
    language: DisplayLanguage,
) -> &'static str {
    let english = crate::lang::effective_language(language) == crate::lang::Language::English;
    match (channel, resolved, english) {
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror1), false) => "自动（镜像 1）",
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror2), false) => "自动（镜像 2）",
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror3), false) => "自动（镜像 3）",
        (DownloadChannel::Auto, Some(DownloadChannel::Official), false) => "自动（官方）",
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror1), true) => "Automatic (Mirror 1)",
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror2), true) => "Automatic (Mirror 2)",
        (DownloadChannel::Auto, Some(DownloadChannel::Mirror3), true) => "Automatic (Mirror 3)",
        (DownloadChannel::Auto, Some(DownloadChannel::Official), true) => "Automatic (Official)",
        (DownloadChannel::Auto, _, false) => "自动",
        (DownloadChannel::Auto, _, true) => "Automatic",
        (DownloadChannel::Mirror1, _, false) => "镜像 1",
        (DownloadChannel::Mirror1, _, true) => "Mirror 1",
        (DownloadChannel::Mirror2, _, false) => "镜像 2",
        (DownloadChannel::Mirror2, _, true) => "Mirror 2",
        (DownloadChannel::Mirror3, _, false) => "镜像 3",
        (DownloadChannel::Mirror3, _, true) => "Mirror 3",
        (DownloadChannel::Official, _, false) => "官方",
        (DownloadChannel::Official, _, true) => "Official",
    }
}

fn segmented_control<T>(
    options: &[(T, &'static str, Icon)],
    selected: T,
    on_selected: fn(T) -> Message,
) -> Element<'static, Message>
where
    T: Copy + Eq + 'static,
{
    segmented_control_enabled(options, selected, true, on_selected)
}

fn segmented_control_enabled<T>(
    options: &[(T, &'static str, Icon)],
    selected: T,
    enabled: bool,
    on_selected: fn(T) -> Message,
) -> Element<'static, Message>
where
    T: Copy + Eq + 'static,
{
    let values = options
        .iter()
        .map(|(value, _, _)| *value)
        .collect::<Vec<_>>();
    let items = options
        .iter()
        .map(|(value, label, icon)| {
            ToggleButtonGroupItem::new(Some(*label), Some(*icon), *value == selected)
        })
        .collect();

    themed_segmented_group_enabled(items, enabled, move |index| on_selected(values[index]))
}

fn ui_scale_control(value: f32) -> Element<'static, Message> {
    let percent = (normalize_ui_scale(value) * 100.0).round();
    row![
        slider(
            MIN_UI_SCALE * 100.0..=MAX_UI_SCALE * 100.0,
            percent,
            |value| Message::SettingsUiScaleChanged(value / 100.0),
        )
        .step(5.0)
        .width(210)
        .style(slider_style),
        container(
            text(format!("{percent:.0}%"))
                .size(12)
                .font(crate::core::typography::medium())
        )
        .width(48)
        .align_x(Alignment::End),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn font_control(state: &SettingsState) -> Element<'_, Message> {
    let picker = combo_box(
        &state.font_choices,
        t("settings.interface.font.placeholder", current_language()),
        Some(&state.selected_font),
        Message::SettingsFontSelected,
    )
    .width(280)
    .padding([8, 11])
    .size(12)
    .font(crate::core::typography::regular())
    .input_style(text_input_style)
    .menu_style(pick_list_menu_style);

    if state.font_loading {
        column![
            picker,
            text(t(
                "settings.interface.font.loading",
                current_language(),
            ))
            .size(10)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style),
        ]
        .spacing(4)
        .into()
    } else {
        picker.into()
    }
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
        .font(crate::core::typography::regular())
        .style(text_input_style)
        .into()
}

fn toggle_control(is_toggled: bool, on_toggle: fn(bool) -> Message) -> Element<'static, Message> {
    crate::theme::switch("", is_toggled, on_toggle)
}

fn readonly_text(label: &'static str) -> Element<'static, Message> {
    text(label)
        .size(11)
        .font(crate::core::typography::medium())
        .style(crate::theme::muted_text_style)
        .into()
}

fn path_control(path: &str, action: SettingsAction) -> Element<'_, Message> {
    row![
        container(
            text(path)
                .size(10)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style)
        )
        .width(190),
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
            crate::theme::muted_icon(icon, 14),
            text(label).size(11).font(crate::core::typography::medium())
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

fn setting_icon_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) {
                0.18
            } else {
                0.09
            },
        ))),
        border: Border {
            radius: 9.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn environment_modal_style(theme: &Theme) -> container::Style {
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

fn environment_task_icon_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) {
                0.24
            } else {
                0.12
            },
        ))),
        border: Border {
            radius: 14.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn environment_log_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(if crate::theme::is_dark(theme) {
            Color::from_rgb8(20, 20, 23)
        } else {
            Color::from_rgb8(247, 247, 248)
        })),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

fn environment_backdrop_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.48))),
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_UI_SCALE, DownloadChannelTestState, NpmRegistry, SettingsState, StartMode,
        TavernDataMode, download_channel_toggle_label,
    };
    use crate::core::network::DownloadChannel;
    use crate::core::settings::DisplayLanguage;
    #[test]
    fn npm_registry_urls_match_the_old_launcher() {
        assert_eq!(NpmRegistry::Official.url(), "https://registry.npmjs.org/");
        assert_eq!(
            NpmRegistry::Npmmirror.url(),
            "https://registry.npmmirror.com/"
        );
        assert_eq!(
            NpmRegistry::Tencent.url(),
            "https://mirrors.cloud.tencent.com/npm/"
        );
        assert_eq!(
            NpmRegistry::HuaweiCloud.url(),
            "https://repo.huaweicloud.com/repository/npm/"
        );
    }

    #[test]
    fn automatic_channel_label_includes_resolved_channel() {
        assert_eq!(
            download_channel_toggle_label(
                DownloadChannel::Auto,
                Some(DownloadChannel::Mirror1),
                DisplayLanguage::SimplifiedChinese,
            ),
            "自动（镜像 1）"
        );
        assert_eq!(
            download_channel_toggle_label(
                DownloadChannel::Auto,
                Some(DownloadChannel::Official),
                DisplayLanguage::English,
            ),
            "Automatic (Official)"
        );
    }

    #[test]
    fn automatic_channel_cache_requires_recent_result() {
        let mut settings = SettingsState::default();
        settings.download_resolved_channel = Some(DownloadChannel::Mirror2);
        settings.download_channel_last_tested = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_secs(),
        );
        assert!(settings.download_channel_cache_valid());
        settings.download_channel_last_tested =
            Some(settings.download_channel_last_tested.unwrap() - 7 * 24 * 60 * 60);
        assert!(!settings.download_channel_cache_valid());
        let _ = DownloadChannelTestState::default();
    }

    #[test]
    fn defaults_match_old_launcher_preferences() {
        let settings = SettingsState::default();
        assert_eq!(settings.start_mode, StartMode::Normal);
        assert_eq!(settings.data_mode, TavernDataMode::Current);
        assert!(settings.remember_window_position);
        assert!(settings.auto_stop_tavern_on_window_close);
        assert!((settings.ui_scale - DEFAULT_UI_SCALE).abs() < f32::EPSILON);
        assert_eq!(
            settings.font_family,
            crate::core::typography::DEFAULT_FONT_KEY
        );
    }
}
