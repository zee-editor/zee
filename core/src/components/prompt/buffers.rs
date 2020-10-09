use ropey::Rope;
use size_format::SizeFormatterBinary;
use std::{borrow::Cow, convert::TryInto, path::PathBuf, rc::Rc};
use zi::{
    components::{
        input::{Cursor, Input, InputChange, InputProperties, InputStyle},
        select::{Select, SelectProperties},
        text::{Text, TextAlign, TextProperties},
    },
    layout, BindingMatch, BindingTransition, Callback, Colour, Component, ComponentExt,
    ComponentLink, FlexBasis, FlexDirection, Key, Layout, Rect, ShouldRender, Style,
};

use super::{
    matcher::Matcher,
    status::{Status, StatusProperties},
    Theme,
};
use crate::{
    editor::{BufferId, Context},
    mode::Mode,
    task::TaskId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BufferEntry {
    pub id: BufferId,
    pub path: Option<PathBuf>,
    pub on_screen: bool,
    pub len_bytes: usize,
    pub mode: &'static Mode,
    pub name: String,
}

impl BufferEntry {
    pub fn new(
        id: BufferId,
        path: Option<PathBuf>,
        on_screen: bool,
        len_bytes: usize,
        mode: &'static Mode,
    ) -> Self {
        let name = path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|path| path.to_string_lossy())
            .unwrap_or_else(|| "(Unnamed)".into())
            .to_string();
        Self {
            id,
            path,
            on_screen,
            len_bytes,
            mode,
            name,
        }
    }
}

#[derive(Debug)]
pub enum Message {
    Select,
    UpdateInput(InputChange),
    UpdateSelected(usize),
}

#[derive(Clone)]
pub struct Properties {
    pub context: Rc<Context>,
    pub theme: Cow<'static, Theme>,
    pub entries: Vec<BufferEntry>,
    pub on_select: Callback<BufferId>,
    pub on_filter: Callback<usize>,
}

pub struct BufferPicker {
    properties: Properties,
    link: ComponentLink<Self>,
    input: Rope,
    cursor: Cursor,
    selected_index: usize,
    current_task_id: Option<TaskId>,
    matcher: Matcher,
}

impl Component for BufferPicker {
    type Message = Message;
    type Properties = Properties;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        let mut matcher = Matcher::new();
        matcher.set_filter(
            properties.entries.iter().map(|entry| entry.name.as_str()),
            "",
        );
        Self {
            properties,
            link,
            input: "\n".into(),
            cursor: Cursor::new(),
            selected_index: 0,
            current_task_id: None,
            matcher,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        let filter_str: Cow<str> = self.input.slice(..).into();
        self.matcher.set_filter(
            properties.entries.iter().map(|entry| entry.name.as_str()),
            &filter_str,
        );
        self.properties = properties;
        ShouldRender::Yes
    }

    fn update(&mut self, message: Message) -> ShouldRender {
        let input_changed = match message {
            Message::Select if self.matcher.num_ranked() > 0 => {
                self.properties
                    .on_select
                    .emit(self.properties.entries[self.matcher[self.selected_index]].id);
                false
            }
            Message::UpdateInput(InputChange { content, cursor }) => {
                self.selected_index = 0;
                self.cursor = cursor;
                if let Some(content) = content {
                    self.input = content;
                    true
                } else {
                    false
                }
            }
            Message::UpdateSelected(index) => {
                self.selected_index = index;
                false
            }
            _ => false,
        };

        if input_changed {
            let filter_str: Cow<str> = self.input.slice(..).into();
            self.matcher.set_filter(
                self.properties
                    .entries
                    .iter()
                    .map(|entry| entry.name.as_str()),
                &filter_str,
            );
            self.properties.on_filter.emit(self.matcher.num_ranked());
        }

        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let input = Input::with(InputProperties {
            style: InputStyle {
                content: self.properties.theme.input,
                cursor: self.properties.theme.cursor,
            },
            content: self.input.clone(),
            cursor: self.cursor.clone(),
            on_change: Some(self.link.callback(Message::UpdateInput)),
            focused: true,
        });

        let entries = self.properties.entries.clone();
        let matcher = self.matcher.clone();
        let selected_index = self.selected_index;
        let theme = self.properties.theme.clone();
        let item_at = move |index| {
            let entry = &entries[matcher[index]];
            let background = if index == selected_index {
                theme.item_focused_background
            } else {
                theme.item_unfocused_background
            };
            layout::fixed(
                1,
                layout::row([
                    Text::item_with_key(
                        FlexBasis::Fixed(20),
                        format!("{}name", entry.id).as_str(),
                        TextProperties::new()
                            .content(entry.name.clone())
                            .style(Style::normal(background, theme.item_file_foreground)),
                    ),
                    Text::item_with_key(
                        FlexBasis::Fixed(16),
                        format!("{}size", entry.id).as_str(),
                        TextProperties::new()
                            .content(format!(
                                " {} ",
                                SizeFormatterBinary::new(entry.len_bytes.try_into().unwrap())
                            ))
                            .style(Style::normal(background, theme.file_size))
                            .align(TextAlign::Right),
                    ),
                    Text::item_with_key(
                        FlexBasis::Fixed(16),
                        format!("{}mode", entry.id).as_str(),
                        TextProperties::new()
                            .content(entry.mode.name.clone())
                            .style(Style::normal(background, theme.mode))
                            .align(TextAlign::Right),
                    ),
                    Text::item_with_key(
                        FlexBasis::Auto,
                        format!("{}path", entry.id).as_str(),
                        TextProperties::new()
                            .content(
                                entry
                                    .path
                                    .as_ref()
                                    .map(|entry| format!("    {}", entry.display()))
                                    .unwrap_or_else(String::new),
                            )
                            .style(Style::normal(background, theme.file_size)),
                    ),
                ]),
            )
        };
        layout::column([
            if self.matcher.num_ranked() == 0 {
                layout::fixed(
                    1,
                    Text::with(
                        TextProperties::new()
                            .content(if self.properties.entries.is_empty() {
                                "No open buffers"
                            } else {
                                "No matching buffers"
                            })
                            .style(Style::normal(
                                self.properties.theme.item_unfocused_background,
                                Colour::rgb(251, 73, 52),
                                // self.properties.theme.action.background,
                            )),
                    ),
                )
            } else {
                Select::item_with(
                    FlexBasis::Auto,
                    SelectProperties {
                        background: Style::normal(
                            self.properties.theme.item_unfocused_background,
                            self.properties.theme.item_file_foreground,
                        ),
                        direction: FlexDirection::ColumnReverse,
                        item_at: item_at.into(),
                        focused: true,
                        num_items: self.matcher.num_ranked(),
                        selected: self.selected_index,
                        on_change: self.link.callback(Message::UpdateSelected).into(),
                        item_size: 1,
                    },
                )
            },
            layout::fixed(
                1,
                layout::row([
                    Status::item_with_key(
                        FlexBasis::Fixed(6),
                        "status",
                        StatusProperties {
                            action_name: "buffer".into(),
                            pending: self.current_task_id.is_some(),
                            style: self.properties.theme.action,
                        },
                    ),
                    Text::item_with_key(
                        FlexBasis::Fixed(1),
                        "spacer",
                        TextProperties::new().style(self.properties.theme.input),
                    ),
                    layout::auto(input),
                    Text::item_with_key(
                        FlexBasis::Fixed(12),
                        "num-results",
                        TextProperties::new()
                            .content(format!(
                                "{} of {} ",
                                self.matcher.num_ranked(),
                                self.properties.entries.len()
                            ))
                            .style(self.properties.theme.action.invert())
                            .align(TextAlign::Right),
                    ),
                ]),
            ),
        ])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let transition = BindingTransition::Clear;
        let message = match pressed {
            [Key::Char('\n')] => Message::Select,
            // [Key::Ctrl('l')] => Message::SelectParentDirectory,
            // [Key::Char('\t')] => Message::AutocompletePath,
            [Key::Ctrl('x')] => {
                return BindingMatch {
                    transition: BindingTransition::Continue,
                    message: None,
                };
            }
            _ => {
                return BindingMatch {
                    transition: BindingTransition::Clear,
                    message: None,
                };
            }
        };
        BindingMatch {
            transition,
            message: Some(message),
        }
    }
}
