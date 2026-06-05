use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;

use crate::lang;
use crate::pages::settings::SettingsState;
use crate::utils;

#[derive(PartialEq, Clone, Copy)]
pub enum VersionTab {
    Local,
    Online,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalInstance {
    pub version: String,
    pub path: String,
    #[serde(default)]
    pub is_current: bool,
    #[serde(default, skip_serializing)]
    pub is_online: bool,
}

pub struct VersionManageState {
    pub active_tab: VersionTab,
    pub local_instances: Vec<LocalInstance>,
}

impl VersionManageState {
    pub fn new() -> Self {
        Self {
            active_tab: VersionTab::Local,
            local_instances: vec![],
        }
    }
}

impl Default for VersionManageState {
    fn default() -> Self {
        Self::new()
    }
}

/// 保存本地实例列表
pub fn save_local_instances(instances: &[LocalInstance]) {
    let path = utils::app_paths().instances_file();
    let locals: Vec<&LocalInstance> = instances.iter().filter(|i| !i.is_online).collect();
    if let Ok(content) = serde_json::to_string_pretty(&locals) {
        let _ = fs::write(path, content);
    }
}

/// 加载本地实例列表
pub fn load_local_instances() -> Vec<LocalInstance> {
    let path = utils::app_paths().instances_file();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(instances) = serde_json::from_str::<Vec<LocalInstance>>(&content) {
                return instances;
            }
        }
    }
    vec![]
}

/// 保存当前版本到 settings.json
pub fn save_current_to_settings(
    instance_type: &str,
    path: Option<&str>,
    version: &str,
    settings: &mut SettingsState,
) {
    settings.sillytavern = Some(crate::pages::settings::CurrentInstance {
        instance_type: instance_type.to_string(),
        path: path.map(|p| p.to_string()),
        version: version.to_string(),
    });
    settings.save();
}

/// 解析版本号字符串为 (major, minor)
fn parse_version_major_minor(version: &str) -> Option<(u32, u32)> {
    let v = version.trim_start_matches('v');
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() >= 2 {
        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        Some((major, minor))
    } else {
        None
    }
}

/// 检查酒馆版本对 Node.js 的最低要求
fn check_nodejs_requirement(st_version: &str, nodejs_version: &str) -> Option<String> {
    let st_mm = parse_version_major_minor(st_version)?;
    let node_mm = parse_version_major_minor(nodejs_version)?;

    if st_mm.0 > 1 || (st_mm.0 == 1 && st_mm.1 > 17) {
        if node_mm.0 < 20 {
            return Some(format!("Min Node.js: >= v20 (current: v{})", nodejs_version.trim_start_matches('v')));
        }
    } else if st_mm.0 > 1 || (st_mm.0 == 1 && st_mm.1 >= 14) {
        if node_mm.0 < 18 {
            return Some(format!("Min Node.js: > v18 (current: v{})", nodejs_version.trim_start_matches('v')));
        }
    }
    None
}

pub fn render(ui: &mut egui::Ui, state: &mut VersionManageState, settings: &mut SettingsState) {
    let lang_owned = settings.language.clone();
    let lang = &lang_owned;

    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.active_tab, VersionTab::Local, lang::t("tab_local_instances", lang));
        ui.selectable_value(&mut state.active_tab, VersionTab::Online, lang::t("tab_online_download", lang));
    });
    ui.separator();

    match state.active_tab {
        VersionTab::Local => {
            render_local_tab(ui, state, settings);
        }
        VersionTab::Online => {
            render_online_tab(ui, lang);
        }
    }
}

fn render_local_tab(ui: &mut egui::Ui, state: &mut VersionManageState, settings: &mut SettingsState) {
    let lang_owned = settings.language.clone();
    let lang = &lang_owned;

    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        if state.local_instances.is_empty() {
            ui.label(lang::t("no_local_instances", lang));
        } else {
            let mut remove_idx = None;
            let mut switch_idx = None;

            for (i, instance) in state.local_instances.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&instance.version).heading());
                            ui.label(egui::RichText::new(&instance.path).small().color(egui::Color32::GRAY));
                            if !settings.nodejs_version.is_empty() {
                                if let Some(warning) = check_nodejs_requirement(&instance.version, &settings.nodejs_version) {
                                    ui.label(
                                        egui::RichText::new(warning)
                                            .size(10.0)
                                            .color(egui::Color32::from_rgb(255, 150, 80)),
                                    );
                                }
                            } else {
                                ui.label(
                                    egui::RichText::new("Node.js: not detected")
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(255, 100, 80)),
                                );
                            }
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_current = match &settings.sillytavern {
                                Some(s) if s.instance_type == "local" => s.path.as_deref() == Some(&instance.path),
                                _ => false,
                            };
                            if is_current {
                                ui.label(egui::RichText::new(lang::t("status_current_version", lang)).color(egui::Color32::GREEN));
                            } else {
                                if ui.button(lang::t("btn_switch_version", lang)).clicked() {
                                    switch_idx = Some(i);
                                }
                            }

                            if ui.add_enabled(!is_current, egui::Button::new(lang::t("btn_remove_list", lang))).clicked() {
                                remove_idx = Some(i);
                            }
                        });
                    });
                });
            }

            if let Some(idx) = remove_idx {
                state.local_instances.remove(idx);
                save_local_instances(&state.local_instances);
            }
            if let Some(idx) = switch_idx {
                for (i, inst) in state.local_instances.iter_mut().enumerate() {
                    inst.is_current = i == idx;
                }
                if let Some(inst) = state.local_instances.get(idx) {
                    save_current_to_settings("local", Some(&inst.path), &inst.version, settings);
                }
            }
        }
    });
}

fn render_online_tab(ui: &mut egui::Ui, lang: &crate::pages::settings::Language) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new(egui_phosphor::regular::BEER_BOTTLE).size(80.0));
        ui.add_space(20.0);
        ui.label(lang::t("status_fetching_releases", lang));
    });
}
