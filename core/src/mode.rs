use once_cell::sync::Lazy;
use std::{ffi::OsStr, fmt, path::Path, ptr};
use tree_sitter::Language;
use zee_grammar as grammar;
use zee_highlight::{
    HighlightRules, BASH_RULES, CPP_RULES, CSS_RULES, C_RULES, HTML_RULES, JAVASCRIPT_RULES,
    JSON_RULES, MARKDOWN_RULES, PYTHON_RULES, RUST_RULES, TSX_RULES, TYPESCRIPT_RULES,
};

pub struct Mode {
    pub name: String,
    pub parser: Option<SyntaxParser>,
    file: Vec<FilenamePattern>,
}

impl Mode {
    pub fn language(&self) -> Option<&Language> {
        self.parser.as_ref().map(|parser| &parser.language)
    }

    pub fn highlights(&self) -> Option<&HighlightRules> {
        self.parser.as_ref().map(|parser| &parser.highlights)
    }
}

impl PartialEq for Mode {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

impl fmt::Debug for Mode {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .debug_struct("Mode")
            .field("name", &self.name)
            .field(
                "parser",
                &if self.parser.is_some() {
                    "Some(SyntaxParser(...))"
                } else {
                    "None"
                },
            )
            .field("file", &self.file)
            .finish()
    }
}

pub struct SyntaxParser {
    pub language: Language,
    pub highlights: HighlightRules,
}

impl Mode {
    fn matches_by_filename(&self, filename: impl AsRef<Path>) -> bool {
        self.file
            .iter()
            .any(|pattern| pattern.matches(filename.as_ref()))
    }
}

impl Default for Mode {
    fn default() -> Self {
        Mode {
            name: "Plain".into(),
            file: vec![],
            parser: None,
        }
    }
}

#[derive(Debug)]
enum FilenamePattern {
    Suffix(String),
    Name(String),
}

impl FilenamePattern {
    fn suffix(suffix: impl Into<String>) -> Self {
        Self::Suffix(suffix.into())
    }

    fn name(suffix: impl Into<String>) -> Self {
        Self::Name(suffix.into())
    }

    fn matches(&self, filename: impl AsRef<Path>) -> bool {
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

pub fn find_by_filename(filename: impl AsRef<Path>) -> &'static Mode {
    LANGUAGE_MODES
        .iter()
        .find(|&mode| mode.matches_by_filename(filename.as_ref()))
        .unwrap_or(&PLAIN_TEXT_MODE)
}

static LANGUAGE_MODES: Lazy<[Mode; 13]> = Lazy::new(|| {
    [
        Mode {
            name: "Shell Script".into(),
            file: vec![FilenamePattern::suffix(".sh")],
            parser: Some(SyntaxParser {
                language: *grammar::BASH,
                highlights: BASH_RULES.clone(),
            }),
        },
        Mode {
            name: "Rust".into(),
            file: vec![FilenamePattern::suffix(".rs")],
            parser: Some(SyntaxParser {
                language: *grammar::RUST,
                highlights: RUST_RULES.clone(),
            }),
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
            parser: Some(SyntaxParser {
                language: *grammar::PYTHON,
                highlights: PYTHON_RULES.clone(),
            }),
        },
        Mode {
            name: "Javascript".into(),
            file: vec![FilenamePattern::suffix(".js")],
            parser: Some(SyntaxParser {
                language: *grammar::JAVASCRIPT,
                highlights: JAVASCRIPT_RULES.clone(),
            }),
        },
        Mode {
            name: "HTML".into(),
            file: vec![
                FilenamePattern::suffix(".html"),
                FilenamePattern::suffix(".htm"),
                FilenamePattern::suffix(".xhtml"),
                FilenamePattern::suffix(".shtml"),
            ],
            parser: Some(SyntaxParser {
                language: *grammar::HTML,
                highlights: HTML_RULES.clone(),
            }),
        },
        Mode {
            name: "JSON".into(),
            file: vec![
                FilenamePattern::suffix(".json"),
                FilenamePattern::suffix(".jsonl"),
            ],
            parser: Some(SyntaxParser {
                language: *grammar::JSON,
                highlights: JSON_RULES.clone(),
            }),
        },
        Mode {
            name: "C".into(),
            file: vec![FilenamePattern::suffix(".c"), FilenamePattern::suffix(".h")],
            parser: Some(SyntaxParser {
                language: *grammar::C,
                highlights: C_RULES.clone(),
            }),
        },
        Mode {
            name: "C++".into(),
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
            parser: Some(SyntaxParser {
                language: *grammar::CPP,
                highlights: CPP_RULES.clone(),
            }),
        },
        Mode {
            name: "CSS".into(),
            file: vec![FilenamePattern::suffix(".css")],
            parser: Some(SyntaxParser {
                language: *grammar::CSS,
                highlights: CSS_RULES.clone(),
            }),
        },
        Mode {
            name: "Markdown".into(),
            file: vec![FilenamePattern::suffix(".md")],
            parser: Some(SyntaxParser {
                language: *grammar::MARKDOWN,
                highlights: MARKDOWN_RULES.clone(),
            }),
        },
        Mode {
            name: "Typescript".into(),
            file: vec![FilenamePattern::suffix(".ts")],
            parser: Some(SyntaxParser {
                language: *grammar::TYPESCRIPT,
                highlights: TYPESCRIPT_RULES.clone(),
            }),
        },
        Mode {
            name: "Typescript TSX".into(),
            file: vec![FilenamePattern::suffix(".tsx")],
            parser: Some(SyntaxParser {
                language: *grammar::TSX,
                highlights: TSX_RULES.clone(),
            }),
        },
        Mode {
            name: "Dockerfile".into(),
            file: vec![FilenamePattern::name("Dockerfile")],
            parser: None,
        },
    ]
});

pub static PLAIN_TEXT_MODE: Lazy<Mode> = Lazy::new(Default::default);
