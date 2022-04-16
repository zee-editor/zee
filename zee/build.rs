use anyhow::Result;
use std::fs::File;
use zee_grammar::mode::Mode as ModeConfig;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=modes.ron");

    if std::env::var("ZEE_DISABLE_GRAMMAR_BUILD").is_err() {
        let mode_configs: Vec<ModeConfig> = ron::de::from_reader(File::open("modes.ron")?)?;
        println!("cargo:rerun-if-changed=modes.ron");
        zee_grammar::builder::fetch_and_build_tree_sitter_parsers(&mode_configs)?;
    }

    Ok(())
}
