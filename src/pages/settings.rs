use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
            git_env: EnvSource::default(),
            nodejs_env: EnvSource::default(),
            npm_registry: NpmRegistry::default(),
            github_proxy_enabled: false,
            github_proxy_url: String::new(),
            proxy_type: ProxyType::default(),
            custom_proxy: String::new(),
            sillytavern: None,
            nodejs_version: String::new(),
        }
    }
}

impl SettingsState {
    fn config_path() -> PathBuf {
        let mut current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        current_exe.pop();

        let path_str = current_exe.to_string_lossy();
        let mut root = if path_str.contains("target\\debug") || path_str.contains("target\\release") {
            let mut p = current_exe.clone();
            p.pop();
            p.pop();
            p
        } else {
            current_exe
        };

        root.push("data");
        root.push("settings.json");
        root
    }

    pub fn load() -> Self {
        let path = Self::config_path();
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
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }
}

use crate::lang;

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

                    // Git 设置
                    setting_section(ui, egui_phosphor::regular::GIT_BRANCH, lang::t("git_settings", &state.language), |ui| {
                        setting_row(
                            ui,
                            egui_phosphor::regular::WRENCH,
                            lang::t("git_env_source", &state.language),
                            lang::t("git_env_source_desc", &state.language),
                            |ui| {
                                egui::ComboBox::from_id_salt("git_env_combo")
                                    .selected_text(match state.git_env {
                                        EnvSource::System => lang::t("system_env", &state.language),
                                        EnvSource::Builtin => lang::t("builtin_env", &state.language),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut state.git_env, EnvSource::System, lang::t("system_env", &state.language));
                                        ui.selectable_value(&mut state.git_env, EnvSource::Builtin, lang::t("builtin_env", &state.language));
                                    });
                            },
                        );
                    });

                    // NodeJs 设置
                    setting_section(ui, egui_phosphor::regular::TERMINAL, lang::t("nodejs_settings", &state.language), |ui| {
                        setting_row(
                            ui,
                            egui_phosphor::regular::WRENCH,
                            lang::t("nodejs_env_source", &state.language),
                            lang::t("nodejs_env_source_desc", &state.language),
                            |ui| {
                                egui::ComboBox::from_id_salt("nodejs_env_combo")
                                    .selected_text(match state.nodejs_env {
                                        EnvSource::System => lang::t("system_env", &state.language),
                                        EnvSource::Builtin => lang::t("builtin_env", &state.language),
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut state.nodejs_env, EnvSource::System, lang::t("system_env", &state.language));
                                        ui.selectable_value(&mut state.nodejs_env, EnvSource::Builtin, lang::t("builtin_env", &state.language));
                                    });
                            },
                        );
                        ui.add_space(10.0);
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
