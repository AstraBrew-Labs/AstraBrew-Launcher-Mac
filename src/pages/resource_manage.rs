use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::lang;
use crate::pages::settings::{Language, TavernDataMode};
use crate::utils;

// ============================================================================
// Tab 枚举
// ============================================================================

#[derive(PartialEq, Default, Clone, Copy)]
pub enum ResourceManageTab {
    #[default]
    CharacterCards,
    WorldBooks,
    ChatHistory,
}

// ============================================================================
// 数据结构
// ============================================================================

/// 角色卡嵌入的世界书条目
#[derive(Clone, Default)]
pub struct WorldEntry {
    pub keys: Vec<String>,
    pub content: String,
    pub comment: String,
    pub enabled: bool,
}

/// 角色卡嵌入的世界书信息
#[derive(Clone, Default)]
pub struct EmbeddedWorldInfo {
    pub name: String,
    pub entries: Vec<WorldEntry>,
}

/// 独立世界书信息
#[derive(Clone)]
#[allow(dead_code)]
pub struct WorldBookInfo {
    pub filename: String,
    pub filepath: PathBuf,
    pub name: String,
    pub author: String,
    pub created_secs: u64,
    pub entry_count: usize,
    pub entries: Vec<WorldEntry>,
    pub file_size: u64,
    pub modified_secs: u64,
}

/// 角色卡完整信息
#[derive(Clone)]
#[allow(dead_code)]
pub struct CharacterCardInfo {
    pub filename: String,
    pub filepath: PathBuf,
    pub name: String,
    pub description: String,
    pub creator: String,
    pub version: String,
    pub tags: Vec<String>,
    pub personality: String,
    pub scenario: String,
    pub first_message: String,
    pub avatar: String,
    pub spec: String,
    pub spec_version: String,
    pub world_info: Option<EmbeddedWorldInfo>,
    pub file_size: u64,
    pub modified_secs: u64,
    pub image_width: u32,
    pub image_height: u32,
}

// ============================================================================
// 页面状态
// ============================================================================

pub struct ResourceManageState {
    pub tab: ResourceManageTab,
    // 角色卡
    pub characters: Vec<CharacterCardInfo>,
    pub characters_loaded: bool,
    pub is_loading: bool,
    // 世界书
    pub world_books: Vec<WorldBookInfo>,
    pub world_books_loaded: bool,
    pub is_loading_wb: bool,
    // 实例信息
    pub instance_path: String,
    pub data_mode: TavernDataMode,
    // 详情弹窗
    pub selected_char_idx: Option<usize>,
    pub worldbook_page: usize,
    // 世界书详情弹窗
    pub selected_wb_idx: Option<usize>,
    pub wb_detail_page: usize,
}

impl Default for ResourceManageState {
    fn default() -> Self {
        Self {
            tab: ResourceManageTab::default(),
            characters: Vec::new(),
            characters_loaded: false,
            is_loading: false,
            world_books: Vec::new(),
            world_books_loaded: false,
            is_loading_wb: false,
            instance_path: String::new(),
            data_mode: TavernDataMode::Current,
            selected_char_idx: None,
            worldbook_page: 0,
            selected_wb_idx: None,
            wb_detail_page: 0,
        }
    }
}

impl ResourceManageState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_instance(&self) -> bool {
        !self.instance_path.is_empty()
    }

    /// 获取角色卡目录路径
    pub fn characters_dir(&self) -> Option<PathBuf> {
        if self.instance_path.is_empty() {
            return None;
        }
        match self.data_mode {
            TavernDataMode::Current => Some(
                PathBuf::from(&self.instance_path)
                    .join("data")
                    .join("default-user")
                    .join("characters"),
            ),
            TavernDataMode::Global => Some(
                utils::app_paths()
                    .default_global_data_dir()
                    .join("default-user")
                    .join("characters"),
            ),
        }
    }

    /// 加载角色卡列表
    pub fn load_characters(&mut self) {
        if self.characters_loaded || self.is_loading {
            return;
        }
        self.is_loading = true;
        self.characters.clear();

        let dir = match self.characters_dir() {
            Some(d) => d,
            None => {
                self.characters_loaded = true;
                self.is_loading = false;
                return;
            }
        };

        if !dir.exists() {
            self.characters_loaded = true;
            self.is_loading = false;
            return;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                self.characters_loaded = true;
                self.is_loading = false;
                return;
            }
        };

        let mut cards = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            // 只处理 .png 文件，跳过子文件夹
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.to_lowercase() != "png" {
                continue;
            }

            let meta = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let file_size = meta.len();
            let modified_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // 从 PNG 解析角色元数据
            let (
                name, description, creator, version, tags,
                personality, scenario, first_message, avatar,
                spec, spec_version, world_info,
            ) = parse_character_png(&path);

            let display_name = if name.is_empty() {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                name
            };

            let (image_width, image_height) = if let Ok(data) = fs::read(&path) {
                read_png_dimensions(&data).unwrap_or((400, 600))
            } else {
                (400, 600)
            };

            cards.push(CharacterCardInfo {
                filename,
                filepath: path,
                name: display_name,
                description,
                creator,
                version,
                tags,
                personality,
                scenario,
                first_message,
                avatar,
                spec,
                spec_version,
                world_info,
                file_size,
                modified_secs,
                image_width,
                image_height,
            });
        }

        // 按修改时间倒序
        cards.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));

        self.characters = cards;
        self.characters_loaded = true;
        self.is_loading = false;
    }

    /// 获取世界书目录路径
    pub fn worlds_dir(&self) -> Option<PathBuf> {
        if self.instance_path.is_empty() {
            return None;
        }
        match self.data_mode {
            TavernDataMode::Current => Some(
                PathBuf::from(&self.instance_path)
                    .join("data")
                    .join("default-user")
                    .join("worlds"),
            ),
            TavernDataMode::Global => Some(
                utils::app_paths()
                    .default_global_data_dir()
                    .join("default-user")
                    .join("worlds"),
            ),
        }
    }

    /// 加载世界书列表
    pub fn load_world_books(&mut self) {
        if self.world_books_loaded || self.is_loading_wb {
            return;
        }
        self.is_loading_wb = true;
        self.world_books.clear();

        let dir = match self.worlds_dir() {
            Some(d) => d,
            None => {
                self.world_books_loaded = true;
                self.is_loading_wb = false;
                return;
            }
        };

        if !dir.exists() {
            self.world_books_loaded = true;
            self.is_loading_wb = false;
            return;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => {
                self.world_books_loaded = true;
                self.is_loading_wb = false;
                return;
            }
        };

        let mut books = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.to_lowercase() != "json" {
                continue;
            }

            let meta = match fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let file_size = meta.len();
            let modified_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            // 从 JSON 解析世界书
            let data = match fs::read_to_string(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let parsed: serde_json::Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let obj = match parsed.as_object() {
                Some(o) => o,
                None => continue,
            };

            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let author = obj
                .get("author")
                .or_else(|| obj.get("creator"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let created_secs = obj
                .get("createdAt")
                .or_else(|| obj.get("created"))
                .or_else(|| obj.get("created_at"))
                .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                .unwrap_or(modified_secs);

            // entries: 可能是数组或对象（ID-keyed）
            let entries_val = obj.get("entries");
            let parsed_entries: Vec<WorldEntry> = match entries_val {
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|e| e.as_object())
                    .map(|e| parse_world_book_entry(e))
                    .collect(),
                Some(serde_json::Value::Object(map)) => map
                    .values()
                    .filter_map(|v| v.as_object())
                    .map(|e| parse_world_book_entry(e))
                    .collect(),
                _ => Vec::new(),
            };

            let display_name = if name.is_empty() {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                name
            };

            books.push(WorldBookInfo {
                filename,
                filepath: path,
                name: display_name,
                author,
                created_secs,
                entry_count: parsed_entries.len(),
                entries: parsed_entries,
                file_size,
                modified_secs,
            });
        }

        // 按修改时间倒序
        books.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));

        self.world_books = books;
        self.world_books_loaded = true;
        self.is_loading_wb = false;
    }

    /// 重新加载
    pub fn refresh(&mut self) {
        self.characters_loaded = false;
        self.is_loading = false;
        self.selected_char_idx = None;
        self.world_books_loaded = false;
        self.is_loading_wb = false;
        self.selected_wb_idx = None;
        self.wb_detail_page = 0;
    }
}

// ============================================================================
// PNG 元数据解析
// ============================================================================

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

/// 读取 PNG 文件的 IHDR 块获取图片宽高
fn read_png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 33 || data[0..8] != PNG_SIGNATURE {
        return None;
    }
    // IHDR 是第一个 chunk：length(4) + "IHDR"(4) + width(4) + height(4) + ...
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

/// 从 PNG 文件中解析角色卡元数据
fn parse_character_png(
    path: &PathBuf,
) -> (
    String,
    String,
    String,
    String,
    Vec<String>,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<EmbeddedWorldInfo>,
) {
    let default = (
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        Vec::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        None,
    );

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return default,
    };

    if data.len() < 8 || data[0..8] != PNG_SIGNATURE {
        return default;
    }

    let mut text_chunks: Vec<(String, String)> = Vec::new();
    let mut pos = 8usize;

    while pos + 12 <= data.len() {
        let chunk_len = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]])
            as usize;
        let chunk_type = &data[pos + 4..pos + 8];

        if pos + 12 + chunk_len > data.len() {
            break;
        }

        let chunk_end = pos + 12 + chunk_len;

        if chunk_type == b"tEXt" {
            let payload = &data[pos + 8..pos + 8 + chunk_len];
            if let Some(null_idx) = payload.iter().position(|&b| b == 0) {
                let keyword =
                    String::from_utf8_lossy(&payload[..null_idx]).to_string();
                let text =
                    String::from_utf8_lossy(&payload[null_idx + 1..]).to_string();
                text_chunks.push((keyword, text));
            }
        }

        if chunk_type == b"IEND" {
            break;
        }

        pos = chunk_end;
    }

    // 按优先级查找：chara > ccv3
    let keyword_priority = ["chara", "ccv3"];
    let mut json_text: Option<String> = None;

    'outer: for kw in &keyword_priority {
        for (keyword, text) in &text_chunks {
            if keyword.to_lowercase().contains(kw) {
                json_text = Some(text.clone());
                break 'outer;
            }
        }
    }

    if json_text.is_none() {
        json_text = text_chunks.first().map(|(_, t)| t.clone());
    }

    let json_text = match json_text {
        Some(t) => t,
        None => return default,
    };

    let decoded = match decode_base64_to_json(&json_text) {
        Ok(v) => v,
        Err(_) => match serde_json::from_str(&json_text) {
            Ok(v) => v,
            Err(_) => return default,
        },
    };

    let data_obj = decoded.get("data").and_then(|v| v.as_object());

    let pick = |keys: &[&str]| -> String {
        for k in keys {
            if let Some(obj) = data_obj {
                if let Some(v) = obj.get(*k).and_then(|v| v.as_str()) {
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
            if let Some(v) = decoded.get(*k).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        String::new()
    };

    let name = pick(&["name"]);
    let description = pick(&["description"]);
    let creator = pick(&["creator"]);
    let personality = pick(&["personality"]);
    let scenario = pick(&["scenario"]);
    let first_message = pick(&["first_mes", "firstMessage"]);
    let avatar = pick(&["avatar"]);
    let spec = decoded.get("spec").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let spec_version = decoded
        .get("spec_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let version = data_obj
        .and_then(|d| d.get("character_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tags: Vec<String> = data_obj
        .and_then(|d| d.get("tags"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let world_info = extract_world_book(&decoded);

    (
        name, description, creator, version, tags, personality, scenario,
        first_message, avatar, spec, spec_version, world_info,
    )
}

/// 从角色卡 JSON 中提取嵌入的世界书
fn extract_world_book(parsed: &serde_json::Value) -> Option<EmbeddedWorldInfo> {
    let wb_keys = ["character_book", "worldbook", "world_info", "lorebook"];
    let mut wb_obj: Option<&serde_json::Map<String, serde_json::Value>> = None;

    for key in &wb_keys {
        if let Some(obj) = parsed.get(*key).and_then(|v| v.as_object()) {
            wb_obj = Some(obj);
            break;
        }
    }

    if wb_obj.is_none() {
        if let Some(data) = parsed.get("data").and_then(|v| v.as_object()) {
            for key in &wb_keys {
                if let Some(obj) = data.get(*key).and_then(|v| v.as_object()) {
                    wb_obj = Some(obj);
                    break;
                }
            }
        }
    }

    if wb_obj.is_none() {
        for ext_parent in ["data", ""] {
            let extensions = if ext_parent.is_empty() {
                parsed.get("extensions")
            } else {
                parsed
                    .get(ext_parent)
                    .and_then(|d| d.get("extensions"))
            };
            if let Some(ext) = extensions.and_then(|v| v.as_object()) {
                for key in &["world", "worldbook", "character_book", "lorebook"] {
                    if let Some(obj) = ext.get(*key).and_then(|v| v.as_object()) {
                        wb_obj = Some(obj);
                        break;
                    }
                }
                if wb_obj.is_some() {
                    break;
                }
            }
        }
    }

    let wb = wb_obj?;
    let entries_arr = wb.get("entries").and_then(|v| v.as_array())?;
    if entries_arr.is_empty() {
        return None;
    }

    let wb_name = wb
        .get("name")
        .or_else(|| wb.get("title"))
        .or_else(|| wb.get("world_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let entries: Vec<WorldEntry> = entries_arr
        .iter()
        .filter_map(|entry| entry.as_object())
        .map(|e| WorldEntry {
            keys: e
                .get("keys")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            content: e
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            comment: e
                .get("comment")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            enabled: e
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
        })
        .collect();

    if entries.is_empty() {
        return None;
    }

    Some(EmbeddedWorldInfo {
        name: wb_name,
        entries,
    })
}

/// 从 JSON 对象解析世界书条目（同时支持 keys/key/disable/enabled）
fn parse_world_book_entry(obj: &serde_json::Map<String, serde_json::Value>) -> WorldEntry {
    // 触发词：支持 "keys"（数组）和 "key"或"primaryKeys"（单个字符串解析为数组）
    let keys: Vec<String> = obj
        .get("keys")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .or_else(|| {
            obj.get("key").or_else(|| obj.get("primaryKeys")).map(|v| {
                if let Some(arr) = v.as_array() {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(|s| s.to_string()))
                        .collect()
                } else if let Some(s) = v.as_str() {
                    s.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    Vec::new()
                }
            })
        })
        .unwrap_or_default();

    let content = obj
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let comment = obj
        .get("comment")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let enabled = obj
        .get("enabled")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            obj.get("disable")
                .and_then(|v| v.as_bool())
                .map(|d| !d)
        })
        .unwrap_or(true);

    WorldEntry {
        keys,
        content,
        comment,
        enabled,
    }
}

/// Base64 解码并解析为 JSON
fn decode_base64_to_json(input: &str) -> Result<serde_json::Value, ()> {
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err(());
    }

    let normalized = cleaned.replace('-', "+").replace('_', "/");
    let padded = match normalized.len() % 4 {
        2 => normalized + "==",
        3 => normalized + "=",
        _ => normalized,
    };

    let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;

    for byte in padded.bytes() {
        if byte == b'=' {
            break;
        }
        let val = table.iter().position(|&c| c == byte).ok_or(())? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    let json_str = String::from_utf8(output).map_err(|_| ())?;
    serde_json::from_str(&json_str).map_err(|_| ())
}

// ============================================================================
// 格式化辅助函数
// ============================================================================

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_timestamp(secs: u64) -> String {
    if secs == 0 {
        return String::from("Unknown");
    }
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    let year = 1970 + (days / 365) as u64;
    let day_of_year = days % 365;

    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u64;
    let mut remaining = day_of_year;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as u64 {
            month = i as u64 + 1;
            break;
        }
        remaining -= md as u64;
        month = i as u64 + 1;
    }
    let day = remaining + 1;

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

// ============================================================================
// UI 渲染
// ============================================================================

pub fn render(
    ui: &mut egui::Ui,
    state: &mut ResourceManageState,
    language: &Language,
) {
    ui.add_space(8.0);

    // -- Tab 切换条 --
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut state.tab,
            ResourceManageTab::CharacterCards,
            lang::t("rm_tab_characters", language),
        );
        ui.selectable_value(
            &mut state.tab,
            ResourceManageTab::WorldBooks,
            lang::t("rm_tab_worlds", language),
        );
        ui.selectable_value(
            &mut state.tab,
            ResourceManageTab::ChatHistory,
            lang::t("rm_tab_chats", language),
        );
    });

    ui.separator();
    ui.add_space(12.0);

    if !state.has_instance() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(lang::t("rm_no_instance", language))
                    .color(egui::Color32::GRAY)
                    .size(14.0),
            );
            ui.label(
                egui::RichText::new(lang::t("rm_no_instance_hint", language))
                    .color(egui::Color32::GRAY)
                    .size(12.0),
            );
        });
        return;
    }

    match state.tab {
        ResourceManageTab::CharacterCards => render_character_cards(ui, state, language),
        ResourceManageTab::WorldBooks => render_world_books(ui, state, language),
        ResourceManageTab::ChatHistory => render_chat_history(ui, language),
    }
}

// ============================================================================
// 角色卡管理 Tab — 卡片网格布局
// ============================================================================

const CARD_COLUMNS: usize = 4;
const CARD_SPACING: f32 = 12.0;

fn render_character_cards(
    ui: &mut egui::Ui,
    state: &mut ResourceManageState,
    language: &Language,
) {
    state.load_characters();

    // 顶部操作栏
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                lang::t("rm_count", language)
                    .replace("{n}", &state.characters.len().to_string()),
            )
            .size(13.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized(
                    [70.0, 24.0],
                    egui::Button::new(
                        egui::RichText::new(lang::t("rm_refresh", language)).size(12.0),
                    ),
                )
                .clicked()
            {
                state.refresh();
            }
        });
    });
    ui.separator();
    ui.add_space(6.0);

    // 加载中
    if state.is_loading {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new(lang::t("rm_loading", language))
                    .size(13.0)
                    .color(egui::Color32::GRAY),
            );
        });
        return;
    }

    // 空状态
    if state.characters.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(lang::t("rm_empty_characters", language))
                    .color(egui::Color32::GRAY)
                    .size(13.0),
            );
        });
        return;
    }

    // 卡片网格
    let available_w = ui.available_width();
    let card_w = ((available_w - CARD_SPACING * (CARD_COLUMNS as f32 - 1.0))
        / CARD_COLUMNS as f32)
        .floor()
        .max(120.0);

    // 根据第一张角色卡的实际尺寸计算图片高度
    let image_h = if let Some(first) = state.characters.first() {
        if first.image_width > 0 && first.image_height > 0 {
            card_w * first.image_height as f32 / first.image_width as f32
        } else {
            200.0
        }
    } else {
        200.0
    };

    let card_count = state.characters.len();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    egui::Grid::new("character_cards_grid")
                        .spacing([CARD_SPACING, CARD_SPACING])
                        .min_col_width(card_w)
                        .max_col_width(card_w)
                        .show(ui, |ui| {
                            let mut to_select: Option<usize> = None;
                            for idx in 0..card_count {
                                if idx > 0 && idx % CARD_COLUMNS == 0 {
                                    ui.end_row();
                                }
                                // 复制必要数据，避免借出冲突
                                let card_data = state.characters[idx].filepath.clone();
                                let card_name = state.characters[idx].name.clone();
                                let hover_text =
                                    lang::t("rm_click_detail", language).to_string();
                                if render_character_card(
                                    ui,
                                    &card_data,
                                    &card_name,
                                    &hover_text,
                                    card_w,
                                    image_h,
                                    idx,
                                ) {
                                    to_select = Some(idx);
                                }
                            }
                            if let Some(idx) = to_select {
                                state.selected_char_idx = Some(idx);
                                state.worldbook_page = 0;
                            }
                        });
                });
        });

    // 详情弹窗
    if let Some(idx) = state.selected_char_idx {
        if idx < state.characters.len() {
            let card = state.characters[idx].clone();
            let close =
                render_character_detail_popup(ui.ctx(), &card, language, &mut state.worldbook_page);
            if close {
                state.selected_char_idx = None;
                state.worldbook_page = 0;
            }
        } else {
            state.selected_char_idx = None;
        }
    }
}

/// 单个角色卡卡片渲染，返回是否被点击
fn render_character_card(
    ui: &mut egui::Ui,
    filepath: &PathBuf,
    name: &str,
    hover_text: &str,
    card_w: f32,
    image_h: f32,
    _idx: usize,
) -> bool {
    let total_h = image_h + 28.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(card_w, total_h),
        egui::Sense::click(),
    );

    // 背景
    let bg = ui.visuals().faint_bg_color;
    ui.painter().rect_filled(rect, 8.0, bg);

    // 图片区域
    let image_rect = egui::Rect::from_min_size(rect.min, egui::vec2(card_w, image_h));

    // 加载图片
    if let Ok(data) = fs::read(filepath) {
        let uri = format!("bytes://char_thumb/{}", filepath.to_string_lossy());
        let image = egui::Image::from_bytes(uri, data)
            .fit_to_exact_size(egui::vec2(card_w, image_h));
        ui.put(image_rect, image);
    } else {
        // 占位
        ui.painter().text(
            image_rect.center(),
            egui::Align2::CENTER_CENTER,
            egui_phosphor::regular::USER_CIRCLE,
            egui::FontId::proportional(36.0),
            egui::Color32::from_gray(140),
        );
    }

    // 悬停遮罩
    if response.hovered() {
        let hover_bg = egui::Color32::from_rgba_premultiplied(0, 0, 0, 160);
        ui.painter().rect_filled(image_rect, 4.0, hover_bg);
        ui.painter().text(
            image_rect.center(),
            egui::Align2::CENTER_CENTER,
            hover_text,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    // 名称
    let name_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x, rect.min.y + image_h),
        egui::vec2(card_w, 24.0),
    );
    ui.painter().text(
        egui::pos2(name_rect.min.x + 6.0, name_rect.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    response.clicked()
}

// ============================================================================
// 角色卡详情弹窗
// ============================================================================

/// 渲染一行信息项（label: value | label: value | label: value）
fn render_info_row(ui: &mut egui::Ui, items: &[(&str, String)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
        for (i, (label, value)) in items.iter().enumerate() {
            if i > 0 {
                ui.label(
                    egui::RichText::new("|")
                        .color(egui::Color32::from_gray(80))
                        .size(11.0),
                );
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(*label)
                        .color(egui::Color32::GRAY)
                        .size(12.0),
                );
                ui.label(egui::RichText::new(value.as_str()).size(12.0));
            });
        }
    });
}

/// 渲染世界书条目卡片
fn render_world_entry_card(
    ui: &mut egui::Ui,
    entry: &WorldEntry,
    language: &Language,
    card_w: f32,
    card_h: f32,
) {
    let (card_rect, _response) = ui.allocate_exact_size(
        egui::vec2(card_w, card_h),
        egui::Sense::hover(),
    );

    // 卡片背景
    let bg = ui.visuals().faint_bg_color;
    ui.painter().rect_filled(card_rect, 6.0, bg);

    // 内边距
    let inner_rect = card_rect.shrink(8.0);
    let mut content_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    content_ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);

    // 第一行：状态指示器 + 触发词
    content_ui.horizontal(|ui| {
        let status_color = if entry.enabled {
            egui::Color32::from_rgb(80, 200, 80)
        } else {
            egui::Color32::GRAY
        };
        ui.label(egui::RichText::new("●").color(status_color).size(10.0));
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(lang::t("rm_detail_wb_keys", language))
                .color(egui::Color32::GRAY)
                .size(11.0),
        );
        if !entry.keys.is_empty() {
            let keys_text = entry.keys.join(", ");
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(keys_text)
                    .size(12.0)
                    .strong(),
            );
        }
    });

    // 第二行：备注
    if !entry.comment.is_empty() {
        content_ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(lang::t("rm_detail_wb_comment", language))
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(&entry.comment)
                    .size(11.0)
                    .color(egui::Color32::from_gray(180)),
            );
        });
    }

    // 第三行起：内容（标签 + 可滚动文本区域）
    if !entry.content.is_empty() {
        content_ui.add_space(2.0);

        // 内容标签
        content_ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(lang::t("rm_detail_wb_content", language))
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
        });

        // 可滚动内容区域
        let content_max_h = 60.0;
        egui::ScrollArea::vertical()
            .max_height(content_max_h)
            .auto_shrink([false, true])
            .show(&mut content_ui, |ui| {
                ui.set_max_width(inner_rect.width() - 14.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.spacing_mut().item_spacing.y = 2.0;
                // 通过左边距实现缩进
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new(&entry.content)
                            .size(11.0)
                            .color(egui::Color32::from_gray(200)),
                    );
                });
            });
    }
}

fn render_character_detail_popup(
    ctx: &egui::Context,
    card: &CharacterCardInfo,
    language: &Language,
    worldbook_page: &mut usize,
) -> bool {
    let mut close = false;

    egui::Window::new(format!(
        "{} - {}",
        lang::t("rm_detail_title", language),
        card.name
    ))
    .collapsible(false)
    .resizable(true)
    .default_size([720.0, 560.0])
    .min_size([480.0, 360.0])
    .show(ctx, |ui| {
        // 上半部分：左右结构
        ui.horizontal(|ui| {
            // 左侧：角色卡图片
            let img_size = 240.0;
            let (image_rect, _response) = ui.allocate_exact_size(
                egui::vec2(img_size, img_size),
                egui::Sense::hover(),
            );

            let bg = ui.visuals().faint_bg_color;
            ui.painter().rect_filled(image_rect, 6.0, bg);

            if let Ok(data) = fs::read(&card.filepath) {
                let uri = format!("bytes://char_detail/{}", card.filepath.to_string_lossy());
                // 根据实际图片尺寸计算弹窗内图片的显示高度
                let detail_image_h = if card.image_width > 0 && card.image_height > 0 {
                    img_size * card.image_height as f32 / card.image_width as f32
                } else {
                    img_size
                };
                let image = egui::Image::from_bytes(uri, data)
                    .fit_to_exact_size(egui::vec2(img_size, detail_image_h));
                let detail_rect = egui::Rect::from_min_size(
                    image_rect.min,
                    egui::vec2(img_size, detail_image_h),
                );
                ui.put(detail_rect, image);
            }
            ui.add_space(16.0);

            // 右侧：基本信息
            ui.vertical(|ui| {
                // 第一行：角色卡名称
                ui.label(egui::RichText::new(&card.name).size(20.0).strong());

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // 每行固定 3 个信息项，行尾无分隔符
                let items_per_row = 3;

                let mut items: Vec<(&str, String)> = Vec::new();
                if !card.creator.is_empty() {
                    items.push((lang::t("rm_detail_creator", language), card.creator.clone()));
                }
                if !card.version.is_empty() {
                    items.push((lang::t("rm_detail_version", language), card.version.clone()));
                }
                if !card.spec.is_empty() {
                    items.push((lang::t("rm_detail_spec", language), card.spec.clone()));
                }
                if !card.spec_version.is_empty() {
                    items.push((lang::t("rm_detail_spec_version", language), card.spec_version.clone()));
                }
                items.push((lang::t("rm_detail_size", language), format_size(card.file_size)));
                if card.modified_secs > 0 {
                    items.push((lang::t("rm_detail_modified", language), format_timestamp(card.modified_secs)));
                }

                // 逐行渲染
                let mut row: Vec<(&str, String)> = Vec::new();
                for (label, value) in items {
                    row.push((label, value));
                    if row.len() >= items_per_row {
                        render_info_row(ui, &row);
                        row.clear();
                    }
                }
                if !row.is_empty() {
                    render_info_row(ui, &row);
                }

                // 标签 badge 单独一行
                if !card.tags.is_empty() {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                        for tag in &card.tags {
                            let tag_bg = egui::Color32::from_rgb(100, 160, 220)
                                .linear_multiply(0.15);
                            egui::Frame::NONE
                                .fill(tag_bg)
                                .corner_radius(3.0)
                                .inner_margin(egui::Margin::symmetric(5, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("#{}", tag))
                                            .color(egui::Color32::from_rgb(100, 160, 220))
                                            .size(11.0),
                                    );
                                });
                        }
                    });
                }

                // 描述：放在最后，垂直排列，加 ScrollArea 防止撑破布局
                if !card.description.is_empty() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(lang::t("rm_detail_description", language))
                            .size(13.0)
                            .strong(),
                    );
                    ui.add_space(4.0);

                    let desc_height = 120.0;
                    egui::ScrollArea::vertical()
                        .max_height(desc_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            ui.label(
                                egui::RichText::new(&card.description)
                                    .size(12.0),
                            );
                        });
                }
            });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // 下半部分：绑定的世界书
        ui.label(
            egui::RichText::new(lang::t("rm_detail_worldbook", language))
                .size(15.0)
                .strong(),
        );
        ui.add_space(6.0);

        match &card.world_info {
            Some(wb) => {
                if wb.entries.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            lang::t("rm_detail_wb_name", language),
                            wb.name
                        ))
                        .size(13.0),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(lang::t("rm_detail_no_worldbook", language))
                            .color(egui::Color32::GRAY)
                            .size(13.0),
                    );
                } else {
                    let cols: usize = 2;
                    let page_size = cols; // 每页 2 个条目（一行）
                    let total_pages = (wb.entries.len() + page_size - 1) / page_size;

                    // 超出范围时修正页码
                    if *worldbook_page >= total_pages {
                        *worldbook_page = total_pages.saturating_sub(1);
                    }

                    // 世界书名称 + 分页组件（同一行，分页靠右）
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                lang::t("rm_detail_wb_name", language),
                                wb.name
                            ))
                            .size(13.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // 下一页按钮
                            if ui
                                .add_sized(
                                    [22.0, 20.0],
                                    egui::Button::new(
                                        egui::RichText::new("▶").size(12.0),
                                    ),
                                )
                                .clicked()
                                && *worldbook_page + 1 < total_pages
                            {
                                *worldbook_page += 1;
                            }

                            // 页数
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} / {}",
                                    *worldbook_page + 1,
                                    total_pages
                                ))
                                .size(12.0),
                            );

                            // 上一页按钮
                            if ui
                                .add_sized(
                                    [22.0, 20.0],
                                    egui::Button::new(
                                        egui::RichText::new("◀").size(12.0),
                                    ),
                                )
                                .clicked()
                                && *worldbook_page > 0
                            {
                                *worldbook_page -= 1;
                            }
                        });
                    });

                    ui.add_space(8.0);

                    // 2 列网格，仅渲染当前页
                    let available_w = ui.available_width();
                    let grid_spacing = 10.0;
                    let card_w = ((available_w - grid_spacing * (cols as f32 - 1.0))
                        / cols as f32)
                        .floor()
                        .max(200.0);
                    let card_h = 135.0;

                    let start = *worldbook_page * page_size;
                    let end = (start + page_size).min(wb.entries.len());

                    egui::Grid::new("world_entries_grid")
                        .spacing([grid_spacing, grid_spacing])
                        .min_col_width(card_w)
                        .max_col_width(card_w)
                        .show(ui, |ui| {
                            for (col, entry) in wb.entries[start..end].iter().enumerate() {
                                ui.push_id(start + col, |ui| {
                                    render_world_entry_card(
                                        ui, entry, language, card_w, card_h,
                                    );
                                });
                            }
                        });
                }
            }
            None => {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(lang::t("rm_detail_no_worldbook", language))
                            .color(egui::Color32::GRAY)
                            .size(13.0),
                    );
                });
            }
        }

        // 关闭按钮
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if ui.button(lang::t("rm_close", language)).clicked() {
                close = true;
            }
        });
    });

    close
}

// ============================================================================
// 世界书管理 Tab — 3 列卡片网格
// ============================================================================

const WB_CARD_COLUMNS: usize = 3;
const WB_CARD_SPACING: f32 = 12.0;
const WB_CARD_HEIGHT: f32 = 120.0;
const WB_CARD_NAME_HEIGHT: f32 = 56.0;
const WB_CARD_INFO_HEIGHT: f32 = 56.0;

fn render_world_books(
    ui: &mut egui::Ui,
    state: &mut ResourceManageState,
    language: &Language,
) {
    state.load_world_books();

    // 顶部操作栏
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                lang::t("rm_count", language)
                    .replace("{n}", &state.world_books.len().to_string()),
            )
            .size(13.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized(
                    [70.0, 24.0],
                    egui::Button::new(
                        egui::RichText::new(lang::t("rm_refresh", language)).size(12.0),
                    ),
                )
                .clicked()
            {
                state.refresh();
            }
        });
    });
    ui.separator();
    ui.add_space(6.0);

    // 加载中
    if state.is_loading_wb {
        ui.add_space(60.0);
        ui.vertical_centered(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new(lang::t("rm_loading", language))
                    .size(13.0)
                    .color(egui::Color32::GRAY),
            );
        });
        return;
    }

    // 空状态
    if state.world_books.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(lang::t("rm_empty_worlds", language))
                    .color(egui::Color32::GRAY)
                    .size(13.0),
            );
        });
        return;
    }

    // 卡片网格
    let available_w = ui.available_width();
    let card_w = ((available_w - WB_CARD_SPACING * (WB_CARD_COLUMNS as f32 - 1.0))
        / WB_CARD_COLUMNS as f32)
        .floor()
        .max(160.0);

    let book_count = state.world_books.len();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(0, 4))
                .show(ui, |ui| {
                    egui::Grid::new("world_books_grid")
                        .spacing([WB_CARD_SPACING, WB_CARD_SPACING])
                        .min_col_width(card_w)
                        .max_col_width(card_w)
                        .show(ui, |ui| {
                            let mut to_select: Option<usize> = None;
                            for idx in 0..book_count {
                                if idx > 0 && idx % WB_CARD_COLUMNS == 0 {
                                    ui.end_row();
                                }
                                let wb_name = state.world_books[idx].name.clone();
                                let wb_author = state.world_books[idx].author.clone();
                                let wb_created = state.world_books[idx].created_secs;
                                let wb_count = state.world_books[idx].entry_count;
                                let hover_text =
                                    lang::t("wb_click_detail", language).to_string();
                                if render_world_book_card(
                                    ui,
                                    &wb_name,
                                    &wb_author,
                                    wb_created,
                                    wb_count,
                                    &hover_text,
                                    card_w,
                                    language,
                                    idx,
                                ) {
                                    to_select = Some(idx);
                                }
                            }
                            if let Some(idx) = to_select {
                                state.selected_wb_idx = Some(idx);
                                state.wb_detail_page = 0;
                            }
                        });
                });
        });

    // 详情弹窗
    if let Some(idx) = state.selected_wb_idx {
        if idx < state.world_books.len() {
            let book = state.world_books[idx].clone();
            let close = render_world_book_detail_popup(
                ui.ctx(),
                &book,
                language,
                &mut state.wb_detail_page,
            );
            if close {
                state.selected_wb_idx = None;
                state.wb_detail_page = 0;
            }
        } else {
            state.selected_wb_idx = None;
        }
    }
}

/// 单个世界书卡片渲染，返回是否被点击
fn render_world_book_card(
    ui: &mut egui::Ui,
    name: &str,
    author: &str,
    created_secs: u64,
    entry_count: usize,
    hover_text: &str,
    card_w: f32,
    language: &Language,
    _idx: usize,
) -> bool {
    let card_h = WB_CARD_HEIGHT;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::click());

    // 卡片背景
    let bg = ui.visuals().faint_bg_color;
    ui.painter().rect_filled(rect, 8.0, bg);

    // --- 上部：世界书名称 ---
    let name_rect = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(card_w, WB_CARD_NAME_HEIGHT),
    );

    // 名称区域背景（略深色以区分上下两部分）
    let header_bg = ui.visuals().extreme_bg_color.linear_multiply(0.5);
    ui.painter()
        .rect_filled(name_rect, egui::CornerRadius::same(4), header_bg);

    // 世界书名称
    let name_inner = name_rect.shrink2(egui::vec2(10.0, 4.0));
    let mut name_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(name_inner)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    name_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
    name_ui.set_max_height(WB_CARD_NAME_HEIGHT - 12.0);
    name_ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    name_ui.label(
        egui::RichText::new(name)
            .size(15.0)
            .strong(),
    );

    // --- 下部：信息栏 ---
    let info_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x, rect.min.y + WB_CARD_NAME_HEIGHT),
        egui::vec2(card_w, WB_CARD_INFO_HEIGHT - 8.0),
    );
    let info_inner = info_rect.shrink2(egui::vec2(10.0, 4.0));
    let mut info_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(info_inner)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    info_ui.spacing_mut().item_spacing = egui::vec2(0.0, 3.0);

    // 作者
    if !author.is_empty() {
        info_ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(format!("{}:", lang::t("wb_author", language)))
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
            ui.label(egui::RichText::new(author).size(11.0));
        });
    }

    // 创建时间
    if created_secs > 0 {
        info_ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(format!("{}:", lang::t("wb_created", language)))
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
            ui.label(
                egui::RichText::new(format_timestamp(created_secs))
                    .size(11.0),
            );
        });
    }

    // 条目数
    info_ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(
            egui::RichText::new(
                lang::t("wb_entry_count", language).replace("{n}", &entry_count.to_string()),
            )
            .size(11.0),
        );
    });

    // 悬停遮罩
    if response.hovered() {
        let hover_bg = egui::Color32::from_rgba_premultiplied(0, 0, 0, 160);
        ui.painter().rect_filled(rect, 8.0, hover_bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            hover_text,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    response.clicked()
}

// ============================================================================
// 世界书详情弹窗 — 每页 6 个条目（3 列 × 2 行）
// ============================================================================

fn render_world_book_detail_popup(
    ctx: &egui::Context,
    book: &WorldBookInfo,
    language: &Language,
    detail_page: &mut usize,
) -> bool {
    let mut close = false;

    egui::Window::new(format!(
        "{} - {}",
        lang::t("wb_detail_title", language),
        book.name
    ))
    .collapsible(false)
    .resizable(true)
    .default_size([760.0, 600.0])
    .min_size([500.0, 400.0])
    .show(ctx, |ui| {
        // 上半部分：基本信息
        ui.horizontal(|ui| {
            // 左侧：世界书图标占位
            let icon_size = 80.0;
            let (icon_rect, _response) = ui.allocate_exact_size(
                egui::vec2(icon_size, icon_size),
                egui::Sense::hover(),
            );

            let icon_bg = ui.visuals().faint_bg_color;
            ui.painter().rect_filled(icon_rect, 8.0, icon_bg);

            // 世界书图标
            let icon_center = icon_rect.center();
            ui.painter().text(
                icon_center,
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::BOOK_OPEN,
                egui::FontId::proportional(36.0),
                ui.visuals().text_color(),
            );

            ui.add_space(16.0);

            // 右侧：基本信息
            ui.vertical(|ui| {
                // 世界书名称
                ui.label(egui::RichText::new(&book.name).size(20.0).strong());

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                // 信息行（每行最多 3 项）
                let entry_count_str =
                    lang::t("wb_entry_count", language).replace("{n}", &book.entry_count.to_string());
                let mut items: Vec<(&str, String)> = Vec::new();
                if !book.author.is_empty() {
                    items.push((lang::t("wb_author", language), book.author.clone()));
                }
                let size_str = format_size(book.file_size);
                items.push((lang::t("rm_detail_size", language), size_str));
                if book.created_secs > 0 {
                    items.push((
                        lang::t("wb_created", language),
                        format_timestamp(book.created_secs),
                    ));
                }
                items.push((&entry_count_str, String::new()));

                let items_per_row = 3;
                let mut row: Vec<(&str, String)> = Vec::new();
                for item in items {
                    row.push(item);
                    if row.len() >= items_per_row {
                        render_info_row(ui, &row);
                        row.clear();
                    }
                }
                if !row.is_empty() {
                    render_info_row(ui, &row);
                }
            });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        // 下半部分：条目列表
        if book.entries.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(lang::t("rm_detail_no_worldbook", language))
                        .color(egui::Color32::GRAY)
                        .size(13.0),
                );
            });
        } else {
            let cols: usize = 3;
            let rows_per_page: usize = 2;
            let page_size = cols * rows_per_page; // 每页 6 个条目
            let total_pages = (book.entries.len() + page_size - 1) / page_size;

            // 超出范围时修正页码
            if *detail_page >= total_pages {
                *detail_page = total_pages.saturating_sub(1);
            }

            // 分页组件（靠右）
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 下一页
                    if ui
                        .add_sized(
                            [22.0, 20.0],
                            egui::Button::new(egui::RichText::new("▶").size(12.0)),
                        )
                        .clicked()
                        && *detail_page + 1 < total_pages
                    {
                        *detail_page += 1;
                    }

                    // 页码
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / {}",
                            *detail_page + 1,
                            total_pages
                        ))
                        .size(12.0),
                    );

                    // 上一页
                    if ui
                        .add_sized(
                            [22.0, 20.0],
                            egui::Button::new(egui::RichText::new("◀").size(12.0)),
                        )
                        .clicked()
                        && *detail_page > 0
                    {
                        *detail_page -= 1;
                    }
                });
            });

            ui.add_space(8.0);

            // 计算可用宽度并扣除右侧的边距，以防窗口滑动条挡住卡片
            let available_w = ui.available_width() - 8.0;
            let grid_spacing = 10.0;
            let card_w = ((available_w - grid_spacing * (cols as f32 - 1.0))
                / cols as f32)
                .floor()
                .max(160.0);
            let card_h = 140.0;

            let start = *detail_page * page_size;
            let end = (start + page_size).min(book.entries.len());
            let page_entries: Vec<&WorldEntry> = book.entries[start..end].iter().collect();

            // 3 列 × 2 行网格
            egui::Grid::new("wb_detail_entries_grid")
                .spacing([grid_spacing, grid_spacing])
                .min_col_width(card_w)
                .max_col_width(card_w)
                .show(ui, |ui| {
                    for (col, entry) in page_entries.iter().enumerate() {
                        if col > 0 && col % cols == 0 {
                            ui.end_row();
                        }
                        ui.push_id(start + col, |ui| {
                            render_world_entry_card(
                                ui, entry, language, card_w, card_h,
                            );
                        });
                    }
                });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(lang::t("wb_scroll_entries", language))
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
        }

        // 关闭按钮
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if ui.button(lang::t("rm_close", language)).clicked() {
                close = true;
            }
        });
    });

    close
}

// ============================================================================
// 聊天记录管理 Tab (占位)
// ============================================================================

fn render_chat_history(ui: &mut egui::Ui, language: &Language) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(lang::t("rm_count", language).replace("{n}", "0"))
                .size(13.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_sized(
                    [70.0, 24.0],
                    egui::Button::new(
                        egui::RichText::new(lang::t("rm_refresh", language)).size(12.0),
                    ),
                )
                .clicked() {}
        });
    });
    ui.separator();
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(lang::t("rm_empty_chats", language))
                .color(egui::Color32::GRAY)
                .size(13.0),
        );
    });
}
