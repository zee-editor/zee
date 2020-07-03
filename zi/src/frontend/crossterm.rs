use crossterm::{self, queue, QueueableCommand};
use futures::stream::{Stream, StreamExt};
use std::{
    io::{self, BufWriter, Stdout, Write},
    pin::Pin,
};

use super::{
    painter::{FullPainter, IncrementalPainter, PaintOperation, Painter},
    utils::MeteredWriter,
    Event, Frontend, Result,
};
use crate::terminal::{Canvas, Colour, Key, Size, Style};

pub fn incremental() -> Result<Crossterm<IncrementalPainter>> {
    Crossterm::<IncrementalPainter>::new()
}

pub fn full() -> Result<Crossterm<FullPainter>> {
    Crossterm::<FullPainter>::new()
}

pub type Error = crossterm::ErrorKind;

pub struct Crossterm<PainterT: Painter = IncrementalPainter> {
    target: MeteredWriter<BufWriter<Stdout>>,
    painter: PainterT,
    events: Option<Pin<Box<dyn Stream<Item = Result<Event>> + Send + 'static>>>,
}

impl<PainterT: Painter> Crossterm<PainterT> {
    pub fn new() -> Result<Self> {
        let mut frontend = Self {
            target: MeteredWriter::new(BufWriter::with_capacity(1 << 20, io::stdout())),
            painter: PainterT::create(
                crossterm::terminal::size()
                    .map(|(width, height)| Size::new(width as usize, height as usize))?,
            ),
            events: Some(new_event_stream()),
        };
        initialise_tty::<PainterT, _>(&mut frontend.target)?;
        Ok(frontend)
    }
}

impl<PainterT: Painter> Frontend for Crossterm<PainterT> {
    type EventStream = Pin<Box<dyn Stream<Item = Result<Event>> + Send + 'static>>;

    #[inline]
    fn size(&self) -> Result<Size> {
        Ok(crossterm::terminal::size()
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
                PaintOperation::WriteContent(grapheme) => {
                    queue!(target, crossterm::style::Print(grapheme))?
                }
                PaintOperation::SetStyle(style) => queue_set_style(target, style)?,
                PaintOperation::MoveTo(position) => queue!(
                    target,
                    crossterm::cursor::MoveTo(position.x as u16, position.y as u16)
                )?, // Go to the begining of line (`MoveTo` uses 0-based indexing)
            }
            Ok(())
        })?;
        target.flush()?;
        Ok(target.num_bytes_written() - initial_num_bytes_written)
    }

    #[inline]
    fn event_stream(&mut self) -> &mut Self::EventStream {
        self.events.as_mut().expect("Frontend events are suspended")
    }

    #[inline]
    fn suspend(&mut self) -> Result<()> {
        self.events = None;
        Ok(())
    }

    #[inline]
    fn resume(&mut self) -> Result<()> {
        self.painter = PainterT::create(self.size()?);
        self.events = Some(new_event_stream());
        initialise_tty::<PainterT, _>(&mut self.target)
    }
}

impl<PainterT: Painter> Drop for Crossterm<PainterT> {
    fn drop(&mut self) {
        queue!(
            self.target,
            crossterm::style::ResetColor,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::Show,
            crossterm::terminal::LeaveAlternateScreen
        )
        .expect("Failed to clear screen when closing `crossterm` frontend.");
        crossterm::terminal::disable_raw_mode()
            .expect("Failed to disable raw mode when closing `crossterm` frontend.");
    }
}

#[inline]
fn initialise_tty<PainterT: Painter, TargetT: Write>(target: &mut TargetT) -> Result<()> {
    target
        .queue(crossterm::terminal::EnterAlternateScreen)?
        .queue(crossterm::cursor::Hide)?;
    crossterm::terminal::enable_raw_mode()?;
    queue_set_style(target, &PainterT::INITIAL_STYLE)?;
    target.flush()?;
    Ok(())
}

#[inline]
fn queue_set_style(target: &mut impl Write, style: &Style) -> Result<()> {
    use crossterm::style::{
        Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor,
    };

    // Bold
    if style.bold {
        queue!(target, SetAttribute(Attribute::Bold))?;
    } else {
        // Using Reset is not ideal as it resets all style attributes. The correct thing to do
        // would be to use `NoBold`, but it seems this is not reliably supported (at least it
        // didn't work for me in tmux, although it does in alacritty).
        // Also see https://github.com/crossterm-rs/crossterm/issues/294
        queue!(target, SetAttribute(Attribute::Reset))?;
    }

    // Underline
    if style.underline {
        queue!(target, SetAttribute(Attribute::Underlined))?;
    } else {
        queue!(target, SetAttribute(Attribute::NoUnderline))?;
    }

    // Background
    {
        let Colour { red, green, blue } = style.background;
        queue!(
            target,
            SetBackgroundColor(Color::Rgb {
                r: red,
                g: green,
                b: blue
            })
        )?;
    }

    // Foreground
    {
        let Colour { red, green, blue } = style.foreground;
        queue!(
            target,
            SetForegroundColor(Color::Rgb {
                r: red,
                g: green,
                b: blue
            })
        )?;
    }

    Ok(())
}

#[inline]
fn new_event_stream() -> <Crossterm as Frontend>::EventStream {
    Box::pin(
        crossterm::event::EventStream::new()
            .filter_map(|event| async move {
                match event {
                    Ok(crossterm::event::Event::Key(key_event)) => {
                        Some(Ok(Event::Key(map_key(key_event))))
                    }
                    Ok(crossterm::event::Event::Resize(width, height)) => Some(Ok(Event::Resize(
                        Size::new(width as usize, height as usize),
                    ))),
                    Ok(_) => None,
                    Err(error) => Some(Err(error.into())),
                }
            })
            .fuse(),
    )
}

#[inline]
fn map_key(key: crossterm::event::KeyEvent) -> Key {
    use crossterm::event::{KeyCode, KeyModifiers};
    match key.code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::BackTab => Key::BackTab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Insert => Key::Insert,
        KeyCode::F(u8) => Key::F(u8),
        KeyCode::Null => Key::Null,
        KeyCode::Esc => Key::Esc,
        KeyCode::Char(char) if key.modifiers.contains(KeyModifiers::CONTROL) => Key::Ctrl(char),
        KeyCode::Char(char) if key.modifiers.contains(KeyModifiers::ALT) => Key::Alt(char),
        KeyCode::Char(char) => Key::Char(char),
        KeyCode::Enter => Key::Char('\n'),
        KeyCode::Tab => Key::Char('\t'),
    }
}
