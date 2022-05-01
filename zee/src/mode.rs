use include_dir::{include_dir, Dir};
use once_cell::sync::Lazy;
use std::path::Path;

use zee_grammar::{self as grammar, config::ModeConfig, Mode};

pub fn find_by_filename(filename: impl AsRef<Path>) -> &'static Mode {
    DEFAULT_LANGUAGE_MODES
        .iter()
        .find(|&mode| mode.matches_by_filename(filename.as_ref()))
        .unwrap_or(&PLAIN_TEXT_MODE)
}

pub fn default_modes_config() -> Vec<ModeConfig> {
    let config_file = DEFAULT_CONFIG_DIR
        .get_file("modes.ron")
        .expect("missing packaged default config file modes.ron");
    let mode_configs: Vec<ModeConfig> = ron::de::from_str(
        config_file
            .contents_utf8()
            .expect("mode configuration file is not valid utf-8"),
    )
    .expect("mode configuration file is well formed");
    mode_configs
}

pub static PLAIN_TEXT_MODE: Lazy<Mode> = Lazy::new(Default::default);

pub static DEFAULT_CONFIG_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config");
static DEFAULT_LANGUAGE_MODES: Lazy<Vec<Mode>> = Lazy::new(|| {
    default_modes_config()
        .into_iter()
        .filter_map(|config| grammar::builder::load_mode(config).ok())
        .collect()
});
