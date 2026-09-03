//! AstraBrew Launcher 应用根模块。
//!
//! 负责在「初始化流程」与「主界面」两个屏幕之间路由。首次启动引导用户完成
//! 运行环境初始化，完成后进入主界面（左侧导航栏 + 右侧内容区）。
//! 界面组件统一来自 astra_ui（Astra UI）组件库。

use iced::time::{self, Duration, Instant};
use iced::widget::{button, column, container, row, space};
use iced::{Alignment, Element, Fill, Point, Size, Subscription, Task, Theme, theme, window};
use lucide_icons::Icon;

use astra_ui::fonts;
use astra_ui::icons;
use astra_ui::{
    AlertKind, Avatar, AvatarColor, AvatarShape, AvatarSize, ButtonVariant, CYAN_500,
    ChipVariant, INK_SUBTLE, ProgressBar, ProgressBarColor, SUCCESS, WHITE,
    chip, tag_style,
};

use crate::core::settings::{PersistentPreferences, SettingsStore};
use crate::lang::{effective_language, t, text};
use crate::theme::button_style;
use crate::pages::Page;
use crate::pages::console::{ConsoleMessage, ConsoleState};
use crate::pages::extensions::{ExtensionsMessage, ExtensionsState};
use crate::pages::resource_manage::{ResourceManageMessage, ResourceManageState};
use crate::pages::settings::{
    CpuCores, DisplayLanguage, NpmRegistry, ProxyMode, QuickStartMode, ServerServiceMode,
    SettingsAction, SettingsState, StartMode, TavernDataMode, TavernVersion, ThemeMode,
};
use crate::pages::tavern::{BrowserType, TavernMessage, TavernState};
use crate::pages::versions::{VersionMessage, VersionState};
use crate::{pages, sidebar};

/// 应用屏幕：初始化流程 / 主界面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// 首次运行的环境初始化流程
    #[allow(dead_code)]
    Init,
    /// 主界面（左侧导航栏 + 右侧内容区）
    Main,
}

/// 初始化阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitStage {
    /// 欢迎页，尚未开始初始化
    Welcome,
    /// 初始化进行中
    Initializing,
    /// 初始化完成
    Complete,
}

/// 单条初始化步骤
struct InitStep {
    /// 步骤名称
    name: &'static str,
    /// 步骤对应的目录或文件说明
    path: &'static str,
    /// 完成该步骤所需达到的总进度（百分比）
    threshold: f32,
}

/// 初始化步骤清单（按执行顺序排列）
const INIT_STEPS: [InitStep; 4] = [
    InitStep {
        name: "数据目录",
        path: "~/Library/Application Support/AstraBrew Launcher",
        threshold: 25.0,
    },
    InitStep {
        name: "缓存目录",
        path: "~/Library/Caches/AstraBrew Launcher",
        threshold: 50.0,
    },
    InitStep {
        name: "日志目录",
        path: "~/Library/Logs/AstraBrew Launcher",
        threshold: 75.0,
    },
    InitStep {
        name: "核心文件",
        path: "SillyTavern 核心运行文件",
        threshold: 100.0,
    },
];

/// 单条步骤的展示状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StepStatus {
    /// 尚未开始
    Pending,
    /// 正在执行
    Running,
    /// 已完成
    Done,
}

/// 窗口固定尺寸档位。
struct WindowProfile {
    /// 当前显示器对应的固定窗口尺寸。
    default_size: Size,
}

/// 依据窗口所在显示器的逻辑分辨率宽高比推断窗口尺寸档位。
///
/// 宽高比 ≥ 1.5 视为宽屏（含 16:9 与 MacBook 的 16:10），采用 16:9 档位；
/// 否则按 4:3 档位处理。无法获取显示器尺寸时默认按宽屏处理（最常用情况）。
fn window_profile(monitor: Option<Size>) -> WindowProfile {
    let widescreen = monitor
        .map(|size| size.width / size.height.max(1.0) >= 1.5)
        .unwrap_or(true);

    if widescreen {
        WindowProfile {
            default_size: Size::new(1280.0, 720.0),
        }
    } else {
        WindowProfile {
            default_size: Size::new(1280.0, 800.0),
        }
    }
}

/// 应用消息
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// 开始初始化
    StartInitialization,
    /// 取消 / 重置初始化
    CancelInitialization,
    /// 初始化完成后进入主界面
    FinishInitialization,
    /// 切换主界面左侧导航栏的当前页面
    Navigate(Page),
    /// 请求一键启动酒馆
    LaunchTavern,
    /// 主页快捷切换酒馆版本
    HomeTavernVersionSelected(TavernVersion),
    /// 主页快捷切换启动模式
    HomeStartModeSelected(QuickStartMode),
    /// 主页普通模式快捷切换浏览器
    HomeBrowserSelected(BrowserType),
    /// 修改界面语言
    SettingsLanguageSelected(DisplayLanguage),
    /// 修改界面主题
    SettingsThemeSelected(ThemeMode),
    /// 切换是否记住窗口位置
    SettingsRememberWindowPosition(bool),
    SettingsAutoStart(bool),
    SettingsCpuCoresSelected(CpuCores),
    SettingsStartModeSelected(StartMode),
    SettingsAutoStopTavern(bool),
    SettingsServerMode(bool),
    SettingsServerServiceModeSelected(ServerServiceMode),
    SettingsAllowTavernBackground(bool),
    SettingsDataModeSelected(TavernDataMode),
    SettingsShowStartupCommand(bool),
    /// 修改 NPM 软件源
    SettingsNpmRegistrySelected(NpmRegistry),
    SettingsGithubProxyEnabled(bool),
    SettingsGithubProxyUrlChanged(String),
    /// 修改网络代理模式
    SettingsProxyModeSelected(ProxyMode),
    SettingsCustomProxyChanged(String),
    /// 触发尚待服务层接入的设置操作
    SettingsAction(SettingsAction),
    /// 恢复设置页默认值
    SettingsRestoreDefaults,
    /// 更新酒馆配置页的本地配置草稿
    Tavern(TavernMessage),
    /// 更新版本管理页的本地界面状态
    Version(VersionMessage),
    /// 更新扩展管理页的本地界面状态
    Extensions(ExtensionsMessage),
    /// 更新资源管理页状态并执行本地文件操作
    Resources(ResourceManageMessage),
    /// 更新控制台页面状态
    Console(ConsoleMessage),
    /// 主窗口已打开，记录初始坐标并校准固定尺寸。
    WindowOpened(window::Id, Option<Point>),
    /// 已测得窗口所在显示器的逻辑分辨率。
    /// `apply_default` 为 true 表示首开，需要把窗口调整为档位默认尺寸；
    /// 为 false 表示跨屏拖动，仅更新尺寸约束、不强制改变当前窗口大小。
    MonitorMeasured(window::Id, Option<Size>, bool),
    /// 窗口缩放因子变化（跨屏拖动到不同 DPI / 比例的显示器）
    WindowRescaled(window::Id),
    /// 窗口移动后更新内存中的最新坐标。
    WindowMoved(Point),
    /// 用户请求关闭窗口，保存位置后显式关闭。
    WindowCloseRequested(window::Id),
    /// macOS 系统明暗外观发生变化。
    SystemThemeChanged(theme::Mode),
    /// 定时器消息，驱动初始化进度
    Tick(Instant),
}

/// 应用状态
pub struct Launcher {
    /// 当前屏幕（初始化流程或主界面）
    screen: Screen,
    /// 主界面当前选中的页面
    page: Page,
    /// 当前初始化阶段
    stage: InitStage,
    /// 初始化进度（0.0 ~ 100.0）
    progress: f32,
    /// 上一次进度更新的时刻
    last_tick: Option<Instant>,
    /// 设置页面的本地界面状态
    settings: SettingsState,
    /// 酒馆配置页面的本地界面状态
    tavern: TavernState,
    /// 版本管理页面的本地界面状态
    versions: VersionState,
    /// 扩展管理页面的本地界面状态
    extensions: ExtensionsState,
    /// 资源管理页面状态
    resources: ResourceManageState,
    /// 控制台页面状态
    console: ConsoleState,
    /// 主页上的启动请求状态（服务层接入前用于反馈操作结果）
    launch_requested: bool,
    /// 保留旧版未知字段的配置存储器。
    settings_store: SettingsStore,
    /// 当前窗口最新的逻辑坐标，仅在正常关闭时写入磁盘。
    window_position: Option<[f32; 2]>,
    /// iced 当前检测到的系统明暗模式。
    system_theme: theme::Mode,
}

impl Launcher {
    pub fn new(
        settings_store: SettingsStore,
        preferences: PersistentPreferences,
    ) -> (Self, Task<Message>) {
        // 调试期间暂时跳过首次运行初始化，直接进入主界面。
        // 初始化状态与视图仍保留，后续恢复时只需将 screen 改回 Screen::Init。
        // 启动即探测主显示器并按宽高比校准窗口尺寸。
        // `Task<Option<_>>::and_then` 仅在取到窗口（Some）时执行后续任务；
        // 若此时窗口尚未注册（None），则依赖 `WindowOpened` 事件订阅再次校准。
        // 应用逻辑幂等，重复执行无副作用。
        let detect = window::latest().and_then(|id| {
            window::monitor_size(id).map(move |size| Message::MonitorMeasured(id, size, true))
        });

        let mut settings = SettingsState::default();
        settings.apply_persistent_preferences(preferences);

        (
            Self {
                screen: Screen::Main,
                page: Page::Home,
                stage: InitStage::Welcome,
                progress: 0.0,
                last_tick: None,
                settings,
                tavern: TavernState::default(),
                versions: VersionState::default(),
                extensions: ExtensionsState::default(),
                resources: ResourceManageState::default(),
                console: ConsoleState::default(),
                launch_requested: false,
                settings_store,
                window_position: preferences.window_position,
                system_theme: theme::Mode::Light,
            },
            Task::batch([
                detect,
                iced::system::theme().map(Message::SystemThemeChanged),
            ]),
        )
    }

    pub fn title(&self) -> String {
        t("星酿启动器", effective_language(self.settings.language)).to_owned()
    }

    pub fn theme(&self) -> Theme {
        crate::theme::resolve(self.settings.theme, self.system_theme)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // 初始化进行中订阅定时器，用于平滑推进进度
        let init_timer = if self.stage == InitStage::Initializing {
            time::every(Duration::from_millis(50)).map(Message::Tick)
        } else {
            Subscription::none()
        };

        // 订阅窗口事件：首开（探测显示器并按档位校准尺寸）与缩放变化
        //（跨屏拖动到不同 DPI / 比例显示器时更新尺寸约束）
        let window_events = window::events().filter_map(|(id, event)| match event {
            window::Event::Opened { position, .. } => Some(Message::WindowOpened(id, position)),
            window::Event::Rescaled(_) => Some(Message::WindowRescaled(id)),
            window::Event::Moved(position) => Some(Message::WindowMoved(position)),
            window::Event::CloseRequested => Some(Message::WindowCloseRequested(id)),
            _ => None,
        });

        let system_theme = iced::system::theme_changes().map(Message::SystemThemeChanged);

        Subscription::batch([init_timer, window_events, system_theme])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartInitialization => {
                self.stage = InitStage::Initializing;
                self.progress = 0.0;
                self.last_tick = None;
            }
            Message::CancelInitialization => {
                self.stage = InitStage::Welcome;
                self.progress = 0.0;
                self.last_tick = None;
            }
            Message::FinishInitialization => {
                // 初始化完成，进入主界面
                self.screen = Screen::Main;
            }
            Message::Navigate(page) => {
                // 切换主界面当前页面
                self.page = page;
                if page == Page::Resources {
                    self.resources.configure(&self.settings, &self.versions);
                    self.resources.refresh_all();
                }
                if page == Page::Console {
                    self.console.network_mode = if self.settings.server_mode_enabled {
                        Some(match self.settings.server_service_mode {
                            ServerServiceMode::Lan => crate::pages::console::NetworkMode::Lan,
                            ServerServiceMode::Internet => {
                                crate::pages::console::NetworkMode::Internet
                            }
                        })
                    } else {
                        None
                    };
                }
            }
            Message::LaunchTavern => {
                self.launch_requested = true;
                self.console.network_mode = if self.settings.server_mode_enabled {
                    Some(match self.settings.server_service_mode {
                        ServerServiceMode::Lan => crate::pages::console::NetworkMode::Lan,
                        ServerServiceMode::Internet => crate::pages::console::NetworkMode::Internet,
                    })
                } else {
                    None
                };
            }
            Message::HomeTavernVersionSelected(version) => {
                self.settings.tavern_version = version;
            }
            Message::HomeStartModeSelected(mode) => match mode {
                QuickStartMode::Normal => {
                    self.settings.server_mode_enabled = false;
                    self.settings.start_mode = StartMode::Normal;
                }
                QuickStartMode::Desktop => {
                    self.settings.server_mode_enabled = false;
                    self.settings.start_mode = StartMode::Desktop;
                }
                QuickStartMode::Server => {
                    self.settings.server_mode_enabled = true;
                    self.settings.start_mode = StartMode::Normal;
                }
            },
            Message::HomeBrowserSelected(browser) => {
                self.tavern.update(TavernMessage::SelectBrowser(browser));
            }
            Message::SettingsLanguageSelected(language) => {
                self.settings.language = language;
                self.persist_preferences();
            }
            Message::SettingsThemeSelected(theme) => {
                self.settings.theme = theme;
                self.persist_preferences();
            }
            Message::SettingsRememberWindowPosition(remember) => {
                self.settings.remember_window_position = remember;
                if !remember {
                    self.window_position = None;
                }
                self.persist_preferences();
            }
            Message::SettingsAutoStart(enabled) => self.settings.auto_start = enabled,
            Message::SettingsCpuCoresSelected(value) => self.settings.cpu_cores = value,
            Message::SettingsStartModeSelected(value) => {
                self.settings.start_mode = if self.settings.server_mode_enabled {
                    StartMode::Normal
                } else {
                    value
                };
            }
            Message::SettingsAutoStopTavern(enabled) => {
                self.settings.auto_stop_tavern_on_window_close = enabled
            }
            Message::SettingsServerMode(enabled) => {
                self.settings.server_mode_enabled = enabled;
                if enabled {
                    self.settings.start_mode = StartMode::Normal;
                }
            }
            Message::SettingsServerServiceModeSelected(value) => {
                self.settings.server_service_mode = value
            }
            Message::SettingsAllowTavernBackground(enabled) => {
                self.settings.allow_tavern_background = enabled
            }
            Message::SettingsDataModeSelected(value) => self.settings.data_mode = value,
            Message::SettingsShowStartupCommand(enabled) => {
                self.settings.show_startup_command = enabled
            }
            Message::SettingsNpmRegistrySelected(registry) => {
                self.settings.npm_registry = registry;
            }
            Message::SettingsGithubProxyEnabled(enabled) => {
                self.settings.github_proxy_enabled = enabled;
                if enabled {
                    self.settings.proxy_mode = ProxyMode::None;
                }
            }
            Message::SettingsGithubProxyUrlChanged(value) => self.settings.github_proxy_url = value,
            Message::SettingsProxyModeSelected(mode) => {
                self.settings.proxy_mode = mode;
                if mode != ProxyMode::None {
                    self.settings.github_proxy_enabled = false;
                }
            }
            Message::SettingsCustomProxyChanged(value) => {
                self.settings.custom_proxy = value;
            }
            Message::SettingsAction(action) => {
                self.settings.last_action = Some(action);
            }
            Message::SettingsRestoreDefaults => {
                self.settings = SettingsState::default();
                self.window_position = None;
                self.persist_preferences();
            }
            Message::Tavern(message) => self.tavern.update(message),
            Message::Version(message) => self.versions.update(message),
            Message::Extensions(message) => self.extensions.update(message),
            Message::Resources(message) => {
                self.resources.configure(&self.settings, &self.versions);
                self.resources.update(message);
            }
            Message::Console(message) => self.console.update(message),
            Message::WindowOpened(id, position) => {
                if let Some(position) = position {
                    self.window_position = Some([position.x, position.y]);
                }
                // 窗口打开：探测显示器并按首开档位校准尺寸（含调整为默认尺寸）
                return window::monitor_size(id)
                    .map(move |size| Message::MonitorMeasured(id, size, true));
            }
            Message::WindowRescaled(id) => {
                // 跨屏拖动导致缩放因子变化：仅更新尺寸约束，不改变当前窗口大小
                return window::monitor_size(id)
                    .map(move |size| Message::MonitorMeasured(id, size, false));
            }
            Message::MonitorMeasured(id, monitor, apply_default) => {
                let profile = window_profile(monitor);
                let tasks = vec![
                    window::set_min_size(id, Some(profile.default_size)),
                    window::set_max_size(id, Some(profile.default_size)),
                    // 跨屏后也恢复到目标显示器对应的固定尺寸档位。
                    window::resize(id, profile.default_size),
                ];
                if apply_default {
                    // 首开时通过原生 API 禁用绿色缩放按钮并移除独占全屏能力
                    #[cfg(target_os = "macos")]
                    crate::platform::disable_zoom_button_and_fullscreen();
                }
                return Task::batch(tasks);
            }
            Message::WindowMoved(position) => {
                self.window_position = Some([position.x, position.y]);
            }
            Message::WindowCloseRequested(id) => {
                if self.settings.remember_window_position {
                    self.persist_preferences();
                }
                return window::close(id);
            }
            Message::SystemThemeChanged(mode) => {
                self.system_theme = mode;
            }
            Message::Tick(now) => {
                if self.stage == InitStage::Initializing {
                    let elapsed = self
                        .last_tick
                        .and_then(|last| now.checked_duration_since(last))
                        .map(|duration| duration.as_secs_f32())
                        .unwrap_or(0.0);
                    self.last_tick = Some(now);
                    // 每秒推进约 20%，总计约 5 秒完成
                    self.progress = (self.progress + elapsed * 20.0).min(100.0);
                    if self.progress >= 100.0 {
                        self.stage = InitStage::Complete;
                    }
                }
            }
        }
        Task::none()
    }

    /// 保存已接入的偏好，同时把失败原因交给设置页展示。
    fn persist_preferences(&mut self) {
        let preferences = PersistentPreferences {
            language: self.settings.language,
            theme: self.settings.theme,
            remember_window_position: self.settings.remember_window_position,
            window_position: if self.settings.remember_window_position {
                self.window_position
            } else {
                None
            },
        };
        self.settings.save_error = self
            .settings_store
            .save(preferences)
            .err()
            .map(|error| error.to_string());
    }

    pub fn view(&self) -> Element<'_, Message> {
        crate::lang::set_language(effective_language(self.settings.language));
        match self.screen {
            Screen::Init => self.init_view(),
            Screen::Main => self.main_view(),
        }
    }

    /// 初始化流程视图（首次运行引导）。
    fn init_view(&self) -> Element<'_, Message> {
        container(
            column![self.brand_header(), self.init_card()]
                .spacing(30)
                .width(600)
                .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(crate::theme::canvas_style)
        .into()
    }

    /// 主界面视图：左侧导航栏 + 右侧内容区。
    fn main_view(&self) -> Element<'_, Message> {
        row![
            sidebar::sidebar(self.page),
            pages::page_view(
                self.page,
                &self.settings,
                &self.tavern,
                &self.versions,
                &self.extensions,
                &self.resources,
                self.launch_requested,
                &self.console,
            ),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    /// 顶部品牌区：Logo、应用名与标语
    fn brand_header(&self) -> Element<'_, Message> {
        column![
            Avatar::new("AstraBrew")
                .fallback(icons::icon(Icon::Beer, 24, WHITE))
                .size(AvatarSize::Large)
                .shape(AvatarShape::Rounded)
                .color(AvatarColor::Accent),
            text("AstraBrew Launcher").size(30).font(fonts::MEDIUM),
            text("Native macOS launcher for AstraBrew-Labs")
                .size(14)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(14)
        .align_x(Alignment::Center)
        .into()
    }

    /// 初始化主卡片：标题、步骤清单、进度条、状态提示与操作按钮
    fn init_card(&self) -> Element<'_, Message> {
        let (title, description) = match self.stage {
            InitStage::Welcome => (
                "欢迎使用 AstraBrew Launcher",
                "首次运行需要初始化运行环境，请点击下方按钮开始。",
            ),
            InitStage::Initializing => ("正在初始化", "正在准备运行环境，请勿关闭应用。"),
            InitStage::Complete => ("初始化完成", "运行环境已准备就绪，可以开始使用。"),
        };

        let header = column![
            text(title).size(18).font(fonts::MEDIUM),
            text(description)
                .size(12)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(6);

        crate::theme::card(
            column![
                header,
                crate::theme::separator(),
                self.steps(),
                self.progress_bar(),
                self.status_alert(),
                self.actions(),
            ]
            .spacing(18),
            600,
            28,
        )
    }

    /// 环境准备步骤清单，每项根据进度显示等待 / 进行中 / 完成
    fn steps(&self) -> Element<'_, Message> {
        // 定位当前正在执行的步骤下标
        let running = INIT_STEPS
            .iter()
            .position(|step| self.progress < step.threshold);

        let rows = INIT_STEPS.iter().enumerate().map(|(index, step)| {
            let status = if self.progress >= step.threshold {
                StepStatus::Done
            } else if running == Some(index) {
                StepStatus::Running
            } else {
                StepStatus::Pending
            };
            self.step_row(step, status)
        });

        column(rows).spacing(14).into()
    }

    /// 渲染单条步骤行：状态图标 + 名称与说明 + 状态标签
    fn step_row(&self, step: &InitStep, status: StepStatus) -> Element<'_, Message> {
        let (icon, color, label) = match status {
            StepStatus::Done => (Icon::CircleCheck, SUCCESS, "完成"),
            StepStatus::Running => (Icon::Loader, CYAN_500, "进行中"),
            StepStatus::Pending => (Icon::Circle, INK_SUBTLE, "等待"),
        };

        row![
            container(icons::icon(icon, 16, color))
                .width(34)
                .height(34)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(tag_style(color)),
            column![
                text(step.name).size(13).font(fonts::MEDIUM),
                text(step.path)
                    .size(11)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style),
            ]
            .spacing(2),
            space::horizontal(),
            chip(label, None, color, ChipVariant::Flat),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    }

    /// 初始化进度条，完成后切换为成功色
    fn progress_bar(&self) -> Element<'_, Message> {
        let (value, color) = match self.stage {
            InitStage::Welcome => (0.0, ProgressBarColor::Accent),
            InitStage::Initializing => (self.progress, ProgressBarColor::Accent),
            InitStage::Complete => (100.0, ProgressBarColor::Success),
        };

        ProgressBar::new(value)
            .label("初始化进度")
            .color(color)
            .into()
    }

    /// 状态提示，随初始化阶段切换语义与文案
    fn status_alert(&self) -> Element<'_, Message> {
        let alert = match self.stage {
            InitStage::Welcome => crate::theme::alert(
                "准备就绪",
                "点击下方按钮开始初始化运行环境。",
                AlertKind::Info,
            ),
            InitStage::Initializing => crate::theme::alert(
                "正在初始化",
                "初始化过程中请保持应用运行，完成后会自动进入就绪状态。",
                AlertKind::Info,
            ),
            InitStage::Complete => crate::theme::alert(
                "初始化完成",
                "运行环境已准备完毕，可以开始使用 AstraBrew Launcher。",
                AlertKind::Success,
            ),
        };
        alert
    }

    /// 底部操作按钮区，随初始化阶段切换
    fn actions(&self) -> Element<'_, Message> {
        match self.stage {
            InitStage::Welcome => row![
                space::horizontal(),
                self.primary_button("开始初始化", Message::StartInitialization),
            ]
            .spacing(10)
            .width(Fill)
            .into(),
            InitStage::Initializing => row![
                space::horizontal(),
                self.disabled_button("初始化中…"),
                self.outline_button("取消", Message::CancelInitialization),
            ]
            .spacing(10)
            .width(Fill)
            .into(),
            InitStage::Complete => row![
                space::horizontal(),
                self.primary_button("开始使用", Message::FinishInitialization),
                self.outline_button("重新初始化", Message::CancelInitialization),
            ]
            .spacing(10)
            .width(Fill)
            .into(),
        }
    }

    /// 主操作按钮（Primary 语义）
    fn primary_button(&self, label: &'static str, message: Message) -> Element<'_, Message> {
        button(text(label).size(13).font(fonts::MEDIUM))
            .on_press(message)
            .height(40)
            .padding([10, 20])
            .style(button_style(ButtonVariant::Primary))
            .into()
    }

    /// 次要操作按钮（Outline 语义）
    fn outline_button(&self, label: &'static str, message: Message) -> Element<'_, Message> {
        button(text(label).size(13).font(fonts::MEDIUM))
            .on_press(message)
            .height(40)
            .padding([10, 20])
            .style(button_style(ButtonVariant::Outline))
            .into()
    }

    /// 禁用态按钮（不绑定点击事件，自动呈现禁用样式）
    fn disabled_button(&self, label: &'static str) -> Element<'_, Message> {
        button(text(label).size(13).font(fonts::MEDIUM))
            .height(40)
            .padding([10, 20])
            .style(button_style(ButtonVariant::Primary))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::{InitStage, Launcher, Message};
    use crate::core::settings::{PersistentPreferences, SettingsStore};
    use crate::pages::settings::{DisplayLanguage, StartMode, ThemeMode};

    fn test_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "astrabrew-app-test-{name}-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    fn launcher() -> Launcher {
        let path = test_path("state");
        let (store, _) = SettingsStore::load(path);
        Launcher::new(store, PersistentPreferences::default()).0
    }

    #[test]
    fn start_then_cancel_resets_progress() {
        let mut launcher = launcher();
        assert_eq!(launcher.stage, InitStage::Welcome);

        let _ = launcher.update(Message::StartInitialization);
        assert_eq!(launcher.stage, InitStage::Initializing);
        assert_eq!(launcher.progress, 0.0);

        let _ = launcher.update(Message::CancelInitialization);
        assert_eq!(launcher.stage, InitStage::Welcome);
        assert_eq!(launcher.progress, 0.0);
    }

    #[test]
    fn progress_advances_toward_completion() {
        let mut launcher = launcher();
        let start = iced::time::Instant::now();
        let _ = launcher.update(Message::StartInitialization);

        // 第一次 tick 无时间差，进度保持不变
        let _ = launcher.update(Message::Tick(start));
        assert_eq!(launcher.progress, 0.0);

        // 推进 6 秒，远超完成所需时间，应完成
        let _ = launcher.update(Message::Tick(
            start + iced::time::Duration::from_millis(6000),
        ));
        assert_eq!(launcher.stage, InitStage::Complete);
        assert_eq!(launcher.progress, 100.0);
    }

    #[test]
    fn settings_restore_defaults_resets_local_preferences() {
        let mut launcher = launcher();

        let _ = launcher.update(Message::SettingsLanguageSelected(DisplayLanguage::English));
        assert_eq!(launcher.title(), "AstraBrew Launcher");
        let _ = launcher.update(Message::SettingsRememberWindowPosition(false));
        let _ = launcher.update(Message::SettingsStartModeSelected(StartMode::Desktop));
        let _ = launcher.update(Message::SettingsRestoreDefaults);

        assert_eq!(launcher.settings.language, DisplayLanguage::System);
        assert_eq!(launcher.settings.start_mode, StartMode::Normal);
        assert!(launcher.settings.remember_window_position);
    }

    #[test]
    fn proxy_and_github_acceleration_are_mutually_exclusive() {
        let mut launcher = launcher();
        let _ = launcher.update(Message::SettingsGithubProxyEnabled(true));
        let _ = launcher.update(Message::SettingsProxyModeSelected(
            crate::pages::settings::ProxyMode::System,
        ));
        assert!(!launcher.settings.github_proxy_enabled);
    }

    #[test]
    fn disabling_window_restore_clears_saved_coordinate() {
        let path = test_path("window-position");
        let (store, _) = SettingsStore::load(&path);
        let mut launcher = Launcher::new(store, PersistentPreferences::default()).0;

        let _ = launcher.update(Message::WindowMoved(iced::Point::new(-320.0, 96.0)));
        let _ = launcher.update(Message::SettingsRememberWindowPosition(false));

        let document: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path).expect("read persisted settings"),
        )
        .expect("parse persisted settings");
        assert_eq!(document["remember_window_pos"], false);
        assert!(document["window_position"].is_null());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn system_theme_only_changes_follow_system_mode() {
        let mut launcher = launcher();
        let _ = launcher.update(Message::SettingsThemeSelected(ThemeMode::Light));
        let _ = launcher.update(Message::SystemThemeChanged(iced::theme::Mode::Dark));
        assert_eq!(launcher.theme().palette().background, crate::theme::light_theme().palette().background);

        let _ = launcher.update(Message::SettingsThemeSelected(ThemeMode::System));
        assert_eq!(launcher.theme().palette().background, crate::theme::dark_theme().palette().background);
    }
}
