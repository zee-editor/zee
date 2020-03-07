#[cfg(feature = "frontend-termion")]
pub mod termion;

#[cfg(feature = "frontend-crossterm")]
pub mod crossterm;

use crossbeam_channel::Receiver;
use std::str::FromStr;
use thiserror::Error;

use crate::terminal::{Key, Screen, Size};

#[cfg(feature = "frontend-termion")]
pub const DEFAULT_FRONTEND_STR: &str = "termion";

#[cfg(all(not(feature = "frontend-termion"), feature = "frontend-crossterm"))]
pub const DEFAULT_FRONTEND_STR: &str = "crossterm";

pub trait Frontend {
    fn size(&self) -> Result<Size>;

    fn present(&mut self, screen: &Screen) -> Result<()>;

    fn events(&self) -> &Receiver<Key>;
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    UnknownFrontend(String),

    #[cfg(feature = "frontend-termion")]
    #[error(transparent)]
    Termion(#[from] termion::Error),

    #[cfg(feature = "frontend-crossterm")]
    #[error(transparent)]
    Crossterm(#[from] crossterm::Error),
}

#[derive(Debug)]
pub enum FrontendKind {
    #[cfg(feature = "frontend-termion")]
    Termion,

    #[cfg(feature = "frontend-crossterm")]
    Crossterm,
}

impl FromStr for FrontendKind {
    type Err = Error;

    fn from_str(frontend_str: &str) -> Result<Self> {
        match frontend_str {
            #[cfg(feature = "frontend-termion")]
            "termion" => Ok(FrontendKind::Termion),

            #[cfg(feature = "frontend-crossterm")]
            "crossterm" => Ok(FrontendKind::Crossterm),

            _ => Err(Error::UnknownFrontend(frontend_str.into())),
        }
    }
}
