use lazy_static::lazy_static;
use std::{ffi::OsStr, path::Path};
use tree_sitter::Language;
use zee_grammar as grammar;

use crate::smallstring::SmallString;

pub struct Mode {
    pub name: SmallString,
    file: Vec<FilenamePattern>,
    pub language: Option<Language>,
}

impl Mode {
    fn matches_by_filename(&self, filename: impl AsRef<Path>) -> bool {
        self.file
            .iter()
            .find(|pattern| pattern.matches(filename.as_ref()))
            .is_some()
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode {
            name: "Plain".into(),
            file: vec![],
            language: None,
        }
    }
}

enum FilenamePattern {
    Suffix(String),
    Name(String),
}

impl FilenamePattern {
    fn suffix(suffix: impl Into<String>) -> Self {
        Self::Suffix(suffix.into())
    }

    fn name(suffix: impl Into<String>) -> Self {
        Self::Suffix(suffix.into())
    }

    fn matches(&self, filename: impl AsRef<Path>) -> bool {
        match self {
            &Self::Suffix(ref suffix) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s.ends_with(suffix))
                .unwrap_or(false),
            &Self::Name(ref expected_name) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s == expected_name.as_str())
                .unwrap_or(false),
        }
    }
}

pub fn find_by_filename(filename: impl AsRef<Path>) -> &'static Mode {
    LANGUAGE_MODES
        .iter()
        .find(|&mode| mode.matches_by_filename(filename.as_ref()))
        .unwrap_or(&PLAIN_TEXT_MODE)
}

lazy_static! {
    pub static ref LANGUAGE_MODES: [Mode; 11] = [
        Mode {
            name: "Rust".into(),
            file: vec![FilenamePattern::suffix(".rs")],
            language: Some(*grammar::RUST),
        },
        Mode {
            name: "Python".into(),
            file: vec![
                FilenamePattern::suffix(".py"),
                FilenamePattern::suffix(".py3"),
                FilenamePattern::suffix(".py2"),
                FilenamePattern::suffix(".pyi"),
                FilenamePattern::suffix(".pyx"),
                FilenamePattern::suffix(".pyx.in"),
                FilenamePattern::suffix(".pxd"),
                FilenamePattern::suffix(".pxd.in"),
                FilenamePattern::suffix(".pxi"),
                FilenamePattern::suffix(".pxi.in"),
                FilenamePattern::suffix(".rpy"),
                FilenamePattern::suffix(".cpy"),
            ],
            language: Some(*grammar::PYTHON),
        },
        Mode {
            name: "Javascript".into(),
            file: vec![FilenamePattern::suffix(".js")],
            language: Some(*grammar::JAVASCRIPT),
        },
        Mode {
            name: "HTML".into(),
            file: vec![
                FilenamePattern::suffix(".html"),
                FilenamePattern::suffix(".htm"),
                FilenamePattern::suffix(".xhtml"),
                FilenamePattern::suffix(".shtml"),
            ],
            language: Some(*grammar::HTML),
        },
        Mode {
            name: "JSON".into(),
            file: vec![
                FilenamePattern::suffix(".json"),
                FilenamePattern::suffix(".jsonl"),
            ],
            language: Some(*grammar::JSON),
        },
        Mode {
            name: "C".into(),
            file: vec![FilenamePattern::suffix(".c"), FilenamePattern::suffix(".h")],
            language: Some(*grammar::C),
        },
        Mode {
            name: "CPP".into(),
            file: vec![
                FilenamePattern::suffix(".cpp"),
                FilenamePattern::suffix(".cc"),
                FilenamePattern::suffix(".cp"),
                FilenamePattern::suffix(".cxx"),
                FilenamePattern::suffix(".c++"),
                FilenamePattern::suffix(".C"),
                FilenamePattern::suffix(".h"),
                FilenamePattern::suffix(".hh"),
                FilenamePattern::suffix(".hpp"),
                FilenamePattern::suffix(".hxx"),
                FilenamePattern::suffix(".h++"),
                FilenamePattern::suffix(".inl"),
                FilenamePattern::suffix(".ipp"),
            ],
            language: Some(*grammar::CPP),
        },
        Mode {
            name: "CSS".into(),
            file: vec![FilenamePattern::suffix(".css"),],
            language: Some(*grammar::CSS),
        },
        Mode {
            name: "Markdown".into(),
            file: vec![FilenamePattern::suffix(".md"),],
            language: Some(*grammar::MARKDOWN),
        },
        Mode {
            name: "Typescript".into(),
            file: vec![FilenamePattern::suffix(".ts"),],
            language: Some(*grammar::TYPESCRIPT),
        },
        Mode {
            name: "Typescript TSX".into(),
            file: vec![FilenamePattern::suffix(".tsx"),],
            language: Some(*grammar::TSX),
        }
    ];
    pub static ref PLAIN_TEXT_MODE: Mode = Default::default();
}
