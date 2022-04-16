use serde_derive::Deserialize;
use std::{ffi::OsStr, path::Path};

use crate::config::Grammar;

#[derive(Debug, Deserialize)]
pub struct Mode {
    pub name: String,
    pub patterns: Vec<FilenamePattern>,
    pub grammar: Option<Grammar>,
}

#[derive(Debug, Deserialize)]
pub enum FilenamePattern {
    Suffix(String),
    Name(String),
}

impl FilenamePattern {
    pub fn suffix(suffix: impl Into<String>) -> Self {
        Self::Suffix(suffix.into())
    }

    pub fn name(suffix: impl Into<String>) -> Self {
        Self::Name(suffix.into())
    }

    pub fn matches(&self, filename: impl AsRef<Path>) -> bool {
        match self {
            Self::Suffix(ref suffix) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s.ends_with(suffix))
                .unwrap_or(false),
            Self::Name(ref expected_name) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s == expected_name.as_str())
                .unwrap_or(false),
        }
    }
}
