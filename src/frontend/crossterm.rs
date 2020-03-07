use crossbeam_channel::{self, Receiver};
use crossterm::{self, queue, QueueableCommand};
use std::{
    io::{self, BufWriter, Stdout, Write},
    thread::{self, JoinHandle},
};

use super::{Frontend, Result};
use crate::terminal::{screen::Textel, Colour, Key, Screen, Size, Style};

pub type Error = crossterm::ErrorKind;

pub struct Crossterm {
    target: BufWriter<Stdout>,
    input: Input,
}

impl Crossterm {
    pub fn new() -> Result<Self> {
        let mut target = BufWriter::with_capacity(1 << 20, io::stdout());
        target
            .queue(crossterm::terminal::EnterAlternateScreen)?
            .queue(crossterm::cursor::Hide)?;
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self {
            target,
            input: Input::new(),
        })
    }
}

impl Frontend for Crossterm {
    #[inline]
    fn size(&self) -> Result<Size> {
        let (width, height) = crossterm::terminal::size()?;
        Ok(Size::new(width as usize, height as usize))
    }

    #[inline]
    fn present(&mut self, screen: &Screen) -> Result<()> {
        let Self { ref mut target, .. } = *self;

        let mut last_style = Style::default();
        queue_set_style(target, &last_style)?;

        screen
            .buffer()
            .chunks(screen.size().width)
            .enumerate()
            .try_for_each(|(y, line)| {
                // Go to the begining of line (`MoveTo` uses 0-based indexing)
                queue!(target, crossterm::cursor::MoveTo(0, y as u16))?;

                line.iter().try_for_each(|textel| -> Result<()> {
                    if let Some(Textel {
                        ref style,
                        ref content,
                    }) = textel
                    {
                        if *style != last_style {
                            queue_set_style(target, &style)?;
                            last_style = *style;
                        }
                        queue!(target, crossterm::style::Print(content))?;
                    }
                    Ok(())
                })
            })?;

        target.flush().map_err(Error::from)?;
        Ok(())
    }

    #[inline]
    fn events(&self) -> &Receiver<Key> {
        &self.input.receiver
    }
}

impl Drop for Crossterm {
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
        let Colour { red, green, blue } = style.background.0;
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
        let Colour { red, green, blue } = style.foreground.0;
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

struct Input {
    receiver: Receiver<Key>,
    _handle: JoinHandle<()>,
}

impl Input {
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(2048);
        let event_loop = move || loop {
            match crossterm::event::read() {
                Ok(crossterm::event::Event::Key(key_event)) => {
                    sender.send(map_key(key_event)).unwrap();
                }
                Ok(_) => {}
                error => {
                    error.unwrap();
                }
            }
        };
        Self {
            receiver,
            _handle: thread::spawn(event_loop),
        }
    }
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
