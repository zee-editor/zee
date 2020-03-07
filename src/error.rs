use ignore;
use std::io;
use thiserror::Error;
use tree_sitter::LanguageError;

use crate::frontend;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("{0}")]
    Frontend(#[from] frontend::Error),

    #[error("{0}")]
    FilePicker(#[from] ignore::Error),

    #[error("task error: {0}")]
    TaskPool(Box<dyn std::error::Error + Send>),

    #[error("incompatible language grammar `{0}`")]
    IncompatibleLanguageGrammar(LanguageError),

    #[error(transparent)]
    Editor(#[from] anyhow::Error), // source and Display delegate to anyhow::Error
}
