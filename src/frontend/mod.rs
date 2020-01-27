pub mod termion;
pub use self::termion::Termion;
use std::fmt::{self, Display};

#[cfg(feature = "frontend-crossterm")]
pub mod crossterm;

use crate::terminal::{Key, Screen, Size};
use crossbeam_channel::Receiver;

pub trait Frontend {
    fn size(&self) -> Result<Size>;

    fn present(&mut self, screen: &Screen) -> Result<()>;

    fn events(&self) -> &Receiver<Key>;
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Termion(termion::Error),

    #[cfg(feature = "frontend-crossterm")]
    Crossterm(crossterm::Error),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self)
    }
}

impl From<termion::Error> for Error {
    fn from(error: termion::Error) -> Self {
        Self::Termion(error)
    }
}

impl From<crossterm::Error> for Error {
    fn from(error: crossterm::Error) -> Self {
        Self::Crossterm(error)
    }
}
