mod windows;

use git2::Repository;
use ropey::Rope;
use std::{
    borrow::Cow,
    collections::hash_map::HashMap,
    fmt::Display,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};
use zi::{
    BindingMatch, BindingTransition, Callback, Component, ComponentExt, ComponentLink, FlexBasis,
    FlexDirection, Item, Key, Layout, Rect, ShouldRender,
};

use crate::{
    clipboard::Clipboard,
    components::{
        buffer::{Buffer, Properties as BufferProperties, RepositoryRc},
        prompt::{
            buffers::BufferEntry, picker::FileSource, Action as PromptAction, Prompt,
            Properties as PromptProperties, PROMPT_INACTIVE_HEIGHT,
        },
        splash::{Properties as SplashProperties, Splash},
        theme::{Theme, THEMES},
    },
    error::Result,
    mode::{self, Mode},
    settings::Settings,
    task::TaskPool,
};

use self::windows::{CycleFocus, Window, WindowTree};

#[derive(Clone, Debug)]
pub enum Message {
    ChangeTheme,
    ClosePane,
    FocusNextComponent,
    FocusPreviousComponent,
    SplitWindow(FlexDirection),
    FullscreenWindow,
    KeyPressed,
    OpenBufferSwitcher,
    ChangePromptHeight(usize),
    OpenFilePicker(FileSource),
    OpenFile(PathBuf),
    SelectBuffer(BufferId),
    Log(Option<String>),
    Cancel,
    Quit,
}

pub struct Context {
    pub args_files: Vec<PathBuf>,
    pub current_working_dir: PathBuf,
    pub settings: Settings,
    pub task_pool: TaskPool,
    pub clipboard: Arc<dyn Clipboard>,
}

pub struct Editor {
    context: Rc<Context>,
    link: ComponentLink<Self>,
    themes: &'static [(Theme, &'static str)],
    theme_index: usize,

    prompt_action: PromptAction,
    prompt_height: usize,

    buffers: HashMap<BufferId, OpenBuffer>,
    next_buffer_id: usize,
    windows: WindowTree<BufferId>,
    log_message: Callback<String>,
}

pub struct OpenBuffer {
    mode: &'static Mode,
    repo: Option<RepositoryRc>,
    content: Rope,
    file_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferId(usize);

impl Display for BufferId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "BufferId({})", self.0)
    }
}

impl Editor {
    fn open_file(&mut self, file_path: PathBuf) -> Result<bool> {
        // Check if the buffer is already open
        if let Some(buffer_id) = self
            .buffers
            .iter()
            .find(|(_, buffer)| {
                buffer
                    .file_path
                    .as_ref()
                    .map(|buffer_path| *buffer_path == file_path)
                    .unwrap_or(false)
            })
            .map(|(buffer_id, _)| buffer_id)
        {
            if self.windows.is_empty() {
                self.windows.add(*buffer_id);
            } else {
                self.windows.set_focused(*buffer_id);
            }
            return Ok(false);
        }

        let mode = mode::find_by_filename(&file_path);
        let repo = Repository::discover(&file_path).ok();
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
                        self.log_message("[New file]".into());
                        Ok(true)
                    }
                    io::ErrorKind::PermissionDenied => {
                        self.log_message(format!(
                            "Permission denied while opening {}",
                            file_path.display()
                        ));
                        Err(error)
                    }
                    _ => {
                        self.log_message(format!(
                            "Could not open {} ({})",
                            file_path.display(),
                            error
                        ));
                        Err(error)
                    }
                })?;
            (is_new_file, Rope::new())
        };
        // Generate a new buffer id
        let buffer_id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;

        // Store the new buffer
        self.buffers.insert(
            buffer_id,
            OpenBuffer {
                mode,
                repo: repo.map(RepositoryRc::new),
                content: text,
                file_path: Some(file_path),
            },
        );

        // Create a new window for the buffer
        if self.windows.is_empty() {
            self.windows.add(buffer_id);
        } else {
            self.windows.set_focused(buffer_id);
        }

        Ok(is_new_file)
    }

    fn log_message(&mut self, message: String) {
        self.prompt_action = PromptAction::Log { message };
        self.prompt_height = self.prompt_action.initial_height();
    }
}

impl Component for Editor {
    type Message = Message;
    type Properties = Rc<Context>;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        for file_path in properties.args_files.iter().cloned() {
            link.send(Message::OpenFile(file_path));
        }
        let log_message = link.callback(|message| Message::Log(Some(message)));

        Self {
            context: properties,
            link,
            themes: &THEMES,
            theme_index: 0,
            prompt_action: PromptAction::None,
            prompt_height: PROMPT_INACTIVE_HEIGHT,
            buffers: HashMap::new(),
            next_buffer_id: 0,
            windows: WindowTree::new(),
            log_message,
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        match message {
            Message::Cancel => {
                self.log_message("Cancel".into());
            }
            Message::ChangeTheme => {
                self.theme_index = (self.theme_index + 1) % self.themes.len();
                if !self.prompt_action.is_interactive() {
                    self.log_message(format!(
                        "Theme changed to {}",
                        self.themes[self.theme_index].1
                    ))
                }
            }
            Message::OpenFilePicker(source) if !self.prompt_action.is_interactive() => {
                self.prompt_action = PromptAction::OpenFile {
                    source,
                    on_open: self.link.callback(Message::OpenFile),
                    on_change_height: self.link.callback(Message::ChangePromptHeight),
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
            Message::OpenBufferSwitcher if !self.prompt_action.is_interactive() => {
                self.prompt_action = PromptAction::SwitchBuffer {
                    entries: self
                        .buffers
                        .iter()
                        .map(|(id, buffer)| {
                            BufferEntry::new(
                                *id,
                                buffer.file_path.clone(),
                                false,
                                buffer.content.len_bytes(),
                                buffer.mode,
                            )
                        })
                        .collect(),
                    on_select: self.link.callback(Message::SelectBuffer),
                    on_change_height: self.link.callback(Message::ChangePromptHeight),
                };
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::SelectBuffer(buffer_id) => {
                self.prompt_action = PromptAction::None;
                self.prompt_height = self.prompt_action.initial_height();
                if self.windows.is_empty() {
                    self.windows.add(buffer_id);
                } else {
                    self.windows.set_focused(buffer_id);
                }
            }
            Message::ChangePromptHeight(height) => {
                self.prompt_height = height;
            }
            Message::FocusNextComponent => self.windows.cycle_focus(CycleFocus::Next),
            Message::FocusPreviousComponent => self.windows.cycle_focus(CycleFocus::Previous),
            Message::SplitWindow(direction) if !self.buffers.is_empty() => {
                if let Some(buffer_id) = self.windows.get_focused() {
                    self.windows.insert_at_focused(buffer_id, direction);
                }
            }
            Message::FullscreenWindow if !self.buffers.is_empty() => {
                self.windows.close_all_except_focused();
            }
            Message::ClosePane if !self.buffers.is_empty() => {
                self.windows.close_focused();
            }
            Message::Log(message) if !self.prompt_action.is_interactive() => {
                self.prompt_action = message
                    .map(|message| PromptAction::Log { message })
                    .unwrap_or(PromptAction::None);
                self.prompt_height = self.prompt_action.initial_height();
            }
            Message::Quit => {
                self.link.exit();
            }
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
                let buffer = self.buffers.get(&id).unwrap();
                Buffer::with_key(
                    format!("{}.{}", index, id).as_str(),
                    BufferProperties {
                        context: self.context.clone(),
                        theme: Cow::Borrowed(&self.themes[self.theme_index].0.buffer),
                        focused: focused && !self.prompt_action.is_interactive(),
                        frame_id: index.one_based_index(),
                        mode: buffer.mode,
                        repo: buffer.repo.clone(),
                        content: buffer.content.clone(),
                        file_path: buffer.file_path.clone(),
                        log_message: self.log_message.clone(),
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
                    // on_change: link.callback(Message::PromptStateChange),
                    // on_open_file: link.callback(Message::OpenFile),
                    // message: self.prompt_message.clone(),
                },
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;

        let message = match pressed {
            [Key::Ctrl('g')] => Message::Cancel,

            // Open a file
            [Key::Ctrl('x'), Key::Ctrl('f')] => Message::OpenFilePicker(FileSource::Directory),
            [Key::Ctrl('x'), Key::Ctrl('v')] => Message::OpenFilePicker(FileSource::Repository),

            // Buffer management
            [Key::Ctrl('x'), Key::Char('b') | Key::Ctrl('b')] => Message::OpenBufferSwitcher,
            [Key::Ctrl('x'), Key::Char('o') | Key::Ctrl('o')] => Message::FocusNextComponent,
            [Key::Ctrl('x'), Key::Char('i') | Key::Ctrl('i') | Key::Char('O') | Key::Ctrl('O')] => {
                Message::FocusPreviousComponent
            }
            // Window management
            [Key::Ctrl('x'), Key::Char('1') | Key::Ctrl('1')] => Message::FullscreenWindow,
            [Key::Ctrl('x'), Key::Char('2') | Key::Ctrl('2')] => {
                Message::SplitWindow(FlexDirection::Column)
            }
            [Key::Ctrl('x'), Key::Char('3') | Key::Ctrl('3')] => {
                Message::SplitWindow(FlexDirection::Row)
            }
            [Key::Ctrl('x'), Key::Char('0') | Key::Ctrl('0')] => Message::ClosePane,

            // Theme
            [Key::Ctrl('t')] => Message::ChangeTheme,

            // Quit
            [Key::Ctrl('x'), Key::Ctrl('c')] => Message::Quit,
            _ => {
                if let PromptAction::Log { .. } = self.prompt_action {
                    self.link.send(Message::Log(None));
                };
                return BindingMatch {
                    transition: BindingTransition::Continue,
                    message: Some(Self::Message::KeyPressed),
                };
            }
        };
        BindingMatch {
            transition,
            message: Some(message),
        }
    }
}
