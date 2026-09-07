//! SillyTavern 控制台页面与运行状态。
//!
//! 页面只负责展示和向运行时发送命令；真实子进程与 PM2 生命周期位于
//! `core::tavern_process` 的后台线程中，避免阻塞 iced 主线程。

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use iced::advanced::text::{Highlighter, Wrapping, highlighter};
use iced::widget::{button, column, container, row, space, stack, text_editor};
use iced::{Alignment, Background, Border, Color, Element, Fill, Theme};
use lucide_icons::Icon;

use astra_ui::{BLUE_600, ButtonVariant, DANGER, INK_MUTED, SUCCESS, icons};

use crate::app::Message;
use crate::core::tavern_process::{
    PortConflict, ProcessCommand, ProcessEvent, RuntimeMode, TavernLaunchMode, TavernLaunchSpec,
    TavernRuntime,
};
use crate::lang::{lang::current_language, t, text};
use crate::theme::button_style;

const MAX_LOG_LINES: usize = 2_000;
/// 控制台日志使用比普通辅助文字更大的基础字号，系统缩放由 iced 继续叠加。
const LOG_FONT_SIZE: f32 = 13.0;
const WARNING: Color = Color::from_rgb8(245, 165, 36);


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleStatus {
    NotStarted,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl ConsoleStatus {
    fn key(self) -> &'static str {
        match self {
            Self::NotStarted => "console.status.not_started",
            Self::Starting => "console.status.starting",
            Self::Running => "console.status.running",
            Self::Stopping => "console.status.stopping",
            Self::Stopped => "console.status.stopped",
            Self::Failed => "console.status.failed",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::NotStarted => INK_MUTED,
            Self::Starting => BLUE_600,
            Self::Running => SUCCESS,
            Self::Stopping | Self::Stopped => WARNING,
            Self::Failed => DANGER,
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::NotStarted | Self::Stopped => Icon::Square,
            Self::Starting | Self::Stopping => Icon::Loader,
            Self::Running => Icon::CircleCheck,
            Self::Failed => Icon::CircleX,
        }
    }

    pub fn is_transitioning(self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    Info,
    Success,
    Warning,
    Error,
    Output,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogHighlight {
    Info,
    Success,
    Warning,
    Error,
    Output,
    System,
}

impl From<LogKind> for LogHighlight {
    fn from(kind: LogKind) -> Self {
        match kind {
            LogKind::Info => Self::Info,
            LogKind::Success => Self::Success,
            LogKind::Warning => Self::Warning,
            LogKind::Error => Self::Error,
            LogKind::Output => Self::Output,
            LogKind::System => Self::System,
        }
    }
}

/// 直接按照日志状态中保存的类型为整行着色，不再依赖可见标签或文本猜测。
#[derive(Debug, Clone)]
struct LogHighlighter {
    line: usize,
    highlights: Arc<Vec<LogHighlight>>,
}

impl Highlighter for LogHighlighter {
    type Settings = Arc<Vec<LogHighlight>>;
    type Highlight = LogHighlight;
    type Iterator<'a> = std::iter::Once<(std::ops::Range<usize>, Self::Highlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            line: 0,
            highlights: Arc::clone(settings),
        }
    }

    fn update(&mut self, settings: &Self::Settings) {
        self.highlights = Arc::clone(settings);
        // iced 更新高亮配置时不会主动回退 current_line；日志追加后必须从首行
        // 重新应用属性，否则新增的 warning/error 会继续使用编辑器默认文字颜色。
        self.line = 0;
    }

    fn change_line(&mut self, line: usize) {
        self.line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let highlight = self
            .highlights
            .get(self.line)
            .copied()
            .unwrap_or(LogHighlight::Output);
        self.line = self.line.saturating_add(1);
        std::iter::once((0..line.len(), highlight))
    }

    fn current_line(&self) -> usize {
        self.line
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
    fn key(self) -> &'static str {
        match self {
            Self::Lan => "console.network.lan",
            Self::Internet => "console.network.internet",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Lan => SUCCESS,
            Self::Internet => DANGER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleAction {
    None,
    OpenServer,
}

#[derive(Debug, Clone)]
pub enum ConsoleMessage {
    Start,
    Stop,
    Kill,
    Restart,
    Poll,
    ClearLogs,
    ExportLogs,
    FollowLogs,
    LogEditorAction(text_editor::Action),
    ToggleNetworkDialog,
    OpenServer,
    ConfirmReleasePort,
    CancelReleasePort,
}

#[derive(Debug)]
pub struct ConsoleState {
    pub status: ConsoleStatus,
    pub logs: VecDeque<ConsoleLog>,
    pub auto_scroll: bool,
    pub show_network_dialog: bool,
    pub network_mode: Option<NetworkMode>,
    pub network_port: Option<u16>,
    pub server_url: Option<String>,
    pub process_pid: Option<u32>,
    pub runtime_mode: Option<RuntimeMode>,
    pub active_launch_mode: Option<TavernLaunchMode>,
    pub active_version: String,
    pub active_export_path: String,
    pub pending_port_conflict: Option<PortConflict>,
    pub restored_pm2_path: Option<String>,
    /// 可框选复制的只读日志编辑器内容。
    log_content: text_editor::Content,
    /// 编辑器每个逻辑行对应的真实日志类型，颜色不依赖可见标签。
    log_highlights: Arc<Vec<LogHighlight>>,
    /// 警告和错误的多行输出上下文，用于给续行继承颜色。
    stream_context: Option<LogKind>,
    /// 直接进程重启会先停止旧进程；下一次 Starting 才是新日志会话边界。
    reset_logs_on_starting: bool,
    /// 当前会话已经启用规范日志文件写入。
    disk_log_active: bool,
    runtime: TavernRuntime,
}

impl Default for ConsoleState {
    fn default() -> Self {
        let mut state = Self {
            status: ConsoleStatus::NotStarted,
            logs: VecDeque::new(),
            auto_scroll: true,
            show_network_dialog: false,
            network_mode: None,
            network_port: None,
            server_url: None,
            process_pid: None,
            runtime_mode: None,
            active_launch_mode: None,
            active_version: String::new(),
            active_export_path: String::new(),
            pending_port_conflict: None,
            restored_pm2_path: None,
            log_content: text_editor::Content::new(),
            log_highlights: Arc::new(Vec::new()),
            stream_context: None,
            reset_logs_on_starting: false,
            disk_log_active: false,
            runtime: TavernRuntime::default(),
        };
        state.push(LogKind::System, tr("console.log.ready"));
        state
    }
}

impl ConsoleState {
    /// 清空内存日志及其派生编辑器状态，不触碰磁盘日志文件。
    fn clear_log_buffers(&mut self) {
        self.logs.clear();
        self.log_content = text_editor::Content::new();
        self.log_highlights = Arc::new(Vec::new());
        self.stream_context = None;
        self.auto_scroll = true;
    }

    /// 创建新的日志会话，并同步清空界面、选择器、高亮和旧访问状态。
    fn reset_log_session(&mut self) {
        self.clear_log_buffers();
        self.server_url = None;
        self.network_port = None;
        self.pending_port_conflict = None;
        self.disk_log_active = match crate::core::tavern_process::prepare_sillytavern_log_file() {
            Ok(()) => true,
            Err(error) => {
                self.disk_log_active = false;
                self.push(LogKind::Error, error);
                false
            }
        };
    }

    pub fn start(&mut self, spec: TavernLaunchSpec, network_mode: Option<NetworkMode>) {
        if matches!(self.status, ConsoleStatus::Starting | ConsoleStatus::Running | ConsoleStatus::Stopping) {
            return;
        }
        self.reset_log_session();
        self.reset_logs_on_starting = false;
        self.server_url = None;
        self.network_port = None;
        self.process_pid = None;
        self.pending_port_conflict = None;
        self.network_mode = network_mode;
        self.active_launch_mode = Some(spec.launch_mode);
        self.active_version = spec.instance_version.clone();
        self.active_export_path = spec.export_path.clone();
        self.status = ConsoleStatus::Starting;
        self.push(LogKind::System, tr("console.log.starting"));
        if let Err(error) = self.runtime.send(ProcessCommand::Start(spec)) {
            self.fail(error);
        }
    }

    pub fn add_error(&mut self, error: impl Into<String>) {
        self.fail(error.into());
    }

    /// 仅记录附属功能错误，不改变 SillyTavern 服务运行状态。
    pub fn add_error_log(&mut self, error: impl Into<String>) {
        self.push(LogKind::Error, error);
    }

    pub fn add_system(&mut self, message: impl Into<String>) {
        self.push(LogKind::System, message);
    }

    pub fn add_success(&mut self, message: impl Into<String>) {
        self.push(LogKind::Success, message);
    }

    pub fn add_warning(&mut self, message: impl Into<String>) {
        self.push(LogKind::Warning, message);
    }

    pub fn update(&mut self, message: ConsoleMessage) -> ConsoleAction {
        match message {
            ConsoleMessage::Start => {}
            ConsoleMessage::Stop => self.send_command(ProcessCommand::Stop),
            ConsoleMessage::Kill => self.send_command(ProcessCommand::Kill),
            ConsoleMessage::Restart => {
                self.reset_logs_on_starting = true;
                self.send_command(ProcessCommand::Restart);
            }
            ConsoleMessage::Poll => self.poll(),
            ConsoleMessage::ClearLogs => {
                self.clear_log_buffers();
                self.reset_logs_on_starting = false;
            }
            ConsoleMessage::ExportLogs => self.export_logs(),
            ConsoleMessage::FollowLogs => {
                self.auto_scroll = true;
                self.log_content
                    .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
            }
            ConsoleMessage::LogEditorAction(action) => {
                if matches!(
                    action,
                    text_editor::Action::Scroll { lines } if lines != 0
                ) || matches!(
                    action,
                    text_editor::Action::Click(_)
                        | text_editor::Action::Drag(_)
                        | text_editor::Action::Select(_)
                        | text_editor::Action::SelectWord
                        | text_editor::Action::SelectLine
                        | text_editor::Action::SelectAll
                ) {
                    self.auto_scroll = false;
                }
                // 只执行选择、移动和滚动动作，键盘输入、粘贴、删除均被忽略。
                if !action.is_edit() {
                    self.log_content.perform(action);
                }
            }
            ConsoleMessage::ToggleNetworkDialog => {
                self.show_network_dialog = !self.show_network_dialog;
            }
            ConsoleMessage::OpenServer => return ConsoleAction::OpenServer,
            ConsoleMessage::ConfirmReleasePort => {
                if let Some(conflict) = self.pending_port_conflict.take() {
                    self.status = ConsoleStatus::Starting;
                    self.push(LogKind::System, tr("console.port.releasing"));
                    self.send_command(ProcessCommand::ReleasePortAndRetry(conflict));
                }
            }
            ConsoleMessage::CancelReleasePort => {
                self.pending_port_conflict = None;
                self.runtime_mode = None;
                self.status = ConsoleStatus::Failed;
                self.push(LogKind::Warning, tr("console.port.cancelled"));
            }
        }
        ConsoleAction::None
    }

    pub fn needs_tick(&self) -> bool {
        true
    }

    pub fn is_running(&self) -> bool {
        self.status == ConsoleStatus::Running
    }

    pub fn is_direct_runtime(&self) -> bool {
        self.runtime_mode == Some(RuntimeMode::Direct)
    }

    fn send_command(&mut self, command: ProcessCommand) {
        if let Err(error) = self.runtime.send(command) {
            self.fail(error);
        }
    }

    fn poll(&mut self) {
        for event in self.runtime.drain() {
            match event {
                ProcessEvent::Starting(mode) => {
                    if self.reset_logs_on_starting {
                        self.reset_log_session();
                        self.reset_logs_on_starting = false;
                    }
                    self.status = ConsoleStatus::Starting;
                    self.runtime_mode = Some(mode);
                }
                ProcessEvent::Running(mode) => {
                    self.status = ConsoleStatus::Running;
                    self.runtime_mode = Some(mode);
                    self.push(LogKind::Success, tr("console.log.started"));
                }
                ProcessEvent::Stopping => {
                    self.status = ConsoleStatus::Stopping;
                    self.push(LogKind::System, tr("console.log.stopping"));
                }
                ProcessEvent::Stopped => {
                    self.reset_logs_on_starting = false;
                    self.status = ConsoleStatus::Stopped;
                    self.process_pid = None;
                    self.server_url = None;
                    self.network_port = None;
                    self.runtime_mode = None;
                    self.pending_port_conflict = None;
                    self.push(LogKind::Success, tr("console.log.stopped"));
                }
                ProcessEvent::Failed(error) => self.fail(error),
                ProcessEvent::Pid(pid) => self.process_pid = pid,
                ProcessEvent::Log(line) => self.push_process_log(line),
                ProcessEvent::ServerUrl(url) => {
                    self.network_port = url_port(&url);
                    self.server_url = Some(url);
                }
                ProcessEvent::PortConflict(conflict) => {
                    self.status = ConsoleStatus::Failed;
                    self.runtime_mode = None;
                    self.push(
                        LogKind::Error,
                        format!("{} {}", tr("console.port.detected"), conflict.port),
                    );
                    self.pending_port_conflict = Some(conflict);
                }
                ProcessEvent::Exited(code) => self.push(
                    if code == Some(0) { LogKind::Info } else { LogKind::Warning },
                    format!("{} {}", tr("console.log.exited"), code.map_or_else(|| "-".to_owned(), |code| code.to_string())),
                ),
                ProcessEvent::Pm2Unavailable => self.push(LogKind::Warning, tr("console.pm2.unavailable")),
                ProcessEvent::Pm2Restored { pid, cwd } => {
                    if !self.disk_log_active {
                        self.disk_log_active = crate::core::tavern_process::ensure_sillytavern_log_file().is_ok();
                    }
                    self.status = ConsoleStatus::Running;
                    self.runtime_mode = Some(RuntimeMode::Pm2);
                    self.active_launch_mode = Some(TavernLaunchMode::Server);
                    self.process_pid = pid;
                    self.restored_pm2_path = cwd;
                    self.push(LogKind::Success, tr("console.pm2.restored"));
                }
            }
        }
    }

    /// 判断酒馆原始输出类型，并让错误堆栈、警告说明等续行继承颜色。
    fn push_process_log(&mut self, line: String) {
        let explicit = classify_log(&line);
        let kind = if explicit == LogKind::Output
            && self
                .stream_context
                .is_some_and(|kind| matches!(kind, LogKind::Warning | LogKind::Error))
            && is_diagnostic_continuation(&line)
        {
            self.stream_context.unwrap_or(LogKind::Output)
        } else {
            explicit
        };
        self.stream_context = match kind {
            LogKind::Warning | LogKind::Error => Some(kind),
            _ => None,
        };
        self.push(kind, line);
    }

    fn fail(&mut self, error: String) {
        self.status = ConsoleStatus::Failed;
        self.runtime_mode = None;
        self.process_pid = None;
        self.push(LogKind::Error, error);
    }

    fn push(&mut self, kind: LogKind, content: impl Into<String>) {
        let text = sanitize_log_text(content.into());
        // SillyTavern 会输出较多空行；普通空行没有诊断价值，直接忽略。
        if kind == LogKind::Output && text.trim().is_empty() {
            return;
        }
        let log = ConsoleLog {
            time: timestamp(),
            kind,
            text,
        };
        let rendered = render_log_line(&log);
        let was_empty = self.logs.is_empty();
        self.logs.push_back(log);
        let trimmed = if self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
            true
        } else {
            false
        };
        if trimmed {
            self.rebuild_log_content();
        } else {
            let line_count = rendered.lines().count().max(1);
            Arc::make_mut(&mut self.log_highlights)
                .extend(std::iter::repeat_n(LogHighlight::from(kind), line_count));
            self.append_log_content(&rendered, was_empty);
        }
        if self.disk_log_active
            && let Err(error) = crate::core::tavern_process::append_sillytavern_log_line(&rendered)
        {
            self.disk_log_active = false;
            self.push(LogKind::Error, error);
        }
    }

    /// 向只读编辑器追加日志；暂停跟随时恢复用户原有光标和选区。
    fn append_log_content(&mut self, line: &str, was_empty: bool) {
        if was_empty {
            self.log_content = text_editor::Content::with_text(line);
            return;
        }
        let cursor = (!self.auto_scroll).then(|| self.log_content.cursor());
        self.log_content
            .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        self.log_content.perform(text_editor::Action::Edit(
            text_editor::Edit::Paste(Arc::new(format!("\n{line}"))),
        ));
        if let Some(cursor) = cursor {
            self.log_content.move_to(cursor);
        }
    }

    fn rebuild_log_content(&mut self) {
        let mut rendered_lines = Vec::new();
        let mut highlights = Vec::new();
        for log in &self.logs {
            let rendered = render_log_line(log);
            highlights.extend(std::iter::repeat_n(
                LogHighlight::from(log.kind),
                rendered.lines().count().max(1),
            ));
            rendered_lines.push(rendered);
        }
        self.log_content = text_editor::Content::with_text(&rendered_lines.join("\n"));
        self.log_highlights = Arc::new(highlights);
        if self.auto_scroll {
            self.log_content
                .perform(text_editor::Action::Move(text_editor::Motion::DocumentEnd));
        }
    }

    /// 将当前完整会话日志导出为 UTF-8 文本文件。
    fn export_logs(&mut self) {
        let directory = export_directory(&self.active_export_path);
        let filename = export_filename();
        let Some(path) = rfd::FileDialog::new()
            .set_directory(directory)
            .set_file_name(&filename)
            .add_filter("Log", &["log", "txt"])
            .save_file()
        else {
            return;
        };
        let in_memory = || {
            self.logs
                .iter()
                .map(render_log_line)
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut contents = if self.disk_log_active {
            // 已启动服务时优先导出规范目录中的完整会话日志。
            let canonical = crate::utils::app_paths().sillytavern_log_file();
            std::fs::read_to_string(canonical).unwrap_or_else(|_| in_memory())
        } else {
            in_memory()
        };
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        match std::fs::write(&path, contents) {
            Ok(()) => self.push(
                LogKind::Success,
                format!("{} {}", tr("console.export.success"), path.display()),
            ),
            Err(error) => self.push(
                LogKind::Error,
                format!("{} {error}", tr("console.export.failed")),
            ),
        }
    }
}

pub fn console_view(state: &ConsoleState) -> Element<'_, Message> {
    let body = column![header_view(state), logs_view(state)]
        .height(Fill)
        .width(Fill);
    let mut layers: Vec<Element<'_, Message>> = vec![body.into()];
    if state.show_network_dialog {
        layers.push(network_dialog(state));
    }
    if let Some(conflict) = state.pending_port_conflict.as_ref() {
        layers.push(port_conflict_dialog(conflict));
    }
    stack(layers).into()
}

fn header_view(state: &ConsoleState) -> Element<'_, Message> {
    let mut left = row![
        crate::theme::muted_icon(Icon::SquareTerminal, 20),
        text(tr("console.title")).size(15).font(crate::core::typography::bold()),
        status_badge(state.status),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    if let Some(pid) = state.process_pid {
        left = left.push(separator()).push(
            text(format!("PID: {pid}"))
                .size(11)
                .font(crate::core::typography::regular())
                .style(crate::theme::muted_text_style),
        );
    }
    if let Some(mode) = state.runtime_mode {
        left = left.push(separator()).push(
            text(match mode { RuntimeMode::Direct => "Direct", RuntimeMode::Pm2 => "PM2" })
                .size(11)
                .font(crate::core::typography::medium())
                .style(crate::theme::muted_text_style),
        );
    }
    if state.status == ConsoleStatus::Running {
        if let Some(mode) = state.network_mode {
            left = left.push(separator()).push(
                button(text(tr(mode.key())).size(11).color(mode.color()))
                    .padding([6, 10])
                    .style(soft_button(mode.color()))
                    .on_press(Message::Console(ConsoleMessage::ToggleNetworkDialog)),
            );
        } else if state.server_url.is_some() {
            left = left.push(separator()).push(open_button());
        }
    }

    let mut actions = row![
        action_button(
            tr("console.export"),
            Icon::Download,
            BLUE_600,
            ConsoleMessage::ExportLogs,
        ),
        button(crate::theme::muted_icon(Icon::Trash2, 16))
            .padding(8)
            .style(icon_button_style())
            .on_press(Message::Console(ConsoleMessage::ClearLogs)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if !state.auto_scroll {
        actions = actions.push(
            button(text(tr("console.follow")).size(11).color(BLUE_600))
                .padding([7, 10])
                .style(soft_button(BLUE_600))
                .on_press(Message::Console(ConsoleMessage::FollowLogs)),
        );
    }
    match state.status {
        ConsoleStatus::Running => {
            actions = actions
                .push(action_button(tr("console.restart"), Icon::RotateCcw, BLUE_600, ConsoleMessage::Restart))
                .push(action_button(tr("console.stop"), Icon::Square, WARNING, ConsoleMessage::Stop))
                .push(action_button(tr("console.kill"), Icon::OctagonX, DANGER, ConsoleMessage::Kill));
        }
        ConsoleStatus::Starting | ConsoleStatus::Stopping => {}
        ConsoleStatus::NotStarted | ConsoleStatus::Stopped | ConsoleStatus::Failed => {
            actions = actions.push(action_button(tr("console.start"), Icon::Play, SUCCESS, ConsoleMessage::Start));
        }
    }

    container(row![left, space::horizontal(), actions].align_y(Alignment::Center))
        .width(Fill)
        .height(58)
        .padding([10, 18])
        .style(header_surface)
        .into()
}

fn action_button(label: &'static str, icon: Icon, color: Color, message: ConsoleMessage) -> Element<'static, Message> {
    button(row![icons::icon(icon, 13, color), text(label).size(11).color(color)].spacing(6))
        .padding([7, 10])
        .style(soft_button(color))
        .on_press(Message::Console(message))
        .into()
}

fn open_button() -> Element<'static, Message> {
    action_button(tr("console.open"), Icon::ExternalLink, SUCCESS, ConsoleMessage::OpenServer)
}

fn logs_view(state: &ConsoleState) -> Element<'_, Message> {
    let editor = text_editor::TextEditor::new(&state.log_content)
        .placeholder(tr("console.logs.empty"))
        .on_action(|action| Message::Console(ConsoleMessage::LogEditorAction(action)))
        .highlight_with::<LogHighlighter>(state.log_highlights.clone(), log_highlight_format)
        .font(crate::core::typography::regular())
        .size(LOG_FONT_SIZE)
        .line_height(iced::advanced::text::LineHeight::Relative(1.5))
        .wrapping(Wrapping::WordOrGlyph)
        .padding([14, 18])
        .height(Fill)
        .style(log_editor_style);

    container(editor)
        .width(Fill)
        .height(Fill)
        .style(log_surface)
        .into()
}

fn network_dialog(state: &ConsoleState) -> Element<'_, Message> {
    let mode = state.network_mode.unwrap_or(NetworkMode::Lan);
    let port = state.network_port.unwrap_or(8000);
    let local_url = state.server_url.clone().unwrap_or_else(|| format!("http://127.0.0.1:{port}"));
    let lan_url = local_ip().map_or_else(|| format!("http://<LAN-IP>:{port}"), |ip| format!("http://{ip}:{port}"));
    let warning = if mode == NetworkMode::Internet {
        tr("console.network.internet_warning")
    } else {
        tr("console.network.lan_hint")
    };
    modal(
        column![
            row![
                icons::icon(if mode == NetworkMode::Lan { Icon::Wifi } else { Icon::Globe }, 22, mode.color()),
                column![
                    text(tr(mode.key())).size(18).font(crate::core::typography::medium()),
                    text(warning).size(11).style(crate::theme::muted_text_style),
                ].spacing(4).width(Fill),
                button(crate::theme::muted_icon(Icon::X, 16))
                    .padding(7)
                    .style(icon_button_style())
                    .on_press(Message::Console(ConsoleMessage::ToggleNetworkDialog)),
            ].spacing(12).align_y(Alignment::Center),
            url_row(tr("console.network.local_url"), local_url),
            url_row(tr("console.network.lan_url"), lan_url),
            crate::theme::alert(tr("console.network.security"), warning, if mode == NetworkMode::Internet { astra_ui::AlertKind::Danger } else { astra_ui::AlertKind::Info }),
            row![space::horizontal(), open_button()].width(Fill),
        ].spacing(16),
        560,
    )
}

fn url_row(label: &'static str, url: String) -> Element<'static, Message> {
    column![
        text(label).size(10).style(crate::theme::muted_text_style),
        container(text(url).size(12).font(crate::core::typography::regular()))
            .padding([10, 12])
            .width(Fill)
            .style(dialog_code_surface),
    ].spacing(5).into()
}

fn port_conflict_dialog(conflict: &PortConflict) -> Element<'_, Message> {
    let processes = conflict.processes.iter().fold(column!().spacing(6), |column, process| {
        column.push(text(format!("PID {} · {}", process.pid, process.name)).size(12).font(crate::core::typography::regular()))
    });
    let mut actions = row![
        space::horizontal(),
        button(text(tr("console.port.cancel")).size(12))
            .padding([9, 14])
            .style(button_style(ButtonVariant::Secondary))
            .on_press(Message::Console(ConsoleMessage::CancelReleasePort)),
    ].spacing(10).align_y(Alignment::Center);
    if conflict.retry_available {
        actions = actions.push(
            button(text(tr("console.port.confirm")).size(12).color(Color::WHITE))
                .padding([9, 14])
                .style(button_style(ButtonVariant::Destructive))
                .on_press(Message::Console(ConsoleMessage::ConfirmReleasePort)),
        );
    }
    modal(
        column![
            row![
                icons::icon(Icon::TriangleAlert, 22, DANGER),
                column![
                    text(tr("console.port.title")).size(18).font(crate::core::typography::medium()),
                    text(format!("{} {}", tr("console.port.description"), conflict.port)).size(12).style(crate::theme::muted_text_style),
                ].spacing(4),
            ].spacing(12).align_y(Alignment::Center),
            container(processes).padding(12).width(Fill).style(dialog_code_surface),
            crate::theme::alert(tr("console.port.warning_title"), tr("console.port.warning"), astra_ui::AlertKind::Danger),
            actions,
        ].spacing(16),
        520,
    )
}

fn modal(content: impl Into<Element<'static, Message>>, width: u32) -> Element<'static, Message> {
    container(container(content).width(width).padding(22).style(dialog_surface))
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(backdrop_surface)
        .into()
}

fn status_badge(status: ConsoleStatus) -> Element<'static, Message> {
    container(row![
        icons::icon(status.icon(), 13, status.color()),
        text(tr(status.key())).size(11).color(status.color()),
    ].spacing(6).align_y(Alignment::Center))
        .padding([6, 10])
        .style(move |_theme| soft_surface(status.color()))
        .into()
}

fn separator() -> Element<'static, Message> {
    container(space::vertical().height(16)).width(1).style(separator_surface).into()
}

fn render_log_line(log: &ConsoleLog) -> String {
    let text = match log.kind {
        LogKind::System => log.text.strip_prefix("[系统] ").unwrap_or(&log.text),
        LogKind::Warning => log.text.strip_prefix("[警告] ").unwrap_or(&log.text),
        LogKind::Error => log.text.strip_prefix("[错误] ").unwrap_or(&log.text),
        LogKind::Info | LogKind::Success | LogKind::Output => &log.text,
    };

    if log.kind == LogKind::System
        && let Some(command) = text.strip_prefix("[启动命令] ")
    {
        let mut lines = vec![format!(
            "{}    {}",
            log.time,
            tr("console.log.startup_command")
        )];
        lines.extend(
            wrap_log_text(command, 112)
                .into_iter()
                .map(|line| format!("            > {line}")),
        );
        return lines.join("\n");
    }

    format!("{}    {}", log.time, text)
}

/// 判断当前行是否属于上一条警告或错误的说明、堆栈或命令续行。
fn is_diagnostic_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.starts_with(char::is_whitespace)
        || trimmed.starts_with("at ")
        || trimmed.starts_with("Caused by:")
        || trimmed.starts_with("To ")
        || trimmed.starts_with("For ")
        || trimmed.starts_with("because ")
        || trimmed.starts_with("原因：")
        || trimmed.starts_with("请")
}

/// 将过长的启动命令按单词折行，避免一条紫色长行占满整个控制台。
fn wrap_log_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let needed = current.chars().count()
            + usize::from(!current.is_empty())
            + word.chars().count();
        if needed > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn log_highlight_format(
    highlight: &LogHighlight,
    theme: &Theme,
) -> highlighter::Format<iced::Font> {
    let color = match highlight {
        LogHighlight::Info => BLUE_600,
        LogHighlight::Success => SUCCESS,
        LogHighlight::Warning => WARNING,
        LogHighlight::Error => DANGER,
        LogHighlight::Output => crate::theme::text(theme),
        LogHighlight::System => Color::from_rgb8(178, 102, 255),
    };
    highlighter::Format {
        color: Some(color),
        font: Some(crate::core::typography::regular()),
    }
}

fn export_directory(configured: &str) -> PathBuf {
    if configured.trim().is_empty() {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Downloads");
    }
    crate::core::tavern_config::expand_home(configured)
}

fn export_filename() -> String {
    let stamp = Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|stamp| !stamp.is_empty())
        .unwrap_or_else(|| "20260907-000000".to_owned());
    format!("astrabrew-sillytavern-{stamp}.log")
}

/// 限制单行日志长度并移除会干扰文本布局的控制字符。
fn sanitize_log_text(text: String) -> String {
    const MAX_LOG_CHARS: usize = 4_096;
    let mut sanitized = String::with_capacity(text.len().min(MAX_LOG_CHARS));
    for (index, character) in text.chars().enumerate() {
        if index >= MAX_LOG_CHARS {
            sanitized.push('…');
            break;
        }
        if character == '\t' || !character.is_control() {
            sanitized.push(character);
        }
    }
    sanitized
}

fn classify_log(line: &str) -> LogKind {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fatal")
        || lower.contains("exception")
        || lower.contains("traceback")
        || lower.contains("unhandled")
        || lower.contains("npm err")
        || lower.contains("panic")
        || lower.contains("eaddrinuse")
        || line.contains("[错误]")
    {
        LogKind::Error
    } else if lower.contains("warn")
        || lower.contains("deprecated")
        || line.contains("[警告]")
    {
        LogKind::Warning
    } else if line.contains("[系统]") || line.contains("[启动命令]") {
        LogKind::System
    } else if lower.contains("listening")
        || lower.contains("go to:")
        || lower.contains("success")
        || lower.contains("started")
        || lower.contains("ready")
    {
        LogKind::Success
    } else if lower.contains("info") {
        LogKind::Info
    } else {
        LogKind::Output
    }
}

fn timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() + 8 * 3600)
        .unwrap_or_default();
    format!("{:02}:{:02}:{:02}", (seconds / 3600) % 24, (seconds / 60) % 60, seconds % 60)
}

fn url_port(url: &str) -> Option<u16> {
    let authority = url.split("//").nth(1)?.split('/').next()?;
    authority.rsplit(':').next()?.parse().ok().or(Some(if url.starts_with("https://") { 443 } else { 80 }))
}

fn local_ip() -> Option<String> {
    for interface in ["en0", "en1"] {
        let output = Command::new("ipconfig").args(["getifaddr", interface]).output().ok()?;
        if output.status.success() {
            let address = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !address.is_empty() {
                return Some(address);
            }
        }
    }
    None
}

fn tr(key: &'static str) -> &'static str {
    t(key, current_language())
}

fn header_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border { color: crate::theme::line(theme), width: 1.0, ..Border::default() },
        ..Default::default()
    }
}
fn log_editor_style(
    theme: &Theme,
    _status: text_editor::Status,
) -> text_editor::Style {
    text_editor::Style {
        background: Background::Color(crate::theme::surface_alt(theme)),
        border: Border::default(),
        placeholder: crate::theme::text_muted(theme),
        value: crate::theme::text(theme),
        selection: Color::from_rgba(BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.32),
    }
}

fn log_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style { background: Some(Background::Color(crate::theme::surface_alt(theme))), ..Default::default() }
}
fn separator_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style { background: Some(Background::Color(crate::theme::line(theme))), ..Default::default() }
}
fn soft_surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(color.r, color.g, color.b, 0.11))),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..Default::default()
    }
}
fn soft_button(color: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| button::Style {
        background: Some(Background::Color(Color::from_rgba(color.r, color.g, color.b, if matches!(status, button::Status::Hovered | button::Status::Pressed) { 0.18 } else { 0.11 }))),
        border: Border { color: Color::from_rgba(color.r, color.g, color.b, 0.26), width: 1.0, radius: 8.0.into() },
        ..Default::default()
    }
}
fn icon_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    |theme, status| button::Style {
        background: matches!(status, button::Status::Hovered).then_some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..Default::default()
    }
}
fn dialog_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border { color: crate::theme::line(theme), width: 1.0, radius: 14.0.into() },
        ..Default::default()
    }
}
fn dialog_code_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border { color: crate::theme::line(theme), width: 1.0, radius: 8.0.into() },
        ..Default::default()
    }
}
fn backdrop_surface(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style { background: Some(Background::Color(Color::from_rgba(15.0 / 255.0, 17.0 / 255.0, 26.0 / 255.0, 0.45))), ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::{
        ConsoleLog, ConsoleMessage, ConsoleState, LogHighlight, LogHighlighter, LogKind,
        MAX_LOG_LINES, classify_log, render_log_line, url_port,
    };
    use iced::advanced::text::Highlighter;
    use std::sync::Arc;

    #[test]
    fn classifies_logs_and_extracts_ports() {
        assert_eq!(classify_log("Error: failed"), LogKind::Error);
        assert_eq!(classify_log("Server listening"), LogKind::Success);
        assert_eq!(url_port("http://localhost:8000/"), Some(8000));
    }

    #[test]
    fn highlighter_update_restarts_from_first_line() {
        let initial = Arc::new(vec![LogHighlight::Output]);
        let mut highlighter = LogHighlighter::new(&initial);
        let _ = highlighter.highlight_line("output").next();
        assert_eq!(highlighter.current_line(), 1);

        let updated = Arc::new(vec![LogHighlight::Warning, LogHighlight::Error]);
        highlighter.update(&updated);
        assert_eq!(highlighter.current_line(), 0);
        assert_eq!(
            highlighter.highlight_line("warning").next().map(|(_, kind)| kind),
            Some(LogHighlight::Warning)
        );
        assert_eq!(
            highlighter.highlight_line("error").next().map(|(_, kind)| kind),
            Some(LogHighlight::Error)
        );
    }

    #[test]
    fn rendered_log_does_not_show_internal_type_label() {
        let rendered = render_log_line(&ConsoleLog {
            time: "22:35:50".to_owned(),
            kind: LogKind::Warning,
            text: "Warning: unsafe configuration".to_owned(),
        });
        assert_eq!(rendered, "22:35:50    Warning: unsafe configuration");
        assert!(!rendered.contains("[warning]"));
        assert!(!rendered.contains("警告  "));
    }

    #[test]
    fn warning_and_error_lines_keep_exact_highlight_metadata() {
        let mut state = ConsoleState::default();
        let _ = state.update(ConsoleMessage::ClearLogs);
        state.push_process_log("Warning: unsafe configuration".to_owned());
        state.push_process_log("To enable protection, update config.yaml".to_owned());
        state.push_process_log("Error: address already in use".to_owned());

        assert_eq!(state.logs[0].kind, LogKind::Warning);
        assert_eq!(state.logs[1].kind, LogKind::Warning);
        assert_eq!(state.logs[2].kind, LogKind::Error);
        assert_eq!(
            state.log_highlights.as_slice(),
            &[LogHighlight::Warning, LogHighlight::Warning, LogHighlight::Error]
        );
    }

    #[test]
    fn clearing_log_buffers_removes_previous_session_state() {
        let mut state = ConsoleState::default();
        state.push(LogKind::Warning, "old warning");
        state.stream_context = Some(LogKind::Warning);
        state.clear_log_buffers();

        assert!(state.logs.is_empty());
        assert!(state.log_content.is_empty());
        assert!(state.log_highlights.is_empty());
        assert!(state.stream_context.is_none());
        assert!(state.auto_scroll);
    }

    #[test]
    fn trims_old_logs() {
        let mut state = ConsoleState::default();
        for index in 0..=MAX_LOG_LINES {
            state.push(LogKind::Output, index.to_string());
        }
        assert_eq!(state.logs.len(), MAX_LOG_LINES);
    }
}
