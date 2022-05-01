mod bindings;
pub mod buffer;
mod windows;

pub use self::buffer::{BufferId, ModifiedStatus};

use git2::Repository;
use ropey::Rope;
use std::{
    borrow::Cow,
    fmt::Display,
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
    sync::Arc,
};
use zi::{
    Bindings, Callback, Component, ComponentExt, ComponentLink, FlexBasis, FlexDirection, Item,
    Key, Layout, NamedBindingQuery, Rect, ShouldRender,
};

use zee_grammar::Mode;

use crate::{
    clipboard::Clipboard,
    components::{
        buffer::{Buffer as BufferView, Properties as BufferViewProperties},
        prompt::{
            buffers::BufferEntry, picker::FileSource, Action as PromptAction, Prompt,
            Properties as PromptProperties, PROMPT_INACTIVE_HEIGHT,
        },
        splash::{Properties as SplashProperties, Splash},
        theme::{Theme, THEMES},
    },
    config::{EditorConfig, PLAIN_TEXT_MODE},
    error::Result,
    task::TaskPool,
};

use self::{
    bindings::KeySequenceSlice,
    buffer::{BufferCursor, Buffers, BuffersMessage, CursorId, RepositoryRc},
    windows::{CycleFocus, Window, WindowTree},
};

#[derive(Debug)]
pub enum Message {
    // Windows
    DeleteWindow,
    FocusNextWindow,
    FocusPreviousWindow,
    SplitWindow(FlexDirection),
    FullscreenWindow,

    // Prompt
    SelectBufferPicker,
    SelectBuffer(BufferId),
    KillBufferPicker,
    KillBuffer(BufferId),
    OpenFilePicker(FileSource),
    OpenFile(PathBuf),
    ChangePromptHeight(usize),
    Buffer(BuffersMessage),
    Log(Option<String>),

    // Global
    ChangeTheme,
    Cancel,
    Quit,
}

impl From<BuffersMessage> for Message {
    fn from(message: BuffersMessage) -> Message {
        Message::Buffer(message)
    }
}

pub struct Properties {
    pub args_files: Vec<PathBuf>,
    pub current_working_dir: PathBuf,
    pub config: EditorConfig,
    pub task_pool: TaskPool,
    pub clipboard: Arc<dyn Clipboard>,
}

pub struct Context {
    pub args_files: Vec<PathBuf>,
    pub current_working_dir: PathBuf,
    pub config: EditorConfig,
    pub modes: Vec<Mode>,
    pub task_pool: TaskPool,
    pub clipboard: Arc<dyn Clipboard>,
    pub link: ComponentLink<Editor>,
}

impl Context {
    pub fn mode_by_filename(&self, filename: impl AsRef<Path>) -> &Mode {
        self.modes
            .iter()
            .find(|&mode| mode.matches_by_filename(filename.as_ref()))
            .unwrap_or(&PLAIN_TEXT_MODE)
    }
}

#[derive(Clone)]
pub struct ContextHandle(pub &'static Context);

impl std::ops::Deref for ContextHandle {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl Context {
    pub fn log(&self, message: impl Into<String>) {
        self.link.send(Message::Log(Some(message.into())));
    }
}

pub struct Editor {
    context: ContextHandle,
    themes: &'static [(Theme, &'static str)],
    theme_index: usize,

    prompt_action: PromptAction,
    prompt_height: usize,

    buffers: Buffers,
    windows: WindowTree<BufferViewId>,
}

impl Editor {
    #[inline]
    fn focus_on_buffer(&mut self, buffer_id: BufferId) {
        if self.windows.is_empty() {
            self.windows
                .add(BufferViewId::new(buffer_id, CursorId::default()));
        } else {
            self.windows
                .set_focused(BufferViewId::new(buffer_id, CursorId::default()));
        }
    }

    fn open_file(&mut self, file_path: PathBuf) -> Result<bool> {
        // Check if the buffer is already open
        if let Some(buffer_id) = self.buffers.find_by_path(&file_path) {
            self.focus_on_buffer(buffer_id);
            return Ok(false);
        }

        let (is_new_file, text) = if file_path.exists() {
            (
                false,
                Rope::from_reader(BufReader::new(File::open(&file_path)?))?,
            )
        } else {
            // Optimistically check if we can create it
            let is_new_file = File::open(&file_path)
                .map(|_| false)
                .or_else(|error| match error.kind() {
                    io::ErrorKind::NotFound => {
                        self.context.log("[New file]");
                        Ok(true)
                    }
                    io::ErrorKind::PermissionDenied => {
                        self.context.log(format!(
                            "Permission denied while opening {}",
                            file_path.display()
                        ));
                        Err(error)
                    }
                    _ => {
                        self.context.log(format!(
                            "Could not open {} ({})",
                            file_path.display(),
                            error
                        ));
                        Err(error)
                    }
                })?;
            (is_new_file, Rope::new())
        };

        let repo = Repository::discover(&file_path).ok().map(RepositoryRc::new);

        // Store the new buffer
        let buffer_id = self.buffers.add(text, Some(file_path), repo);

        // Focus on the new buffer
        self.focus_on_buffer(buffer_id);

        Ok(is_new_file)
    }

    fn open_buffer_picker(&mut self, message: Cow<'static, str>, on_select: Callback<BufferId>) {
        self.prompt_action = PromptAction::PickBuffer {
            message,
            entries: self
                .buffers
                .iter()
                .map(|buffer| {
                    BufferEntry::new(
                        buffer.id(),
                        buffer.file_path().cloned(),
                        false,
                        buffer.edit_tree().len_bytes(),
                        buffer.mode(),
                    )
                })
                .collect(),
            on_select,
            on_change_height: self.context.link.callback(Message::ChangePromptHeight),
        };
        self.prompt_height = self.prompt_action.initial_height();
    }
}

impl Component for Editor {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        for (index, file_path) in properties.args_files.iter().cloned().enumerate() {
            link.send(Message::OpenFile(file_path));
            if index < properties.args_files.len().saturating_sub(1) {
                link.send(Message::SplitWindow(FlexDirection::Row));
            }
        }

        let theme_name = properties.config.theme.clone();
        let context = ContextHandle(Box::leak(
            Context {
                args_files: properties.args_files,
                current_working_dir: properties.current_working_dir,
                modes: properties
                    .config
                    .modes
                    .iter()
                    .cloned()
                    .map(Mode::new)
                    .collect(),
                config: properties.config,
                task_pool: properties.task_pool,
                clipboard: properties.clipboard,
                link,
            }
            .into(),
        ));

        let theme_index = {
            let theme = THEMES.iter().position(|(_, name)| *name == theme_name);
            if theme.is_none() {
                context.log(format!("Unknown theme `{}`", theme_name));
            }
            theme
        }
        .unwrap_or(0);

        Self {
            themes: &THEMES,
            theme_index,
            prompt_action: PromptAction::None,
            prompt_height: PROMPT_INACTIVE_HEIGHT,
            buffers: Buffers::new(context.clone()),
            context,
            windows: WindowTree::new(),
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        match message {
            Message::Cancel => {
                self.prompt_action = PromptAction::None;
                self.prompt_height = self.prompt_action.initial_height();
                self.context.log("Cancel");
            }
            Message::ChangeTheme => {
                self.theme_index = (self.theme_index + 1) % self.themes.len();
                if !self.prompt_action.is_interactive() {
                    self.context.log(format!(
                        "Theme changed to {}",
                        self.themes[self.theme_index].1
                    ));
                }
            }
            Message::OpenFilePicker(source) if !self.prompt_action.is_interactive() => {
                self.prompt_action = PromptAction::OpenFile {
                    source,
                    on_open: self.context.link.callback(Message::OpenFile),
                    on_change_height: self.context.link.callback(Message::ChangePromptHeight),
                };
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::OpenFile(path) => {
                self.prompt_action = self.open_file(path).map_or_else(
                    |error| PromptAction::Log {
                        message: format!("Could not open file: {}", error),
                    },
                    |new_file| {
                        if new_file {
                            PromptAction::Log {
                                message: "[New file]".into(),
                            }
                        } else {
                            PromptAction::None
                        }
                    },
                );
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::SelectBufferPicker if !self.prompt_action.is_interactive() => {
                self.open_buffer_picker(
                    "buffer".into(),
                    self.context.link.callback(Message::SelectBuffer),
                );
            }
            Message::SelectBuffer(buffer_id) => {
                self.prompt_action = PromptAction::None;
                self.prompt_height = self.prompt_action.initial_height();
                self.focus_on_buffer(buffer_id);
            }
            Message::KillBufferPicker if !self.prompt_action.is_interactive() => {
                self.open_buffer_picker(
                    "kill buffer".into(),
                    self.context.link.callback(Message::KillBuffer),
                );
            }
            Message::KillBuffer(buffer_id) => {
                self.prompt_action = PromptAction::None;
                self.prompt_height = self.prompt_action.initial_height();
                let removed_buffer = self.buffers.remove(buffer_id);
                debug_assert!(removed_buffer.is_some());
                if self.buffers.is_empty() {
                    self.windows.clear();
                } else {
                    let some_buffer = self.buffers.iter_mut().next().unwrap();
                    self.windows.nodes_mut().for_each(|view_id| {
                        if view_id.buffer_id == buffer_id {
                            *view_id =
                                BufferViewId::new(some_buffer.id(), some_buffer.new_cursor());
                        }
                    });
                }
            }
            Message::ChangePromptHeight(height) => {
                self.prompt_height = height;
            }
            Message::FocusNextWindow => self.windows.cycle_focus(CycleFocus::Next),
            Message::FocusPreviousWindow => self.windows.cycle_focus(CycleFocus::Previous),
            Message::SplitWindow(direction) if !self.buffers.is_empty() => {
                if let Some(view_id) = self.windows.get_focused() {
                    let buffer = self.buffers.get_mut(view_id.buffer_id).unwrap();
                    self.windows.insert_at_focused(
                        BufferViewId::new(
                            view_id.buffer_id,
                            buffer.duplicate_cursor(view_id.cursor_id),
                        ),
                        direction,
                    );
                }
            }
            Message::FullscreenWindow if !self.buffers.is_empty() => {
                self.windows.delete_all_except_focused();
            }
            Message::DeleteWindow if !self.buffers.is_empty() => {
                self.windows.delete_focused();
            }
            Message::Log(message) if !self.prompt_action.is_interactive() => {
                self.prompt_action = message
                    .map(|message| PromptAction::Log { message })
                    .unwrap_or(PromptAction::None);
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::Quit => {
                self.context.link.exit();
            }
            Message::Buffer(message) => self.buffers.handle_message(message),
            _ => {}
        }
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let buffers = if self.windows.is_empty() {
            Splash::item_with_key(
                FlexBasis::Auto,
                "splash",
                SplashProperties {
                    theme: Cow::Borrowed(&self.themes[self.theme_index].0.splash),
                },
            )
        } else {
            Item::auto(self.windows.layout(&mut |Window { id, focused, index }| {
                let buffer = self.buffers.get(id.buffer_id).unwrap();
                BufferView::with_key(
                    format!("{}.{}", index, id).as_str(),
                    BufferViewProperties {
                        context: self.context.clone(),
                        theme: Cow::Borrowed(&self.themes[self.theme_index].0.buffer),
                        focused: focused && !self.prompt_action.is_interactive(),
                        frame_id: index.one_based_index(),
                        mode: buffer.mode(),
                        repo: buffer.repository().cloned(),
                        content: buffer.edit_tree_handle(),
                        file_path: buffer.file_path().cloned(),
                        cursor: BufferCursor::new(
                            id.buffer_id,
                            id.cursor_id,
                            buffer.cursor(id.cursor_id).clone(),
                            self.context.link.clone(),
                        ),
                        parse_tree: buffer.parse_tree().cloned(),
                        modified_status: buffer.modified_status(),
                    },
                )
            }))
        };

        Layout::column([
            buffers,
            Prompt::item_with_key(
                FlexBasis::Fixed(if self.prompt_action.is_none() {
                    PROMPT_INACTIVE_HEIGHT
                } else {
                    self.prompt_height
                }),
                "prompt",
                PromptProperties {
                    context: self.context.clone(),
                    theme: Cow::Borrowed(&self.themes[self.theme_index].0.prompt),
                    action: self.prompt_action.clone(),
                },
            ),
        ])
    }

    fn bindings(&self, bindings: &mut Bindings<Self>) {
        if bindings.is_empty() {
            bindings::initialize(bindings);
        }
    }

    fn notify_binding_queries(&self, queries: &[Option<NamedBindingQuery>], keys: &[Key]) {
        let merge_queries = |lhs, rhs| match (lhs, rhs) {
            (some_match @ Some(NamedBindingQuery::Match(_)), _)
            | (_, some_match @ Some(NamedBindingQuery::Match(_))) => some_match,
            (
                Some(NamedBindingQuery::PrefixOf(mut lhs)),
                Some(NamedBindingQuery::PrefixOf(rhs)),
            ) => {
                lhs.extend(rhs.into_iter());
                Some(NamedBindingQuery::PrefixOf(lhs))
            }
            (some @ Some(_), None) | (None, some @ Some(_)) => some,
            (None, None) => None,
        };
        let merged_without_self = queries
            .iter()
            .skip(1)
            .cloned()
            .reduce(merge_queries)
            .flatten();
        let merged_all = queries.iter().cloned().reduce(merge_queries).flatten();

        match merged_all {
            Some(NamedBindingQuery::Match(_command)) => match merged_without_self {
                Some(NamedBindingQuery::Match(_command)) if self.prompt_action.is_log() => {
                    // Clear log message
                    self.context.link.send(Message::Log(None));
                }
                _ => {}
            },
            Some(NamedBindingQuery::PrefixOf(prefix_of)) => {
                self.context.log(format!(
                    "{} ({} commands)",
                    KeySequenceSlice::new(keys, true),
                    prefix_of.len()
                ));
            }
            None => {
                self.context.log(format!(
                    "{} is undefined",
                    KeySequenceSlice::new(keys, false)
                ));
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BufferViewId {
    buffer_id: BufferId,
    cursor_id: CursorId,
}

impl BufferViewId {
    fn new(buffer_id: BufferId, cursor_id: CursorId) -> Self {
        Self {
            buffer_id,
            cursor_id,
        }
    }
}

impl Display for BufferViewId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "BufferViewId(buffer={}, cursor={})",
            self.buffer_id, self.cursor_id
        )
    }
}
