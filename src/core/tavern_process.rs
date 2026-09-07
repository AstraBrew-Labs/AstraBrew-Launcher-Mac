//! SillyTavern 进程运行时。
//!
//! 运行时线程独占直接子进程和 PM2 CLI 调用，通过命令/事件通道与 iced 主线程通信。

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use regex::Regex;
use serde_json::Value;

use crate::core::pm2::Pm2Manager;

/// 酒馆数据目录模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TavernDataMode {
    Current,
    Global,
}

/// 酒馆界面启动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TavernLaunchMode {
    Normal,
    Desktop,
    Server,
}

/// 当前进程由启动器直接持有，或交给 PM2 托管。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Direct,
    Pm2,
}

/// 冻结一次启动所需的全部配置，运行期间设置变化不会污染当前进程。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TavernLaunchSpec {
    pub instance_path: PathBuf,
    pub instance_version: String,
    pub data_mode: TavernDataMode,
    pub global_data_path: PathBuf,
    pub proxy: Option<String>,
    pub github_proxy_url: Option<String>,
    pub launch_mode: TavernLaunchMode,
    pub allow_background: bool,
    pub show_startup_command: bool,
    pub export_path: String,
}

impl TavernLaunchSpec {
    pub fn runtime_mode(&self) -> RuntimeMode {
        if self.launch_mode == TavernLaunchMode::Server
            && self.allow_background
            && Pm2Manager::is_installed()
        {
            RuntimeMode::Pm2
        } else {
            RuntimeMode::Direct
        }
    }

    fn config_path(&self) -> PathBuf {
        match self.data_mode {
            TavernDataMode::Current => self.instance_path.join("config.yaml"),
            TavernDataMode::Global => self.global_data_path.join("config.yaml"),
        }
    }

    fn data_root(&self) -> Option<PathBuf> {
        (self.data_mode == TavernDataMode::Global).then(|| self.global_data_path.clone())
    }
}

/// 端口占用进程，仅允许用户确认后结束这里列出的 PID。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortProcess {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortConflict {
    pub port: u16,
    pub processes: Vec<PortProcess>,
    pub retry_available: bool,
}

/// 主线程发送给进程运行时的命令。
#[derive(Debug, Clone)]
pub enum ProcessCommand {
    Start(TavernLaunchSpec),
    Stop,
    Kill,
    Restart,
    ReleasePortAndRetry(PortConflict),
    Shutdown,
}

/// 进程运行时回传给控制台的事件。
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Starting(RuntimeMode),
    Running(RuntimeMode),
    Stopping,
    Stopped,
    Failed(String),
    Pid(Option<u32>),
    Log(String),
    ServerUrl(String),
    PortConflict(PortConflict),
    Exited(Option<i32>),
    Pm2Unavailable,
    Pm2Restored { pid: Option<u32>, cwd: Option<String> },
}

/// 主线程持有的运行时句柄。
pub struct TavernRuntime {
    command_tx: Sender<ProcessCommand>,
    event_rx: Receiver<ProcessEvent>,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for TavernRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("TavernRuntime").finish_non_exhaustive()
    }
}

impl Default for TavernRuntime {
    fn default() -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let worker = thread::spawn(move || worker_loop(command_rx, event_tx));
        Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        }
    }
}

impl TavernRuntime {
    pub fn send(&self, command: ProcessCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|_| "酒馆进程运行时已经停止。".to_owned())
    }

    pub fn drain(&self) -> Vec<ProcessEvent> {
        let mut events = Vec::new();
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        events
    }
}

impl Drop for TavernRuntime {
    fn drop(&mut self) {
        let _ = self.command_tx.send(ProcessCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct DirectProcess {
    child: Child,
    logs: Receiver<String>,
    stop_deadline: Option<Instant>,
}

struct WorkerState {
    direct: Option<DirectProcess>,
    active_spec: Option<TavernLaunchSpec>,
    active_mode: Option<RuntimeMode>,
    restart_spec: Option<TavernLaunchSpec>,
    conflict_port: Option<u16>,
    conflict_retried: bool,
    pm2: Pm2Manager,
    pm2_out_offset: u64,
    pm2_error_offset: u64,
    last_pm2_poll: Instant,
}

fn worker_loop(commands: Receiver<ProcessCommand>, events: Sender<ProcessEvent>) {
    let mut state = WorkerState {
        direct: None,
        active_spec: None,
        active_mode: None,
        restart_spec: None,
        conflict_port: None,
        conflict_retried: false,
        pm2: Pm2Manager,
        pm2_out_offset: 0,
        pm2_error_offset: 0,
        last_pm2_poll: Instant::now() - Duration::from_secs(2),
    };
    restore_pm2(&mut state, &events);

    loop {
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(ProcessCommand::Start(spec)) => start(&mut state, spec, &events),
            Ok(ProcessCommand::Stop) => stop(&mut state, &events),
            Ok(ProcessCommand::Kill) => kill(&mut state, &events),
            Ok(ProcessCommand::Restart) => restart(&mut state, &events),
            Ok(ProcessCommand::ReleasePortAndRetry(conflict)) => {
                release_port_and_retry(&mut state, conflict, &events)
            }
            Ok(ProcessCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                shutdown(&mut state);
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        poll(&mut state, &events);
    }
}

fn restore_pm2(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    if !Pm2Manager::is_installed() {
        return;
    }
    if let Ok(Some(info)) = state.pm2.info()
        && info.status == "online"
    {
        state.active_mode = Some(RuntimeMode::Pm2);
        // 恢复时只读取新日志，避免历史输出瞬间占满界面与字形缓存。
        state.pm2_out_offset = state.pm2.log_length(false);
        state.pm2_error_offset = state.pm2.log_length(true);
        state.last_pm2_poll = Instant::now() - Duration::from_secs(2);
        let _ = events.send(ProcessEvent::Pm2Restored {
            pid: info.pid,
            cwd: info.cwd,
        });
    }
}

fn start(state: &mut WorkerState, spec: TavernLaunchSpec, events: &Sender<ProcessEvent>) {
    if state.active_mode == Some(RuntimeMode::Pm2) {
        if let Ok(Some(info)) = state.pm2.info() {
            let _ = events.send(ProcessEvent::Pm2Restored {
                pid: info.pid,
                cwd: info.cwd,
            });
        }
        return;
    }
    if state.direct.is_some() || state.active_mode.is_some() {
        let _ = events.send(ProcessEvent::Running(RuntimeMode::Direct));
        return;
    }
    state.conflict_port = None;
    state.conflict_retried = false;
    let requested_pm2 = spec.launch_mode == TavernLaunchMode::Server && spec.allow_background;
    let mode = spec.runtime_mode();
    // Starting 是新日志会话的边界，必须先于校验失败和任何新进程日志。
    let _ = events.send(ProcessEvent::Starting(mode));
    if let Err(error) = validate_spec(&spec) {
        let _ = events.send(ProcessEvent::Failed(error));
        return;
    }
    if let Err(error) = prepare_webui_settings(&spec) {
        let _ = events.send(ProcessEvent::Failed(error));
        return;
    }
    if requested_pm2 && mode == RuntimeMode::Direct {
        let _ = events.send(ProcessEvent::Pm2Unavailable);
    }
    if spec.github_proxy_url.is_some() && !node_supports_import() {
        let warning = "[警告] 当前 Node.js 不支持 GitHub 加速拦截器（需要 19 或更高版本），已回退到普通代理设置。".to_owned();
        let _ = events.send(ProcessEvent::Log(warning));
    }
    if spec.show_startup_command {
        let _ = events.send(ProcessEvent::Log(format!(
            "[启动命令] {}",
            display_command(&spec, mode)
        )));
    }
    let result = match mode {
        RuntimeMode::Direct => start_direct(state, &spec, events),
        RuntimeMode::Pm2 => start_pm2(state, &spec, events),
    };
    match result {
        Ok(()) => {
            state.active_spec = Some(spec);
            state.active_mode = Some(mode);
            let _ = events.send(ProcessEvent::Running(mode));
        }
        Err(error) => {
            state.active_spec = None;
            state.active_mode = None;
            let _ = events.send(ProcessEvent::Failed(error));
        }
    }
}

fn validate_spec(spec: &TavernLaunchSpec) -> Result<(), String> {
    if !spec.instance_path.is_dir() {
        return Err(format!("酒馆实例目录不存在：{}", spec.instance_path.display()));
    }
    if !spec.instance_path.join("server.js").is_file() {
        return Err(format!(
            "所选目录不是可启动的 SillyTavern 实例，缺少 server.js：{}",
            spec.instance_path.display()
        ));
    }
    if crate::core::settings::env_detect::detect_nodejs().is_none() {
        return Err("未检测到可用的 Node.js 和 npm，请先在设置中安装。".to_owned());
    }
    let config = spec.config_path();
    if !config.is_file() {
        return Err(format!("酒馆配置文件不存在：{}", config.display()));
    }
    Ok(())
}

fn prepare_webui_settings(spec: &TavernLaunchSpec) -> Result<(), String> {
    let target = match spec.data_mode {
        TavernDataMode::Current => spec.instance_path.join("data/default-user/settings.json"),
        TavernDataMode::Global => spec.global_data_path.join("default-user/settings.json"),
    };
    if target.exists() {
        return Ok(());
    }
    let parent = target.parent().ok_or_else(|| "酒馆设置目标路径无效。".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建酒馆数据目录 {}：{error}", parent.display()))?;
    crate::utils::app_paths().ensure_default_tavern_settings();
    let template = fs::read_to_string(crate::utils::app_paths().default_tavern_settings_file())
        .unwrap_or_else(|_| crate::utils::TEMPLATE_TAVERN_SETTINGS_JSON.to_owned());
    let mut value: Value = serde_json::from_str(&template)
        .map_err(|error| format!("内置酒馆设置模板无效：{error}"))?;
    if !spec.instance_version.trim().is_empty()
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "currentVersion".to_owned(),
            Value::String(spec.instance_version.clone()),
        );
    }
    let bytes = serde_json::to_vec_pretty(&value)
        .map_err(|error| format!("无法生成酒馆默认设置：{error}"))?;
    fs::write(&target, bytes)
        .map_err(|error| format!("无法写入酒馆默认设置 {}：{error}", target.display()))
}

struct LaunchCommand {
    arguments: Vec<String>,
    environment: Vec<(String, String)>,
    node_import: Option<String>,
}

fn launch_command(spec: &TavernLaunchSpec) -> Result<LaunchCommand, String> {
    let mut arguments = vec!["server.js".to_owned()];
    if let Some(data_root) = spec.data_root() {
        arguments.extend([
            "--configPath".to_owned(),
            data_root.join("config.yaml").to_string_lossy().into_owned(),
            "--dataRoot".to_owned(),
            data_root.to_string_lossy().into_owned(),
        ]);
    }
    if spec.launch_mode != TavernLaunchMode::Normal {
        arguments.extend([
            "--browserLaunchEnabled".to_owned(),
            "false".to_owned(),
        ]);
    }
    let mut environment = Vec::new();
    let mut proxy = spec.proxy.clone().map(|value| normalize_proxy_url(&value));
    let node_import = if let Some(url) = spec.github_proxy_url.as_deref() {
        if node_supports_import() {
            proxy = None;
            let path = prepare_interceptor()?;
            environment.push(("GITHUB_PROXY_URL".to_owned(), url.to_owned()));
            Some(path.to_string_lossy().into_owned())
        } else {
            None
        }
    } else {
        None
    };
    if let Some(proxy) = proxy {
        for key in ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"] {
            environment.push((key.to_owned(), proxy.clone()));
        }
        arguments.extend([
            "--requestProxyEnabled".to_owned(),
            "true".to_owned(),
            "--requestProxyUrl".to_owned(),
            proxy,
            "--requestProxyBypass".to_owned(),
            "localhost 127.0.0.1 ::1".to_owned(),
        ]);
    }
    Ok(LaunchCommand {
        arguments,
        environment,
        node_import,
    })
}

fn start_direct(
    state: &mut WorkerState,
    spec: &TavernLaunchSpec,
    events: &Sender<ProcessEvent>,
) -> Result<(), String> {
    let launch = launch_command(spec)?;
    let mut command = crate::core::settings::env_detect::cmd("node");
    if let Some(import) = launch.node_import {
        command.arg("--import").arg(import);
    }
    command
        .args(&launch.arguments)
        .envs(launch.environment.iter().cloned())
        .current_dir(&spec.instance_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 SillyTavern：{error}"))?;
    let pid = child.id();
    let (log_tx, log_rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(stdout, log_tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(stderr, log_tx);
    }
    state.direct = Some(DirectProcess {
        child,
        logs: log_rx,
        stop_deadline: None,
    });
    let _ = events.send(ProcessEvent::Pid(Some(pid)));
    Ok(())
}

fn spawn_log_reader(reader: impl std::io::Read + Send + 'static, sender: Sender<String>) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn start_pm2(
    state: &mut WorkerState,
    spec: &TavernLaunchSpec,
    events: &Sender<ProcessEvent>,
) -> Result<(), String> {
    let launch = launch_command(spec)?;
    state.pm2.clear_logs();
    state.pm2_out_offset = 0;
    state.pm2_error_offset = 0;
    let node_args = launch
        .node_import
        .as_deref()
        .map(|path| format!("--import {path}"));
    state.pm2.start(
        &spec.instance_path,
        node_args.as_deref(),
        &launch.arguments[1..],
        &launch.environment,
    )?;
    if let Some(info) = state.pm2.info()? {
        let _ = events.send(ProcessEvent::Pid(info.pid));
    }
    Ok(())
}

fn stop(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    let Some(mode) = state.active_mode else {
        return;
    };
    let _ = events.send(ProcessEvent::Stopping);
    match mode {
        RuntimeMode::Direct => {
            if let Some(process) = state.direct.as_mut() {
                let _ = Command::new("kill")
                    .arg("-TERM")
                    .arg(process.child.id().to_string())
                    .status();
                process.stop_deadline = Some(Instant::now() + Duration::from_secs(8));
            }
        }
        RuntimeMode::Pm2 => match state.pm2.stop() {
            Ok(()) => finish_stopped(state, events),
            Err(error) => recover_pm2_after_command_error(state, events, error),
        },
    }
}

fn kill(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    match state.active_mode {
        Some(RuntimeMode::Direct) => {
            if let Some(mut process) = state.direct.take() {
                let _ = process.child.kill();
                let _ = process.child.wait();
            }
            finish_stopped(state, events);
        }
        Some(RuntimeMode::Pm2) => match state.pm2.delete() {
            Ok(()) => finish_stopped(state, events),
            Err(error) => recover_pm2_after_command_error(state, events, error),
        },
        None => {}
    }
}

fn restart(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    if state.active_mode == Some(RuntimeMode::Pm2) {
        let _ = events.send(ProcessEvent::Starting(RuntimeMode::Pm2));
        state.pm2.clear_logs();
        state.pm2_out_offset = 0;
        state.pm2_error_offset = 0;
        match state.pm2.restart() {
            Ok(()) => {
                let _ = events.send(ProcessEvent::Running(RuntimeMode::Pm2));
            }
            Err(error) => recover_pm2_after_command_error(state, events, error),
        }
        return;
    }
    let Some(spec) = state.active_spec.clone() else {
        return;
    };
    match state.active_mode {
        Some(RuntimeMode::Direct) => {
            state.restart_spec = Some(spec);
            stop(state, events);
        }
        Some(RuntimeMode::Pm2) => unreachable!("PM2 已在前置分支处理"),
        None => start(state, spec, events),
    }
}

fn release_port_and_retry(
    state: &mut WorkerState,
    conflict: PortConflict,
    events: &Sender<ProcessEvent>,
) {
    if state.conflict_retried
        || state.conflict_port != Some(conflict.port)
        || conflict.processes.is_empty()
    {
        let _ = events.send(ProcessEvent::Failed(
            "端口占用信息已经失效，请重新启动后再试。".to_owned(),
        ));
        return;
    }
    let current = match query_port_processes(conflict.port) {
        Ok(processes) => processes,
        Err(error) => {
            let _ = events.send(ProcessEvent::Failed(error));
            return;
        }
    };
    let confirmed: Vec<_> = current
        .into_iter()
        .filter(|current| {
            conflict
                .processes
                .iter()
                .any(|old| old.pid == current.pid && old.name == current.name)
        })
        .collect();
    if confirmed.is_empty() {
        let _ = events.send(ProcessEvent::Failed(
            "端口占用进程已经变化，未结束任何程序，请重新启动后确认。".to_owned(),
        ));
        return;
    }
    for process in &confirmed {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(process.pid.to_string())
            .status();
    }
    thread::sleep(Duration::from_millis(500));
    let remaining = query_port_processes(conflict.port).unwrap_or_default();
    for process in remaining {
        if confirmed
            .iter()
            .any(|old| old.pid == process.pid && old.name == process.name)
        {
            let _ = Command::new("kill")
                .arg("-KILL")
                .arg(process.pid.to_string())
                .status();
        }
    }
    state.conflict_retried = true;
    state.conflict_port = None;
    if let Some(spec) = state.active_spec.take() {
        state.active_mode = None;
        start(state, spec, events);
        // `start` 会初始化普通启动状态；端口释放路径必须保留“已经重试过”。
        state.conflict_retried = true;
    }
}

fn poll(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    if state.active_mode == Some(RuntimeMode::Direct) {
        poll_direct(state, events);
    } else if state.active_mode == Some(RuntimeMode::Pm2)
        && state.last_pm2_poll.elapsed() >= Duration::from_secs(1)
    {
        poll_pm2(state, events);
        state.last_pm2_poll = Instant::now();
    }
}

fn poll_direct(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    let mut exited = None;
    let mut pending_logs = Vec::new();
    if let Some(process) = state.direct.as_mut() {
        while let Ok(line) = process.logs.try_recv() {
            pending_logs.push(line);
        }
        if process.stop_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let _ = process.child.kill();
        }
        match process.child.try_wait() {
            Ok(Some(status)) => exited = Some(status.code()),
            Ok(None) => {}
            Err(error) => {
                let _ = events.send(ProcessEvent::Log(format!(
                    "[错误] 无法查询酒馆进程状态：{error}"
                )));
            }
        }
    }
    for line in pending_logs {
        emit_log(state, events, line);
    }
    if let Some(code) = exited {
        state.direct = None;
        let _ = events.send(ProcessEvent::Pid(None));
        let _ = events.send(ProcessEvent::Exited(code));
        if let Some(port) = state.conflict_port {
            let processes = query_port_processes(port).unwrap_or_default();
            let _ = events.send(ProcessEvent::PortConflict(PortConflict {
                port,
                retry_available: !processes.is_empty() && !state.conflict_retried,
                processes,
            }));
            state.active_mode = None;
            return;
        }
        if let Some(spec) = state.restart_spec.take() {
            state.active_mode = None;
            state.active_spec = None;
            start(state, spec, events);
        } else {
            finish_stopped(state, events);
        }
    }
}

fn poll_pm2(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    for error_log in [false, true] {
        let offset = if error_log {
            &mut state.pm2_error_offset
        } else {
            &mut state.pm2_out_offset
        };
        if let Ok(lines) = state.pm2.read_log(error_log, offset) {
            for line in lines {
                emit_log(state, events, line);
            }
        }
    }
    match state.pm2.info() {
        Ok(Some(info)) if info.status == "online" => {
            let _ = events.send(ProcessEvent::Pid(info.pid));
        }
        Ok(Some(info)) if matches!(info.status.as_str(), "launching" | "stopping") => {}
        Ok(Some(info)) if info.status == "errored" => {
            state.active_mode = None;
            state.active_spec = None;
            let _ = events.send(ProcessEvent::Failed("PM2 中的 SillyTavern 进程进入错误状态。".to_owned()));
        }
        Ok(_) => finish_stopped(state, events),
        Err(error) => {
            let _ = events.send(ProcessEvent::Log(format!("[错误] {error}")));
        }
    }
}

/// PM2 命令失败后重新查询真实状态，避免界面误报服务已经停止。
fn recover_pm2_after_command_error(
    state: &mut WorkerState,
    events: &Sender<ProcessEvent>,
    error: String,
) {
    let _ = events.send(ProcessEvent::Log(format!("[错误] {error}")));
    match state.pm2.info() {
        Ok(Some(info)) if info.status == "online" => {
            state.active_mode = Some(RuntimeMode::Pm2);
            let _ = events.send(ProcessEvent::Pid(info.pid));
            let _ = events.send(ProcessEvent::Running(RuntimeMode::Pm2));
        }
        Ok(_) => finish_stopped(state, events),
        Err(query_error) => {
            let _ = events.send(ProcessEvent::Failed(format!(
                "{error}；随后查询 PM2 状态也失败：{query_error}"
            )));
        }
    }
}

fn emit_log(state: &mut WorkerState, events: &Sender<ProcessEvent>, line: String) {
    let cleaned = strip_terminal_sequences(&line);
    if let Some(url) = extract_tavern_url(&cleaned) {
        let _ = events.send(ProcessEvent::ServerUrl(url));
    }
    if let Some(port) = extract_conflict_port(&cleaned) {
        state.conflict_port = Some(port);
    }
    let _ = events.send(ProcessEvent::Log(cleaned));
}

fn finish_stopped(state: &mut WorkerState, events: &Sender<ProcessEvent>) {
    state.direct = None;
    state.active_mode = None;
    state.active_spec = None;
    state.restart_spec = None;
    let _ = events.send(ProcessEvent::Pid(None));
    let _ = events.send(ProcessEvent::Stopped);
}

fn shutdown(state: &mut WorkerState) {
    if state.active_mode == Some(RuntimeMode::Direct)
        && let Some(mut process) = state.direct.take()
    {
        let _ = process.child.kill();
        let _ = process.child.wait();
    }
}

/// 统一代理地址格式。
pub fn normalize_proxy_url(value: &str) -> String {
    let value = value.trim();
    if ["http://", "https://", "socks5://", "socks4://"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        value.to_owned()
    } else {
        format!("http://{value}")
    }
}

fn node_supports_import() -> bool {
    crate::core::settings::env_detect::detect_nodejs()
        .and_then(|version| {
            version
                .trim_start_matches('v')
                .split('.')
                .next()
                .and_then(|major| major.parse::<u32>().ok())
        })
        .is_some_and(|major| major >= 19)
}

const INTERCEPTOR: &str = r#"const base=(process.env.GITHUB_PROXY_URL||'').replace(/\/+$/,'');
const rewrite=(value)=>typeof value==='string'&&(value.includes('github.com')||value.includes('raw.githubusercontent.com'))&&!value.includes('api.github.com')?base+'/'+value:value;
for (const name of ['https','http']) { const mod=await import(name); const original=mod.default.request; mod.default.request=function(url,...rest){ return original.call(this,rewrite(url),...rest); }; }
"#;

fn prepare_interceptor() -> Result<PathBuf, String> {
    let path = crate::utils::app_paths().temp.join("github-interceptor.mjs");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("无法创建临时目录：{error}"))?;
    }
    fs::write(&path, INTERCEPTOR)
        .map_err(|error| format!("无法写入 GitHub 加速脚本：{error}"))?;
    Ok(path)
}

fn display_command(spec: &TavernLaunchSpec, mode: RuntimeMode) -> String {
    let launch = launch_command(spec).unwrap_or(LaunchCommand {
        arguments: vec!["server.js".to_owned()],
        environment: Vec::new(),
        node_import: None,
    });
    let mut parts = Vec::new();
    for (key, value) in launch.environment {
        parts.push(format!("{key}={value}"));
    }
    match mode {
        RuntimeMode::Direct => {
            parts.push("node".to_owned());
            if let Some(path) = launch.node_import {
                parts.extend(["--import".to_owned(), shell_quote(&path)]);
            }
            parts.extend(launch.arguments.into_iter().map(|part| shell_quote(&part)));
        }
        RuntimeMode::Pm2 => {
            parts.extend([
                "pm2".to_owned(),
                "start".to_owned(),
                "server.js".to_owned(),
                "--name".to_owned(),
                crate::core::pm2::PROCESS_NAME.to_owned(),
            ]);
            if let Some(path) = launch.node_import {
                parts.extend(["--node-args".to_owned(), shell_quote(&format!("--import {path}"))]);
            }
            if launch.arguments.len() > 1 {
                parts.push("--".to_owned());
                parts.extend(launch.arguments[1..].iter().map(|part| shell_quote(part)));
            }
        }
    }
    parts.join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.chars().all(|ch| ch.is_ascii_alphanumeric() || "-._/:".contains(ch)) {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// 清理 ANSI SGR 与 OSC 控制序列，日志文件和界面仅保留可读文本。
pub fn strip_terminal_sequences(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut result = String::with_capacity(line.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0x1b {
            let ch = line[index..].chars().next().unwrap_or_default();
            result.push(ch);
            index += ch.len_utf8();
            continue;
        }
        index += 1;
        if index >= bytes.len() {
            break;
        }
        match bytes[index] {
            b'[' => {
                index += 1;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            b']' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == 0x07 {
                        index += 1;
                        break;
                    }
                    if bytes[index] == 0x1b
                        && bytes.get(index + 1).copied() == Some(b'\\')
                    {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    result
}

/// SillyTavern 启动完成后输出的唯一可信访问地址格式。
///
/// 只接受 `Go to: <URL> to open SillyTavern`，禁止从启动命令、代理配置或
/// 其他包含 HTTP 地址的普通日志中猜测，避免把代理端口当成酒馆端口。
static TAVERN_URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bGo\s+to:\s*(https?://[^\s]+?)\s+to\s+open\s+SillyTavern\b")
        .expect("固定的 SillyTavern 地址正则必须有效")
});

pub fn extract_tavern_url(line: &str) -> Option<String> {
    let captures = TAVERN_URL_PATTERN.captures(line)?;
    let url = captures.get(1)?.as_str().trim_end_matches([',', '.', ')']);
    Some(url.to_owned())
}


pub fn extract_conflict_port(line: &str) -> Option<u16> {
    let lower = line.to_ascii_lowercase();
    if !lower.contains("eaddrinuse") && !lower.contains("already in use") {
        return None;
    }
    line.rsplit(':')
        .find_map(|part| {
            let digits: String = part.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
        })
        .or_else(|| {
            line.split(|ch: char| !ch.is_ascii_digit())
                .filter(|value| value.len() >= 2)
                .filter_map(|value| value.parse::<u16>().ok())
                .next_back()
        })
}

pub fn query_port_processes(port: u16) -> Result<Vec<PortProcess>, String> {
    let output = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fpc"])
        .output()
        .map_err(|error| format!("无法查询端口占用：{error}"))?;
    if !output.status.success() && output.stdout.is_empty() {
        return Ok(Vec::new());
    }
    parse_lsof_processes(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_lsof_processes(output: &str) -> Result<Vec<PortProcess>, String> {
    let mut processes = Vec::new();
    let mut pid = None;
    for line in output.lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = value.parse::<u32>().ok();
        } else if let Some(name) = line.strip_prefix('c')
            && let Some(pid) = pid.take()
        {
            if !processes.iter().any(|process: &PortProcess| process.pid == pid) {
                processes.push(PortProcess {
                    pid,
                    name: name.to_owned(),
                });
            }
        }
    }
    if processes.is_empty() && !output.trim().is_empty() {
        return Err("无法解析端口占用进程。".to_owned());
    }
    Ok(processes)
}

/// 创建规范日志目录，并保证当前日志文件存在。
pub fn ensure_sillytavern_log_file() -> Result<(), String> {
    let paths = crate::utils::app_paths();
    fs::create_dir_all(&paths.logs)
        .map_err(|error| format!("无法创建酒馆日志目录 {}：{error}", paths.logs.display()))?;
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.sillytavern_log_file())
        .map(|_| ())
        .map_err(|error| format!("无法创建酒馆日志文件：{error}"))
}

/// 新启动前将当前日志轮换为 latest，并创建新的实时日志。
pub fn prepare_sillytavern_log_file() -> Result<(), String> {
    let paths = crate::utils::app_paths();
    fs::create_dir_all(&paths.logs)
        .map_err(|error| format!("无法创建酒馆日志目录 {}：{error}", paths.logs.display()))?;
    let current = paths.sillytavern_log_file();
    let latest = paths.sillytavern_latest_log_file();
    if current.exists() {
        if latest.exists() {
            fs::remove_file(&latest)
                .map_err(|error| format!("无法替换上一份酒馆日志：{error}"))?;
        }
        fs::rename(&current, &latest)
            .map_err(|error| format!("无法轮换酒馆日志：{error}"))?;
    }
    fs::File::create(&current)
        .map(|_| ())
        .map_err(|error| format!("无法创建酒馆实时日志 {}：{error}", current.display()))
}

/// 将一行已经清理和分类的日志写入规范实时日志。
pub fn append_sillytavern_log_line(line: &str) -> Result<(), String> {
    let path = crate::utils::app_paths().sillytavern_log_file();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("无法打开酒馆实时日志 {}：{error}", path.display()))?;
    writeln!(file, "{line}")
        .map_err(|error| format!("无法写入酒馆实时日志 {}：{error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        TavernDataMode, TavernLaunchMode, TavernLaunchSpec, extract_conflict_port,
        extract_tavern_url, launch_command, normalize_proxy_url, parse_lsof_processes,
        strip_terminal_sequences,
    };
    use std::path::PathBuf;

    #[test]
    fn builds_global_data_and_proxy_arguments() {
        let command = launch_command(&TavernLaunchSpec {
            instance_path: PathBuf::from("/tmp/tavern"),
            instance_version: "1.0.0".to_owned(),
            data_mode: TavernDataMode::Global,
            global_data_path: PathBuf::from("/tmp/global"),
            proxy: Some("127.0.0.1:7890".to_owned()),
            github_proxy_url: None,
            launch_mode: TavernLaunchMode::Server,
            allow_background: false,
            show_startup_command: true,
            export_path: "/tmp".to_owned(),
        })
        .unwrap();
        assert!(
            command
                .arguments
                .windows(2)
                .any(|pair| pair == ["--dataRoot", "/tmp/global"])
        );
        assert!(
            command
                .arguments
                .windows(2)
                .any(|pair| pair == ["--browserLaunchEnabled", "false"])
        );
        assert!(command.environment.iter().any(|(key, value)| {
            key == "HTTP_PROXY" && value == "http://127.0.0.1:7890"
        }));
    }

    #[test]
    fn browser_launch_argument_matches_launch_mode() {
        let base = TavernLaunchSpec {
            instance_path: PathBuf::from("/tmp/tavern"),
            instance_version: "1.0.0".to_owned(),
            data_mode: TavernDataMode::Current,
            global_data_path: PathBuf::from("/tmp/global"),
            proxy: None,
            github_proxy_url: None,
            launch_mode: TavernLaunchMode::Normal,
            allow_background: false,
            show_startup_command: false,
            export_path: "/tmp".to_owned(),
        };
        let normal = launch_command(&base).unwrap();
        assert!(!normal.arguments.iter().any(|argument| argument == "--browserLaunchEnabled"));

        let desktop = launch_command(&TavernLaunchSpec {
            launch_mode: TavernLaunchMode::Desktop,
            ..base.clone()
        })
        .unwrap();
        assert!(desktop.arguments.windows(2).any(|pair| {
            pair == ["--browserLaunchEnabled", "false"]
        }));

        let server = launch_command(&TavernLaunchSpec {
            launch_mode: TavernLaunchMode::Server,
            ..base
        })
        .unwrap();
        assert!(server.arguments.windows(2).any(|pair| {
            pair == ["--browserLaunchEnabled", "false"]
        }));
    }

    #[test]
    fn normalizes_proxy_protocol() {
        assert_eq!(normalize_proxy_url("127.0.0.1:7890"), "http://127.0.0.1:7890");
        assert_eq!(normalize_proxy_url("socks5://127.0.0.1:1080"), "socks5://127.0.0.1:1080");
    }

    #[test]
    fn extracts_url_and_conflict_port() {
        assert_eq!(
            extract_tavern_url("Go to: http://localhost:8000/ to open SillyTavern").as_deref(),
            Some("http://localhost:8000/")
        );
        assert_eq!(
            extract_tavern_url(
                "[启动命令] HTTP_PROXY=http://127.0.0.1:7892 node server.js --requestProxyUrl http://127.0.0.1:7892"
            ),
            None
        );
        assert_eq!(
            extract_tavern_url("Proxy URL is used: http://127.0.0.1:7892"),
            None
        );
        assert_eq!(extract_conflict_port("Error: listen EADDRINUSE: address already in use :::8000"), Some(8000));
    }

    #[test]
    fn strips_ansi_and_osc_sequences() {
        assert_eq!(strip_terminal_sequences("\x1b[31merror\x1b[0m"), "error");
        assert_eq!(strip_terminal_sequences("a\x1b]2;title\x07b"), "ab");
    }

    #[test]
    fn parses_lsof_records() {
        let processes = parse_lsof_processes("p42\ncnode\np43\ncother\n").unwrap();
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].pid, 42);
        assert_eq!(processes[0].name, "node");
    }
}
