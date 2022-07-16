pub mod builder;
pub mod config;

mod git;

use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;
use tree_sitter::{Language, Query};

use self::config::{CommentConfig, FilenamePattern, IndentationConfig};

static SHEBANG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^#!\s*(?:\S*[/\\](?:env\s+(?:\-\S+\s+)*)?)?([^\s\.\d]+)").unwrap());

#[derive(Debug)]
pub struct Mode {
    pub name: String,
    pub scope: String,
    pub injection_regex: String,
    pub patterns: Vec<FilenamePattern>,
    pub shebangs: Vec<String>,
    pub comment: Option<CommentConfig>,
    pub indentation: IndentationConfig,
    pub grammar: Option<Grammar>,
}

impl Mode {
    pub fn language(&self) -> Option<Language> {
        self.grammar.as_ref().map(|parser| parser.language)
    }

    pub fn matches_by_filename(&self, filename: impl AsRef<Path>) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(filename.as_ref()))
    }

    pub fn matches_by_shebang(&self, shebang: &str) -> bool {
        SHEBANG_REGEX
            .captures(shebang)
            .and_then(|captures| self.shebangs.contains(&captures[1].into()).then(|| 0))
            .is_some()
    }

}

impl Default for Mode {
    fn default() -> Self {
        Self {
            name: "Plain".into(),
            scope: "plaintext".into(),
            injection_regex: "".into(),
            patterns: vec![],
            shebangs: vec![],
            comment: None,
            indentation: Default::default(),
            grammar: None,
        }
    }
}

#[derive(Debug)]
pub struct Grammar {
    pub id: String,
    pub language: Language,
    pub highlights: Option<Query>,
    pub indents: Option<Query>,
    pub injections: Option<Query>,
    pub locals: Option<Query>,
}

impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
