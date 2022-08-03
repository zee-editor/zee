use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde_derive::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename = "Mode")]
pub struct ModeConfig {
    pub name: String,
    pub scope: String,
    pub injection_regex: String,
    pub patterns: Vec<FilenamePattern>,
    #[serde(default)]
    pub comment: Option<CommentConfig>,
    pub indentation: IndentationConfig,
    pub grammar: Option<GrammarConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename = "Comment")]
pub struct CommentConfig {
    pub token: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename = "Indentation")]
pub struct IndentationConfig {
    pub width: usize,
    pub unit: IndentationUnit,
}

impl IndentationConfig {
    pub fn to_char(&self) -> char {
        self.unit.to_char()
    }

    pub fn char_count(&self) -> usize {
        match self.unit {
            IndentationUnit::Space => self.width,
            IndentationUnit::Tab => 1,
        }
    }

    pub fn tab_width(&self) -> usize {
        self.width
    }
}

impl Default for IndentationConfig {
    fn default() -> Self {
        Self {
            width: 4,
            unit: IndentationUnit::Space,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum IndentationUnit {
    Space,
    Tab,
}

impl IndentationUnit {
    pub fn to_char(&self) -> char {
        match self {
            Self::Space => ' ',
            Self::Tab => '\t',
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FilenamePattern {
    Suffix(String),
    Name(String),
}

impl FilenamePattern {
    pub fn suffix(suffix: impl Into<String>) -> Self {
        Self::Suffix(suffix.into())
    }

    pub fn name(suffix: impl Into<String>) -> Self {
        Self::Name(suffix.into())
    }

    pub fn matches(&self, filename: impl AsRef<Path>) -> bool {
        match self {
            Self::Suffix(ref suffix) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| {
                    s.ends_with(suffix)
                        || s.ends_with(&(suffix.to_owned() + ".bak"))
                        || s.ends_with(&(suffix.to_owned() + ".in"))
                        || s.ends_with(&(suffix.to_owned() + ".out"))
                })
                .unwrap_or(false),
            Self::Name(ref expected_name) => filename
                .as_ref()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|s| s == expected_name.as_str())
                .unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename = "Grammar")]
pub struct GrammarConfig {
    #[serde(rename = "id")]
    pub grammar_id: String,
    pub source: GrammarSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    if let Ok(env_dir) = std::env::var("ZEE_CONFIG_DIR") {
        return Ok(env_dir.into());
    }

    if let Ok(cargo_manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        // This is the root directory of the crate, we take the parent to get the workspace path
        return PathBuf::from(cargo_manifest_dir)
            .parent()
            .map(|path| path.join("config"))
            .context("Could not get parent path of CARGO_MANIFEST_DIR");
    }

    if let Ok(config_dir) = dirs::config_dir()
        .map(|mut config_dir| {
            config_dir.push("zee");
            config_dir
        })
        .context("Could not get path to the user's config directory")
    {
        return Ok(config_dir);
    }

    // Fallback to location of the executable being run
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|path| path.to_path_buf()))
        .context("Could not get the path of the current executable")
}

pub static CONFIG_DIR: Lazy<Result<PathBuf>> = Lazy::new(config_dir);
