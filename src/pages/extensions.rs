use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;

use crate::lang;
use crate::pages::settings::Language;
use crate::utils;
use crate::ui::switch::toggle;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExtensionManifest {
    #[serde(default)]
    pub display_name: String,
    #[serde(rename = "homePage", default)]
    pub home_page: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub auto_update: Option<bool>,
    #[serde(default)]
    pub minimum_client_version: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ExtensionInfo {
    pub id: String,
    pub path: PathBuf,
    pub manifest: ExtensionManifest,
    pub is_official: bool,
    pub is_enabled: bool,     // 仅UI展示
    pub modified_at: u64,     // Unix 时间戳（秒），用于排序
}

#[allow(dead_code)]
pub enum ExtensionMsg {
    Loaded(Vec<ExtensionInfo>),
    Error(String),
}

#[derive(PartialEq)]
pub enum AddDialogTab {
    Git,
    Offline,
}

pub struct ExtensionManageState {
    pub extensions: Vec<ExtensionInfo>,
    pub is_loading: bool,
    pub error_msg: Option<String>,
    rx: Option<Receiver<ExtensionMsg>>,
    pub show_add_dialog: bool,
    pub add_dialog_tab: AddDialogTab,
    pub git_url: String,
    pub offline_path: String,
    pub selected_extensions: HashSet<String>,
    pub current_page: usize,
    pub page_size: usize,
    pub batch_mode: bool,
    pub show_system_extensions: bool,
}

impl ExtensionManageState {
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
            is_loading: false,
            error_msg: None,
            rx: None,
            show_add_dialog: false,
            add_dialog_tab: AddDialogTab::Git,
            git_url: String::new(),
            offline_path: String::new(),
            selected_extensions: HashSet::new(),
            current_page: 0,
            page_size: 10,
            batch_mode: false,
            show_system_extensions: false,
        }
    }

    pub fn load_extensions(&mut self, instance_path: Option<&str>) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = Some(rx);
        self.is_loading = true;
        self.error_msg = None;

        let base_path = match instance_path {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => utils::app_paths().sillytavern_dir(),
        };

        thread::spawn(move || {
            let mut results = Vec::new();
            
            // 官方扩展目录
            let official_dir = base_path.join("public").join("scripts").join("extensions");
            // 第三方扩展目录
            let third_party_dir = official_dir.join("third-party");

            // 读取第三方扩展
            if third_party_dir.exists() && third_party_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&third_party_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(ext) = Self::parse_extension(&path, false) {
                                results.push(ext);
                            }
                        }
                    }
                }
            }

            // 读取官方扩展
            if official_dir.exists() && official_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&official_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() && path.file_name().unwrap_or_default() != "third-party" {
                            if let Some(ext) = Self::parse_extension(&path, true) {
                                results.push(ext);
                            }
                        }
                    }
                }
            }

            // 排序：第三方在前（is_official=false），官方在后（is_official=true）
            // 同一组内按修改时间从新到旧排列
            results.sort_by(|a, b| {
                // 按 is_official 分组（false < true → 第三方在前）
                match a.is_official.cmp(&b.is_official) {
                    std::cmp::Ordering::Equal => b.modified_at.cmp(&a.modified_at), // 时间倒序
                    other => other,
                }
            });

            let _ = tx.send(ExtensionMsg::Loaded(results));
        });
    }

    fn parse_extension(dir: &Path, is_official: bool) -> Option<ExtensionInfo> {
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.exists() {
            return None;
        }

        let content = fs::read_to_string(&manifest_path).ok()?;
        let mut manifest: ExtensionManifest = serde_json::from_str(&content).unwrap_or_else(|_| ExtensionManifest {
            display_name: dir.file_name().unwrap_or_default().to_string_lossy().to_string(),
            home_page: String::new(),
            version: String::new(),
            author: String::new(),
            auto_update: None,
            minimum_client_version: String::new(),
        });

        if manifest.display_name.is_empty() {
            manifest.display_name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        }

        let id = dir.file_name().unwrap_or_default().to_string_lossy().to_string();

        // 获取目录修改时间（用于排序）
        let modified_at = fs::metadata(dir)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Some(ExtensionInfo {
            id,
            path: dir.to_path_buf(),
            manifest,
            is_official,
            is_enabled: true, // 默认开启
            modified_at,
        })
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok(msg) = rx.try_recv() {
                match msg {
                    ExtensionMsg::Loaded(exts) => {
                        self.extensions = exts;
                        self.is_loading = false;
                        self.current_page = 0; // 加载新数据后回到第一页
                    }
                    ExtensionMsg::Error(err) => {
                        self.error_msg = Some(err);
                        self.is_loading = false;
                    }
                }
                self.rx = None;
            }
        }
    }
}

pub fn render(ui: &mut egui::Ui, state: &mut ExtensionManageState, lang: &Language, instance_path: Option<&str>) {
    state.poll();

    // 计算可见扩展索引（受「显示系统扩展」开关控制）
    let visible_indices: Vec<usize> = state
        .extensions
        .iter()
        .enumerate()
        .filter(|(_, e)| state.show_system_extensions || !e.is_official)
        .map(|(i, _)| i)
        .collect();
    let total_visible = visible_indices.len();

    ui.horizontal(|ui| {
        ui.heading(lang::t("extension_manage", lang));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(lang::t("btn_refresh", lang)).clicked() {
                state.load_extensions(instance_path);
            }
            ui.add_space(4.0);
            // 显示系统扩展开关
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(lang::t("ext_show_system", lang)).size(12.0),
                );
                ui.add(toggle(&mut state.show_system_extensions));
            });
        });
    });
    
    ui.separator();

    // ---- 工具栏 ----
    render_toolbar(ui, state, lang, &visible_indices);
    ui.add_space(4.0);

    if state.is_loading {
        ui.centered_and_justified(|ui| {
            ui.spinner();
        });
        return;
    }

    if let Some(err) = &state.error_msg {
        ui.colored_label(egui::Color32::RED, err);
        return;
    }

    if total_visible == 0 {
        let available = ui.available_size();
        let content_height = 80.0;
        let top_offset = (available.y - content_height) / 2.0;

        ui.vertical(|ui| {
            ui.add_space(top_offset.max(40.0));

            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::PACKAGE)
                        .size(48.0)
                        .color(egui::Color32::from_gray(100)),
                );

                ui.add_space(12.0);

                ui.label(
                    egui::RichText::new(lang::t("no_extensions_found", lang))
                        .size(16.0)
                        .color(egui::Color32::GRAY),
                );
            });
        });
        return;
    }

    // 切换可见性时钳位页码
    let total_pages = if total_visible == 0 {
        0
    } else {
        (total_visible + state.page_size - 1) / state.page_size
    };
    if state.current_page >= total_pages {
        state.current_page = total_pages.saturating_sub(1);
    }

    // ---- 扩展卡片网格（2列，分页显示） ----
    // 为底部分页栏预留 32px 高度
    let pagination_height = if total_pages > 1 { 32.0 } else { 0.0 };
    let scroll_height = (ui.available_height() - pagination_height).max(0.0);

    egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(4, 8))
                .show(ui, |ui| {
                    let max_columns = 2;
                    let spacing = 16.0;
                    let available_width = ui.available_width();
                    let item_width = ((available_width - spacing * (max_columns as f32 - 1.0))
                        / max_columns as f32)
                        .floor();

                    egui::Grid::new("extensions_grid")
                        .spacing([spacing, spacing])
                        .min_col_width(item_width)
                        .max_col_width(item_width)
                        .show(ui, |ui| {
                            let start = (state.current_page * state.page_size).min(total_visible);
                            let end = ((state.current_page + 1) * state.page_size).min(total_visible);
                            let page_indices = &visible_indices[start..end];

                            for (col_idx, &idx) in page_indices.iter().enumerate() {
                                if col_idx > 0 && col_idx % max_columns == 0 {
                                    ui.end_row();
                                }

                                render_extension_card(
                                    ui, &mut state.extensions[idx], item_width, lang,
                                    &mut state.selected_extensions,
                                    state.batch_mode,
                                );
                            }
                        });
                });
        });

    // ---- 底部分页栏 ----
    if total_pages > 1 {
        render_pagination_bar(ui, state, total_visible);
    }

    // ---- 添加扩展弹窗 ----
    render_add_dialog(ui, state, lang);
}

// ============ 工具栏 ============

fn render_toolbar(ui: &mut egui::Ui, state: &mut ExtensionManageState, lang: &Language, visible_indices: &[usize]) {
    ui.horizontal(|ui| {
        if state.batch_mode {
            // ---- 批量管理模式 ----
            let has_selection = !state.selected_extensions.is_empty();
            // 仅统计当前可见扩展中全部被选中的情况
            let all_visible_selected = !visible_indices.is_empty()
                && visible_indices
                    .iter()
                    .all(|&i| state.selected_extensions.contains(&state.extensions[i].id));

            let select_label = if all_visible_selected {
                lang::t("ext_batch_deselect_all", lang)
            } else {
                lang::t("ext_batch_select_all", lang)
            };
            if ui.button(select_label).clicked() {
                if all_visible_selected {
                    for &i in visible_indices {
                        state.selected_extensions.remove(&state.extensions[i].id);
                    }
                } else {
                    for &i in visible_indices {
                        state.selected_extensions.insert(state.extensions[i].id.clone());
                    }
                }
            }

            if ui.add_enabled(has_selection, egui::Button::new(lang::t("ext_batch_disable", lang))).clicked() {
                for ext in state.extensions.iter_mut() {
                    if state.selected_extensions.contains(&ext.id) {
                        ext.is_enabled = false;
                    }
                }
            }

            if ui.add_enabled(has_selection, egui::Button::new(lang::t("ext_batch_enable", lang))).clicked() {
                for ext in state.extensions.iter_mut() {
                    if state.selected_extensions.contains(&ext.id) {
                        ext.is_enabled = true;
                    }
                }
            }

            // 清除选中
            if has_selection {
                if ui.button("✕").clicked() {
                    state.selected_extensions.clear();
                }
            }

            ui.separator();

            // 退出批量模式
            if ui.button(lang::t("ext_batch_edit_exit", lang)).clicked() {
                state.batch_mode = false;
                state.selected_extensions.clear();
            }
        } else {
            // ---- 普通模式：仅显示 "批量编辑" 按钮 ----
            if ui.button(lang::t("ext_batch_edit", lang)).clicked() {
                state.batch_mode = true;
            }
        }

        // -- 右侧：添加扩展按钮（始终显示） --
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let icon = egui_phosphor::regular::PLUS;
            if ui.button(format!(" {}  {}", icon, lang::t("ext_add_extension", lang))).clicked() {
                state.show_add_dialog = true;
                state.git_url.clear();
                state.offline_path.clear();
                state.add_dialog_tab = AddDialogTab::Git;
            }
        });
    });
}

// ============ 分页栏 ============

fn render_pagination_bar(ui: &mut egui::Ui, state: &mut ExtensionManageState, total_visible: usize) {
    let total = (total_visible + state.page_size - 1) / state.page_size;
    if total <= 1 {
        return;
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // ◀ 上一页
        let prev_enabled = state.current_page > 0;
        if ui
            .add_enabled(
                prev_enabled,
                egui::Button::new(egui::RichText::new(egui_phosphor::regular::CARET_LEFT).size(14.0)),
            )
            .clicked()
        {
            state.current_page -= 1;
        }

        ui.add_space(4.0);

        // 页码按钮
        let max_visible = 7usize;
        if total <= max_visible {
            for p in 0..total {
                render_page_button(ui, state, p);
            }
        } else {
            // 总是显示第一页
            render_page_button(ui, state, 0);

            let window_start = state.current_page.saturating_sub(2).max(1);
            let window_end = (state.current_page + 2).min(total - 2);

            // 前面省略号
            if window_start > 1 {
                ui.add_sized(
                    [24.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("…").color(egui::Color32::GRAY),
                    )
                    .selectable(false),
                );
            }

            // 中间窗口
            for p in window_start..=window_end {
                render_page_button(ui, state, p);
            }

            // 后面省略号
            if window_end < total - 2 {
                ui.add_sized(
                    [24.0, 20.0],
                    egui::Label::new(
                        egui::RichText::new("…").color(egui::Color32::GRAY),
                    )
                    .selectable(false),
                );
            }

            // 总是显示最后一页
            render_page_button(ui, state, total - 1);
        }

        ui.add_space(4.0);

        // ▶ 下一页
        let next_enabled = state.current_page + 1 < total;
        if ui
            .add_enabled(
                next_enabled,
                egui::Button::new(egui::RichText::new(egui_phosphor::regular::CARET_RIGHT).size(14.0)),
            )
            .clicked()
        {
            state.current_page += 1;
        }

        // 总数
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("共 {} 个", total_visible))
                .size(12.0)
                .color(egui::Color32::GRAY),
        );
    });
}

fn render_page_button(ui: &mut egui::Ui, state: &mut ExtensionManageState, page: usize) {
    let is_current = page == state.current_page;
    let resp = ui.add_sized(
        [24.0, 20.0],
        egui::Button::selectable(is_current, (page + 1).to_string()),
    );
    if resp.clicked() {
        state.current_page = page;
    }
}

// ============ 添加扩展弹窗 ============

fn render_add_dialog(ui: &mut egui::Ui, state: &mut ExtensionManageState, lang: &Language) {
    if !state.show_add_dialog {
        return;
    }

    let mut was_open = true;

    egui::Window::new(lang::t("ext_add_dialog_title", lang))
        .collapsible(false)
        .resizable(false)
        .fixed_size([480.0, 260.0])
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .open(&mut was_open)
        .show(ui.ctx(), |ui| {
            // ---- Tab 切换栏 ----
            ui.horizontal(|ui| {
                let git_sel = state.add_dialog_tab == AddDialogTab::Git;
                if ui.selectable_label(git_sel, lang::t("ext_add_git_tab", lang)).clicked() {
                    state.add_dialog_tab = AddDialogTab::Git;
                }

                let off_sel = state.add_dialog_tab == AddDialogTab::Offline;
                if ui.selectable_label(off_sel, lang::t("ext_add_offline_tab", lang)).clicked() {
                    state.add_dialog_tab = AddDialogTab::Offline;
                }
            });

            ui.separator();
            ui.add_space(8.0);

            // ---- Tab 内容 ----
            match state.add_dialog_tab {
                AddDialogTab::Git => {
                    ui.label(lang::t("ext_add_git_url", lang));
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut state.git_url)
                            .hint_text(lang::t("ext_add_git_placeholder", lang))
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(lang::t("ext_add_git_note", lang))
                            .size(11.0)
                            .color(egui::Color32::GRAY),
                    );
                }
                AddDialogTab::Offline => {
                    ui.label(lang::t("ext_add_offline_hint", lang));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button(lang::t("ext_browse", lang)).clicked() {
                            let title = lang::t("dialog_select_folder", lang);
                            let picked = rfd::FileDialog::new().set_title(title).pick_folder();
                            if let Some(path) = picked {
                                state.offline_path = path.to_string_lossy().to_string();
                            }
                        }
                        ui.add_space(6.0);
                        if state.offline_path.is_empty() {
                            ui.label(
                                egui::RichText::new(lang::t("ext_add_offline_hint", lang))
                                    .color(egui::Color32::GRAY)
                                    .size(12.0),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(&state.offline_path)
                                    .size(12.0),
                            );
                        }
                    });
                }
            }

            ui.add_space(12.0);
            ui.separator();

            // ---- 底部按钮 ----
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(lang::t("ext_add_cancel", lang)).clicked() {
                        state.show_add_dialog = false;
                    }
                    ui.add_space(8.0);

                    let can_confirm = match state.add_dialog_tab {
                        AddDialogTab::Git => !state.git_url.trim().is_empty(),
                        AddDialogTab::Offline => !state.offline_path.is_empty(),
                    };
                    if ui.add_enabled(can_confirm, egui::Button::new(lang::t("ext_add_confirm", lang))).clicked() {
                        // TODO: 执行添加扩展逻辑
                        state.show_add_dialog = false;
                    }
                });
            });
        });

    if !was_open {
        state.show_add_dialog = false;
        state.git_url.clear();
        state.offline_path.clear();
        state.add_dialog_tab = AddDialogTab::Git;
    }
}

// ============ 扩展卡片 ============

fn render_extension_card(
    ui: &mut egui::Ui,
    ext: &mut ExtensionInfo,
    _cell_width: f32,
    lang: &Language,
    selected_extensions: &mut HashSet<String>,
    batch_mode: bool,
) {
    let is_selected = selected_extensions.contains(&ext.id);

    let border_stroke = if is_selected {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 180, 255))
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };

    let _card_rect = egui::Frame::NONE
        .fill(ui.visuals().window_fill())
        .corner_radius(8.0)
        .stroke(border_stroke)
        .inner_margin(12.0)
        .show(ui, |ui| {
            // 用 available_width() 确保内容填满 Frame 内边距后的剩余空间，绝不超出 Grid 单元格
            ui.set_width(ui.available_width());

            // 整体垂直布局
            ui.vertical(|ui| {
                // 上层：选中框（仅批量模式）+ 图标 + 名称 + [按钮组|分割线|开关]
                ui.horizontal(|ui| {
                    // 选中复选框（仅批量模式下显示）
                    if batch_mode {
                        let check_icon = if is_selected {
                            egui_phosphor::regular::CHECK_SQUARE
                        } else {
                            egui_phosphor::regular::SQUARE
                        };
                        let check_resp = ui.add_sized(
                            [22.0, 22.0],
                            egui::Button::selectable(
                                is_selected,
                                egui::RichText::new(check_icon).size(18.0),
                            ),
                        );
                        if check_resp.clicked() {
                            if is_selected {
                                selected_extensions.remove(&ext.id);
                            } else {
                                selected_extensions.insert(ext.id.clone());
                            }
                        }
                    }

                    // 图标
                    ui.label(egui::RichText::new(egui_phosphor::regular::PUZZLE_PIECE).size(20.0));
                    
                    // 名称
                    ui.label(
                        egui::RichText::new(&ext.manifest.display_name)
                            .strong()
                            .size(14.0)
                    );

                    // 靠右：按钮组 | 分割线 | 开关
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 开关
                        ui.add(toggle(&mut ext.is_enabled));

                        // 垂直分割线
                        ui.add(
                            egui::Separator::default()
                                .vertical()
                                .spacing(4.0),
                        );

                        // 图标按钮组
                        let btn_size = egui::vec2(20.0, 20.0);
                        let icon_size = 14.0;

                        // 删除扩展（官方扩展不可删除，防止酒馆异常）
                        if !ext.is_official {
                            if ui.add_sized(
                                btn_size,
                                egui::Button::new(
                                    egui::RichText::new(egui_phosphor::regular::TRASH).size(icon_size),
                                ),
                            ).on_hover_text(lang::t("ext_delete", lang)).clicked() {
                                // TODO: 删除扩展
                            }
                        }

                        // 打开目录
                        let folder_path = ext.path.clone();
                        if ui.add_sized(
                            btn_size,
                            egui::Button::new(
                                egui::RichText::new(egui_phosphor::regular::FOLDER_OPEN).size(icon_size),
                            ),
                        ).on_hover_text(lang::t("ext_open_folder", lang)).clicked() {
                            let _ = std::process::Command::new("open")
                                .arg(&folder_path)
                                .spawn();
                        }

                        // 打开主页（仅当 URL 存在且非默认值）
                        let hp = &ext.manifest.home_page;
                        if !hp.is_empty() && hp != "https://github.com/SillyTavern/SillyTavern" && hp != "None" {
                            let url = hp.clone();
                            if ui.add_sized(
                                btn_size,
                                egui::Button::new(
                                    egui::RichText::new(egui_phosphor::regular::LINK).size(icon_size),
                                ),
                            ).on_hover_text(lang::t("ext_open_homepage", lang)).clicked() {
                                let _ = std::process::Command::new("open").arg(&url).spawn();
                            }
                        }
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // 下层：信息区域 (自动换行流式布局)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
                    
                    let mut items_added = 0;

                    // 官方标记（移到信息栏第一位，在 add_item 闭包定义之前渲染）
                    if ext.is_official {
                        let tag_color = egui::Color32::from_rgb(100, 180, 255);
                        egui::Frame::NONE
                            .fill(tag_color.linear_multiply(0.1))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(4, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(lang::t("ext_official", lang))
                                        .color(tag_color)
                                        .size(10.0)
                                );
                            });
                        items_added += 1;
                    }

                    let mut add_item = |ui: &mut egui::Ui, label: &str, value_widget: Box<dyn FnOnce(&mut egui::Ui)>| {
                        if items_added > 0 {
                            ui.label(egui::RichText::new("|").color(egui::Color32::from_gray(80)));
                        }
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            ui.label(egui::RichText::new(label).color(egui::Color32::GRAY).size(12.0));
                            value_widget(ui);
                        });
                        items_added += 1;
                    };

                    // 版本
                    if !ext.manifest.version.is_empty() {
                        add_item(ui, lang::t("ext_version", lang), Box::new(|ui| {
                            ui.label(egui::RichText::new(&ext.manifest.version).size(12.0));
                        }));
                    }

                    // 作者
                    if !ext.manifest.author.is_empty() {
                        add_item(ui, lang::t("ext_author", lang), Box::new(|ui| {
                            ui.label(egui::RichText::new(&ext.manifest.author).size(12.0));
                        }));
                    }

                    // 自动更新（仅当清单中有该字段时才显示）
                    if let Some(auto_update) = ext.manifest.auto_update {
                        add_item(ui, lang::t("ext_auto_update", lang), Box::new(move |ui| {
                            let text = if auto_update {
                                lang::t("on", lang)
                            } else {
                                lang::t("off", lang)
                            };
                            ui.label(egui::RichText::new(text).size(12.0));
                        }));
                    }

                    // 主页
                    let hp = &ext.manifest.home_page;
                    if !hp.is_empty() && hp != "https://github.com/SillyTavern/SillyTavern" && hp != "None" {
                        let url = hp.clone();
                        add_item(ui, lang::t("ext_homepage", lang), Box::new(move |ui| {
                            ui.hyperlink_to(lang::t("ext_view", lang), url);
                        }));
                    }

                    // 最低客户端版本 (必须放在最后)
                    if !ext.manifest.minimum_client_version.is_empty() {
                        add_item(ui, lang::t("ext_min_version", lang), Box::new(|ui| {
                            ui.label(egui::RichText::new(&ext.manifest.minimum_client_version).size(12.0));
                        }));
                    }
                });
            });
        });
}
