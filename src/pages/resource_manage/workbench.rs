//! 角色卡、世界书与预设的独立资源工作台。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fmt, mem};

use iced::widget::{
    button, checkbox, column, container, image, mouse_area, pick_list, responsive, row, scrollable,
    space, stack, text_editor, text_input,
};
use iced::{Alignment, Background, Border, Color, ContentFit, Element, Fill, Length, Task, Theme};
use lucide_icons::Icon;

use astra_ui::{BLUE_600, ButtonVariant, DANGER, SUCCESS, icons, pick_list_handle};

use crate::core::library::ResourceKind;
use crate::core::library::edit::{
    EditableCharacter, EditablePreset, EditablePresetPrompt, EditableWorldBook, EditableWorldEntry,
    EditorData, LoadedEditor, LoadedEditorSnapshot, create_preset_prompt, load_editor,
    load_world_book_for_binding, save_editor,
};
use crate::lang::{current_language, raw, resolve, t, t_in, text, tf};
use crate::theme::{button_style, pick_list_menu_style, pick_list_style, text_input_style};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(500);
// 预设只构建当前页的编辑器，避免大量条目同时参与布局和绘制。
const PRESET_PAGE_SIZE: usize = 12;
const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone)]
pub(crate) struct WorldBookOption {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkbenchKind {
    Character,
    WorldBook,
    Preset,
}

impl WorkbenchKind {
    pub const fn resource_kind(self) -> ResourceKind {
        match self {
            Self::Character => ResourceKind::CharacterCard,
            Self::WorldBook => ResourceKind::WorldBook,
            Self::Preset => ResourceKind::Preset,
        }
    }

    fn title_key(self) -> &'static str {
        match self {
            Self::Character => "workbench.character.title",
            Self::WorldBook => "workbench.world_book.title",
            Self::Preset => "workbench.preset.title",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Character => Icon::ContactRound,
            Self::WorldBook => Icon::BookOpenText,
            Self::Preset => Icon::SlidersHorizontal,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CharacterField {
    Name,
    Creator,
    Version,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CharacterArea {
    Description,
    Personality,
    Scenario,
    FirstMessage,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WorldField {
    Name,
    Author,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum EntryField {
    Comment,
    Order,
    Probability,
    Depth,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PresetField {
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryTagKind {
    Keys,
    SecondaryKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldPosition {
    BeforeCharacter,
    AfterCharacter,
    BeforeExamples,
    AfterExamples,
    BeforeAuthorNote,
    AfterAuthorNote,
    AtDepthSystem,
    AtDepthUser,
    AtDepthAssistant,
    Outlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresetRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkbenchMessage {
    Loaded(u64, Result<PreparedParcel, String>),
    RetryLoad,
    Undo,
    Redo,
    SelectCharacterCover,
    CharacterCoverLoaded(Result<Option<Vec<u8>>, String>),
    CharacterChanged(CharacterField, String),
    CharacterTagInputChanged(String),
    AddCharacterTag,
    RemoveCharacterTag(usize),
    CharacterAreaChanged(CharacterArea, text_editor::Action),
    WorldChanged(WorldField, String),
    EntryChanged(usize, EntryField, String),
    EntryTagInputChanged(usize, EntryTagKind, String),
    AddEntryTag(usize, EntryTagKind),
    RemoveEntryTag(usize, EntryTagKind, usize),
    EntryPositionChanged(usize, WorldPosition),
    EntryContentChanged(usize, text_editor::Action),
    EntryToggled(usize, bool),
    PresetChanged(usize, PresetField, String),
    PresetRoleChanged(usize, PresetRole),
    PresetContentChanged(usize, text_editor::Action),
    PresetToggled(usize, bool),
    PresetPageChanged(usize),
    AddPresetPrompt,
    PresetPromptAdded(Result<EditablePresetPrompt, String>),
    RequestDeletePreset(usize),
    ConfirmDeletePreset,
    CancelConfirmation,
    ShowWorldPicker,
    HideWorldPicker,
    PickWorldBook(PathBuf),
    RequestUnbindWorld,
    ConfirmWorldAction,
    WorldBookBound(Result<EditableWorldBook, String>),
    DebounceElapsed(u64),
    SaveFinished(u64, Result<(), String>),
    RetrySave,
    Close,
    Interact,
}

pub(crate) type PreparedParcel = Arc<Mutex<Option<PreparedWorkbench>>>;

#[derive(Debug)]
pub(crate) struct PreparedWorkbench {
    loaded: LoadedEditor,
    form: WorkbenchForm,
}

#[derive(Debug, Clone)]
enum WorkbenchPhase {
    Closed,
    Loading,
    LoadFailed(String),
    Ready,
}

#[derive(Debug)]
enum WorkbenchForm {
    Character(CharacterForm),
    WorldBook(WorldBookForm),
    Preset(PresetForm),
}

#[derive(Debug)]
struct CharacterForm {
    cover: image::Handle,
    name: String,
    creator: String,
    version: String,
    tags: Vec<String>,
    tag_input: String,
    description: text_editor::Content,
    personality: text_editor::Content,
    scenario: text_editor::Content,
    first_message: text_editor::Content,
    world_book: Option<WorldBookForm>,
}

#[derive(Debug)]
struct WorldBookForm {
    name: String,
    author: String,
    entries: Vec<WorldEntryForm>,
}

#[derive(Debug)]
struct WorldEntryForm {
    comment: String,
    keys: Vec<String>,
    keys_input: String,
    secondary_keys: Vec<String>,
    secondary_keys_input: String,
    content: text_editor::Content,
    enabled: bool,
    order: String,
    position: String,
    position_role: String,
    probability: String,
    depth: String,
}

#[derive(Debug)]
struct PresetForm {
    prompts: Vec<PresetPromptForm>,
    has_prompt_order: bool,
    current_page: usize,
}

#[derive(Debug)]
struct PresetPromptForm {
    identifier: String,
    name: String,
    role: String,
    content: Option<text_editor::Content>,
    enabled: bool,
    partial: bool,
    marker: bool,
}

const WORLD_POSITIONS: [WorldPosition; 10] = [
    WorldPosition::BeforeCharacter,
    WorldPosition::AfterCharacter,
    WorldPosition::BeforeExamples,
    WorldPosition::AfterExamples,
    WorldPosition::BeforeAuthorNote,
    WorldPosition::AfterAuthorNote,
    WorldPosition::AtDepthSystem,
    WorldPosition::AtDepthUser,
    WorldPosition::AtDepthAssistant,
    WorldPosition::Outlet,
];

const PRESET_ROLES: [PresetRole; 3] = [
    PresetRole::System,
    PresetRole::User,
    PresetRole::Assistant,
];

impl WorldPosition {
    fn from_values(position: &str, role: &str) -> Self {
        match position.trim() {
            "1" | "after_char" => Self::AfterCharacter,
            "2" => Self::BeforeAuthorNote,
            "3" => Self::AfterAuthorNote,
            "4" => match role.trim() {
                "1" | "user" => Self::AtDepthUser,
                "2" | "assistant" => Self::AtDepthAssistant,
                _ => Self::AtDepthSystem,
            },
            "5" => Self::BeforeExamples,
            "6" => Self::AfterExamples,
            "7" => Self::Outlet,
            _ => Self::BeforeCharacter,
        }
    }

    const fn values(self) -> (&'static str, &'static str) {
        match self {
            Self::BeforeCharacter => ("0", ""),
            Self::AfterCharacter => ("1", ""),
            Self::BeforeExamples => ("5", ""),
            Self::AfterExamples => ("6", ""),
            Self::BeforeAuthorNote => ("2", ""),
            Self::AfterAuthorNote => ("3", ""),
            Self::AtDepthSystem => ("4", "0"),
            Self::AtDepthUser => ("4", "1"),
            Self::AtDepthAssistant => ("4", "2"),
            Self::Outlet => ("7", ""),
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::BeforeCharacter => "workbench.position.before_character",
            Self::AfterCharacter => "workbench.position.after_character",
            Self::BeforeExamples => "workbench.position.before_examples",
            Self::AfterExamples => "workbench.position.after_examples",
            Self::BeforeAuthorNote => "workbench.position.before_author_note",
            Self::AfterAuthorNote => "workbench.position.after_author_note",
            Self::AtDepthSystem => "workbench.position.depth_system",
            Self::AtDepthUser => "workbench.position.depth_user",
            Self::AtDepthAssistant => "workbench.position.depth_assistant",
            Self::Outlet => "workbench.position.outlet",
        }
    }
}

impl fmt::Display for WorldPosition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(t_in(self.label_key(), current_language()))
    }
}

impl PresetRole {
    fn from_value(value: &str) -> Self {
        match value.trim() {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            _ => Self::System,
        }
    }

    const fn value(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    const fn label_key(self) -> &'static str {
        match self {
            Self::System => "workbench.role.system",
            Self::User => "workbench.role.user",
            Self::Assistant => "workbench.role.assistant",
        }
    }
}

impl fmt::Display for PresetRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(t_in(self.label_key(), current_language()))
    }
}

#[derive(Debug, Clone)]
enum Confirmation {
    DeletePreset(usize),
    BindWorld(PathBuf),
    UnbindWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkbenchEvent {
    None,
    Closed,
}

#[derive(Debug)]
pub(crate) struct WorkbenchState {
    phase: WorkbenchPhase,
    kind: WorkbenchKind,
    path: Option<PathBuf>,
    loaded: Option<Arc<Mutex<LoadedEditor>>>,
    form: Option<WorkbenchForm>,
    world_books: Vec<WorldBookOption>,
    world_picker_visible: bool,
    confirmation: Option<Confirmation>,
    load_serial: u64,
    revision: u64,
    saved_revision: u64,
    saving_revision: Option<u64>,
    debounce_serial: u64,
    save_error: Option<String>,
    operation_error: Option<String>,
    close_pending: bool,
    preset_add_pending: bool,
    character_cover_pending: bool,
    undo_stack: Vec<LoadedEditorSnapshot>,
    redo_stack: Vec<LoadedEditorSnapshot>,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            phase: WorkbenchPhase::Closed,
            kind: WorkbenchKind::Character,
            path: None,
            loaded: None,
            form: None,
            world_books: Vec::new(),
            world_picker_visible: false,
            confirmation: None,
            load_serial: 0,
            revision: 0,
            saved_revision: 0,
            saving_revision: None,
            debounce_serial: 0,
            save_error: None,
            operation_error: None,
            close_pending: false,
            preset_add_pending: false,
            character_cover_pending: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl WorkbenchState {
    pub fn is_open(&self) -> bool {
        !matches!(self.phase, WorkbenchPhase::Closed)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn open(
        &mut self,
        path: PathBuf,
        kind: WorkbenchKind,
        world_books: Vec<WorldBookOption>,
    ) -> Task<WorkbenchMessage> {
        self.phase = WorkbenchPhase::Loading;
        self.kind = kind;
        self.path = Some(path.clone());
        self.loaded = None;
        self.form = None;
        self.world_books = world_books;
        self.world_picker_visible = false;
        self.confirmation = None;
        self.load_serial = self.load_serial.wrapping_add(1);
        self.revision = 0;
        self.saved_revision = 0;
        self.saving_revision = None;
        self.save_error = None;
        self.operation_error = None;
        self.close_pending = false;
        self.preset_add_pending = false;
        self.character_cover_pending = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        load_task(path, kind, self.load_serial)
    }

    pub fn update(
        &mut self,
        message: WorkbenchMessage,
    ) -> (Task<WorkbenchMessage>, WorkbenchEvent) {
        match message {
            WorkbenchMessage::Loaded(serial, result) => {
                if serial != self.load_serial || matches!(self.phase, WorkbenchPhase::Closed) {
                    return (Task::none(), WorkbenchEvent::None);
                }
                match result {
                    Ok(parcel) => match take_prepared(parcel) {
                        Ok(prepared) => {
                            self.form = Some(prepared.form);
                            self.loaded = Some(Arc::new(Mutex::new(prepared.loaded)));
                            self.phase = WorkbenchPhase::Ready;
                        }
                        Err(error) => self.phase = WorkbenchPhase::LoadFailed(error),
                    },
                    Err(error) => self.phase = WorkbenchPhase::LoadFailed(error),
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::RetryLoad => {
                let Some(path) = self.path.clone() else {
                    return (Task::none(), WorkbenchEvent::None);
                };
                self.phase = WorkbenchPhase::Loading;
                self.load_serial = self.load_serial.wrapping_add(1);
                (
                    load_task(path, self.kind, self.load_serial),
                    WorkbenchEvent::None,
                )
            }
            WorkbenchMessage::Undo => (self.undo(), WorkbenchEvent::None),
            WorkbenchMessage::Redo => (self.redo(), WorkbenchEvent::None),
            WorkbenchMessage::SelectCharacterCover => {
                if self.character_cover_pending || !matches!(self.kind, WorkbenchKind::Character) {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.character_cover_pending = true;
                self.operation_error = None;
                (select_character_cover_task(), WorkbenchEvent::None)
            }
            WorkbenchMessage::CharacterCoverLoaded(result) => {
                self.character_cover_pending = false;
                match result {
                    Ok(Some(bytes)) => {
                        let handle = image::Handle::from_bytes(bytes.clone());
                        self.record_history();
                        let update_result = self.loaded.as_ref().map(|loaded| {
                            loaded
                                .lock()
                                .map_err(|_| t("workbench.error.state_corrupted").to_owned())
                                .and_then(|mut loaded| loaded.set_character_cover(bytes))
                        });
                        match update_result.transpose() {
                            Ok(Some(())) => {
                                if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                                    form.cover = handle;
                                }
                                return (self.mark_changed(true), WorkbenchEvent::None);
                            }
                            Ok(None) => {
                                self.operation_error = Some(t("workbench.character.not_loaded").to_owned());
                            }
                            Err(error) => self.operation_error = Some(resolve(&error)),
                        }
                        self.close_pending = false;
                    }
                    Ok(None) => {
                        // 用户取消文件选择不是错误；若之前请求关闭，继续等待后续关闭操作。
                        if self.close_pending {
                            self.close_pending = false;
                        }
                    }
                    Err(error) => {
                        self.operation_error = Some(resolve(&error));
                        self.close_pending = false;
                    }
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::CharacterChanged(field, value) => {
                self.record_history();
                if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                    match field {
                        CharacterField::Name => form.name.clone_from(&value),
                        CharacterField::Creator => form.creator.clone_from(&value),
                        CharacterField::Version => form.version.clone_from(&value),
                    }
                }
                self.update_loaded(|data| {
                    let EditorData::Character(character) = data else {
                        return;
                    };
                    match field {
                        CharacterField::Name => character.name = value,
                        CharacterField::Creator => character.creator = value,
                        CharacterField::Version => character.version = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::CharacterTagInputChanged(value) => {
                if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                    form.tag_input = value;
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::AddCharacterTag => {
                let can_add = match self.form.as_ref() {
                    Some(WorkbenchForm::Character(form)) => {
                        let tag = form.tag_input.trim();
                        !tag.is_empty()
                            && !form
                                .tags
                                .iter()
                                .any(|existing| existing.eq_ignore_ascii_case(tag))
                    }
                    _ => false,
                };
                if !can_add {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.record_history();
                let tag = self
                    .form
                    .as_mut()
                    .and_then(|form| match form {
                        WorkbenchForm::Character(form) => {
                            let tag = form.tag_input.trim();
                            if tag.is_empty()
                                || form
                                    .tags
                                    .iter()
                                    .any(|existing| existing.eq_ignore_ascii_case(tag))
                            {
                                return None;
                            }
                            let tag = tag.to_owned();
                            form.tags.insert(0, tag.clone());
                            form.tag_input.clear();
                            Some(tag)
                        }
                        _ => None,
                    });
                if tag.is_some() {
                    let tags = match self.form.as_ref() {
                        Some(WorkbenchForm::Character(form)) => form.tags.join(", "),
                        _ => String::new(),
                    };
                    self.update_loaded(|data| {
                        if let EditorData::Character(character) = data {
                            character.tags = tags;
                        }
                    });
                    return (self.mark_changed(true), WorkbenchEvent::None);
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::RemoveCharacterTag(index) => {
                let can_remove = matches!(
                    self.form.as_ref(),
                    Some(WorkbenchForm::Character(form)) if index < form.tags.len()
                );
                if !can_remove {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.record_history();
                let removed = if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                    (index < form.tags.len()).then(|| form.tags.remove(index))
                } else {
                    None
                };
                if removed.is_some() {
                    let tags = match self.form.as_ref() {
                        Some(WorkbenchForm::Character(form)) => form.tags.join(", "),
                        _ => String::new(),
                    };
                    self.update_loaded(|data| {
                        if let EditorData::Character(character) = data {
                            character.tags = tags;
                        }
                    });
                    return (self.mark_changed(true), WorkbenchEvent::None);
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::CharacterAreaChanged(field, action) => {
                let edited = action.is_edit();
                if edited {
                    self.record_history();
                }
                let value = if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                    let content = match field {
                        CharacterArea::Description => &mut form.description,
                        CharacterArea::Personality => &mut form.personality,
                        CharacterArea::Scenario => &mut form.scenario,
                        CharacterArea::FirstMessage => &mut form.first_message,
                    };
                    content.perform(action);
                    edited.then(|| content.text())
                } else {
                    None
                };
                if let Some(value) = value {
                    self.update_loaded(|data| {
                        let EditorData::Character(character) = data else {
                            return;
                        };
                        match field {
                            CharacterArea::Description => character.description = value,
                            CharacterArea::Personality => character.personality = value,
                            CharacterArea::Scenario => character.scenario = value,
                            CharacterArea::FirstMessage => character.first_message = value,
                        }
                    });
                }
                (
                    if edited {
                        self.mark_changed(false)
                    } else {
                        Task::none()
                    },
                    WorkbenchEvent::None,
                )
            }
            WorkbenchMessage::WorldChanged(field, value) => {
                self.record_history();
                if let Some(form) = self.world_form_mut() {
                    match field {
                        WorldField::Name => form.name.clone_from(&value),
                        WorldField::Author => form.author.clone_from(&value),
                    }
                }
                self.update_world(|world| match field {
                    WorldField::Name => world.name = value,
                    WorldField::Author => world.author = value,
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::EntryChanged(index, field, value) => {
                self.record_history();
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    match field {
                        EntryField::Comment => entry.comment.clone_from(&value),
                        EntryField::Order => entry.order.clone_from(&value),
                        EntryField::Probability => entry.probability.clone_from(&value),
                        EntryField::Depth => entry.depth.clone_from(&value),
                    }
                }
                self.update_world(|world| {
                    let Some(entry) = world.entries.get_mut(index) else {
                        return;
                    };
                    match field {
                        EntryField::Comment => entry.comment = value,
                        EntryField::Order => entry.order = value,
                        EntryField::Probability => entry.probability = value,
                        EntryField::Depth => entry.depth = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::EntryTagInputChanged(index, kind, value) => {
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    match kind {
                        EntryTagKind::Keys => entry.keys_input = value,
                        EntryTagKind::SecondaryKeys => entry.secondary_keys_input = value,
                    }
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::AddEntryTag(index, kind) => {
                let can_add = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get(index))
                    .is_some_and(|entry| entry.can_add_tag(kind));
                if !can_add {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.record_history();
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    entry.add_tag(kind);
                }
                self.sync_entry_tags(index, kind);
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::RemoveEntryTag(index, kind, tag_index) => {
                let can_remove = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get(index))
                    .is_some_and(|entry| entry.tags(kind).get(tag_index).is_some());
                if !can_remove {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.record_history();
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    entry.tags_mut(kind).remove(tag_index);
                }
                self.sync_entry_tags(index, kind);
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::EntryPositionChanged(index, position) => {
                self.record_history();
                let (position_value, role) = position.values();
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    entry.position = position_value.to_owned();
                    entry.position_role = role.to_owned();
                }
                self.update_world(|world| {
                    if let Some(entry) = world.entries.get_mut(index) {
                        entry.position = position_value.to_owned();
                        entry.position_role = role.to_owned();
                    }
                });
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::EntryContentChanged(index, action) => {
                let edited = action.is_edit();
                if edited {
                    self.record_history();
                }
                let value = if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    entry.content.perform(action);
                    edited.then(|| entry.content.text())
                } else {
                    None
                };
                if let Some(value) = value {
                    self.update_world(|world| {
                        if let Some(entry) = world.entries.get_mut(index) {
                            entry.content = value;
                        }
                    });
                }
                (
                    if edited {
                        self.mark_changed(false)
                    } else {
                        Task::none()
                    },
                    WorkbenchEvent::None,
                )
            }
            WorkbenchMessage::EntryToggled(index, enabled) => {
                self.record_history();
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    entry.enabled = enabled;
                }
                self.update_world(|world| {
                    if let Some(entry) = world.entries.get_mut(index) {
                        entry.enabled = enabled;
                    }
                });
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetChanged(index, field, value) => {
                self.record_history();
                if let Some(prompt) = self.preset_prompt_mut(index) {
                    match field {
                        PresetField::Name => prompt.name.clone_from(&value),
                    }
                }
                self.update_preset(|preset| {
                    let Some(prompt) = preset.prompts.get_mut(index) else {
                        return;
                    };
                    match field {
                        PresetField::Name => prompt.name = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetRoleChanged(index, role) => {
                self.record_history();
                let value = role.value().to_owned();
                if let Some(prompt) = self.preset_prompt_mut(index) {
                    prompt.role.clone_from(&value);
                }
                self.update_preset(|preset| {
                    if let Some(prompt) = preset.prompts.get_mut(index) {
                        prompt.role = value;
                    }
                });
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetContentChanged(index, action) => {
                let edited = action.is_edit();
                if edited {
                    self.record_history();
                }
                let value = if let Some(prompt) = self.preset_prompt_mut(index) {
                    if let Some(content) = &mut prompt.content {
                        content.perform(action);
                        edited.then(|| content.text())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(value) = value {
                    self.update_preset(|preset| {
                        if let Some(prompt) = preset.prompts.get_mut(index) {
                            prompt.content = value;
                        }
                    });
                }
                (
                    if edited {
                        self.mark_changed(false)
                    } else {
                        Task::none()
                    },
                    WorkbenchEvent::None,
                )
            }
            WorkbenchMessage::PresetToggled(index, enabled) => {
                self.record_history();
                let mut resolved = None;
                if let Some(prompt) = self.preset_prompt_mut(index) {
                    // 多套顺序模板状态不一致时，第一次点击统一启用，随后才按普通开关切换。
                    prompt.enabled = if prompt.partial { true } else { enabled };
                    prompt.partial = false;
                    resolved = Some(prompt.enabled);
                }
                if let Some(enabled) = resolved {
                    self.update_preset(|preset| {
                        if let Some(prompt) = preset.prompts.get_mut(index) {
                            prompt.enabled = enabled;
                            prompt.partial = false;
                            prompt.enabled_changed = true;
                        }
                    });
                }
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetPageChanged(page) => {
                self.prepare_preset_page(page);
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::AddPresetPrompt => {
                if self.preset_add_pending {
                    return (Task::none(), WorkbenchEvent::None);
                }
                let Some(loaded) = self.loaded.clone() else {
                    return (Task::none(), WorkbenchEvent::None);
                };
                self.operation_error = None;
                self.preset_add_pending = true;
                (add_preset_prompt_task(loaded), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetPromptAdded(result) => {
                self.preset_add_pending = false;
                match result {
                    Ok(prompt) => {
                        self.record_history();
                        self.operation_error = None;
                        if let Some(WorkbenchForm::Preset(form)) = &mut self.form {
                            form.clear_current_page();
                            form.prompts
                                .insert(0, PresetPromptForm::from_prompt(&prompt, false));
                            form.current_page = 0;
                        }
                        self.update_preset(|preset| preset.prompts.insert(0, prompt));
                        self.prepare_preset_page(0);
                        return (self.mark_changed(true), WorkbenchEvent::None);
                    }
                    Err(error) => {
                        self.operation_error = Some(resolve(&error));
                        self.close_pending = false;
                    }
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::RequestDeletePreset(index) => {
                self.confirmation = Some(Confirmation::DeletePreset(index));
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::ShowWorldPicker => {
                self.world_picker_visible = true;
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::HideWorldPicker => {
                self.world_picker_visible = false;
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::PickWorldBook(path) => {
                self.world_picker_visible = false;
                if self.world_form_mut().is_some() {
                    self.confirmation = Some(Confirmation::BindWorld(path));
                    (Task::none(), WorkbenchEvent::None)
                } else {
                    (bind_world_task(path), WorkbenchEvent::None)
                }
            }
            WorkbenchMessage::RequestUnbindWorld => {
                self.confirmation = Some(Confirmation::UnbindWorld);
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::ConfirmWorldAction => {
                let confirmation = self.confirmation.take();
                match confirmation {
                    Some(Confirmation::BindWorld(path)) => {
                        (bind_world_task(path), WorkbenchEvent::None)
                    }
                    Some(Confirmation::UnbindWorld) => {
                        self.record_history();
                        if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                            form.world_book = None;
                        }
                        self.update_loaded(|data| {
                            if let EditorData::Character(character) = data {
                                character.world_book = None;
                            }
                        });
                        (self.mark_changed(true), WorkbenchEvent::None)
                    }
                    _ => (Task::none(), WorkbenchEvent::None),
                }
            }
            WorkbenchMessage::WorldBookBound(result) => {
                match result {
                    Ok(world) => {
                        self.record_history();
                        if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                            form.world_book = Some(WorldBookForm::from(&world));
                        }
                        self.update_loaded(|data| {
                            if let EditorData::Character(character) = data {
                                character.world_book = Some(world);
                            }
                        });
                        return (self.mark_changed(true), WorkbenchEvent::None);
                    }
                    Err(error) => self.save_error = Some(resolve(&error)),
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::ConfirmDeletePreset => {
                let Some(Confirmation::DeletePreset(index)) = self.confirmation.take() else {
                    return (Task::none(), WorkbenchEvent::None);
                };
                self.record_history();
                if let Some(WorkbenchForm::Preset(form)) = &mut self.form
                    && index < form.prompts.len()
                {
                    form.clear_current_page();
                    form.prompts.remove(index);
                }
                self.update_preset(|preset| {
                    if index < preset.prompts.len() {
                        preset.prompts.remove(index);
                    }
                });
                let page = match &self.form {
                    Some(WorkbenchForm::Preset(form)) => {
                        form.current_page.min(form.page_count().saturating_sub(1))
                    }
                    _ => 0,
                };
                self.prepare_preset_page(page);
                (self.mark_changed(true), WorkbenchEvent::None)
            }
            WorkbenchMessage::CancelConfirmation => {
                self.confirmation = None;
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::DebounceElapsed(serial) => {
                if serial != self.debounce_serial
                    || self.saving_revision.is_some()
                    || self.revision == self.saved_revision
                {
                    return (Task::none(), WorkbenchEvent::None);
                }
                (self.start_save(), WorkbenchEvent::None)
            }
            WorkbenchMessage::SaveFinished(revision, result) => {
                if self.saving_revision != Some(revision) {
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.saving_revision = None;
                match result {
                    Ok(()) => {
                        self.saved_revision = revision;
                        self.save_error = None;
                        if self.revision > revision {
                            return (self.start_save(), WorkbenchEvent::None);
                        }
                        if self.close_pending {
                            self.reset_closed();
                            return (Task::none(), WorkbenchEvent::Closed);
                        }
                    }
                    Err(error) => {
                        self.save_error = Some(resolve(&error));
                        self.close_pending = false;
                    }
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::RetrySave => {
                if self.saving_revision.is_none() && self.revision > self.saved_revision {
                    (self.start_save(), WorkbenchEvent::None)
                } else {
                    (Task::none(), WorkbenchEvent::None)
                }
            }
            WorkbenchMessage::Close => {
                if self.save_error.is_some() {
                    return (Task::none(), WorkbenchEvent::None);
                }
                if self.character_cover_pending
                    || self.preset_add_pending
                    || self.saving_revision.is_some()
                    || self.revision > self.saved_revision
                {
                    self.close_pending = true;
                    if !self.character_cover_pending
                        && !self.preset_add_pending
                        && self.saving_revision.is_none()
                    {
                        return (self.start_save(), WorkbenchEvent::None);
                    }
                    return (Task::none(), WorkbenchEvent::None);
                }
                self.reset_closed();
                (Task::none(), WorkbenchEvent::Closed)
            }
            WorkbenchMessage::Interact => (Task::none(), WorkbenchEvent::None),
        }
    }

    fn mark_changed(&mut self, immediate: bool) -> Task<WorkbenchMessage> {
        if !matches!(self.phase, WorkbenchPhase::Ready) {
            return Task::none();
        }
        self.revision = self.revision.wrapping_add(1);
        self.save_error = None;
        self.debounce_serial = self.debounce_serial.wrapping_add(1);
        if immediate && self.saving_revision.is_none() {
            self.start_save()
        } else {
            let serial = self.debounce_serial;
            Task::perform(
                async move {
                    tokio::time::sleep(AUTOSAVE_DELAY).await;
                    serial
                },
                WorkbenchMessage::DebounceElapsed,
            )
        }
    }

    /// 在一次可编辑操作前保存快照；新操作会清空重做栈，符合常见编辑器行为。
    fn record_history(&mut self) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        let Ok(loaded) = loaded.lock() else {
            return;
        };
        self.undo_stack.push(loaded.snapshot());
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) -> Task<WorkbenchMessage> {
        let Some(snapshot) = self.undo_stack.pop() else {
            return Task::none();
        };
        self.apply_history_snapshot(snapshot, true)
    }

    fn redo(&mut self) -> Task<WorkbenchMessage> {
        let Some(snapshot) = self.redo_stack.pop() else {
            return Task::none();
        };
        self.apply_history_snapshot(snapshot, false)
    }

    fn apply_history_snapshot(
        &mut self,
        snapshot: LoadedEditorSnapshot,
        from_undo: bool,
    ) -> Task<WorkbenchMessage> {
        let Some(loaded) = self.loaded.clone() else {
            return Task::none();
        };
        let current_page = match &self.form {
            Some(WorkbenchForm::Preset(form)) => Some(form.current_page),
            _ => None,
        };
        let Ok(mut loaded) = loaded.lock() else {
            self.operation_error = Some(t("workbench.error.state_corrupted").to_owned());
            return Task::none();
        };
        let current = loaded.snapshot();
        loaded.restore_snapshot(snapshot);
        let form = form_from_loaded(&loaded);
        if from_undo {
            self.redo_stack.push(current);
        } else {
            self.undo_stack.push(current);
        }
        drop(loaded);
        self.form = Some(form);
        if let Some(page) = current_page {
            self.prepare_preset_page(page);
        }
        self.operation_error = None;
        self.mark_changed(true)
    }

    fn start_save(&mut self) -> Task<WorkbenchMessage> {
        let Some(loaded) = self.loaded.clone() else {
            return Task::none();
        };
        let revision = self.revision;
        self.saving_revision = Some(revision);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    // 大型预设的快照克隆、序列化和磁盘写入全部离开 UI 线程。
                    let snapshot = loaded
                        .lock()
                        .map_err(|_| t("workbench.error.state_corrupted").to_owned())?
                        .clone();
                    let saved = save_editor(snapshot)?;
                    loaded
                        .lock()
                        .map_err(|_| t("workbench.error.state_corrupted").to_owned())?
                        .adopt_saved_document(saved);
                    Ok(())
                })
                .await
                .unwrap_or_else(|_| Err(t("workbench.error.save_thread_crashed").to_owned()))
            },
            move |result| WorkbenchMessage::SaveFinished(revision, result),
        )
    }

    fn update_loaded(&self, update: impl FnOnce(&mut EditorData)) {
        let Some(loaded) = &self.loaded else {
            return;
        };
        if let Ok(mut loaded) = loaded.lock() {
            update(&mut loaded.data);
        }
    }

    fn update_world(&self, update: impl FnOnce(&mut EditableWorldBook)) {
        self.update_loaded(|data| {
            let world = match data {
                EditorData::Character(character) => character.world_book.as_mut(),
                EditorData::WorldBook(world) => Some(world),
                EditorData::Preset(_) => None,
            };
            if let Some(world) = world {
                update(world);
            }
        });
    }

    fn sync_entry_tags(&self, index: usize, kind: EntryTagKind) {
        let value = match self.form.as_ref() {
            Some(WorkbenchForm::Character(form)) => form.world_book.as_ref(),
            Some(WorkbenchForm::WorldBook(form)) => Some(form),
            _ => None,
        }
        .and_then(|form| form.entries.get(index))
        .map(|entry| entry.tags(kind).join(", "));
        let Some(value) = value else {
            return;
        };
        self.update_world(|world| {
            let Some(entry) = world.entries.get_mut(index) else {
                return;
            };
            match kind {
                EntryTagKind::Keys => entry.keys = value,
                EntryTagKind::SecondaryKeys => entry.secondary_keys = value,
            }
        });
    }

    fn update_preset(&self, update: impl FnOnce(&mut EditablePreset)) {
        self.update_loaded(|data| {
            if let EditorData::Preset(preset) = data {
                update(preset);
            }
        });
    }

    fn prepare_preset_page(&mut self, requested_page: usize) {
        let Some(WorkbenchForm::Preset(form)) = &self.form else {
            return;
        };
        let page = requested_page.min(form.page_count().saturating_sub(1));
        let (start, end) = form.page_range(page);
        let contents = self.loaded.as_ref().and_then(|loaded| {
            let loaded = loaded.lock().ok()?;
            let EditorData::Preset(preset) = &loaded.data else {
                return None;
            };
            Some(
                preset.prompts[start..end]
                    .iter()
                    .map(|prompt| prompt.content.clone())
                    .collect::<Vec<_>>(),
            )
        });
        let Some(contents) = contents else {
            return;
        };
        if let Some(WorkbenchForm::Preset(form)) = &mut self.form {
            form.clear_current_page();
            form.current_page = page;
            for (index, content) in (start..end).zip(contents) {
                if let Some(prompt) = form.prompts.get_mut(index) {
                    prompt.content = Some(text_editor::Content::with_text(&content));
                }
            }
        }
    }

    fn world_form_mut(&mut self) -> Option<&mut WorldBookForm> {
        match self.form.as_mut()? {
            WorkbenchForm::Character(form) => form.world_book.as_mut(),
            WorkbenchForm::WorldBook(form) => Some(form),
            WorkbenchForm::Preset(_) => None,
        }
    }

    fn preset_prompt_mut(&mut self, index: usize) -> Option<&mut PresetPromptForm> {
        match self.form.as_mut()? {
            WorkbenchForm::Preset(form) => form.prompts.get_mut(index),
            _ => None,
        }
    }

    fn reset_closed(&mut self) {
        self.phase = WorkbenchPhase::Closed;
        self.path = None;
        self.load_serial = self.load_serial.wrapping_add(1);
        self.loaded = None;
        self.form = None;
        self.world_picker_visible = false;
        self.confirmation = None;
        self.save_error = None;
        self.operation_error = None;
        self.close_pending = false;
        self.preset_add_pending = false;
        self.character_cover_pending = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

fn load_task(path: PathBuf, kind: WorkbenchKind, serial: u64) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let loaded = load_editor(&path, kind.resource_kind())?;
                // 预设可能包含大量条目，文本编辑器内容也必须在后台一次性准备。
                let form = form_from_loaded(&loaded);
                Ok(Arc::new(Mutex::new(Some(PreparedWorkbench {
                    loaded,
                    form,
                }))))
            })
            .await
            .unwrap_or_else(|_| Err(t("workbench.error.load_thread_crashed").to_owned()))
        },
        move |result| WorkbenchMessage::Loaded(serial, result),
    )
}

fn select_character_cover_task() -> Task<WorkbenchMessage> {
    Task::perform(
        async {
            let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter(t("resources.filter.png_image"), &["png"])
                .pick_file()
                .await
            else {
                return Ok(None);
            };
            tokio::task::spawn_blocking(move || {
                std::fs::read(file.path())
                    .map(Some)
                    .map_err(|error| tf("workbench.error.cover_read_failed", &[("error", &error)]))
            })
            .await
            .unwrap_or_else(|_| Err(t("workbench.error.cover_thread_crashed").to_owned()))
        },
        WorkbenchMessage::CharacterCoverLoaded,
    )
}

fn take_prepared(parcel: PreparedParcel) -> Result<PreparedWorkbench, String> {
    parcel
        .lock()
        .map_err(|_| t("workbench.error.parcel_corrupted").to_owned())?
        .take()
        .ok_or_else(|| t("workbench.error.parcel_consumed").to_owned())
}

fn add_preset_prompt_task(loaded: Arc<Mutex<LoadedEditor>>) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let loaded = loaded
                    .lock()
                    .map_err(|_| t("workbench.error.state_corrupted").to_owned())?;
                let EditorData::Preset(preset) = &loaded.data else {
                    return Err(t("workbench.error.not_preset_editor").to_owned());
                };
                Ok(create_preset_prompt(&preset.prompts))
            })
            .await
            .unwrap_or_else(|_| Err(t("workbench.error.prompt_create_thread_crashed").to_owned()))
        },
        WorkbenchMessage::PresetPromptAdded,
    )
}

fn bind_world_task(path: PathBuf) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || load_world_book_for_binding(&path))
                .await
                .unwrap_or_else(|_| Err(t("workbench.error.world_load_thread_crashed").to_owned()))
        },
        WorkbenchMessage::WorldBookBound,
    )
}

fn form_from_data(data: &EditorData) -> WorkbenchForm {
    match data {
        EditorData::Character(character) => {
            WorkbenchForm::Character(CharacterForm::from(character))
        }
        EditorData::WorldBook(world) => WorkbenchForm::WorldBook(WorldBookForm::from(world)),
        EditorData::Preset(preset) => WorkbenchForm::Preset(PresetForm::from(preset)),
    }
}

fn form_from_loaded(loaded: &LoadedEditor) -> WorkbenchForm {
    let mut form = form_from_data(&loaded.data);
    if let WorkbenchForm::Character(character) = &mut form {
        character.cover =
            image::Handle::from_bytes(loaded.character_cover_bytes().unwrap_or_default().to_vec());
    }
    form
}

impl From<&EditableCharacter> for CharacterForm {
    fn from(value: &EditableCharacter) -> Self {
        Self {
            cover: image::Handle::from_bytes(Vec::new()),
            name: value.name.clone(),
            creator: value.creator.clone(),
            version: value.version.clone(),
            tags: value
                .tags
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            tag_input: String::new(),
            description: text_editor::Content::with_text(&value.description),
            personality: text_editor::Content::with_text(&value.personality),
            scenario: text_editor::Content::with_text(&value.scenario),
            first_message: text_editor::Content::with_text(&value.first_message),
            world_book: value.world_book.as_ref().map(WorldBookForm::from),
        }
    }
}

impl From<&EditableWorldBook> for WorldBookForm {
    fn from(value: &EditableWorldBook) -> Self {
        Self {
            name: value.name.clone(),
            author: value.author.clone(),
            entries: value.entries.iter().map(WorldEntryForm::from).collect(),
        }
    }
}

impl From<&EditableWorldEntry> for WorldEntryForm {
    fn from(value: &EditableWorldEntry) -> Self {
        Self {
            comment: value.comment.clone(),
            keys: split_tags(&value.keys),
            keys_input: String::new(),
            secondary_keys: split_tags(&value.secondary_keys),
            secondary_keys_input: String::new(),
            content: text_editor::Content::with_text(&value.content),
            enabled: value.enabled,
            order: value.order.clone(),
            position: value.position.clone(),
            position_role: value.position_role.clone(),
            probability: value.probability.clone(),
            depth: value.depth.clone(),
        }
    }
}

impl WorldEntryForm {
    fn tags(&self, kind: EntryTagKind) -> &[String] {
        match kind {
            EntryTagKind::Keys => &self.keys,
            EntryTagKind::SecondaryKeys => &self.secondary_keys,
        }
    }

    fn tags_mut(&mut self, kind: EntryTagKind) -> &mut Vec<String> {
        match kind {
            EntryTagKind::Keys => &mut self.keys,
            EntryTagKind::SecondaryKeys => &mut self.secondary_keys,
        }
    }

    fn tag_input(&self, kind: EntryTagKind) -> &str {
        match kind {
            EntryTagKind::Keys => &self.keys_input,
            EntryTagKind::SecondaryKeys => &self.secondary_keys_input,
        }
    }

    fn can_add_tag(&self, kind: EntryTagKind) -> bool {
        let candidate = self.tag_input(kind).trim();
        !candidate.is_empty()
            && !self
                .tags(kind)
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(candidate))
    }

    fn add_tag(&mut self, kind: EntryTagKind) {
        let input = match kind {
            EntryTagKind::Keys => &mut self.keys_input,
            EntryTagKind::SecondaryKeys => &mut self.secondary_keys_input,
        };
        let tag = mem::take(input).trim().to_owned();
        if !tag.is_empty() {
            self.tags_mut(kind).insert(0, tag);
        }
    }
}

fn split_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

impl From<&EditablePreset> for PresetForm {
    fn from(value: &EditablePreset) -> Self {
        Self {
            prompts: value
                .prompts
                .iter()
                .enumerate()
                .map(|(index, prompt)| {
                    PresetPromptForm::from_prompt(prompt, index < PRESET_PAGE_SIZE)
                })
                .collect(),
            has_prompt_order: value.has_prompt_order(),
            current_page: 0,
        }
    }
}

impl PresetForm {
    fn page_count(&self) -> usize {
        self.prompts.len().div_ceil(PRESET_PAGE_SIZE).max(1)
    }

    fn page_range(&self, page: usize) -> (usize, usize) {
        let start = page
            .saturating_mul(PRESET_PAGE_SIZE)
            .min(self.prompts.len());
        let end = (start + PRESET_PAGE_SIZE).min(self.prompts.len());
        (start, end)
    }

    fn clear_current_page(&mut self) {
        let (start, end) = self.page_range(self.current_page);
        for prompt in &mut self.prompts[start..end] {
            prompt.content = None;
        }
    }
}

impl PresetPromptForm {
    fn from_prompt(value: &EditablePresetPrompt, load_content: bool) -> Self {
        Self {
            identifier: value.identifier.clone(),
            name: value.name.clone(),
            role: value.role.clone(),
            content: load_content.then(|| text_editor::Content::with_text(&value.content)),
            enabled: value.enabled,
            partial: value.partial,
            marker: value.marker,
        }
    }
}

pub(crate) fn view(state: &WorkbenchState) -> Element<'_, WorkbenchMessage> {
    let backdrop = button(space::horizontal())
        .width(Fill)
        .height(Fill)
        .on_press(WorkbenchMessage::Interact)
        .style(backdrop_style);
    let surface = container(
        column![workbench_header(state), workbench_body(state)]
            .width(Fill)
            .height(Fill),
    )
    .width(Fill)
    .height(Fill)
    .style(workbench_surface);
    let mut layers = stack![
        backdrop,
        mouse_area(surface).on_press(WorkbenchMessage::Interact)
    ];
    if state.world_picker_visible {
        layers = layers.push(world_picker(state));
    }
    if let Some(confirmation) = &state.confirmation {
        layers = layers.push(confirmation_modal(confirmation));
    }
    layers.into()
}

fn workbench_header(state: &WorkbenchState) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let status = if state.save_error.is_some() {
        row![
            icons::icon(Icon::CircleAlert, 14, DANGER),
            raw(t_in("workbench.save.failed", language))
                .size(11)
                .color(DANGER),
            button(raw(t_in("workbench.save.retry", language)).size(11))
                .on_press(WorkbenchMessage::RetrySave)
                .padding([5, 9])
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
    } else if state.saving_revision.is_some()
        || state.close_pending
        || state.preset_add_pending
        || state.character_cover_pending
    {
        row![
            icons::icon(Icon::LoaderCircle, 14, BLUE_600),
            text(if state.close_pending {
                t_in("workbench.save.closing", language)
            } else if state.preset_add_pending {
                t_in("workbench.preset.adding", language)
            } else if state.character_cover_pending {
                t_in("workbench.character.cover.loading", language)
            } else {
                t_in("workbench.save.saving", language)
            })
            .size(11),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    } else {
        row![
            icons::icon(Icon::CircleCheck, 14, SUCCESS),
            raw(t_in("workbench.save.saved", language)).size(11),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    };
    let history_controls = row![
        history_button(
            Icon::Undo2,
            (!state.undo_stack.is_empty()).then_some(WorkbenchMessage::Undo),
        ),
        history_button(
            Icon::Redo2,
            (!state.redo_stack.is_empty()).then_some(WorkbenchMessage::Redo),
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center);
    // 保存状态文案长度不同，预留固定区域避免状态切换时推动撤销/重做按钮。
    let status_slot = container(status)
        .width(Length::Fixed(220.0))
        .align_x(Alignment::End)
        .align_y(Alignment::Center);
    container(
        row![
            container(icons::icon(state.kind.icon(), 18, BLUE_600))
                .width(36)
                .height(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(accent_surface),
            column![
                raw(t_in(state.kind.title_key(), language)).size(17),
                raw(
                    state
                        .path
                        .as_deref()
                        .and_then(Path::file_name)
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                )
                .size(11)
                .style(crate::theme::muted_text_style),
            ]
            .spacing(2),
            space::horizontal(),
            history_controls,
            status_slot,
            button(icons::icon(Icon::X, 17, BLUE_600))
                .on_press(WorkbenchMessage::Close)
                .width(34)
                .height(34)
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([10, 14])
    .style(header_surface)
    .into()
}

fn history_button(
    icon: Icon,
    message: Option<WorkbenchMessage>,
) -> Element<'static, WorkbenchMessage> {
    button(
        container(icons::icon(icon, 16, BLUE_600))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .on_press_maybe(message)
    .width(30)
    .height(30)
    .padding(0)
    .style(button_style(ButtonVariant::Ghost))
    .into()
}

fn workbench_body(state: &WorkbenchState) -> Element<'_, WorkbenchMessage> {
    let body = match (&state.phase, &state.form) {
        (WorkbenchPhase::Loading, _) => {
            centered_state(Icon::LoaderCircle, "workbench.loading", None)
        }
        (WorkbenchPhase::LoadFailed(error), _) => centered_state(
            Icon::CircleAlert,
            "workbench.load_failed",
            Some((error.as_str(), WorkbenchMessage::RetryLoad)),
        ),
        (WorkbenchPhase::Ready, Some(WorkbenchForm::Character(form))) => character_view(form),
        (WorkbenchPhase::Ready, Some(WorkbenchForm::WorldBook(form))) => world_view(form, false),
        (WorkbenchPhase::Ready, Some(WorkbenchForm::Preset(form))) => {
            preset_view(form, state.preset_add_pending)
        }
        _ => container(space::horizontal()).into(),
    };
    if let Some(error) = state.save_error.as_ref().or(state.operation_error.as_ref())
        && matches!(state.phase, WorkbenchPhase::Ready)
    {
        column![
            container(
                row![
                    icons::icon(Icon::CircleAlert, 15, DANGER),
                    raw(error).size(11).width(Fill).color(DANGER),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Fill)
            .padding([8, 14])
            .style(error_surface),
            body,
        ]
        .height(Fill)
        .into()
    } else {
        body
    }
}

fn character_view(form: &CharacterForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let cover = container(
        row![
            container(
                image::Image::new(&form.cover)
                    .width(Length::Fill)
                    .height(Length::Fixed(210.0))
                    .content_fit(ContentFit::Contain),
            )
            .width(Length::FillPortion(2))
            .height(Length::Fixed(210.0))
            .padding(6)
            .style(panel_surface),
            column![
                section_title("workbench.character.cover"),
                raw(t_in("workbench.character.cover.detail", language))
                    .size(11)
                    .style(crate::theme::muted_text_style),
                button(
                    row![
                        icons::icon(Icon::ImagePlus, 14, BLUE_600),
                        raw(t_in("workbench.character.cover.change", language)).size(11),
                    ]
                    .spacing(6),
                )
                .on_press(WorkbenchMessage::SelectCharacterCover)
                .padding([7, 10])
                .style(button_style(ButtonVariant::Secondary)),
            ]
            .spacing(8)
            .width(Length::FillPortion(3)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(10)
    .style(panel_surface);
    let metadata = column![
        cover,
        section_title("workbench.character.basic"),
        row![
            field_input("workbench.field.name", &form.name, |value| {
                WorkbenchMessage::CharacterChanged(CharacterField::Name, value)
            }),
            field_input("workbench.field.author", &form.creator, |value| {
                WorkbenchMessage::CharacterChanged(CharacterField::Creator, value)
            }),
            field_input("workbench.field.version", &form.version, |value| {
                WorkbenchMessage::CharacterChanged(CharacterField::Version, value)
            }),
        ]
        .spacing(10),
        character_tags_field(form),
    ]
    .spacing(10);

    let world_controls = row![
        button(
            row![
                icons::icon(Icon::BookPlus, 14, BLUE_600),
                text(if form.world_book.is_some() {
                    t_in("workbench.world.change", language)
                } else {
                    t_in("workbench.world.bind", language)
                })
                .size(11),
            ]
            .spacing(6),
        )
        .on_press(WorkbenchMessage::ShowWorldPicker)
        .padding([7, 10])
        .style(button_style(ButtonVariant::Secondary)),
        button(
            row![
                icons::icon(Icon::Unlink, 14, DANGER),
                raw(t_in("workbench.world.unbind", language)).size(11),
            ]
            .spacing(6),
        )
        .on_press_maybe(
            form.world_book
                .is_some()
                .then_some(WorkbenchMessage::RequestUnbindWorld),
        )
        .padding([7, 10])
        .style(button_style(ButtonVariant::Secondary)),
    ]
    .spacing(8);
    let world: Element<'_, WorkbenchMessage> = if let Some(world) = &form.world_book {
        column![
            section_title("workbench.character.world"),
            world_controls,
            world_view(world, true)
        ]
        .spacing(10)
        .into()
    } else {
        column![
            section_title("workbench.character.world"),
            world_controls,
            container(raw(t_in("workbench.world.empty", language)).size(12))
                .width(Fill)
                .padding(18)
                .style(panel_surface),
        ]
        .spacing(10)
        .into()
    };

    // 角色卡上半区使用左右分栏：左侧承载资料，右侧用 2×2 网格编辑长文本。
    let text_grid = column![
        row![
            container(area_field(
                "workbench.character.description",
                &form.description,
                |action| {
                    WorkbenchMessage::CharacterAreaChanged(CharacterArea::Description, action)
                },
            ))
            .width(Length::FillPortion(1)),
            container(area_field(
                "workbench.character.personality",
                &form.personality,
                |action| {
                    WorkbenchMessage::CharacterAreaChanged(CharacterArea::Personality, action)
                },
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(12),
        row![
            container(area_field(
                "workbench.character.scenario",
                &form.scenario,
                |action| WorkbenchMessage::CharacterAreaChanged(CharacterArea::Scenario, action),
            ))
            .width(Length::FillPortion(1)),
            container(area_field(
                "workbench.character.first_message",
                &form.first_message,
                |action| {
                    WorkbenchMessage::CharacterAreaChanged(CharacterArea::FirstMessage, action)
                },
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(12),
    ]
    .spacing(12);

    // 世界书单独占用整行，避免被左侧栏限制成单列并浪费右侧空间。
    scrollable(
        container(
            column![
                row![
                    container(metadata).width(Length::FillPortion(4)),
                    container(text_grid).width(Length::FillPortion(6)),
                ]
                .spacing(16)
                .align_y(Alignment::Start),
                world,
            ]
            .spacing(18),
        )
        .padding(16),
    )
    .height(Fill)
    .into()
}

fn world_view(form: &WorldBookForm, embedded: bool) -> Element<'_, WorkbenchMessage> {
    let header = column![
        if embedded {
            space::vertical().height(Length::Shrink).into()
        } else {
            section_title("workbench.world_book.basic")
        },
        row![
            field_input("workbench.field.name", &form.name, |value| {
                WorkbenchMessage::WorldChanged(WorldField::Name, value)
            }),
            field_input("workbench.field.author", &form.author, |value| {
                WorkbenchMessage::WorldChanged(WorldField::Author, value)
            }),
        ]
        .spacing(10),
    ]
    .spacing(10);

    let entries = world_entries_grid(form);
    let content = column![header, section_title("workbench.world.entries"), entries].spacing(10);
    if embedded {
        content.into()
    } else {
        scrollable(container(content).padding(16))
            .height(Fill)
            .into()
    }
}

fn world_entries_grid(form: &WorldBookForm) -> Element<'_, WorkbenchMessage> {
    let scale = crate::core::typography::current_ui_scale();
    responsive(move |size| {
        let columns = adaptive_grid_columns(size.width, 280.0, scale, 5);
        let compact = columns >= 4;
        let mut grid = column![].spacing(10);

        for chunk in form
            .entries
            .iter()
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(columns)
        {
            let mut line = row![].spacing(10);
            for (index, entry) in chunk {
                line = line.push(
                    container(world_entry_view(*index, entry, compact))
                        .width(Length::FillPortion(1)),
                );
            }
            for _ in chunk.len()..columns {
                line = line.push(container(space::horizontal()).width(Length::FillPortion(1)));
            }
            grid = grid.push(line);
        }
        grid.into()
    })
    .into()
}

/// 按当前工作区宽度和界面缩放计算桌面网格列数，保证字体变大时卡片不会被挤坏。
fn adaptive_grid_columns(
    available_width: f32,
    base_card_width: f32,
    scale: f32,
    max_columns: usize,
) -> usize {
    let scaled_card_width = base_card_width * scale.max(0.75);
    let width = available_width.max(scaled_card_width);
    ((width / scaled_card_width).floor() as usize).clamp(1, max_columns)
}

fn world_entry_view(
    index: usize,
    entry: &WorldEntryForm,
    compact: bool,
) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let numeric_fields: Element<'_, WorkbenchMessage> = if compact {
        column![
            row![
                container(indexed_input(
                    "workbench.field.order",
                    &entry.order,
                    index,
                    EntryField::Order
                ))
                .width(Length::FillPortion(1)),
                container(world_position_field(index, entry, true))
                    // 位置选项文本较长，给选择器更多宽度避免中文换行。
                    .width(Length::FillPortion(3)),
            ]
            .spacing(8),
            row![
                container(indexed_input(
                    "workbench.field.probability",
                    &entry.probability,
                    index,
                    EntryField::Probability,
                ))
                .width(Length::FillPortion(1)),
                container(indexed_input(
                    "workbench.field.depth",
                    &entry.depth,
                    index,
                    EntryField::Depth
                ))
                .width(Length::FillPortion(1)),
            ]
            .spacing(8),
        ]
        .spacing(8)
        .into()
    } else {
        row![
            container(indexed_input(
                "workbench.field.order",
                &entry.order,
                index,
                EntryField::Order
            ))
            .width(Length::FillPortion(1)),
            container(world_position_field(index, entry, false))
                // 位置选择器需要容纳完整的中文/英文选项，其余数字字段保持紧凑。
                .width(Length::FillPortion(4)),
            container(indexed_input(
                "workbench.field.probability",
                &entry.probability,
                index,
                EntryField::Probability,
            ))
            .width(Length::FillPortion(1)),
            container(indexed_input(
                "workbench.field.depth",
                &entry.depth,
                index,
                EntryField::Depth
            ))
            .width(Length::FillPortion(1)),
        ]
        .spacing(8)
        .into()
    };
    container(
        column![
            row![
                raw(format!(
                    "{} {}",
                    t_in("workbench.world.entry", language),
                    index + 1
                ))
                .size(13),
                space::horizontal(),
                checkbox(entry.enabled)
                    .label(t_in("workbench.field.enabled", language))
                    .on_toggle(move |value| WorkbenchMessage::EntryToggled(index, value)),
            ]
            .align_y(Alignment::Center),
            indexed_input(
                "workbench.field.comment",
                &entry.comment,
                index,
                EntryField::Comment
            ),
            row![
                entry_tags_field(index, entry, EntryTagKind::Keys),
                entry_tags_field(index, entry, EntryTagKind::SecondaryKeys),
            ]
            .spacing(8),
            area_field("workbench.field.content", &entry.content, move |action| {
                WorkbenchMessage::EntryContentChanged(index, action)
            }),
            numeric_fields,
        ]
        .spacing(9),
    )
    .width(Fill)
    .padding(12)
    .style(panel_surface)
    .into()
}

fn preset_view(form: &PresetForm, add_pending: bool) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let toolbar = container(
        row![
            column![
                section_title("workbench.preset.prompts"),
                text(if form.has_prompt_order {
                    t_in("workbench.preset.order_managed", language)
                } else {
                    t_in("workbench.preset.direct_managed", language)
                })
                .size(11)
                .style(crate::theme::muted_text_style),
            ]
            .spacing(2),
            space::horizontal(),
            button(
                row![
                    icons::icon(
                        if add_pending {
                            Icon::LoaderCircle
                        } else {
                            Icon::Plus
                        },
                        14,
                        BLUE_600
                    ),
                    raw(t_in(if add_pending {
                            "workbench.preset.adding"
                        } else {
                            "workbench.preset.add"
                        }, language))
                    .size(11),
                ]
                .spacing(6),
            )
            .on_press_maybe((!add_pending).then_some(WorkbenchMessage::AddPresetPrompt))
            .padding([7, 10])
            .style(button_style(ButtonVariant::Secondary)),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([12, 16]);

    let (start, end) = form.page_range(form.current_page);
    let scale = crate::core::typography::current_ui_scale();
    let cards = responsive(move |size| {
        let columns = adaptive_grid_columns(size.width, 330.0, scale, 4);
        let mut cards = column![].spacing(10);
        for chunk in (start..end).collect::<Vec<_>>().chunks(columns) {
            let mut line = row![].spacing(10);
            for index in chunk {
                line = line.push(
                    container(preset_prompt_view(*index, &form.prompts[*index]))
                        .width(Length::FillPortion(1)),
                );
            }
            for _ in chunk.len()..columns {
                line = line.push(container(space::horizontal()).width(Length::FillPortion(1)));
            }
            cards = cards.push(line);
        }
        cards.into()
    });

    let page_count = form.page_count();
    // 只有一页时不展示分页栏，但保留底栏与条目计数，避免卡片区域高度跳动。
    let pager: Element<'_, WorkbenchMessage> = if page_count > 1 {
        crate::pages::pager::pagination(
            form.current_page,
            page_count,
            WorkbenchMessage::PresetPageChanged,
        )
    } else {
        space::horizontal().width(Length::Shrink).into()
    };
    let status_bar = container(
        row![
            raw(format!(
                "{} {}",
                form.prompts.len(),
                t_in("workbench.preset.items", language),
            ))
            .size(11)
            .style(crate::theme::muted_text_style),
            space::horizontal(),
            pager,
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([8, 16])
    .align_y(Alignment::Center);

    column![
        toolbar,
        scrollable(container(cards).padding([0, 16])).height(Fill),
        status_bar,
    ]
    .height(Fill)
    .into()
}

fn preset_prompt_view(index: usize, prompt: &PresetPromptForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let state_label = if prompt.partial {
        t_in("workbench.preset.partial", language)
    } else if prompt.enabled {
        t_in("workbench.field.enabled", language)
    } else {
        t_in("workbench.field.disabled", language)
    };
    let mut body = column![
        row![
            column![
                text(if prompt.marker {
                    t_in("workbench.preset.marker", language)
                } else {
                    t_in("workbench.preset.prompt", language)
                })
                .size(11)
                .style(crate::theme::muted_text_style),
                raw(&prompt.identifier).size(12),
            ]
            .spacing(2),
            space::horizontal(),
            text(state_label)
                .size(11)
                .style(crate::theme::muted_text_style),
            checkbox(prompt.enabled)
                .on_toggle(move |value| { WorkbenchMessage::PresetToggled(index, value) }),
            button(icons::icon(Icon::Trash2, 14, DANGER))
                .on_press(WorkbenchMessage::RequestDeletePreset(index))
                .width(30)
                .height(30)
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(9);
    if !prompt.marker {
        body = body
            // 普通提示词的短字段横向排列，把卡片垂直空间留给正文编辑器。
            .push(
                row![
                    container(field_input(
                        "workbench.field.name",
                        &prompt.name,
                        move |value| {
                            WorkbenchMessage::PresetChanged(index, PresetField::Name, value)
                        }
                    ))
                    .width(Length::FillPortion(3)),
                    container(preset_role_field(index, prompt))
                    .width(Length::FillPortion(2)),
                ]
                .spacing(8),
            )
            .push(if let Some(content) = &prompt.content {
                area_field("workbench.field.content", content, move |action| {
                    WorkbenchMessage::PresetContentChanged(index, action)
                })
            } else {
                container(space::vertical()).into()
            });
    } else {
        body = body.push(field_input(
            "workbench.field.name",
            &prompt.name,
            move |value| WorkbenchMessage::PresetChanged(index, PresetField::Name, value),
        ));
    }
    container(body)
        .width(Fill)
        .padding(12)
        .style(panel_surface)
        .into()
}

fn world_position_field(
    index: usize,
    entry: &WorldEntryForm,
    compact: bool,
) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let selector_text_size = selector_text_size(compact);
    let selected = WorldPosition::from_values(&entry.position, &entry.position_role);
    column![
        raw(t_in("workbench.field.position", language))
            .size(11)
            .style(crate::theme::muted_text_style),
        pick_list(&WORLD_POSITIONS[..], Some(selected), move |position| {
            WorkbenchMessage::EntryPositionChanged(index, position)
        })
        .width(Fill)
        .padding([7, 9])
        .text_size(selector_text_size)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style),
    ]
    .spacing(4)
    .width(Fill)
    .into()
}

fn preset_role_field(index: usize, prompt: &PresetPromptForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let selector_text_size = selector_text_size(false);
    let selected = PresetRole::from_value(&prompt.role);
    column![
        raw(t_in("workbench.field.role", language))
            .size(11)
            .style(crate::theme::muted_text_style),
        pick_list(&PRESET_ROLES[..], Some(selected), move |role| {
            WorkbenchMessage::PresetRoleChanged(index, role)
        })
        .width(Fill)
        .padding([7, 9])
        .text_size(selector_text_size)
        .handle(pick_list_handle())
        .style(pick_list_style)
        .menu_style(pick_list_menu_style),
    ]
    .spacing(4)
    .width(Fill)
    .into()
}

/// 选择器字号跟随界面缩放，但限制在桌面工作台可读且不易换行的范围内。
fn selector_text_size(compact: bool) -> f32 {
    let base = if compact { 12.0 } else { 13.0 };
    (base * crate::core::typography::current_ui_scale()).clamp(11.0, 14.0)
}

fn entry_tags_field(
    index: usize,
    entry: &WorldEntryForm,
    kind: EntryTagKind,
) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let (label_key, placeholder_key) = match kind {
        EntryTagKind::Keys => (
            "workbench.field.keys",
            "workbench.world.keys.placeholder",
        ),
        EntryTagKind::SecondaryKeys => (
            "workbench.field.secondary_keys",
            "workbench.world.secondary_keys.placeholder",
        ),
    };
    let chips = row(
        entry
            .tags(kind)
            .iter()
            .enumerate()
            .map(move |(tag_index, tag)| entry_tag_chip(index, kind, tag_index, tag)),
    )
    .spacing(5);
    // 关键词只保留一行，超出卡片宽度时使用横向滚动查看，避免 Tags 数量改变卡片高度。
    let chips_view = scrollable(container(chips).width(Length::Shrink).padding([2, 0]))
        .width(Fill)
        .height(Length::Fixed(
            34.0 * crate::core::typography::current_ui_scale().max(0.85),
        ))
        .horizontal();
    let input = text_input(t_in(placeholder_key, language), entry.tag_input(kind))
        .on_input(move |value| WorkbenchMessage::EntryTagInputChanged(index, kind, value))
        .on_submit(WorkbenchMessage::AddEntryTag(index, kind))
        .padding([7, 9])
        .size(13)
        .style(text_input_style)
        .width(Fill);
    column![
        raw(t_in(label_key, language))
            .size(11)
            .style(crate::theme::muted_text_style),
        container(chips_view).width(Fill),
        row![
            input,
            button(
                container(icons::icon(Icon::Plus, 14, BLUE_600))
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .on_press_maybe(
                entry
                    .can_add_tag(kind)
                    .then_some(WorkbenchMessage::AddEntryTag(index, kind)),
            )
            .width(32)
            .height(32)
            .padding(0)
            .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    ]
    .spacing(5)
    .width(Fill)
    .into()
}

fn entry_tag_chip(
    entry_index: usize,
    kind: EntryTagKind,
    tag_index: usize,
    label: &str,
) -> Element<'_, WorkbenchMessage> {
    container(
        row![
            raw(label).size(11).color(BLUE_600),
            button(
                container(icons::icon(Icon::X, 11, DANGER))
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .on_press(WorkbenchMessage::RemoveEntryTag(
                entry_index,
                kind,
                tag_index,
            ))
            .width(20)
            .height(20)
            .padding(0)
            .style(icon_button_style),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3, 5])
    .style(accent_surface)
    .into()
}

fn character_tags_field(form: &CharacterForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let chips = row(
        form.tags
            .iter()
            .enumerate()
            .map(|(index, tag)| character_tag_chip(index, tag)),
    )
    .spacing(6)
    .wrap();
    let input = text_input(
        t_in("workbench.character.tags.placeholder", language),
        &form.tag_input,
    )
        .on_input(WorkbenchMessage::CharacterTagInputChanged)
        .on_submit(WorkbenchMessage::AddCharacterTag)
        .padding([7, 9])
        .size(13)
        .style(text_input_style)
        .width(Fill);
    column![
        raw(t_in("workbench.field.tags", language))
            .size(11)
            .style(crate::theme::muted_text_style),
        container(chips).width(Fill),
        row![
            input,
            button(
                container(icons::icon(Icon::Plus, 14, BLUE_600))
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
                .on_press_maybe(
                    (!form.tag_input.trim().is_empty())
                        .then_some(WorkbenchMessage::AddCharacterTag),
                )
                .width(32)
                .height(32)
                .padding(0)
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    ]
    .spacing(5)
    .width(Fill)
    .into()
}

fn character_tag_chip<'a>(index: usize, label: &'a str) -> Element<'a, WorkbenchMessage> {
    container(
        row![
            raw(label).size(11).color(BLUE_600),
            button(
                container(icons::icon(Icon::X, 11, DANGER))
                    .width(Fill)
                    .height(Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
                .on_press(WorkbenchMessage::RemoveCharacterTag(index))
                .width(20)
                .height(20)
                .padding(0)
                .style(icon_button_style),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3, 5])
    .style(accent_surface)
    .into()
}

fn field_input<'a>(
    key: &'static str,
    value: &'a str,
    on_input: impl Fn(String) -> WorkbenchMessage + 'a,
) -> Element<'a, WorkbenchMessage> {
    let label = t_in(key, current_language());
    column![
        text(label).size(11).style(crate::theme::muted_text_style),
        text_input(label, value)
            .on_input(on_input)
            .padding([7, 9])
            .size(13)
            .width(Fill)
            .style(text_input_style),
    ]
    .spacing(4)
    .width(Fill)
    .into()
}

fn indexed_input<'a>(
    key: &'static str,
    value: &'a str,
    index: usize,
    field: EntryField,
) -> Element<'a, WorkbenchMessage> {
    field_input(key, value, move |value| {
        WorkbenchMessage::EntryChanged(index, field, value)
    })
}

fn area_field<'a>(
    key: &'static str,
    content: &'a text_editor::Content,
    on_action: impl Fn(text_editor::Action) -> WorkbenchMessage + 'a,
) -> Element<'a, WorkbenchMessage> {
    let label = t_in(key, current_language());
    column![
        text(label).size(11).style(crate::theme::muted_text_style),
        text_editor::TextEditor::new(content)
            .placeholder(label)
            .on_action(on_action)
            .height(110)
            .padding([8, 9]),
    ]
    .spacing(4)
    .into()
}

fn section_title(key: &'static str) -> Element<'static, WorkbenchMessage> {
    raw(t_in(key, current_language()))
        .size(14)
        .font(crate::core::typography::medium())
        .into()
}

fn centered_state<'a>(
    icon: Icon,
    title_key: &'static str,
    action: Option<(&'a str, WorkbenchMessage)>,
) -> Element<'a, WorkbenchMessage> {
    let language = current_language();
    let mut content = column![
        icons::icon(icon, 30, BLUE_600),
        raw(t_in(title_key, language)).size(14),
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    if let Some((detail, message)) = action {
        content = content
            .push(raw(detail).size(11).style(crate::theme::muted_text_style))
            .push(
                button(raw(t_in("workbench.load.retry", language)).size(11))
                    .on_press(message)
                    .padding([7, 12])
                    .style(button_style(ButtonVariant::Secondary)),
            );
    }
    container(content)
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn world_picker(state: &WorkbenchState) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let mut list = column![].spacing(4);
    for option in &state.world_books {
        let path = option.path.clone();
        list = list.push(
            button(
                row![
                    icons::icon(Icon::BookOpenText, 14, BLUE_600),
                    raw(&option.name).size(12),
                ]
                .spacing(7),
            )
            .on_press(WorkbenchMessage::PickWorldBook(path))
            .width(Fill)
            .padding([8, 10])
            .style(button_style(ButtonVariant::Secondary)),
        );
    }
    if state.world_books.is_empty() {
        list = list.push(raw(t_in("workbench.world.no_options", language)).size(12));
    }
    modal_shell(
        column![
            row![
                raw(t_in("workbench.world.select", language)).size(15),
                space::horizontal(),
                button(icons::icon(Icon::X, 15, BLUE_600))
                    .on_press(WorkbenchMessage::HideWorldPicker)
                    .style(button_style(ButtonVariant::Secondary)),
            ]
            .align_y(Alignment::Center),
            scrollable(list).height(320),
        ]
        .spacing(12),
        520,
    )
}

fn confirmation_modal(confirmation: &Confirmation) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let (title, detail, confirm) = match confirmation {
        Confirmation::DeletePreset(_) => (
            t_in("workbench.confirm.delete.title", language),
            t_in("workbench.confirm.delete.detail", language),
            WorkbenchMessage::ConfirmDeletePreset,
        ),
        Confirmation::BindWorld(_) => (
            t_in("workbench.confirm.replace.title", language),
            t_in("workbench.confirm.replace.detail", language),
            WorkbenchMessage::ConfirmWorldAction,
        ),
        Confirmation::UnbindWorld => (
            t_in("workbench.confirm.unbind.title", language),
            t_in("workbench.confirm.unbind.detail", language),
            WorkbenchMessage::ConfirmWorldAction,
        ),
    };
    modal_shell(
        column![
            text(title).size(15),
            text(detail).size(12).style(crate::theme::muted_text_style),
            row![
                space::horizontal(),
                button(raw(t_in("workbench.confirm.cancel", language)).size(11))
                    .on_press(WorkbenchMessage::CancelConfirmation)
                    .padding([7, 12])
                    .style(button_style(ButtonVariant::Secondary)),
                button(raw(t_in("workbench.confirm.continue", language)).size(11))
                    .on_press(confirm)
                    .padding([7, 12])
                    .style(button_style(ButtonVariant::Primary)),
            ]
            .spacing(8),
        ]
        .spacing(12),
        460,
    )
}

fn modal_shell<'a>(
    content: impl Into<Element<'a, WorkbenchMessage>>,
    width: impl Into<Length>,
) -> Element<'a, WorkbenchMessage> {
    let backdrop = button(space::horizontal())
        .width(Fill)
        .height(Fill)
        .on_press(WorkbenchMessage::Interact)
        .style(backdrop_style);
    let dialog = container(content)
        .width(width)
        .max_height(430)
        .padding(16)
        .style(dialog_surface);
    stack![
        backdrop,
        container(mouse_area(dialog).on_press(WorkbenchMessage::Interact))
            .width(Fill)
            .height(Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    ]
    .into()
}

fn workbench_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::canvas(theme))),
        ..container::Style::default()
    }
}

fn header_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 0.0,
            radius: 0.0.into(),
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
            radius: 7.0.into(),
        },
        ..container::Style::default()
    }
}

fn dialog_surface(theme: &Theme) -> container::Style {
    let mut style = panel_surface(theme);
    style.shadow = iced::Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
        offset: iced::Vector::new(0.0, 8.0),
        blur_radius: 26.0,
    };
    style
}

fn accent_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            BLUE_600.r, BLUE_600.g, BLUE_600.b, 0.10,
        ))),
        border: Border {
            radius: 6.0.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// 标签操作只保留图标本身，避免小按钮背景挤压标签内容；固定尺寸和零内边距保证图标居中。
fn icon_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: None,
        border: Border::default(),
        ..button::Style::default()
    }
}

fn error_surface(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            DANGER.r, DANGER.g, DANGER.b, 0.08,
        ))),
        border: Border {
            color: Color::from_rgba(DANGER.r, DANGER.g, DANGER.b, 0.24),
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

fn backdrop_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.58))),
        ..button::Style::default()
    }
}
