use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::utils;

#[derive(PartialEq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    About,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum Language {
    #[default]
    Chinese,
    English,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    #[default]
    Dark,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum CpuCores {
    #[default]
    Auto,
    Half,
    All,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum StartMode {
    #[default]
    Normal,
    Desktop,
    Lan,
    Public,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum TavernDataMode {
    #[default]
    Current,
    Global,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum EnvSource {
    System,
    #[default]
    Builtin,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum NpmRegistry {
    Official,
    #[default]
    Taobao,
    Tencent,
}

#[derive(PartialEq, Default, Clone, Serialize, Deserialize)]
pub enum ProxyType {
    #[default]
    None,
    System,
    Custom,
}

/// 当前激活的 SillyTavern 实例
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CurrentInstance {
    #[serde(rename = "type")]
    pub instance_type: String,
    pub path: Option<String>,
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SettingsState {
    // 界面设置
    pub language: Language,
    pub theme: Theme,
    pub remember_window_pos: bool,
    pub window_position: Option<[f32; 2]>,

    // 基本设置
    pub cpu_cores: CpuCores,
    pub start_mode: StartMode,
    pub data_mode: TavernDataMode,
    pub auto_start: bool,
    pub auto_minimize: bool,
    pub auto_start_tavern: bool,
    pub allow_tavern_background: bool,

    // Homebrew 设置
    pub homebrew_env: EnvSource,

    // Git 设置
    pub git_env: EnvSource,

    // NodeJs 设置
    pub nodejs_env: EnvSource,
    pub npm_registry: NpmRegistry,

    // Github 设置
    pub github_proxy_enabled: bool,
    pub github_proxy_url: String,

    // 网络设置
    pub proxy_type: ProxyType,
    pub custom_proxy: String,

    // 当前版本实例
    #[serde(default)]
    pub sillytavern: Option<CurrentInstance>,

    // Node.js 运行时版本（不持久化）
    #[serde(skip)]
    pub nodejs_version: String,

    // 环境依赖检测结果（不持久化）
    #[serde(skip)]
    pub homebrew_version: Option<String>,
    #[serde(skip)]
    pub git_version: Option<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            language: Language::default(),
            theme: Theme::default(),
            remember_window_pos: true,
            window_position: None,
            cpu_cores: CpuCores::default(),
            start_mode: StartMode::default(),
            data_mode: TavernDataMode::default(),
            auto_start: false,
            auto_minimize: false,
            auto_start_tavern: false,
            allow_tavern_background: false,
            homebrew_env: EnvSource::default(),
            git_env: EnvSource::default(),
            nodejs_env: EnvSource::default(),
            npm_registry: NpmRegistry::default(),
            github_proxy_enabled: false,
            github_proxy_url: String::new(),
            proxy_type: ProxyType::default(),
            custom_proxy: String::new(),
            sillytavern: None,
            nodejs_version: String::new(),
            homebrew_version: None,
            git_version: None,
        }
    }
}

impl SettingsState {
    pub fn load() -> Self {
        let path = utils::app_paths().settings_file();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str(&content) {
                    return state;
                }
            }
        }
        let default_state = Self::default();
        default_state.save();
        default_state
    }

    pub fn save(&self) {
        let path = utils::app_paths().settings_file();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    /// 检测所有环境依赖版本
    pub fn detect_all_env(&mut self) {
        use crate::core::settings::env_detect;
        self.homebrew_version = env_detect::detect_homebrew();
        self.git_version = env_detect::detect_git();
        let node_ver = env_detect::detect_nodejs();
        if let Some(v) = node_ver {
            self.nodejs_version = v;
        }
    }
}

/// brew 任务弹窗状态（更新/安装通用）
pub struct BrewTaskState {
    pub show: bool,
    pub log: String,
    pub running: bool,
    pub receiver: Option<std::sync::mpsc::Receiver<String>>,
    /// 任务完成的时间点（用于 3 秒后自动关闭）
    pub done_at: Option<std::time::Instant>,
}

impl BrewTaskState {
    pub fn new() -> Self {
        Self {
            show: false,
            log: String::new(),
            running: false,
            receiver: None,
            done_at: None,
        }
    }

    /// 启动 brew update
    pub fn start_update(&mut self) {
        use crate::core::settings::env_detect;
        let (tx, rx) = std::sync::mpsc::channel();
        self.receiver = Some(rx);
        self.log = String::new();
        self.running = true;
        self.show = true;
        std::thread::spawn(move || {
            env_detect::run_brew_update(tx);
        });
    }

    /// 启动 brew install <package>
    pub fn start_install(&mut self, package: &str) {
        use crate::core::settings::env_detect;
        let package = package.to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        self.receiver = Some(rx);
        self.log = String::new();
        self.running = true;
        self.show = true;
        std::thread::spawn(move || {
            env_detect::run_brew_install(&package, tx);
        });
    }

    /// 轮询日志，返回完成后的新版本号
    pub fn poll(&mut self) -> Option<String> {
        let mut new_version = None;
        if let Some(ref rx) = self.receiver {
            while let Ok(line) = rx.try_recv() {
                if line == "__DONE__" {
                    self.running = false;
                    self.log.push_str("\n✅ 安装完成，3秒后自动关闭");
                    self.done_at = Some(std::time::Instant::now());
                    self.receiver = None;
                    break;
                }
                if let Some(ver) = line.strip_prefix("__VERSION__:") {
                    new_version = Some(ver.to_string());
                    continue;
                }
                if !self.log.is_empty() {
                    self.log.push('\n');
                }
                self.log.push_str(&line);
            }
        }
        new_version
    }
}

use crate::lang;

fn render_brew_task_window(
    ctx: &egui::Context,
    task: &mut BrewTaskState,
    title: &str,
    desc: &str,
    waiting: &str,
    running_label: &str,
    close_label: &str,
) {
    // 完成后 3 秒自动关闭
    if let Some(done_at) = task.done_at {
        if done_at.elapsed().as_secs() >= 3 {
            task.show = false;
            task.done_at = None;
            return;
        }
        ctx.request_repaint();
    }

    if !task.show {
        return;
    }
    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .min_width(500.0)
        .show(ctx, |ui| {
            ui.label(desc);
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let log = if task.log.is_empty() {
                        waiting.to_string()
                    } else {
                        task.log.clone()
                    };
                    ui.label(log);
                });
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if task.running {
                    ui.spinner();
                    ui.label(running_label);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !task.running {
                        if ui.button(close_label).clicked() {
                            task.show = false;
                        }
                    }
                });
            });
        });
}

fn setting_section(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    add_content: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).size(18.0).color(ui.visuals().text_color()));
        ui.heading(egui::RichText::new(title).strong());
    });
    ui.add_space(5.0);

    egui::Frame::NONE
        .fill(ui.visuals().faint_bg_color)
        .corner_radius(8.0)
        .inner_margin(15.0)
        .show(ui, |ui| {
            add_content(ui);
        });
}

fn setting_row(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    description: &str,
    add_content: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [30.0, 30.0],
            egui::Label::new(egui::RichText::new(icon).size(20.0)),
        );

        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(egui::RichText::new(title).size(14.0).strong());
            if !description.is_empty() {
                ui.label(
                    egui::RichText::new(description)
                        .color(egui::Color32::GRAY)
                        .size(12.0),
                );
            }
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            add_content(ui);
        });
    });
}

pub fn render(
    ui: &mut egui::Ui,
    tab: &mut SettingsTab,
    state: &mut SettingsState,
    homebrew_update: &mut BrewTaskState,
    git_install: &mut BrewTaskState,
    nodejs_install: &mut BrewTaskState,
) {
    ui.horizontal(|ui| {
        ui.selectable_value(tab, SettingsTab::General, lang::t("general_settings", &state.language));
        ui.selectable_value(tab, SettingsTab::About, lang::t("about_software", &state.language));
    });
    ui.separator();

    match tab {
        SettingsTab::General => {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(10.0);

                    // 界面设置
                    setting_section(
                        ui,
                        egui_phosphor::regular::PAINT_BRUSH,
                        lang::t("interface_settings", &state.language),
                        |ui| {
                            setting_row(
                                ui,
                                egui_phosphor::regular::TRANSLATE,
                                lang::t("language", &state.language),
                                lang::t("language_desc", &state.language),
                                |ui| {
                                    egui::ComboBox::from_id_salt("lang_combo")
                                        .selected_text(match state.language {
                                            Language::Chinese => lang::t("zh_cn", &state.language),
                                            Language::English => lang::t("en_us", &state.language),
                                        })
                                        .show_ui(ui, |ui| {
                                            let text_zh = lang::t("zh_cn", &state.language);
                                            let text_en = lang::t("en_us", &state.language);
                                            ui.selectable_value(&mut state.language, Language::Chinese, text_zh);
                                            ui.selectable_value(&mut state.language, Language::English, text_en);
                                        });
                                },
                            );
                            ui.add_space(10.0);
                            setting_row(
                                ui,
                                egui_phosphor::regular::PALETTE,
                                lang::t("theme", &state.language),
                                lang::t("theme_desc", &state.language),
                                |ui| {
                                    egui::ComboBox::from_id_salt("theme_combo")
                                        .selected_text(match state.theme {
                                            Theme::Light => lang::t("light_theme", &state.language),
                                            Theme::Dark => lang::t("dark_theme", &state.language),
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut state.theme, Theme::Light, lang::t("light_theme", &state.language));
                                            ui.selectable_value(&mut state.theme, Theme::Dark, lang::t("dark_theme", &state.language));
                                        });
                                },
                            );
                            ui.add_space(10.0);
                            setting_row(
                                ui,
                                egui_phosphor::regular::CORNERS_OUT,
                                lang::t("remember_window_pos", &state.language),
                                lang::t("remember_window_pos_desc", &state.language),
                                |ui| {
                                    ui.add(crate::ui::switch::toggle(&mut state.remember_window_pos));
                                },
                            );
                        },
                    );

                    // 基本设置
                    setting_section(ui, egui_phosphor::regular::SLIDERS, lang::t("basic_settings", &state.language), |ui| {
                        setting_row(
                            ui,
                            egui_phosphor::regular::POWER,
                            lang::t("auto_start", &state.language),
                            lang::t("auto_start_desc", &state.language),
                            |ui| {
                                ui.add(crate::ui::switch::toggle(&mut state.auto_start));
                            },
                        );
                        ui.add_space(10.0);
                        setting_row(
                            ui,
                            egui_phosphor::regular::ARROW_DOWN,
                            lang::t("auto_minimize", &state.language),
                            lang::t("auto_minimize_desc", &state.language),
                            |ui| {
                                ui.add(crate::ui::switch::toggle(&mut state.auto_minimize));
                            },
                        );
                        ui.add_space(10.0);
                        setting_row(
                            ui,
                            egui_phosphor::regular::ROCKET,
                            lang::t("auto_start_tavern", &state.language),
                            lang::t("auto_start_tavern_desc", &state.language),
                            |ui| {
                                ui.add(crate::ui::switch::toggle(&mut state.auto_start_tavern));
                            },
                        );
                        ui.add_space(10.0);

                        setting_row(
                            ui,
                            egui_phosphor::regular::CPU,
                            lang::t("cpu_cores", &state.language),
                            lang::t("cpu_cores_desc", &state.language),
                            |ui| {
                                egui::ComboBox::from_id_salt("cpu_combo")
                                    .selected_text(match state.cpu_cores {
                                        CpuCores::Auto => lang::t("auto", &state.language),
                                        CpuCores::Half => lang::t("half_cores", &state.language),
                                        CpuCores::All => lang::t("all_cores", &state.language),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut state.cpu_cores, CpuCores::Auto, lang::t("auto", &state.language));
                                        ui.selectable_value(&mut state.cpu_cores, CpuCores::Half, lang::t("half_cores", &state.language));
                                        ui.selectable_value(&mut state.cpu_cores, CpuCores::All, lang::t("all_cores", &state.language));
                                    });
                            },
                        );
                        ui.add_space(10.0);

                        setting_row(
                            ui,
                            egui_phosphor::regular::ARROW_ARC_LEFT,
                            lang::t("allow_tavern_background", &state.language),
                            lang::t("allow_tavern_background_desc", &state.language),
                            |ui| {
                                ui.add(crate::ui::switch::toggle(&mut state.allow_tavern_background));
                            },
                        );
                        ui.add_space(10.0);

                        setting_row(
                            ui,
                            egui_phosphor::regular::PLAY_CIRCLE,
                            lang::t("start_mode", &state.language),
                            lang::t("start_mode_desc", &state.language),
                            |ui| {
                                crate::ui::segmented::segmented_control(
                                    ui,
                                    &mut state.start_mode,
                                    &[
                                        (StartMode::Normal, lang::t("normal_mode", &state.language)),
                                        (StartMode::Desktop, lang::t("desktop_mode", &state.language)),
                                        (StartMode::Lan, lang::t("lan_mode", &state.language)),
                                        (StartMode::Public, lang::t("public_mode", &state.language)),
                                    ],
                                );
                            },
                        );
                        ui.add_space(10.0);

                        let data_mode_desc = match state.data_mode {
                            TavernDataMode::Global => lang::t("data_mode_global_desc", &state.language),
                            TavernDataMode::Current => lang::t("data_mode_current_desc", &state.language),
                        };
                        setting_row(
                            ui,
                            egui_phosphor::regular::DATABASE,
                            lang::t("data_mode", &state.language),
                            data_mode_desc,
                            |ui| {
                                crate::ui::segmented::segmented_control(
                                    ui,
                                    &mut state.data_mode,
                                    &[
                                        (TavernDataMode::Global, lang::t("data_mode_global", &state.language)),
                                        (TavernDataMode::Current, lang::t("data_mode_current", &state.language)),
                                    ],
                                );
                            },
                        );
                    });

                    // 环境依赖
                    {
                        let brew_installed = state.homebrew_version.is_some();

                        setting_section(ui, egui_phosphor::regular::PACKAGE, lang::t("env_dependencies", &state.language), |ui| {
                        // Homebrew
                        {
                            let hv = state.homebrew_version.clone();
                            let hv_outdated = hv.as_ref().map_or(false, |v| {
                                crate::core::settings::env_detect::is_homebrew_outdated(v)
                            });
                            setting_row(
                                ui,
                                egui_phosphor::regular::BEER_BOTTLE,
                                lang::t("homebrew_env_source", &state.language),
                                lang::t("homebrew_purpose", &state.language),
                                |ui| {
                                    match hv {
                                        Some(ref ver) if hv_outdated => {
                                            if ui.button(lang::t("update_btn", &state.language)).clicked() {
                                                homebrew_update.start_update();
                                            }
                                        }
                                        Some(ref ver) => {
                                            ui.label(egui::RichText::new(ver.as_str()).size(14.0));
                                        }
                                        None => {
                                            if ui.button(lang::t("install", &state.language)).clicked() {
                                                // TODO: 触发 Homebrew 安装逻辑
                                            }
                                        }
                                    }
                                },
                            );
                        }
                        ui.add_space(10.0);
                        // Git
                        {
                            let gv = state.git_version.clone();
                            setting_row(
                                ui,
                                egui_phosphor::regular::GIT_BRANCH,
                                lang::t("git_env_source", &state.language),
                                lang::t("git_purpose", &state.language),
                                |ui| {
                                    match gv {
                                        Some(ref ver) => {
                                            ui.label(egui::RichText::new(ver.as_str()).size(14.0));
                                        }
                                        None => {
                                            let btn = egui::Button::new(lang::t("install", &state.language));
                                            let resp = if brew_installed {
                                                ui.add_enabled(true, btn)
                                            } else {
                                                ui.add_enabled(false, btn)
                                            };
                                            if resp.clicked() {
                                                git_install.start_install("git");
                                            }
                                        }
                                    }
                                },
                            );
                        }
                        ui.add_space(10.0);
                        // NodeJs
                        {
                            let nv = if state.nodejs_version.is_empty() { None } else { Some(state.nodejs_version.clone()) };
                            let nv_outdated = nv.as_ref().map_or(false, |v| {
                                crate::core::settings::env_detect::is_nodejs_outdated(v)
                            });
                            let title = if nv_outdated {
                                format!("{}  ⚠ {}", lang::t("nodejs_env_source", &state.language), lang::t("version_too_low", &state.language))
                            } else {
                                lang::t("nodejs_env_source", &state.language).to_string()
                            };
                            setting_row(
                                ui,
                                egui_phosphor::regular::CODE,
                                &title,
                                lang::t("nodejs_purpose", &state.language),
                                |ui| {
                                    match nv {
                                        Some(ref ver) if nv_outdated => {
                                            if ui.button(lang::t("update_btn", &state.language)).clicked() {
                                                // TODO: 触发 NodeJs 更新逻辑
                                            }
                                        }
                                        Some(ref ver) => {
                                            ui.label(egui::RichText::new(ver.as_str()).size(14.0));
                                        }
                                        None => {
                                            let btn = egui::Button::new(lang::t("install", &state.language));
                                            let resp = if brew_installed {
                                                ui.add_enabled(true, btn)
                                            } else {
                                                ui.add_enabled(false, btn)
                                            };
                                            if resp.clicked() {
                                                nodejs_install.start_install("node@24");
                                            }
                                        }
                                    }
                                },
                            );
                        }
                        ui.add_space(10.0);
                        // NPM 源设置
                        setting_row(
                            ui,
                            egui_phosphor::regular::GLOBE,
                            lang::t("npm_registry", &state.language),
                            lang::t("npm_registry_desc", &state.language),
                            |ui| {
                                egui::ComboBox::from_id_salt("npm_registry_combo")
                                    .selected_text(match state.npm_registry {
                                        NpmRegistry::Official => lang::t("official_registry", &state.language),
                                        NpmRegistry::Taobao => lang::t("taobao_registry", &state.language),
                                        NpmRegistry::Tencent => lang::t("tencent_registry", &state.language),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut state.npm_registry, NpmRegistry::Official, lang::t("official_registry", &state.language));
                                        ui.selectable_value(&mut state.npm_registry, NpmRegistry::Taobao, lang::t("taobao_registry", &state.language));
                                        ui.selectable_value(&mut state.npm_registry, NpmRegistry::Tencent, lang::t("tencent_registry", &state.language));
                                    });
                            },
                        );
                    });

                    // Homebrew 更新弹窗
                    render_brew_task_window(
                        ui.ctx(),
                        homebrew_update,
                        lang::t("homebrew_update_title", &state.language),
                        lang::t("homebrew_update_desc", &state.language),
                        lang::t("homebrew_update_waiting", &state.language),
                        lang::t("homebrew_update_running", &state.language),
                        lang::t("close", &state.language),
                    );

                    // Git 安装弹窗
                    render_brew_task_window(
                        ui.ctx(),
                        git_install,
                        lang::t("git_install_title", &state.language),
                        lang::t("git_install_desc", &state.language),
                        lang::t("brew_install_waiting", &state.language),
                        lang::t("brew_install_running", &state.language),
                        lang::t("close", &state.language),
                    );

                    // NodeJs 安装弹窗
                    render_brew_task_window(
                        ui.ctx(),
                        nodejs_install,
                        lang::t("nodejs_install_title", &state.language),
                        lang::t("nodejs_install_desc", &state.language),
                        lang::t("brew_install_waiting", &state.language),
                        lang::t("brew_install_running", &state.language),
                        lang::t("close", &state.language),
                    );
                    }

                    // Github 设置
                    setting_section(ui, egui_phosphor::regular::GITHUB_LOGO, lang::t("github_settings", &state.language), |ui| {
                        setting_row(
                            ui,
                            egui_phosphor::regular::POWER,
                            lang::t("github_proxy", &state.language),
                            lang::t("github_proxy_desc", &state.language),
                            |ui| {
                                let mut enabled = state.github_proxy_enabled;
                                if ui.add(crate::ui::switch::toggle(&mut enabled)).changed() {
                                    state.github_proxy_enabled = enabled;
                                    if enabled {
                                        state.proxy_type = ProxyType::None;
                                    }
                                }
                            },
                        );
                        if state.github_proxy_enabled {
                            ui.add_space(10.0);
                            setting_row(
                                ui,
                                egui_phosphor::regular::LINK,
                                lang::t("github_proxy_url", &state.language),
                                "",
                                |ui| {
                                    ui.add_sized(
                                        [250.0, 24.0],
                                        egui::TextEdit::singleline(&mut state.github_proxy_url),
                                    );
                                },
                            );
                        }
                    });

                    // 网络设置
                    setting_section(ui, egui_phosphor::regular::WIFI_HIGH, lang::t("network_settings", &state.language), |ui| {
                        setting_row(
                            ui,
                            egui_phosphor::regular::SHIELD,
                            lang::t("proxy_settings", &state.language),
                            lang::t("proxy_settings_desc", &state.language),
                            |ui| {
                                let mut pt = state.proxy_type.clone();
                                egui::ComboBox::from_id_salt("proxy_type_combo")
                                    .selected_text(match pt {
                                        ProxyType::None => lang::t("off", &state.language),
                                        ProxyType::System => lang::t("follow_system", &state.language),
                                        ProxyType::Custom => lang::t("custom_proxy", &state.language),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut pt, ProxyType::None, lang::t("off", &state.language));
                                        ui.selectable_value(&mut pt, ProxyType::System, lang::t("follow_system", &state.language));
                                        ui.selectable_value(&mut pt, ProxyType::Custom, lang::t("custom_proxy", &state.language));
                                    });

                                if pt != state.proxy_type {
                                    state.proxy_type = pt;
                                    if state.proxy_type != ProxyType::None {
                                        state.github_proxy_enabled = false;
                                    }
                                }
                            },
                        );

                        if state.proxy_type == ProxyType::Custom {
                            ui.add_space(10.0);
                            setting_row(
                                ui,
                                egui_phosphor::regular::LINK,
                                lang::t("proxy_address", &state.language),
                                lang::t("proxy_address_desc", &state.language),
                                |ui| {
                                    ui.text_edit_singleline(&mut state.custom_proxy);
                                },
                            );
                        }
                    });

                    ui.add_space(20.0);
                });
        }
        SettingsTab::About => {
            ui.vertical_centered(|ui| {
                ui.heading(lang::t("about_title", &state.language));
                ui.label(lang::t("about_version", &state.language));
                ui.label(lang::t("about_desc", &state.language));

                ui.add_space(20.0);
                ui.separator();
                ui.add_space(10.0);

                ui.heading(lang::t("tech_stack", &state.language));
                ui.add_space(10.0);

                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("tech_stack_grid")
                                .striped(true)
                                .num_columns(4)
                                .min_col_width(100.0)
                                .spacing(egui::vec2(30.0, 15.0))
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| ui.strong(lang::t("tech_col_1", &state.language)));
                                    ui.vertical_centered(|ui| ui.strong(lang::t("tech_col_2", &state.language)));
                                    ui.vertical_centered(|ui| ui.strong(lang::t("tech_col_3", &state.language)));
                                    ui.vertical_centered(|ui| ui.strong(lang::t("tech_col_4", &state.language)));
                                    ui.end_row();

                                    ui.vertical_centered(|ui| ui.label("MiSans"));
                                    ui.vertical_centered(|ui| ui.label("2022"));
                                    ui.vertical_centered(|ui| {
                                        ui.hyperlink_to(lang::t("free_commercial", &state.language), "https://hyperos.mi.com/font/zh/faq/");
                                    });
                                    ui.vertical_centered(|ui| ui.label(lang::t("mi_font", &state.language)));
                                    ui.end_row();

                                    ui.vertical_centered(|ui| ui.label("Rust"));
                                    ui.vertical_centered(|ui| ui.label("2024"));
                                    ui.vertical_centered(|ui| {
                                        ui.hyperlink_to("MIT / Apache-2.0", "https://github.com/rust-lang/rust/blob/master/LICENSE-MIT");
                                    });
                                    ui.vertical_centered(|ui| ui.label(lang::t("rust_desc", &state.language)));
                                    ui.end_row();

                                    ui.vertical_centered(|ui| ui.label("egui"));
                                    ui.vertical_centered(|ui| ui.label("0.33"));
                                    ui.vertical_centered(|ui| {
                                        ui.hyperlink_to("MIT / Apache-2.0", "https://github.com/emilk/egui/blob/master/LICENSE-MIT");
                                    });
                                    ui.vertical_centered(|ui| ui.label(lang::t("egui_desc", &state.language)));
                                    ui.end_row();

                                    ui.vertical_centered(|ui| ui.label("eframe"));
                                    ui.vertical_centered(|ui| ui.label("0.33"));
                                    ui.vertical_centered(|ui| {
                                        ui.hyperlink_to("MIT / Apache-2.0", "https://github.com/emilk/egui/blob/master/LICENSE-MIT");
                                    });
                                    ui.vertical_centered(|ui| ui.label(lang::t("eframe_desc", &state.language)));
                                    ui.end_row();

                                    ui.vertical_centered(|ui| ui.label("egui_phosphor"));
                                    ui.vertical_centered(|ui| ui.label("0.11"));
                                    ui.vertical_centered(|ui| {
                                        ui.hyperlink_to("MIT / Apache-2.0", "https://github.com/amPerl/egui-phosphor/blob/main/LICENSE-MIT");
                                    });
                                    ui.vertical_centered(|ui| ui.label(lang::t("phosphor_desc", &state.language)));
                                    ui.end_row();
                                });
                        });
                });
            });
        }
    }
}
