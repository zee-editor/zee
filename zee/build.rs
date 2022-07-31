use anyhow::Result;
use include_dir::{include_dir, Dir};
use serde_derive::Deserialize;
use std::fs::File;
use zee_grammar::config::ModeConfig;

static DEFAULT_CONFIG_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config");

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename = "Zee")]
pub struct EditorConfig {
    #[serde(default)]
    pub theme: String,
    pub indentation_override: Option<zee_grammar::config::IndentationConfig>,
    pub modes: Vec<ModeConfig>,
    #[serde(default)]
    pub trim_trailing_whitespace_on_save: bool,
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=config");

    if std::env::var("ZEE_DISABLE_GRAMMAR_BUILD").is_err() {
        let config: EditorConfig = ron::de::from_reader(File::open("config/config.ron")?)?;
        zee_grammar::builder::fetch_and_build_tree_sitter_parsers(
            &config.modes,
            &DEFAULT_CONFIG_DIR,
        )?;
    }

    Ok(())
}
