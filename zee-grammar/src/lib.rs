pub mod builder;
pub mod config;

mod git;

use anyhow::Result;
use once_cell::sync::Lazy;
use std::path::Path;
use tree_sitter::{Language, Query};

use self::config::{CommentConfig, FilenamePattern, IndentationConfig, ModeConfig};

#[derive(Debug)]
pub struct Mode {
    pub name: String,
    pub scope: String,
    pub injection_regex: String,
    pub patterns: Vec<FilenamePattern>,
    pub comment: Option<CommentConfig>,
    pub indentation: IndentationConfig,
    grammar: LazyGrammar,
}

impl Mode {
    pub fn new(config: ModeConfig) -> Self {
        let ModeConfig {
            name,
            scope,
            injection_regex,
            patterns,
            comment,
            indentation,
            grammar: grammar_config,
        } = config;
        Self {
            name,
            scope,
            injection_regex,
            patterns,
            comment,
            indentation,
            grammar: Lazy::new(Box::new(move || {
                grammar_config
                    .map(|grammar_config| grammar_config.grammar_id)
                    .map(builder::load_grammar)
            })),
        }
    }

    pub fn matches_by_filename(&self, filename: impl AsRef<Path>) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(filename.as_ref()))
    }

    pub fn language(&self) -> Option<Result<Language, &anyhow::Error>> {
        Some(self.grammar()?.map(|parser| parser.language))
    }

    pub fn grammar(&self) -> Option<std::result::Result<&Grammar, &anyhow::Error>> {
        Lazy::force(&self.grammar)
            .as_ref()
            .map(|result| result.as_ref())
    }
}

impl Default for Mode {
    fn default() -> Self {
        Self {
            name: "Plain".into(),
            scope: "plaintext".into(),
            injection_regex: "".into(),
            patterns: vec![],
            comment: None,
            indentation: Default::default(),
            grammar: Lazy::new(Box::new(|| None)),
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

type LazyGrammar = Lazy<
    Option<Result<Grammar>>,
    Box<dyn FnOnce() -> Option<Result<Grammar>> + Send + Sync + 'static>,
>;
