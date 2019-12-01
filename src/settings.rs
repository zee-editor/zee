use dirs;
use std::{fs, path::PathBuf};

use crate::error::{Error, Result};

pub struct Paths {
    pub settings: PathBuf,
}

pub fn find(config: Option<PathBuf>) -> Result<Paths> {
    let config_path = config.map(Ok).unwrap_or_else(|| {
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
    fs::create_dir_all(&settings).map_err(|err| {
        Error::Config(format!(
            "Could not create config directory {}: {}",
            config_path.display(),
            err
        ))
    })?;
    Ok(Paths { settings })
}
