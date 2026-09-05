//! AstraBrew Launcher 应用根模块。
//!
//! 负责在「初始化流程」与「主界面」两个屏幕之间路由。首次启动引导用户完成
//! 运行环境初始化，完成后进入主界面（左侧导航栏 + 右侧内容区）。
//! 界面组件统一来自 astra_ui（Astra UI）组件库。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use iced::time::{self, Duration, Instant};
use iced::widget::{button, column, container, row, space};
use iced::{Alignment, Element, Fill, Point, Size, Subscription, Task, Theme, theme, window};
use lucide_icons::Icon;

use astra_ui::fonts;
use astra_ui::icons;
use astra_ui::{
    AlertKind, Avatar, AvatarColor, AvatarShape, AvatarSize, ButtonVariant, CYAN_500, ChipVariant,
    INK_SUBTLE, ProgressBar, ProgressBarColor, SUCCESS, WHITE, chip, tag_style,
};

use crate::core::network::{
    DownloadChannel, DownloadChannelTestEvent, GithubTestEvent, SillyTavernCatalog,
    SillyTavernInstallEvent, SillyTavernInstallTarget,
};
use crate::core::settings::{PersistentPreferences, SettingsStore};
use crate::lang::{effective_language, t, text};
use crate::pages::Page;
use crate::pages::console::{ConsoleMessage, ConsoleState};
use crate::pages::extensions::{ExtensionsMessage, ExtensionsState};
use crate::pages::resource_manage::{ResourceManageMessage, ResourceManageState};
#[cfg(not(test))]
use crate::pages::settings::EnvironmentVersions;
use crate::pages::settings::{
    CpuCores, DisplayLanguage, DownloadChannelTestState, EnvironmentDependency,
    EnvironmentTaskState, GithubLiveItem, GithubLiveItemStatus, GithubTestState, NpmRegistry,
    ProxyMode, QuickStartMode, ServerServiceMode, SettingsAction, SettingsState, StartMode,
    SystemProxyStatus, TavernDataMode, TavernVersion, ThemeMode,
};
use crate::pages::tavern::{BrowserType, TavernMessage, TavernState};
use crate::pages::versions::{TavernBranch, VersionMessage, VersionState};
use crate::theme::button_style;
use crate::{pages, sidebar};

mod local_instances;
mod tavern_config;

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
    /// 修改酒馆启动模式；主页与设置页共用此消息，主页只提供快捷入口。
    SettingsLaunchModeSelected(QuickStartMode),
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
    SettingsAutoStopTavern(bool),
    SettingsServerMode(bool),
    SettingsServerServiceModeSelected(ServerServiceMode),
    SettingsAllowTavernBackground(bool),
    SettingsDataModeSelected(TavernDataMode),
    SettingsShowStartupCommand(bool),
    /// 修改 NPM 软件源
    SettingsNpmRegistrySelected(NpmRegistry),
    /// 修改酒馆下载渠道。
    SettingsDownloadChannelSelected(DownloadChannel),
    /// 修改网络代理模式
    SettingsProxyModeSelected(ProxyMode),
    SettingsCustomProxyChanged(String),
    /// 触发尚待服务层接入的设置操作
    SettingsAction(SettingsAction),
    /// 驱动 GitHub 连接测试的后台轮询。
    GithubTestTick(Instant),
    /// 关闭 GitHub 测试结果。
    GithubTestClose,
    /// 消费 GitHub 测试弹窗内部及遮罩点击。
    GithubTestInteract,
    /// 驱动酒馆下载渠道自动测速。
    DownloadChannelTestTick(Instant),
    /// 关闭酒馆下载渠道测速弹窗。
    DownloadChannelTestClose,
    /// 消费下载渠道测速弹窗内部及遮罩点击。
    DownloadChannelTestInteract,
    /// 安装或更新旧版环境依赖。
    EnvironmentInstall(EnvironmentDependency),
    /// 驱动旧版安装任务的日志轮询、超时与自动关闭。
    EnvironmentTaskTick(Instant),
    /// 关闭已经完成或超时的安装窗口。
    EnvironmentTaskClose,
    /// 消费环境安装弹窗内部及遮罩点击。
    EnvironmentModalInteract,
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
    /// 驱动在线版本列表后台任务。
    VersionCatalogTick,
    /// 驱动在线酒馆安装后台任务和完成倒计时。
    VersionInstallTick,
    /// 独立于弹窗可见性的本地任务轮询。
    LocalInstancesTick,
    /// 配置文本去抖、外部文件轮询及保存回执。
    TavernConfigTick,
    /// 文件选择器结果绑定发起时的配置目标，不能套用到后来的实例。
    ConfigImportChosen(String, Option<PathBuf>),
    /// 原生文件选择器返回的清单路径；None 表示用户取消。
    LocalImportChosen(Option<PathBuf>),
}

/// 应用状态
pub struct Launcher {
    /// 按配置目标隔离的实时保存协调器。
    config_runtime: tavern_config::ConfigRuntime,
    /// 本地实例后台服务及其事件通道。
    local_runtime: local_instances::LocalRuntime,
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
    /// 旧版环境安装任务的后台日志通道。
    environment_task_receiver: Option<Receiver<String>>,
    /// GitHub 测试完成结果的后台通道。
    github_test_receiver: Option<Receiver<GithubTestEvent>>,
    /// 当前 GitHub 测试序号，用于丢弃取消后的旧结果。
    github_test_id: u64,
    /// 当前 GitHub 测试的取消信号。
    github_test_cancel: Option<Arc<AtomicBool>>,
    /// 酒馆下载渠道测速后台事件通道。
    download_channel_test_receiver: Option<Receiver<DownloadChannelTestEvent>>,
    /// 当前下载渠道测速的取消信号。
    download_channel_test_cancel: Option<Arc<AtomicBool>>,
    /// 在线版本列表后台结果通道。
    version_catalog_receiver: Option<Receiver<Result<SillyTavernCatalog, String>>>,
    /// 在线酒馆安装后台事件通道。
    version_install_receiver: Option<Receiver<SillyTavernInstallEvent>>,
    /// 在线酒馆安装取消信号。
    version_install_cancel: Option<Arc<AtomicBool>>,
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
        settings.apply_persistent_preferences(&preferences);
        if let Some(cache) = crate::core::network::load_download_channel_cache() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs())
                .unwrap_or_default();
            if cache.is_valid_at(now) {
                settings.download_resolved_channel = Some(cache.resolved_channel);
                settings.download_channel_last_tested = Some(cache.tested_at);
                // 复用上次测速明细，打开设置时不需要再次请求网络。
                settings.download_channel_test.results = cache.results;
            }
        }
        settings.auto_start = crate::core::auto_launch::is_auto_launch_enabled();
        if settings.proxy_mode == ProxyMode::System {
            settings.system_proxy_status = Self::current_system_proxy_status();
        }
        // 与旧版一致：应用创建时同步检测全部环境依赖。
        #[cfg(not(test))]
        {
            settings.environment = EnvironmentVersions::detect_all();
        }

        let mut versions = VersionState::default();
        versions.set_staging_risk_confirmed(preferences.staging_risk_confirmed);
        if let Some(installed) = crate::core::network::installed_sillytavern_state() {
            versions.restore_installed(&installed);
        }

        let mut launcher = Self {
            config_runtime: tavern_config::ConfigRuntime::default(),
            local_runtime: local_instances::LocalRuntime::default(),
            screen: Screen::Main,
            page: Page::Home,
            stage: InitStage::Welcome,
            progress: 0.0,
            last_tick: None,
            settings,
            environment_task_receiver: None,
            github_test_receiver: None,
            github_test_id: 0,
            github_test_cancel: None,
            download_channel_test_receiver: None,
            download_channel_test_cancel: None,
            version_catalog_receiver: None,
            version_install_receiver: None,
            version_install_cancel: None,
            tavern: TavernState::default(),
            versions,
            extensions: ExtensionsState::default(),
            resources: ResourceManageState::default(),
            console: ConsoleState::default(),
            launch_requested: false,
            settings_store,
            window_position: preferences.window_position,
            system_theme: theme::Mode::Light,
        };
        // 应用启动即加载默认稳定版目录，避免用户必须进入页面后点击安装才能恢复状态。
        launcher.start_version_catalog_load(false);
        #[cfg(not(test))]
        launcher.load_local_instances();
        #[cfg(not(test))]
        launcher.reconcile_tavern_config();
        if launcher.settings.auto_start != preferences.auto_start {
            launcher.persist_preferences();
        }

        (
            launcher,
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

        let environment_timer = if self.settings.environment_task.running
            || self.settings.environment_task.done_at.is_some()
        {
            time::every(Duration::from_millis(100)).map(Message::EnvironmentTaskTick)
        } else {
            Subscription::none()
        };

        let github_test_timer = if self.settings.github_test.running {
            time::every(Duration::from_millis(100)).map(Message::GithubTestTick)
        } else {
            Subscription::none()
        };

        let download_channel_timer = if self.settings.download_channel_test.running
            || self.settings.download_channel_test.done_at.is_some()
        {
            time::every(Duration::from_millis(100)).map(Message::DownloadChannelTestTick)
        } else {
            Subscription::none()
        };

        let version_catalog_timer = if self.version_catalog_receiver.is_some()
            || self.versions.online_status == crate::pages::versions::OnlineVersionsStatus::Loading
        {
            time::every(Duration::from_millis(100)).map(|_| Message::VersionCatalogTick)
        } else {
            Subscription::none()
        };
        let version_install_timer =
            if self.version_install_receiver.is_some() || self.versions.install_task.running {
                time::every(Duration::from_millis(100)).map(|_| Message::VersionInstallTick)
            } else if self.versions.install_task.auto_close_ticks > 0 {
                // 成功后的倒计时按秒驱动，确保弹窗在 3 秒后而不是 0.3 秒后关闭。
                time::every(Duration::from_secs(1)).map(|_| Message::VersionInstallTick)
            } else {
                Subscription::none()
            };

        let system_theme = iced::system::theme_changes().map(Message::SystemThemeChanged);

        let local_timer = if self.local_needs_tick() {
            time::every(Duration::from_millis(100)).map(|_| Message::LocalInstancesTick)
        } else {
            Subscription::none()
        };

        let config_timer = if self.config_needs_tick() {
            time::every(Duration::from_millis(50)).map(|_| Message::TavernConfigTick)
        } else {
            Subscription::none()
        };
        Subscription::batch([
            config_timer,
            local_timer,
            init_timer,
            environment_timer,
            github_test_timer,
            download_channel_timer,
            version_catalog_timer,
            version_install_timer,
            window_events,
            system_theme,
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // 所有早返回路径也要同步配置上下文，不能遗漏异步实例切换或首页快捷操作。
        self.reconcile_tavern_config();
        let task = self.update_inner(message);
        self.reconcile_tavern_config();
        task
    }

    fn update_inner(&mut self, message: Message) -> Task<Message> {
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
                if page == Page::TavernConfig {
                    self.request_config_refresh();
                }
                if page == Page::Resources {
                    self.resources.configure(&self.settings, &self.versions);
                    self.resources.refresh_all();
                }
                if page == Page::Version {
                    self.start_version_catalog_load(false);
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
            Message::SettingsLaunchModeSelected(mode) => {
                self.apply_launch_mode(mode);
                self.persist_preferences();
            }
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
            Message::SettingsAutoStart(enabled) => {
                match crate::core::auto_launch::set_auto_launch(enabled) {
                    Ok(()) => {
                        self.settings.auto_start = enabled;
                        self.persist_preferences();
                    }
                    Err(error) => {
                        self.settings.auto_start =
                            crate::core::auto_launch::is_auto_launch_enabled();
                        self.persist_preferences();
                        self.settings.save_error = Some(error);
                    }
                }
            }
            Message::SettingsCpuCoresSelected(value) => self.settings.cpu_cores = value,
            Message::SettingsAutoStopTavern(enabled) => {
                self.settings.auto_stop_tavern_on_window_close = enabled
            }
            Message::SettingsServerMode(enabled) => {
                self.apply_launch_mode(if enabled {
                    QuickStartMode::Server
                } else {
                    QuickStartMode::Normal
                });
                self.persist_preferences();
            }
            Message::SettingsServerServiceModeSelected(value) => {
                self.settings.server_service_mode = value
            }
            Message::SettingsAllowTavernBackground(enabled) => {
                self.settings.allow_tavern_background = enabled
            }
            Message::SettingsDataModeSelected(value) => {
                self.settings.data_mode = value;
                self.resources.configure(&self.settings, &self.versions);
                self.resources.refresh_all();
                self.persist_preferences();
            }
            Message::SettingsShowStartupCommand(enabled) => {
                self.settings.show_startup_command = enabled
            }
            Message::SettingsNpmRegistrySelected(registry) => {
                self.settings.npm_registry = registry;
                self.persist_preferences();
            }
            Message::SettingsDownloadChannelSelected(channel) => {
                self.settings.download_channel = channel;
                if channel == DownloadChannel::Auto {
                    self.persist_preferences();
                    if !self.settings.download_channel_cache_valid() {
                        self.start_download_channel_test();
                    }
                } else {
                    // 手动渠道切换只改变当前使用渠道，不覆盖自动模式的缓存结果。
                    // 这样用户之后切回“自动”时仍可复用原有缓存，而不会重复测速；
                    // “自动”按钮也会继续展示缓存对应的实际渠道。
                    self.cancel_download_channel_test();
                    self.settings.download_channel_test = DownloadChannelTestState::default();
                    self.persist_preferences();
                }
            }
            Message::SettingsProxyModeSelected(mode) => {
                self.settings.proxy_mode = mode;
                self.settings.system_proxy_status = if mode == ProxyMode::System {
                    Self::current_system_proxy_status()
                } else {
                    SystemProxyStatus::Unknown
                };
                self.persist_preferences();
            }
            Message::SettingsCustomProxyChanged(value) => {
                self.settings.custom_proxy = value;
                self.persist_preferences();
            }
            Message::SettingsAction(action) => match action {
                SettingsAction::TestGithub => self.start_github_test(),
                SettingsAction::OpenLoginItemSettings => {
                    match crate::core::auto_launch::open_login_item_settings() {
                        Ok(()) => self.settings.last_action = Some(action),
                        Err(error) => self.settings.save_error = Some(error),
                    }
                }
                SettingsAction::ChooseExportPath => {
                    if let Some(path) = Self::pick_directory(&self.settings.tavern_export_path) {
                        self.settings.tavern_export_path = path;
                        self.persist_preferences();
                        self.settings.last_action = Some(action);
                    }
                }
                SettingsAction::ChooseGlobalDataPath => {
                    if let Some(path) = Self::pick_directory(&self.settings.global_data_path) {
                        self.settings.global_data_path = path;
                        self.resources.configure(&self.settings, &self.versions);
                        self.resources.refresh_all();
                        self.persist_preferences();
                        self.settings.last_action = Some(action);
                    }
                }
                SettingsAction::RefreshDownloadChannel => {
                    self.settings.download_channel = DownloadChannel::Auto;
                    self.persist_preferences();
                    self.start_download_channel_test();
                }
                _ => self.settings.last_action = Some(action),
            },
            Message::GithubTestTick(now) => {
                self.poll_github_test(now);
            }
            Message::GithubTestClose => {
                self.github_test_id = self.github_test_id.wrapping_add(1);
                if let Some(cancel) = &self.github_test_cancel {
                    cancel.store(true, Ordering::Relaxed);
                }
                self.github_test_cancel = None;
                self.github_test_receiver = None;
                self.settings.github_test = GithubTestState::default();
            }
            Message::GithubTestInteract => {}
            Message::DownloadChannelTestTick(now) => {
                self.poll_download_channel_test(now);
            }
            Message::DownloadChannelTestClose => {
                self.cancel_download_channel_test();
                self.settings.download_channel_test = DownloadChannelTestState::default();
            }
            Message::DownloadChannelTestInteract => {}
            Message::EnvironmentInstall(dependency) => {
                self.start_environment_install(dependency);
            }
            Message::EnvironmentTaskTick(now) => {
                self.poll_environment_task(now);
            }
            Message::EnvironmentTaskClose => {
                if !self.settings.environment_task.running {
                    self.settings.environment_task.show = false;
                    self.settings.environment_task.timed_out = false;
                    self.settings.environment_task.failed = false;
                    self.settings.environment_task.started_at = None;
                    self.settings.environment_task.done_at = None;
                }
            }
            Message::EnvironmentModalInteract => {}
            Message::SettingsRestoreDefaults => {
                let environment = self.settings.environment.clone();
                self.settings = SettingsState::default();
                self.settings.environment = environment;
                self.environment_task_receiver = None;
                if let Some(cancel) = &self.github_test_cancel {
                    cancel.store(true, Ordering::Relaxed);
                }
                self.github_test_cancel = None;
                self.github_test_receiver = None;
                self.github_test_id = self.github_test_id.wrapping_add(1);
                self.cancel_download_channel_test();
                if let Some(cancel) = &self.version_install_cancel {
                    cancel.store(true, Ordering::Relaxed);
                }
                self.version_install_receiver = None;
                self.version_install_cancel = None;
                self.window_position = None;
                self.persist_preferences();
            }
            Message::Tavern(message) => return self.handle_tavern_config_message(message),
            Message::TavernConfigTick => return self.poll_tavern_config(),
            Message::ConfigImportChosen(key, path) => self.config_import_chosen(key, path),
            Message::Version(VersionMessage::ImportLocal) => return self.pick_local_instance(),
            Message::LocalImportChosen(path) => self.import_local_file(path),
            Message::LocalInstancesTick => self.poll_local_instances(),
            Message::Version(message) => self.handle_version_message(message),
            Message::VersionCatalogTick => self.poll_version_catalog(),
            Message::VersionInstallTick => self.poll_version_install(),
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
                if self.defer_config_close(id) {
                    return Task::none();
                }
                // 酒馆安装期间弹窗不可关闭，也不允许通过窗口关闭绕过安装流程。
                if self.versions.install_task.running || self.versions.local.install.running {
                    return Task::none();
                }
                if self.local_has_pending_save() {
                    self.versions
                        .local
                        .notify("正在保存本地实例，请稍后关闭。", "", false);
                    return Task::none();
                }
                if self.settings.remember_window_position {
                    self.persist_preferences();
                }
                self.stop_scan_for_exit();
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

    fn persist_staging_risk_confirmation(&mut self) {
        let preferences = PersistentPreferences {
            staging_risk_confirmed: self.versions.staging_risk_confirmed,
            ..self.current_preferences()
        };
        self.settings.save_error = self
            .settings_store
            .save(preferences)
            .err()
            .map(|error| error.to_string());
    }

    fn current_preferences(&self) -> PersistentPreferences {
        PersistentPreferences {
            language: self.settings.language,
            theme: self.settings.theme,
            remember_window_position: self.settings.remember_window_position,
            window_position: if self.settings.remember_window_position {
                self.window_position
            } else {
                None
            },
            proxy_mode: match self.settings.proxy_mode {
                ProxyMode::None => "none".to_owned(),
                ProxyMode::System => "system".to_owned(),
                ProxyMode::Custom => "custom".to_owned(),
            },
            custom_proxy: self.settings.custom_proxy.clone(),
            github_proxy_enabled: self.settings.github_proxy_enabled,
            github_proxy_url: self.settings.github_proxy_url.clone(),
            npm_registry: self.settings.npm_registry.url().to_owned(),
            download_channel: self.settings.download_channel.key().to_owned(),
            auto_start: self.settings.auto_start,
            data_mode: match self.settings.data_mode {
                TavernDataMode::Global => "global".to_owned(),
                TavernDataMode::Current => "current".to_owned(),
            },
            global_data_path: self.settings.global_data_path.clone(),
            tavern_export_path: self.settings.tavern_export_path.clone(),
            start_mode: match self.settings.start_mode {
                StartMode::Normal => "normal".to_owned(),
                StartMode::Desktop => "desktop".to_owned(),
            },
            server_mode_enabled: self.settings.server_mode_enabled,
            staging_risk_confirmed: self.versions.staging_risk_confirmed,
        }
    }

    /// 开始读取酒馆在线版本；版本页内部只负责发送 RefreshOnline，网络逻辑集中在应用层。
    fn start_version_catalog_load(&mut self, force: bool) {
        if self.version_catalog_receiver.is_some() || (!force && self.versions.branch_loaded) {
            return;
        }
        self.versions.update(VersionMessage::RefreshOnline);
        let (sender, receiver) = mpsc::channel();
        self.version_catalog_receiver = Some(receiver);
        let branch = self.versions.branch.name().to_owned();
        let channel = self.settings.download_channel;
        let proxy_mode = match self.settings.proxy_mode {
            ProxyMode::None => "none".to_owned(),
            ProxyMode::System => "system".to_owned(),
            ProxyMode::Custom => "custom".to_owned(),
        };
        let proxy_host = self.settings.custom_proxy.clone();
        std::thread::spawn(move || {
            let result = crate::core::network::fetch_sillytavern_catalog(
                &branch,
                channel,
                &proxy_mode,
                &proxy_host,
            );
            let _ = sender.send(result);
        });
    }

    /// 将网络层的版本模型转换为版本页面模型。
    fn apply_version_catalog(&mut self, catalog: SillyTavernCatalog) {
        let installed = crate::core::network::installed_sillytavern_state();
        self.versions
            .set_online_instance_exists(installed.is_some());
        let installed_tag = installed
            .as_ref()
            .and_then(|state| state.tag_name.as_deref());
        let installed_staging =
            installed.as_ref().and_then(|state| state.branch.as_deref()) == Some("staging");
        let releases = catalog
            .releases
            .into_iter()
            .map(|release| {
                let is_installed = installed_tag == Some(release.tag_name.as_str());
                crate::pages::versions::OnlineRelease {
                    version: release.version,
                    tag_name: release.tag_name,
                    published_at: release.published_at,
                    created_at: release.created_at,
                    body: release.body.clone(),
                    summary: release.body,
                    installed: is_installed,
                    mirror_available: release.mirror_available,
                }
            })
            .collect::<Vec<_>>();
        self.versions.update(VersionMessage::OnlineVersionsLoaded {
            branch: if catalog.branch == "staging" {
                TavernBranch::Staging
            } else {
                TavernBranch::Release
            },
            releases,
            staging: catalog.staging,
            last_sync: format_version_sync_time(catalog.cached_at),
            from_cache: catalog.used_stale_cache,
        });
        self.versions.sync_online_installation(installed.as_ref());
        // 在线目录刷新不能覆盖用户刚刚切换的本地实例。
        if installed_staging
            && self.versions.branch == TavernBranch::Staging
            && self.versions.current_source != Some(crate::pages::versions::VersionSource::Local)
        {
            if let Some(staging) = self.versions.staging.as_ref() {
                self.versions.current_version = Some("staging".to_owned());
                self.versions.online_instance_path = Some(
                    crate::core::network::sillytavern_install_dir()
                        .to_string_lossy()
                        .into_owned(),
                );
                self.versions.current_path = self.versions.online_instance_path.clone();
                self.versions.current_source = Some(crate::pages::versions::VersionSource::Online);
                let _ = staging;
            }
        }
    }

    /// 处理版本页消息并启动后台网络任务。
    fn handle_version_message(&mut self, message: VersionMessage) {
        if self.handle_local_message(&message) {
            return;
        }
        if matches!(
            message,
            VersionMessage::InstallOnline(_)
                | VersionMessage::InstallBranch(_)
                | VersionMessage::SwitchOnline(_)
        ) {
            if self.versions.local.loading {
                self.versions.local.notify("正在加载本地实例…", "", false);
                return;
            }
            if self.versions.local.install.running || self.versions.install_task.running {
                self.versions
                    .local
                    .notify("已有安装任务正在执行，请稍后再试。", "", true);
                return;
            }
            self.invalidate_local_switch();
        }
        match &message {
            VersionMessage::RefreshOnline => {
                self.start_version_catalog_load(true);
                return;
            }
            VersionMessage::SelectBranch(_branch) => {
                // 分支选择只切换页面上下文并加载对应信息，不自动修改本地 Git 工作目录。
                // 真正切换 release/staging 必须由用户点击对应的安装或切换按钮确认。
                let previous = self.versions.branch;
                self.versions.update(message);
                if self.versions.branch != previous {
                    self.start_version_catalog_load(true);
                }
                return;
            }
            VersionMessage::ConfirmStagingRisk => {
                // 风险确认只确认开发版选择，不触发安装；安装仍由用户主动点击按钮启动。
                self.versions.update(message);
                self.persist_staging_risk_confirmation();
                self.start_version_catalog_load(true);
                return;
            }
            VersionMessage::SelectTab(crate::pages::versions::VersionTab::Online) => {
                self.versions.update(message);
                self.start_version_catalog_load(false);
                return;
            }
            VersionMessage::SwitchOnline(version) => {
                let installed = crate::core::network::installed_sillytavern_state();
                self.switch_online_version(version.clone(), installed.as_ref());
                return;
            }
            VersionMessage::InstallBranch(branch) => {
                let branch_name = branch.clone();
                self.versions.update(message);
                self.start_version_install_target(
                    SillyTavernInstallTarget::Branch(branch_name.clone()),
                    branch_name,
                );
                return;
            }
            VersionMessage::InstallOnline(version) => {
                let release = self
                    .versions
                    .online_releases
                    .iter()
                    .find(|release| release.version == *version)
                    .cloned();
                self.versions.update(message);
                if let Some(release) = release {
                    self.start_version_install(release);
                }
                return;
            }
            _ => {}
        }
        self.versions.update(message);
    }

    /// 以本次磁盘检测为准切换实例；传入快照也便于测试时避免访问真实 Git 目录。
    fn switch_online_version(
        &mut self,
        version: String,
        installed: Option<&crate::core::network::InstalledSillyTavern>,
    ) {
        let release = self
            .versions
            .online_releases
            .iter()
            .find(|item| item.version == version)
            .cloned();
        if version != "staging" && release.is_none() {
            self.versions
                .local
                .notify("未找到要切换的在线版本，请刷新列表。", version, true);
            return;
        }
        self.versions.sync_online_installation(installed);
        if self.versions.is_online_installed(&version) {
            // 已经安装目标版本时只切换当前实例，不重新下载或安装。
            self.versions.update(VersionMessage::SwitchOnline(version));
            return;
        }
        if version == "staging" {
            self.versions
                .update(VersionMessage::InstallBranch(version.clone()));
            self.versions.install_task.switch_requested = true;
            self.start_version_install_target(
                SillyTavernInstallTarget::Branch(version.clone()),
                version,
            );
        } else if let Some(release) = release {
            self.versions.update(VersionMessage::InstallOnline(version));
            // 即使规范目录已被删除，用户明确点击的“切换”也应在重新安装成功后生效。
            self.versions.install_task.switch_requested = true;
            self.start_version_install(release);
        }
    }

    /// 启动在线酒馆安装任务。弹窗先由状态更新显示，再在后台执行 git/npm。
    fn start_version_install(&mut self, release: crate::pages::versions::OnlineRelease) {
        self.start_version_install_target(
            SillyTavernInstallTarget::Tag(release.tag_name),
            release.version,
        );
    }

    fn start_version_install_target(
        &mut self,
        target_ref: SillyTavernInstallTarget,
        display_version: String,
    ) {
        if self.version_install_receiver.is_some() {
            return;
        }
        let channel = self.settings.download_channel;
        let target = crate::core::network::sillytavern_install_dir();
        let npm_registry = self.settings.npm_registry.url().to_owned();
        let proxy_mode = match self.settings.proxy_mode {
            ProxyMode::None => "none".to_owned(),
            ProxyMode::System => "system".to_owned(),
            ProxyMode::Custom => "custom".to_owned(),
        };
        let proxy_host = self.settings.custom_proxy.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        self.version_install_cancel = Some(cancel.clone());
        self.version_install_receiver = Some(receiver);
        if self.versions.install_task.version.as_deref() != Some(display_version.as_str()) {
            self.versions
                .update(VersionMessage::InstallDownloadStarted(display_version));
        }
        std::thread::spawn(move || {
            crate::core::network::run_sillytavern_install_with_cancel(
                target_ref,
                target,
                channel,
                npm_registry,
                proxy_mode,
                proxy_host,
                sender,
                cancel,
            );
        });
    }

    /// 消费在线版本请求结果。
    fn poll_version_catalog(&mut self) {
        let Some(receiver) = self.version_catalog_receiver.take() else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(catalog)) => self.apply_version_catalog(catalog),
            Ok(Err(error)) => self
                .versions
                .update(VersionMessage::OnlineVersionsFailed(error)),
            Err(TryRecvError::Empty) => {
                self.versions.update(VersionMessage::TickLoading);
                self.version_catalog_receiver = Some(receiver);
            }
            Err(TryRecvError::Disconnected) => self.versions.update(
                VersionMessage::OnlineVersionsFailed("版本任务已中断".to_owned()),
            ),
        }
    }

    /// 消费 git/npm 安装日志，并在成功后按规则切换当前在线实例。
    fn poll_version_install(&mut self) {
        let Some(receiver) = self.version_install_receiver.take() else {
            self.versions.update(VersionMessage::InstallTaskTick);
            return;
        };
        let mut keep = true;
        let mut completed_this_tick = false;
        while let Ok(event) = receiver.try_recv() {
            match event {
                SillyTavernInstallEvent::Log(log) => {
                    self.versions.update(VersionMessage::InstallLog(log));
                }
                SillyTavernInstallEvent::DownloadComplete => {
                    self.versions
                        .update(VersionMessage::InstallDownloadCompleted);
                }
                SillyTavernInstallEvent::InstallStarted => {
                    self.versions
                        .update(VersionMessage::InstallDependenciesStarted);
                }
                SillyTavernInstallEvent::Cancelled => {
                    self.versions
                        .update(VersionMessage::InstallFailed("安装已取消".to_owned()));
                    keep = false;
                }
                SillyTavernInstallEvent::Completed(result) => {
                    match result {
                        Ok(()) => {
                            self.versions.update(VersionMessage::InstallCompleted);
                            if let Some(installed) =
                                crate::core::network::installed_sillytavern_state()
                            {
                                self.versions.restore_installed(&installed);
                            }
                        }
                        Err(error) => self.versions.update(VersionMessage::InstallFailed(error)),
                    }
                    self.version_install_cancel = None;
                    completed_this_tick = true;
                    keep = false;
                }
            }
        }
        if keep {
            self.version_install_receiver = Some(receiver);
        }
        if !completed_this_tick {
            self.versions.update(VersionMessage::InstallTaskTick);
        }
        if !self.versions.install_task.visible {
            self.version_install_receiver = None;
            self.version_install_cancel = None;
        }
    }

    fn cancel_download_channel_test(&mut self) {
        if let Some(cancel) = &self.download_channel_test_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        self.download_channel_test_cancel = None;
        self.download_channel_test_receiver = None;
    }

    fn start_download_channel_test(&mut self) {
        if self.settings.download_channel_test.running {
            return;
        }
        if let Some(cancel) = &self.download_channel_test_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        let proxy_mode = match self.settings.proxy_mode {
            ProxyMode::None => "none",
            ProxyMode::System => "system",
            ProxyMode::Custom => "custom",
        };
        let proxy_host = self.settings.custom_proxy.clone();
        self.download_channel_test_cancel = Some(cancel.clone());
        self.download_channel_test_receiver = Some(receiver);
        self.settings.download_channel_test = DownloadChannelTestState {
            show: true,
            running: true,
            timed_out: false,
            all_failed: false,
            started_at: Some(Instant::now()),
            done_at: None,
            current_channel: None,
            clone_stage: None,
            clone_current: None,
            clone_total: None,
            clone_percentage: None,
            results: Vec::new(),
        };
        std::thread::spawn(move || {
            crate::core::network::run_download_channel_test(
                proxy_mode,
                &proxy_host,
                Some(sender),
                cancel,
            );
        });
    }

    fn poll_download_channel_test(&mut self, now: Instant) {
        const TEST_TIMEOUT: Duration = Duration::from_secs(60);
        const AUTO_CLOSE_DELAY: Duration = Duration::from_secs(3);
        if !self.settings.download_channel_test.running
            && self
                .settings
                .download_channel_test
                .done_at
                .is_some_and(|done_at| now.duration_since(done_at) >= AUTO_CLOSE_DELAY)
        {
            self.settings.download_channel_test.show = false;
            self.settings.download_channel_test.done_at = None;
            return;
        }
        if self.settings.download_channel_test.running
            && self
                .settings
                .download_channel_test
                .started_at
                .is_some_and(|started| now.duration_since(started) >= TEST_TIMEOUT)
        {
            self.settings.download_channel_test.running = false;
            self.settings.download_channel_test.timed_out = true;
            self.settings.download_channel_test.done_at = Some(now);
            if let Some(cancel) = &self.download_channel_test_cancel {
                cancel.store(true, Ordering::Relaxed);
            }
            self.download_channel_test_cancel = None;
            self.download_channel_test_receiver = None;
            return;
        }

        let Some(receiver) = self.download_channel_test_receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        loop {
            match receiver.try_recv() {
                Ok(DownloadChannelTestEvent::ChannelStarted { channel }) => {
                    self.settings.download_channel_test.current_channel = Some(channel);
                    self.settings.download_channel_test.clone_stage = None;
                    self.settings.download_channel_test.clone_current = None;
                    self.settings.download_channel_test.clone_total = None;
                    self.settings.download_channel_test.clone_percentage = None;
                }
                Ok(DownloadChannelTestEvent::CloneProgress {
                    channel,
                    stage,
                    current,
                    total,
                    percentage,
                }) => {
                    self.settings.download_channel_test.current_channel = Some(channel);
                    self.settings.download_channel_test.clone_stage = Some(stage);
                    self.settings.download_channel_test.clone_current = current;
                    self.settings.download_channel_test.clone_total = total;
                    self.settings.download_channel_test.clone_percentage = percentage;
                }
                Ok(DownloadChannelTestEvent::ChannelFinished(result)) => {
                    self.settings.download_channel_test.results.push(result);
                }
                Ok(DownloadChannelTestEvent::Completed {
                    selected,
                    results,
                    all_failed,
                }) => {
                    self.settings.download_channel_test.running = false;
                    self.settings.download_channel_test.done_at = Some(now);
                    self.settings.download_channel_test.all_failed = all_failed;
                    self.settings.download_channel_test.results = results.clone();
                    self.settings.download_resolved_channel = Some(selected);
                    self.settings.download_channel_last_tested = None;
                    match crate::core::network::save_download_channel_cache(selected, &results) {
                        Ok(cache) => {
                            self.settings.download_channel_last_tested = Some(cache.tested_at);
                        }
                        Err(error) => {
                            self.settings.save_error =
                                Some(format!("无法保存下载渠道缓存：{error}"));
                        }
                    }
                    self.download_channel_test_cancel = None;
                    self.persist_preferences();
                    keep_receiver = false;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    keep_receiver = false;
                    self.settings.download_channel_test.running = false;
                    self.settings.download_channel_test.timed_out = true;
                    self.settings.download_channel_test.done_at = Some(now);
                    break;
                }
            }
        }
        if keep_receiver {
            self.download_channel_test_receiver = Some(receiver);
        }
    }

    /// 统一更新启动模式，设置页为唯一状态源，主页仅调用同一入口进行快捷切换。
    fn apply_launch_mode(&mut self, mode: QuickStartMode) {
        match mode {
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
        }
    }

    fn start_github_test(&mut self) {
        if self.settings.github_test.running || self.settings.download_channel_test.running {
            return;
        }
        self.github_test_id = self.github_test_id.wrapping_add(1);
        let proxy_mode = match self.settings.proxy_mode {
            ProxyMode::None => "none",
            ProxyMode::System => "system",
            ProxyMode::Custom => "custom",
        };
        let proxy_host = self.settings.custom_proxy.clone();
        // GitHub 连接测试固定测试官方仓库，不受酒馆下载渠道选择影响。
        let selected_channel = DownloadChannel::Official;
        let accelerate_url = None;
        let proxy_address = match self.settings.proxy_mode {
            ProxyMode::None => None,
            ProxyMode::Custom => (!proxy_host.trim().is_empty()).then_some(proxy_host.clone()),
            ProxyMode::System => crate::core::network::read_system_proxy()
                .filter(|(_, enabled)| *enabled)
                .map(|(address, _)| address),
        };
        let mode_label = match self.settings.proxy_mode {
            ProxyMode::None => "直连".to_owned(),
            ProxyMode::System => "系统代理".to_owned(),
            ProxyMode::Custom => "自定义代理".to_owned(),
        };

        if let Some(cancel) = &self.github_test_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        self.github_test_cancel = Some(cancel.clone());
        self.github_test_receiver = Some(receiver);
        self.settings.github_test = GithubTestState {
            show: true,
            running: true,
            timed_out: false,
            results: None,
            error: None,
            mode_label: mode_label.to_owned(),
            proxy_address,
            accelerate_url: accelerate_url.clone(),
            started_at: Some(Instant::now()),
            current_key: None,
            current_name: None,
            clone_stage: None,
            clone_current: None,
            clone_total: None,
            clone_percentage: None,
            download_total_bytes: None,
            download_downloaded_bytes: 0,
            download_bytes_per_second: 0,
            download_percentage: None,
            live_items: [
                ("raw", "文件访问"),
                ("repo", "仓库访问"),
                ("homepage", "首页访问"),
                ("api", "API 访问"),
                ("clone", "仓库克隆"),
                ("speed", "下载速度"),
            ]
            .into_iter()
            .map(|(key, name)| GithubLiveItem {
                key: key.to_owned(),
                name: name.to_owned(),
                status: GithubLiveItemStatus::Running,
                result: None,
            })
            .collect(),
        };

        std::thread::spawn(move || {
            crate::core::network::run_github_test_with_cancel_for_channel(
                proxy_mode,
                &proxy_host,
                selected_channel,
                accelerate_url,
                true,
                Some(sender),
                cancel,
            );
        });
    }

    fn poll_github_test(&mut self, now: Instant) {
        const TEST_TIMEOUT: Duration = Duration::from_secs(60);

        if self.settings.github_test.running
            && self
                .settings
                .github_test
                .started_at
                .is_some_and(|started| now.duration_since(started) >= TEST_TIMEOUT)
        {
            self.settings.github_test.running = false;
            self.settings.github_test.timed_out = true;
            self.settings.github_test.results = Some(crate::core::network::timeout_results());
            self.settings.github_test.error = Some("GitHub 连接测试超过 60 秒。".to_owned());
            if let Some(cancel) = &self.github_test_cancel {
                cancel.store(true, Ordering::Relaxed);
            }
            self.github_test_cancel = None;
            self.github_test_receiver = None;
            return;
        }

        let Some(receiver) = self.github_test_receiver.take() else {
            return;
        };
        let mut keep_receiver = true;
        loop {
            match receiver.try_recv() {
                Ok(GithubTestEvent::ItemStarted { key, name }) => {
                    self.settings.github_test.current_key = Some(key.clone());
                    self.settings.github_test.current_name = Some(name.clone());
                    self.upsert_github_live_item(key, name, GithubLiveItemStatus::Running, None);
                }
                Ok(GithubTestEvent::CloneProgress {
                    stage,
                    current,
                    total,
                    percentage,
                }) => {
                    self.settings.github_test.current_key = Some("clone".to_owned());
                    self.settings.github_test.clone_stage = Some(stage);
                    self.settings.github_test.clone_current = current;
                    self.settings.github_test.clone_total = total;
                    self.settings.github_test.clone_percentage = percentage;
                }
                Ok(GithubTestEvent::DownloadProgress {
                    total_bytes,
                    downloaded_bytes,
                    bytes_per_second,
                    percentage,
                }) => {
                    self.settings.github_test.current_key = Some("speed".to_owned());
                    self.settings.github_test.download_total_bytes = total_bytes;
                    self.settings.github_test.download_downloaded_bytes = downloaded_bytes;
                    self.settings.github_test.download_bytes_per_second = bytes_per_second;
                    self.settings.github_test.download_percentage = percentage;
                }
                Ok(GithubTestEvent::ItemFinished(item)) => {
                    self.upsert_github_live_item(
                        item.key.clone(),
                        item.name.clone(),
                        GithubLiveItemStatus::Finished,
                        Some(item),
                    );
                }
                Ok(GithubTestEvent::Completed(results)) => {
                    self.settings.github_test.running = false;
                    self.settings.github_test.timed_out = false;
                    self.settings.github_test.results = Some(results);
                    self.settings.github_test.error = None;
                    self.settings.github_test.started_at = None;
                    self.github_test_cancel = None;
                    keep_receiver = false;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    keep_receiver = false;
                    if self.settings.github_test.running {
                        self.settings.github_test.running = false;
                        self.settings.github_test.results = None;
                        self.settings.github_test.error =
                            Some("GitHub 测试进程意外结束。".to_owned());
                        self.settings.github_test.started_at = None;
                    }
                    break;
                }
            }
        }
        if keep_receiver {
            self.github_test_receiver = Some(receiver);
        }
    }

    fn upsert_github_live_item(
        &mut self,
        key: String,
        name: String,
        status: GithubLiveItemStatus,
        result: Option<crate::core::network::GithubMultiTestItem>,
    ) {
        if let Some(item) = self
            .settings
            .github_test
            .live_items
            .iter_mut()
            .find(|item| item.key == key)
        {
            item.status = status;
            if result.is_some() {
                item.result = result;
            }
        } else {
            self.settings.github_test.live_items.push(GithubLiveItem {
                key,
                name,
                status,
                result,
            });
        }
    }

    fn start_environment_install(&mut self, dependency: EnvironmentDependency) {
        // 旧版 Homebrew 安装按钮本身就是占位入口，保持其原有行为。
        if dependency == EnvironmentDependency::Homebrew {
            return;
        }

        let (sender, receiver) = mpsc::channel();
        self.environment_task_receiver = Some(receiver);
        self.settings.environment_task = EnvironmentTaskState {
            dependency: Some(dependency),
            show: true,
            log: String::new(),
            running: true,
            done_at: None,
            started_at: Some(Instant::now()),
            timed_out: false,
            failed: false,
        };

        std::thread::spawn(move || match dependency {
            EnvironmentDependency::Git => {
                crate::core::settings::env_detect::run_brew_install("git", sender)
            }
            EnvironmentDependency::NodeJs => {
                crate::core::settings::env_detect::run_brew_install("node@24", sender)
            }
            EnvironmentDependency::Caddy => {
                crate::core::settings::env_detect::run_brew_install("caddy", sender)
            }
            EnvironmentDependency::Pm2 => {
                crate::core::settings::env_detect::run_npm_install_global("pm2", sender)
            }
            EnvironmentDependency::Homebrew => {}
        });
    }

    fn poll_environment_task(&mut self, now: Instant) {
        const TASK_TIMEOUT: Duration = Duration::from_secs(300);
        const AUTO_CLOSE_DELAY: Duration = Duration::from_secs(3);

        if self.settings.environment_task.running
            && self
                .settings
                .environment_task
                .started_at
                .is_some_and(|started| now.duration_since(started) >= TASK_TIMEOUT)
        {
            self.settings.environment_task.running = false;
            self.settings.environment_task.timed_out = true;
            self.settings.environment_task.done_at = None;
            self.environment_task_receiver = None;
            if !self.settings.environment_task.log.is_empty() {
                self.settings.environment_task.log.push('\n');
            }
            self.settings
                .environment_task
                .log
                .push_str("⏰ 安装超时，请稍后重试。");
            return;
        }

        let mut keep_receiver = true;
        if let Some(receiver) = self.environment_task_receiver.take() {
            loop {
                match receiver.try_recv() {
                    Ok(line) if line == "__FAILED__" => {
                        self.settings.environment_task.running = false;
                        self.settings.environment_task.failed = true;
                        self.settings.environment_task.done_at = None;
                        keep_receiver = false;
                        break;
                    }
                    Ok(line) if line == "__DONE__" => {
                        self.settings.environment_task.running = false;
                        if !self.settings.environment_task.log.is_empty() {
                            self.settings.environment_task.log.push('\n');
                        }
                        self.settings
                            .environment_task
                            .log
                            .push_str("✅ 安装完成，3 秒后自动关闭");
                        self.settings.environment_task.done_at = Some(now);
                        keep_receiver = false;
                        break;
                    }
                    Ok(line) => {
                        if let Some(error) = line.strip_prefix("__ERROR__:") {
                            if !self.settings.environment_task.log.is_empty() {
                                self.settings.environment_task.log.push('\n');
                            }
                            self.settings
                                .environment_task
                                .log
                                .push_str(&format!("❌ {error}"));
                            continue;
                        }
                        if let Some(version) = line.strip_prefix("__VERSION__:") {
                            if let Some(dependency) = self.settings.environment_task.dependency {
                                self.settings
                                    .environment
                                    .set(dependency, version.to_owned());
                            }
                            continue;
                        }
                        if !self.settings.environment_task.log.is_empty() {
                            self.settings.environment_task.log.push('\n');
                        }
                        self.settings.environment_task.log.push_str(&line);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        keep_receiver = false;
                        if self.settings.environment_task.running {
                            self.settings.environment_task.running = false;
                            self.settings.environment_task.failed = true;
                            self.settings.environment_task.timed_out = false;
                            self.settings
                                .environment_task
                                .log
                                .push_str("\n安装进程意外结束，请关闭窗口后重试。");
                        }
                        break;
                    }
                }
            }
            if keep_receiver {
                self.environment_task_receiver = Some(receiver);
            }
        }

        if self
            .settings
            .environment_task
            .done_at
            .is_some_and(|done_at| now.duration_since(done_at) >= AUTO_CLOSE_DELAY)
        {
            self.settings.environment_task.show = false;
            self.settings.environment_task.done_at = None;
            self.settings.environment_task.started_at = None;
        }
    }

    fn current_system_proxy_status() -> SystemProxyStatus {
        match crate::core::network::read_system_proxy() {
            Some((_, true)) => SystemProxyStatus::Enabled,
            Some((_, false)) => SystemProxyStatus::Disabled,
            None => SystemProxyStatus::Unknown,
        }
    }

    fn pick_directory(initial: &str) -> Option<String> {
        let dialog = rfd::FileDialog::new().set_directory(expand_home_path(initial));
        dialog
            .pick_folder()
            .map(|path| path.to_string_lossy().into_owned())
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
            proxy_mode: match self.settings.proxy_mode {
                ProxyMode::None => "none".to_owned(),
                ProxyMode::System => "system".to_owned(),
                ProxyMode::Custom => "custom".to_owned(),
            },
            custom_proxy: self.settings.custom_proxy.clone(),
            github_proxy_enabled: self.settings.github_proxy_enabled,
            github_proxy_url: self.settings.github_proxy_url.clone(),
            npm_registry: self.settings.npm_registry.url().to_owned(),
            download_channel: self.settings.download_channel.key().to_owned(),
            auto_start: self.settings.auto_start,
            data_mode: match self.settings.data_mode {
                TavernDataMode::Global => "global".to_owned(),
                TavernDataMode::Current => "current".to_owned(),
            },
            global_data_path: self.settings.global_data_path.clone(),
            tavern_export_path: self.settings.tavern_export_path.clone(),
            start_mode: match self.settings.start_mode {
                StartMode::Normal => "normal".to_owned(),
                StartMode::Desktop => "desktop".to_owned(),
            },
            server_mode_enabled: self.settings.server_mode_enabled,
            staging_risk_confirmed: self.versions.staging_risk_confirmed,
        };
        self.settings.save_error = self
            .settings_store
            .save(preferences)
            .err()
            .map(|error| error.to_string());
    }

    pub fn view(&self) -> Element<'_, Message> {
        crate::lang::set_language(effective_language(self.settings.language));
        let mut page = match self.screen {
            Screen::Init => self.init_view(),
            Screen::Main => self.main_view(),
        };
        if let Some(modal) = crate::pages::versions::local::modal_view(&self.versions.local) {
            page = iced::widget::stack![page, modal.map(Message::Version)].into();
        }
        if let Some(toast) = crate::pages::versions::local::toast_view(&self.versions.local) {
            page = iced::widget::stack![
                page,
                container(toast.map(Message::Version))
                    .width(Fill)
                    .align_x(Alignment::Center)
            ]
            .into();
        }
        if self.tavern.sync.close_prompt {
            page = iced::widget::stack![
                page,
                crate::pages::tavern::sync::close_overlay(&self.tavern.sync).map(Message::Tavern)
            ]
            .into();
        }
        page
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
            sidebar::sidebar(self.page, &self.versions),
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

fn format_version_sync_time(timestamp: u64) -> String {
    if timestamp == 0 {
        return "未知".to_owned();
    }
    // macOS 自带 date，使用本地时间显示缓存写入时间，不会触发网络请求。
    std::process::Command::new("date")
        .args(["-r", &timestamp.to_string(), "+%Y-%m-%d %H:%M"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| timestamp.to_string())
}

fn expand_home_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

#[cfg(test)]
mod tests {
    use super::{InitStage, Launcher, Message};
    use crate::core::network::{
        DownloadChannel, DownloadChannelTestEvent, DownloadChannelTestResult, GithubTestEvent,
    };
    use crate::core::settings::{PersistentPreferences, SettingsStore};
    use crate::pages::settings::{
        DisplayLanguage, DownloadChannelTestState, EnvironmentDependency, EnvironmentTaskState,
        GithubTestState, ProxyMode, QuickStartMode, StartMode, ThemeMode,
    };

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
        let _ = launcher.update(Message::SettingsLaunchModeSelected(QuickStartMode::Desktop));
        let _ = launcher.update(Message::SettingsRestoreDefaults);

        assert_eq!(launcher.settings.language, DisplayLanguage::System);
        assert_eq!(launcher.settings.start_mode, StartMode::Normal);
        assert!(launcher.settings.remember_window_position);
    }

    #[test]
    fn environment_task_failure_is_not_reported_as_success() {
        let mut launcher = launcher();
        let (sender, receiver) = std::sync::mpsc::channel();
        let now = iced::time::Instant::now();
        launcher.environment_task_receiver = Some(receiver);
        launcher.settings.environment_task = EnvironmentTaskState {
            dependency: Some(EnvironmentDependency::Git),
            show: true,
            log: String::new(),
            running: true,
            done_at: None,
            started_at: Some(now),
            timed_out: false,
            failed: false,
        };
        sender
            .send("__ERROR__:命令执行失败（退出码：7）".into())
            .expect("send error");
        sender.send("__FAILED__".into()).expect("send failure");

        let _ = launcher.update(Message::EnvironmentTaskTick(now));
        assert!(launcher.settings.environment_task.failed);
        assert!(!launcher.settings.environment_task.running);
        assert!(
            launcher
                .settings
                .environment_task
                .log
                .contains("命令执行失败")
        );
        assert!(launcher.settings.environment_task.done_at.is_none());
    }

    #[test]
    fn github_test_timeout_keeps_results_and_does_not_report_success() {
        let mut launcher = launcher();
        let now = iced::time::Instant::now();
        launcher.settings.github_test = GithubTestState {
            show: true,
            running: true,
            mode_label: "直连".into(),
            started_at: Some(now),
            ..GithubTestState::default()
        };

        let _ = launcher.update(Message::GithubTestTick(
            now + iced::time::Duration::from_secs(61),
        ));
        assert!(!launcher.settings.github_test.running);
        assert!(launcher.settings.github_test.timed_out);
        assert_eq!(
            launcher
                .settings
                .github_test
                .results
                .as_ref()
                .unwrap()
                .len(),
            6
        );
    }

    #[test]
    fn closing_github_test_invalidates_the_current_receiver() {
        let mut launcher = launcher();
        launcher.settings.github_test.show = true;
        launcher.settings.github_test.running = true;
        let old_id = launcher.github_test_id;

        let _ = launcher.update(Message::GithubTestClose);
        assert!(!launcher.settings.github_test.show);
        assert!(launcher.github_test_id != old_id);
        assert!(launcher.github_test_receiver.is_none());
    }

    #[test]
    fn github_progress_events_update_clone_and_download_state() {
        let mut launcher = launcher();
        let (sender, receiver) = std::sync::mpsc::channel();
        let now = iced::time::Instant::now();
        launcher.github_test_receiver = Some(receiver);
        launcher.settings.github_test = GithubTestState {
            show: true,
            running: true,
            started_at: Some(now),
            live_items: [("clone", "仓库克隆"), ("speed", "下载速度")]
                .into_iter()
                .map(|(key, name)| crate::pages::settings::GithubLiveItem {
                    key: key.into(),
                    name: name.into(),
                    status: crate::pages::settings::GithubLiveItemStatus::Running,
                    result: None,
                })
                .collect(),
            ..GithubTestState::default()
        };
        sender
            .send(GithubTestEvent::CloneProgress {
                stage: "Receiving objects".into(),
                current: Some(42),
                total: Some(100),
                percentage: Some(42.0),
            })
            .expect("send clone progress");
        sender
            .send(GithubTestEvent::DownloadProgress {
                total_bytes: Some(10_000),
                downloaded_bytes: 2_500,
                bytes_per_second: 1_024,
                percentage: Some(25.0),
            })
            .expect("send download progress");

        let _ = launcher.update(Message::GithubTestTick(now));
        assert_eq!(
            launcher.settings.github_test.clone_stage.as_deref(),
            Some("Receiving objects")
        );
        assert_eq!(launcher.settings.github_test.clone_percentage, Some(42.0));
        assert_eq!(
            launcher.settings.github_test.download_total_bytes,
            Some(10_000)
        );
        assert_eq!(
            launcher.settings.github_test.download_downloaded_bytes,
            2_500
        );
        assert_eq!(
            launcher.settings.github_test.download_bytes_per_second,
            1_024
        );
        assert_eq!(
            launcher.settings.github_test.download_percentage,
            Some(25.0)
        );
    }

    #[test]
    fn environment_task_applies_version_and_auto_closes() {
        let mut launcher = launcher();
        let (sender, receiver) = std::sync::mpsc::channel();
        let now = iced::time::Instant::now();
        launcher.environment_task_receiver = Some(receiver);
        launcher.settings.environment_task = EnvironmentTaskState {
            dependency: Some(EnvironmentDependency::Git),
            show: true,
            log: String::new(),
            running: true,
            done_at: None,
            started_at: Some(now),
            timed_out: false,
            failed: false,
        };
        sender
            .send("__VERSION__:2.47.0".into())
            .expect("send version");
        sender.send("__DONE__".into()).expect("send completion");

        let _ = launcher.update(Message::EnvironmentTaskTick(now));
        assert_eq!(launcher.settings.environment.git.as_deref(), Some("2.47.0"));
        assert!(!launcher.settings.environment_task.running);
        assert!(launcher.settings.environment_task.show);

        let _ = launcher.update(Message::EnvironmentTaskTick(
            now + iced::time::Duration::from_secs(4),
        ));
        assert!(!launcher.settings.environment_task.show);
    }

    #[test]
    fn homebrew_install_keeps_the_old_placeholder_behavior() {
        let mut launcher = launcher();
        let _ = launcher.update(Message::EnvironmentInstall(EnvironmentDependency::Homebrew));
        assert!(launcher.environment_task_receiver.is_none());
        assert!(!launcher.settings.environment_task.show);
    }

    #[test]
    fn download_channel_result_is_cached_and_modal_auto_closes() {
        let mut launcher = launcher();
        let (sender, receiver) = std::sync::mpsc::channel();
        let now = iced::time::Instant::now();
        launcher.download_channel_test_receiver = Some(receiver);
        launcher.settings.download_channel_test = DownloadChannelTestState {
            show: true,
            running: true,
            started_at: Some(now),
            ..DownloadChannelTestState::default()
        };
        sender
            .send(DownloadChannelTestEvent::Completed {
                selected: DownloadChannel::Mirror1,
                results: vec![
                    DownloadChannelTestResult {
                        channel: DownloadChannel::Mirror1,
                        success: true,
                        latency_ms: Some(100),
                        error: None,
                    },
                    DownloadChannelTestResult {
                        channel: DownloadChannel::Mirror2,
                        success: true,
                        latency_ms: Some(200),
                        error: None,
                    },
                    DownloadChannelTestResult {
                        channel: DownloadChannel::Official,
                        success: true,
                        latency_ms: Some(300),
                        error: None,
                    },
                ],
                all_failed: false,
            })
            .expect("send channel result");

        let _ = launcher.update(Message::DownloadChannelTestTick(now));
        assert_eq!(
            launcher.settings.download_resolved_channel,
            Some(DownloadChannel::Mirror1)
        );
        // 测试环境可能限制写入用户级 Caches 目录；运行时会在 macOS 用户目录中写入缓存。
        assert!(launcher.settings.download_channel_test.show);
        assert!(launcher.settings.download_channel_test.done_at.is_some());

        let _ = launcher.update(Message::DownloadChannelTestTick(
            now + iced::time::Duration::from_secs(4),
        ));
        assert!(!launcher.settings.download_channel_test.show);
    }

    #[test]
    fn proxy_mode_is_saved_and_restored() {
        let path = test_path("proxy-mode");
        let (store, _) = SettingsStore::load(&path);
        let mut launcher = Launcher::new(store, PersistentPreferences::default()).0;
        let _ = launcher.update(Message::SettingsProxyModeSelected(ProxyMode::System));

        let (_, preferences) = SettingsStore::load(&path);
        assert_eq!(preferences.proxy_mode, "system");
        let (store, preferences) = SettingsStore::load(&path);
        let restored = Launcher::new(store, preferences).0;
        assert_eq!(restored.settings.proxy_mode, ProxyMode::System);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn disabling_window_restore_clears_saved_coordinate() {
        let path = test_path("window-position");
        let (store, _) = SettingsStore::load(&path);
        let mut launcher = Launcher::new(store, PersistentPreferences::default()).0;

        let _ = launcher.update(Message::WindowMoved(iced::Point::new(-320.0, 96.0)));
        let _ = launcher.update(Message::SettingsRememberWindowPosition(false));

        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("read persisted settings"))
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
        assert_eq!(
            launcher.theme().palette().background,
            crate::theme::light_theme().palette().background
        );

        let _ = launcher.update(Message::SettingsThemeSelected(ThemeMode::System));
        assert_eq!(
            launcher.theme().palette().background,
            crate::theme::dark_theme().palette().background
        );
    }
}
