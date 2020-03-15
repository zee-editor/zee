use dirs;
use serde_derive::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::error::{Error, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub theme_index: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme_index: 0 }
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let mut path = dirs::config_dir()
        .ok_or_else(|| Error::Config("Could not get path to the user's config directory".into()))
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
            .map_err(|err| {
                Error::Config(format!(
                    "Could not read settings file `{}`: {}",
                    path.as_ref().display(),
                    err
                ))
            })
            .and_then(|contents| {
                log::info!("Reading settings file `{}`", path.as_ref().display());
                toml::de::from_str(&contents).map_err(|err| {
                    Error::Config(format!(
                        "Could not parse settings file `{}`: {}",
                        path.as_ref().display(),
                        err
                    ))
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
        fs::create_dir_all(parent).map_err(|err| {
            Error::Config(format!(
                "Could not create config directory {}: {}",
                parent.display(),
                err
            ))
        })?;
    }
    let settings_str = toml::to_string_pretty(&Settings::default()).map_err(|err| {
        Error::Config(format!(
            "Could not serialize settings to file `{}`: {}",
            path.as_ref().display(),
            err
        ))
    })?;
    File::create(path.as_ref())
        .and_then(|mut file| file.write(settings_str.as_bytes()))
        .map_err(|err| {
            Error::Config(format!(
                "Could not read settings file `{}`: {}",
                path.as_ref().display(),
                err
            ))
        })?;

    Ok(())
}
