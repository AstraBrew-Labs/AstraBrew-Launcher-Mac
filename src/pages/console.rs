use crate::core::tavern_process::TavernProcess;
use crate::lang;
use crate::pages::settings::{Language, ProxyType, TavernDataMode};
use egui::text::LayoutJob;
use egui::{Color32, RichText, TextFormat, Vec2};

#[derive(PartialEq, Clone)]
pub enum ConsoleStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
}

pub struct ConsoleState {
    pub status: ConsoleStatus,
    pub logs: Vec<String>,

    // 进程管理
    process: TavernProcess,
    /// 重启标志：停止完成后自动启动
    restart_pending: bool,
    /// 酒馆实例工作目录
    instance_path: String,
    /// 实例类型（"builtin" / "local"）
    instance_type: String,
    /// 实例版本号
    instance_version: String,
    /// 当前数据模式
    data_mode: TavernDataMode,
    /// HTTP 代理类型
    proxy_type: ProxyType,
    /// 自定义代理地址（ProxyType::Custom 时有效）
    custom_proxy: String,
    /// GitHub 加速代理 URL（启用时通过拦截器注入）
    github_proxy_url: Option<String>,
    /// 是否在启动日志中显示完整命令行
    show_startup_command: bool,
    /// 酒馆访问地址（从日志中解析 "Go to: http://... to open SillyTavern"）
    tavern_url: Option<String>,
}

impl ConsoleState {
    pub fn new() -> Self {
        Self {
            status: ConsoleStatus::Stopped,
            logs: vec![String::from("[系统] 控制台已就绪")],
            process: TavernProcess::new(),
            restart_pending: false,
            instance_path: String::new(),
            instance_type: String::new(),
            instance_version: String::new(),
            data_mode: TavernDataMode::Current,
            proxy_type: ProxyType::None,
            custom_proxy: String::new(),
            github_proxy_url: None,
            show_startup_command: false,
            tavern_url: None,
        }
    }

    /// 同步来自 SettingsState 的配置（每帧调用）
    pub fn sync_with_settings(
        &mut self,
        instance_path: String,
        instance_type: String,
        instance_version: String,
        data_mode: &TavernDataMode,
        proxy_type: &ProxyType,
        custom_proxy: &str,
        github_proxy_url: Option<String>,
        show_startup_command: bool,
    ) {
        self.instance_path = instance_path;
        self.instance_type = instance_type;
        self.instance_version = instance_version;
        self.data_mode = data_mode.clone();
        self.proxy_type = proxy_type.clone();
        self.custom_proxy = custom_proxy.to_string();
        self.github_proxy_url = github_proxy_url;
        self.show_startup_command = show_startup_command;
    }

    /// 是否有已选择的酒馆实例
    pub fn has_instance(&self) -> bool {
        !self.instance_path.is_empty()
    }

    // ---- 进程操作（供 UI 按钮和主页调用）----

    /// 根据 proxy_type 解析实际代理地址
    fn resolve_proxy(&self) -> Option<String> {
        match self.proxy_type {
            ProxyType::None => None,
            ProxyType::Custom => {
                if self.custom_proxy.is_empty() {
                    None
                } else {
                    Some(self.custom_proxy.clone())
                }
            }
            ProxyType::System => {
                // 读取 macOS 系统代理（优先 HTTPS，回退 HTTP）
                crate::core::network::read_system_proxy()
                    .map(|(addr, _enabled)| addr)
            }
        }
    }

    /// 启动酒馆
    pub fn start(&mut self, lang: &Language) {
        if !self.has_instance() {
            self.add_log("[错误] 未选择酒馆实例，请先前往版本管理选择");
            return;
        }
        if self.process.is_running() {
            self.add_log("[警告] 酒馆已在运行中");
            return;
        }

        // 启动前清空日志
        self.logs.clear();
        self.tavern_url = None;

        self.status = ConsoleStatus::Starting;
        self.add_log(&lang::t("console_log_starting_instance", lang));

        // GitHub 加速与 HTTP 代理互斥，加速优先
        let github_proxy = if self.github_proxy_url.is_some() {
            if crate::core::tavern_process::node_supports_import() {
                self.github_proxy_url.clone()
            } else {
                self.add_log(&format!(
                    "[警告] 当前 Node.js 版本不支持 GitHub 加速拦截器（需要 >= 19），已自动关闭加速"
                ));
                None
            }
        } else {
            None
        };

        let proxy = if github_proxy.is_some() {
            None
        } else {
            self.resolve_proxy()
        };
        if let Some(ref addr) = proxy {
            self.add_log(&format!("[系统] 代理已应用: {}", addr));
        }
        if let Some(ref gh_proxy) = github_proxy {
            self.add_log(&format!("[系统] GitHub 加速已启用: {}", gh_proxy));
        }
        if self.show_startup_command {
            let cmd = crate::core::tavern_process::build_startup_command(
                &self.instance_path,
                &self.data_mode,
                proxy.as_deref(),
                github_proxy.as_deref(),
            );
            self.add_log(&format!("[启动命令] {}", cmd));
        }
        match self.process.start(
            &self.instance_path,
            &self.data_mode,
            proxy.as_deref(),
            github_proxy.as_deref(),
        ) {
            Ok(()) => {
                self.status = ConsoleStatus::Running;
                self.add_log(&lang::t("console_log_started", lang));
            }
            Err(e) => {
                self.status = ConsoleStatus::Stopped;
                self.add_log(&format!("[错误] 启动失败: {}", e));
            }
        }
    }

    /// 优雅停止酒馆
    pub fn stop(&mut self, lang: &Language) {
        if !self.process.is_running() {
            self.status = ConsoleStatus::Stopped;
            return;
        }

        self.status = ConsoleStatus::Stopping;
        self.add_log(&lang::t("console_log_stopping", lang));
        self.process.stop();
    }

    /// 强制停止酒馆
    pub fn force_kill(&mut self, lang: &Language) {
        if !self.process.is_running() {
            self.status = ConsoleStatus::Stopped;
            return;
        }

        self.process.kill();
        self.status = ConsoleStatus::Stopped;
        self.restart_pending = false;
        self.add_log(&lang::t("console_log_killed", lang));
    }

    /// 重启酒馆
    pub fn restart(&mut self, lang: &Language) {
        if !self.process.is_running() {
            // 未运行则直接启动
            self.start(lang);
            return;
        }

        self.status = ConsoleStatus::Stopping;
        self.restart_pending = true;
        self.add_log(&lang::t("console_log_restarting", lang));
        self.process.stop();
    }

    // ---- 每帧轮询 ----

    /// 每帧调用：拉取日志、检测进程退出、处理重启逻辑
    pub fn poll(&mut self, lang: &Language) {
        // 拉取新日志
        let new_logs = self.process.poll_logs();
        for line in new_logs {
            let cleaned = strip_osc(&line);

            // 解析酒馆访问地址: "Go to: http://localhost:11451/ to open SillyTavern"
            if self.tavern_url.is_none() {
                // 先剥离 ANSI 再匹配
                let plain = strip_ansi(&cleaned);
                if let Some(url) = extract_tavern_url(&plain) {
                    self.tavern_url = Some(url);
                }
            }

            self.add_log(&cleaned);
        }

        // 检查进程是否已退出
        if let Some(exit_code) = self.process.check_exited() {
            self.add_log(&format!(
                "[系统] 酒馆进程已退出, 退出码: {}",
                exit_code.map_or("无".to_string(), |c| c.to_string())
            ));

            if self.restart_pending {
                // 重启流程：停止已完成 → 清空日志并自动启动
                self.restart_pending = false;
                self.logs.clear();
                self.add_log(&lang::t("console_log_restarting_start", lang));

                let github_proxy = if self.github_proxy_url.is_some() {
                    if crate::core::tavern_process::node_supports_import() {
                        self.github_proxy_url.clone()
                    } else {
                        self.add_log(&format!(
                            "[警告] 当前 Node.js 版本不支持 GitHub 加速拦截器（需要 >= 19），已自动关闭加速"
                        ));
                        None
                    }
                } else {
                    None
                };

                let proxy = if github_proxy.is_some() {
                    None
                } else {
                    self.resolve_proxy()
                };
                if let Some(ref addr) = proxy {
                    self.add_log(&format!("[系统] 代理已应用: {}", addr));
                }
                if let Some(ref gh_proxy) = github_proxy {
                    self.add_log(&format!("[系统] GitHub 加速已启用: {}", gh_proxy));
                }
                match self.process.start(
                    &self.instance_path,
                    &self.data_mode,
                    proxy.as_deref(),
                    github_proxy.as_deref(),
                ) {
                    Ok(()) => {
                        self.status = ConsoleStatus::Running;
                        self.add_log(&lang::t("console_log_restarted", lang));
                    }
                    Err(e) => {
                        self.status = ConsoleStatus::Stopped;
                        self.add_log(&format!("[错误] 重启时启动失败: {}", e));
                    }
                }
            } else {
                // 正常停止完成
                self.status = ConsoleStatus::Stopped;
            }
        }

        // 如果状态已为 Stopped 但进程仍在（异常情况），同步状态
        if self.status == ConsoleStatus::Stopped && self.process.is_running() {
            // 不应该出现，但做保护
            self.status = ConsoleStatus::Running;
        }
    }

    // ---- 日志 ----

    pub fn add_log(&mut self, msg: &str) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                let h = (secs / 3600) % 24 + 8; // UTC+8
                let m = (secs / 60) % 60;
                let s = secs % 60;
                format!("{:02}:{:02}:{:02}", h, m, s)
            })
            .unwrap_or_else(|_| String::from("--:--:--"));
        self.logs.push(format!("[{}] {}", timestamp, msg));
    }
}

/// 剥离 OSC 终端序列（窗口标题设置等），例如 `\x1b]2;...\x07` 或 `\x1b]2;...\x1b\\`
fn strip_osc(line: &str) -> String {
    if !line.contains('\x1b') {
        return line.to_string();
    }
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&']') {
            chars.next(); // skip ']'
            // 消耗直到 BEL (\x07) 或 ST (\x1b\\)
            while let Some(&c) = chars.peek() {
                if c == '\x07' {
                    chars.next();
                    break;
                }
                if c == '\x1b' {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                    break; // 意外情况，跳出
                }
                chars.next();
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// 解析日志行中的 ANSI 颜色转义码，生成 egui LayoutJob
/// - 包含 ANSI 码的行：按码着色
/// - 不包含 ANSI 码的行：根据前缀 [错误]/[警告] 着色
fn parse_ansi_line(line: &str, font_id: egui::FontId) -> LayoutJob {
    // SGR 颜色码 → Color32
    fn sgr_to_color(code: u8) -> Option<Color32> {
        match code {
            30 => Some(Color32::BLACK),
            31 => Some(Color32::RED),
            32 => Some(Color32::GREEN),
            33 => Some(Color32::from_rgb(220, 190, 50)), // 终端黄色
            34 => Some(Color32::from_rgb(80, 120, 255)), // 终端蓝
            35 => Some(Color32::from_rgb(200, 80, 255)), // 终端品红
            36 => Some(Color32::from_rgb(60, 200, 200)), // 终端青
            37 => Some(Color32::from_rgb(210, 210, 210)), // 终端白
            90 => Some(Color32::from_rgb(128, 128, 128)), // 亮黑(灰)
            91 => Some(Color32::from_rgb(255, 110, 110)), // 亮红
            92 => Some(Color32::from_rgb(100, 255, 100)), // 亮绿
            93 => Some(Color32::from_rgb(255, 255, 120)), // 亮黄
            94 => Some(Color32::from_rgb(140, 160, 255)), // 亮蓝
            95 => Some(Color32::from_rgb(255, 130, 255)), // 亮品红
            96 => Some(Color32::from_rgb(100, 255, 255)), // 亮青
            97 => Some(Color32::from_rgb(255, 255, 255)), // 亮白
            _ => None,
        }
    }

    fn default_color() -> Color32 {
        Color32::from_rgb(180, 200, 220)
    }

    // 没有 ANSI 码 → 前缀着色
    if !line.contains('\x1b') {
        let color = if line.contains("[错误]") || line.contains("[ERROR]") {
            Color32::from_rgb(255, 100, 100)
        } else if line.contains("[警告]") || line.contains("[WARN]") {
            Color32::from_rgb(255, 200, 80)
        } else {
            default_color()
        };
        let mut job = LayoutJob::default();
        job.append(
            line,
            0.0,
            TextFormat {
                font_id,
                color,
                ..Default::default()
            },
        );
        return job;
    }

    // 有 ANSI 码 → 逐段解析着色
    let mut job = LayoutJob::default();
    let mut current_color = default_color();
    let mut buf = String::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            // 清空前一个文字段
            if !buf.is_empty() {
                job.append(
                    &buf,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color: current_color,
                        ..Default::default()
                    },
                );
                buf.clear();
            }
            chars.next(); // 跳过 '['

            // 读取参数直到 'm'
            let mut params = String::new();
            while let Some(&p) = chars.peek() {
                if p == 'm' {
                    chars.next();
                    break;
                }
                if p.is_ascii_digit() || p == ';' {
                    params.push(p);
                    chars.next();
                } else {
                    // 非 SGR 序列（如光标移动等），跳过剩余
                    while let Some(&q) = chars.peek() {
                        chars.next();
                        if q.is_ascii_alphabetic() {
                            break;
                        }
                    }
                    params.clear();
                    break;
                }
            }

            if params.is_empty() {
                continue;
            }

            // 应用 SGR 参数
            for code_str in params.split(';') {
                if let Ok(n) = code_str.parse::<u8>() {
                    match n {
                        0 => current_color = default_color(),
                        1 => {} // bold — egui 不支持 layoutjob 内 bold，忽略
                        c => {
                            if let Some(color) = sgr_to_color(c) {
                                current_color = color;
                            }
                        }
                    }
                }
            }
        } else {
            buf.push(ch);
        }
    }

    // 清空末尾文字段
    if !buf.is_empty() {
        job.append(
            &buf,
            0.0,
            TextFormat {
                font_id,
                color: current_color,
                ..Default::default()
            },
        );
    }

    job
}

/// 渲染单行日志：自动识别 URL 并渲染为可点击超链接，无 URL 时使用 ANSI 着色
fn render_log_line(ui: &mut egui::Ui, line: &str, monospace: &egui::FontId) {
    // 检查是否包含 URL
    let has_http = line.contains("http://") || line.contains("https://");

    if !has_http {
        // 无 URL — 纯 ANSI 着色
        ui.label(parse_ansi_line(line, monospace.clone()));
        return;
    }

    // 含 URL — 剥离 ANSI 码后按 URL 拆段渲染
    let plain = strip_ansi(line);
    let url_color = Color32::from_rgb(80, 180, 255);
    let text_color = Color32::from_rgb(180, 200, 220);

    // 前缀着色（系统日志）
    let text_color = if plain.contains("[错误]") || plain.contains("[ERROR]") {
        Color32::from_rgb(255, 100, 100)
    } else if plain.contains("[警告]") || plain.contains("[WARN]") {
        Color32::from_rgb(255, 200, 80)
    } else {
        text_color
    };

    let fmt = |s: &str, c: Color32| {
        RichText::new(s.to_string())
            .font(monospace.clone())
            .color(c)
    };

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let mut remaining = plain.as_str();
        while !remaining.is_empty() {
            if let Some(pos) = remaining.find("http://")
                .or_else(|| remaining.find("https://"))
            {
                // 渲染 URL 前的文本
                if pos > 0 {
                    ui.label(fmt(&remaining[..pos], text_color));
                }
                // 提取 URL（直到空白或行尾）
                let url_start = pos;
                let url_end = remaining[url_start..]
                    .find(|c: char| c.is_whitespace())
                    .map(|p| url_start + p)
                    .unwrap_or(remaining.len());
                let url = &remaining[url_start..url_end];
                ui.add(
                    egui::Hyperlink::from_label_and_url(fmt(url, url_color), url),
                );
                remaining = &remaining[url_end..];
            } else {
                ui.label(fmt(remaining, text_color));
                break;
            }
        }
    });
}

/// 剥离 ANSI 转义码，返回纯文本
fn strip_ansi(line: &str) -> String {
    if !line.contains('\x1b') {
        return line.to_string();
    }
    let mut result = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // skip '['
            // 消耗至终止字符
            while let Some(&c) = chars.peek() {
                chars.next();
                if c.is_ascii_alphabetic() || c == '~' {
                    break;
                }
            }
        } else if ch == '\x1b' && chars.peek() == Some(&']') {
            chars.next();
            while let Some(&c) = chars.peek() {
                if c == '\x07' {
                    chars.next();
                    break;
                }
                if c == '\x1b' {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                    break;
                }
                chars.next();
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn render(ui: &mut egui::Ui, state: &mut ConsoleState, lang: &Language) {
    let available = ui.available_size();

    // ---- 状态栏区域（固定高度）----
    let status_bar_height = 72.0;
    let log_area_height = (available.y - status_bar_height - 8.0).max(100.0);

    // 根据状态选择颜色和图标
    let (status_color, status_icon) = match state.status {
        ConsoleStatus::Stopped => (
            Color32::from_rgb(150, 150, 150),
            egui_phosphor::regular::STOP_CIRCLE,
        ),
        ConsoleStatus::Starting => (
            Color32::from_rgb(255, 200, 50),
            egui_phosphor::regular::ARROW_CLOCKWISE,
        ),
        ConsoleStatus::Running => (
            Color32::from_rgb(80, 220, 80),
            egui_phosphor::regular::PLAY_CIRCLE,
        ),
        ConsoleStatus::Stopping => (
            Color32::from_rgb(255, 150, 50),
            egui_phosphor::regular::STOP_CIRCLE,
        ),
    };

    let (status_title, status_subtitle) = match state.status {
        ConsoleStatus::Stopped => (
            lang::t("console_status_stopped", lang),
            lang::t("console_subtitle_stopped", lang),
        ),
        ConsoleStatus::Starting => (
            lang::t("console_status_starting", lang),
            lang::t("console_subtitle_starting", lang),
        ),
        ConsoleStatus::Running => (
            lang::t("console_status_running", lang),
            lang::t("console_subtitle_running", lang),
        ),
        ConsoleStatus::Stopping => (
            lang::t("console_status_stopping", lang),
            lang::t("console_subtitle_stopping", lang),
        ),
    };

    // 绘制状态栏
    egui::Frame::NONE
        .fill(ui.style().visuals.extreme_bg_color)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // 左侧：状态图标 + 文本
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(status_icon).size(28.0).color(status_color),
                        )
                        .selectable(false),
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::Label::new(RichText::new(status_title).size(18.0).strong())
                                    .selectable(false),
                            );
                            // 访问酒馆链接（运行中 + URL 已捕获时显示）
                            if state.status == ConsoleStatus::Running {
                                if let Some(ref url) = state.tavern_url {
                                    // 分隔符
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new("|").size(18.0).color(Color32::from_rgb(100, 100, 100)),
                                        )
                                        .selectable(false),
                                    );
                                    ui.add_space(6.0);
                                    // 超链接样式（无下划线）
                                    let link_color = Color32::from_rgb(80, 180, 255);
                                    let link = RichText::new(
                                        format!("{} {}", egui_phosphor::regular::GLOBE, lang::t("console_btn_visit", lang)),
                                    )
                                    .size(15.0)
                                    .color(link_color);
                                    let resp = ui.add(
                                        egui::Label::new(link)
                                            .sense(egui::Sense::click()),
                                    );
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        let _ = std::process::Command::new("open")
                                            .arg(url)
                                            .spawn();
                                    }
                                }
                            }
                        });
                        // 显示实例信息
                        if state.has_instance() {
                            if state.instance_type == "builtin" {
                                let subtitle = format!(
                                    "{}  |  {} - v{}",
                                    status_subtitle,
                                    lang::t("console_online_instance", lang),
                                    state.instance_version,
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(subtitle).size(12.0).color(Color32::GRAY),
                                    )
                                    .selectable(false),
                                );
                            } else {
                                // 本地实例：路径按宽度智能截断，框选/点击复制完整路径
                                render_instance_path(
                                    ui,
                                    status_subtitle,
                                    &state.instance_path,
                                );
                            }
                        } else {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(status_subtitle.to_string())
                                        .size(12.0)
                                        .color(Color32::GRAY),
                                )
                                .selectable(false),
                            );
                        }
                    });
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 右侧：按钮组
                    let is_stopped = state.status == ConsoleStatus::Stopped;
                    let is_running = state.status == ConsoleStatus::Running;
                    let is_transitioning = state.status == ConsoleStatus::Starting
                        || state.status == ConsoleStatus::Stopping;

                    let btn_height = 30.0;

                    // 强行停止
                    let kill_enabled = is_running || is_transitioning;
                    let kill_btn = egui::Button::new(
                        RichText::new(lang::t("console_btn_kill", lang)).size(13.0),
                    )
                    .min_size(Vec2::new(90.0, btn_height))
                    .fill(if kill_enabled {
                        Color32::from_rgb(200, 50, 50)
                    } else {
                        Color32::from_rgb(80, 30, 30)
                    });
                    if ui.add_enabled(kill_enabled, kill_btn).clicked() {
                        state.force_kill(lang);
                    }

                    ui.add_space(6.0);

                    // 停止
                    let stop_enabled = is_running;
                    let stop_btn = egui::Button::new(
                        RichText::new(lang::t("console_btn_stop", lang)).size(13.0),
                    )
                    .min_size(Vec2::new(70.0, btn_height))
                    .fill(if stop_enabled {
                        Color32::from_rgb(200, 120, 30)
                    } else {
                        Color32::from_rgb(60, 40, 20)
                    });
                    if ui.add_enabled(stop_enabled, stop_btn).clicked() {
                        state.stop(lang);
                    }

                    ui.add_space(6.0);

                    // 重启
                    let restart_enabled = is_running;
                    let restart_btn = egui::Button::new(
                        RichText::new(lang::t("console_btn_restart", lang)).size(13.0),
                    )
                    .min_size(Vec2::new(70.0, btn_height))
                    .fill(if restart_enabled {
                        Color32::from_rgb(60, 120, 200)
                    } else {
                        Color32::from_rgb(30, 50, 80)
                    });
                    if ui.add_enabled(restart_enabled, restart_btn).clicked() {
                        state.restart(lang);
                    }

                    ui.add_space(6.0);

                    // 启动
                    let start_enabled = is_stopped && state.has_instance();
                    let start_btn = egui::Button::new(
                        RichText::new(lang::t("console_btn_start", lang)).size(13.0),
                    )
                    .min_size(Vec2::new(70.0, btn_height))
                    .fill(if start_enabled {
                        Color32::from_rgb(50, 180, 80)
                    } else {
                        Color32::from_rgb(30, 60, 35)
                    });
                    if ui.add_enabled(start_enabled, start_btn).clicked() {
                        state.start(lang);
                    }
                });
            });
        });

    ui.add_space(8.0);

    // ---- 日志输出区域 ----
    egui::Frame::NONE
        .fill(ui.style().visuals.extreme_bg_color)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(lang::t("console_log_area", lang))
                            .size(13.0)
                            .strong(),
                    )
                    .selectable(false),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [60.0, 22.0],
                            egui::Button::new(
                                RichText::new(lang::t("console_btn_clear", lang)).size(11.0),
                            ),
                        )
                        .clicked()
                    {
                        state.logs.clear();
                        state.add_log(lang::t("console_log_cleared", lang));
                    }
                });
            });
            ui.separator();

            let log_text_height = log_area_height - 36.0;
            egui::ScrollArea::vertical()
                .max_height(log_text_height)
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let monospace = egui::FontId::monospace(12.0);
                    for line in state.logs.iter() {
                        render_log_line(ui, line, &monospace);
                    }
                });
        });
}

/// 渲染本地实例路径：宽度不够时按文件夹级截断，悬停显示完整路径，点击复制
fn render_instance_path(ui: &mut egui::Ui, prefix: &str, full_path: &str) {
    let full_text = format!("{}  |  {}", prefix, full_path);
    let font_id = egui::FontId::monospace(12.0);

    let text_width = ui
        .painter()
        .layout(full_text.clone(), font_id.clone(), Color32::WHITE, f32::MAX)
        .size()
        .x;
    let available = ui.available_width();

    if text_width <= available {
        // 放得下 → 直接显示完整文本，可选
        ui.add(
            egui::Label::new(
                RichText::new(full_text).font(font_id).color(Color32::GRAY),
            )
            .selectable(true),
        );
    } else {
        // 放不下 → 文件夹级截断: xxx/.../xxx
        let display = folder_truncate(full_path);
        let display_text = format!("{}  |  {}", prefix, display);

        let resp = ui
            .add(
                egui::Label::new(
                    RichText::new(display_text).font(font_id).color(Color32::GRAY),
                )
                .sense(egui::Sense::click()),
            )
            .on_hover_text(full_path);
        if resp.clicked() {
            ui.ctx().copy_text(full_path.to_string());
        }
    }
}

/// 文件夹级路径截断：保留首个和末个文件夹，中间用 /.../ 替换
fn folder_truncate(path: &str) -> String {
    let sep = if path.contains('/') { '/' } else { std::path::MAIN_SEPARATOR };
    let parts: Vec<&str> = path.split(sep).filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return path.to_string();
    }
    format!("{}{}...{}{}", parts[0], sep, sep, parts[parts.len() - 1])
}

/// 从日志行提取酒馆访问地址: "Go to: http://localhost:11451/ to open SillyTavern"
fn extract_tavern_url(line: &str) -> Option<String> {
    let prefix = "Go to: ";
    let suffix = " to open SillyTavern";
    if let Some(start) = line.find(prefix) {
        let after = &line[start + prefix.len()..];
        if let Some(end) = after.find(suffix) {
            return Some(after[..end].trim().to_string());
        }
    }
    None
}
