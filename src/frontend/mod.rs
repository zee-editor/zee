pub mod termion;

#[cfg(feature = "frontend-crossterm")]
pub mod crossterm;

use crossbeam_channel::Receiver;
use thiserror::Error;

pub use self::termion::Termion;

use crate::terminal::{Key, Screen, Size};

pub trait Frontend {
    fn size(&self) -> Result<Size>;

    fn present(&mut self, screen: &Screen) -> Result<()>;

    fn events(&self) -> &Receiver<Key>;
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Termion(#[from] termion::Error),

    #[cfg(feature = "frontend-crossterm")]
    #[error(transparent)]
    Crossterm(#[from] crossterm::Error),
}
