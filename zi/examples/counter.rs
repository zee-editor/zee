use zi::{
    components::{
        border::{Border, BorderProperties},
        text::{Text, TextAlign, TextProperties},
    },
    layout, App, BindingMatch, BindingTransition, Colour, Component, ComponentLink, Key, Layout,
    Rect, Result, ShouldRender, Style,
};

enum Message {
    Increment,
    Decrement,
}

struct Counter {
    count: usize,
    link: ComponentLink<Self>,
}

impl Component for Counter {
    type Message = Message;
    type Properties = ();

    fn create(_properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        Self { count: 0, link }
    }

    fn view(&self) -> Layout {
        layout::component::<Border>(
            BorderProperties::new(layout::component::<Text>(
                TextProperties::new()
                    .align(TextAlign::Centre)
                    .style(STYLE)
                    .content(format!(
                        "\nCounter: {:>3}  [+ to increment | - to decrement | C-c to exit]",
                        self.count
                    )),
            ))
            .style(STYLE),
        )
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        let new_count = match message {
            Message::Increment => self.count.saturating_add(1),
            Message::Decrement => self.count.saturating_sub(1),
        };
        if new_count != self.count {
            self.count = new_count;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn has_focus(&self) -> bool {
        true
    }

    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        BindingMatch {
            transition: BindingTransition::Clear,
            message: match pressed {
                &[Key::Char('+')] | &[Key::Char('=')] => Some(Message::Increment),
                &[Key::Char('-')] => Some(Message::Decrement),
                &[Key::Ctrl('c')] | &[Key::Esc] => {
                    self.link.exit();
                    None
                }
                _ => None,
            },
        }
    }
}

const BACKGROUND: Colour = Colour::rgb(50, 48, 47);
const FOREGROUND: Colour = Colour::rgb(213, 196, 161);
const STYLE: Style = Style::bold(BACKGROUND, FOREGROUND);

fn main() -> Result<()> {
    let mut app = App::new(layout::component::<Counter>(()));
    app.run_event_loop(zi::frontend::default()?)
}
