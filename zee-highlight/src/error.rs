use std::fmt::{self, Display};

// type NomError = nom::Err<nom::error::ErrorKind>;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum Error {
    SelectorSyntax,
    NodeKindNotFound(String),
    RegexSyntax(regex::Error),
}

impl From<regex::Error> for Error {
    fn from(error: regex::Error) -> Self {
        Self::RegexSyntax(error)
    }
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SelectorSyntax => write!(formatter, "Invalid selector syntax."),
            Self::NodeKindNotFound(ref node_kind) => write!(
                formatter,
                "Node kind `{}` does not exist in the supplied language.",
                node_kind
            ),
            Self::RegexSyntax(ref error) => write!(formatter, "Invalid regex syntax: {}", error),
        }
    }
}
