use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grammar {
    #[serde(rename = "id")]
    pub grammar_id: String,
    pub source: GrammarSource,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GrammarSource {
    Local {
        path: PathBuf,
    },
    Git {
        #[serde(rename = "git")]
        remote: String,
        #[serde(rename = "rev")]
        revision: String,
        path: Option<PathBuf>,
    },
}

pub fn config_dir() -> Result<PathBuf> {
    dirs::config_dir()
        .map(|mut config_dir| {
            config_dir.push("zee");
            config_dir
        })
        .context("Could not get path to the user's config directory")
}

pub fn runtime_dir() -> Result<PathBuf> {
    if let Ok(env_dir) = std::env::var("ZEE_RUNTIME") {
        return Ok(env_dir.into());
    }

    if let Ok(cargo_manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        // this is the root directory of the crate, we take the parent to get the workspace path
        return PathBuf::from(cargo_manifest_dir)
            .parent()
            .map(|path| path.join(RUNTIME_DIR))
            .context("Could not get parent path of CARGO_MANIFEST_DIR");
    }

    if let Ok(config_dir) = config_dir() {
        return Ok(config_dir.join(RUNTIME_DIR));
    }

    // fallback to location of the executable being run
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|path| path.to_path_buf().join(RUNTIME_DIR))
        })
        .context("Could not get the path of the current executable")
}

const RUNTIME_DIR: &str = "runtime";
