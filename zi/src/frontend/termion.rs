use crossbeam_channel::{self, Receiver};
use std::{
    fmt::{self, Display, Formatter},
    io::{self, BufWriter, Read, Stdout, Write},
    thread::{self, JoinHandle},
};
use termion::{
    self,
    event::Key as TermionKey,
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
};

use super::{
    painter::{FullPainter, IncrementalPainter, PaintOperation, Painter},
    utils::MeteredWriter,
    Frontend, Result,
};
use crate::terminal::{Canvas, Colour, Key, Position, Size, Style};

pub fn incremental() -> Result<Termion<IncrementalPainter>> {
    Termion::<IncrementalPainter>::new()
}

pub fn full() -> Result<Termion<FullPainter>> {
    Termion::<FullPainter>::new()
}

pub struct Termion<PainterT: Painter = IncrementalPainter> {
    target: RawTerminal<MeteredWriter<BufWriter<Stdout>>>,
    input: Input,
    painter: PainterT,
}

impl<PainterT: Painter> Termion<PainterT> {
    pub fn new() -> Result<Self> {
        let mut frontend = Self {
            target: MeteredWriter::new(BufWriter::with_capacity(1 << 20, io::stdout()))
                .into_raw_mode()?,
            input: Input::from_reader(termion::get_tty()?),
            painter: PainterT::create(
                termion::terminal_size()
                    .map(|(width, height)| Size::new(width as usize, height as usize))?,
            ),
        };
        initialise_tty::<PainterT, _>(&mut frontend.target)?;
        Ok(frontend)
    }
}

impl<PainterT: Painter> Frontend for Termion<PainterT> {
    #[inline]
    fn initialise(&mut self) -> Result<()> {
        self.painter = PainterT::create(self.size()?);
        initialise_tty::<PainterT, _>(&mut self.target)
    }

    #[inline]
    fn size(&self) -> Result<Size> {
        Ok(termion::terminal_size()
            .map(|(width, height)| Size::new(width as usize, height as usize))?)
    }

    #[inline]
    fn present(&mut self, canvas: &Canvas) -> Result<usize> {
        let Self {
            ref mut target,
            ref mut painter,
            ..
        } = *self;
        let initial_num_bytes_written = target.num_bytes_written();
        painter.paint(canvas, |operation| {
            match operation {
                PaintOperation::WriteContent(content) => write!(target, "{}", content)?,
                PaintOperation::SetStyle(style) => write!(target, "{}", style)?,
                PaintOperation::MoveTo(position) => write!(target, "{}", goto(&position))?,
            }
            Ok(())
        })?;
        target.flush()?;
        Ok(target.num_bytes_written() - initial_num_bytes_written)
    }

    #[inline]
    fn events(&self) -> &Receiver<Key> {
        &self.input.receiver
    }
}

impl<PainterT: Painter> Drop for Termion<PainterT> {
    fn drop(&mut self) {
        write!(
            self.target,
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

#[inline]
fn initialise_tty<PainterT: Painter, TargetT: Write>(target: &mut TargetT) -> Result<()> {
    write!(
        target,
        "{}{}{}{}",
        termion::screen::ToAlternateScreen,
        termion::cursor::Hide,
        goto(&PainterT::INITIAL_POSITION),
        PainterT::INITIAL_STYLE
    )?;
    Ok(())
}

#[inline]
fn goto(position: &Position) -> termion::cursor::Goto {
    // `Goto` uses 1-based indexing
    termion::cursor::Goto(position.x as u16 + 1, position.y as u16 + 1)
}

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
            let Colour { red, green, blue } = self.background;
            write!(
                formatter,
                "{}",
                termion::color::Bg(termion::color::Rgb(red, green, blue))
            )?;
        }

        // Foreground
        {
            let Colour { red, green, blue } = self.foreground;
            write!(
                formatter,
                "{}",
                termion::color::Fg(termion::color::Rgb(red, green, blue))
            )?;
        }

        Ok(())
    }
}

struct Input {
    receiver: Receiver<Key>,
    _handle: JoinHandle<()>,
}

impl Input {
    pub fn from_reader(reader: impl Read + Send + 'static) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(2048);
        let _handle = thread::spawn(move || {
            for event in reader.keys() {
                match event {
                    Ok(termion_key) => {
                        sender.send(map_key(termion_key)).unwrap();
                    }
                    error => {
                        error.unwrap();
                    }
                }
            }
        });
        Self { receiver, _handle }
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        // ??
    }
}

#[inline]
fn map_key(key: TermionKey) -> Key {
    match key {
        TermionKey::Backspace => Key::Backspace,
        TermionKey::Left => Key::Left,
        TermionKey::Right => Key::Right,
        TermionKey::Up => Key::Up,
        TermionKey::Down => Key::Down,
        TermionKey::Home => Key::Home,
        TermionKey::End => Key::End,
        TermionKey::PageUp => Key::PageUp,
        TermionKey::PageDown => Key::PageDown,
        TermionKey::BackTab => Key::BackTab,
        TermionKey::Delete => Key::Delete,
        TermionKey::Insert => Key::Insert,
        TermionKey::F(u8) => Key::F(u8),
        TermionKey::Char(char) => Key::Char(char),
        TermionKey::Alt(char) => Key::Alt(char),
        TermionKey::Ctrl(char) => Key::Ctrl(char),
        TermionKey::Null => Key::Null,
        TermionKey::Esc => Key::Esc,
        _ => panic!("Unknown termion key event: {:?}", key),
    }
}
