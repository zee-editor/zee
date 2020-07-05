use thiserror::Error;

use crate::frontend;

pub type Result<T> = std::result::Result<T, Error>;

/// A representation of a runtime error encountered by Zi.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Frontend(#[from] frontend::Error),

    #[error("Tokio error: {0}")]
    Tokio(#[from] tokio::io::Error),
}
