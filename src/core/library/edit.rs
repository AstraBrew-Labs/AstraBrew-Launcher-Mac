//! 资源工作台使用的无损读取与安全写回能力。
//!
//! 编辑器始终保留原始 JSON 对象，只覆盖用户实际编辑的字段。角色卡则保留 PNG
//! 中除目标角色元数据块以外的所有字节，避免编辑文本时破坏图片或扩展元数据。

use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use serde_json::{Map, Number, Value};

use super::{
    PNG_SIGNATURE, ResourceData, ResourceKind, ValidatedResource, character_keyword_rank,
    parse_json_candidate, validate_bytes, validate_path,
};

/// 工作台加载后的原始文档及可编辑内容。
#[derive(Debug, Clone)]
pub(crate) struct LoadedEditor {
    pub document: EditableDocument,
    pub data: EditorData,
}

/// 三类资源各自独立的可编辑数据。
#[derive(Debug, Clone)]
pub(crate) enum EditorData {
    Character(EditableCharacter),
    WorldBook(EditableWorldBook),
    Preset(EditablePreset),
}

impl EditorData {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Character(_) => ResourceKind::CharacterCard,
            Self::WorldBook(_) => ResourceKind::WorldBook,
            Self::Preset(_) => ResourceKind::Preset,
        }
    }
}

/// 保存时用于检测外部修改并重建文件的原始上下文。
#[derive(Debug, Clone)]
pub(crate) struct EditableDocument {
    path: PathBuf,
    bytes: Vec<u8>,
    root: Map<String, Value>,
    character_metadata: Option<CharacterMetadata>,
    kind: ResourceKind,
}

impl EditableDocument {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// 角色卡常用字段及卡内世界书副本。
#[derive(Debug, Clone)]
pub(crate) struct EditableCharacter {
    pub name: String,
    pub creator: String,
    pub version: String,
    pub description: String,
    pub personality: String,
    pub scenario: String,
    pub first_message: String,
    pub tags: String,
    pub world_book: Option<EditableWorldBook>,
}

/// 世界书及其原始结构。`raw` 和定位器只由核心层读写。
#[derive(Debug, Clone)]
pub(crate) struct EditableWorldBook {
    pub name: String,
    pub author: String,
    pub entries: Vec<EditableWorldEntry>,
    raw: Map<String, Value>,
}

/// 世界书条目的常用字段。
#[derive(Debug, Clone)]
pub(crate) struct EditableWorldEntry {
    pub comment: String,
    pub keys: String,
    pub secondary_keys: String,
    pub content: String,
    pub enabled: bool,
    pub order: String,
    pub position: String,
    pub probability: String,
    pub depth: String,
    locator: EntryLocator,
}

#[derive(Debug, Clone)]
enum EntryLocator {
    Array(usize),
    Object(String),
}

/// 预设提示词及原始扩展字段。
#[derive(Debug, Clone)]
pub(crate) struct EditablePreset {
    pub prompts: Vec<EditablePresetPrompt>,
    has_prompt_order: bool,
}

/// `partial` 表示不同 prompt_order 模板中的启用状态不一致。
#[derive(Debug, Clone)]
pub(crate) struct EditablePresetPrompt {
    pub identifier: String,
    pub name: String,
    pub role: String,
    pub content: String,
    pub enabled: bool,
    pub partial: bool,
    pub marker: bool,
    pub enabled_changed: bool,
    pub newly_added: bool,
    raw: Map<String, Value>,
}

impl EditablePreset {
    pub const fn has_prompt_order(&self) -> bool {
        self.has_prompt_order
    }
}

/// 在后台为预设创建一个可被 SillyTavern 稳定引用的普通 system 提示词。
pub(crate) fn create_preset_prompt(
    prompts: &[EditablePresetPrompt],
) -> EditablePresetPrompt {
    let existing = prompts
        .iter()
        .map(|prompt| prompt.identifier.as_str())
        .collect::<HashSet<_>>();
    let mut sequence = 1_u64;
    let identifier = loop {
        let candidate = format!("astrabrew_prompt_{sequence}");
        if !existing.contains(candidate.as_str()) {
            break candidate;
        }
        sequence += 1;
    };
    EditablePresetPrompt {
        identifier: identifier.clone(),
        name: "New Prompt".to_owned(),
        role: "system".to_owned(),
        content: String::new(),
        enabled: true,
        partial: false,
        marker: false,
        enabled_changed: true,
        newly_added: true,
        raw: Map::from_iter([
            ("identifier".to_owned(), Value::String(identifier)),
            ("name".to_owned(), Value::String("New Prompt".to_owned())),
            ("role".to_owned(), Value::String("system".to_owned())),
            ("content".to_owned(), Value::String(String::new())),
            ("system_prompt".to_owned(), Value::Bool(true)),
            ("marker".to_owned(), Value::Bool(false)),
            ("enabled".to_owned(), Value::Bool(true)),
        ]),
    }
}

impl LoadedEditor {
    /// 保存成功后只更新磁盘基线，保留保存期间用户继续输入的编辑内容。
    pub(crate) fn adopt_saved_document(&mut self, saved: Self) {
        self.document = saved.document;
    }
}

/// 从磁盘读取并构造对应类型的工作台数据。
pub(crate) fn load_editor(path: &Path, kind: ResourceKind) -> Result<LoadedEditor, String> {
    let ValidatedResource {
        data,
        bytes,
        editor_root,
        source_bytes,
        ..
    } = validate_path(kind, path).map_err(format_issues)?;
    let bytes = source_bytes.unwrap_or(bytes);
    match data {
        ResourceData::CharacterCard(_) => load_character_editor(path, bytes),
        ResourceData::WorldBook(_) => load_validated_json_editor(
            path,
            bytes,
            editor_root.ok_or_else(|| "世界书缺少已校验的 JSON 数据。".to_owned())?,
            ResourceKind::WorldBook,
        ),
        ResourceData::Preset(_) => load_validated_json_editor(
            path,
            bytes,
            editor_root.ok_or_else(|| "预设缺少已校验的 JSON 数据。".to_owned())?,
            ResourceKind::Preset,
        ),
    }
}

/// 绑定角色卡时读取外部世界书的完整 JSON，未知字段也会一并复制。
pub(crate) fn load_world_book_for_binding(path: &Path) -> Result<EditableWorldBook, String> {
    let loaded = load_editor(path, ResourceKind::WorldBook)?;
    match loaded.data {
        EditorData::WorldBook(world) => Ok(world),
        _ => Err("选择的文件不是世界书。".to_owned()),
    }
}

/// 保存当前修订并返回以新文件为基线的编辑器数据。
pub(crate) fn save_editor(loaded: LoadedEditor) -> Result<LoadedEditor, String> {
    if loaded.data.kind() != loaded.document.kind {
        return Err("编辑内容与资源类型不匹配。".to_owned());
    }
    let current =
        fs::read(loaded.document.path()).map_err(|error| format!("重新读取资源失败：{error}"))?;
    if current != loaded.document.bytes {
        return Err("资源文件已被其他程序修改，请重新打开工作台后再编辑。".to_owned());
    }

    let mut root = loaded.document.root.clone();
    let output = match &loaded.data {
        EditorData::Character(character) => {
            apply_character(&mut root, character)?;
            let metadata = loaded
                .document
                .character_metadata
                .as_ref()
                .ok_or_else(|| "角色卡缺少可写回的 PNG 元数据。".to_owned())?;
            rebuild_character_png(&loaded.document.bytes, &root, metadata)?
        }
        EditorData::WorldBook(world) => {
            root = apply_world_book(world)?;
            serde_json::to_vec_pretty(&Value::Object(root))
                .map_err(|error| format!("序列化世界书失败：{error}"))?
        }
        EditorData::Preset(preset) => {
            apply_preset(&mut root, preset)?;
            serde_json::to_vec_pretty(&Value::Object(root))
                .map_err(|error| format!("序列化预设失败：{error}"))?
        }
    };

    validate_bytes(
        loaded.data.kind(),
        file_name(&loaded.document.path),
        output.clone(),
    )
    .map_err(format_issues)?;
    ensure_backup(&loaded.document.path, &loaded.document.bytes)?;
    atomic_replace(&loaded.document.path, &output)?;
    load_editor(&loaded.document.path, loaded.data.kind())
}

fn load_validated_json_editor(
    path: &Path,
    bytes: Vec<u8>,
    root: Map<String, Value>,
    kind: ResourceKind,
) -> Result<LoadedEditor, String> {
    let data = match kind {
        ResourceKind::WorldBook => EditorData::WorldBook(world_book_from_raw(root.clone())?),
        ResourceKind::Preset => EditorData::Preset(preset_from_raw(&root)?),
        ResourceKind::CharacterCard => return Err("资源类型不匹配。".to_owned()),
    };
    Ok(LoadedEditor {
        document: EditableDocument {
            path: path.to_path_buf(),
            bytes,
            root,
            character_metadata: None,
            kind,
        },
        data,
    })
}

fn load_character_editor(path: &Path, bytes: Vec<u8>) -> Result<LoadedEditor, String> {
    let (root, metadata) = extract_character_metadata(&bytes)?;
    let data_object = root.get("data").and_then(Value::as_object).unwrap_or(&root);
    let world_book = find_world_value(&root, data_object)
        .map(|world| world_book_from_raw(world.clone()))
        .transpose()?;
    let character = EditableCharacter {
        name: pick_string(data_object, &["name"]),
        creator: pick_string(data_object, &["creator"]),
        version: pick_string(data_object, &["character_version", "version"]),
        description: pick_string(data_object, &["description"]),
        personality: pick_string(data_object, &["personality"]),
        scenario: pick_string(data_object, &["scenario"]),
        first_message: pick_string(data_object, &["first_mes", "firstMessage"]),
        tags: data_object
            .get("tags")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        world_book,
    };
    Ok(LoadedEditor {
        document: EditableDocument {
            path: path.to_path_buf(),
            bytes,
            root,
            character_metadata: Some(metadata),
            kind: ResourceKind::CharacterCard,
        },
        data: EditorData::Character(character),
    })
}

fn apply_character(root: &mut Map<String, Value>, edit: &EditableCharacter) -> Result<(), String> {
    if edit.name.trim().is_empty() {
        return Err("角色卡名称不能为空。".to_owned());
    }
    let has_data = root.get("data").is_some_and(Value::is_object);
    let target = if has_data {
        root.get_mut("data")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "角色卡 data 字段无效。".to_owned())?
    } else {
        &mut *root
    };
    set_string_alias(target, &["name"], "name", &edit.name);
    set_string_alias(target, &["creator"], "creator", &edit.creator);
    set_string_alias(
        target,
        &["character_version", "version"],
        if has_data {
            "character_version"
        } else {
            "version"
        },
        &edit.version,
    );
    set_string_alias(target, &["description"], "description", &edit.description);
    set_string_alias(target, &["personality"], "personality", &edit.personality);
    set_string_alias(target, &["scenario"], "scenario", &edit.scenario);
    set_string_alias(
        target,
        &["first_mes", "firstMessage"],
        "first_mes",
        &edit.first_message,
    );
    let tags = split_list(&edit.tags)
        .into_iter()
        .map(Value::String)
        .collect::<Vec<_>>();
    target.insert("tags".to_owned(), Value::Array(tags));

    let existing_location = find_world_location(root);
    match &edit.world_book {
        Some(world) => {
            let value = Value::Object(apply_world_book(world)?);
            set_world_value(root, existing_location, has_data, value)?;
        }
        None => remove_world_value(root, existing_location),
    }
    Ok(())
}

fn world_book_from_raw(raw: Map<String, Value>) -> Result<EditableWorldBook, String> {
    let name = pick_string(&raw, &["name", "title", "world_name"]);
    let author = pick_string(&raw, &["author", "creator"]);
    let entries = match raw.get("entries") {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, value)| world_entry_from_raw(value, EntryLocator::Array(index)))
            .collect::<Result<Vec<_>, _>>()?,
        Some(Value::Object(items)) => items
            .iter()
            .map(|(key, value)| world_entry_from_raw(value, EntryLocator::Object(key.clone())))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("世界书缺少有效的 entries。".to_owned()),
    };
    Ok(EditableWorldBook {
        name,
        author,
        entries,
        raw,
    })
}

fn world_entry_from_raw(
    value: &Value,
    locator: EntryLocator,
) -> Result<EditableWorldEntry, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "世界书条目必须是对象。".to_owned())?;
    Ok(EditableWorldEntry {
        comment: pick_string(object, &["comment", "name"]),
        keys: string_list(object, &["keys", "key"]),
        secondary_keys: string_list(object, &["secondary_keys", "keysecondary"]),
        content: pick_string(object, &["content"]),
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                !object
                    .get("disable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            }),
        order: number_text(object.get("order")),
        position: scalar_text(object.get("position")),
        probability: number_text(object.get("probability")),
        depth: number_text(object.get("depth")),
        locator,
    })
}

fn apply_world_book(edit: &EditableWorldBook) -> Result<Map<String, Value>, String> {
    let mut root = edit.raw.clone();
    set_string_alias(
        &mut root,
        &["name", "title", "world_name"],
        "name",
        &edit.name,
    );
    if !edit.author.is_empty() || root.contains_key("author") || root.contains_key("creator") {
        set_string_alias(&mut root, &["author", "creator"], "author", &edit.author);
    }
    for entry in &edit.entries {
        let object = entry_object_mut(&mut root, &entry.locator)?;
        set_string_alias(object, &["comment", "name"], "comment", &entry.comment);
        set_array_alias(object, &["keys", "key"], "keys", split_list(&entry.keys));
        set_array_alias(
            object,
            &["secondary_keys", "keysecondary"],
            "secondary_keys",
            split_list(&entry.secondary_keys),
        );
        set_string_alias(object, &["content"], "content", &entry.content);
        if object.contains_key("enabled") {
            object.insert("enabled".to_owned(), Value::Bool(entry.enabled));
        }
        if object.contains_key("disable") {
            object.insert("disable".to_owned(), Value::Bool(!entry.enabled));
        }
        if !object.contains_key("enabled") && !object.contains_key("disable") {
            object.insert("enabled".to_owned(), Value::Bool(entry.enabled));
        }
        set_optional_number(object, "order", &entry.order)?;
        set_optional_scalar(object, "position", &entry.position);
        set_optional_number(object, "probability", &entry.probability)?;
        set_optional_number(object, "depth", &entry.depth)?;
    }
    Ok(root)
}

fn entry_object_mut<'a>(
    root: &'a mut Map<String, Value>,
    locator: &EntryLocator,
) -> Result<&'a mut Map<String, Value>, String> {
    let entries = root
        .get_mut("entries")
        .ok_or_else(|| "世界书 entries 已不存在。".to_owned())?;
    let value = match locator {
        EntryLocator::Array(index) => entries
            .as_array_mut()
            .and_then(|items| items.get_mut(*index)),
        EntryLocator::Object(key) => entries.as_object_mut().and_then(|items| items.get_mut(key)),
    }
    .ok_or_else(|| "世界书条目结构已改变。".to_owned())?;
    value
        .as_object_mut()
        .ok_or_else(|| "世界书条目必须是对象。".to_owned())
}

fn preset_from_raw(root: &Map<String, Value>) -> Result<EditablePreset, String> {
    let order_states = preset_order_states(root);
    let has_prompt_order = root.get("prompt_order").is_some_and(Value::is_array);
    let mut prompts = Vec::new();
    let mut used_identifiers = HashSet::new();
    if let Some(items) = root.get("prompts").and_then(Value::as_array) {
        for (index, value) in items.iter().enumerate() {
            let raw = value
                .as_object()
                .ok_or_else(|| format!("预设第 {} 个提示词不是对象。", index + 1))?
                .clone();
            let preferred = pick_string(&raw, &["identifier", "name"]);
            let identifier = unique_identifier(&preferred, index, &used_identifiers);
            used_identifiers.insert(identifier.clone());
            let states = order_states.get(&identifier);
            let direct = raw.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let enabled = states
                .and_then(|states| states.first().copied())
                .unwrap_or(direct);
            let partial = states.is_some_and(|states| states.iter().any(|state| *state != enabled));
            prompts.push(EditablePresetPrompt {
                identifier,
                name: pick_string(&raw, &["name", "identifier"]),
                role: pick_string(&raw, &["role"]),
                content: pick_string(&raw, &["content"]),
                enabled,
                partial,
                marker: raw.get("marker").and_then(Value::as_bool).unwrap_or(false),
                enabled_changed: false,
                newly_added: false,
                raw,
            });
        }
    }
    Ok(EditablePreset {
        prompts,
        has_prompt_order,
    })
}

fn unique_identifier(preferred: &str, index: usize, used: &HashSet<String>) -> String {
    if !preferred.is_empty() && !used.contains(preferred) {
        return preferred.to_owned();
    }
    let base = format!("astrabrew_prompt_{}", index + 1);
    if !used.contains(&base) {
        return base;
    }
    let mut suffix = 2_u64;
    loop {
        let candidate = format!("{base}_{suffix}");
        if !used.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn preset_order_states(root: &Map<String, Value>) -> HashMap<String, Vec<bool>> {
    let mut states: HashMap<String, Vec<bool>> = HashMap::new();
    let Some(orders) = root.get("prompt_order").and_then(Value::as_array) else {
        return states;
    };
    for template in orders {
        let Some(items) = template.get("order").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(identifier) = item.get("identifier").and_then(Value::as_str) else {
                continue;
            };
            let enabled = item.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            states
                .entry(identifier.to_owned())
                .or_default()
                .push(enabled);
        }
    }
    states
}

fn apply_preset(root: &mut Map<String, Value>, edit: &EditablePreset) -> Result<(), String> {
    let identifiers = edit
        .prompts
        .iter()
        .map(|prompt| prompt.identifier.as_str())
        .collect::<HashSet<_>>();
    if identifiers.len() != edit.prompts.len() || identifiers.contains("") {
        return Err("预设提示词 identifier 必须存在且不能重复。".to_owned());
    }

    let original_identifiers = preset_from_raw(root)?
        .prompts
        .into_iter()
        .map(|prompt| prompt.identifier)
        .collect::<HashSet<_>>();
    let mut prompts = Vec::with_capacity(edit.prompts.len());
    for prompt in &edit.prompts {
        if !prompt.marker && (prompt.name.trim().is_empty() || prompt.role.trim().is_empty()) {
            return Err("普通预设条目的名称和角色不能为空。".to_owned());
        }
        let mut raw = prompt.raw.clone();
        raw.insert(
            "identifier".to_owned(),
            Value::String(prompt.identifier.clone()),
        );
        set_string_alias(&mut raw, &["name"], "name", &prompt.name);
        if !prompt.marker {
            set_string_alias(&mut raw, &["role"], "role", &prompt.role);
            set_string_alias(&mut raw, &["content"], "content", &prompt.content);
        }
        if prompt.enabled_changed || raw.contains_key("enabled") {
            raw.insert("enabled".to_owned(), Value::Bool(prompt.enabled));
        }
        prompts.push(Value::Object(raw));
    }
    root.insert("prompts".to_owned(), Value::Array(prompts));

    // 只清理用户本次删除的 prompts；扩展可能在 prompt_order 中保存额外引用，必须原样保留。
    let deleted_identifiers = original_identifiers
        .into_iter()
        .filter(|identifier| !identifiers.contains(identifier.as_str()))
        .collect::<HashSet<_>>();

    if let Some(templates) = root.get_mut("prompt_order").and_then(Value::as_array_mut) {
        for template in templates {
            let Some(order) = template.get_mut("order").and_then(Value::as_array_mut) else {
                continue;
            };
            order.retain(|item| {
                item.get("identifier")
                    .and_then(Value::as_str)
                    .is_none_or(|identifier| !deleted_identifiers.contains(identifier))
            });
            for prompt in &edit.prompts {
                if let Some(item) = order.iter_mut().find(|item| {
                    item.get("identifier").and_then(Value::as_str)
                        == Some(prompt.identifier.as_str())
                }) {
                    if prompt.enabled_changed {
                        item["enabled"] = Value::Bool(prompt.enabled);
                    }
                } else if prompt.newly_added {
                    // 工作台新增条目显示在顶部，顺序模板也插入顶部以保持执行顺序一致。
                    order.insert(0, Value::Object(Map::from_iter([
                        (
                            "identifier".to_owned(),
                            Value::String(prompt.identifier.clone()),
                        ),
                        ("enabled".to_owned(), Value::Bool(prompt.enabled)),
                    ])));
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CharacterMetadata {
    chunk_start: usize,
    chunk_end: usize,
    keyword: String,
    chunk_kind: [u8; 4],
    encoding: CharacterEncoding,
}

#[derive(Debug, Clone, Copy)]
enum CharacterEncoding {
    Plain,
    StandardBase64,
    UrlSafeBase64 { padded: bool },
}

fn extract_character_metadata(
    bytes: &[u8],
) -> Result<(Map<String, Value>, CharacterMetadata), String> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err("角色卡不是有效的 PNG。".to_owned());
    }
    let mut candidates = Vec::new();
    let mut offset = PNG_SIGNATURE.len();
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12 + length)
            .ok_or_else(|| "PNG 块长度溢出。".to_owned())?;
        if end > bytes.len() {
            return Err("PNG 块超出文件边界。".to_owned());
        }
        let kind: [u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
        if let Some((keyword, text)) =
            decode_text_chunk(kind, &bytes[offset + 8..offset + 8 + length])
            && character_keyword_rank(&keyword) != usize::MAX
            && let Some(root) = parse_json_candidate(&text)
            && let Some(encoding) = detect_character_encoding(&text)
        {
            candidates.push((
                character_keyword_rank(&keyword),
                root,
                CharacterMetadata {
                    chunk_start: offset,
                    chunk_end: end,
                    keyword,
                    chunk_kind: kind,
                    encoding,
                },
            ));
        }
        offset = end;
        if &kind == b"IEND" {
            break;
        }
    }
    candidates.sort_by_key(|candidate| candidate.0);
    candidates
        .into_iter()
        .next()
        .map(|(_, root, metadata)| (root, metadata))
        .ok_or_else(|| "没有找到可编辑的角色卡元数据。".to_owned())
}

fn detect_character_encoding(text: &str) -> Option<CharacterEncoding> {
    let trimmed = text.trim();
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return Some(CharacterEncoding::Plain);
    }
    if STANDARD
        .decode(trimmed)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some()
    {
        return Some(CharacterEncoding::StandardBase64);
    }
    let normalized = normalize_url_base64(trimmed)?;
    URL_SAFE
        .decode(normalized)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .map(|_| CharacterEncoding::UrlSafeBase64 {
            padded: trimmed.ends_with('='),
        })
}

fn normalize_url_base64(value: &str) -> Option<String> {
    let mut normalized = value.replace('-', "+").replace('_', "/");
    match normalized.len() % 4 {
        0 => {}
        2 => normalized.push_str("=="),
        3 => normalized.push('='),
        _ => return None,
    }
    Some(normalized.replace('+', "-").replace('/', "_"))
}

fn rebuild_character_png(
    original: &[u8],
    root: &Map<String, Value>,
    metadata: &CharacterMetadata,
) -> Result<Vec<u8>, String> {
    let json = serde_json::to_vec(&Value::Object(root.clone()))
        .map_err(|error| format!("序列化角色卡失败：{error}"))?;
    let payload = match metadata.encoding {
        CharacterEncoding::Plain => escape_json_non_ascii(
            &String::from_utf8(json).map_err(|error| format!("角色卡 JSON 编码失败：{error}"))?,
        ),
        CharacterEncoding::StandardBase64 => STANDARD.encode(json),
        CharacterEncoding::UrlSafeBase64 { padded } => {
            let encoded = URL_SAFE.encode(json);
            if padded {
                encoded
            } else {
                encoded.trim_end_matches('=').to_owned()
            }
        }
    };
    let replacement = encode_text_chunk(metadata.chunk_kind, &metadata.keyword, &payload)?;
    let mut output = Vec::with_capacity(original.len() + replacement.len());
    output.extend_from_slice(&original[..metadata.chunk_start]);
    output.extend_from_slice(&replacement);
    output.extend_from_slice(&original[metadata.chunk_end..]);
    Ok(output)
}

/// PNG 的 tEXt/zTXt 使用 Latin-1；将 JSON 字符串转成纯 ASCII 可兼容任意语言输入。
fn escape_json_non_ascii(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_ascii() {
            output.push(character);
            continue;
        }
        let code = u32::from(character);
        if code <= 0xffff {
            output.push_str(&format!("\\u{code:04x}"));
        } else {
            let adjusted = code - 0x1_0000;
            let high = 0xd800 + (adjusted >> 10);
            let low = 0xdc00 + (adjusted & 0x3ff);
            output.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
        }
    }
    output
}

fn decode_text_chunk(kind: [u8; 4], data: &[u8]) -> Option<(String, String)> {
    match &kind {
        b"tEXt" => {
            let split = data.iter().position(|byte| *byte == 0)?;
            Some((latin1(&data[..split]), latin1(&data[split + 1..])))
        }
        b"zTXt" => {
            let split = data.iter().position(|byte| *byte == 0)?;
            if data.get(split + 1).copied()? != 0 {
                return None;
            }
            let mut decoder = ZlibDecoder::new(&data[split + 2..]);
            let mut text = Vec::new();
            decoder.read_to_end(&mut text).ok()?;
            Some((latin1(&data[..split]), latin1(&text)))
        }
        b"iTXt" => decode_itxt(data),
        _ => None,
    }
}

fn decode_itxt(data: &[u8]) -> Option<(String, String)> {
    let first = data.iter().position(|byte| *byte == 0)?;
    let keyword = String::from_utf8_lossy(&data[..first]).into_owned();
    let compressed = *data.get(first + 1)? == 1;
    if *data.get(first + 2)? != 0 {
        return None;
    }
    let language_start = first + 3;
    let language_end = data[language_start..].iter().position(|byte| *byte == 0)? + language_start;
    let translated_start = language_end + 1;
    let translated_end = data[translated_start..]
        .iter()
        .position(|byte| *byte == 0)?
        + translated_start;
    let text = &data[translated_end + 1..];
    let text = if compressed {
        let mut decoder = ZlibDecoder::new(text);
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).ok()?;
        decoded
    } else {
        text.to_vec()
    };
    Some((keyword, String::from_utf8(text).ok()?))
}

fn encode_text_chunk(kind: [u8; 4], keyword: &str, text: &str) -> Result<Vec<u8>, String> {
    let data = match &kind {
        b"tEXt" => [keyword.as_bytes(), &[0], text.as_bytes()].concat(),
        b"zTXt" => {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(text.as_bytes())
                .map_err(|error| format!("压缩角色卡元数据失败：{error}"))?;
            let compressed = encoder
                .finish()
                .map_err(|error| format!("压缩角色卡元数据失败：{error}"))?;
            [keyword.as_bytes(), &[0, 0], compressed.as_slice()].concat()
        }
        b"iTXt" => [keyword.as_bytes(), &[0, 0, 0, 0, 0], text.as_bytes()].concat(),
        _ => return Err("角色卡元数据块类型不受支持。".to_owned()),
    };
    let length = u32::try_from(data.len()).map_err(|_| "角色卡元数据过大。".to_owned())?;
    let mut output = Vec::with_capacity(data.len() + 12);
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(&kind);
    output.extend_from_slice(&data);
    let mut crc_input = Vec::with_capacity(data.len() + 4);
    crc_input.extend_from_slice(&kind);
    crc_input.extend_from_slice(&data);
    output.extend_from_slice(&png_crc32(&crc_input).to_be_bytes());
    Ok(output)
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn ensure_backup(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut name = path
        .file_name()
        .ok_or_else(|| "资源文件名无效。".to_owned())?
        .to_os_string();
    name.push(".bak");
    let backup = path.with_file_name(name);
    if backup.exists() {
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&backup)
        .map_err(|error| format!("创建资源备份失败：{error}"))?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&backup);
        return Err(format!("写入资源备份失败：{error}"));
    }
    Ok(())
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = directory.join(format!(
        ".{file_name}.astrabrew-workbench-{}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| format!("原子写入资源失败：{error}"))
}

fn format_issues(issues: Vec<super::ValidationIssue>) -> String {
    issues
        .into_iter()
        .map(|issue| {
            if issue.field_path.is_empty() {
                issue.detail
            } else {
                format!("{}：{}", issue.field_path, issue.detail)
            }
        })
        .collect::<Vec<_>>()
        .join("；")
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

fn pick_string(object: &Map<String, Value>, fields: &[&str]) -> String {
    fields
        .iter()
        .find_map(|field| object.get(*field).and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned()
}

fn set_string_alias(
    object: &mut Map<String, Value>,
    aliases: &[&str],
    fallback: &str,
    value: &str,
) {
    let existing = aliases
        .iter()
        .filter(|key| object.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if existing.is_empty() {
        object.insert(fallback.to_owned(), Value::String(value.to_owned()));
    } else {
        for key in existing {
            object.insert(key.to_owned(), Value::String(value.to_owned()));
        }
    }
}

fn set_array_alias(
    object: &mut Map<String, Value>,
    aliases: &[&str],
    fallback: &str,
    values: Vec<String>,
) {
    let existing = aliases
        .iter()
        .filter(|key| object.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    let value = Value::Array(values.into_iter().map(Value::String).collect());
    if existing.is_empty() {
        object.insert(fallback.to_owned(), value);
    } else {
        for key in existing {
            object.insert(key.to_owned(), value.clone());
        }
    }
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split([',', '，', '\n'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn string_list(object: &Map<String, Value>, aliases: &[&str]) -> String {
    aliases
        .iter()
        .find_map(|key| object.get(*key).and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn number_text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_f64)
        .map_or_else(String::new, |number| {
            if number.fract() == 0.0 {
                format!("{number:.0}")
            } else {
                number.to_string()
            }
        })
}

fn scalar_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(value) if value.is_number() => value.to_string(),
        _ => String::new(),
    }
}

fn set_optional_number(
    object: &mut Map<String, Value>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        object.remove(key);
        return Ok(());
    }
    let parsed = trimmed
        .parse::<f64>()
        .map_err(|_| format!("{key} 必须是数值。"))?;
    let number = Number::from_f64(parsed).ok_or_else(|| format!("{key} 必须是有限数值。"))?;
    object.insert(key.to_owned(), Value::Number(number));
    Ok(())
}

fn set_optional_scalar(object: &mut Map<String, Value>, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        object.remove(key);
    } else if object.get(key).is_some_and(Value::is_string) {
        object.insert(key.to_owned(), Value::String(trimmed.to_owned()));
    } else if let Ok(number) = trimmed.parse::<f64>()
        && let Some(number) = Number::from_f64(number)
    {
        object.insert(key.to_owned(), Value::Number(number));
    } else {
        object.insert(key.to_owned(), Value::String(trimmed.to_owned()));
    }
}

#[derive(Debug, Clone)]
enum WorldLocation {
    Root(String),
    Data(String),
    RootExtension(String),
    DataExtension(String),
}

fn find_world_value<'a>(
    root: &'a Map<String, Value>,
    data: &'a Map<String, Value>,
) -> Option<&'a Map<String, Value>> {
    find_world_location(root)
        .and_then(|location| world_value_at(root, &location))
        .or_else(|| {
            [data, root]
                .into_iter()
                .find_map(|object| object.get("character_book").and_then(Value::as_object))
        })
}

fn find_world_location(root: &Map<String, Value>) -> Option<WorldLocation> {
    let aliases = ["character_book", "worldbook", "world_info", "lorebook"];
    if let Some(key) = aliases.iter().find(|key| root.contains_key(**key)) {
        return Some(WorldLocation::Root((*key).to_owned()));
    }
    if let Some(data) = root.get("data").and_then(Value::as_object)
        && let Some(key) = aliases.iter().find(|key| data.contains_key(**key))
    {
        return Some(WorldLocation::Data((*key).to_owned()));
    }
    for (scope, object) in [
        (false, root),
        (
            true,
            root.get("data").and_then(Value::as_object).unwrap_or(root),
        ),
    ] {
        if let Some(extensions) = object.get("extensions").and_then(Value::as_object) {
            for key in [
                "world",
                "worldbook",
                "character_book",
                "world_info",
                "lorebook",
            ] {
                if extensions
                    .get(key)
                    .and_then(Value::as_object)
                    .is_some_and(|world| world.contains_key("entries"))
                {
                    return Some(if scope {
                        WorldLocation::DataExtension(key.to_owned())
                    } else {
                        WorldLocation::RootExtension(key.to_owned())
                    });
                }
            }
        }
    }
    None
}

fn world_value_at<'a>(
    root: &'a Map<String, Value>,
    location: &WorldLocation,
) -> Option<&'a Map<String, Value>> {
    match location {
        WorldLocation::Root(key) => root.get(key)?.as_object(),
        WorldLocation::Data(key) => root.get("data")?.get(key)?.as_object(),
        WorldLocation::RootExtension(key) => root.get("extensions")?.get(key)?.as_object(),
        WorldLocation::DataExtension(key) => {
            root.get("data")?.get("extensions")?.get(key)?.as_object()
        }
    }
}

fn set_world_value(
    root: &mut Map<String, Value>,
    location: Option<WorldLocation>,
    has_data: bool,
    value: Value,
) -> Result<(), String> {
    match location {
        Some(WorldLocation::Root(key)) => {
            root.insert(key, value);
        }
        Some(WorldLocation::Data(key)) => {
            root.get_mut("data")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "角色卡 data 字段无效。".to_owned())?
                .insert(key, value);
        }
        Some(WorldLocation::RootExtension(key)) => {
            root.get_mut("extensions")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "角色卡 extensions 字段无效。".to_owned())?
                .insert(key, value);
        }
        Some(WorldLocation::DataExtension(key)) => {
            root.get_mut("data")
                .and_then(Value::as_object_mut)
                .and_then(|data| data.get_mut("extensions"))
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "角色卡 data.extensions 字段无效。".to_owned())?
                .insert(key, value);
        }
        None if has_data => {
            root.get_mut("data")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| "角色卡 data 字段无效。".to_owned())?
                .insert("character_book".to_owned(), value);
        }
        None => {
            root.insert("character_book".to_owned(), value);
        }
    }
    Ok(())
}

fn remove_world_value(root: &mut Map<String, Value>, location: Option<WorldLocation>) {
    match location {
        Some(WorldLocation::Root(key)) => {
            root.remove(&key);
        }
        Some(WorldLocation::Data(key)) => {
            if let Some(data) = root.get_mut("data").and_then(Value::as_object_mut) {
                data.remove(&key);
            }
        }
        Some(WorldLocation::RootExtension(key)) => {
            if let Some(extensions) = root.get_mut("extensions").and_then(Value::as_object_mut) {
                extensions.remove(&key);
            }
        }
        Some(WorldLocation::DataExtension(key)) => {
            if let Some(extensions) = root
                .get_mut("data")
                .and_then(Value::as_object_mut)
                .and_then(|data| data.get_mut("extensions"))
                .and_then(Value::as_object_mut)
            {
                extensions.remove(&key);
            }
        }
        None => {}
    }
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn world_book_edit_keeps_aliases_and_unknown_fields() {
        let raw = json!({
            "title": "Lore",
            "unknown": {"keep": true},
            "entries": {
                "42": {
                    "name": "Old",
                    "key": ["alpha"],
                    "keysecondary": ["beta"],
                    "content": "old",
                    "disable": false,
                    "extension": 7
                }
            }
        })
        .as_object()
        .unwrap()
        .clone();
        let mut world = world_book_from_raw(raw).unwrap();
        world.name = "New Lore".to_owned();
        world.entries[0].secondary_keys = "gamma, delta".to_owned();
        world.entries[0].enabled = false;
        let saved = apply_world_book(&world).unwrap();

        assert_eq!(saved["title"], "New Lore");
        assert_eq!(saved["unknown"]["keep"], true);
        assert_eq!(saved["entries"]["42"]["keysecondary"][0], "gamma");
        assert_eq!(saved["entries"]["42"]["disable"], true);
        assert_eq!(saved["entries"]["42"]["extension"], 7);
    }

    #[test]
    fn preset_edit_updates_all_orders_without_removing_extension_references() {
        let mut root = json!({
            "prompts": [
                {"identifier":"main","name":"Main","role":"system","content":"A"},
                {"identifier":"remove","name":"Remove","role":"user","content":"B"},
                {"marker":true}
            ],
            "prompt_order": [
                {"order":[
                    {"identifier":"main","enabled":true},
                    {"identifier":"remove","enabled":true},
                    {"identifier":"extension-only","enabled":true}
                ]},
                {"order":[{"identifier":"main","enabled":false}]}
            ],
            "extensions": {"spreset":{"keep":true}}
        })
        .as_object()
        .unwrap()
        .clone();
        let mut preset = preset_from_raw(&root).unwrap();
        assert!(preset.prompts[0].partial);
        preset.prompts[0].enabled = true;
        preset.prompts[0].enabled_changed = true;
        preset.prompts.remove(1);
        preset.prompts.push(create_preset_prompt(&preset.prompts));
        apply_preset(&mut root, &preset).unwrap();

        let first_order = root["prompt_order"][0]["order"].as_array().unwrap();
        assert!(
            first_order
                .iter()
                .any(|item| item["identifier"] == "extension-only")
        );
        assert!(
            !first_order
                .iter()
                .any(|item| item["identifier"] == "remove")
        );
        assert!(
            first_order.iter().any(|item| {
                item["identifier"] == "astrabrew_prompt_1" && item["enabled"] == true
            })
        );
        assert_eq!(root["extensions"]["spreset"]["keep"], true);
    }

    #[test]
    fn character_png_replacement_keeps_other_chunks_and_accepts_chinese_text() {
        let metadata = STANDARD.encode(r#"{"name":"Astra"}"#);
        let mut original = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut original, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .add_text_chunk("comment".to_owned(), "keep-me".to_owned())
                .unwrap();
            encoder
                .add_text_chunk("chara".to_owned(), metadata)
                .unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0, 0, 0, 255]).unwrap();
        }
        let (mut root, metadata) = extract_character_metadata(&original).unwrap();
        root.insert("name".to_owned(), Value::String("星酿".to_owned()));
        let output = rebuild_character_png(&original, &root, &metadata).unwrap();
        let validated =
            validate_bytes(ResourceKind::CharacterCard, "card.png", output.clone()).unwrap();
        let ResourceData::CharacterCard(card) = validated.data else {
            panic!("expected character card");
        };
        assert_eq!(card.name, "星酿");
        assert!(
            output
                .windows("keep-me".len())
                .any(|bytes| bytes == b"keep-me")
        );
    }

    #[test]
    fn backup_is_created_only_once() {
        let root = std::env::temp_dir().join(format!(
            "astrabrew-workbench-backup-{}-{}",
            std::process::id(),
            super::super::TEMPORARY_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("world.json");
        fs::write(&path, b"original").unwrap();
        ensure_backup(&path, b"original").unwrap();
        ensure_backup(&path, b"changed").unwrap();
        assert_eq!(fs::read(root.join("world.json.bak")).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    }
}
