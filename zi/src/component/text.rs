use unicode_width::UnicodeWidthStr;

use super::{layout::Layout, Component, ComponentLink, ShouldRender};
use crate::{
    task::Scheduler,
    terminal::{Canvas, Rect, Size, Style},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Centre,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProperties {
    pub style: Style,
    pub content: String,
    pub align: TextAlign,
}

#[derive(Debug)]
pub struct Text {
    properties: <Self as Component>::Properties,
}

impl Text {
    fn new(properties: <Self as Component>::Properties) -> Self {
        Self { properties }
    }
}

impl Component for Text {
    type Message = ();
    type Properties = TextProperties;

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
        let Self::Properties {
            align,
            style,
            ref content,
        } = self.properties;

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(style);

        let content_size = text_block_size(content);
        let position_x = match align {
            TextAlign::Left => 0,
            TextAlign::Centre => (frame.size.width / 2).saturating_sub(content_size.width / 2),
            TextAlign::Right => frame.size.width.saturating_sub(content_size.width),
        };
        for (position_y, line) in content.lines().enumerate() {
            canvas.draw_str(position_x, position_y, style, line);
        }

        Layout::Canvas(canvas)
    }
}

fn text_block_size(text: &str) -> Size {
    let width = text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
    let height = text.lines().count();
    Size::new(width, height)
}
