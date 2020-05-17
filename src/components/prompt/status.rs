use palette::{Gradient, Hsv, LinSrgb};
use zi::{
    component::text::{Text, TextAlign, TextProperties},
    layout,
    terminal::Style,
    Colour, Component, ComponentLink, Layout, Rect, ShouldRender,
};

use super::State;

#[derive(Clone, PartialEq)]
pub struct StatusProperties {
    pub status: State,
    pub pending: bool,
    pub style: Style,
}

pub struct Status {
    properties: StatusProperties,
    animation_offset: f32,
    gradient: Gradient<Hsv>,
}

impl Component for Status {
    type Message = ();
    type Properties = StatusProperties;

    fn create(properties: Self::Properties, _frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self {
            gradient: gradient_from_style(properties.style),
            properties,
            animation_offset: 1.0,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.gradient = gradient_from_style(properties.style);
            if self.properties.pending != properties.pending {
                self.animation_offset = 1.0;
            }
            self.properties = properties;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn update(&mut self, _message: Self::Message) -> ShouldRender {
        // `animation_offset` ticks in the interval [0, 2]:
        self.animation_offset = (self.animation_offset + 0.125) % 2.0;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            properties:
                StatusProperties {
                    ref status,
                    style,
                    pending,
                },
            ..
        } = *self;

        let content = match status {
            State::PickingFileFromRepo => "repo",
            State::PickingFileFromDirectory => "open",
            State::Inactive => "",
        };
        let style = if pending {
            self.animated_style()
        } else {
            style
        };
        layout::component::<Text>(TextProperties::new().content(content).style(style))
    }

    fn tick(&self) -> Option<Self::Message> {
        if self.properties.pending {
            Some(())
        } else {
            None
        }
    }
}

fn gradient_from_style(style: Style) -> Gradient<Hsv> {
    Gradient::new(vec![
        Hsv::from(
            LinSrgb::new(
                style.background.red,
                style.background.green,
                style.background.blue,
            )
            .into_format::<f32>(),
        ),
        Hsv::from(
            LinSrgb::new(
                style.foreground.red,
                style.foreground.green,
                style.foreground.blue,
            )
            .into_format::<f32>(),
        ),
    ])
}

impl Status {
    fn animated_style(&self) -> Style {
        let background = LinSrgb::from(self.gradient.get((self.animation_offset - 1.0).abs()))
            .into_format::<u8>();
        let foreground =
            LinSrgb::from(self.gradient.get(1.0 - (self.animation_offset - 1.0).abs()))
                .into_format::<u8>();

        Style::normal(
            Colour {
                red: background.red,
                green: background.green,
                blue: background.blue,
            },
            Colour {
                red: foreground.red,
                green: foreground.green,
                blue: foreground.blue,
            },
        )
    }
}

// const PROGRESS_PATTERN: [char; 16] = [
//     '⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷', '⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈',
// ];
// const PROGRESS_PATTERN: [char; 13] = [
//     '▉', '▊', '▋', '▌', '▍', '▎', '▏', '▎', '▍', '▌', '▋', '▊', '▉',
// ];
// const PROGRESS_PATTERN: [char; 8] = ['▙', '▛', '▜', '▟', '▘', '▝', '▖', '▗'];
// const PROGRESS_PATTERN: [char; 6] = ['◜', '◠', '◝', '◞', '◡', '◟'];
// const PROGRESS_PATTERN: [char; 4] = ['■', '□', '▪', '▫'];
// const PROGRESS_PATTERN: [char; 8] = ['▘', '▀', '▝', '▐', '▗', '▄', '▖', '▌'];
// const PROGRESS_PATTERN: [char; 29] = [
//     '⠁', '⠁', '⠉', '⠙', '⠚', '⠒', '⠂', '⠂', '⠒', '⠲', '⠴', '⠤', '⠄', '⠄', '⠤', '⠠', '⠠', '⠤', '⠦',
//     '⠖', '⠒', '⠐', '⠐', '⠒', '⠓', '⠋', '⠉', '⠈', '⠈',
// ];
