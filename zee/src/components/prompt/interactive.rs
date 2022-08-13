use std::borrow::Cow;

use zi::{
    components::text::{Text, TextProperties},
    prelude::*,
    Callback,
};

use super::Theme;

// Message type handled by the `InteractiveMessage` component
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Message {
    Accept,
    Decline,
}

pub struct Properties {
    pub theme: Cow<'static, Theme>,
    pub on_input: Callback<bool>,
    pub message: String,
}

pub struct InteractiveMessage {
    properties: Properties,
}

impl Component for InteractiveMessage {
    type Message = Message;

    type Properties = Properties;

    fn create(properties: Self::Properties, _frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties }
    }

    fn view(&self) -> Layout {
        let message = format!("{} (y/n)", self.properties.message);
        Text::with(
            TextProperties::new()
                .style(self.properties.theme.input)
                .content(message),
        )
    }

    fn update(&mut self, message: Self::Message) -> ShouldRender {
        self.properties.on_input.emit(message == Message::Accept);
        ShouldRender::No
    }

    fn bindings(&self, bindings: &mut Bindings<Self>) {
        if !bindings.is_empty() {
            return;
        }

        // Set focus to `true` in order to react to key presses
        bindings.set_focus(true);

        bindings
            .command("accept", || Message::Accept)
            .with([Key::Char('y')]);

        bindings
            .command("decline", || Message::Decline)
            .with([Key::Esc])
            .with([Key::Char('n')]);
    }
}
