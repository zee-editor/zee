use git2::Repository;
use ropey::Rope;
use std::{
    borrow::Cow,
    cmp,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};
use zi::{
    layout, BindingMatch, BindingTransition, Component, ComponentLink, Key, Layout, Rect,
    ShouldRender,
};

use crate::{
    clipboard::Clipboard,
    components::{
        buffer::{Buffer, Properties as BufferProperties, RepositoryRc},
        prompt::{
            Prompt, Properties as PromptProperties, State as PromptState, PROMPT_INACTIVE_HEIGHT,
        },
        splash::{Properties as SplashProperties, Splash},
        theme::{Theme, THEMES},
    },
    error::Result,
    mode,
    settings::Settings,
    task::TaskPool,
};

#[derive(Clone, Debug)]
pub enum Message {
    ChangeTheme,
    ClosePane,
    FocusNextComponent,
    FocusPreviousComponent,
    KeyPressed,
    OpenFile(PathBuf),
    PromptStateChange((PromptState, usize)),
    LogMessage(String),
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
    prompt_state: PromptState,
    prompt_height: usize,
    buffers: Vec<OpenBuffer>,
    next_buffer_id: usize,
    focused: usize,
    prompt_message: String,
}

pub struct OpenBuffer {
    id: usize,
    properties: BufferProperties,
}

enum CycleFocus {
    Next,
    Previous,
}

impl Editor {
    fn cycle_focus(&mut self, direction: CycleFocus) {
        match (self.buffers.len(), direction) {
            (0, _) => {
                self.focused = 0;
            }
            (num_buffers, CycleFocus::Next) => {
                self.focused = (self.focused + 1) % num_buffers;
            }
            (num_buffers, CycleFocus::Previous) => {
                self.focused = (num_buffers + self.focused).saturating_sub(1) % num_buffers;
            }
        }
    }

    fn open_file(&mut self, file_path: PathBuf) -> Result<()> {
        let mode = mode::find_by_filename(&file_path);
        let repo = Repository::discover(&file_path).ok();
        let text = if file_path.exists() {
            Rope::from_reader(BufReader::new(File::open(&file_path)?))?
        } else {
            // Optimistically check if we can create it
            File::open(&file_path)
                .map(|_| ())
                .or_else(|error| match error.kind() {
                    io::ErrorKind::NotFound => {
                        self.prompt_message = "[New file]".into();
                        Ok(())
                    }
                    io::ErrorKind::PermissionDenied => {
                        self.prompt_message =
                            format!("Permission denied while opening {}", file_path.display());
                        Err(error)
                    }
                    _ => {
                        self.prompt_message =
                            format!("Could not open {} ({})", file_path.display(), error);
                        Err(error)
                    }
                })?;
            Rope::new()
        };
        self.buffers.push(OpenBuffer {
            id: self.next_buffer_id,
            properties: BufferProperties {
                context: self.context.clone(),
                theme: Cow::Borrowed(&self.themes[self.theme_index].0.buffer),
                focused: false,
                frame_id: self.buffers.len() + 1,
                mode,
                repo: repo.map(RepositoryRc::new),
                content: text,
                file_path: Some(file_path),
                log_message: self.link.callback(Message::LogMessage),
            },
        });
        self.next_buffer_id += 1;
        Ok(())
    }
}

impl Component for Editor {
    type Message = Message;
    type Properties = Rc<Context>;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        for file_path in properties.args_files.iter().cloned() {
            link.send(Message::OpenFile(file_path));
        }

        Self {
            context: properties,
            link,
            themes: &THEMES,
            theme_index: 0,
            prompt_state: PromptState::Inactive,
            prompt_height: PROMPT_INACTIVE_HEIGHT,
            buffers: Vec::new(),
            next_buffer_id: 0,
            focused: 0,
            prompt_message: String::new(),
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        let cleared_prompt = if !self.prompt_message.is_empty() {
            self.prompt_message.clear();
            true
        } else {
            false
        };
        match message {
            Message::ChangeTheme => {
                self.theme_index = (self.theme_index + 1) % self.themes.len();
                self.prompt_message =
                    format!("Theme changed to {}", self.themes[self.theme_index].1);
            }
            Message::PromptStateChange((state, height)) => {
                self.prompt_state = state;
                self.prompt_height = height;
            }
            Message::OpenFile(path) => {
                if let Err(error) = self.open_file(path) {
                    self.prompt_message = format!("Could not open file: {}", error.to_string());
                }
            }
            Message::FocusNextComponent => self.cycle_focus(CycleFocus::Next),
            Message::FocusPreviousComponent => self.cycle_focus(CycleFocus::Previous),
            Message::ClosePane if !self.buffers.is_empty() => {
                self.buffers.remove(self.focused);
                self.focused = cmp::min(self.buffers.len().saturating_sub(1), self.focused);
            }
            Message::LogMessage(message) => {
                self.prompt_message = message;
            }
            Message::Quit => {
                self.link.exit();
            }
            _ => return cleared_prompt.into(),
        }
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let link = self.link.clone();
        let buffers = if self.buffers.is_empty() {
            layout::auto(layout::component::<Splash>(SplashProperties {
                theme: Cow::Borrowed(&self.themes[self.theme_index].0.splash),
            }))
        } else {
            layout::auto(layout::row_iter(self.buffers.iter().enumerate().map(
                |(index, OpenBuffer { id, properties })| {
                    let mut properties = properties.clone();
                    properties.focused = index == self.focused && !self.prompt_state.is_active();
                    properties.frame_id = index + 1;
                    properties.theme = Cow::Borrowed(&self.themes[self.theme_index].0.buffer);
                    layout::auto(layout::component_with_key::<Buffer>(*id, properties))
                },
            )))
        };

        layout::column([
            buffers,
            layout::fixed(
                self.prompt_height,
                layout::component::<Prompt>(PromptProperties {
                    context: self.context.clone(),
                    theme: Cow::Borrowed(&self.themes[self.theme_index].0.prompt),
                    on_change: link.callback(Message::PromptStateChange),
                    on_open_file: link.callback(Message::OpenFile),
                    message: self.prompt_message.clone(),
                }),
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;
        let message = match pressed {
            [Key::Ctrl('x'), Key::Char('o')] | [Key::Ctrl('x'), Key::Ctrl('o')] => {
                Message::FocusNextComponent
            }
            [Key::Ctrl('x'), Key::Char('O')] | [Key::Ctrl('x'), Key::Ctrl('O')] => {
                Message::FocusPreviousComponent
            }
            [Key::Ctrl('x'), Key::Char('0')] => Message::ClosePane,
            [Key::Ctrl('t')] => Message::ChangeTheme,
            [Key::Ctrl('x'), Key::Ctrl('c')] => Message::Quit,
            _ => {
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
