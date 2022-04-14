use anyhow::anyhow;
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::error::{Context, Result};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    pub theme_index: usize,
}

pub fn settings_path() -> Result<PathBuf> {
    let mut path = dirs::config_dir()
        .ok_or_else(|| anyhow!("Could not get path to the user's config directory"))
        .map(|mut config_dir| {
            config_dir.push("zee");
            config_dir
        })?;
    path.push("settings.toml");
    Ok(path)
}

pub fn read_settings(path: impl AsRef<Path>) -> Settings {
    if path.as_ref().exists() {
        File::open(path.as_ref())
            .and_then(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                Ok(contents)
            })
            .with_context(|| format!("Could not read settings file `{}`", path.as_ref().display()))
            .and_then(|contents| {
                log::info!("Reading settings file `{}`", path.as_ref().display());
                toml::de::from_str(&contents).with_context(|| {
                    format!(
                        "Could not parse settings file `{}`",
                        path.as_ref().display(),
                    )
                })
            })
            .map_err(|err| log::error!("{}", err))
            .unwrap_or_else(|_| Default::default())
    } else {
        Default::default()
    }
}

pub fn create_default_file(path: impl AsRef<Path>) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Could not create config directory `{}`", parent.display(),)
        })?;
    }
    let settings_str = toml::to_string_pretty(&Settings::default()).with_context(|| {
        format!(
            "Could not serialize settings to file `{}`",
            path.as_ref().display()
        )
    })?;
    File::create(path.as_ref())
        .and_then(|mut file| file.write(settings_str.as_bytes()))
        .with_context(|| format!("Could not read settings file `{}`", path.as_ref().display(),))?;

    Ok(())
}
