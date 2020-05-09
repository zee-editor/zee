use std::io;
use thiserror::Error;

use crate::frontend;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Frontend(#[from] frontend::Error),

    #[error("task error: {0}")]
    TaskPool(Box<dyn std::error::Error>),
}
