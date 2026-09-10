//! SillyTavern 角色卡、世界书和预设的统一解析、校验与安全导入。
//!
//! 页面层只传入资源类型、源文件和目标目录；本模块保证校验与写入使用同一份字节，
//! 并通过同目录临时文件和原子重命名避免留下半成品。

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use serde_json::{Map, Value};

const CHARACTER_MAX_BYTES: usize = 64 * 1024 * 1024;
const JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const CHARACTER_TEXT_MAX_BYTES: usize = 16 * 1024 * 1024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// 启动器可管理的资源类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceKind {
    CharacterCard,
    WorldBook,
    Preset,
}

impl ResourceKind {
    const fn expected_extension(self) -> &'static str {
        match self {
            Self::CharacterCard => "png",
            Self::WorldBook | Self::Preset => "json",
        }
    }

    const fn byte_limit(self) -> usize {
        match self {
            Self::CharacterCard => CHARACTER_MAX_BYTES,
            Self::WorldBook | Self::Preset => JSON_MAX_BYTES,
        }
    }
}

/// 校验后识别出的具体资源格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceFormat {
    CharacterCardV1,
    CharacterCardV2,
    CharacterCardV3,
    WorldBookJson,
    PresetJson,
    SPresetJson,
}

impl ResourceFormat {
    /// 返回适合界面展示的稳定格式名称。
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::CharacterCardV1 => "Character Card V1",
            Self::CharacterCardV2 => "Character Card V2",
            Self::CharacterCardV3 => "Character Card V3",
            Self::WorldBookJson => "World Book JSON",
            Self::PresetJson => "Preset JSON",
            Self::SPresetJson => "SPreset JSON",
        }
    }
}

/// 稳定的校验错误分类，界面不需要解析错误字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationCode {
    InvalidExtension,
    FileTooLarge,
    ReadFailed,
    InvalidEncoding,
    InvalidPng,
    MissingMetadata,
    InvalidJson,
    UnrecognizedVersion,
    MissingField,
    InvalidFieldType,
    EmptyEntries,
    TargetConflict,
    WriteFailed,
}

/// 单个可定位到字段的资源校验问题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidationIssue {
    pub code: ValidationCode,
    pub field_path: String,
    pub message_key: &'static str,
    pub detail: String,
}

impl ValidationIssue {
    fn new(
        code: ValidationCode,
        field_path: impl Into<String>,
        message_key: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            field_path: field_path.into(),
            message_key,
            detail: detail.into(),
        }
    }
}

/// 统一后的世界书条目，供资源扫描和详情页复用。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct WorldEntry {
    pub keys: Vec<String>,
    pub secondary_keys: Vec<String>,
    pub content: String,
    pub comment: String,
    pub enabled: bool,
    pub uid: Option<Value>,
    pub order: Option<f64>,
    pub position: Option<Value>,
    pub probability: Option<f64>,
    pub depth: Option<f64>,
}

/// 统一后的世界书信息。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct WorldBookData {
    pub name: String,
    pub author: String,
    pub entries: Vec<WorldEntry>,
}

/// 统一后的角色卡信息。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CharacterCardData {
    pub name: String,
    pub description: String,
    pub creator: String,
    pub version: String,
    pub tags: Vec<String>,
    pub personality: String,
    pub scenario: String,
    pub first_message: String,
    pub example_messages: String,
    pub creator_notes: String,
    pub spec: String,
    pub spec_version: String,
    pub world_book: Option<WorldBookData>,
    pub image_width: u32,
    pub image_height: u32,
}

/// 统一后的预设提示词。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PresetPrompt {
    pub name: String,
    pub role: String,
    pub content: String,
    pub enabled: bool,
    pub marker: bool,
}

/// 统一后的预设信息。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PresetData {
    pub source: String,
    pub model: String,
    pub max_context: i64,
    pub max_tokens: i64,
    pub stream: bool,
    pub prompts: Vec<PresetPrompt>,
    pub has_spreset: bool,
    pub requires_tavern_helper: bool,
}

/// 校验后的类型化资源内容。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResourceData {
    CharacterCard(CharacterCardData),
    WorldBook(WorldBookData),
    Preset(PresetData),
}

/// 已通过完整校验、可安全写入目标目录的资源。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValidatedResource {
    pub kind: ResourceKind,
    pub format: ResourceFormat,
    pub display_name: String,
    pub data: ResourceData,
    bytes: Vec<u8>,
}

/// 批量导入中的单文件结果。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ResourceImportItem {
    pub source: PathBuf,
    pub destination: Option<PathBuf>,
    pub format: Option<ResourceFormat>,
    pub result: Result<ResourceFormat, Vec<ValidationIssue>>,
}

/// 一次批量导入的完整结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ResourceImportReport {
    pub imported: usize,
    pub failed: Vec<ResourceImportItem>,
}

impl ResourceImportReport {
    /// 后台任务异常退出时，为原批次中的每个源文件生成明确失败项。
    pub(crate) fn task_failed(sources: Vec<PathBuf>, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        Self {
            imported: 0,
            failed: sources
                .into_iter()
                .map(|source| ResourceImportItem {
                    source,
                    destination: None,
                    format: None,
                    result: Err(vec![ValidationIssue::new(
                        ValidationCode::ReadFailed,
                        "",
                        "resources.validation.task_failed",
                        detail.clone(),
                    )]),
                })
                .collect(),
        }
    }
}

/// 从文件路径读取并校验资源；文件内容只读取一次。
pub(crate) fn validate_path(
    kind: ResourceKind,
    path: &Path,
) -> Result<ValidatedResource, Vec<ValidationIssue>> {
    validate_extension(kind, path.file_name().unwrap_or_default())?;
    let metadata = fs::metadata(path).map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::ReadFailed,
            "",
            "resources.validation.read_failed",
            error.to_string(),
        )]
    })?;
    if !metadata.is_file() {
        return Err(vec![ValidationIssue::new(
            ValidationCode::ReadFailed,
            "",
            "resources.validation.read_failed",
            "所选路径不是普通文件。",
        )]);
    }
    if metadata.len() > kind.byte_limit() as u64 {
        return Err(vec![file_too_large_issue(kind, metadata.len())]);
    }
    let bytes = fs::read(path).map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::ReadFailed,
            "",
            "resources.validation.read_failed",
            error.to_string(),
        )]
    })?;
    validate_bytes(
        kind,
        &path.file_name().unwrap_or_default().to_string_lossy(),
        bytes,
    )
}

/// 校验内存中的资源，并返回类型化结果和用于落盘的同一份字节。
pub(crate) fn validate_bytes(
    kind: ResourceKind,
    file_name: &str,
    bytes: Vec<u8>,
) -> Result<ValidatedResource, Vec<ValidationIssue>> {
    validate_extension(kind, OsStr::new(file_name))?;
    if bytes.len() > kind.byte_limit() {
        return Err(vec![file_too_large_issue(kind, bytes.len() as u64)]);
    }

    match kind {
        ResourceKind::CharacterCard => validate_character(bytes),
        ResourceKind::WorldBook => validate_world_book(bytes),
        ResourceKind::Preset => validate_preset(bytes, file_name),
    }
}

/// 逐文件校验并导入；单个失败不会阻止同批次中的其他有效文件。
pub(crate) fn import_batch(
    kind: ResourceKind,
    sources: Vec<PathBuf>,
    destination: &Path,
) -> ResourceImportReport {
    let mut report = ResourceImportReport::default();
    if let Err(error) = fs::create_dir_all(destination) {
        for source in sources {
            report.failed.push(ResourceImportItem {
                source,
                destination: None,
                format: None,
                result: Err(vec![ValidationIssue::new(
                    ValidationCode::WriteFailed,
                    "",
                    "resources.validation.write_failed",
                    error.to_string(),
                )]),
            });
        }
        return report;
    }

    for source in sources {
        let validated = match validate_path(kind, &source) {
            Ok(validated) => validated,
            Err(issues) => {
                report.failed.push(ResourceImportItem {
                    source,
                    destination: None,
                    format: None,
                    result: Err(issues),
                });
                continue;
            }
        };
        let Some(file_name) = source.file_name() else {
            report.failed.push(ResourceImportItem {
                source,
                destination: None,
                format: Some(validated.format),
                result: Err(vec![ValidationIssue::new(
                    ValidationCode::ReadFailed,
                    "",
                    "resources.validation.read_failed",
                    "源文件缺少有效文件名。",
                )]),
            });
            continue;
        };
        let target = destination.join(file_name);
        if target.exists() {
            report.failed.push(ResourceImportItem {
                source,
                destination: Some(target),
                format: Some(validated.format),
                result: Err(vec![ValidationIssue::new(
                    ValidationCode::TargetConflict,
                    "",
                    "resources.validation.target_conflict",
                    "同名资源已经存在。",
                )]),
            });
            continue;
        }
        match atomic_write_new(&target, &validated.bytes) {
            Ok(()) => report.imported += 1,
            Err(error) => report.failed.push(ResourceImportItem {
                source,
                destination: Some(target),
                format: Some(validated.format),
                result: Err(vec![if error.kind() == std::io::ErrorKind::AlreadyExists {
                    ValidationIssue::new(
                        ValidationCode::TargetConflict,
                        "",
                        "resources.validation.target_conflict",
                        error.to_string(),
                    )
                } else {
                    ValidationIssue::new(
                        ValidationCode::WriteFailed,
                        "",
                        "resources.validation.write_failed",
                        error.to_string(),
                    )
                }]),
            }),
        }
    }
    report
}

fn validate_extension(kind: ResourceKind, file_name: &OsStr) -> Result<(), Vec<ValidationIssue>> {
    let matches = Path::new(file_name)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(kind.expected_extension()));
    if matches {
        Ok(())
    } else {
        Err(vec![ValidationIssue::new(
            ValidationCode::InvalidExtension,
            "",
            "resources.validation.invalid_extension",
            format!("仅支持 .{} 文件。", kind.expected_extension()),
        )])
    }
}

fn file_too_large_issue(kind: ResourceKind, actual: u64) -> ValidationIssue {
    ValidationIssue::new(
        ValidationCode::FileTooLarge,
        "",
        "resources.validation.file_too_large",
        format!(
            "文件大小为 {actual} 字节，上限为 {} 字节。",
            kind.byte_limit()
        ),
    )
}

fn validate_character(bytes: Vec<u8>) -> Result<ValidatedResource, Vec<ValidationIssue>> {
    if !bytes.starts_with(PNG_SIGNATURE) {
        return Err(vec![ValidationIssue::new(
            ValidationCode::InvalidPng,
            "",
            "resources.validation.invalid_png",
            "文件签名不是 PNG。",
        )]);
    }

    let cursor = Cursor::new(bytes.as_slice());
    let decoder = png::Decoder::new(cursor);
    let mut reader = decoder.read_info().map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::InvalidPng,
            "",
            "resources.validation.invalid_png",
            error.to_string(),
        )]
    })?;
    reader.finish().map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::InvalidPng,
            "",
            "resources.validation.invalid_png",
            error.to_string(),
        )]
    })?;

    let info = reader.info();
    let mut candidates = Vec::new();
    for chunk in &info.uncompressed_latin1_text {
        candidates.push((chunk.keyword.clone(), chunk.text.clone()));
    }
    for chunk in &info.compressed_latin1_text {
        let mut chunk = chunk.clone();
        chunk
            .decompress_text_with_limit(CHARACTER_TEXT_MAX_BYTES)
            .map_err(|error| {
                vec![ValidationIssue::new(
                    ValidationCode::InvalidPng,
                    "metadata",
                    "resources.validation.invalid_png_text",
                    error.to_string(),
                )]
            })?;
        let text = chunk.get_text().map_err(|error| {
            vec![ValidationIssue::new(
                ValidationCode::InvalidPng,
                "metadata",
                "resources.validation.invalid_png_text",
                error.to_string(),
            )]
        })?;
        if text.len() > CHARACTER_TEXT_MAX_BYTES {
            return Err(vec![ValidationIssue::new(
                ValidationCode::FileTooLarge,
                "metadata",
                "resources.validation.file_too_large",
                "解压后的角色卡元数据超过 16 MiB。",
            )]);
        }
        candidates.push((chunk.keyword.clone(), text));
    }
    for chunk in &info.utf8_text {
        let mut chunk = chunk.clone();
        chunk
            .decompress_text_with_limit(CHARACTER_TEXT_MAX_BYTES)
            .map_err(|error| {
                vec![ValidationIssue::new(
                    ValidationCode::InvalidPng,
                    "metadata",
                    "resources.validation.invalid_png_text",
                    error.to_string(),
                )]
            })?;
        let text = chunk.get_text().map_err(|error| {
            vec![ValidationIssue::new(
                ValidationCode::InvalidPng,
                "metadata",
                "resources.validation.invalid_png_text",
                error.to_string(),
            )]
        })?;
        if text.len() > CHARACTER_TEXT_MAX_BYTES {
            return Err(vec![ValidationIssue::new(
                ValidationCode::FileTooLarge,
                "metadata",
                "resources.validation.file_too_large",
                "解压后的角色卡元数据超过 16 MiB。",
            )]);
        }
        candidates.push((chunk.keyword.clone(), text));
    }
    if candidates.is_empty() {
        return Err(vec![ValidationIssue::new(
            ValidationCode::MissingMetadata,
            "metadata",
            "resources.validation.missing_metadata",
            "PNG 中没有文本元数据块。",
        )]);
    }

    candidates.retain(|(keyword, _)| character_keyword_rank(keyword) != usize::MAX);
    candidates.sort_by_key(|(keyword, _)| character_keyword_rank(keyword));
    let Some((root, keyword)) = candidates.iter().find_map(|(keyword, text)| {
        parse_json_candidate(text).map(|value| (value, keyword.as_str()))
    }) else {
        return Err(vec![ValidationIssue::new(
            ValidationCode::MissingMetadata,
            "metadata",
            "resources.validation.missing_metadata",
            "PNG 文本块中没有可解析的角色卡 JSON。",
        )]);
    };
    let mut issues = Vec::new();
    let object = &root;
    validate_optional_strings(object, &["spec", "spec_version"], "", &mut issues);
    if object
        .get("data")
        .is_some_and(|value| !value.is_object() && !value.is_null())
    {
        issues.push(invalid_type("data", "角色卡 data 必须是对象。"));
    }
    validate_character_world_types(object, "", &mut issues);
    let format = detect_character_format(object, keyword)?;
    let data = object
        .get("data")
        .and_then(Value::as_object)
        .unwrap_or(object);
    if !std::ptr::eq(data, object) {
        validate_character_world_types(data, "data", &mut issues);
    }
    validate_optional_strings(
        data,
        &[
            "name",
            "description",
            "personality",
            "scenario",
            "first_mes",
            "firstMessage",
            "mes_example",
            "example_messages",
            "creator",
            "creator_notes",
            "creatorcomment",
            "character_version",
            "version",
        ],
        "data",
        &mut issues,
    );
    validate_string_array(data, "tags", "data.tags", &mut issues);
    let name = pick_string(data, &["name"]);
    if name.trim().is_empty() {
        issues.push(ValidationIssue::new(
            ValidationCode::MissingField,
            "data.name",
            "resources.validation.missing_field",
            "角色名称不能为空。",
        ));
    }

    let world_value = find_character_world(object, data);
    let world_book = world_value
        .and_then(|value| validate_world_object(value, "data.character_book", &mut issues, true));
    if !issues.is_empty() {
        return Err(issues);
    }

    let card = CharacterCardData {
        name: name.clone(),
        description: pick_string(data, &["description"]),
        creator: pick_string(data, &["creator"]),
        version: pick_string(data, &["character_version", "version"]),
        tags: string_array(data.get("tags")),
        personality: pick_string(data, &["personality"]),
        scenario: pick_string(data, &["scenario"]),
        first_message: pick_string(data, &["first_mes", "firstMessage"]),
        example_messages: pick_string(data, &["mes_example", "example_messages"]),
        creator_notes: pick_string(data, &["creator_notes", "creatorcomment"]),
        spec: pick_string(object, &["spec"]),
        spec_version: pick_string(object, &["spec_version"]),
        world_book,
        image_width: info.width,
        image_height: info.height,
    };
    Ok(ValidatedResource {
        kind: ResourceKind::CharacterCard,
        format,
        display_name: name,
        data: ResourceData::CharacterCard(card),
        bytes,
    })
}

fn detect_character_format(
    object: &Map<String, Value>,
    keyword: &str,
) -> Result<ResourceFormat, Vec<ValidationIssue>> {
    let spec = pick_string(object, &["spec"]).to_ascii_lowercase();
    let version = pick_string(object, &["spec_version"]);
    if spec.contains("v3") || version.starts_with('3') || keyword.eq_ignore_ascii_case("ccv3") {
        return Ok(ResourceFormat::CharacterCardV3);
    }
    if spec.contains("v2") || version.starts_with('2') {
        return Ok(ResourceFormat::CharacterCardV2);
    }
    if !spec.is_empty() || !version.is_empty() {
        return Err(vec![ValidationIssue::new(
            ValidationCode::UnrecognizedVersion,
            "spec_version",
            "resources.validation.unrecognized_version",
            format!("无法识别 spec={spec:?}, spec_version={version:?}。"),
        )]);
    }
    if object.get("data").is_some() {
        return Ok(ResourceFormat::CharacterCardV2);
    }
    Ok(ResourceFormat::CharacterCardV1)
}

fn validate_world_book(bytes: Vec<u8>) -> Result<ValidatedResource, Vec<ValidationIssue>> {
    let normalized = normalize_json_bytes(&bytes)?;
    let root = parse_json_object(&normalized)?;
    let mut issues = Vec::new();
    let Some(world) = validate_world_object(&root, "", &mut issues, true) else {
        return Err(issues);
    };
    if !issues.is_empty() {
        return Err(issues);
    }
    let display_name = if world.name.trim().is_empty() {
        "未命名世界书".to_owned()
    } else {
        world.name.clone()
    };
    Ok(ValidatedResource {
        kind: ResourceKind::WorldBook,
        format: ResourceFormat::WorldBookJson,
        display_name,
        data: ResourceData::WorldBook(world),
        bytes: normalized,
    })
}

fn validate_world_object(
    value: &Map<String, Value>,
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
    require_entries: bool,
) -> Option<WorldBookData> {
    validate_optional_strings(
        value,
        &["name", "title", "world_name", "author", "creator"],
        prefix,
        issues,
    );
    let entries_path = join_path(prefix, "entries");
    let entries_value = value.get("entries");
    if entries_value.is_none() {
        if require_entries {
            issues.push(ValidationIssue::new(
                ValidationCode::MissingField,
                entries_path,
                "resources.validation.missing_field",
                "世界书缺少 entries。",
            ));
        }
        return None;
    }
    let entries_value = entries_value.expect("上方已经检查 entries 存在");
    let indexed: Vec<(String, &Value)> = match entries_value {
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value))
            .collect(),
        Value::Object(items) => items
            .iter()
            .map(|(key, value)| (key.clone(), value))
            .collect(),
        _ => {
            issues.push(invalid_type(&entries_path, "entries 必须是数组或对象。"));
            return None;
        }
    };
    if indexed.is_empty() {
        issues.push(ValidationIssue::new(
            ValidationCode::EmptyEntries,
            entries_path,
            "resources.validation.empty_entries",
            "世界书至少需要一个条目。",
        ));
        return None;
    }

    let mut entries = Vec::with_capacity(indexed.len());
    for (key, entry) in indexed {
        let path = format!("{}[{key}]", join_path(prefix, "entries"));
        if let Some(entry) = validate_world_entry(entry, &path, issues) {
            entries.push(entry);
        }
    }
    Some(WorldBookData {
        name: pick_string(value, &["name", "title", "world_name"]),
        author: pick_string(value, &["author", "creator"]),
        entries,
    })
}

fn validate_world_entry(
    value: &Value,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) -> Option<WorldEntry> {
    let Some(object) = value.as_object() else {
        issues.push(invalid_type(path, "世界书条目必须是对象。"));
        return None;
    };
    validate_optional_strings(object, &["content", "comment", "name"], path, issues);
    validate_string_array_alias(object, &["keys", "key"], path, issues);
    validate_string_array(
        object,
        "secondary_keys",
        &join_path(path, "secondary_keys"),
        issues,
    );
    validate_optional_bool(object, "enabled", path, issues);
    validate_optional_bool(object, "disable", path, issues);
    validate_number(object, "order", path, issues);
    validate_number(object, "probability", path, issues);
    validate_number(object, "depth", path, issues);
    validate_number_or_string(object, "uid", path, issues);
    validate_number_or_string(object, "position", path, issues);

    Some(WorldEntry {
        keys: object
            .get("keys")
            .or_else(|| object.get("key"))
            .map(string_array_value)
            .unwrap_or_default(),
        secondary_keys: object
            .get("secondary_keys")
            .map(string_array_value)
            .unwrap_or_default(),
        content: pick_string(object, &["content"]),
        comment: pick_string(object, &["comment", "name"]),
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                !object
                    .get("disable")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            }),
        uid: object.get("uid").cloned(),
        order: object.get("order").and_then(Value::as_f64),
        position: object.get("position").cloned(),
        probability: object.get("probability").and_then(Value::as_f64),
        depth: object.get("depth").and_then(Value::as_f64),
    })
}

fn validate_preset(
    bytes: Vec<u8>,
    file_name: &str,
) -> Result<ValidatedResource, Vec<ValidationIssue>> {
    let normalized = normalize_json_bytes(&bytes)?;
    let root = parse_json_object(&normalized)?;
    let mut issues = Vec::new();
    let source = pick_string(&root, &["chat_completion_source"]);
    let model = resolve_preset_model(&root, &source);
    validate_optional_strings(&root, &preset_string_fields(), "", &mut issues);
    for field in preset_number_fields() {
        validate_number(&root, field, "", &mut issues);
    }
    for field in ["stream_openai", "wrap_in_quotes"] {
        validate_optional_bool(&root, field, "", &mut issues);
    }
    validate_number_or_string(&root, "names_behavior", "", &mut issues);
    validate_optional_strings(&root, &["send_if_empty"], "", &mut issues);

    let prompts = match root.get("prompts") {
        Some(Value::Array(items)) => {
            if items.is_empty() {
                issues.push(ValidationIssue::new(
                    ValidationCode::EmptyEntries,
                    "prompts",
                    "resources.validation.empty_entries",
                    "预设 prompts 不能为空数组。",
                ));
            }
            items
                .iter()
                .enumerate()
                .filter_map(|(index, value)| validate_preset_prompt(value, index, &mut issues))
                .collect()
        }
        Some(_) => {
            issues.push(invalid_type("prompts", "prompts 必须是数组。"));
            Vec::new()
        }
        None => Vec::new(),
    };

    let mut has_spreset = false;
    let mut requires_tavern_helper = false;
    if let Some(extensions) = root.get("extensions") {
        let Some(extensions) = extensions.as_object() else {
            issues.push(invalid_type("extensions", "extensions 必须是对象。"));
            return Err(issues);
        };
        for key in extensions.keys() {
            match key.to_ascii_lowercase().as_str() {
                "spreset" => {
                    has_spreset = true;
                    requires_tavern_helper = true;
                }
                "tavern_helper" | "tavernhelper" | "tavern helper" => {
                    requires_tavern_helper = true;
                }
                _ => {}
            }
        }
    }

    let has_parameter = preset_number_fields()
        .iter()
        .any(|field| root.contains_key(*field));
    let has_model_feature = !source.trim().is_empty() || !model.trim().is_empty();
    if prompts.is_empty() && !has_parameter && !has_model_feature && !has_spreset {
        issues.push(ValidationIssue::new(
            ValidationCode::MissingField,
            "",
            "resources.validation.unrecognized_resource",
            "JSON 中没有可识别的预设字段。",
        ));
    }
    if !issues.is_empty() {
        return Err(issues);
    }

    let format = if has_spreset {
        ResourceFormat::SPresetJson
    } else {
        ResourceFormat::PresetJson
    };
    Ok(ValidatedResource {
        kind: ResourceKind::Preset,
        format,
        display_name: Path::new(file_name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        data: ResourceData::Preset(PresetData {
            source,
            model,
            max_context: root
                .get("openai_max_context")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            max_tokens: root
                .get("openai_max_tokens")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            stream: root
                .get("stream_openai")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            prompts,
            has_spreset,
            requires_tavern_helper,
        }),
        bytes: normalized,
    })
}

fn validate_preset_prompt(
    value: &Value,
    index: usize,
    issues: &mut Vec<ValidationIssue>,
) -> Option<PresetPrompt> {
    let path = format!("prompts[{index}]");
    let Some(object) = value.as_object() else {
        issues.push(invalid_type(&path, "提示词条目必须是对象。"));
        return None;
    };
    validate_optional_strings(
        object,
        &["name", "role", "content", "identifier"],
        &path,
        issues,
    );
    validate_optional_bool(object, "enabled", &path, issues);
    validate_optional_bool(object, "marker", &path, issues);
    let marker = object
        .get("marker")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !marker {
        for (field, value, detail) in [
            (
                "name",
                pick_string(object, &["name", "identifier"]),
                "普通提示词必须包含 name 或 identifier。",
            ),
            (
                "role",
                pick_string(object, &["role"]),
                "普通提示词必须包含 role。",
            ),
        ] {
            if value.trim().is_empty() {
                issues.push(ValidationIssue::new(
                    ValidationCode::MissingField,
                    join_path(&path, field),
                    "resources.validation.missing_field",
                    detail,
                ));
            }
        }
        if !object.contains_key("content") {
            issues.push(ValidationIssue::new(
                ValidationCode::MissingField,
                join_path(&path, "content"),
                "resources.validation.missing_field",
                "普通提示词必须包含 content。",
            ));
        }
    }
    Some(PresetPrompt {
        name: pick_string(object, &["name", "identifier"]),
        role: pick_string(object, &["role"]),
        content: pick_string(object, &["content"]),
        enabled: object
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        marker,
    })
}

fn parse_json_object(bytes: &[u8]) -> Result<Map<String, Value>, Vec<ValidationIssue>> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::InvalidJson,
            "",
            "resources.validation.invalid_json",
            error.to_string(),
        )]
    })?;
    value.as_object().cloned().ok_or_else(|| {
        vec![ValidationIssue::new(
            ValidationCode::InvalidJson,
            "",
            "resources.validation.invalid_json_root",
            "JSON 根节点必须是对象。",
        )]
    })
}

fn normalize_json_bytes(bytes: &[u8]) -> Result<Vec<u8>, Vec<ValidationIssue>> {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|error| {
        vec![ValidationIssue::new(
            ValidationCode::InvalidEncoding,
            "",
            "resources.validation.invalid_encoding",
            error.to_string(),
        )]
    })?;
    Ok(bytes.to_vec())
}

fn parse_json_candidate(text: &str) -> Option<Map<String, Value>> {
    let trimmed = text.trim();
    if let Ok(Value::Object(object)) = serde_json::from_str(trimmed) {
        return Some(object);
    }
    let normalized = normalize_base64(trimmed)?;
    for engine in [&STANDARD, &URL_SAFE] {
        if let Ok(decoded) = engine.decode(normalized.as_bytes())
            && let Ok(Value::Object(object)) = serde_json::from_slice(&decoded)
        {
            return Some(object);
        }
    }
    None
}

fn normalize_base64(value: &str) -> Option<String> {
    let mut normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if normalized.is_empty()
        || !normalized.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'_' | b'-' | b'=')
        })
    {
        return None;
    }
    match normalized.len() % 4 {
        0 => {}
        2 => normalized.push_str("=="),
        3 => normalized.push('='),
        _ => return None,
    }
    Some(normalized)
}

fn character_keyword_rank(keyword: &str) -> usize {
    let keyword = keyword.to_ascii_lowercase();
    ["chara", "ccv3", "character", "card"]
        .iter()
        .position(|candidate| keyword.contains(candidate))
        .unwrap_or(usize::MAX)
}

fn find_character_world<'a>(
    root: &'a Map<String, Value>,
    data: &'a Map<String, Value>,
) -> Option<&'a Map<String, Value>> {
    const KEYS: &[&str] = &["character_book", "worldbook", "world_info", "lorebook"];
    for object in [data, root] {
        for key in KEYS {
            if let Some(value) = object.get(*key).and_then(Value::as_object) {
                return Some(value);
            }
        }
    }
    for object in [data, root] {
        if let Some(extensions) = object.get("extensions").and_then(Value::as_object) {
            for key in ["world", "worldbook", "character_book", "world_info", "lorebook"] {
                if let Some(value) = extensions
                    .get(key)
                    .and_then(Value::as_object)
                    .filter(|value| value.contains_key("entries"))
                {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// 顶层已知世界书别名若存在就必须是对象，扩展字段仍按原样兼容保留。
fn validate_character_world_types(
    object: &Map<String, Value>,
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for key in ["character_book", "worldbook", "world_info", "lorebook"] {
        if object
            .get(key)
            .is_some_and(|value| !value.is_object() && !value.is_null())
        {
            issues.push(invalid_type(
                &join_path(prefix, key),
                "角色卡内嵌世界书必须是对象。",
            ));
        }
    }
}

fn validate_optional_strings(
    object: &Map<String, Value>,
    fields: &[&str],
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for field in fields {
        if object
            .get(*field)
            .is_some_and(|value| !value.is_string() && !value.is_null())
        {
            issues.push(invalid_type(
                &join_path(prefix, field),
                "字段必须是字符串。",
            ));
        }
    }
}

fn validate_string_array(
    object: &Map<String, Value>,
    field: &str,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if let Some(value) = object.get(field)
        && (!value.is_array()
            || value
                .as_array()
                .is_some_and(|items| items.iter().any(|item| !item.is_string())))
    {
        issues.push(invalid_type(path, "字段必须是字符串数组。"));
    }
}

fn validate_string_array_alias(
    object: &Map<String, Value>,
    fields: &[&str],
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for field in fields {
        validate_string_array(object, field, &join_path(prefix, field), issues);
    }
}

fn validate_optional_bool(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if object
        .get(field)
        .is_some_and(|value| !value.is_boolean() && !value.is_null())
    {
        issues.push(invalid_type(
            &join_path(prefix, field),
            "字段必须是布尔值。",
        ));
    }
}

fn validate_number(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if object
        .get(field)
        .is_some_and(|value| !value.is_number() && !value.is_null())
    {
        issues.push(invalid_type(
            &join_path(prefix, field),
            "字段必须是有限数值。",
        ));
    }
}

fn validate_number_or_string(
    object: &Map<String, Value>,
    field: &str,
    prefix: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if object
        .get(field)
        .is_some_and(|value| !value.is_number() && !value.is_string() && !value.is_null())
    {
        issues.push(invalid_type(
            &join_path(prefix, field),
            "字段必须是数值或字符串。",
        ));
    }
}

fn invalid_type(path: &str, detail: &str) -> ValidationIssue {
    ValidationIssue::new(
        ValidationCode::InvalidFieldType,
        path,
        "resources.validation.invalid_field_type",
        detail,
    )
}

fn join_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_owned()
    } else {
        format!("{prefix}.{field}")
    }
}

fn pick_string(object: &Map<String, Value>, fields: &[&str]) -> String {
    fields
        .iter()
        .find_map(|field| {
            object
                .get(*field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default()
        .to_owned()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value.map(string_array_value).unwrap_or_default()
}

fn string_array_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn preset_string_fields() -> [&'static str; 14] {
    [
        "chat_completion_source",
        "openai_model",
        "claude_model",
        "windowai_model",
        "openrouter_model",
        "ai21_model",
        "mistralai_model",
        "cohere_model",
        "perplexity_model",
        "groq_model",
        "zerooneai_model",
        "blockentropy_model",
        "custom_model",
        "google_model",
    ]
}

fn preset_number_fields() -> [&'static str; 15] {
    [
        "temperature",
        "top_p",
        "top_k",
        "top_k_openai",
        "min_p",
        "min_p_openai",
        "top_a_openai",
        "openai_max_context",
        "openai_max_tokens",
        "seed",
        "frequency_penalty",
        "presence_penalty",
        "repetition_penalty",
        "repetition_penalty_openai",
        "typical_p_openai",
    ]
}

fn resolve_preset_model(root: &Map<String, Value>, source: &str) -> String {
    let field = match source.to_ascii_lowercase().as_str() {
        "openai" => Some("openai_model"),
        "claude" => Some("claude_model"),
        "windowai" => Some("windowai_model"),
        "openrouter" => Some("openrouter_model"),
        "ai21" => Some("ai21_model"),
        "mistralai" => Some("mistralai_model"),
        "cohere" => Some("cohere_model"),
        "perplexity" => Some("perplexity_model"),
        "groq" => Some("groq_model"),
        "zerooneai" => Some("zerooneai_model"),
        "blockentropy" => Some("blockentropy_model"),
        "custom" => Some("custom_model"),
        "google" => Some("google_model"),
        _ => None,
    };
    field
        .and_then(|field| root.get(field).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| pick_string(root, &preset_string_fields()[1..]))
}

fn atomic_write_new(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target.file_name().unwrap_or_default().to_string_lossy();
    let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(
        ".{file_name}.astrabrew-import-{}-{sequence}.tmp",
        std::process::id()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // 同目录硬链接只在目标不存在时成功，既保持原子可见性，也不会覆盖竞态中出现的文件。
        fs::hard_link(&temporary, target)?;
        if let Err(error) = fs::remove_file(&temporary) {
            // 发布后的清理失败时回滚目标，避免把一次失败同时表现为已导入。
            let _ = fs::remove_file(target);
            return Err(error);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_with_text(keyword: &str, text: &str, kind: &str) -> Vec<u8> {
        let mut output = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut output, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            match kind {
                "ztxt" => encoder
                    .add_ztxt_chunk(keyword.to_owned(), text.to_owned())
                    .unwrap(),
                "itxt" => encoder
                    .add_itxt_chunk(keyword.to_owned(), text.to_owned())
                    .unwrap(),
                "compressed_itxt" => {}
                _ => encoder
                    .add_text_chunk(keyword.to_owned(), text.to_owned())
                    .unwrap(),
            }
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0, 0, 0, 255]).unwrap();
            if kind == "compressed_itxt" {
                let mut chunk = png::text_metadata::ITXtChunk::new(keyword, text);
                chunk.compress_text().unwrap();
                writer.write_text_chunk(&chunk).unwrap();
            }
        }
        output
    }

    #[test]
    fn validates_character_versions_and_png_text_kinds() {
        let v1 = STANDARD.encode(r#"{"name":"Astra"}"#);
        let v2 = STANDARD
            .encode(r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"Brew"}}"#);
        let v3 = URL_SAFE
            .encode(r#"{"spec":"chara_card_v3","spec_version":"3.0","data":{"name":"Nova"}}"#)
            .trim_end_matches('=')
            .to_owned();
        for (text, chunk, expected) in [
            (v1, "text", ResourceFormat::CharacterCardV1),
            (v2, "ztxt", ResourceFormat::CharacterCardV2),
            (v3.clone(), "itxt", ResourceFormat::CharacterCardV3),
            (v3, "compressed_itxt", ResourceFormat::CharacterCardV3),
        ] {
            let result = validate_bytes(
                ResourceKind::CharacterCard,
                "card.png",
                png_with_text("chara", &text, chunk),
            )
            .unwrap();
            assert_eq!(result.format, expected);
        }
    }

    #[test]
    fn rejects_png_without_character_metadata() {
        let issues = validate_bytes(
            ResourceKind::CharacterCard,
            "image.png",
            png_with_text("comment", "plain image", "text"),
        )
        .unwrap_err();
        assert_eq!(issues[0].code, ValidationCode::MissingMetadata);
    }

    #[test]
    fn rejects_json_in_unrelated_png_text_chunks() {
        let issues = validate_bytes(
            ResourceKind::CharacterCard,
            "image.png",
            png_with_text("comment", r#"{"name":"Astra"}"#, "text"),
        )
        .unwrap_err();
        assert_eq!(issues[0].code, ValidationCode::MissingMetadata);
    }

    #[test]
    fn keeps_optional_character_extensions_compatible() {
        let metadata = STANDARD.encode(
            r#"{"name":"Astra","extensions":{"world":"Astra","depth_prompt":{"prompt":"","depth":4,"role":"system"}}}"#,
        );
        let valid = validate_bytes(
            ResourceKind::CharacterCard,
            "card.png",
            png_with_text("chara", &metadata, "text"),
        )
        .unwrap();
        assert_eq!(valid.format, ResourceFormat::CharacterCardV1);

        let world_book = STANDARD
            .encode(r#"{"name":"Astra","extensions":{"worldbook":{"entries":[]}}}"#);
        let issues = validate_bytes(
            ResourceKind::CharacterCard,
            "card.png",
            png_with_text("chara", &world_book, "text"),
        )
        .unwrap_err();
        assert!(issues
            .iter()
            .any(|issue| issue.code == ValidationCode::EmptyEntries));
    }

    #[test]
    fn rejects_invalid_png_missing_name_and_wrong_character_types() {
        let invalid_png = validate_bytes(
            ResourceKind::CharacterCard,
            "card.png",
            [PNG_SIGNATURE.as_slice(), b"broken"].concat(),
        )
        .unwrap_err();
        assert_eq!(invalid_png[0].code, ValidationCode::InvalidPng);

        let missing_name = STANDARD.encode(r#"{"spec":"chara_card_v2","data":{}}"#);
        let issues = validate_bytes(
            ResourceKind::CharacterCard,
            "card.png",
            png_with_text("chara", &missing_name, "text"),
        )
        .unwrap_err();
        assert!(issues.iter().any(|issue| issue.field_path == "data.name"));

        let wrong_tags = STANDARD.encode(r#"{"name":"Astra","tags":"not-an-array"}"#);
        let issues = validate_bytes(
            ResourceKind::CharacterCard,
            "card.png",
            png_with_text("chara", &wrong_tags, "text"),
        )
        .unwrap_err();
        assert!(issues.iter().any(|issue| issue.field_path == "data.tags"));
    }

    #[test]
    fn validates_world_book_array_and_object_entries() {
        for json in [
            br#"{"name":"Lore","entries":[{"keys":["A"],"content":"B","enabled":true}]}"#
                .as_slice(),
            br#"{"entries":{"0":{"key":["A"],"content":"B","disable":false}}}"#.as_slice(),
        ] {
            let result =
                validate_bytes(ResourceKind::WorldBook, "world.json", json.to_vec()).unwrap();
            assert_eq!(result.format, ResourceFormat::WorldBookJson);
        }
    }

    #[test]
    fn rejects_empty_world_book_and_invalid_entry_type() {
        let empty = validate_bytes(
            ResourceKind::WorldBook,
            "world.json",
            br#"{"entries":[]}"#.to_vec(),
        )
        .unwrap_err();
        assert_eq!(empty[0].code, ValidationCode::EmptyEntries);
        let invalid = validate_bytes(
            ResourceKind::WorldBook,
            "world.json",
            br#"{"entries":[false]}"#.to_vec(),
        )
        .unwrap_err();
        assert_eq!(invalid[0].field_path, "entries[0]");
    }

    #[test]
    fn accepts_world_book_bom_and_unknown_fields_but_rejects_invalid_utf8() {
        let mut bom = b"\xef\xbb\xbf".to_vec();
        bom.extend_from_slice(
            br#"{"unknown":{"kept":true},"entries":[{"keys":["A"],"content":"B","extension_value":1}]}"#,
        );
        let validated = validate_bytes(ResourceKind::WorldBook, "world.json", bom).unwrap();
        assert_eq!(validated.format, ResourceFormat::WorldBookJson);
        assert!(!validated.bytes.starts_with(b"\xef\xbb\xbf"));

        let invalid = validate_bytes(
            ResourceKind::WorldBook,
            "world.json",
            vec![0xff, 0xfe, 0xfd],
        )
        .unwrap_err();
        assert_eq!(invalid[0].code, ValidationCode::InvalidEncoding);
    }

    #[test]
    fn validates_standard_and_spreset_presets() {
        let standard = validate_bytes(ResourceKind::Preset, "preset.json", br#"{"chat_completion_source":"openai","openai_model":"gpt","prompts":[{"name":"System","role":"system","content":"Hello"}]}"#.to_vec()).unwrap();
        assert_eq!(standard.format, ResourceFormat::PresetJson);
        let spreset = validate_bytes(
            ResourceKind::Preset,
            "preset.json",
            br#"{"extensions":{"spreset":{}},"prompts":[{"marker":true}]}"#.to_vec(),
        )
        .unwrap();
        assert_eq!(spreset.format, ResourceFormat::SPresetJson);
    }

    #[test]
    fn rejects_unrelated_json_and_wrong_known_types() {
        let unrelated = validate_bytes(
            ResourceKind::Preset,
            "preset.json",
            br#"{"hello":"world"}"#.to_vec(),
        )
        .unwrap_err();
        assert_eq!(unrelated[0].code, ValidationCode::MissingField);
        let invalid = validate_bytes(
            ResourceKind::Preset,
            "preset.json",
            br#"{"temperature":"hot"}"#.to_vec(),
        )
        .unwrap_err();
        assert_eq!(invalid[0].field_path, "temperature");
    }

    #[test]
    fn rejects_files_over_category_limits_and_wrong_extensions() {
        let oversized_json = vec![b' '; JSON_MAX_BYTES + 1];
        let issues =
            validate_bytes(ResourceKind::WorldBook, "world.json", oversized_json).unwrap_err();
        assert_eq!(issues[0].code, ValidationCode::FileTooLarge);

        let issues = validate_bytes(
            ResourceKind::Preset,
            "preset.yaml",
            br#"{"temperature":1}"#.to_vec(),
        )
        .unwrap_err();
        assert_eq!(issues[0].code, ValidationCode::InvalidExtension);
    }

    #[test]
    fn batch_import_keeps_valid_files_and_rejects_conflicts() {
        let root = std::env::temp_dir().join(format!(
            "astrabrew-resource-test-{}",
            TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let source_dir = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source_dir).unwrap();
        let valid = source_dir.join("valid.json");
        let invalid = source_dir.join("invalid.json");
        fs::write(&valid, br#"{"entries":[{"keys":["A"],"content":"B"}]}"#).unwrap();
        fs::write(&invalid, br#"{"entries":[]}"#).unwrap();
        let report = import_batch(
            ResourceKind::WorldBook,
            vec![valid.clone(), invalid],
            &destination,
        );
        assert_eq!(report.imported, 1);
        assert_eq!(report.failed.len(), 1);
        assert!(destination.join("valid.json").is_file());

        let conflict = import_batch(ResourceKind::WorldBook, vec![valid], &destination);
        assert_eq!(
            conflict.failed[0].result.as_ref().unwrap_err()[0].code,
            ValidationCode::TargetConflict
        );
        assert!(
            fs::read_dir(&destination)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp"))
        );
        fs::remove_dir_all(root).unwrap();
    }
}
