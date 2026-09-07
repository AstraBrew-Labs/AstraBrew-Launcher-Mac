//! 版本管理页面。
//!
//! 本文件只负责版本页的界面状态、消息定义和视图渲染。
//! 网络请求、Git 操作和 npm 进程由上层应用接入这些公开状态与消息。

use iced::widget::{
    button, column, container, image, markdown, mouse_area, row, rule, scrollable, space, stack,
    tooltip,
};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use crate::lang::text;
use crate::theme::button_style;
use astra_ui::{
    BLUE_600, ButtonVariant, DANGER, INK_MUTED, INK_SUBTLE, SUCCESS, WHITE, icons,
};

/// 版本页当前展示的实例类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionTab {
    #[default]
    Local,
    Online,
}

/// 在线实例使用的酒馆代码分支。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TavernBranch {
    /// 稳定发行分支，对应 Git 的 `release`。
    #[default]
    Release,
    /// 开发分支，对应 Git 的 `staging`。
    Staging,
}

impl TavernBranch {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Staging => "staging",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Release => "稳定版",
            Self::Staging => "开发版",
        }
    }
}

/// 当前正在使用的实例来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionSource {
    /// 用户扫描或导入的本地实例。
    #[default]
    Local,
    /// 启动器管理目录中的在线实例。
    Online,
}

// 这些方法是侧边栏和主应用的集成接口，当前文件单独编译时可能暂未被调用。
#[allow(dead_code)]
impl VersionSource {
    /// 返回侧边栏可以直接使用的来源文本。
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "本地",
            Self::Online => "在线",
        }
    }

    /// 返回来源对应的视觉颜色。
    pub const fn color(self) -> Color {
        match self {
            Self::Local => Color::from_rgb8(23, 201, 100),
            Self::Online => BLUE_600,
        }
    }
}

/// 在线版本列表的加载状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnlineVersionsStatus {
    /// 尚未开始或正在获取版本列表。
    Loading,
    /// 已经取得网络数据或缓存数据。
    #[default]
    Ready,
    /// 网络不可用，但当前没有可以展示的缓存。
    Error,
    /// 网络失败，当前正在展示过期缓存。
    StaleCache,
}

/// 在线酒馆安装流程的当前步骤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallPhase {
    /// 执行 git clone / fetch / checkout。
    #[default]
    Download,
    /// 下载完成后的三秒等待期。
    WaitingInstall,
    /// 执行 npm install。
    Install,
    /// 两个步骤均已成功。
    Completed,
    /// 任一步骤失败。
    Failed,
}

/// 在线酒馆安装弹窗状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTaskState {
    /// 弹窗是否显示。
    pub visible: bool,
    /// 当前安装的版本号。
    pub version: Option<String>,
    /// 当前步骤。
    pub phase: InstallPhase,
    /// 是否仍有后台任务运行。
    pub running: bool,
    /// 下载和 npm 安装的完整日志。
    pub logs: String,
    /// 失败原因，供上层任务回传后显示。
    pub error: Option<String>,
    /// 完成后是否允许关闭弹窗。
    pub can_close: bool,
    /// 完成后的自动关闭倒计时，单位为 UI tick。
    pub auto_close_ticks: u8,
    /// 用户是否明确点击了“切换版本”，安装完成后应立即切换当前实例。
    pub switch_requested: bool,
}

impl Default for InstallTaskState {
    fn default() -> Self {
        Self {
            visible: false,
            version: None,
            phase: InstallPhase::Download,
            running: false,
            logs: String::new(),
            error: None,
            can_close: false,
            auto_close_ticks: 0,
            switch_requested: false,
        }
    }
}

pub(crate) mod local;
pub use crate::core::local_instances::{DependencyStatus, LocalInstance};

/// 可从远端下载的酒馆发行版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnlineRelease {
    /// 展示用的版本号，例如 `1.18.0`。
    pub version: String,
    /// GitHub 的原始 tag，例如 `release/1.18.0`。
    pub tag_name: String,
    pub published_at: String,
    pub created_at: String,
    /// GitHub Release body 的 Markdown 原文。
    pub body: String,
    pub summary: String,
    /// 是否已经安装到启动器管理目录。
    pub installed: bool,
    /// 当前设置的下载源是否存在对应 tag。
    pub mirror_available: bool,
}

/// 版本管理页面的本地界面状态。
#[derive(Debug, Clone)]
pub struct VersionState {
    pub active_tab: VersionTab,
    pub branch: TavernBranch,
    /// 首次切换开发版前必须确认一次风险，确认后本次运行不再重复提示。
    pub staging_risk_confirmed: bool,
    pub staging_confirm_visible: bool,
    pub current_version: Option<String>,
    /// 当前实例路径；在线实例使用启动器规范目录。
    pub current_path: Option<String>,
    /// 明确记录当前来源，禁止仅通过 current_path 推断来源。
    pub current_source: Option<VersionSource>,
    /// 在线实例规范路径，安装服务完成后由上层写入。
    pub online_instance_path: Option<String>,
    /// 为已有扩展页面保留的最新版本字段。
    pub latest_version: String,
    pub local_instances: Vec<LocalInstance>,
    pub local: local::LocalUiState,
    pub online_releases: Vec<OnlineRelease>,
    pub online_status: OnlineVersionsStatus,
    pub branch_loaded: bool,
    pub selected_online_version: Option<String>,
    pub selection_modal_open: bool,
    pub release_log_version: Option<String>,
    pub markdown_items: Vec<markdown::Item>,
    pub staging: Option<crate::core::network::SillyTavernStaging>,
    pub staging_installed: bool,
    /// 规范目录是否已经存在在线酒馆，即使当前分支/版本不是选中的目标。
    pub online_instance_exists: bool,
    pub last_sync: String,
    pub notice: Option<String>,
    pub install_task: InstallTaskState,
    /// 安装开始前是否已经存在本地实例，用于关闭弹窗后的自动切换决策。
    pub had_local_instance_before_install: bool,
    /// 用于 Loading 文案动画的帧号；由上层定时发送 TickLoading。
    pub loading_frame: u8,
}

impl Default for VersionState {
    fn default() -> Self {
        let online_releases = vec![
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
        ];

        Self {
            active_tab: VersionTab::Local,
            branch: TavernBranch::Release,
            staging_risk_confirmed: false,
            staging_confirm_visible: false,
            current_version: None,
            current_path: None,
            current_source: None,
            online_instance_path: None,
            latest_version: online_releases
                .first()
                .map(|item| item.version.clone())
                .unwrap_or_default(),
            local_instances: Vec::new(),
            local: local::LocalUiState::default(),
            online_releases,
            online_status: OnlineVersionsStatus::Ready,
            branch_loaded: false,
            selected_online_version: Some("1.18.0".into()),
            selection_modal_open: false,
            release_log_version: None,
            markdown_items: Vec::new(),
            staging: None,
            staging_installed: false,
            online_instance_exists: false,
            last_sync: "2026/08/30 21:37".into(),
            notice: None,
            install_task: InstallTaskState::default(),
            had_local_instance_before_install: false,
            loading_frame: 0,
        }
    }
}

fn release(version: &str, published_at: &str, created_at: &str, summary: &str) -> OnlineRelease {
    OnlineRelease {
        version: version.into(),
        tag_name: format!("v{version}"),
        published_at: published_at.into(),
        created_at: created_at.into(),
        body: summary.into(),
        summary: summary.into(),
        installed: false,
        mirror_available: true,
    }
}

/// 版本页产生的用户操作，以及上层服务回传的异步任务事件。
#[derive(Debug, Clone)]
// 新增的异步事件由 app.rs 接入；在只实现版本页的阶段允许暂未构造的消息存在。
#[allow(dead_code)]
pub enum VersionMessage {
    SelectTab(VersionTab),
    ImportLocal,
    ScanLocal,
    OpenScanLog,
    CloseScanLog,
    CancelScan,
    RequestCancelScan,
    KeepScanning,
    ToggleScanDetails,
    RecheckLocalDependencies(String),
    CloseLocalInstall,
    DismissLocalToast,
    LocalModalInteract,
    InstallLocalDependencies(String),
    SwitchLocal(String),
    RemoveLocal(String),
    /// 切换在线实例分支。
    SelectBranch(TavernBranch),
    /// 确认首次使用开发版的风险提示。
    ConfirmStagingRisk,
    /// 取消开发版风险确认。
    CancelStagingRisk,
    /// 开始获取版本列表；上层应先尝试未过期缓存，再请求镜像和直连。
    RefreshOnline,
    /// 上层将接口或缓存解析结果提交给页面。
    OnlineVersionsLoaded {
        branch: TavernBranch,
        releases: Vec<OnlineRelease>,
        staging: Option<crate::core::network::SillyTavernStaging>,
        last_sync: String,
        from_cache: bool,
    },
    /// 上层在无可用缓存时回传错误。
    OnlineVersionsFailed(String),
    /// Loading 文案动画 tick。
    TickLoading,
    /// 打开/关闭稳定版本选择弹窗。
    OpenVersionSelector,
    CloseVersionSelector,
    SelectOnlineVersion(String),
    /// 打开/关闭 Release Markdown 日志弹窗。
    OpenReleaseLog(String),
    CloseReleaseLog,
    MarkdownLinkClicked(markdown::Uri),
    /// 安装指定分支。
    InstallBranch(String),

    /// 用户点击在线安装/更新按钮。
    InstallOnline(String),
    SwitchOnline(String),
    DeleteOnline(String),
    /// 下载阶段开始。
    InstallDownloadStarted(String),
    /// 上层将下载日志追加到弹窗。
    InstallLog(String),
    /// 下载成功后，等待三秒再进入 npm 安装阶段。
    InstallDownloadCompleted,
    /// 安装阶段开始，registry 等设置由上层进程使用。
    InstallDependenciesStarted,
    /// npm install 完成。
    InstallCompleted,
    /// 任一步骤失败。
    InstallFailed(String),
    /// 安装任务轮询；完成后由页面驱动倒计时关闭。
    InstallTaskTick,
    /// 弹窗内部点击/遮罩点击。运行中故意不执行任何操作。
    InstallModalInteract,
    /// 只允许在成功或失败状态关闭弹窗。
    CloseInstallModal,
}

impl VersionState {
    /// 更新页面本地状态；真实服务接入时由 app 将异步结果映射到这些消息。
    pub fn update(&mut self, message: VersionMessage) {
        match message {
            VersionMessage::SelectTab(tab) => self.active_tab = tab,
            VersionMessage::SelectBranch(branch) => {
                if branch == TavernBranch::Staging && !self.staging_risk_confirmed {
                    self.staging_confirm_visible = true;
                } else {
                    self.branch = branch;
                    self.branch_loaded = false;
                    self.online_status = OnlineVersionsStatus::Loading;
                    self.selection_modal_open = false;
                    self.release_log_version = None;
                }
            }
            VersionMessage::ConfirmStagingRisk => {
                self.staging_risk_confirmed = true;
                self.staging_confirm_visible = false;
                self.branch = TavernBranch::Staging;
                self.branch_loaded = false;
                self.online_status = OnlineVersionsStatus::Loading;
            }
            VersionMessage::CancelStagingRisk => self.staging_confirm_visible = false,
            // 文件选择、扫描和依赖任务只由应用层执行；状态层不能伪造成功。
            VersionMessage::ImportLocal
            | VersionMessage::ScanLocal
            | VersionMessage::CancelScan
            | VersionMessage::RecheckLocalDependencies(_)
            | VersionMessage::InstallLocalDependencies(_)
            | VersionMessage::LocalModalInteract => {}
            VersionMessage::OpenScanLog => {
                self.local.scan.visible = true;
                self.local.scan.show_details = true;
                self.local.scan.auto_hide_at = None;
            }
            VersionMessage::RequestCancelScan => {
                if self.local.scan.phase.active() {
                    self.local.scan.visible = true;
                    self.local.scan.cancel_confirm_visible = true;
                }
            }
            VersionMessage::KeepScanning => self.local.scan.cancel_confirm_visible = false,
            VersionMessage::ToggleScanDetails => {
                self.local.scan.show_details = !self.local.scan.show_details;
                self.local.scan.auto_hide_at = None;
            }
            VersionMessage::CloseScanLog => self.local.close_scan(),
            VersionMessage::CloseLocalInstall => {
                if !self.local.install.running {
                    self.local.install.visible = false;
                }
            }
            VersionMessage::DismissLocalToast => self.local.toast = None,
            VersionMessage::SwitchLocal(path) => {
                if let Some(instance) = self.local_instances.iter().find(|instance| {
                    instance.path == path && instance.dependencies == DependencyStatus::Ready
                }) {
                    self.current_version = Some(instance.version.clone());
                    self.current_path = Some(instance.path.clone());
                    self.current_source = Some(VersionSource::Local);
                    self.local
                        .notify("已切换到本地实例。", &instance.path, false);
                }
            }
            VersionMessage::RemoveLocal(path) => {
                let is_current = self.current_source == Some(VersionSource::Local)
                    && self.current_path.as_deref() == Some(path.as_str());
                if is_current {
                    self.local
                        .notify("当前正在使用的实例不能从列表中移除。", "", true);
                } else if !self.local_instances.iter().any(|item| {
                    item.path == path && item.dependencies == DependencyStatus::Installing
                }) {
                    self.local_instances
                        .retain(|instance| instance.path != path);
                    self.local.notify("已从本地实例列表移除。", "", false);
                }
            }
            VersionMessage::RefreshOnline => {
                self.online_status = OnlineVersionsStatus::Loading;
                self.loading_frame = 0;
                self.notice = Some("正在获取在线版本…".into());
            }
            VersionMessage::OnlineVersionsLoaded {
                branch,
                mut releases,
                staging,
                last_sync,
                from_cache,
            } => {
                self.branch = branch;
                self.branch_loaded = true;
                self.staging = staging;

                // 网络层负责按发布时间排序；这里再做一次稳定排序，避免服务端顺序变化导致选择跳动。
                releases.sort_by(|left, right| {
                    right
                        .published_at
                        .cmp(&left.published_at)
                        .then_with(|| right.version.cmp(&left.version))
                });
                self.latest_version = releases
                    .first()
                    .map(|item| item.version.clone())
                    .unwrap_or_default();
                if self
                    .selected_online_version
                    .as_ref()
                    .is_none_or(|version| !releases.iter().any(|item| &item.version == version))
                {
                    self.selected_online_version =
                        releases.first().map(|item| item.version.clone());
                }
                // 上层已经按磁盘 Git 状态标记 installed，不能再用旧列表覆盖新检测结果。
                self.online_releases = releases;
                self.last_sync = last_sync;
                self.online_status = if from_cache {
                    OnlineVersionsStatus::StaleCache
                } else {
                    OnlineVersionsStatus::Ready
                };
                self.notice = Some(if from_cache {
                    "在线版本请求失败，当前使用旧缓存。".into()
                } else {
                    "在线版本列表已更新。".into()
                });
            }
            VersionMessage::OnlineVersionsFailed(error) => {
                // 记录本次请求已经结束，重新进入页面不应自动重复请求；用户可点击刷新重试。
                self.branch_loaded = true;
                self.online_status = OnlineVersionsStatus::Error;
                self.notice = Some(format!("在线版本获取失败：{error}"));
            }
            VersionMessage::TickLoading => {
                self.loading_frame = self.loading_frame.wrapping_add(1) % 4;
            }
            VersionMessage::OpenVersionSelector => {
                if self.branch == TavernBranch::Release && !self.online_releases.is_empty() {
                    self.selection_modal_open = true;
                }
            }
            VersionMessage::CloseVersionSelector => self.selection_modal_open = false,
            VersionMessage::SelectOnlineVersion(version) => {
                if self
                    .online_releases
                    .iter()
                    .any(|release| release.version == version)
                {
                    self.selected_online_version = Some(version);
                    self.selection_modal_open = false;
                }
            }
            VersionMessage::OpenReleaseLog(version) => {
                if let Some(release) = self
                    .online_releases
                    .iter()
                    .find(|release| release.version == version)
                {
                    self.release_log_version = Some(version);
                    self.markdown_items = markdown::parse(&release.body).collect();
                }
            }
            VersionMessage::CloseReleaseLog => self.release_log_version = None,
            VersionMessage::MarkdownLinkClicked(_uri) => {}
            VersionMessage::InstallOnline(version) => self.begin_online_install(version),
            VersionMessage::InstallBranch(branch) => self.begin_branch_install(branch),
            VersionMessage::SwitchOnline(version) => {
                if self.is_online_installed(&version) {
                    self.current_version = Some(version.clone());
                    if version == "staging" {
                        self.branch = TavernBranch::Staging;
                    } else {
                        self.branch = TavernBranch::Release;
                        self.selected_online_version = Some(version.clone());
                    }
                    let path = online_instance_path();
                    self.current_path = Some(path.clone());
                    self.current_source = Some(VersionSource::Online);
                    self.online_instance_path = Some(path);
                    self.notice = Some(format!("已切换到在线安装版本 v{version}。"));
                    self.local.notify("已切换到在线实例。", version, false);
                } else {
                    self.local
                        .notify("在线实例尚未就绪，请重新选择版本。", version, true);
                }
            }
            VersionMessage::DeleteOnline(version) => {
                let is_current = self.current_source == Some(VersionSource::Online)
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
            VersionMessage::InstallDownloadStarted(version) => {
                self.install_task.visible = true;
                self.install_task.running = true;
                self.install_task.phase = InstallPhase::Download;
                self.install_task.version = Some(version);
                self.install_task.can_close = false;
                self.install_task.error = None;
            }
            VersionMessage::InstallLog(log) => {
                append_log(&mut self.install_task.logs, &log);
            }
            VersionMessage::InstallDownloadCompleted => {
                self.install_task.phase = InstallPhase::WaitingInstall;
                self.install_task.running = true;
                append_log(
                    &mut self.install_task.logs,
                    "下载完成，3 秒后开始安装 npm 依赖…",
                );
            }
            VersionMessage::InstallDependenciesStarted => {
                self.install_task.phase = InstallPhase::Install;
                self.install_task.running = true;
            }
            VersionMessage::InstallCompleted => {
                let version = self.install_task.version.clone();
                if let Some(version) = version.as_deref() {
                    if version == "staging" {
                        self.staging_installed = true;
                    }
                    // 在线实例共用同一个规范目录，因此只能有一个实际已检出的版本。
                    for release in &mut self.online_releases {
                        release.installed = release.version == version;
                    }
                }
                self.install_task.phase = InstallPhase::Completed;
                self.install_task.running = false;
                self.install_task.can_close = true;
                self.install_task.auto_close_ticks = 3;
                append_log(&mut self.install_task.logs, "npm 依赖安装完成。");
            }
            VersionMessage::InstallFailed(error) => {
                self.install_task.phase = InstallPhase::Failed;
                self.install_task.running = false;
                self.install_task.can_close = true;
                self.install_task.error = Some(error.clone());
                append_log(&mut self.install_task.logs, &format!("安装失败：{error}"));
            }
            VersionMessage::InstallTaskTick => {
                if self.install_task.phase == InstallPhase::Completed
                    && self.install_task.auto_close_ticks > 0
                {
                    self.install_task.auto_close_ticks -= 1;
                    if self.install_task.auto_close_ticks == 0 {
                        self.close_install_modal();
                    }
                }
            }
            VersionMessage::InstallModalInteract => {
                // 安装期间弹窗不可关闭，遮罩点击也不能取消后台进程。
            }
            VersionMessage::CloseInstallModal => {
                if self.install_task.can_close {
                    self.close_install_modal();
                }
            }
        }
    }

    fn begin_branch_install(&mut self, branch: String) {
        if self.install_task.running {
            self.notice = Some("已有在线酒馆安装任务正在执行。".into());
            return;
        }
        self.had_local_instance_before_install = !self.local_instances.is_empty();
        self.install_task = InstallTaskState {
            visible: true,
            version: Some(branch.clone()),
            phase: InstallPhase::Download,
            running: true,
            logs: format!("准备切换 SillyTavern {branch} 分支\n"),
            error: None,
            can_close: false,
            auto_close_ticks: 0,
            switch_requested: self.online_instance_exists,
        };
        self.notice = Some(format!("开始切换到 {branch} 分支。"));
    }

    fn begin_online_install(&mut self, version: String) {
        if self.install_task.running {
            self.notice = Some("已有在线酒馆安装任务正在执行。".into());
            return;
        }
        let Some(release) = self
            .online_releases
            .iter()
            .find(|release| release.version == version)
        else {
            self.notice = Some(format!("未找到在线版本 v{version}。"));
            return;
        };

        self.selected_online_version = Some(version.clone());
        self.had_local_instance_before_install = !self.local_instances.is_empty();
        self.install_task = InstallTaskState {
            visible: true,
            version: Some(version.clone()),
            phase: InstallPhase::Download,
            running: true,
            logs: format!("准备安装 SillyTavern v{version}\n"),
            error: None,
            can_close: false,
            auto_close_ticks: 0,
            switch_requested: self.online_instance_exists,
        };
        self.notice = Some(format!("开始安装在线版本 v{version}。"));
        // 镜像 tag 不存在时由上层网络服务自动改用官方 GitHub 地址。
        if !release.mirror_available {
            append_log(
                &mut self.install_task.logs,
                "当前下载源没有同步此版本，将使用官方直连地址。",
            );
        }
    }

    fn close_install_modal(&mut self) {
        let completed = self.install_task.phase == InstallPhase::Completed;
        let version = self.install_task.version.clone();
        self.install_task.visible = false;
        self.install_task.running = false;
        self.install_task.can_close = false;
        self.install_task.auto_close_ticks = 0;

        // 只有安装开始前没有本地实例时，关闭成功弹窗才自动切到在线实例。
        if completed
            && (self.install_task.switch_requested || !self.had_local_instance_before_install)
        {
            if let Some(version) = version {
                self.current_version = Some(version);
                let path = online_instance_path();
                self.current_path = Some(path.clone());
                self.current_source = Some(VersionSource::Online);
                self.online_instance_path = Some(path);
                self.active_tab = VersionTab::Online;
            }
        }
    }

    /// 恢复持久化的开发版风险确认标记。
    pub fn set_staging_risk_confirmed(&mut self, confirmed: bool) {
        self.staging_risk_confirmed = confirmed;
    }

    /// 用磁盘上已经存在的 Git 状态恢复重启后的在线实例。
    pub fn restore_installed(&mut self, installed: &crate::core::network::InstalledSillyTavern) {
        self.online_instance_exists = true;
        if let Some(tag) = installed.tag_name.as_deref() {
            self.current_version = Some(tag.trim_start_matches(['v', 'V']).to_owned());
            self.current_path = Some(online_instance_path());
            self.current_source = Some(VersionSource::Online);
            self.online_instance_path = self.current_path.clone();
        } else if installed.branch.as_deref() == Some("staging") {
            self.current_version = Some("staging".into());
            self.current_path = Some(online_instance_path());
            self.current_source = Some(VersionSource::Online);
            self.online_instance_path = self.current_path.clone();
            self.branch = TavernBranch::Staging;
            self.staging_installed = true;
        } else if installed.branch.as_deref() == Some("release") {
            // 分支切换到稳定版时可能没有精确 tag，仍然要恢复在线实例来源。
            self.current_version = Some("release".into());
            self.current_path = Some(online_instance_path());
            self.current_source = Some(VersionSource::Online);
            self.online_instance_path = self.current_path.clone();
            self.branch = TavernBranch::Release;
            self.staging_installed = false;
        }
    }

    /// 设置规范在线目录是否存在，用于把另一个分支的安装按钮显示为“切换”。
    pub fn set_online_instance_exists(&mut self, exists: bool) {
        self.online_instance_exists = exists;
    }

    /// 将磁盘检测到的 staging 状态同步到当前页面。
    pub fn set_staging_installed(&mut self, installed: bool) {
        self.staging_installed = installed;
    }

    /// 同步在线目录的实际状态，不修改当前选中的本地实例。
    /// 点击切换和目录刷新共用同一判断，防止磁盘结果与 UI 缓存各自作出相反结论。
    pub fn sync_online_installation(
        &mut self,
        installed: Option<&crate::core::network::InstalledSillyTavern>,
    ) {
        self.set_online_instance_exists(installed.is_some());
        self.set_staging_installed(
            installed.and_then(|item| item.branch.as_deref()) == Some("staging"),
        );
        let tag = installed.and_then(|item| item.tag_name.as_deref());
        for release in &mut self.online_releases {
            release.installed = !self.staging_installed && tag == Some(release.tag_name.as_str());
        }
        self.online_instance_path = installed.map(|_| online_instance_path());
    }

    pub fn is_online_installed(&self, version: &str) -> bool {
        if version == "staging" {
            return self.staging_installed;
        }
        self.online_releases
            .iter()
            .any(|release| release.version == version && release.installed)
    }
}

fn append_log(logs: &mut String, line: &str) {
    if !logs.is_empty() && !logs.ends_with('\n') {
        logs.push('\n');
    }
    logs.push_str(line);
    if !logs.ends_with('\n') {
        logs.push('\n');
    }
}

/// 在线酒馆统一安装目录，与 ai.md 中的目录规范保持一致。
fn online_instance_path() -> String {
    // 安装服务会在这个绝对路径中执行 Git 与 npm 命令。
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    format!("{home}/Library/Application Support/AstraBrew Launcher/sillytavern")
}

/// 渲染版本管理页面。
pub fn versions_view(state: &VersionState) -> Element<'_, VersionMessage> {
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

    let mut page: Element<'_, VersionMessage> = container(
        container(column![tabs, content].spacing(18).width(Fill).height(Fill))
            .width(Fill)
            .height(Fill)
            .max_width(940),
    )
    .width(Fill)
    .height(Fill)
    .padding([26, 30])
    .align_x(Alignment::Center)
    .style(crate::theme::canvas_style)
    .into();

    if state.staging_confirm_visible {
        page = stack![page, staging_confirm_modal()]
            .width(Fill)
            .height(Fill)
            .into();
    } else {
        // 版本选择弹窗保留在底层，更新日志弹窗叠加在上层；关闭日志后可回到原列表。
        if state.selection_modal_open {
            page = stack![page, version_selector_modal(state)]
                .width(Fill)
                .height(Fill)
                .into();
        }
        if state.release_log_version.is_some() {
            page = stack![page, release_log_modal(state)]
                .width(Fill)
                .height(Fill)
                .into();
        }
        if state.install_task.visible {
            page = stack![page, install_modal(&state.install_task)]
                .width(Fill)
                .height(Fill)
                .into();
        }
    }
    page
}

fn tab_button(
    label: &'static str,
    tab: VersionTab,
    active: bool,
) -> Element<'static, VersionMessage> {
    let color = if active { BLUE_600 } else { INK_MUTED };
    button(
        column![
            container(text(label).size(13).font(crate::core::typography::medium()).color(color))
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
            text(notice)
                .size(10)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
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
        Some(
            if state.local.scan.phase.active() || state.local.scan.auto_hide_at.is_some() {
                format!(
                    "{}  {}",
                    crate::lang::display_label(state.local.scan.status_label()),
                    local::truncate_path(&state.local.scan.progress.path, 60)
                )
            } else {
                format!("已添加 {} 个本地实例", state.local_instances.len())
            },
        ),
        vec![
            icon_button_enabled(
                Icon::FolderPlus,
                "导入本地实例",
                VersionMessage::ImportLocal,
                !state.local.loading && !state.local.import_pending,
            ),
            icon_button_enabled(
                if state.local.scan.phase.active() {
                    Icon::X
                } else {
                    Icon::Search
                },
                if state.local.scan.phase.active() {
                    "取消扫描"
                } else {
                    "扫描本机"
                },
                if state.local.scan.phase.active() {
                    VersionMessage::RequestCancelScan
                } else {
                    VersionMessage::ScanLocal
                },
                !state.local.loading,
            ),
            icon_button(Icon::FileText, "查看扫描日志", VersionMessage::OpenScanLog),
        ],
    );

    let list = if state.local.loading {
        container(text("正在加载本地实例…").size(13))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    } else if state.local_instances.is_empty() {
        container(
            column![
                crate::theme::subtle_icon(Icon::FolderSearch, 36),
                text("尚未发现本地实例")
                    .size(13)
                    .font(crate::core::typography::medium())
                    .style(crate::theme::muted_text_style),
                button(text("开始扫描"))
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
                let current = state.current_source == Some(VersionSource::Local)
                    && state.current_path.as_deref() == Some(item.path.as_str());
                let rows = rows.push(local_instance_row(item, current));
                if index + 1 == state.local_instances.len() {
                    rows
                } else {
                    rows.push(crate::theme::separator())
                }
            },
        );
        scrollable(rows).height(Fill).into()
    };

    panel(header, list)
}

fn local_instance_row<'a>(item: &'a LocalInstance, current: bool) -> Element<'a, VersionMessage> {
    let primary_action = if item.dependencies == DependencyStatus::Ready {
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
                text(label).size(12).font(crate::core::typography::medium()),
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
    } else if matches!(
        item.dependencies,
        DependencyStatus::Checking | DependencyStatus::Installing | DependencyStatus::Failed(_)
    ) {
        let label = match item.dependencies {
            DependencyStatus::Checking => "检测中…",
            DependencyStatus::Installing => "安装中…",
            _ => "重试检测",
        };
        let mut action = button(text(label).size(12))
            .padding([9, 14])
            .style(button_style(ButtonVariant::Outline));
        if matches!(item.dependencies, DependencyStatus::Failed(_)) {
            action = action.on_press(VersionMessage::RecheckLocalDependencies(item.path.clone()));
        }
        action
    } else {
        button(
            row![
                icons::icon(Icon::Download, 15, WHITE),
                text("安装依赖").size(12).font(crate::core::typography::medium()).color(WHITE),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::InstallLocalDependencies(item.path.clone()))
        .padding([9, 14])
        .style(warning_button_style)
    };

    let primary_action: Element<'_, VersionMessage> =
        if matches!(item.dependencies, DependencyStatus::Failed(_)) {
            tooltip(
                primary_action,
                text("依赖检测失败，请点击重试查看原因。").size(11),
                tooltip::Position::Bottom,
            )
            .into()
        } else {
            primary_action.into()
        };

    let mut remove = button(
        row![
            icons::icon(Icon::Trash2, 15, DANGER),
            text("从列表中移除")
                .size(12)
                .font(crate::core::typography::medium())
                .color(DANGER),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .padding([9, 14])
    .style(button_style(ButtonVariant::DangerSoft));
    if !current && item.dependencies != DependencyStatus::Installing {
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
                    text(if item.version == "未知版本" {
                        crate::lang::display_label("未知版本")
                    } else {
                        format!("v{}", item.version)
                    })
                    .size(15)
                    .font(crate::core::typography::medium()),
                    current_badge(current),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    crate::theme::subtle_icon(Icon::MapPin, 12),
                    text(&item.path)
                        .size(10)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style),
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
    let branch_controls = row![
        branch_button(TavernBranch::Release, state.branch),
        branch_button(TavernBranch::Staging, state.branch),
        icon_button(
            Icon::RefreshCw,
            "刷新在线版本",
            VersionMessage::RefreshOnline
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let body = match state.online_status {
        OnlineVersionsStatus::Loading => online_loading_panel(state),
        OnlineVersionsStatus::Error => online_error_panel(state),
        OnlineVersionsStatus::Ready | OnlineVersionsStatus::StaleCache => match state.branch {
            TavernBranch::Release => online_ready_panel(state),
            TavernBranch::Staging => staging_ready_panel(state),
        },
    };

    panel(
        panel_header(
            Icon::CloudDownload,
            "在线实例",
            Some(format!("上次同步：{}", state.last_sync)),
            vec![branch_controls.into()],
        ),
        body,
    )
}

fn branch_button(branch: TavernBranch, current: TavernBranch) -> Element<'static, VersionMessage> {
    let active = branch == current;
    button(text(branch.label()).size(11).font(crate::core::typography::medium()))
        .on_press(VersionMessage::SelectBranch(branch))
        .padding([6, 10])
        .style(move |_theme, status| branch_button_style(active, status))
        .into()
}

fn branch_button_style(active: bool, status: button::Status) -> button::Style {
    let color = if active { BLUE_600 } else { INK_MUTED };
    let background = if active {
        Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.14,
        )))
    } else if matches!(status, button::Status::Hovered) {
        Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.08,
        )))
    } else {
        None
    };
    button::Style {
        background,
        text_color: color,
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn online_loading_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    let dots = ["", ".", "..", "..."][usize::from(state.loading_frame)];
    container(
        column![
            crate::theme::subtle_icon(Icon::LoaderCircle, 34),
            text(format!("正在获取酒馆版本{dots}"))
                .size(14)
                .font(crate::core::typography::medium())
                .style(crate::theme::muted_text_style),
            text("正在读取镜像与直连数据，操作区域将在完成后显示。")
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
        ]
        .spacing(10)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn online_error_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    container(
        column![
            crate::theme::subtle_icon(Icon::CloudOff, 34),
            text("在线版本获取失败")
                .size(14)
                .font(crate::core::typography::medium())
                .style(crate::theme::muted_text_style),
            state
                .notice
                .as_deref()
                .map(text)
                .unwrap_or_else(|| text("没有可用的缓存版本。"))
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
            button("重新获取")
                .on_press(VersionMessage::RefreshOnline)
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
}

fn online_ready_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    let releases = &state.online_releases;
    let selected = state
        .selected_online_version
        .as_deref()
        .or_else(|| releases.first().map(|release| release.version.as_str()));
    let selected_release =
        selected.and_then(|version| releases.iter().find(|release| release.version == version));
    let picker_label = selected.unwrap_or("选择版本");
    let picker = button(
        row![
            text(picker_label).size(13).font(crate::core::typography::regular()),
            space::horizontal(),
            icons::icon(Icon::ChevronDown, 15, INK_MUTED),
        ]
        .align_y(Alignment::Center),
    )
    .on_press(VersionMessage::OpenVersionSelector)
    .width(Fill)
    .padding([11, 14])
    .style(button_style(ButtonVariant::Outline));
    let action = selected_release
        .map(|release| online_action_button(state, release))
        .unwrap_or_else(|| {
            button(text("暂无可用版本").size(13).font(crate::core::typography::medium()))
                .width(Fill)
                .padding([12, 16])
                .style(button_style(ButtonVariant::Secondary))
                .into()
        });
    let action = container(action).width(Fill).max_width(430);
    let status = selected_release
        .map(online_release_status)
        .unwrap_or_else(|| text("请选择一个版本").size(11).font(crate::core::typography::regular()).into());
    let stale_hint: Element<'_, VersionMessage> =
        if state.online_status == OnlineVersionsStatus::StaleCache {
            text("当前显示的是旧缓存版本。")
                .size(10)
                .font(crate::core::typography::regular())
                .color(Color::from_rgb8(190, 120, 20))
                .into()
        } else {
            space::vertical().height(0).into()
        };

    container(
        column![
            image("assets/icon/sillytavern.png")
                .width(150)
                .height(150)
                .content_fit(ContentFit::Contain),
            text("SillyTavern").size(20).font(crate::core::typography::medium()),
            text("选择需要安装或使用的酒馆版本")
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
            container(picker).width(Fill).max_width(430),
            status,
            stale_hint,
            action,
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .padding([36, 54])
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn staging_ready_panel(state: &VersionState) -> Element<'_, VersionMessage> {
    let info = state.staging.as_ref();
    let version = info
        .map(|item| format!("staging · {}", short_sha(&item.commit_sha)))
        .unwrap_or_else(|| "staging".to_owned());
    let detail = info
        .map(|item| item.message.as_str())
        .unwrap_or("正在使用 staging 开发分支");
    let action: Element<'_, VersionMessage> = if state.staging_installed
        && state.current_source == Some(VersionSource::Online)
        && state.current_version.as_deref() == Some("staging")
    {
        button(
            row![
                icons::icon(Icon::CircleCheck, 16, SUCCESS),
                text("当前版本").size(13).font(crate::core::typography::medium())
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Secondary))
        .into()
    } else if state.staging_installed || state.online_instance_exists {
        button(
            row![
                icons::icon(Icon::Power, 16, WHITE),
                text("切换到此版本")
                    .size(13)
                    .font(crate::core::typography::medium())
                    .color(WHITE)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::SwitchOnline("staging".into()))
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Primary))
        .into()
    } else {
        button(
            row![
                icons::icon(Icon::Download, 16, WHITE),
                text("安装").size(13).font(crate::core::typography::medium()).color(WHITE)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::InstallBranch("staging".into()))
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Primary))
        .into()
    };
    container(
        column![
            image("assets/icon/sillytavern.png")
                .width(150)
                .height(150)
                .content_fit(ContentFit::Contain),
            text("SillyTavern staging").size(20).font(crate::core::typography::medium()),
            text(version).size(13).font(crate::core::typography::medium()).color(BLUE_600),
            text(detail)
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
            container(action).width(Fill).max_width(430),
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .padding([36, 54])
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn short_sha(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

fn staging_confirm_modal() -> Element<'static, VersionMessage> {
    let panel = container(
        column![
            text("切换到开发版？").size(18).font(crate::core::typography::medium()),
            text("开发版包含最新功能，但请注意它可能随时出现问题。")
                .size(12)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
            row![
                button(text("取消").size(12).font(crate::core::typography::medium()))
                    .on_press(VersionMessage::CancelStagingRisk)
                    .padding([8, 16])
                    .style(button_style(ButtonVariant::Secondary)),
                button(text("确认切换").size(12).font(crate::core::typography::medium()).color(WHITE))
                    .on_press(VersionMessage::ConfirmStagingRisk)
                    .padding([8, 16])
                    .style(button_style(ButtonVariant::Primary)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(16),
    )
    .width(460)
    .padding(24)
    .style(install_modal_style);

    stack![
        button(space::Space::new())
            .on_press(VersionMessage::CancelStagingRisk)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(install_backdrop_style),
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

fn version_selector_modal(state: &VersionState) -> Element<'_, VersionMessage> {
    let rows = state
        .online_releases
        .iter()
        .fold(column![].width(Fill), |rows, release| {
            let selected =
                state.selected_online_version.as_deref() == Some(release.version.as_str());
            let status = if release.installed { "已安装" } else { "" };
            let row_content = row![
                button(
                    column![
                        row![
                            text(format!("v{}", release.version))
                                .size(13)
                                .font(crate::core::typography::medium()),
                            text(status).size(10).font(crate::core::typography::regular()).color(SUCCESS),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        text(format!("发布于 {}", release.published_at))
                            .size(10)
                            .font(crate::core::typography::regular())
                            .style(crate::theme::muted_text_style),
                        if release.mirror_available {
                            text("镜像已同步")
                                .size(10)
                                .font(crate::core::typography::regular())
                                .color(SUCCESS)
                        } else {
                            text("镜像未同步，将使用直连")
                                .size(10)
                                .font(crate::core::typography::regular())
                                .color(Color::from_rgb8(190, 120, 20))
                        },
                    ]
                    .spacing(4)
                    .width(Fill)
                )
                .on_press(VersionMessage::SelectOnlineVersion(release.version.clone()))
                .padding([10, 12])
                .style(move |theme, button_status| selector_row_style(
                    theme,
                    selected,
                    button_status
                )),
                button(icons::icon(Icon::FileText, 15, BLUE_600))
                    .on_press(VersionMessage::OpenReleaseLog(release.version.clone()))
                    .padding([10, 12])
                    .style(button_style(ButtonVariant::Ghost)),
            ]
            .spacing(4)
            .align_y(Alignment::Center)
            .width(Fill);
            rows.push(row_content).push(crate::theme::separator())
        });
    let panel = container(
        column![
            row![
                text("选择酒馆版本").size(18).font(crate::core::typography::medium()),
                space::horizontal(),
                button(icons::icon(Icon::X, 16, INK_MUTED))
                    .on_press(VersionMessage::CloseVersionSelector)
                    .padding(6)
                    .style(button_style(ButtonVariant::Ghost)),
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1.0).style(crate::theme::separator_style),
            scrollable(rows).height(if crate::core::typography::current_ui_scale() >= 1.35 {
                250
            } else {
                360
            }),
        ]
        .spacing(14),
    )
    .width(Fill).max_width(560)
    .padding(20)
    .style(install_modal_style);
    stack![
        button(space::Space::new())
            .on_press(VersionMessage::CloseVersionSelector)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(install_backdrop_style),
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

fn selector_row_style(theme: &Theme, active: bool, status: button::Status) -> button::Style {
    let background = if active {
        Some(Background::Color(Color::from_rgba(
            BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.14,
        )))
    } else if matches!(status, button::Status::Hovered) {
        Some(Background::Color(Color::from_rgba(
            BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.08,
        )))
    } else {
        None
    };
    button::Style {
        background,
        text_color: crate::theme::text(theme),
        border: Border {
            radius: 8.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn release_log_modal(state: &VersionState) -> Element<'_, VersionMessage> {
    let title = state.release_log_version.as_deref().unwrap_or("版本");
    let body: Element<'_, VersionMessage> = if state.markdown_items.is_empty() {
        text("该版本没有更新日志。")
            .size(12)
            .font(crate::core::typography::regular())
            .into()
    } else {
        markdown::view(&state.markdown_items, Theme::Dark)
            .map(VersionMessage::MarkdownLinkClicked)
            .into()
    };
    let panel = container(
        column![
            row![
                column![
                    text(format!("SillyTavern v{title}"))
                        .size(18)
                        .font(crate::core::typography::medium()),
                    text("版本更新日志")
                        .size(11)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style),
                ]
                .spacing(4),
                space::horizontal(),
                button(icons::icon(Icon::X, 16, INK_MUTED))
                    .on_press(VersionMessage::CloseReleaseLog)
                    .padding(6)
                    .style(button_style(ButtonVariant::Ghost)),
            ]
            .align_y(Alignment::Center),
            rule::horizontal(1.0).style(crate::theme::separator_style),
            scrollable(container(body).width(Fill).padding(8)).height(
                if crate::core::typography::current_ui_scale() >= 1.35 {
                    270
                } else {
                    420
                },
            ),
        ]
        .spacing(14),
    )
    .width(Fill).max_width(700)
    .padding(20)
    .style(install_modal_style);
    stack![
        button(space::Space::new())
            .on_press(VersionMessage::CloseReleaseLog)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(install_backdrop_style),
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

fn online_action_button<'a>(
    state: &VersionState,
    release: &'a OnlineRelease,
) -> Element<'a, VersionMessage> {
    let current = state.current_source == Some(VersionSource::Online)
        && state.current_version.as_deref() == Some(release.version.as_str());
    if current {
        return button(
            row![
                icons::icon(Icon::CircleCheck, 16, SUCCESS),
                text("当前版本").size(13).font(crate::core::typography::medium()),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Secondary))
        .into();
    }

    if release.installed || state.online_instance_exists {
        button(
            row![
                icons::icon(Icon::Power, 16, WHITE),
                text("切换到此版本")
                    .size(13)
                    .font(crate::core::typography::medium())
                    .color(WHITE),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::SwitchOnline(release.version.clone()))
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Primary))
        .into()
    } else {
        button(
            row![
                icons::icon(Icon::Download, 16, WHITE),
                text("安装").size(13).font(crate::core::typography::medium()).color(WHITE),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(VersionMessage::InstallOnline(release.version.clone()))
        .width(Fill)
        .padding([12, 16])
        .style(button_style(ButtonVariant::Primary))
        .into()
    }
}

fn online_release_status(release: &OnlineRelease) -> Element<'static, VersionMessage> {
    let mirror = if release.mirror_available {
        text("镜像已同步")
            .size(10)
            .font(crate::core::typography::regular())
            .color(SUCCESS)
    } else {
        text("镜像未同步，将使用直连")
            .size(10)
            .font(crate::core::typography::regular())
            .color(Color::from_rgb8(190, 120, 20))
    };
    row![
        text(format!("发布于 {}", release.published_at))
            .size(10)
            .font(crate::core::typography::regular())
            .style(crate::theme::muted_text_style),
        mirror,
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn install_modal(task: &InstallTaskState) -> Element<'_, VersionMessage> {
    let version = task.version.as_deref().unwrap_or("—");
    let download_active = task.phase == InstallPhase::Download;
    let install_active = matches!(
        task.phase,
        InstallPhase::Install | InstallPhase::Completed | InstallPhase::Failed
    );
    let download_color = if download_active { BLUE_600 } else { SUCCESS };
    let install_color = if install_active { BLUE_600 } else { INK_MUTED };

    let steps = row![
        install_step(Icon::Download, "下载", download_color, download_active),
        container(space::horizontal()).width(36),
        install_step(Icon::PackageCheck, "安装", install_color, install_active),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Fill);

    let status = match task.phase {
        InstallPhase::Download => "正在下载酒馆源码…",
        InstallPhase::WaitingInstall => "下载完成，3 秒后开始安装…",
        InstallPhase::Install => "正在安装 npm 依赖…",
        InstallPhase::Completed => "安装完成，窗口将在 3 秒后自动关闭。",
        InstallPhase::Failed => "安装失败，请查看日志后重试。",
    };
    let status_color = match task.phase {
        InstallPhase::Failed => DANGER,
        InstallPhase::Completed => SUCCESS,
        _ => INK_MUTED,
    };

    let mut footer = row![
        text(status)
            .size(11)
            .font(crate::core::typography::medium())
            .color(status_color),
        space::horizontal(),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Fill);
    if task.can_close {
        footer = footer.push(
            button(text("关闭").size(12).font(crate::core::typography::medium()))
                .on_press(VersionMessage::CloseInstallModal)
                .height(34)
                .padding([7, 14])
                .style(button_style(ButtonVariant::Secondary)),
        );
    }

    let error_detail: Element<'_, VersionMessage> = match task.error.as_deref() {
        Some(error) => text(error)
            .size(11)
            .font(crate::core::typography::regular())
            .color(DANGER)
            .into(),
        None => text("").size(1).into(),
    };

    let panel = mouse_area(
        container(
            column![
                column![
                    text("酒馆安装").size(18).font(crate::core::typography::medium()),
                    text(format!("正在处理 SillyTavern v{version}"))
                        .size(12)
                        .font(crate::core::typography::regular())
                        .style(crate::theme::muted_text_style),
                ]
                .spacing(4),
                steps,
                rule::horizontal(1.0).style(crate::theme::separator_style),
                scrollable(
                    container(
                        text(&task.logs)
                            .size(11)
                            .font(crate::core::typography::regular())
                            .style(crate::theme::text_style),
                    )
                    .width(Fill)
                    .padding(14)
                    .style(install_log_style),
                )
                .height(250),
                error_detail,
                rule::horizontal(1.0).style(crate::theme::separator_style),
                footer,
            ]
            .spacing(16),
        )
        .width(Fill).max_width(620)
        .padding(20)
        .style(install_modal_style),
    )
    .on_press(VersionMessage::InstallModalInteract);

    stack![
        button(space::Space::new())
            .on_press(VersionMessage::InstallModalInteract)
            .width(Fill)
            .height(Fill)
            .padding(0)
            .style(install_backdrop_style),
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

fn install_step(
    icon: Icon,
    label: &'static str,
    color: Color,
    active: bool,
) -> Element<'static, VersionMessage> {
    container(
        column![
            icons::icon(icon, 20, color),
            text(label).size(11).font(crate::core::typography::medium()).color(color)
        ]
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .padding([8, 6])
    .style(move |_theme| step_surface(color, active))
    .into()
}

fn panel<'a>(
    header: Element<'a, VersionMessage>,
    content: Element<'a, VersionMessage>,
) -> Element<'a, VersionMessage> {
    container(
        column![header, crate::theme::separator(), content]
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
        crate::theme::muted_icon(icon, 19),
        text(title).size(15).font(crate::core::typography::medium()),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(meta) = meta {
        title_row = title_row.push(
            container(
                text(meta)
                    .size(9)
                    .font(crate::core::typography::regular())
                    .style(crate::theme::muted_text_style),
            )
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
    icon_button_enabled(icon, label, message, true)
}

fn icon_button_enabled(
    icon: Icon,
    label: &'static str,
    message: VersionMessage,
    enabled: bool,
) -> Element<'static, VersionMessage> {
    let action = button(
        container(crate::theme::muted_icon(icon, 16))
            .width(28)
            .height(28)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press_maybe(enabled.then_some(message))
    .padding(0)
    .style(button_style(ButtonVariant::Ghost));

    tooltip(
        action,
        container(text(label).size(10).font(crate::core::typography::regular()).color(WHITE))
            .padding([6, 9])
            .style(tooltip_surface),
        tooltip::Position::Bottom,
    )
    .gap(5)
    .delay(iced::time::Duration::from_millis(350))
    .into()
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
    container(text(label).size(8).font(crate::core::typography::medium()).color(color))
        .padding([4, 7])
        .style(move |_theme| badge_surface(color))
        .into()
}

fn tab_button_style(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(crate::theme::surface_alt(theme))),
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

fn panel_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 16.0.into(),
        },
        ..container::Style::default()
    }
}

fn panel_header_surface(_theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            radius: 16.0.into(),
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
            radius: 12.0.into(),
        },
        ..container::Style::default()
    }
}

fn indigo_icon_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
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

fn warning_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Color::from_rgba(
            theme.palette().warning.r,
            theme.palette().warning.g,
            theme.palette().warning.b,
            0.86,
        )
    } else {
        theme.palette().warning
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

fn install_modal_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 16.0.into(),
        },
        ..container::Style::default()
    }
}

fn install_log_style(theme: &Theme) -> container::Style {
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

fn install_backdrop_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.42))),
        ..button::Style::default()
    }
}

fn step_surface(color: Color, active: bool) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r,
            color.g,
            color.b,
            if active { 0.12 } else { 0.06 },
        ))),
        border: Border {
            color: Color::from_rgba(color.r, color.g, color.b, if active { 0.32 } else { 0.16 }),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{VersionMessage, VersionSource, VersionState, VersionTab};

    #[test]
    fn tab_can_switch_to_online_instances() {
        let mut state = VersionState::default();
        state.update(VersionMessage::SelectTab(VersionTab::Online));
        assert_eq!(state.active_tab, VersionTab::Online);
    }

    #[test]
    fn online_selection_is_source_aware() {
        let mut state = VersionState::default();
        state.online_releases[0].installed = true;
        state.update(VersionMessage::SwitchOnline("1.18.0".into()));
        assert_eq!(state.current_source, Some(VersionSource::Online));
        assert_eq!(
            state.current_path.as_deref(),
            Some(super::online_instance_path().as_str())
        );
    }

    #[test]
    fn install_opens_non_dismissible_modal_and_completes() {
        let mut state = VersionState::default();
        state.update(VersionMessage::InstallOnline("1.18.0".into()));
        assert!(state.install_task.visible);
        assert!(state.install_task.running);
        state.update(VersionMessage::InstallModalInteract);
        assert!(state.install_task.visible);
        state.update(VersionMessage::InstallCompleted);
        assert!(state.install_task.can_close);
        assert_eq!(state.install_task.auto_close_ticks, 3);
    }

    #[test]
    fn current_local_instance_cannot_be_removed() {
        let mut state = VersionState::default();
        let path = "/tmp/example-tavern".to_owned();
        state.local_instances.push(super::LocalInstance {
            path: path.clone(),
            version: "1".into(),
            dependencies: super::DependencyStatus::Ready,
            identity: None,
        });
        state.update(VersionMessage::SwitchLocal(path.clone()));
        state.update(VersionMessage::RemoveLocal(path.clone()));
        assert!(
            state
                .local_instances
                .iter()
                .any(|instance| instance.path == path)
        );
    }
    #[test]
    fn catalog_refresh_keeps_fresh_disk_flags_and_clears_stale_flags() {
        let mut state = VersionState::default();
        state.current_source = Some(VersionSource::Local);
        state.current_path = Some("/fixture/local".into());
        let version = state.online_releases[0].version.clone();
        for installed in [true, false] {
            let mut releases = state.online_releases.clone();
            for release in &mut releases {
                release.installed = release.version == version && installed;
            }
            state.update(VersionMessage::OnlineVersionsLoaded {
                branch: super::TavernBranch::Release,
                releases,
                staging: None,
                last_sync: String::new(),
                from_cache: false,
            });
            assert_eq!(state.is_online_installed(&version), installed);
            assert_eq!(state.current_source, Some(VersionSource::Local));
            assert_eq!(state.current_path.as_deref(), Some("/fixture/local"));
        }
    }

    #[test]
    fn disk_snapshot_updates_installation_without_switching_current_instance() {
        let mut state = VersionState::default();
        state.current_source = Some(VersionSource::Local);
        state.current_path = Some("/fixture/local".into());
        let version = state.online_releases[0].version.clone();
        let mut snapshot = crate::core::network::InstalledSillyTavern {
            tag_name: Some(state.online_releases[0].tag_name.clone()),
            branch: None,
            head: "fixture".into(),
        };
        state.sync_online_installation(Some(&snapshot));
        assert!(state.is_online_installed(&version));
        assert_eq!(state.current_source, Some(VersionSource::Local));
        assert_eq!(state.current_path.as_deref(), Some("/fixture/local"));
        snapshot.tag_name = Some("v0.0.0-fixture".into());
        state.sync_online_installation(Some(&snapshot));
        assert!(!state.is_online_installed(&version));
        state.sync_online_installation(None);
        assert!(!state.online_instance_exists);
        assert!(
            state
                .online_releases
                .iter()
                .all(|release| !release.installed)
        );
        assert!(!state.staging_installed);
        assert_eq!(state.current_source, Some(VersionSource::Local));
    }

    #[test]
    fn same_tag_on_staging_is_not_treated_as_checked_out_stable_release() {
        let mut state = VersionState::default();
        let version = state.online_releases[0].version.clone();
        let snapshot = crate::core::network::InstalledSillyTavern {
            tag_name: Some(state.online_releases[0].tag_name.clone()),
            branch: Some("staging".into()),
            head: "fixture".into(),
        };
        state.sync_online_installation(Some(&snapshot));
        assert!(state.is_online_installed("staging"));
        assert!(!state.is_online_installed(&version));
    }

    #[test]
    fn switching_to_same_numbered_online_release_changes_source_path_and_branch() {
        let mut state = VersionState::default();
        let version = state.online_releases[0].version.clone();
        state.current_source = Some(VersionSource::Local);
        state.current_path = Some("/fixture/local".into());
        state.current_version = Some(version.clone());
        state.branch = super::TavernBranch::Staging;
        state.online_releases[0].installed = true;
        state.update(VersionMessage::SwitchOnline(version.clone()));
        assert_eq!(state.current_source, Some(VersionSource::Online));
        assert_eq!(state.current_path, state.online_instance_path);
        assert_eq!(state.current_version, Some(version.clone()));
        assert_eq!(state.selected_online_version, Some(version));
        assert_eq!(state.branch, super::TavernBranch::Release);
        assert!(!state.install_task.running);
        assert!(state.local.toast.is_some());
    }

    #[test]
    fn unavailable_online_instance_reports_failure_without_changing_selection() {
        let mut state = VersionState::default();
        state.current_source = Some(VersionSource::Local);
        state.current_path = Some("/fixture/local".into());
        state.update(VersionMessage::SwitchOnline(
            state.online_releases[0].version.clone(),
        ));
        assert_eq!(state.current_source, Some(VersionSource::Local));
        assert_eq!(state.current_path.as_deref(), Some("/fixture/local"));
        assert!(state.local.toast.as_ref().unwrap().danger);
    }
}
