use ignore;
use std::{
    error::Error as StdError,
    fmt::{self, Display},
    io,
    result::Result as StdResult,
};
use tree_sitter::LanguageError;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum Error {
    Config(String),
    Editor(io::Error),
    Io(io::Error),
    FilePicker(ignore::Error),
    TaskPool(Box<dyn StdError + Send>),
    CancelledLanguageParser,
    MissingLanguageParser(String),
    IncompatibleLanguageGrammar(LanguageError),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        // .expect("Compatible tree sitter grammer version");

        // match self {

        // }
        write!(formatter, "{:?}", self)
    }
}

impl StdError for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ignore::Error> for Error {
    fn from(error: ignore::Error) -> Self {
        Self::FilePicker(error)
    }
}
