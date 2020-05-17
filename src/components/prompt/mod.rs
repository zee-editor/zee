mod picker;
mod status;

use ropey::Rope;
use std::{
    borrow::Cow,
    cmp,
    path::{Path, PathBuf},
    rc::Rc,
};
use zi::{
    component::{
        input::{Cursor, Input, InputChange, InputProperties, InputStyle},
        select::{Select, SelectProperties},
        text::{Text, TextProperties},
    },
    layout::{self, FlexDirection},
    terminal::{Background, Foreground, Key, Style},
    BindingMatch, BindingTransition, Callback, Component, ComponentLink, Layout, Rect,
    ShouldRender,
};

use self::{
    picker::FilePicker,
    status::{Status, StatusProperties},
};
use crate::{
    editor2::Context,
    error::Result,
    task::TaskId,
    utils::{self},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub action: Style,
    pub input: Style,
    pub cursor: Style,
    pub item_focused_background: Background,
    pub item_unfocused_background: Background,
    pub item_file_foreground: Foreground,
    pub item_directory_foreground: Foreground,
}

pub struct FileListingDone {
    task_id: TaskId,
    file_picker: FilePicker,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Inactive,
    PickingFileFromRepo,
    PickingFileFromDirectory,
}

impl State {
    pub fn is_active(&self) -> bool {
        *self != Self::Inactive
    }
}

pub enum Message {
    Clear,
    FileListingDone(Result<FileListingDone>),
    ListFilesInDirectory,
    ListFilesInRepository,
    Nop,
    OpenFile,

    // Path navigation
    AutocompletePath,
    ChangePath(InputChange),
    ChangeSelectedFile(usize),
    SelectParentDirectory,
}

#[derive(Clone)]
pub struct Properties {
    pub context: Rc<Context>,
    pub theme: Cow<'static, Theme>,
    pub on_change: Callback<(State, usize)>,
    pub on_open_file: Callback<PathBuf>,
    pub message: String,
}

pub struct Prompt {
    properties: Properties,
    frame: Rect,
    link: ComponentLink<Self>,
    input: Rope,
    cursor: Cursor,
    state: State,
    // File picker:
    file_picker: Rc<FilePicker>,
    file_index: usize,
    current_task_id: Option<TaskId>,
}

impl Prompt {
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    pub fn height(&self) -> usize {
        if self.is_active() {
            PROMPT_INACTIVE_HEIGHT + cmp::min(self.file_picker.num_filtered(), PROMPT_MAX_HEIGHT)
        } else {
            PROMPT_INACTIVE_HEIGHT
        }
    }

    fn pick_from_directory(&mut self) {
        let link = self.link.clone();
        let input = self.input.clone();
        let mut file_picker = (*self.file_picker).clone();
        self.current_task_id = Some(self.properties.context.task_pool.spawn(move |task_id| {
            let path_str = input.to_string();
            link.send(Message::FileListingDone(
                picker::pick_from_directory(&mut file_picker, path_str)
                    .map(|_| FileListingDone {
                        task_id,
                        file_picker,
                    })
                    .map_err(|error| error.into()),
            ))
        }))
    }

    fn pick_from_repository(&mut self) {
        let link = self.link.clone();
        let input = self.input.clone();
        let mut file_picker = (*self.file_picker).clone();
        self.current_task_id = Some(self.properties.context.task_pool.spawn(move |task_id| {
            let path_str = input.to_string();
            link.send(Message::FileListingDone(
                picker::pick_from_repository(&mut file_picker, path_str)
                    .map(|_| FileListingDone {
                        task_id,
                        file_picker,
                    })
                    .map_err(|error| error.into()),
            ))
        }));
    }

    #[inline]
    fn set_input_to_cwd(&mut self) {
        // let mut current_working_dir = self
        //     .properties
        //     .context
        //     .current_working_dir
        //     .parent()
        //     .unwrap_or(&self.properties.context.current_working_dir)
        //     .to_str()
        //     .unwrap_or("")
        //     .chars()
        //     .collect::<String>();
        // current_working_dir.push('/');
        // current_working_dir.push('\n');
        // self.input = current_working_dir.into();
        self.cursor.delete_line(&mut self.input);
        self.cursor.insert_chars(
            &mut self.input,
            self.properties
                .context
                .current_working_dir
                .parent()
                .unwrap_or(&self.properties.context.current_working_dir)
                .to_str()
                .unwrap_or("")
                .chars(),
        );
        self.cursor.move_to_end_of_line(&self.input);
        self.cursor.insert_char(&mut self.input, '/');
        self.cursor.move_right(&self.input);
    }

    #[inline]
    fn clear_state(&mut self) {
        self.state = State::Inactive;
        self.cursor = Cursor::new();
        self.input.remove(..);
        self.current_task_id = None;
        self.file_index = 0;
        Rc::make_mut(&mut self.file_picker).clear();
    }

    #[inline]
    fn emit_change(&self) {
        self.properties.on_change.emit((self.state, self.height()));
    }
}

impl Component for Prompt {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Self::Properties, frame: Rect, link: ComponentLink<Self>) -> Self {
        Self {
            properties,
            frame,
            link,
            input: Rope::new(),
            cursor: Cursor::new(),
            state: State::Inactive,
            file_picker: Rc::new(FilePicker::new()),
            current_task_id: None,
            file_index: 0,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let should_render = (self.properties.message != properties.message
            || self.properties.theme != properties.theme)
            .into();
        self.properties = properties;
        should_render
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn update(&mut self, message: Message) -> ShouldRender {
        let input_changed = match message {
            Message::Clear => {
                self.clear_state();
                self.emit_change();
                false
            }
            Message::ListFilesInDirectory if !self.is_active() => {
                self.state = State::PickingFileFromDirectory;
                self.emit_change();
                self.set_input_to_cwd();
                self.pick_from_directory();
                false
            }
            Message::ListFilesInRepository if !self.is_active() => {
                self.state = State::PickingFileFromRepo;
                self.emit_change();
                self.set_input_to_cwd();
                self.pick_from_repository();
                false
            }
            Message::OpenFile if self.is_active() => {
                let path_str: Cow<str> = self.input.slice(..).into();
                self.properties
                    .on_open_file
                    .emit(PathBuf::from(path_str.trim()));
                self.clear_state();
                self.emit_change();
                false
            }
            Message::SelectParentDirectory if self.is_active() => {
                let path_str: String = self.input.slice(..).into();
                self.input = Path::new(&path_str.trim())
                    .parent()
                    .map(|parent| parent.to_string_lossy())
                    .unwrap_or_else(|| "".into())
                    .into();
                utils::ensure_trailing_newline_with_content(&mut self.input);
                self.cursor.move_to_end_of_line(&self.input);
                self.cursor.insert_char(&mut self.input, '/');
                self.cursor.move_right(&self.input);
                true
            }
            Message::AutocompletePath if self.is_active() => {
                if let Some(path) = self.file_picker.selected(self.file_index) {
                    self.input = path.to_string_lossy().into();
                    utils::ensure_trailing_newline_with_content(&mut self.input);
                    self.cursor.move_to_end_of_line(&self.input);
                    if path.is_dir() {
                        self.cursor.insert_char(&mut self.input, '/');
                        self.cursor.move_right(&self.input);
                    }
                    self.file_index = 0;
                    true
                } else {
                    false
                }
            }
            Message::ChangePath(InputChange { content, cursor }) if self.is_active() => {
                self.cursor = cursor;
                if let Some(content) = content {
                    self.input = content;
                    true
                } else {
                    false
                }
            }
            Message::ChangeSelectedFile(index) if self.is_active() => {
                self.file_index = index;
                false
            }
            Message::FileListingDone(Ok(FileListingDone {
                task_id,
                file_picker,
            })) if self
                .current_task_id
                .as_ref()
                .map(|&expected_task_id| expected_task_id == task_id)
                .unwrap_or(false) =>
            {
                self.file_picker = Rc::new(file_picker);
                self.current_task_id = None;
                self.emit_change();
                false
            }
            _ => {
                return ShouldRender::No;
            }
        };

        if input_changed {
            match self.state {
                State::PickingFileFromDirectory => self.pick_from_directory(),
                State::PickingFileFromRepo => self.pick_from_repository(),
                State::Inactive => {}
            }
        }

        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        if !self.is_active() && !self.properties.message.is_empty() {
            return layout::component::<Text>(
                TextProperties::new()
                    .content(self.properties.message.clone())
                    .style(self.properties.theme.input),
            );
        }

        let input = layout::component::<Input>(InputProperties {
            style: InputStyle {
                content: self.properties.theme.input,
                cursor: self.properties.theme.cursor,
            },
            content: self.input.clone(),
            cursor: self.cursor.clone(),
            on_change: Some(self.link.callback(Message::ChangePath)),
            focused: self.is_active(),
        });

        if !self.is_active() {
            return input;
        }

        let file_picker = self.file_picker.clone();
        let file_index = self.file_index;
        let theme = self.properties.theme.clone();
        let item_at = move |index| {
            let path = file_picker.selected(index).unwrap();
            let background = if index == file_index {
                theme.item_focused_background
            } else {
                theme.item_unfocused_background
            };
            let style = if path.is_dir() {
                Style::bold(background, theme.item_directory_foreground)
            } else {
                Style::normal(background, theme.item_file_foreground)
            };
            layout::fixed(
                1,
                layout::component::<Text>(
                    TextProperties::new()
                        .content(
                            &path.to_string_lossy()[file_picker
                                .prefix()
                                .to_str()
                                .map(|prefix| prefix.len() + 1)
                                .unwrap_or(0)..],
                        )
                        .style(style),
                ),
            )
        };
        layout::column([
            layout::auto(layout::component::<Select>(SelectProperties {
                background: Style::normal(
                    self.properties.theme.item_unfocused_background,
                    self.properties.theme.item_file_foreground,
                ),
                direction: FlexDirection::ColumnReverse,
                item_at: item_at.into(),
                focused: true,
                num_items: self.file_picker.num_filtered(),
                selected: self.file_index,
                on_change: self.link.callback(Message::ChangeSelectedFile).into(),
                item_size: 1,
            })),
            layout::fixed(
                1,
                if self.is_active() {
                    layout::row([
                        layout::fixed(
                            4,
                            layout::component::<Status>(StatusProperties {
                                status: self.state,
                                pending: self.current_task_id.is_some(),
                                style: self.properties.theme.action,
                            }),
                        ),
                        layout::fixed(
                            1,
                            layout::component::<Text>(
                                TextProperties::new().style(self.properties.theme.input),
                            ),
                        ),
                        layout::auto(input),
                    ])
                } else {
                    input
                },
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let mut transition = BindingTransition::Clear;
        let message = match pressed {
            // Path navigation
            &[Key::Ctrl('g')] => Message::Clear,
            &[Key::Ctrl('x'), Key::Ctrl('f')] => Message::ListFilesInDirectory,
            &[Key::Ctrl('x'), Key::Ctrl('v')] => Message::ListFilesInRepository,
            &[Key::Ctrl('x')] => {
                transition = BindingTransition::Continue;
                Message::Nop
            }
            &[Key::Char('\n')] => Message::OpenFile,

            // Path navigation
            &[Key::Ctrl('l')] => Message::SelectParentDirectory,
            &[Key::Char('\t')] => Message::AutocompletePath,

            _ => Message::Nop,
        };
        BindingMatch {
            transition,
            message: Some(message),
        }
    }
}

pub const PROMPT_INACTIVE_HEIGHT: usize = 1;
const PROMPT_MAX_HEIGHT: usize = 15;
