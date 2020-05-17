use ropey::Rope;
use std::{cmp, rc::Rc};
use unicode_width::UnicodeWidthStr;

use zi::{
    component::{
        input::{Cursor, Input, InputChange, InputProperties, InputStyle},
        select::{Select, SelectProperties},
        text::{Text, TextAlign, TextProperties},
        Callback,
    },
    frontend,
    layout::{self, FlexDirection},
    App, BindingMatch, BindingTransition, Canvas, Colour, Component, ComponentLink, Key, Layout,
    Rect, Result, ShouldRender, Style,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckboxProperties {
    pub style: Style,
    pub checked: bool,
}

#[derive(Debug)]
pub struct Checkbox {
    properties: <Self as Component>::Properties,
    frame: Rect,
}

impl Component for Checkbox {
    type Message = ();
    type Properties = CheckboxProperties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.properties = properties;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            frame,
            properties: Self::Properties { style, checked },
        } = *self;

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(style);
        match checked {
            true => canvas.draw_str(0, 0, style, CHECKED),
            false => canvas.draw_str(0, 0, style, UNCHECKED),
        };

        canvas.into()
    }
}

// The missing space from `CHECKED` is because of a terminal rendering bug (?)
// when using unicode combining characters for strikethrough styling.
const UNCHECKED: &str = " [ ] ";
const CHECKED: &str = " [x] ";

#[derive(Clone, PartialEq)]
struct TodoProperties {
    content: String,
    checked: bool,
    content_style: Style,
    cursor_style: Style,
    editing: bool,
    on_change: Callback<Rope>,
}

enum TodoMessage {
    SetCursor(Cursor),
}

struct Todo {
    properties: TodoProperties,
    link: ComponentLink<Self>,
    cursor: Cursor,
    handle_input_change: Callback<InputChange>,
}

impl Component for Todo {
    type Message = TodoMessage;
    type Properties = TodoProperties;

    fn create(properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        let handle_input_change = {
            let link = link.clone();
            let on_change = properties.on_change.clone();
            (move |InputChange { cursor, content }| {
                link.send(TodoMessage::SetCursor(cursor));
                if let Some(content) = content {
                    on_change.emit(content)
                }
            })
            .into()
        };

        Self {
            properties,
            link,
            cursor: Cursor::new(),
            handle_input_change,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.properties = properties;
            self.handle_input_change = {
                let link = self.link.clone();
                let on_change = self.properties.on_change.clone();
                (move |InputChange { cursor, content }| {
                    link.send(TodoMessage::SetCursor(cursor));
                    if let Some(content) = content {
                        on_change.emit(content)
                    }
                })
                .into()
            };
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        match message {
            Self::Message::SetCursor(cursor) => self.cursor = cursor,
        }
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self::Properties {
            ref content,
            checked,
            content_style,
            cursor_style,
            editing,
            ..
        } = self.properties;
        let todo_component = if editing {
            let cursor = self.cursor.clone();
            layout::component::<Input>(InputProperties {
                style: InputStyle {
                    content: content_style,
                    cursor: cursor_style,
                },
                content: Rope::from_str(&content),
                cursor,
                on_change: self.handle_input_change.clone().into(),
                focused: true,
            })
        } else {
            let content = if checked {
                unicode_strikethrough(content)
            } else {
                content.clone()
            };
            layout::component::<Text>(TextProperties::new().content(content).style(content_style))
        };

        let checkbox_width = UnicodeWidthStr::width(if checked { CHECKED } else { UNCHECKED });
        layout::row([
            layout::fixed(
                checkbox_width,
                layout::component::<Checkbox>(CheckboxProperties {
                    style: content_style,
                    checked,
                }),
            ),
            layout::auto(todo_component),
        ])
    }
}

fn unicode_strikethrough(content: &str) -> String {
    let content = content.trim_end();
    if content.is_empty() {
        return "\n".into();
    }

    let mut styled_content = String::new();
    for character in content.chars() {
        styled_content.push('\u{0336}');
        styled_content.push(character);
    }
    styled_content.push('\n');
    styled_content
}

#[derive(Clone, Debug)]
struct Theme {
    checked: Style,
    unchecked: Style,
    focused: Style,
    cursor: Style,
}

impl Default for Theme {
    fn default() -> Self {
        const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
        const LIGHT2: Colour = Colour::rgb(213, 196, 161);
        const GRAY_245: Colour = Colour::rgb(146, 131, 116);
        const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);

        Self {
            unchecked: Style::normal(DARK0_SOFT.into(), LIGHT2.into()),
            checked: Style::normal(DARK0_SOFT.into(), GRAY_245.into()),
            focused: Style::normal(BRIGHT_BLUE.into(), DARK0_SOFT.into()),
            cursor: Style::normal(BRIGHT_BLUE.into(), DARK0_SOFT.into()),
        }
    }
}

#[derive(Clone, Debug)]
struct TodoItem {
    id: usize,
    checked: bool,
    content: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Message {
    AddItem,
    ChangeContent((usize, Rope)),
    DeleteDone,
    DeleteItem,
    Edit,
    FocusItem(usize),
    MoveItemDown,
    MoveItemUp,
    ToggleDone,
}

struct TodoMvc {
    link: ComponentLink<Self>,
    theme: Theme,
    todos: Rc<Vec<TodoItem>>,
    next_id: usize,
    focus_index: usize,
    editing: bool,
}

impl TodoMvc {
    fn insert_todo(&mut self, index: usize, checked: bool, content: String) {
        Rc::make_mut(&mut self.todos).insert(
            index,
            TodoItem {
                id: self.next_id,
                checked,
                content,
            },
        );
        self.next_id += 1;
    }
}

impl Component for TodoMvc {
    type Message = Message;
    type Properties = ();

    fn create(_properties: (), _frame: Rect, link: ComponentLink<Self>) -> Self {
        Self {
            link,
            theme: Default::default(),
            todos: (0..1)
                .into_iter()
                .map(|index| TodoItem {
                    id: index,
                    checked: false,
                    content: format!(
                        "    豈 更 車 賈 滑 串 句 All work and no play makes Jack a dull boy {}\n",
                        index
                    ),
                })
                .collect::<Vec<_>>()
                .into(),
            next_id: 100,
            focus_index: 0,
            editing: true,
        }
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        eprintln!("Message: {:?}", message);

        match message {
            Message::Edit => {
                self.editing = false;
            }
            Message::AddItem if !self.editing => {
                self.editing = true;
                if self.todos.is_empty() {
                    self.insert_todo(0, false, "\n".into());
                }
            }
            Message::AddItem if self.editing => {
                self.insert_todo(
                    cmp::min(self.focus_index + 1, self.todos.len()),
                    false,
                    "\n".into(),
                );
                self.focus_index =
                    cmp::min(self.focus_index + 1, self.todos.len().saturating_sub(1));
            }
            Message::FocusItem(index) => {
                self.editing = false;
                self.focus_index = index;
            }
            Message::MoveItemUp => {
                self.editing = false;
                let current_index = self.focus_index;
                let new_index = self.focus_index.saturating_sub(1);
                if current_index != new_index {
                    Rc::make_mut(&mut self.todos).swap(current_index, new_index);
                    self.focus_index = new_index;
                }
            }
            Message::MoveItemDown => {
                self.editing = false;
                let current_index = self.focus_index;
                let new_index = cmp::min(self.focus_index + 1, self.todos.len().saturating_sub(1));
                if current_index != new_index {
                    Rc::make_mut(&mut self.todos).swap(current_index, new_index);
                    self.focus_index = new_index;
                }
            }
            Message::DeleteItem if !self.todos.is_empty() => {
                Rc::make_mut(&mut self.todos).remove(self.focus_index);
                self.focus_index = cmp::min(self.focus_index, self.todos.len().saturating_sub(1));
            }
            Message::DeleteDone if !self.todos.is_empty() => {
                self.focus_index = 0;
                Rc::make_mut(&mut self.todos).retain(|item| !item.checked);
            }
            Message::ToggleDone if !self.todos.is_empty() => {
                let checked = &mut Rc::make_mut(&mut self.todos)[self.focus_index].checked;
                *checked = !*checked;
                self.focus_index =
                    cmp::min(self.focus_index + 1, self.todos.len().saturating_sub(1));
            }
            Message::ChangeContent(content) => {
                Rc::make_mut(&mut self.todos)[content.0].content = content.1.into();
            }
            _ => {
                return ShouldRender::No;
            }
        }
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            ref link,
            ref theme,
            ref todos,
            editing,
            focus_index,
            ..
        } = *self;
        let num_left = todos.iter().filter(|item| !item.checked).count();

        // Title component
        let title = layout::fixed(
            LOGO.lines().count() + 1,
            layout::component_with_key_str::<Text>(
                "title",
                TextProperties::new()
                    .content(LOGO)
                    .style(theme.checked)
                    .align(TextAlign::Centre),
            ),
        );

        // The list of todo items
        let todo_items = layout::auto(layout::component_with_key_str::<Select>(
            "select",
            SelectProperties {
                background: theme.unchecked,
                direction: FlexDirection::Column,
                num_items: todos.len(),
                selected: focus_index,
                item_at: {
                    let todos = todos.clone();
                    let link = self.link.clone();
                    let theme = theme.clone();
                    (move |index| {
                        let item: &TodoItem = &todos[index];
                        let link = link.clone();
                        layout::fixed(
                            1,
                            layout::component_with_key::<Todo>(
                                item.id,
                                TodoProperties {
                                    content_style: if focus_index == index && !editing {
                                        theme.focused
                                    } else if item.checked {
                                        theme.checked
                                    } else {
                                        theme.unchecked
                                    },
                                    cursor_style: theme.cursor,
                                    checked: item.checked,
                                    content: item.content.clone(),
                                    editing: index == focus_index && editing,
                                    on_change: (move |content| {
                                        link.send(Message::ChangeContent((index, content)))
                                    })
                                    .into(),
                                },
                            ),
                        )
                    })
                    .into()
                },
                item_size: 1,
                focused: true,
                on_change: Some(link.callback(|index| Message::FocusItem(index))),
            },
        ));

        // Status bar at the bottom counting how many items have been ticked off
        let status_bar = layout::fixed(
            1,
            layout::component_with_key_str::<Text>(
                "status-bar",
                TextProperties::new()
                    .content(format!(
                        "Item {} of {} ({} remaining, {} done)",
                        focus_index + 1,
                        todos.len(),
                        num_left,
                        todos.len() - num_left
                    ))
                    .style(theme.checked),
            ),
        );

        layout::column([title, todo_items, status_bar])
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let mut transition = BindingTransition::Clear;
        let message = match pressed {
            &[Key::Esc] | &[Key::Alt('\u{1b}')] => Some(Message::Edit),
            &[Key::Char('\n')] => Some(Message::AddItem),
            &[Key::Alt('p')] => Some(Message::MoveItemUp),
            &[Key::Alt('n')] => Some(Message::MoveItemDown),
            &[Key::Ctrl('k')] => Some(Message::DeleteItem),
            &[Key::Ctrl('x'), Key::Ctrl('k')] => Some(Message::DeleteDone),
            &[Key::Char('\t')] => Some(Message::ToggleDone),
            &[Key::Ctrl('x'), Key::Ctrl('c')] => {
                self.link.exit();
                None
            }
            &[Key::Ctrl('x')] => {
                transition = BindingTransition::Continue;
                None
            }
            _ => {
                transition = BindingTransition::Clear;
                None
            }
        };
        BindingMatch {
            transition,
            message,
        }
    }
}

const LOGO: &str = r#"
              ████████╗ ██████╗ ██████╗  ██████╗ ███████╗
              ╚══██╔══╝██╔═══██╗██╔══██╗██╔═══██╗██╔════╝
                 ██║   ██║   ██║██║  ██║██║   ██║███████╗
                 ██║   ██║   ██║██║  ██║██║   ██║╚════██║
                 ██║   ╚██████╔╝██████╔╝╚██████╔╝███████║
                 ╚═╝    ╚═════╝ ╚═════╝  ╚═════╝ ╚══════╝

    RET: new item         TAB: toggle done          C-k: delete item
    C-p, Up: cursor up    C-n, Down: cursor down    C-x C-k: delete done
    A-p: move item up     A-n: move item down       C-x C-c: exit

"#;

fn main() -> Result<()> {
    let mut app = App::new(layout::component::<TodoMvc>(Default::default()));
    app.run_event_loop(frontend::default()?)
}
