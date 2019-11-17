use dirs;
use std::{fs, path::PathBuf};

use crate::error::{Error, Result};

pub struct Paths {
    pub settings: PathBuf,
    pub syntax_definitions: PathBuf,
    pub syntax_themes: PathBuf,
}

pub fn find(config: Option<PathBuf>) -> Result<Paths> {
    let mut config_path = config.map(Ok).unwrap_or_else(|| {
        dirs::config_dir()
            .ok_or::<Error>(Error::Config(
                "Could not get path to the user's config directory".into(),
            ))
            .map(|mut config_dir| {
                config_dir.push("zee");
                config_dir
            })
    })?;

    let settings = {
        let mut path = config_path.clone();
        path.push("settings.toml");
        path
    };

    config_path.push("syntect");
    let syntax_definitions = {
        let mut path = config_path.clone();
        path.push("syntax");
        path
    };
    let syntax_themes = {
        let mut path = config_path.clone();
        path.push("themes");
        path
    };

    fs::create_dir_all(&syntax_definitions).map_err(|err| {
        Error::Config(format!(
            "Could not create config directory {}: {}",
            config_path.display(),
            err
        ))
    })?;
    fs::create_dir_all(&syntax_themes).map_err(|err| {
        Error::Config(format!(
            "Could not create config directory {}: {}",
            config_path.display(),
            err
        ))
    })?;

    Ok(Paths {
        settings,
        syntax_definitions,
        syntax_themes,
    })
}
