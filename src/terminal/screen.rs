use ropey::RopeSlice;
use std::{self, cmp, iter};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::Size;
use crate::{smallstring::SmallString, terminal::Rect, utils::RopeGraphemes};

#[derive(Default, Clone, PartialEq)]
pub struct Textel {
    pub style: Style,
    pub content: SmallString,
}

pub struct Screen {
    width: usize,
    height: usize,
    buffer: Vec<Option<Textel>>,
}

impl Screen {
    pub fn new(size: Size) -> Self {
        // Allocate initial draw buffer
        Screen {
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
        let y_range = region.origin.y..region.origin.y + cmp::min(region.size.height, self.height);
        let x_range = region.origin.x..region.origin.x + cmp::min(region.size.width, self.width);
        for y in y_range {
            self.buffer[y * self.width + x_range.start..y * self.width + x_range.end]
                .iter_mut()
                .for_each(|textel| Self::clear_textel(textel, style))
        }
    }

    #[inline]
    pub fn draw_str(&mut self, x: usize, y: usize, style: Style, text: &str) -> usize {
        self.draw_graphemes(x, y, style, UnicodeSegmentation::graphemes(text, true))
    }

    #[inline]
    pub fn draw_rope_slice(&mut self, x: usize, y: usize, style: Style, text: &RopeSlice) -> usize {
        self.draw_graphemes(x, y, style, RopeGraphemes::new(text))
    }

    #[inline]
    fn draw_graphemes<IteratorT, ItemT>(
        &mut self,
        x: usize,
        y: usize,
        style: Style,
        grapheme_iter: IteratorT,
    ) -> usize
    where
        IteratorT: Iterator<Item = ItemT>,
        ItemT: Into<SmallString>,
    {
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
            // eprintln!("'{}' {}", grapheme, grapheme_width);
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
    fn clear_textel(textel: &mut Option<Textel>, style: Style) {
        match *textel {
            Some(Textel {
                style: ref mut textel_style,
                ref mut content,
            }) => {
                *textel_style = style;
                content.clear();
                content.push_str(" ");
            }
            _ => {
                *textel = Some(Textel {
                    style,
                    content: " ".into(),
                });
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Style {
    pub background: Background,
    pub foreground: Foreground,
    pub bold: bool,
    pub underline: bool,
}

impl Style {
    #[inline]
    pub fn normal(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
            bold: false,
            underline: false,
        }
    }

    #[inline]
    pub fn bold(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
            bold: true,
            underline: false,
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn underline(background: Background, foreground: Foreground) -> Self {
        Self {
            background,
            foreground,
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

#[derive(Debug, Copy, Clone, PartialEq)]
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Background(pub Colour);

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Foreground(pub Colour);
