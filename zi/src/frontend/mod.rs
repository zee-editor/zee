#[cfg(feature = "frontend-crossterm")]
pub mod crossterm;
#[cfg(feature = "frontend-crossterm")]
pub use self::crossterm::Crossterm;

pub mod painter;

mod utils;

use futures::Stream;
use std::io;
use thiserror::Error;

use crate::terminal::{Canvas, Key, Size};

/// A trait for frontends that draw a [`Canvas`](../terminal/struct.Canvas.html) to the terminal.
pub trait Frontend {
    type EventStream: Stream<Item = Result<Event>> + Unpin;

    /// Initialises the underlying terminal.
    ///
    /// Typically hides the cursor and enters an "alternative screen mode" in
    /// order to restore the previous terminal content on exit.
    fn initialise(&mut self) -> Result<()>;

    /// Returns the size of the underlying terminal.
    fn size(&self) -> Result<Size>;

    /// Draws the [`Canvas`](../terminal/struct.Canvas.html) to the terminal.
    fn present(&mut self, canvas: &Canvas) -> Result<usize>;

    /// Returns a stream with user input events.
    fn event_stream(&mut self) -> &mut Self::EventStream;
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Event {
    Key(Key),
    Resize(Size),
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    UnknownFrontend(String),

    #[cfg(feature = "frontend-crossterm")]
    #[error(transparent)]
    Crossterm(#[from] crossterm::Error),

    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(feature = "frontend-crossterm")]
pub fn default() -> Result<crossterm::Crossterm> {
    crossterm::Crossterm::new()
}
