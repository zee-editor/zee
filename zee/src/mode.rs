use once_cell::sync::Lazy;
use std::{collections::hash_map::HashMap, fmt, path::Path, ptr};
use tree_sitter::Language;
use zee_grammar::{
    self as grammar,
    mode::{FilenamePattern, Mode as ModeConfig},
};
use zee_highlight::HighlightRules;

pub struct Mode {
    pub name: String,
    pub parser: Option<SyntaxParser>,
    file: Vec<FilenamePattern>,
}

impl Mode {
    pub fn language(&self) -> Option<Language> {
        self.parser.as_ref().map(|parser| parser.language)
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

pub fn find_by_filename(filename: impl AsRef<Path>) -> &'static Mode {
    LANGUAGE_MODES
        .iter()
        .find(|&mode| mode.matches_by_filename(filename.as_ref()))
        .unwrap_or(&PLAIN_TEXT_MODE)
}

pub static PLAIN_TEXT_MODE: Lazy<Mode> = Lazy::new(Default::default);

pub const MODES_CONFIG_STR: &str = include_str!("../modes.ron");

static LANGUAGE_MODES: Lazy<Vec<Mode>> = Lazy::new(|| {
    let mode_configs: Vec<ModeConfig> =
        ron::de::from_str(MODES_CONFIG_STR).expect("mode configuration file is well formed");
    mode_configs
        .into_iter()
        .map(|config| Mode {
            name: config.name,
            file: config.patterns,
            parser: config.grammar.and_then(|grammar_config| {
                let language = grammar::builder::load_language(&grammar_config.grammar_id).ok()?;
                let rules_str = HIGHLIGHT_RULES.get(grammar_config.grammar_id.as_str())?;
                let highlights = zee_highlight::parse_rules_unwrap(language, rules_str);
                Some(SyntaxParser {
                    language,
                    highlights,
                })
            }),
        })
        .collect()
});

static HIGHLIGHT_RULES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("rust", include_str!("../highlights/rust.json"));
    map.insert("json", include_str!("../highlights/json.json"));
    map.insert("python", include_str!("../highlights/python.json"));
    map.insert("html", include_str!("../highlights/html.json"));
    map.insert("markdown", include_str!("../highlights/markdown.json"));
    map.insert("bash", include_str!("../highlights/bash.json"));
    map.insert("c", include_str!("../highlights/c.json"));
    map.insert("cpp", include_str!("../highlights/cpp.json"));
    map.insert("css", include_str!("../highlights/css.json"));
    map.insert("javascript", include_str!("../highlights/javascript.json"));
    map.insert("typescript", include_str!("../highlights/typescript.json"));
    map.insert("tsx", include_str!("../highlights/tsx.json"));
    map
});
