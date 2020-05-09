use smallstr::SmallString;
use std::{self, cmp, iter};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{Position, Size};
use crate::terminal::Rect;

pub type TextelContent = SmallString<[u8; 8]>;

#[derive(Default, Clone, PartialEq)]
pub struct Textel {
    pub style: Style,
    pub content: TextelContent,
}

pub struct Canvas {
    width: usize,
    height: usize,
    buffer: Vec<Option<Textel>>,
}

impl Canvas {
    pub fn new(size: Size) -> Self {
        // Allocate initial draw buffer
        Canvas {
            width: size.width,
            height: size.height,
            buffer: iter::repeat(Textel::default())
                .map(Some)
                .take(size.width * size.height)
                .collect(),
        }
    }

    #[inline]
    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    #[inline]
    pub fn buffer(&self) -> &[Option<Textel>] {
        self.buffer.as_slice()
    }

    #[inline]
    pub fn resize(&mut self, size: Size) {
        self.width = size.width;
        self.height = size.height;
        self.buffer
            .resize(size.width * size.height, Default::default());
    }

    #[inline]
    pub fn clear_region(&mut self, region: Rect, style: Style) {
        eprintln!("cs: {} {}", self.width, self.height);
        eprintln!("cl: {}", region);
        let y_range = region.origin.y..cmp::min(region.origin.y + region.size.height, self.height);
        let x_range = region.origin.x..cmp::min(region.origin.x + region.size.width, self.width);
        eprintln!("xr: {:?}", x_range);
        eprintln!("yr: {:?}", y_range);
        for y in y_range {
            self.buffer[y * self.width + x_range.start..y * self.width + x_range.end]
                .iter_mut()
                .for_each(|textel| clear_textel(textel, style, " "));
        }
    }

    #[inline]
    pub fn clear(&mut self, style: Style) {
        self.buffer
            .iter_mut()
            .for_each(|textel| clear_textel(textel, style, " "))
    }

    #[inline]
    pub fn clear_with(&mut self, style: Style, content: &str) {
        self.buffer
            .iter_mut()
            .for_each(|textel| clear_textel(textel, style, content))
    }

    #[inline]
    pub fn draw_str(&mut self, x: usize, y: usize, style: Style, text: &str) -> usize {
        self.draw_graphemes(x, y, style, UnicodeSegmentation::graphemes(text, true))
    }

    #[inline]
    pub fn draw_graphemes(
        &mut self,
        x: usize,
        y: usize,
        style: Style,
        grapheme_iter: impl Iterator<Item = impl Into<TextelContent>>,
    ) -> usize {
        if y >= self.height || x >= self.width {
            return 0;
        }

        let initial_offset = y * self.width + x;
        let max_offset = (y + 1) * self.width + x - 1;
        let mut current_offset = initial_offset;

        for grapheme in grapheme_iter {
            if current_offset >= max_offset {
                break;
            }

            let grapheme = grapheme.into();
            let grapheme_width = UnicodeWidthStr::width(grapheme.as_ref());
            if grapheme_width == 0 {
                continue;
            }

            self.buffer[current_offset] = Some(Textel {
                style,
                content: grapheme,
            });

            let num_modified = cmp::min(grapheme_width, max_offset - current_offset);
            self.buffer[current_offset + 1..current_offset + num_modified]
                .iter_mut()
                .for_each(|textel| *textel = None);

            current_offset += num_modified;
        }
        current_offset - initial_offset
    }

    #[inline]
    pub fn copy_region(&mut self, source: &Self, region: Rect) {
        let y_range = cmp::min(region.origin.y, self.height)
            ..cmp::min(region.origin.y + source.height, self.height);
        let x_range = cmp::min(region.origin.x, self.width)
            ..cmp::min(region.origin.x + source.width, self.width);

        for y in y_range {
            self.buffer[y * self.width + x_range.start..y * self.width + x_range.end]
                .iter_mut()
                .zip(
                    source.buffer[(y - region.origin.y) * source.width
                        ..(y - region.origin.y) * source.width + (x_range.end - region.origin.x)]
                        .iter(),
                )
                .for_each(|(textel, other)| *textel = other.clone());
        }
    }

    #[inline]
    pub fn draw_raw(&mut self, x: usize, y: usize) -> &mut Option<Textel> {
        &mut self.buffer[y * self.width + x]
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Style {
    pub background: Background,
    pub foreground: Foreground,
    pub bold: bool,
    pub underline: bool,
}

impl Style {
    #[inline]
    pub fn normal(background: impl Into<Background>, foreground: impl Into<Foreground>) -> Self {
        Self {
            background: background.into(),
            foreground: foreground.into(),
            bold: false,
            underline: false,
        }
    }

    #[inline]
    pub fn bold(background: impl Into<Background>, foreground: impl Into<Foreground>) -> Self {
        Self {
            background: background.into(),
            foreground: foreground.into(),
            bold: true,
            underline: false,
        }
    }

    #[inline]
    pub fn underline(background: impl Into<Background>, foreground: impl Into<Foreground>) -> Self {
        Self {
            background: background.into(),
            foreground: foreground.into(),
            bold: false,
            underline: true,
        }
    }

    #[inline]
    pub fn same_colour(colour: Colour) -> Self {
        Self {
            background: Background(colour),
            foreground: Foreground(colour),
            bold: false,
            underline: false,
        }
    }
}

impl Default for Style {
    #[inline]
    fn default() -> Self {
        Style::same_colour(Colour::black())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Colour {
    pub red: u8,
    pub blue: u8,
    pub green: u8,
}

impl Colour {
    #[inline]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[inline]
    pub fn black() -> Self {
        Self {
            red: 0,
            green: 0,
            blue: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Background(pub Colour);

impl From<Colour> for Background {
    fn from(colour: Colour) -> Self {
        Self(colour)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Foreground(pub Colour);

impl From<Colour> for Foreground {
    fn from(colour: Colour) -> Self {
        Self(colour)
    }
}

#[inline]
fn clear_textel(textel: &mut Option<Textel>, style: Style, value: &str) {
    match *textel {
        Some(Textel {
            style: ref mut textel_style,
            ref mut content,
        }) => {
            *textel_style = style;
            content.clear();
            content.push_str(value);
        }
        _ => {
            *textel = Some(Textel {
                style,
                content: " ".into(),
            });
        }
    }
}

const UPPER_HALF_BLOCK: &str = "▀";

pub struct SquarePixelGrid {
    canvas: Canvas,
}

impl SquarePixelGrid {
    pub fn new(size: Size) -> Self {
        assert!(size.height % 2 == 0);
        let mut canvas = Canvas::new(Size::new(size.width, size.height / 2));
        canvas.clear_with(Default::default(), UPPER_HALF_BLOCK);
        Self { canvas }
    }

    pub fn from_available(size: Size) -> Self {
        let mut canvas = Canvas::new(Size::new(size.width, size.height));
        canvas.clear_with(Default::default(), UPPER_HALF_BLOCK);
        Self { canvas }
    }

    #[inline]
    pub fn size(&self) -> Size {
        let canvas_size = self.canvas.size();
        Size::new(canvas_size.width, canvas_size.height * 2)
    }

    #[inline]
    pub fn draw(&mut self, position: Position, colour: Colour) {
        let textel = self
            .canvas
            .draw_raw(position.x, position.y / 2)
            .as_mut()
            .expect("No textels should be uninitialised");
        if position.y % 2 == 0 {
            textel.style.foreground = Foreground(colour);
        } else {
            textel.style.background = Background(colour);
        }
    }

    #[inline]
    pub fn into_canvas(self) -> Canvas {
        self.canvas
    }
}
