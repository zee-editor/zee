use unicode_width::UnicodeWidthStr;

use crate::{layout::Layout, Canvas, Component, ComponentLink, Rect, ShouldRender, Size, Style};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Centre,
    Right,
}

impl Default for TextAlign {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextWrap {
    None,
    Word,
}

impl Default for TextWrap {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextProperties {
    pub style: Style,
    pub content: String,
    pub align: TextAlign,
    pub wrap: TextWrap,
}

impl TextProperties {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }
}

#[derive(Debug)]
pub struct Text {
    frame: Rect,
    properties: <Self as Component>::Properties,
}

impl Component for Text {
    type Message = ();
    type Properties = TextProperties;

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
        if self.frame != frame {
            self.frame = frame;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn view(&self) -> Layout {
        let Self {
            frame,
            properties:
                Self::Properties {
                    ref content,
                    align,
                    style,
                    wrap,
                },
        } = *self;

        let mut canvas = Canvas::new(frame.size);
        canvas.clear(style);

        let content_size = text_block_size(content);
        let position_x = match align {
            TextAlign::Left => 0,
            TextAlign::Centre => (frame.size.width / 2).saturating_sub(content_size.width / 2),
            TextAlign::Right => frame.size.width.saturating_sub(content_size.width),
        };

        let mut position_y = 0;
        for line in content.lines() {
            match wrap {
                TextWrap::None => {
                    canvas.draw_str(position_x, position_y, style, line);
                }
                TextWrap::Word => {
                    let mut cursor_x = position_x;
                    for word in line.split_whitespace() {
                        let word_width = UnicodeWidthStr::width(word);
                        if cursor_x > position_x {
                            if cursor_x >= frame.size.width
                                || word_width > frame.size.width.saturating_sub(cursor_x + 1)
                            {
                                position_y += 1;
                                cursor_x = position_x
                            } else {
                                canvas.draw_str(cursor_x, position_y, style, " ");
                                cursor_x += 1;
                            }
                        }
                        canvas.draw_str(cursor_x, position_y, style, word);
                        cursor_x += word_width;
                    }
                }
            }
            position_y += 1;
        }

        Layout::Canvas(canvas)
    }
}

fn text_block_size(text: &str) -> Size {
    let width = text.lines().map(UnicodeWidthStr::width).max().unwrap_or(0);
    let height = text.lines().count();
    Size::new(width, height)
}
