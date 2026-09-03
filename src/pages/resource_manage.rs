//! SillyTavern 资源管理页面。
//!
//! 合并旧版 Web 页面的管理操作与原生 v2 页面中的文件解析能力，统一管理
//! 角色卡、世界书、历史对话和预设。所有操作都直接作用于当前数据目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use iced::widget::{button, column, container, image, row, scrollable, space, text_input, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Fill, Length, Theme};
use lucide_icons::Icon;

use astra_ui::{
    BLUE_600, ButtonVariant, DANGER, INK, INK_MUTED, INK_SUBTLE, SUCCESS, WARNING, WHITE, fonts,
    icons,
};

use super::settings::{SettingsState, TavernDataMode};
use super::versions::VersionState;
use crate::lang::text;
use crate::theme::button_style;

const LIST_WIDTH: f32 = 390.0;
const CHARACTER_THUMB_WIDTH: f32 = 52.0;
const CHARACTER_THUMB_HEIGHT: f32 = 70.0;
const CHAT_MESSAGE_LIMIT: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourceTab {
    #[default]
    Characters,
    WorldBooks,
    Chats,
    Presets,
}

impl ResourceTab {
    const ALL: [Self; 4] = [
        Self::Characters,
        Self::WorldBooks,
        Self::Chats,
        Self::Presets,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Characters => "角色卡",
            Self::WorldBooks => "世界书",
            Self::Chats => "历史对话",
            Self::Presets => "预设",
        }
    }

    const fn singular(self) -> &'static str {
        match self {
            Self::Characters => "角色卡",
            Self::WorldBooks => "世界书",
            Self::Chats => "对话记录",
            Self::Presets => "预设",
        }
    }

    const fn icon(self) -> Icon {
        match self {
            Self::Characters => Icon::ContactRound,
            Self::WorldBooks => Icon::BookOpenText,
            Self::Chats => Icon::MessagesSquare,
            Self::Presets => Icon::SlidersHorizontal,
        }
    }

    const fn directory(self) -> &'static str {
        match self {
            Self::Characters => "characters",
            Self::WorldBooks => "worlds",
            Self::Chats => "chats",
            Self::Presets => "OpenAI Settings",
        }
    }

    const fn supports_import(self) -> bool {
        !matches!(self, Self::Chats)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorldEntry {
    pub keys: Vec<String>,
    pub content: String,
    pub comment: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
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
    pub spec: String,
    pub spec_version: String,
    pub world_name: String,
    pub world_entries: Vec<WorldEntry>,
    pub file_size: u64,
    pub modified_secs: u64,
    pub image_width: u32,
    pub image_height: u32,
}

#[derive(Debug, Clone)]
pub struct WorldBookInfo {
    pub filename: String,
    pub filepath: PathBuf,
    pub name: String,
    pub author: String,
    pub entries: Vec<WorldEntry>,
    pub file_size: u64,
    pub modified_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ChatFileInfo {
    pub filename: String,
    pub filepath: PathBuf,
    pub display_time: String,
    pub file_size: u64,
    pub modified_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ChatGroup {
    pub name: String,
    pub files: Vec<ChatFileInfo>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub name: String,
    pub is_user: bool,
    pub send_date: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct PresetPrompt {
    pub name: String,
    pub role: String,
    pub content: String,
    pub enabled: bool,
    pub marker: bool,
}

#[derive(Debug, Clone)]
pub struct PresetInfo {
    pub filename: String,
    pub filepath: PathBuf,
    pub name: String,
    pub source: String,
    pub model: String,
    pub max_context: i64,
    pub max_tokens: i64,
    pub stream: bool,
    pub prompts: Vec<PresetPrompt>,
    pub has_spreset: bool,
    pub file_size: u64,
    pub modified_secs: u64,
}

#[derive(Debug, Clone)]
struct PendingDelete {
    path: PathBuf,
    label: String,
}

#[derive(Debug, Clone)]
pub enum ResourceManageMessage {
    SelectTab(ResourceTab),
    SearchChanged(String),
    Refresh,
    OpenDirectory,
    Import,
    SelectCharacter(usize),
    SelectWorldBook(usize),
    SelectChat(usize, usize),
    SelectPreset(usize),
    RequestDelete,
    ConfirmDelete,
    CancelDelete,
    ClearNotice,
}

#[derive(Debug, Clone)]
pub struct ResourceManageState {
    pub tab: ResourceTab,
    pub search: String,
    data_root: Option<PathBuf>,
    source_label: String,
    context_key: String,
    pub characters: Vec<CharacterCardInfo>,
    pub world_books: Vec<WorldBookInfo>,
    pub chat_groups: Vec<ChatGroup>,
    pub presets: Vec<PresetInfo>,
    pub selected_character: Option<usize>,
    pub selected_world_book: Option<usize>,
    pub selected_chat: Option<(usize, usize)>,
    pub selected_preset: Option<usize>,
    pub chat_messages: Vec<ChatMessage>,
    pub notice: Option<String>,
    pending_delete: Option<PendingDelete>,
}

impl Default for ResourceManageState {
    fn default() -> Self {
        Self {
            tab: ResourceTab::Characters,
            search: String::new(),
            data_root: None,
            source_label: "尚未选择 SillyTavern 实例".into(),
            context_key: String::new(),
            characters: Vec::new(),
            world_books: Vec::new(),
            chat_groups: Vec::new(),
            presets: Vec::new(),
            selected_character: None,
            selected_world_book: None,
            selected_chat: None,
            selected_preset: None,
            chat_messages: Vec::new(),
            notice: None,
            pending_delete: None,
        }
    }
}

impl ResourceManageState {
    pub fn configure(&mut self, settings: &SettingsState, versions: &VersionState) {
        let (root, label) = match settings.data_mode {
            TavernDataMode::Global => {
                let path = expand_home(&settings.global_data_path).join("default-user");
                (Some(path), "全局数据".to_owned())
            }
            TavernDataMode::Current => {
                let instance = versions.current_path.as_deref().or_else(|| {
                    versions
                        .local_instances
                        .first()
                        .map(|item| item.path.as_str())
                });
                match instance {
                    Some(path) => (
                        Some(PathBuf::from(path).join("data").join("default-user")),
                        format!("独立数据 · {}", compact_path(path)),
                    ),
                    None => (None, "尚未选择 SillyTavern 实例".to_owned()),
                }
            }
        };

        let key = root
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        if key != self.context_key {
            self.context_key = key;
            self.data_root = root;
            self.source_label = label;
            self.clear_loaded_data();
        } else {
            self.source_label = label;
        }
    }

    pub fn update(&mut self, message: ResourceManageMessage) {
        match message {
            ResourceManageMessage::SelectTab(tab) => {
                self.tab = tab;
                self.search.clear();
                self.pending_delete = None;
            }
            ResourceManageMessage::SearchChanged(value) => self.search = value,
            ResourceManageMessage::Refresh => {
                self.refresh_all();
                self.notice = Some("资源目录已重新扫描。".into());
            }
            ResourceManageMessage::OpenDirectory => self.open_current_directory(),
            ResourceManageMessage::Import => self.import_current_resource(),
            ResourceManageMessage::SelectCharacter(index) => {
                self.selected_character = Some(index);
                self.pending_delete = None;
            }
            ResourceManageMessage::SelectWorldBook(index) => {
                self.selected_world_book = Some(index);
                self.pending_delete = None;
            }
            ResourceManageMessage::SelectChat(group_index, file_index) => {
                self.selected_chat = Some((group_index, file_index));
                self.pending_delete = None;
                self.chat_messages = self
                    .chat_groups
                    .get(group_index)
                    .and_then(|group| group.files.get(file_index))
                    .map(|file| load_chat_messages(&file.filepath))
                    .unwrap_or_default();
            }
            ResourceManageMessage::SelectPreset(index) => {
                self.selected_preset = Some(index);
                self.pending_delete = None;
            }
            ResourceManageMessage::RequestDelete => self.request_delete(),
            ResourceManageMessage::ConfirmDelete => self.confirm_delete(),
            ResourceManageMessage::CancelDelete => self.pending_delete = None,
            ResourceManageMessage::ClearNotice => self.notice = None,
        }
    }

    pub fn refresh_all(&mut self) {
        self.characters = self.scan_characters();
        self.world_books = self.scan_world_books();
        self.chat_groups = self.scan_chats();
        self.presets = self.scan_presets();
        self.repair_selections();
    }

    fn clear_loaded_data(&mut self) {
        self.characters.clear();
        self.world_books.clear();
        self.chat_groups.clear();
        self.presets.clear();
        self.selected_character = None;
        self.selected_world_book = None;
        self.selected_chat = None;
        self.selected_preset = None;
        self.chat_messages.clear();
        self.pending_delete = None;
    }

    fn repair_selections(&mut self) {
        if self
            .selected_character
            .is_some_and(|i| i >= self.characters.len())
        {
            self.selected_character = None;
        }
        if self
            .selected_world_book
            .is_some_and(|i| i >= self.world_books.len())
        {
            self.selected_world_book = None;
        }
        if self
            .selected_preset
            .is_some_and(|i| i >= self.presets.len())
        {
            self.selected_preset = None;
        }
        if let Some((group, file)) = self.selected_chat {
            if self
                .chat_groups
                .get(group)
                .is_none_or(|item| file >= item.files.len())
            {
                self.selected_chat = None;
                self.chat_messages.clear();
            }
        }
    }

    fn directory_for(&self, tab: ResourceTab) -> Option<PathBuf> {
        self.data_root
            .as_ref()
            .map(|root| root.join(tab.directory()))
    }

    fn open_current_directory(&mut self) {
        let Some(directory) = self.directory_for(self.tab) else {
            self.notice = Some("请先在版本管理中选择一个 SillyTavern 实例。".into());
            return;
        };
        if let Err(error) = fs::create_dir_all(&directory) {
            self.notice = Some(format!("无法创建资源目录：{error}"));
            return;
        }
        match Command::new("open").arg(&directory).spawn() {
            Ok(_) => self.notice = Some(format!("已打开 {} 目录。", self.tab.label())),
            Err(error) => self.notice = Some(format!("无法打开资源目录：{error}")),
        }
    }

    fn import_current_resource(&mut self) {
        if !self.tab.supports_import() {
            self.notice = Some("历史对话请通过资源迁移或直接放入角色对应目录。".into());
            return;
        }
        let Some(directory) = self.directory_for(self.tab) else {
            self.notice = Some("请先在版本管理中选择一个 SillyTavern 实例。".into());
            return;
        };
        let dialog = match self.tab {
            ResourceTab::Characters => rfd::FileDialog::new().add_filter("角色卡 PNG", &["png"]),
            ResourceTab::WorldBooks | ResourceTab::Presets => {
                rfd::FileDialog::new().add_filter("JSON 文件", &["json"])
            }
            ResourceTab::Chats => return,
        };
        let Some(files) = dialog.pick_files() else {
            return;
        };
        if let Err(error) = fs::create_dir_all(&directory) {
            self.notice = Some(format!("无法创建资源目录：{error}"));
            return;
        }

        let mut imported = 0usize;
        let mut failed = Vec::new();
        for source in files {
            let Some(filename) = source.file_name() else {
                continue;
            };
            let destination = directory.join(filename);
            match fs::copy(&source, destination) {
                Ok(_) => imported += 1,
                Err(error) => failed.push(error.to_string()),
            }
        }
        self.refresh_all();
        self.notice = if failed.is_empty() {
            Some(format!("已导入 {imported} 个{}。", self.tab.singular()))
        } else {
            Some(format!(
                "已导入 {imported} 个文件，{} 个失败。",
                failed.len()
            ))
        };
    }

    fn selected_path_and_label(&self) -> Option<(PathBuf, String)> {
        match self.tab {
            ResourceTab::Characters => self
                .selected_character
                .and_then(|index| self.characters.get(index))
                .map(|item| (item.filepath.clone(), item.name.clone())),
            ResourceTab::WorldBooks => self
                .selected_world_book
                .and_then(|index| self.world_books.get(index))
                .map(|item| (item.filepath.clone(), item.name.clone())),
            ResourceTab::Chats => self
                .selected_chat
                .and_then(|(group, file)| {
                    self.chat_groups
                        .get(group)
                        .and_then(|item| item.files.get(file))
                })
                .map(|item| (item.filepath.clone(), item.filename.clone())),
            ResourceTab::Presets => self
                .selected_preset
                .and_then(|index| self.presets.get(index))
                .map(|item| (item.filepath.clone(), item.name.clone())),
        }
    }

    fn request_delete(&mut self) {
        self.pending_delete = self
            .selected_path_and_label()
            .map(|(path, label)| PendingDelete { path, label });
    }

    fn confirm_delete(&mut self) {
        let Some(pending) = self.pending_delete.take() else {
            return;
        };
        match fs::remove_file(&pending.path) {
            Ok(()) => {
                self.notice = Some(format!("已删除“{}”。", pending.label));
                self.refresh_all();
            }
            Err(error) => self.notice = Some(format!("删除失败：{error}")),
        }
    }

    fn scan_characters(&self) -> Vec<CharacterCardInfo> {
        let Some(directory) = self.directory_for(ResourceTab::Characters) else {
            return Vec::new();
        };
        let mut items = read_files(&directory, "png")
            .into_iter()
            .filter_map(|path| {
                let metadata = fs::metadata(&path).ok()?;
                let parsed = parse_character_png(&path);
                let filename = file_name(&path);
                let name = non_empty(parsed.name, file_stem(&path));
                let (image_width, image_height) = fs::read(&path)
                    .ok()
                    .and_then(|data| read_png_dimensions(&data))
                    .unwrap_or((0, 0));
                Some(CharacterCardInfo {
                    filename,
                    filepath: path,
                    name,
                    description: parsed.description,
                    creator: parsed.creator,
                    version: parsed.version,
                    tags: parsed.tags,
                    personality: parsed.personality,
                    scenario: parsed.scenario,
                    first_message: parsed.first_message,
                    spec: parsed.spec,
                    spec_version: parsed.spec_version,
                    world_name: parsed.world_name,
                    world_entries: parsed.world_entries,
                    file_size: metadata.len(),
                    modified_secs: modified_secs(&metadata),
                    image_width,
                    image_height,
                })
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
        items
    }

    fn scan_world_books(&self) -> Vec<WorldBookInfo> {
        let Some(directory) = self.directory_for(ResourceTab::WorldBooks) else {
            return Vec::new();
        };
        let mut items = read_files(&directory, "json")
            .into_iter()
            .filter_map(|path| {
                let metadata = fs::metadata(&path).ok()?;
                let value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
                let name = value
                    .get("name")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| file_stem(&path));
                let author = value
                    .get("author")
                    .or_else(|| value.get("creator"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned();
                Some(WorldBookInfo {
                    filename: file_name(&path),
                    filepath: path,
                    name,
                    author,
                    entries: parse_world_entries(&value),
                    file_size: metadata.len(),
                    modified_secs: modified_secs(&metadata),
                })
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
        items
    }

    fn scan_chats(&self) -> Vec<ChatGroup> {
        let Some(directory) = self.directory_for(ResourceTab::Chats) else {
            return Vec::new();
        };
        let Ok(entries) = fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut groups = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let path = entry.path();
                let mut files = read_files(&path, "jsonl")
                    .into_iter()
                    .filter_map(|path| {
                        let metadata = fs::metadata(&path).ok()?;
                        let filename = file_name(&path);
                        Some(ChatFileInfo {
                            display_time: chat_display_time(&filename),
                            filename,
                            filepath: path,
                            file_size: metadata.len(),
                            modified_secs: modified_secs(&metadata),
                        })
                    })
                    .collect::<Vec<_>>();
                files.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
                (!files.is_empty()).then(|| ChatGroup {
                    name: file_name(&path),
                    files,
                })
            })
            .collect::<Vec<_>>();
        groups.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        groups
    }

    fn scan_presets(&self) -> Vec<PresetInfo> {
        let Some(directory) = self.directory_for(ResourceTab::Presets) else {
            return Vec::new();
        };
        let mut items = read_files(&directory, "json")
            .into_iter()
            .filter_map(|path| {
                let metadata = fs::metadata(&path).ok()?;
                let value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
                let source = string_value(&value, "chat_completion_source");
                let model = if source.to_lowercase().contains("claude") {
                    string_value(&value, "claude_model")
                } else {
                    non_empty(
                        string_value(&value, "openai_model"),
                        string_value(&value, "claude_model"),
                    )
                };
                let prompts = value
                    .get("prompts")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .map(|prompt| PresetPrompt {
                                name: string_value(prompt, "name"),
                                role: string_value(prompt, "role"),
                                content: string_value(prompt, "content"),
                                enabled: prompt
                                    .get("enabled")
                                    .and_then(|value| value.as_bool())
                                    .unwrap_or(true),
                                marker: prompt
                                    .get("marker")
                                    .and_then(|value| value.as_bool())
                                    .unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let has_spreset = value
                    .get("extensions")
                    .and_then(|value| value.as_object())
                    .is_some_and(|extensions| extensions.contains_key("SPreset"));
                Some(PresetInfo {
                    filename: file_name(&path),
                    filepath: path.clone(),
                    name: file_stem(&path),
                    source,
                    model,
                    max_context: value
                        .get("openai_max_context")
                        .and_then(|value| value.as_i64())
                        .unwrap_or_default(),
                    max_tokens: value
                        .get("openai_max_tokens")
                        .and_then(|value| value.as_i64())
                        .unwrap_or_default(),
                    stream: value
                        .get("stream_openai")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    prompts,
                    has_spreset,
                    file_size: metadata.len(),
                    modified_secs: modified_secs(&metadata),
                })
            })
            .collect::<Vec<_>>();
        items.sort_by(|a, b| b.modified_secs.cmp(&a.modified_secs));
        items
    }

    fn resource_count(&self, tab: ResourceTab) -> usize {
        match tab {
            ResourceTab::Characters => self.characters.len(),
            ResourceTab::WorldBooks => self.world_books.len(),
            ResourceTab::Chats => self.chat_groups.iter().map(|group| group.files.len()).sum(),
            ResourceTab::Presets => self.presets.len(),
        }
    }

    fn current_count(&self) -> usize {
        self.resource_count(self.tab)
    }

    fn current_directory(&self) -> String {
        self.directory_for(self.tab)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| "未配置数据目录".into())
    }
}

#[derive(Default)]
struct ParsedCharacter {
    name: String,
    description: String,
    creator: String,
    version: String,
    tags: Vec<String>,
    personality: String,
    scenario: String,
    first_message: String,
    spec: String,
    spec_version: String,
    world_name: String,
    world_entries: Vec<WorldEntry>,
}

fn parse_character_png(path: &Path) -> ParsedCharacter {
    let Ok(data) = fs::read(path) else {
        return ParsedCharacter::default();
    };
    if data.len() < 8 || data[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return ParsedCharacter::default();
    }

    let mut text_chunks = Vec::new();
    let mut position = 8usize;
    while position + 12 <= data.len() {
        let length = u32::from_be_bytes([
            data[position],
            data[position + 1],
            data[position + 2],
            data[position + 3],
        ]) as usize;
        if position + 12 + length > data.len() {
            break;
        }
        let kind = &data[position + 4..position + 8];
        if kind == b"tEXt" {
            let payload = &data[position + 8..position + 8 + length];
            if let Some(separator) = payload.iter().position(|byte| *byte == 0) {
                text_chunks.push((
                    String::from_utf8_lossy(&payload[..separator]).to_string(),
                    String::from_utf8_lossy(&payload[separator + 1..]).to_string(),
                ));
            }
        }
        position += length + 12;
        if kind == b"IEND" {
            break;
        }
    }

    let encoded = ["chara", "ccv3"]
        .into_iter()
        .find_map(|wanted| {
            text_chunks
                .iter()
                .find(|(key, _)| key.to_lowercase().contains(wanted))
                .map(|(_, value)| value.as_str())
        })
        .or_else(|| text_chunks.first().map(|(_, value)| value.as_str()));
    let Some(encoded) = encoded else {
        return ParsedCharacter::default();
    };
    let value = decode_base64(encoded)
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .or_else(|| serde_json::from_str::<serde_json::Value>(encoded).ok());
    let Some(value) = value else {
        return ParsedCharacter::default();
    };
    let data = value.get("data").unwrap_or(&value);
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| data.get(*key).and_then(|value| value.as_str()))
            .unwrap_or_default()
            .to_owned()
    };
    let tags = data
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let world = data
        .get("character_book")
        .or_else(|| value.get("character_book"));
    ParsedCharacter {
        name: pick(&["name"]),
        description: pick(&["description"]),
        creator: pick(&["creator"]),
        version: pick(&["character_version", "version"]),
        tags,
        personality: pick(&["personality"]),
        scenario: pick(&["scenario"]),
        first_message: pick(&["first_mes", "firstMessage"]),
        spec: string_value(&value, "spec"),
        spec_version: string_value(&value, "spec_version"),
        world_name: world
            .map(|value| string_value(value, "name"))
            .unwrap_or_default(),
        world_entries: world.map(parse_world_entries).unwrap_or_default(),
    }
}

fn parse_world_entries(value: &serde_json::Value) -> Vec<WorldEntry> {
    let entries = value.get("entries").unwrap_or(value);
    match entries {
        serde_json::Value::Array(items) => items.iter().filter_map(parse_world_entry).collect(),
        serde_json::Value::Object(items) => items.values().filter_map(parse_world_entry).collect(),
        _ => Vec::new(),
    }
}

fn parse_world_entry(value: &serde_json::Value) -> Option<WorldEntry> {
    let object = value.as_object()?;
    let keys = object
        .get("keys")
        .or_else(|| object.get("key"))
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    Some(WorldEntry {
        keys,
        content: object
            .get("content")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned(),
        comment: object
            .get("comment")
            .or_else(|| object.get("name"))
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_owned(),
        enabled: object
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or_else(|| {
                !object
                    .get("disable")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
            }),
    })
}

fn load_chat_messages(path: &Path) -> Vec<ChatMessage> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut messages = content
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| {
            !value
                .get("is_system")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        })
        .filter_map(|value| {
            let content = value
                .get("mes")
                .or_else(|| value.get("message"))
                .and_then(|value| value.as_str())?
                .to_owned();
            Some(ChatMessage {
                name: string_value(&value, "name"),
                is_user: value
                    .get("is_user")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                send_date: string_value(&value, "send_date"),
                content,
            })
        })
        .collect::<Vec<_>>();
    if messages.len() > CHAT_MESSAGE_LIMIT {
        messages.drain(..messages.len() - CHAT_MESSAGE_LIMIT);
    }
    messages
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(output)
}

fn read_png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 24 || data[..8] != [137, 80, 78, 71, 13, 10, 26, 10] {
        return None;
    }
    Some((
        u32::from_be_bytes([data[16], data[17], data[18], data[19]]),
        u32::from_be_bytes([data[20], data[21], data[22], data[23]]),
    ))
}

fn read_files(directory: &Path, extension: &str) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        })
        .collect()
}

fn modified_secs(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn string_value(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn non_empty(value: String, fallback: String) -> String {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(rest);
    }
    PathBuf::from(path)
}

fn compact_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn chat_display_time(filename: &str) -> String {
    filename
        .strip_suffix(".jsonl")
        .unwrap_or(filename)
        .split_once(" - ")
        .map(|(_, value)| value.to_owned())
        .unwrap_or_else(|| filename.trim_end_matches(".jsonl").to_owned())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn truncate(value: &str, length: usize) -> String {
    let mut chars = value.chars();
    let preview = chars.by_ref().take(length).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

pub fn resource_manage_view(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let header = row![
        column![
            row![
                container(icons::icon(Icon::LibraryBig, 19, BLUE_600))
                    .width(36)
                    .height(36)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .style(page_icon_surface),
                column![
                    text("资源管理")
                        .size(20)
                        .font(fonts::MEDIUM)
                        .style(crate::theme::text_style),
                    text("统一查看与整理 SillyTavern 本地资源")
                        .size(11)
                        .font(fonts::REGULAR)
                        .style(crate::theme::muted_text_style),
                ]
                .spacing(3),
            ]
            .spacing(11)
            .align_y(Alignment::Center),
        ],
        space::horizontal(),
        container(
            row![
                icons::icon(
                    if state.data_root.is_some() {
                        Icon::Database
                    } else {
                        Icon::CircleAlert
                    },
                    14,
                    if state.data_root.is_some() {
                        SUCCESS
                    } else {
                        WARNING
                    },
                ),
                column![
                    text(&state.source_label)
                        .size(11)
                        .font(fonts::MEDIUM)
                        .style(crate::theme::text_style),
                    text(state.current_directory())
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::subtle_text_style),
                ]
                .spacing(2),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .max_width(390)
        .padding([8, 12])
        .style(source_surface),
    ]
    .align_y(Alignment::Center);

    let tabs = row(ResourceTab::ALL
        .into_iter()
        .map(|tab| resource_tab(state, tab)))
    .spacing(3)
    .width(Fill)
    .align_y(Alignment::End);

    let import_button: Element<'_, ResourceManageMessage> = if state.tab.supports_import() {
        button(
            row![
                icons::icon(Icon::FileUp, 14, WHITE),
                text(format!("导入{}", state.tab.singular()))
                    .size(11)
                    .font(fonts::MEDIUM),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(ResourceManageMessage::Import)
        .padding([8, 12])
        .style(button_style(ButtonVariant::Primary))
        .into()
    } else {
        space::horizontal().width(Length::Shrink).into()
    };

    let toolbar = row![
        container(
            row![
                crate::theme::subtle_icon(Icon::Search, 14),
                text_input("搜索名称、文件名或标签", &state.search)
                    .on_input(ResourceManageMessage::SearchChanged)
                    .padding([7, 2])
                    .size(11)
                    .font(fonts::REGULAR),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        )
        .width(300)
        .padding([0, 10])
        .style(search_surface),
        container(
            row![
                icons::icon(state.tab.icon(), 13, BLUE_600),
                text(format!("{} 项", state.current_count()))
                    .size(10)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([7, 10])
        .style(count_surface),
        space::horizontal(),
        tooltip(
            button(crate::theme::muted_icon(Icon::FolderOpen, 15))
                .on_press(ResourceManageMessage::OpenDirectory)
                .width(34)
                .height(34)
                .style(button_style(ButtonVariant::Outline)),
            container(text("打开目录").size(10))
                .padding([5, 8])
                .style(tooltip_surface),
            tooltip::Position::Bottom,
        ),
        tooltip(
            button(crate::theme::muted_icon(Icon::RefreshCw, 15))
                .on_press(ResourceManageMessage::Refresh)
                .width(34)
                .height(34)
                .style(button_style(ButtonVariant::Outline)),
            container(text("重新扫描").size(10))
                .padding([5, 8])
                .style(tooltip_surface),
            tooltip::Position::Bottom,
        ),
        import_button,
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let feedback = if let Some(pending) = &state.pending_delete {
        container(
            row![
                icons::icon(Icon::TriangleAlert, 15, DANGER),
                text(format!("确定删除“{}”吗？此操作无法撤销。", pending.label))
                    .size(11)
                    .font(fonts::REGULAR)
                    .style(crate::theme::text_style),
                space::horizontal(),
                button(text("取消").size(10).font(fonts::MEDIUM))
                    .on_press(ResourceManageMessage::CancelDelete)
                    .padding([6, 10])
                    .style(button_style(ButtonVariant::Outline)),
                button(text("确认删除").size(10).font(fonts::MEDIUM))
                    .on_press(ResourceManageMessage::ConfirmDelete)
                    .padding([6, 10])
                    .style(danger_button_style),
            ]
            .spacing(9)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(delete_notice_surface)
    } else if let Some(notice) = &state.notice {
        container(
            row![
                icons::icon(Icon::Info, 14, BLUE_600),
                text(notice)
                    .size(10)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style),
                space::horizontal(),
                button(crate::theme::subtle_icon(Icon::X, 13))
                    .on_press(ResourceManageMessage::ClearNotice)
                    .padding(4)
                    .style(quiet_button_style),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([7, 11])
        .style(notice_surface)
    } else {
        container(space::vertical()).height(0)
    };

    let body = if state.data_root.is_none() {
        empty_page(
            Icon::FolderCog,
            "尚未连接资源目录",
            "请先在版本管理中切换到一个本地 SillyTavern 实例，或在设置中启用全局数据。",
        )
    } else {
        row![resource_list(state), resource_detail(state)]
            .spacing(14)
            .height(Fill)
            .into()
    };

    container(
        container(
            column![header, tabs, toolbar, feedback, body]
                .spacing(12)
                .width(Fill)
                .height(Fill),
        )
        .max_width(1120)
        .width(Fill)
        .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .padding([22, 28])
    .align_x(Alignment::Center)
    .style(crate::theme::canvas_style)
    .into()
}

fn resource_tab(
    state: &ResourceManageState,
    tab: ResourceTab,
) -> Element<'static, ResourceManageMessage> {
    let active = state.tab == tab;
    let color = if active { BLUE_600 } else { INK_MUTED };
    button(
        column![
            container(
                row![
                    icons::icon(tab.icon(), 14, color),
                    text(tab.label()).size(12).font(fonts::MEDIUM).color(color),
                    container(
                        text(state.resource_count(tab).to_string())
                            .size(9)
                            .font(fonts::MEDIUM)
                            .color(color),
                    )
                    .padding([2, 6])
                    .style(move |_theme| tab_count_surface(active)),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .height(32)
            .align_y(Alignment::Center),
            container(space::vertical())
                .height(2)
                .width(Fill)
                .style(move |_theme| tab_indicator(active)),
        ]
        .align_x(Alignment::Center),
    )
    .on_press(ResourceManageMessage::SelectTab(tab))
    .padding([0, 10])
    .style(tab_button_style)
    .into()
}

fn resource_list(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let content = match state.tab {
        ResourceTab::Characters => character_list(state),
        ResourceTab::WorldBooks => world_book_list(state),
        ResourceTab::Chats => chat_list(state),
        ResourceTab::Presets => preset_list(state),
    };
    container(
        column![
            container(
                row![
                    text(format!("{}列表", state.tab.label()))
                        .size(12)
                        .font(fonts::MEDIUM)
                        .style(crate::theme::text_style),
                    space::horizontal(),
                    text("按修改时间排序")
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::subtle_text_style),
                ]
                .align_y(Alignment::Center),
            )
            .padding([11, 13])
            .style(panel_header_surface),
            content,
        ]
        .spacing(0)
        .height(Fill),
    )
    .width(LIST_WIDTH)
    .height(Fill)
    .style(panel_surface)
    .into()
}

fn character_list(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let query = state.search.to_lowercase();
    let rows = state
        .characters
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            query.is_empty()
                || item.name.to_lowercase().contains(&query)
                || item.filename.to_lowercase().contains(&query)
                || item
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query))
        })
        .map(|(index, item)| {
            let selected = state.selected_character == Some(index);
            button(
                row![
                    container(
                        image(item.filepath.clone())
                            .width(CHARACTER_THUMB_WIDTH)
                            .height(CHARACTER_THUMB_HEIGHT)
                            .content_fit(iced::ContentFit::Cover),
                    )
                    .width(CHARACTER_THUMB_WIDTH)
                    .height(CHARACTER_THUMB_HEIGHT)
                    .style(thumbnail_surface),
                    column![
                        text(&item.name)
                            .size(12)
                            .font(fonts::MEDIUM)
                            .style(crate::theme::text_style),
                        text(if item.creator.is_empty() {
                            "未知作者".to_owned()
                        } else {
                            format!("作者：{}", item.creator)
                        })
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::muted_text_style),
                        text(format!(
                            "{} · {}×{} · {} 个标签",
                            format_size(item.file_size),
                            item.image_width,
                            item.image_height,
                            item.tags.len()
                        ))
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::subtle_text_style),
                    ]
                    .spacing(5),
                    space::horizontal(),
                    icons::icon(
                        Icon::ChevronRight,
                        14,
                        if selected { BLUE_600 } else { INK_SUBTLE }
                    ),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .on_press(ResourceManageMessage::SelectCharacter(index))
            .width(Fill)
            .padding([9, 11])
            .style(move |theme, status| list_item_style(theme, selected, status))
            .into()
        })
        .collect::<Vec<Element<'_, ResourceManageMessage>>>();
    list_scroll(rows, "没有找到角色卡", "导入 PNG 角色卡后会显示在这里。")
}

fn world_book_list(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let query = state.search.to_lowercase();
    let rows = state
        .world_books
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            query.is_empty()
                || item.name.to_lowercase().contains(&query)
                || item.filename.to_lowercase().contains(&query)
                || item.author.to_lowercase().contains(&query)
        })
        .map(|(index, item)| {
            simple_list_item(
                Icon::BookMarked,
                &item.name,
                if item.author.is_empty() {
                    "未知作者"
                } else {
                    &item.author
                },
                format!(
                    "{} 条目 · {}",
                    item.entries.len(),
                    format_size(item.file_size)
                ),
                state.selected_world_book == Some(index),
                ResourceManageMessage::SelectWorldBook(index),
            )
        })
        .collect::<Vec<_>>();
    list_scroll(rows, "没有找到世界书", "导入 JSON 世界书后会显示在这里。")
}

fn chat_list(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let query = state.search.to_lowercase();
    let mut rows = Vec::new();
    for (group_index, group) in state.chat_groups.iter().enumerate() {
        let matching = group
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| {
                query.is_empty()
                    || group.name.to_lowercase().contains(&query)
                    || file.filename.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        rows.push(
            container(
                row![
                    icons::icon(Icon::UserRound, 13, BLUE_600),
                    text(&group.name)
                        .size(10)
                        .font(fonts::MEDIUM)
                        .style(crate::theme::text_style),
                    space::horizontal(),
                    text(format!("{} 个会话", matching.len()))
                        .size(9)
                        .font(fonts::REGULAR)
                        .style(crate::theme::subtle_text_style),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
            )
            .padding([8, 11])
            .style(group_header_surface)
            .into(),
        );
        for (file_index, file) in matching {
            rows.push(simple_list_item(
                Icon::MessageCircle,
                &file.display_time,
                &file.filename,
                format_size(file.file_size),
                state.selected_chat == Some((group_index, file_index)),
                ResourceManageMessage::SelectChat(group_index, file_index),
            ));
        }
    }
    list_scroll(
        rows,
        "没有找到历史对话",
        "开始聊天后，JSONL 对话记录会按角色分组显示。",
    )
}

fn preset_list(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let query = state.search.to_lowercase();
    let rows = state
        .presets
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            query.is_empty()
                || item.name.to_lowercase().contains(&query)
                || item.filename.to_lowercase().contains(&query)
                || item.model.to_lowercase().contains(&query)
        })
        .map(|(index, item)| {
            simple_list_item(
                Icon::ListChecks,
                &item.name,
                if item.model.is_empty() {
                    "未指定模型"
                } else {
                    &item.model
                },
                format!(
                    "{} 段提示词 · {}",
                    item.prompts.len(),
                    format_size(item.file_size)
                ),
                state.selected_preset == Some(index),
                ResourceManageMessage::SelectPreset(index),
            )
        })
        .collect::<Vec<_>>();
    list_scroll(rows, "没有找到预设", "导入 JSON 预设后会显示在这里。")
}

fn simple_list_item<'a>(
    icon: Icon,
    title: &'a str,
    subtitle: &'a str,
    meta: String,
    selected: bool,
    message: ResourceManageMessage,
) -> Element<'a, ResourceManageMessage> {
    button(
        row![
            container(icons::icon(
                icon,
                17,
                if selected { BLUE_600 } else { INK_MUTED }
            ))
            .width(36)
            .height(36)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_theme| item_icon_surface(selected)),
            column![
                text(title)
                    .size(11)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
                text(truncate(subtitle, 35))
                    .size(9)
                    .font(fonts::REGULAR)
                    .style(crate::theme::muted_text_style),
                text(meta)
                    .size(9)
                    .font(fonts::REGULAR)
                    .style(crate::theme::subtle_text_style),
            ]
            .spacing(3),
            space::horizontal(),
            icons::icon(
                Icon::ChevronRight,
                14,
                if selected { BLUE_600 } else { INK_SUBTLE }
            ),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .on_press(message)
    .width(Fill)
    .padding([9, 11])
    .style(move |theme, status| list_item_style(theme, selected, status))
    .into()
}

fn list_scroll<'a>(
    rows: Vec<Element<'a, ResourceManageMessage>>,
    title: &'static str,
    description: &'static str,
) -> Element<'a, ResourceManageMessage> {
    if rows.is_empty() {
        return container(
            column![
                crate::theme::subtle_icon(Icon::Inbox, 25),
                text(title)
                    .size(11)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::muted_text_style),
                text(description)
                    .size(9)
                    .font(fonts::REGULAR)
                    .style(crate::theme::subtle_text_style),
            ]
            .spacing(7)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into();
    }
    scrollable(column(rows).spacing(2).padding(6))
        .height(Fill)
        .into()
}

fn resource_detail(state: &ResourceManageState) -> Element<'_, ResourceManageMessage> {
    let content = match state.tab {
        ResourceTab::Characters => state
            .selected_character
            .and_then(|index| state.characters.get(index))
            .map(character_detail),
        ResourceTab::WorldBooks => state
            .selected_world_book
            .and_then(|index| state.world_books.get(index))
            .map(world_book_detail),
        ResourceTab::Chats => state.selected_chat.and_then(|(group, file)| {
            state.chat_groups.get(group).and_then(|group| {
                group
                    .files
                    .get(file)
                    .map(|item| chat_detail(group, item, state))
            })
        }),
        ResourceTab::Presets => state
            .selected_preset
            .and_then(|index| state.presets.get(index))
            .map(preset_detail),
    };

    container(content.unwrap_or_else(|| {
        empty_page(
            state.tab.icon(),
            "选择一项查看详情",
            match state.tab {
                ResourceTab::Characters => "可查看角色设定、首条消息和内嵌世界书。",
                ResourceTab::WorldBooks => "可检查触发关键词、条目状态和正文内容。",
                ResourceTab::Chats => "可预览最近 300 条用户与角色消息。",
                ResourceTab::Presets => "可检查模型参数、提示词顺序和启用状态。",
            },
        )
    }))
    .width(Fill)
    .height(Fill)
    .style(panel_surface)
    .into()
}

fn detail_header<'a>(
    icon: Icon,
    title: &'a str,
    subtitle: String,
) -> Element<'a, ResourceManageMessage> {
    container(
        row![
            container(icons::icon(icon, 18, BLUE_600))
                .width(38)
                .height(38)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(page_icon_surface),
            column![
                text(title)
                    .size(15)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
                text(subtitle)
                    .size(9)
                    .font(fonts::REGULAR)
                    .style(crate::theme::subtle_text_style),
            ]
            .spacing(3),
            space::horizontal(),
            tooltip(
                button(icons::icon(Icon::Trash2, 15, DANGER))
                    .on_press(ResourceManageMessage::RequestDelete)
                    .width(34)
                    .height(34)
                    .style(danger_outline_button_style),
                container(text("删除文件").size(10))
                    .padding([5, 8])
                    .style(tooltip_surface),
                tooltip::Position::Bottom,
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([11, 13])
    .style(panel_header_surface)
    .into()
}

fn character_detail(item: &CharacterCardInfo) -> Element<'_, ResourceManageMessage> {
    let tags: Element<'_, ResourceManageMessage> = if item.tags.is_empty() {
        text("无标签")
            .size(9)
            .font(fonts::REGULAR)
            .style(crate::theme::subtle_text_style)
            .into()
    } else {
        row(item.tags.iter().take(8).map(|tag| tag_chip(tag, BLUE_600)))
            .spacing(5)
            .into()
    };
    let mut content = column![
        row![
            container(
                image(item.filepath.clone())
                    .width(140)
                    .height(196)
                    .content_fit(iced::ContentFit::Cover),
            )
            .width(140)
            .height(196)
            .style(detail_image_surface),
            column![
                info_grid_row(
                    "创建者",
                    value_or(&item.creator, "未知"),
                    "版本",
                    value_or(&item.version, "未标注")
                ),
                info_grid_row(
                    "卡片规范",
                    spec_label(item),
                    "图片尺寸",
                    format!("{} × {}", item.image_width, item.image_height)
                ),
                info_grid_row(
                    "文件大小",
                    format_size(item.file_size),
                    "内嵌世界书",
                    if item.world_entries.is_empty() {
                        "无".into()
                    } else {
                        format!("{} 条", item.world_entries.len())
                    }
                ),
                text("标签")
                    .size(9)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::subtle_text_style),
                tags,
            ]
            .spacing(9),
        ]
        .spacing(15),
        detail_section("角色描述", &item.description),
        detail_section("性格", &item.personality),
        detail_section("场景", &item.scenario),
        detail_section("首条消息", &item.first_message),
    ]
    .spacing(14);
    if !item.world_entries.is_empty() {
        content = content.push(section_heading(
            Icon::BookOpenText,
            if item.world_name.is_empty() {
                "内嵌世界书"
            } else {
                &item.world_name
            },
            format!("{} 个条目", item.world_entries.len()),
        ));
        for entry in item.world_entries.iter().take(30) {
            content = content.push(world_entry_card(entry));
        }
    }

    column![
        detail_header(
            Icon::ContactRound,
            &item.name,
            format!("{} · {}", item.filename, format_size(item.file_size)),
        ),
        scrollable(container(content).padding(14)).height(Fill),
    ]
    .height(Fill)
    .into()
}

fn world_book_detail(item: &WorldBookInfo) -> Element<'_, ResourceManageMessage> {
    let mut content = column![
        row![
            metric_card(
                Icon::ListTree,
                "条目",
                item.entries.len().to_string(),
                BLUE_600
            ),
            metric_card(
                Icon::CircleCheck,
                "已启用",
                item.entries
                    .iter()
                    .filter(|entry| entry.enabled)
                    .count()
                    .to_string(),
                SUCCESS,
            ),
            metric_card(
                Icon::HardDrive,
                "大小",
                format_size(item.file_size),
                Color::from_rgb8(142, 68, 220)
            ),
        ]
        .spacing(9),
        section_heading(
            Icon::ListTree,
            "世界书条目",
            if item.author.is_empty() {
                "未知作者".into()
            } else {
                format!("作者：{}", item.author)
            },
        ),
    ]
    .spacing(12);
    for entry in item.entries.iter().take(100) {
        content = content.push(world_entry_card(entry));
    }
    if item.entries.is_empty() {
        content = content.push(inline_empty("这个世界书中没有可识别的条目。"));
    }
    column![
        detail_header(
            Icon::BookMarked,
            &item.name,
            format!("{} · {}", item.filename, format_size(item.file_size)),
        ),
        scrollable(container(content).padding(14)).height(Fill),
    ]
    .height(Fill)
    .into()
}

fn chat_detail<'a>(
    group: &'a ChatGroup,
    file: &'a ChatFileInfo,
    state: &'a ResourceManageState,
) -> Element<'a, ResourceManageMessage> {
    let mut messages = column![
        row![
            metric_card(
                Icon::MessagesSquare,
                "消息",
                state.chat_messages.len().to_string(),
                BLUE_600
            ),
            metric_card(
                Icon::UserRound,
                "用户消息",
                state
                    .chat_messages
                    .iter()
                    .filter(|message| message.is_user)
                    .count()
                    .to_string(),
                SUCCESS,
            ),
            metric_card(
                Icon::HardDrive,
                "大小",
                format_size(file.file_size),
                Color::from_rgb8(142, 68, 220)
            ),
        ]
        .spacing(9),
        section_heading(
            Icon::MessageCircle,
            "对话预览",
            "显示最近 300 条消息".into()
        ),
    ]
    .spacing(11);
    if state.chat_messages.is_empty() {
        messages = messages.push(inline_empty("此文件没有可识别的聊天消息。"));
    } else {
        for message in &state.chat_messages {
            messages = messages.push(chat_bubble(message));
        }
    }
    column![
        detail_header(
            Icon::MessagesSquare,
            &group.name,
            format!("{} · {}", file.display_time, format_size(file.file_size)),
        ),
        scrollable(container(messages).padding(14)).height(Fill),
    ]
    .height(Fill)
    .into()
}

fn preset_detail(item: &PresetInfo) -> Element<'_, ResourceManageMessage> {
    let mut content = column![
        row![
            metric_card(
                Icon::ListChecks,
                "提示词",
                item.prompts.len().to_string(),
                BLUE_600
            ),
            metric_card(
                Icon::CircleCheck,
                "已启用",
                item.prompts
                    .iter()
                    .filter(|prompt| prompt.enabled)
                    .count()
                    .to_string(),
                SUCCESS,
            ),
            metric_card(
                Icon::PlugZap,
                "扩展格式",
                if item.has_spreset {
                    "SPreset"
                } else {
                    "标准"
                }
                .into(),
                Color::from_rgb8(142, 68, 220),
            ),
        ]
        .spacing(9),
        container(
            column![
                info_grid_row(
                    "接口来源",
                    value_or(&item.source, "未指定"),
                    "模型",
                    value_or(&item.model, "未指定")
                ),
                info_grid_row(
                    "上下文",
                    number_or(item.max_context),
                    "最大输出",
                    number_or(item.max_tokens)
                ),
                info_grid_row(
                    "流式输出",
                    bool_label(item.stream).into(),
                    "文件大小",
                    format_size(item.file_size)
                ),
            ]
            .spacing(8),
        )
        .padding(12)
        .style(info_surface),
        section_heading(
            Icon::ListChecks,
            "提示词结构",
            format!("{} 个条目", item.prompts.len())
        ),
    ]
    .spacing(12);
    if item.prompts.is_empty() {
        content = content.push(inline_empty("这个预设中没有可识别的 prompts 数组。"));
    } else {
        for (index, prompt) in item.prompts.iter().take(100).enumerate() {
            content = content.push(prompt_card(index, prompt));
        }
    }
    column![
        detail_header(
            Icon::SlidersHorizontal,
            &item.name,
            format!("{} · {}", item.filename, format_size(item.file_size)),
        ),
        scrollable(container(content).padding(14)).height(Fill),
    ]
    .height(Fill)
    .into()
}

fn detail_section<'a>(title: &'static str, value: &'a str) -> Element<'a, ResourceManageMessage> {
    container(
        column![
            text(title)
                .size(10)
                .font(fonts::MEDIUM)
                .style(crate::theme::muted_text_style),
            text(if value.trim().is_empty() {
                "未填写"
            } else {
                value
            })
            .size(10)
            .font(fonts::REGULAR)
            .color(if value.trim().is_empty() {
                INK_SUBTLE
            } else {
                INK
            }),
        ]
        .spacing(6),
    )
    .width(Fill)
    .padding(12)
    .style(info_surface)
    .into()
}

fn info_grid_row<'a>(
    first_label: &'static str,
    first_value: String,
    second_label: &'static str,
    second_value: String,
) -> Element<'a, ResourceManageMessage> {
    row![
        info_pair(first_label, first_value),
        info_pair(second_label, second_value),
    ]
    .spacing(8)
    .into()
}

fn info_pair<'a>(label: &'static str, value: String) -> Element<'a, ResourceManageMessage> {
    container(
        column![
            text(label)
                .size(8)
                .font(fonts::MEDIUM)
                .style(crate::theme::subtle_text_style),
            text(value)
                .size(10)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
        ]
        .spacing(2),
    )
    .width(Fill)
    .padding([7, 9])
    .style(meta_surface)
    .into()
}

fn metric_card(
    icon: Icon,
    label: &'static str,
    value: String,
    accent: Color,
) -> Element<'static, ResourceManageMessage> {
    container(
        row![
            container(icons::icon(icon, 15, accent))
                .width(30)
                .height(30)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_theme| accent_surface(accent)),
            column![
                text(label)
                    .size(8)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::subtle_text_style),
                text(value)
                    .size(12)
                    .font(fonts::MEDIUM)
                    .style(crate::theme::text_style),
            ]
            .spacing(2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([9, 10])
    .style(info_surface)
    .into()
}

fn section_heading<'a>(
    icon: Icon,
    title: &'a str,
    meta: String,
) -> Element<'a, ResourceManageMessage> {
    row![
        icons::icon(icon, 14, BLUE_600),
        text(title)
            .size(11)
            .font(fonts::MEDIUM)
            .style(crate::theme::text_style),
        space::horizontal(),
        text(meta)
            .size(9)
            .font(fonts::REGULAR)
            .style(crate::theme::subtle_text_style),
    ]
    .spacing(7)
    .align_y(Alignment::Center)
    .into()
}

fn world_entry_card(entry: &WorldEntry) -> Element<'_, ResourceManageMessage> {
    let keywords = if entry.keys.is_empty() {
        "无关键词".into()
    } else {
        entry.keys.join("、")
    };
    container(
        column![
            row![
                container(icons::icon(
                    if entry.enabled {
                        Icon::CircleCheck
                    } else {
                        Icon::CircleOff
                    },
                    13,
                    if entry.enabled { SUCCESS } else { INK_SUBTLE },
                ))
                .width(24),
                text(if entry.comment.is_empty() {
                    "未命名条目"
                } else {
                    &entry.comment
                })
                .size(10)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
                space::horizontal(),
                text(if entry.enabled {
                    "已启用"
                } else {
                    "已禁用"
                })
                .size(8)
                .font(fonts::MEDIUM)
                .color(if entry.enabled { SUCCESS } else { INK_SUBTLE }),
            ]
            .align_y(Alignment::Center),
            text(format!("关键词：{}", truncate(&keywords, 80)))
                .size(9)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
            text(if entry.content.trim().is_empty() {
                "无正文".into()
            } else {
                truncate(&entry.content, 220)
            })
            .size(9)
            .font(fonts::REGULAR)
            .style(crate::theme::text_style),
        ]
        .spacing(6),
    )
    .width(Fill)
    .padding(11)
    .style(info_surface)
    .into()
}

fn chat_bubble(message: &ChatMessage) -> Element<'_, ResourceManageMessage> {
    let accent = if message.is_user {
        BLUE_600
    } else {
        Color::from_rgb8(142, 68, 220)
    };
    container(
        column![
            row![
                icons::icon(
                    if message.is_user {
                        Icon::UserRound
                    } else {
                        Icon::Bot
                    },
                    13,
                    accent
                ),
                text(if message.name.is_empty() {
                    if message.is_user { "用户" } else { "角色" }
                } else {
                    &message.name
                })
                .size(9)
                .font(fonts::MEDIUM)
                .color(accent),
                space::horizontal(),
                text(&message.send_date)
                    .size(8)
                    .font(fonts::REGULAR)
                    .style(crate::theme::subtle_text_style),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            text(&message.content)
                .size(9)
                .font(fonts::REGULAR)
                .style(crate::theme::text_style),
        ]
        .spacing(6),
    )
    .width(Fill)
    .padding(11)
    .style(move |_theme| chat_surface(accent))
    .into()
}

fn prompt_card(index: usize, prompt: &PresetPrompt) -> Element<'_, ResourceManageMessage> {
    container(
        column![
            row![
                container(
                    text((index + 1).to_string())
                        .size(9)
                        .font(fonts::MEDIUM)
                        .color(BLUE_600),
                )
                .width(24)
                .height(24)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(count_surface),
                text(if prompt.name.is_empty() {
                    "未命名提示词"
                } else {
                    &prompt.name
                })
                .size(10)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
                space::horizontal(),
                tag_chip(
                    if prompt.role.is_empty() {
                        "marker"
                    } else {
                        &prompt.role
                    },
                    if prompt.enabled { SUCCESS } else { INK_SUBTLE },
                ),
            ]
            .spacing(7)
            .align_y(Alignment::Center),
            text(if prompt.marker {
                "结构标记，不包含正文".into()
            } else if prompt.content.trim().is_empty() {
                "无正文".into()
            } else {
                truncate(&prompt.content, 260)
            })
            .size(9)
            .font(fonts::REGULAR)
            .color(if prompt.content.trim().is_empty() {
                INK_SUBTLE
            } else {
                INK
            }),
        ]
        .spacing(7),
    )
    .width(Fill)
    .padding(11)
    .style(info_surface)
    .into()
}

fn tag_chip<'a>(label: &'a str, color: Color) -> Element<'a, ResourceManageMessage> {
    container(text(label).size(8).font(fonts::MEDIUM).color(color))
        .padding([3, 7])
        .style(move |_theme| accent_surface(color))
        .into()
}

fn inline_empty(message: &str) -> Element<'_, ResourceManageMessage> {
    container(
        row![
            crate::theme::subtle_icon(Icon::Inbox, 15),
            text(message)
                .size(9)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(13)
    .style(info_surface)
    .into()
}

fn empty_page(
    icon: Icon,
    title: &'static str,
    description: &'static str,
) -> Element<'static, ResourceManageMessage> {
    container(
        column![
            container(icons::icon(icon, 26, BLUE_600))
                .width(58)
                .height(58)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(page_icon_surface),
            text(title)
                .size(13)
                .font(fonts::MEDIUM)
                .style(crate::theme::text_style),
            text(description)
                .size(10)
                .font(fonts::REGULAR)
                .style(crate::theme::muted_text_style),
        ]
        .spacing(9)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn value_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

fn bool_label(value: bool) -> &'static str {
    if value { "开启" } else { "关闭" }
}

fn number_or(value: i64) -> String {
    if value > 0 {
        value.to_string()
    } else {
        "未设置".into()
    }
}

fn spec_label(item: &CharacterCardInfo) -> String {
    match (item.spec.trim(), item.spec_version.trim()) {
        ("", "") => "未标注".into(),
        (spec, "") => spec.into(),
        ("", version) => version.into(),
        (spec, version) => format!("{spec} {version}"),
    }
}

fn page_icon_surface(_theme: &Theme) -> container::Style {
    accent_surface(BLUE_600)
}

fn source_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 8.0.into(),
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
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

fn panel_header_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 0.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

fn search_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

fn count_surface(_theme: &Theme) -> container::Style {
    accent_surface(BLUE_600)
}

fn tab_count_surface(active: bool) -> container::Style {
    if active {
        accent_surface(BLUE_600)
    } else {
        accent_surface(INK_SUBTLE)
    }
}

fn tab_button_style(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            radius: 6.0.into(),
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

fn list_item_style(theme: &Theme, selected: bool, status: button::Status) -> button::Style {
    let background = if selected {
        Some(Background::Color(Color::from_rgba(
            theme.palette().primary.r,
            theme.palette().primary.g,
            theme.palette().primary.b,
            if crate::theme::is_dark(theme) {
                0.24
            } else {
                0.12
            },
        )))
    } else if matches!(status, button::Status::Hovered) {
        Some(Background::Color(crate::theme::surface_alt(theme)))
    } else {
        None
    };
    button::Style {
        background,
        border: Border {
            color: if selected {
                Color::from_rgba(
                    theme.palette().primary.r,
                    theme.palette().primary.g,
                    theme.palette().primary.b,
                    0.45,
                )
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: 7.0.into(),
        },
        ..button::Style::default()
    }
}

fn item_icon_surface(selected: bool) -> container::Style {
    accent_surface(if selected { BLUE_600 } else { INK_SUBTLE })
}

fn thumbnail_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

fn detail_image_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 8.0.into(),
        },
        ..container::Style::default()
    }
}

fn group_header_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 0.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

fn info_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 7.0.into(),
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
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

fn accent_surface(accent: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            accent.r, accent.g, accent.b, 0.10,
        ))),
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn chat_surface(accent: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            accent.r, accent.g, accent.b, 0.055,
        ))),
        border: Border {
            color: Color::from_rgba(accent.r, accent.g, accent.b, 0.18),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

fn notice_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.08,
        ))),
        border: Border {
            color: Color::from_rgba(BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.18),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

fn delete_notice_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            DANGER.r, DANGER.g, DANGER.b, 0.07,
        ))),
        border: Border {
            color: Color::from_rgba(DANGER.r, DANGER.g, DANGER.b, 0.22),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

fn tooltip_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(38, 38, 42))),
        text_color: Some(WHITE),
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

fn quiet_button_style(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered)
            .then_some(Background::Color(crate::theme::surface_alt(theme))),
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn danger_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = if matches!(status, button::Status::Hovered) {
        Color::from_rgb8(190, 36, 45)
    } else {
        DANGER
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: WHITE,
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn danger_outline_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered).then_some(Background::Color(
            Color::from_rgba(DANGER.r, DANGER.g, DANGER.b, 0.08),
        )),
        border: Border {
            color: Color::from_rgba(DANGER.r, DANGER.g, DANGER.b, 0.35),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..button::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_filename_is_presented_without_character_prefix() {
        assert_eq!(
            chat_display_time("Seraphina - 2026-06-15@17h18m44s700ms.jsonl"),
            "2026-06-15@17h18m44s700ms"
        );
    }

    #[test]
    fn parses_world_entries_from_object_and_array_shapes() {
        let object = serde_json::json!({
            "entries": {
                "0": { "key": ["Astra"], "content": "Lore", "disable": false }
            }
        });
        let array = serde_json::json!({
            "entries": [
                { "keys": ["Brew"], "content": "World", "enabled": false }
            ]
        });
        assert_eq!(parse_world_entries(&object).len(), 1);
        assert!(parse_world_entries(&object)[0].enabled);
        assert_eq!(parse_world_entries(&array)[0].keys, vec!["Brew"]);
        assert!(!parse_world_entries(&array)[0].enabled);
    }

    #[test]
    fn base64_decoder_handles_character_json() {
        let decoded = decode_base64("eyJuYW1lIjoiQXN0cmEifQ==").expect("valid base64");
        assert_eq!(String::from_utf8(decoded).unwrap(), r#"{"name":"Astra"}"#);
    }
}
