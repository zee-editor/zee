use zi::{
    components::{
        border::{Border, BorderProperties},
        text::{Text, TextAlign, TextProperties},
    },
    prelude::*,
};

// Message type handled by the `Counter` component.
enum Message {
    Increment,
    Decrement,
}

// The `Counter` component.
struct Counter {
    // The state of the component -- the current value of the counter.
    count: usize,

    // A `ComponentLink` allows us to send messages to the component in reaction
    // to user input as well as to gracefully exit.
    link: ComponentLink<Self>,
}

// Components implement the `Component` trait and are the building blocks of the
// UI in Zi. The trait describes stateful components and their lifecycle.
impl Component for Counter {
    // Messages are used to make components dynamic and interactive. For simple
    // or pure components, this will be `()`. Complex, stateful ones will
    // typically use an enum to declare multiple Message types. In this case, we
    // will emit two kinds of message (`Increment` or `Decrement`) in reaction
    // to user input.
    type Message = Message;

    // Properties are the inputs to a Component passed in by their parent.
    type Properties = ();

    // Creates ("mounts") a new `Counter` component.
    fn create(_properties: Self::Properties, _frame: Rect, link: ComponentLink<Self>) -> Self {
        Self { count: 0, link }
    }

    // Returns the current visual layout of the component.
    fn view(&self) -> Layout {
        zi::component::<Border>(
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

    // Components handle messages in their `update` method and commonly use this
    // method to update their state and (optionally) re-render themselves.
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

    // Whether the component is currently focused which will caused
    // `input_binding` to be called on user input.
    fn has_focus(&self) -> bool {
        true
    }

    // Send messages in reaction to user input.
    fn input_binding(&self, pressed: &[Key]) -> BindingMatch<Self::Message> {
        BindingMatch::clear(match pressed {
            &[Key::Char('+')] | &[Key::Char('=')] => Some(Message::Increment),
            &[Key::Char('-')] => Some(Message::Decrement),
            &[Key::Ctrl('c')] | &[Key::Esc] => {
                self.link.exit();
                None
            }
            _ => None,
        })
    }
}

const BACKGROUND: Colour = Colour::rgb(50, 48, 47);
const FOREGROUND: Colour = Colour::rgb(213, 196, 161);
const STYLE: Style = Style::bold(BACKGROUND, FOREGROUND);

fn main() -> zi::Result<()> {
    let mut app = App::new(layout::component::<Counter>(()));
    app.run_event_loop(zi::frontend::default()?)
}
