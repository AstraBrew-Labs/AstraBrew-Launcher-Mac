use eframe::egui;
use egui::{FontData, FontDefinitions, FontFamily};

fn main() -> eframe::Result {
    let settings = pages::settings::SettingsState::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0])
        .with_max_inner_size([1280.0, 720.0])
        .with_maximize_button(false);

    let mut is_centered = true;
    if settings.remember_window_pos {
        if let Some(pos) = settings.window_position {
            viewport = viewport.with_position(egui::pos2(pos[0], pos[1]));
            is_centered = false;
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        centered: is_centered,
        ..Default::default()
    };

    eframe::run_native(
        "星酿启动器 - AstraBrew Launcher",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(MyApp::new(settings)))
        }),
    )
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "MiSans".to_owned(),
        FontData::from_static(include_bytes!("../assets/fonts/MiSans-Regular.ttf")).into(),
    );

    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "MiSans".to_owned());

    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "MiSans".to_owned());

    ctx.set_fonts(fonts);
}

#[derive(PartialEq)]
enum Page {
    OneClickStart,
    TavernConfig,
    VersionManage,
    ExtensionManage,
    ResourceManage,
    Console,
    Settings,
}

mod core;
#[path = "lang/lang.rs"]
mod lang;
mod pages;
mod ui;
mod utils;

use pages::console::ConsoleState;
use pages::settings::{SettingsState, SettingsTab, Theme};
use pages::tavern_config::TavernConfigUI;

struct MyApp {
    current_page: Page,
    last_monitor_size: Option<egui::Vec2>,
    settings_tab: SettingsTab,
    settings_state: SettingsState,
    last_save_time: Option<std::time::Instant>,

    // 版本管理状态
    version_manage_state: pages::version_manage::VersionManageState,
    // 酒馆配置 UI 状态
    tavern_config_ui: TavernConfigUI,
    // 控制台状态
    console_state: ConsoleState,
    // brew 任务状态
    homebrew_update_state: pages::settings::BrewTaskState,
    git_install_state: pages::settings::BrewTaskState,
    nodejs_install_state: pages::settings::BrewTaskState,

    // Github 节点状态
    github_node_rx: Option<
        std::sync::mpsc::Receiver<crate::core::settings::github_proxy::NodeLoadMsg>,
    >,
    github_node_state: crate::core::settings::github_proxy::NodeLoadState,
    on_refresh_nodes: bool,
}

impl MyApp {
    fn new(mut settings_state: SettingsState) -> Self {
        // 检测环境依赖版本
        settings_state.detect_all_env();

        Self {
            current_page: Page::OneClickStart,
            last_monitor_size: None,
            settings_tab: SettingsTab::default(),
            settings_state,
            last_save_time: None,
            version_manage_state: pages::version_manage::VersionManageState::new(),
            tavern_config_ui: TavernConfigUI::new(
                crate::core::settings::tavern::ConfigMode::Current,
                None,
            ),
            console_state: ConsoleState::new(),
            homebrew_update_state: pages::settings::BrewTaskState::new(),
            git_install_state: pages::settings::BrewTaskState::new(),
            nodejs_install_state: pages::settings::BrewTaskState::new(),
            github_node_rx: None,
            github_node_state: crate::core::settings::github_proxy::NodeLoadState::Idle,
            on_refresh_nodes: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 应用主题
        let visuals = match self.settings_state.theme {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        };
        ctx.set_visuals(visuals);

        // 动态适配屏幕比例
        if let Some(monitor_size) = ctx.input(|i| i.viewport().monitor_size) {
            if self.last_monitor_size != Some(monitor_size) {
                let aspect_ratio = monitor_size.x / monitor_size.y;

                if (aspect_ratio - 4.0 / 3.0).abs() < 0.1 {
                    if self.last_monitor_size.is_none() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(800.0, 600.0)));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(800.0, 600.0)));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(1200.0, 800.0)));
                } else {
                    if self.last_monitor_size.is_none() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1280.0, 720.0)));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(800.0, 600.0)));
                    ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(1280.0, 720.0)));
                }

                self.last_monitor_size = Some(monitor_size);
            }
        }

        let panel_width = match self.settings_state.language {
            pages::settings::Language::Chinese => 150.0,
            pages::settings::Language::English => 180.0,
        };

        // 左侧导航栏
        egui::SidePanel::left("left_panel")
            .resizable(false)
            .exact_width(panel_width)
            .show(ctx, |ui| {
                ui.add_space(10.0);

                ui.vertical_centered(|ui| {
                    ui.heading(lang::t("app_title", &self.settings_state.language));
                    ui.heading(egui::RichText::new(lang::t("app_subtitle", &self.settings_state.language)).size(12.0));
                });

                // 当前版本信息
                let lang = &self.settings_state.language;
                if let Some(ref inst) = self.settings_state.sillytavern {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(lang::t("sidebar_current_version", lang))
                                .size(10.0)
                                .color(egui::Color32::GRAY),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} {}", lang::t("sidebar_version_label", lang), &inst.version))
                                .size(13.0)
                                .strong(),
                        );
                        let is_online = inst.instance_type == "builtin";
                        let inst_type = if is_online {
                            lang::t("sidebar_instance_online", lang)
                        } else {
                            lang::t("sidebar_instance_local", lang)
                        };
                        ui.label(
                            egui::RichText::new(inst_type)
                                .size(10.0)
                                .color(if is_online { egui::Color32::from_rgb(100, 180, 255) } else { egui::Color32::from_rgb(100, 255, 150) }),
                        );
                    });
                    ui.add_space(14.0);
                }

                // 导航按钮
                let nav_button = |ui: &mut egui::Ui,
                                  current: &mut Page,
                                  target: Page,
                                  icon: &str,
                                  text: &str| {
                    let is_selected = *current == target;
                    let response = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::selectable(is_selected, ""),
                    );

                    let rect = response.rect;
                    let text_color = ui.style().interact_selectable(&response, is_selected).text_color();

                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()));
                    child_ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.add_sized(
                            [20.0, rect.height()],
                            |ui: &mut egui::Ui| {
                                ui.centered_and_justified(|ui| {
                                    ui.add(egui::Label::new(egui::RichText::new(icon).size(16.0).color(text_color)).selectable(false));
                                }).response
                            }
                        );
                        ui.add_space(4.0);
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(text).size(16.0).color(text_color)).selectable(false));
                        });
                    });

                    if response.clicked() {
                        *current = target;
                    }
                    response
                };

                nav_button(ui, &mut self.current_page, Page::OneClickStart, egui_phosphor::regular::ROCKET, lang::t("one_click_start", &self.settings_state.language));
                nav_button(ui, &mut self.current_page, Page::TavernConfig, egui_phosphor::regular::SLIDERS, lang::t("tavern_config", &self.settings_state.language));
                nav_button(ui, &mut self.current_page, Page::VersionManage, egui_phosphor::regular::GIT_BRANCH, lang::t("version_manage", &self.settings_state.language));
                nav_button(ui, &mut self.current_page, Page::ExtensionManage, egui_phosphor::regular::PUZZLE_PIECE, lang::t("extension_manage", &self.settings_state.language));
                nav_button(ui, &mut self.current_page, Page::ResourceManage, egui_phosphor::regular::FOLDER, lang::t("resource_manage", &self.settings_state.language));

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(10.0);
                    let button_height = 32.0;

                    // 设置按钮
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), button_height),
                        egui::Sense::hover(),
                    );
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
                    nav_button(&mut child_ui, &mut self.current_page, Page::Settings, egui_phosphor::regular::GEAR, lang::t("software_settings", &self.settings_state.language));

                    // 控制台按钮
                    ui.add_space(2.0);
                    let (rect2, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), button_height),
                        egui::Sense::hover(),
                    );
                    let mut child_ui2 = ui.new_child(egui::UiBuilder::new().max_rect(rect2).layout(egui::Layout::top_down(egui::Align::Min)));
                    nav_button(&mut child_ui2, &mut self.current_page, Page::Console, egui_phosphor::regular::TERMINAL_WINDOW, lang::t("console", &self.settings_state.language));
                });
            });

        // 右侧页面区域
        let old_state = self.settings_state.clone();

        // 轮询 brew 任务日志
        if let Some(new_ver) = self.homebrew_update_state.poll() {
            self.settings_state.homebrew_version = Some(new_ver);
        }
        if let Some(new_ver) = self.git_install_state.poll() {
            self.settings_state.git_version = Some(new_ver);
        }
        if let Some(new_ver) = self.nodejs_install_state.poll() {
            self.settings_state.nodejs_version = new_ver;
        }
        if self.homebrew_update_state.running
            || self.git_install_state.running
            || self.nodejs_install_state.running
            || self.homebrew_update_state.done_at.is_some()
            || self.git_install_state.done_at.is_some()
            || self.nodejs_install_state.done_at.is_some()
        {
            ctx.request_repaint();
        }

        // 轮询 Github 节点加载消息
        {
            let mut clear_rx = false;
            if let Some(ref rx) = self.github_node_rx {
                while let Ok(msg) = rx.try_recv() {
                    use crate::core::settings::github_proxy::NodeLoadMsg;
                    match msg {
                        NodeLoadMsg::Nodes(entries) => {
                            // 自动选择 ghfast.top（如果列表里有且当前未选中）
                            if self.settings_state.github_proxy_url.is_empty()
                                || !entries
                                    .iter()
                                    .any(|e| e.url == self.settings_state.github_proxy_url)
                            {
                                if let Some(ghfast) =
                                    entries.iter().find(|e| e.url.contains("ghfast.top"))
                                {
                                    self.settings_state.github_proxy_url = ghfast.url.clone();
                                }
                            }
                            self.github_node_state =
                                crate::core::settings::github_proxy::NodeLoadState::Done(entries);
                        }
                        NodeLoadMsg::LatencyUpdate => {
                            ctx.request_repaint();
                        }
                        NodeLoadMsg::Done => {
                            clear_rx = true;
                        }
                        NodeLoadMsg::DoneWithWarning(warning) => {
                            // 数据已在 NodeLoadMsg::Nodes 中设置，这里只附加警告
                            if let crate::core::settings::github_proxy::NodeLoadState::Done(
                                ref entries,
                            ) = self.github_node_state
                            {
                                self.github_node_state =
                                    crate::core::settings::github_proxy::NodeLoadState::DoneWithWarning(
                                        entries.clone(),
                                        warning,
                                    );
                            }
                            clear_rx = true;
                        }
                        NodeLoadMsg::Error(e) => {
                            self.github_node_state =
                                crate::core::settings::github_proxy::NodeLoadState::Error(e);
                            clear_rx = true;
                        }
                    }
                }
            }
            if clear_rx {
                self.github_node_rx = None;
            }
        }

        // 处理刷新节点请求
        if self.on_refresh_nodes {
            self.on_refresh_nodes = false;
            let (tx, rx) = std::sync::mpsc::channel();
            self.github_node_rx = Some(rx);
            self.github_node_state =
                crate::core::settings::github_proxy::NodeLoadState::Loading;
            crate::core::settings::github_proxy::start_fetch_and_test(tx, false);
        }

        // 节点加载中或测试进行中时持续重绘
        if matches!(
            self.github_node_state,
            crate::core::settings::github_proxy::NodeLoadState::Loading
        ) || crate::core::network::is_github_multi_test_in_progress()
        {
            ctx.request_repaint();
        }

        // 每帧同步酒馆配置页的数据模式 & 实例
        {
            use crate::core::settings::tavern::{ConfigMode, InstanceInfo};
            self.tavern_config_ui.config_mode = match self.settings_state.data_mode {
                crate::pages::settings::TavernDataMode::Current => ConfigMode::Current,
                crate::pages::settings::TavernDataMode::Global => ConfigMode::Global,
            };
            self.tavern_config_ui.instance = self.settings_state.sillytavern.as_ref().map(|i| InstanceInfo {
                instance_type: i.instance_type.clone(),
                path: i.path.clone(),
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.current_page {
                Page::OneClickStart => {
                    let version = self
                        .settings_state
                        .sillytavern
                        .as_ref()
                        .map(|inst| inst.version.as_str());
                    let start_mode_label = match self.settings_state.start_mode {
                        pages::settings::StartMode::Normal => lang::t("normal_mode", &self.settings_state.language),
                        pages::settings::StartMode::Desktop => lang::t("desktop_mode", &self.settings_state.language),
                    };
                    pages::home::render(
                        ui,
                        &mut self.current_page,
                        &mut self.console_state,
                        &self.settings_state.language,
                        version,
                        start_mode_label,
                    );
                }
                Page::TavernConfig => {
                    let current_key = self.tavern_config_ui.config_key();
                    if current_key != self.tavern_config_ui.last_config_key {
                        self.tavern_config_ui.refresh();
                    }
                    pages::tavern_config::render(ui, &mut self.tavern_config_ui, &self.settings_state.language);
                }
                Page::VersionManage => {
                    ui.heading(lang::t("version_manage", &self.settings_state.language));
                    ui.separator();
                    pages::version_manage::render(ui, &mut self.version_manage_state, &mut self.settings_state);
                }
                Page::ExtensionManage => {
                    ui.heading(lang::t("extension_manage", &self.settings_state.language));
                    ui.separator();
                    ui.label("这里是扩展管理页面的内容...");
                }
                Page::ResourceManage => {
                    ui.heading(lang::t("resource_manage", &self.settings_state.language));
                    ui.separator();
                    ui.label("这里是资源管理页面的内容...");
                }
                Page::Console => {
                    pages::console::render(ui, &mut self.console_state, &self.settings_state.language);
                }
                Page::Settings => {
                    ui.heading(lang::t("software_settings", &self.settings_state.language));
                    ui.separator();

                    // 代理开关已开启 + 节点列表未加载 → 自动加载缓存数据
                    if self.settings_state.github_proxy_enabled
                        && matches!(
                            self.github_node_state,
                            crate::core::settings::github_proxy::NodeLoadState::Idle
                        )
                    {
                        self.on_refresh_nodes = true;
                    }

                    pages::settings::render(
                        ui,
                        &mut self.settings_tab,
                        &mut self.settings_state,
                        &mut self.homebrew_update_state,
                        &mut self.git_install_state,
                        &mut self.nodejs_install_state,
                        &self.github_node_state,
                        &mut self.on_refresh_nodes,
                    );
                }
            }
        });

        // 设置变化时保存
        if old_state != self.settings_state {
            self.settings_state.save();
            self.last_save_time = Some(std::time::Instant::now());
        }

        // Toast 提示（设置保存）
        let mut show_toast = None;
        if let Some(save_time) = self.last_save_time {
            let elapsed = save_time.elapsed().as_secs_f32();
            if elapsed < 3.0 {
                show_toast = Some((lang::t("settings_saved", &self.settings_state.language).to_string(), elapsed));
            } else {
                self.last_save_time = None;
            }
        }

        if let Some((toast_text, elapsed)) = show_toast {
            let alpha = if elapsed > 2.0 {
                1.0 - (elapsed - 2.0)
            } else {
                1.0
            };

            let visuals = ctx.style().visuals.clone();
            let text_color = visuals.text_color().linear_multiply(alpha);
            let bg_color = visuals.window_fill().linear_multiply(alpha);
            let stroke_color = visuals.window_stroke().color.linear_multiply(alpha);

            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("toast")));

            let font_id = egui::FontId::proportional(16.0);
            let text_galley = painter.layout_no_wrap(toast_text, font_id, text_color);

            let screen_rect = ctx.content_rect();
            let center_x = screen_rect.center().x;
            let bottom_y = screen_rect.max.y - 50.0;

            let padding = egui::vec2(16.0, 10.0);
            let rect = egui::Rect::from_center_size(
                egui::pos2(center_x, bottom_y),
                text_galley.size() + padding * 2.0,
            );

            painter.rect(rect, 8.0, bg_color, egui::Stroke::new(1.0, stroke_color), egui::StrokeKind::Middle);
            let text_pos = egui::pos2(
                rect.center().x - text_galley.size().x / 2.0,
                rect.center().y - text_galley.size().y / 2.0,
            );
            painter.galley(text_pos, text_galley, text_color);

            ctx.request_repaint();
        }

        // 关闭时保存窗口位置
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.settings_state.remember_window_pos {
                if let Some(pos) = ctx.input(|i| i.viewport().inner_rect).map(|r| r.min) {
                    let pos_array = [pos.x, pos.y];
                    if self.settings_state.window_position != Some(pos_array) {
                        self.settings_state.window_position = Some(pos_array);
                        self.settings_state.save();
                    }
                }
            }
        }
    }
}
