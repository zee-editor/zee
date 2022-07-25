use include_dir::{include_dir, Dir};
use once_cell::sync::Lazy;
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

use zee_grammar::{config::ModeConfig, Mode};

use crate::{
    error::{Context, Result},
    utils,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename = "Zee")]
pub struct EditorConfig {
    #[serde(default)]
    pub theme: String,
    pub modes: Vec<ModeConfig>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        DEFAULT_EDITOR_CONFIG.clone()
    }
}

/// Finds the editor configuration. If we cannot for any reason, we'll use the
/// default configuration to ensure the editor opens in any environment.
pub fn find_editor_config(config_dir: Option<PathBuf>) -> EditorConfig {
    let config_dir = config_dir
        .or_else(|| zee_grammar::config::config_dir().ok())
        .ok_or_else(|| anyhow::Error::msg("Unable to determine configuration directory"))
        .ok();

    // First construct the `EditorConfig` from the configuration file
    let mut editor_config = config_dir
        .as_ref()
        .map(|config_dir| config_dir.join("config.ron"))
        .map_or_else(Default::default, |path| read_config_file(&path));

    // Second, load modes from the modes directory (if available)
    if let Some(config_dir) = config_dir {
        let modes_dir = config_dir.join("modes");
        if let Ok(mut additional_modes) = load_modes_from_dir(&modes_dir) {
            editor_config.modes.append(&mut additional_modes);
        }
    }

    editor_config
}

fn read_config_file(path: &Path) -> EditorConfig {
    if path.exists() {
        std::fs::read_to_string(path)
            .with_context(|| format!("Could not read configuration file `{}`", path.display()))
            .and_then(|contents| {
                log::info!("Reading configuration file `{}`", path.display());
                ron::de::from_str(&contents).with_context(|| {
                    format!("Could not parse configuration file `{}`", path.display())
                })
            })
            .map_err(|err| log::error!("{}", err))
            .unwrap_or_else(|_| Default::default())
    } else {
        Default::default()
    }
}

fn load_modes_from_dir(dir: &PathBuf) -> Result<Vec<ModeConfig>> {
    if !dir.exists() {
        if let Err(e) = std::fs::create_dir(dir) {
            log::warn!(
                "Unable to create modes configuration directory `{}`. {}",
                dir.display(),
                e
            );
        }
    }

    let mut modes = vec![];
    for mode_file in utils::files_with_extension(dir, "ron")? {
        match File::open(&mode_file) {
            Ok(handle) => match ron::de::from_reader(handle) {
                Ok(mode_cfg) => modes.push(mode_cfg),
                Err(e) => log::warn!(
                    "Unable to parse configuration file `{}`: {}",
                    mode_file.display(),
                    e
                ),
            },
            Err(e) => log::warn!(
                "Unable to open configuration file `{}`: {}",
                &mode_file.display(),
                e
            ),
        };
    }
    Ok(modes)
}

pub fn create_default_config_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Could not create config directory `{}`", parent.display()))?;
    }
    std::fs::write(path, default_config_str().as_bytes())
        .with_context(|| format!("Could not write config file to `{}`", path.display()))?;
    Ok(())
}

pub fn default_config_str() -> &'static str {
    let config_file = DEFAULT_CONFIG_DIR
        .get_file("config.ron")
        .expect("missing packaged default configuration file config.ron");
    config_file
        .contents_utf8()
        .expect("mode configuration file is not valid utf-8")
}

pub static PLAIN_TEXT_MODE: Lazy<Mode> = Lazy::new(Default::default);

pub static DEFAULT_CONFIG_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/config");
static DEFAULT_EDITOR_CONFIG: Lazy<EditorConfig> = Lazy::new(|| {
    ron::de::from_str(default_config_str())
        .expect("packaged default configuration file is well formed")
});
