use ropey::RopeSlice;
use std::{
    self, cmp,
    fmt::{self, Display, Formatter},
    io::{self, BufWriter, Stdout, Write},
    iter,
};
use termion::{
    self,
    cursor::Goto,
    raw::{IntoRawMode, RawTerminal},
    screen::AlternateScreen,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{smallstring::SmallString, terminal::Rect, utils::RopeGraphemes};

#[derive(Default, Clone, PartialEq)]
pub struct Textel {
    style: Style,
    content: SmallString,
}

pub struct Screen {
    pub width: usize,
    pub height: usize,
    screen: AlternateScreen<RawTerminal<BufWriter<Stdout>>>,
    buffer: Vec<Option<Textel>>,
}

impl Screen {
    pub fn new() -> Result<Self, io::Error> {
        // Determine the current size of the terminal
        let (width, height) = termion::terminal_size()?;
        let (width, height) = (width as usize, height as usize);

        // Allocate initial draw buffer
        let buffer = iter::repeat(Textel::default())
            .map(Some)
            .take(width * height)
            .collect();

        // Create draw target
        let mut screen =
            AlternateScreen::from(BufWriter::with_capacity(1 << 20, io::stdout()).into_raw_mode()?);
        write!(screen, "{}", termion::cursor::Hide)?;

        Ok(Screen {
            width,
            height,
            screen,
            buffer,
        })
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.buffer.resize(width * height, Default::default());
    }

    pub fn resize_to_terminal(&mut self) -> Result<(), io::Error> {
        let (width, height) = termion::terminal_size()?;
        self.resize(width as usize, height as usize);
        Ok(())
    }

    pub fn present(&mut self) -> Result<(), io::Error> {
        let Self {
            width,
            ref mut buffer,
            ref mut screen,
            ..
        } = *self;

        let mut last_style = Style::default();
        write!(screen, "{}", last_style)?;

        buffer.chunks(width).enumerate().try_for_each(|(y, line)| {
            write!(screen, "{}", Self::goto(0, y as u16))?;
            line.iter().try_for_each(|textel| -> Result<(), io::Error> {
                if let Some(Textel {
                    ref style,
                    ref content,
                }) = textel
                {
                    if *style != last_style {
                        write!(screen, "{}", style)?;
                        last_style = *style;
                    }
                    write!(screen, "{}", content)?;
                }
                Ok(())
            })
        })?;

        screen.flush()
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

    #[inline]
    pub fn goto(x: u16, y: u16) -> Goto {
        Goto(x + 1, y + 1)
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        write!(
            self.screen,
            "{}{}{}{}{}",
            termion::color::Fg(termion::color::Reset),
            termion::color::Bg(termion::color::Reset),
            termion::clear::All,
            termion::cursor::Show,
            termion::screen::ToMainScreen
        )
        .expect("clear screen on drop");
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
    red: u8,
    blue: u8,
    green: u8,
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

impl Display for Style {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        // Bold
        if self.bold {
            write!(formatter, "{}", termion::style::Bold)?;
        } else {
            // Using Reset is not ideal as it resets all style attributes. The correct thing to do
            // would be to use `NoBold`, but it seems this is not reliably supported (at least it
            // didn't work for me in tmux, although it does in alacritty).
            // Also see https://github.com/crossterm-rs/crossterm/issues/294
            write!(formatter, "{}", termion::style::Reset)?;
        }

        // Underline
        if self.underline {
            write!(formatter, "{}", termion::style::Underline)?;
        } else {
            write!(formatter, "{}", termion::style::NoUnderline)?;
        }

        // Background
        {
            let Colour { red, green, blue } = self.background.0;
            write!(
                formatter,
                "{}",
                termion::color::Bg(termion::color::Rgb(red, green, blue))
            )?;
        }

        // Foreground
        {
            let Colour { red, green, blue } = self.foreground.0;
            write!(
                formatter,
                "{}",
                termion::color::Fg(termion::color::Rgb(red, green, blue))
            )?;
        }

        Ok(())
    }
}
