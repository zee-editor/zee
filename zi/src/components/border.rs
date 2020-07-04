use std::iter;

use crate::{layout, Canvas, Component, ComponentLink, Layout, Rect, ShouldRender, Size, Style};

#[derive(Clone)]
pub struct BorderProperties {
    pub component: Layout,
    pub style: Style,
    pub stroke: BorderStroke,
    pub title: Option<(String, Style)>,
}

impl BorderProperties {
    pub fn new(component: Layout) -> Self {
        Self {
            component,
            style: Style::default(),
            stroke: BorderStroke::default(),
            title: None,
        }
    }

    pub fn style(mut self, style: impl Into<Style>) -> Self {
        self.style = style.into();
        self
    }

    pub fn stroke(mut self, stroke: BorderStroke) -> Self {
        self.stroke = stroke;
        self
    }

    pub fn title(mut self, title: Option<(impl Into<String>, Style)>) -> Self {
        self.title = title.map(|title| (title.0.into(), title.1));
        self
    }
}

pub struct Border {
    properties: BorderProperties,
    frame: Rect,
}

impl Component for Border {
    type Message = ();
    type Properties = BorderProperties;

    fn create(properties: Self::Properties, frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self { properties, frame }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        self.properties = properties;
        ShouldRender::Yes
    }

    fn resize(&mut self, frame: Rect) -> ShouldRender {
        self.frame = frame;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let stroke = &self.properties.stroke;

        // Draw the top border
        let mut top_border = Canvas::new(Size::new(self.frame.size.width, 1));
        top_border.draw_graphemes(
            0,
            0,
            self.properties.style,
            iter::once(stroke.top_left_corner)
                .chain(
                    iter::repeat(stroke.top_horizontal)
                        .take(self.frame.size.width.saturating_sub(2)),
                )
                .chain(iter::once(stroke.top_right_corner)),
        );

        // Draw title if present
        if let Some(title) = self.properties.title.as_ref() {
            top_border.draw_str(2, 0, title.1, &title.0);
        }

        // Draw right border
        let mut right_border = Canvas::new(Size::new(1, self.frame.size.height.saturating_sub(2)));
        (0..self.frame.size.height.saturating_sub(2)).for_each(|y| {
            right_border.draw_graphemes(
                0,
                y,
                self.properties.style,
                iter::once(stroke.right_vertical),
            );
        });

        // Draw bottom border
        let mut bottom_border = Canvas::new(Size::new(self.frame.size.width, 1));
        bottom_border.draw_graphemes(
            0,
            0,
            self.properties.style,
            iter::once(stroke.bottom_left_corner)
                .chain(
                    iter::repeat(stroke.bottom_horizontal)
                        .take(self.frame.size.width.saturating_sub(2)),
                )
                .chain(iter::once(stroke.bottom_right_corner)),
        );

        // Draw left border
        let mut left_border = Canvas::new(Size::new(1, self.frame.size.height.saturating_sub(2)));
        (0..self.frame.size.height.saturating_sub(2)).for_each(|y| {
            left_border.draw_graphemes(
                0,
                y,
                self.properties.style,
                iter::once(stroke.left_vertical),
            );
        });

        // Assemble layout
        layout::column([
            layout::fixed(1, top_border.into()),
            layout::auto(layout::row([
                layout::fixed(1, left_border.into()),
                layout::auto(self.properties.component.clone()),
                layout::fixed(1, right_border.into()),
            ])),
            layout::fixed(1, bottom_border.into()),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct BorderStroke {
    pub top_left_corner: char,
    pub top_horizontal: char,
    pub top_right_corner: char,
    pub bottom_left_corner: char,
    pub bottom_horizontal: char,
    pub bottom_right_corner: char,
    pub left_vertical: char,
    pub right_vertical: char,
}

impl Default for BorderStroke {
    fn default() -> Self {
        Self::light_rounded()
    }
}

impl BorderStroke {
    pub const fn light_rounded() -> Self {
        Self {
            top_left_corner: '╭',
            top_horizontal: '─',
            top_right_corner: '╮',
            bottom_left_corner: '╰',
            bottom_horizontal: '─',
            bottom_right_corner: '╯',
            left_vertical: '│',
            right_vertical: '│',
        }
    }

    pub const fn block() -> Self {
        Self {
            top_left_corner: '█',
            top_horizontal: '▀',
            top_right_corner: '█',
            bottom_left_corner: '█',
            bottom_horizontal: '▄',
            bottom_right_corner: '█',
            left_vertical: '█',
            right_vertical: '█',
        }
    }

    pub const fn heavy() -> Self {
        Self {
            top_left_corner: '┏',
            top_horizontal: '━',
            top_right_corner: '┓',
            bottom_left_corner: '┗',
            bottom_horizontal: '━',
            bottom_right_corner: '┛',
            left_vertical: '┃',
            right_vertical: '┃',
        }
    }

    pub const fn double() -> Self {
        Self {
            top_left_corner: '╔',
            top_horizontal: '═',
            top_right_corner: '╗',
            bottom_left_corner: '╚',
            bottom_horizontal: '═',
            bottom_right_corner: '╝',
            left_vertical: '║',
            right_vertical: '║',
        }
    }
}
