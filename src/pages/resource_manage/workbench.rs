//! 角色卡、世界书与预设的独立资源工作台。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::{
    button, checkbox, column, container, mouse_area, row, scrollable, space, stack, text_editor,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Fill, Length, Task, Theme};
use lucide_icons::Icon;

use astra_ui::{BLUE_600, ButtonVariant, DANGER, SUCCESS, icons};

use crate::core::library::ResourceKind;
use crate::core::library::edit::{
    EditableCharacter, EditablePreset, EditablePresetPrompt, EditableWorldBook, EditableWorldEntry,
    EditorData, LoadedEditor, create_preset_prompt, load_editor, load_world_book_for_binding,
    save_editor,
};
use crate::lang::{current_language, t, text};
use crate::theme::button_style;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(500);
// 预设只构建当前页的编辑器，避免大量条目同时参与布局和绘制。
const PRESET_PAGE_SIZE: usize = 12;

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
    Tags,
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
    Keys,
    SecondaryKeys,
    Order,
    Position,
    Probability,
    Depth,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PresetField {
    Name,
    Role,
}

#[derive(Debug, Clone)]
pub(crate) enum WorkbenchMessage {
    Loaded(u64, Result<PreparedParcel, String>),
    RetryLoad,
    CharacterChanged(CharacterField, String),
    CharacterAreaChanged(CharacterArea, text_editor::Action),
    WorldChanged(WorldField, String),
    EntryChanged(usize, EntryField, String),
    EntryContentChanged(usize, text_editor::Action),
    EntryToggled(usize, bool),
    PresetChanged(usize, PresetField, String),
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
    name: String,
    creator: String,
    version: String,
    tags: String,
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
    keys: String,
    secondary_keys: String,
    content: text_editor::Content,
    enabled: bool,
    order: String,
    position: String,
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
                    }
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
            WorkbenchMessage::CharacterChanged(field, value) => {
                if let Some(WorkbenchForm::Character(form)) = &mut self.form {
                    match field {
                        CharacterField::Name => form.name.clone_from(&value),
                        CharacterField::Creator => form.creator.clone_from(&value),
                        CharacterField::Version => form.version.clone_from(&value),
                        CharacterField::Tags => form.tags.clone_from(&value),
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
                        CharacterField::Tags => character.tags = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::CharacterAreaChanged(field, action) => {
                let edited = action.is_edit();
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
                if let Some(entry) = self
                    .world_form_mut()
                    .and_then(|form| form.entries.get_mut(index))
                {
                    match field {
                        EntryField::Comment => entry.comment.clone_from(&value),
                        EntryField::Keys => entry.keys.clone_from(&value),
                        EntryField::SecondaryKeys => entry.secondary_keys.clone_from(&value),
                        EntryField::Order => entry.order.clone_from(&value),
                        EntryField::Position => entry.position.clone_from(&value),
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
                        EntryField::Keys => entry.keys = value,
                        EntryField::SecondaryKeys => entry.secondary_keys = value,
                        EntryField::Order => entry.order = value,
                        EntryField::Position => entry.position = value,
                        EntryField::Probability => entry.probability = value,
                        EntryField::Depth => entry.depth = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::EntryContentChanged(index, action) => {
                let edited = action.is_edit();
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
                if let Some(prompt) = self.preset_prompt_mut(index) {
                    match field {
                        PresetField::Name => prompt.name.clone_from(&value),
                        PresetField::Role => prompt.role.clone_from(&value),
                    }
                }
                self.update_preset(|preset| {
                    let Some(prompt) = preset.prompts.get_mut(index) else {
                        return;
                    };
                    match field {
                        PresetField::Name => prompt.name = value,
                        PresetField::Role => prompt.role = value,
                    }
                });
                (self.mark_changed(false), WorkbenchEvent::None)
            }
            WorkbenchMessage::PresetContentChanged(index, action) => {
                let edited = action.is_edit();
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
                        self.operation_error = Some(error);
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
                    Err(error) => self.save_error = Some(error),
                }
                (Task::none(), WorkbenchEvent::None)
            }
            WorkbenchMessage::ConfirmDeletePreset => {
                let Some(Confirmation::DeletePreset(index)) = self.confirmation.take() else {
                    return (Task::none(), WorkbenchEvent::None);
                };
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
                    Some(WorkbenchForm::Preset(form)) => form
                        .current_page
                        .min(form.page_count().saturating_sub(1)),
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
                        self.save_error = Some(error);
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
                if self.preset_add_pending
                    || self.saving_revision.is_some()
                    || self.revision > self.saved_revision
                {
                    self.close_pending = true;
                    if !self.preset_add_pending && self.saving_revision.is_none() {
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
                        .map_err(|_| "资源编辑状态已损坏。".to_owned())?
                        .clone();
                    let saved = save_editor(snapshot)?;
                    loaded
                        .lock()
                        .map_err(|_| "资源编辑状态已损坏。".to_owned())?
                        .adopt_saved_document(saved);
                    Ok(())
                })
                .await
                .unwrap_or_else(|_| Err("资源保存线程意外退出。".to_owned()))
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
    }
}

fn load_task(path: PathBuf, kind: WorkbenchKind, serial: u64) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let loaded = load_editor(&path, kind.resource_kind())?;
                // 预设可能包含大量条目，文本编辑器内容也必须在后台一次性准备。
                let form = form_from_data(&loaded.data);
                Ok(Arc::new(Mutex::new(Some(PreparedWorkbench {
                    loaded,
                    form,
                }))))
            })
            .await
            .unwrap_or_else(|_| Err("资源加载线程意外退出。".to_owned()))
        },
        move |result| WorkbenchMessage::Loaded(serial, result),
    )
}

fn take_prepared(parcel: PreparedParcel) -> Result<PreparedWorkbench, String> {
    parcel
        .lock()
        .map_err(|_| "工作台加载结果已损坏。".to_owned())?
        .take()
        .ok_or_else(|| "工作台加载结果已被读取。".to_owned())
}

fn add_preset_prompt_task(loaded: Arc<Mutex<LoadedEditor>>) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let loaded = loaded
                    .lock()
                    .map_err(|_| "资源编辑状态已损坏。".to_owned())?;
                let EditorData::Preset(preset) = &loaded.data else {
                    return Err("当前工作台不是预设编辑器。".to_owned());
                };
                Ok(create_preset_prompt(&preset.prompts))
            })
            .await
            .unwrap_or_else(|_| Err("预设条目创建线程意外退出。".to_owned()))
        },
        WorkbenchMessage::PresetPromptAdded,
    )
}

fn bind_world_task(path: PathBuf) -> Task<WorkbenchMessage> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || load_world_book_for_binding(&path))
                .await
                .unwrap_or_else(|_| Err("世界书加载线程意外退出。".to_owned()))
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

impl From<&EditableCharacter> for CharacterForm {
    fn from(value: &EditableCharacter) -> Self {
        Self {
            name: value.name.clone(),
            creator: value.creator.clone(),
            version: value.version.clone(),
            tags: value.tags.clone(),
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
            keys: value.keys.clone(),
            secondary_keys: value.secondary_keys.clone(),
            content: text_editor::Content::with_text(&value.content),
            enabled: value.enabled,
            order: value.order.clone(),
            position: value.position.clone(),
            probability: value.probability.clone(),
            depth: value.depth.clone(),
        }
    }
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
        let start = page.saturating_mul(PRESET_PAGE_SIZE).min(self.prompts.len());
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
            text(t("workbench.save.failed", language))
                .size(11)
                .color(DANGER),
            button(text(t("workbench.save.retry", language)).size(11))
                .on_press(WorkbenchMessage::RetrySave)
                .padding([5, 9])
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(7)
        .align_y(Alignment::Center)
    } else if state.saving_revision.is_some() || state.close_pending || state.preset_add_pending {
        row![
            icons::icon(Icon::LoaderCircle, 14, BLUE_600),
            text(if state.close_pending {
                t("workbench.save.closing", language)
            } else if state.preset_add_pending {
                t("workbench.preset.adding", language)
            } else {
                t("workbench.save.saving", language)
            })
            .size(11),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    } else {
        row![
            icons::icon(Icon::CircleCheck, 14, SUCCESS),
            text(t("workbench.save.saved", language)).size(11),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
    };
    container(
        row![
            container(icons::icon(state.kind.icon(), 18, BLUE_600))
                .width(36)
                .height(36)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(accent_surface),
            column![
                text(t(state.kind.title_key(), language)).size(17),
                text(
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
            status,
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
                    text(error).size(11).width(Fill).color(DANGER),
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
    let metadata = column![
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
        field_input("workbench.field.tags", &form.tags, |value| {
            WorkbenchMessage::CharacterChanged(CharacterField::Tags, value)
        }),
        area_field(
            "workbench.character.description",
            &form.description,
            |action| { WorkbenchMessage::CharacterAreaChanged(CharacterArea::Description, action) }
        ),
        area_field(
            "workbench.character.personality",
            &form.personality,
            |action| { WorkbenchMessage::CharacterAreaChanged(CharacterArea::Personality, action) }
        ),
        area_field("workbench.character.scenario", &form.scenario, |action| {
            WorkbenchMessage::CharacterAreaChanged(CharacterArea::Scenario, action)
        }),
        area_field(
            "workbench.character.first_message",
            &form.first_message,
            |action| {
                WorkbenchMessage::CharacterAreaChanged(CharacterArea::FirstMessage, action)
            }
        ),
    ]
    .spacing(10);

    let world_controls = row![
        button(
            row![
                icons::icon(Icon::BookPlus, 14, BLUE_600),
                text(if form.world_book.is_some() {
                    t("workbench.world.change", language)
                } else {
                    t("workbench.world.bind", language)
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
                text(t("workbench.world.unbind", language)).size(11),
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
            container(text(t("workbench.world.empty", language)).size(12))
                .width(Fill)
                .padding(18)
                .style(panel_surface),
        ]
        .spacing(10)
        .into()
    };
    scrollable(container(column![metadata, world].spacing(18)).padding(16))
        .height(Fill)
        .into()
}

fn world_view(form: &WorldBookForm, embedded: bool) -> Element<'_, WorkbenchMessage> {
    let mut content = column![
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
        section_title("workbench.world.entries"),
    ]
    .spacing(10);
    for (index, entry) in form.entries.iter().enumerate() {
        content = content.push(world_entry_view(index, entry));
    }
    if embedded {
        content.into()
    } else {
        scrollable(container(content).padding(16))
            .height(Fill)
            .into()
    }
}

fn world_entry_view(index: usize, entry: &WorldEntryForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    container(
        column![
            row![
                text(format!(
                    "{} {}",
                    t("workbench.world.entry", language),
                    index + 1
                ))
                .size(13),
                space::horizontal(),
                checkbox(entry.enabled)
                    .label(t("workbench.field.enabled", language))
                    .on_toggle(move |value| WorkbenchMessage::EntryToggled(index, value)),
            ]
            .align_y(Alignment::Center),
            row![
                indexed_input(
                    "workbench.field.comment",
                    &entry.comment,
                    index,
                    EntryField::Comment
                ),
                indexed_input("workbench.field.keys", &entry.keys, index, EntryField::Keys),
                indexed_input(
                    "workbench.field.secondary_keys",
                    &entry.secondary_keys,
                    index,
                    EntryField::SecondaryKeys,
                ),
            ]
            .spacing(8),
            area_field("workbench.field.content", &entry.content, move |action| {
                WorkbenchMessage::EntryContentChanged(index, action)
            }),
            row![
                indexed_input(
                    "workbench.field.order",
                    &entry.order,
                    index,
                    EntryField::Order
                ),
                indexed_input(
                    "workbench.field.position",
                    &entry.position,
                    index,
                    EntryField::Position,
                ),
                indexed_input(
                    "workbench.field.probability",
                    &entry.probability,
                    index,
                    EntryField::Probability,
                ),
                indexed_input(
                    "workbench.field.depth",
                    &entry.depth,
                    index,
                    EntryField::Depth
                ),
            ]
            .spacing(8),
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
                    t("workbench.preset.order_managed", language)
                } else {
                    t("workbench.preset.direct_managed", language)
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
                    text(t(
                        if add_pending {
                            "workbench.preset.adding"
                        } else {
                            "workbench.preset.add"
                        },
                        language
                    ))
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
    let mut cards = column![].spacing(10);
    for index in (start..end).step_by(2) {
        let left = preset_prompt_view(index, &form.prompts[index]);
        let right: Element<'_, WorkbenchMessage> = if index + 1 < end {
            preset_prompt_view(index + 1, &form.prompts[index + 1])
        } else {
            container(space::horizontal()).width(Fill).into()
        };
        cards = cards.push(row![left, right].spacing(10));
    }

    let page_count = form.page_count();
    // 禁用按钮也可能构建消息，因此分页索引必须使用检查算术，不能预先执行 0 - 1。
    let previous_page = form
        .current_page
        .checked_sub(1)
        .map(WorkbenchMessage::PresetPageChanged);
    let next_page = form
        .current_page
        .checked_add(1)
        .filter(|page| *page < page_count)
        .map(WorkbenchMessage::PresetPageChanged);
    let pager = container(
        row![
            button(icons::icon(Icon::ChevronLeft, 16, BLUE_600))
                .on_press_maybe(previous_page)
                .width(34)
                .height(30)
                .style(button_style(ButtonVariant::Secondary)),
            text(format!(
                "{} {} / {} · {} {}",
                t("workbench.preset.page", language),
                form.current_page.saturating_add(1),
                page_count,
                form.prompts.len(),
                t("workbench.preset.items", language),
            ))
            .size(11)
            .style(crate::theme::muted_text_style),
            button(icons::icon(Icon::ChevronRight, 16, BLUE_600))
                .on_press_maybe(next_page)
                .width(34)
                .height(30)
                .style(button_style(ButtonVariant::Secondary)),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding([8, 16])
    .align_x(Alignment::Center);

    column![
        toolbar,
        scrollable(container(cards).padding([0, 16])).height(Fill),
        pager,
    ]
    .height(Fill)
    .into()
}

fn preset_prompt_view(index: usize, prompt: &PresetPromptForm) -> Element<'_, WorkbenchMessage> {
    let language = current_language();
    let state_label = if prompt.partial {
        t("workbench.preset.partial", language)
    } else if prompt.enabled {
        t("workbench.field.enabled", language)
    } else {
        t("workbench.field.disabled", language)
    };
    let mut body = column![
        row![
            column![
                text(if prompt.marker {
                    t("workbench.preset.marker", language)
                } else {
                    t("workbench.preset.prompt", language)
                })
                .size(11)
                .style(crate::theme::muted_text_style),
                text(&prompt.identifier).size(12),
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
        field_input("workbench.field.name", &prompt.name, move |value| {
            WorkbenchMessage::PresetChanged(index, PresetField::Name, value)
        }),
    ]
    .spacing(9);
    if !prompt.marker {
        body = body
            .push(field_input(
                "workbench.field.role",
                &prompt.role,
                move |value| WorkbenchMessage::PresetChanged(index, PresetField::Role, value),
            ))
            .push(if let Some(content) = &prompt.content {
                area_field("workbench.field.content", content, move |action| {
                    WorkbenchMessage::PresetContentChanged(index, action)
                })
            } else {
                container(space::vertical()).into()
            });
    }
    container(body)
        .width(Fill)
        .padding(12)
        .style(panel_surface)
        .into()
}

fn field_input<'a>(
    key: &'static str,
    value: &'a str,
    on_input: impl Fn(String) -> WorkbenchMessage + 'a,
) -> Element<'a, WorkbenchMessage> {
    let label = t(key, current_language());
    column![
        text(label).size(11).style(crate::theme::muted_text_style),
        text_input(label, value).on_input(on_input).padding([7, 9]),
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
    let label = t(key, current_language());
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
    text(t(key, current_language()))
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
        text(t(title_key, language)).size(14),
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    if let Some((detail, message)) = action {
        content = content
            .push(text(detail).size(11).style(crate::theme::muted_text_style))
            .push(
                button(text(t("workbench.load.retry", language)).size(11))
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
                    text(&option.name).size(12),
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
        list = list.push(text(t("workbench.world.no_options", language)).size(12));
    }
    modal_shell(
        column![
            row![
                text(t("workbench.world.select", language)).size(15),
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
            t("workbench.confirm.delete.title", language),
            t("workbench.confirm.delete.detail", language),
            WorkbenchMessage::ConfirmDeletePreset,
        ),
        Confirmation::BindWorld(_) => (
            t("workbench.confirm.replace.title", language),
            t("workbench.confirm.replace.detail", language),
            WorkbenchMessage::ConfirmWorldAction,
        ),
        Confirmation::UnbindWorld => (
            t("workbench.confirm.unbind.title", language),
            t("workbench.confirm.unbind.detail", language),
            WorkbenchMessage::ConfirmWorldAction,
        ),
    };
    modal_shell(
        column![
            text(title).size(15),
            text(detail).size(12).style(crate::theme::muted_text_style),
            row![
                space::horizontal(),
                button(text(t("workbench.confirm.cancel", language)).size(11))
                    .on_press(WorkbenchMessage::CancelConfirmation)
                    .padding([7, 12])
                    .style(button_style(ButtonVariant::Secondary)),
                button(text(t("workbench.confirm.continue", language)).size(11))
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
