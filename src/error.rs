use std::{
    error::Error as StdError,
    fmt::{self, Display},
    io,
    result::Result as StdResult,
};

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum Error {
    Config(String),
    Editor(io::Error),
    MissingSyntectTheme(String),
    Io(io::Error),
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{:?}", self)
    }
}

impl StdError for Error {}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
