use ropey::Rope;
use std::{cmp, iter};
use unicode_width::UnicodeWidthStr;

use zi::{
    component::{
        input::{Input, InputProperties},
        text::{Text, TextAlign, TextProperties},
    },
    frontend::Termion,
    layout, App, BindingMatch, BindingTransition, Canvas, Colour, Component, ComponentLink, Key,
    Layout, Rect, Result, Scheduler, ShouldRender, Style,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckboxProperties {
    pub style: Style,
    pub checked: bool,
}

#[derive(Debug)]
pub struct Checkbox {
    properties: <Self as Component>::Properties,
}

impl Checkbox {
    fn new(properties: <Self as Component>::Properties) -> Self {
        Self { properties }
    }
}

impl Component for Checkbox {
    type Message = ();
    type Properties = CheckboxProperties;

    fn create(
        properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Self::new(properties)
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn view(&self, frame: Rect) -> Layout {
        let Self::Properties { style, checked } = self.properties;

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
const UNCHECKED: &str = "  [ ] ";
const CHECKED: &str = "  [x] ";

#[derive(Clone, Debug)]
struct TodoProperties<CallbackT> {
    style: Style,
    checked: bool,
    editing: bool,
    content: String,
    on_change: CallbackT,
}

#[derive(Debug)]
struct Todo<CallbackT: FnMut(String) + Clone + 'static> {
    properties: TodoProperties<CallbackT>,
    styled_content: String,
}

impl<CallbackT> Component for Todo<CallbackT>
where
    CallbackT: FnMut(String) + Clone + 'static,
{
    type Message = ();
    type Properties = TodoProperties<CallbackT>;

    fn create(
        properties: Self::Properties,
        _link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Self {
            properties,
            styled_content: String::new(),
        }
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn view(&self, _frame: Rect) -> Layout {
        let Self::Properties {
            ref on_change,
            ref content,
            checked,
            style,
            editing,
        } = self.properties;
        let todo_component = if editing {
            layout::component::<Input<_>>(InputProperties {
                style: Default::default(),
                content: Rope::from_str(&content),
                on_change: Some(on_change.clone()),
            })
        } else {
            layout::component::<Text>(TextProperties {
                style,
                content: if checked {
                    unicode_strikethrough(content)
                } else {
                    content.clone()
                },
                align: TextAlign::Left,
            })
        };

        let checkbox_width = UnicodeWidthStr::width(if checked { CHECKED } else { UNCHECKED }) + 1;
        layout::row([
            layout::fixed(
                checkbox_width,
                layout::component::<Checkbox>(CheckboxProperties { style, checked }),
            ),
            layout::stretched(todo_component),
        ])
    }
}

fn unicode_strikethrough(content: &str) -> String {
    let content = content.trim();
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
}

impl Default for Theme {
    fn default() -> Self {
        const DARK0_SOFT: Colour = Colour::rgb(50, 48, 47);
        const LIGHT2: Colour = Colour::rgb(213, 196, 161);
        const GRAY_245: Colour = Colour::rgb(146, 131, 116);
        const BRIGHT_BLUE: Colour = Colour::rgb(131, 165, 152);

        Self {
            unchecked: Style::normal(DARK0_SOFT, LIGHT2),
            checked: Style::normal(DARK0_SOFT, GRAY_245),
            focused: Style::normal(BRIGHT_BLUE, DARK0_SOFT),
        }
    }
}

#[derive(Clone, Debug)]
struct TodoItem {
    checked: bool,
    content: String,
}
#[derive(Clone, Debug)]
struct State {
    theme: Theme,
    focus_index: usize,
    todos: Vec<TodoItem>,
    editing: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            theme: Default::default(),
            focus_index: 0,
            todos: vec![
                TodoItem {
                    checked: false,
                    content: "All work and no play makes Jack a dull boy\n".into(),
                },
                TodoItem {
                    checked: false,
                    content: "Выучить китайский для хорошего блага\n".into(),
                },
                TodoItem {
                    checked: true,
                    content: "Unicode support 學點中文造福大家\n".into(),
                },
            ],
            editing: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Message {
    AddItem,
    FocusNextItem,
    FocusPreviousItem,
    MoveItemUp,
    MoveItemDown,
    DeleteItem,
    DeleteDone,
    ToggleDone,
    ChangeContent((usize, String)),
    Edit,
}

#[derive(Debug)]
struct TodoMvc {
    properties: State,
    link: ComponentLink<Self>,
}

impl Component for TodoMvc {
    type Message = Message;
    type Properties = State;

    fn create(
        properties: Self::Properties,
        link: ComponentLink<Self>,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> Self {
        Self { properties, link }
    }

    fn update(
        &mut self,
        message: Self::Message,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        eprintln!("Message: {:?}", message);

        match message {
            Message::Edit => {
                self.properties.editing = false;
            }
            Message::AddItem if !self.properties.editing => {
                self.properties.editing = true;
                if self.properties.todos.is_empty() {
                    self.properties.todos.push(TodoItem {
                        checked: false,
                        content: "\n".into(),
                    });
                }
            }
            Message::AddItem if self.properties.editing => {
                self.properties.todos.insert(
                    cmp::min(self.properties.focus_index + 1, self.properties.todos.len()),
                    TodoItem {
                        checked: false,
                        content: "\n".into(),
                    },
                );
                self.properties.focus_index = cmp::min(
                    self.properties.focus_index + 1,
                    self.properties.todos.len().saturating_sub(1),
                );
            }
            Message::FocusNextItem => {
                self.properties.editing = false;
                self.properties.focus_index = cmp::min(
                    self.properties.focus_index + 1,
                    self.properties.todos.len().saturating_sub(1),
                );
            }
            Message::FocusPreviousItem => {
                self.properties.editing = false;
                self.properties.focus_index = self.properties.focus_index.saturating_sub(1);
            }
            Message::MoveItemUp => {
                let current_index = self.properties.focus_index;
                let new_index = self.properties.focus_index.saturating_sub(1);
                if current_index != new_index {
                    self.properties.todos.swap(current_index, new_index);
                    self.properties.focus_index = new_index;
                }
            }
            Message::MoveItemDown => {
                let current_index = self.properties.focus_index;
                let new_index = cmp::min(
                    self.properties.focus_index + 1,
                    self.properties.todos.len().saturating_sub(1),
                );
                if current_index != new_index {
                    self.properties.todos.swap(current_index, new_index);
                    self.properties.focus_index = new_index;
                }
            }
            Message::DeleteItem if !self.properties.todos.is_empty() => {
                self.properties.todos.remove(self.properties.focus_index);
                self.properties.focus_index = cmp::min(
                    self.properties.focus_index,
                    self.properties.todos.len().saturating_sub(1),
                );
            }
            Message::DeleteDone if !self.properties.todos.is_empty() => {
                self.properties.focus_index = 0;
                self.properties.todos.retain(|item| !item.checked);
            }
            Message::ToggleDone if !self.properties.todos.is_empty() => {
                let checked = &mut self.properties.todos[self.properties.focus_index].checked;
                *checked = !*checked;
                self.update(Message::FocusNextItem, _scheduler);
            }
            Message::ChangeContent(content) => {
                self.properties.todos[content.0].content = content.1;
            }
            _ => {}
        }
        ShouldRender::Yes
    }

    fn change(
        &mut self,
        properties: Self::Properties,
        _scheduler: &mut Scheduler<Self::Message>,
    ) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn view(&self, _frame: Rect) -> Layout {
        let Self::Properties {
            ref theme,
            ref todos,
            focus_index,
            editing,
        } = self.properties;
        let num_left = todos.iter().filter(|item| !item.checked).count();

        // Title component
        let title = iter::once(layout::fixed(
            LOGO.lines().count() + 1,
            layout::component::<Text>(TextProperties {
                style: theme.checked,
                content: LOGO.into(),
                align: TextAlign::Centre,
            }),
        ));

        // The list of todo items
        let todo_items = todos.iter().enumerate().map(|(index, item)| {
            let link = self.link.clone();
            layout::fixed(
                1,
                layout::component_with_key::<Todo<_>>(
                    index,
                    TodoProperties {
                        style: if index == focus_index && !editing {
                            theme.focused.clone()
                        } else if item.checked {
                            theme.checked.clone()
                        } else {
                            theme.unchecked.clone()
                        },
                        checked: item.checked,
                        content: item.content.clone(),
                        editing: index == focus_index && editing,
                        on_change: move |content| {
                            link.send(Message::ChangeContent((index, content)))
                        },
                    },
                ),
            )
        });

        // "Filler" component for the unused space
        let filler = iter::once(layout::stretched(layout::component::<Text>(
            TextProperties {
                style: theme.checked,
                content: "".into(),
                align: TextAlign::Left,
            },
        )));

        // Status bar at the bottom counting how many items have been ticked off
        let status_bar = iter::once(layout::fixed(
            1,
            layout::component::<Text>(TextProperties {
                style: theme.checked,
                content: format!("{} items left ({} done)", num_left, todos.len() - num_left),
                align: TextAlign::Left,
            }),
        ));

        layout::column_iter(title.chain(todo_items).chain(filler).chain(status_bar))
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        let mut transition = BindingTransition::Clear;
        let message = match pressed {
            &[Key::Esc] | &[Key::Alt('\u{1b}')] => Some(Message::Edit),
            &[Key::Char('\n')] => Some(Message::AddItem),
            &[Key::Ctrl('p')] | &[Key::Up] => Some(Message::FocusPreviousItem),
            &[Key::Ctrl('n')] | &[Key::Down] => Some(Message::FocusNextItem),
            &[Key::Alt('p')] => Some(Message::MoveItemUp),
            &[Key::Alt('n')] => Some(Message::MoveItemDown),
            &[Key::Ctrl('k')] => Some(Message::DeleteItem),
            &[Key::Ctrl('x'), Key::Ctrl('k')] => Some(Message::DeleteDone),
            &[Key::Char('\t')] => Some(Message::ToggleDone),
            &[Key::Ctrl('x'), Key::Ctrl('c')] => {
                transition = BindingTransition::Exit;
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
    let mut app = App::new::<TodoMvc>(State::default())?;
    app.run_event_loop(Termion::new()?)
}
