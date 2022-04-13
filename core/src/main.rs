#![allow(clippy::reversed_empty_ranges)]

mod clipboard;
mod components;
mod edit;
mod editor;
mod error;
mod mode;
mod settings;
mod syntax;
mod task;
mod utils;
mod versioned;

use flexi_logger::{FileSpec, Logger};
use std::{env, path::PathBuf};
use zi::ComponentExt;

use crate::{
    editor::{Editor, Properties as EditorProperties},
    error::Result,
    task::TaskPool,
};

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[clap(name = "file", parse(from_os_str))]
    /// Open file to edit
    files: Vec<PathBuf>,

    #[clap(long = "settings-path", parse(from_os_str))]
    /// Path to the configuration file. It's usually ~/.config/zee on Linux.
    settings_path: Option<PathBuf>,

    #[clap(long = "create-settings")]
    /// Writes the default configuration to file, if the file doesn't exist
    create_settings: bool,

    #[clap(long = "log")]
    /// Enable debug logging to `zee.log` file
    enable_logging: bool,
}

fn configure_logging() -> Result<()> {
    Logger::try_with_env_or_str("myprog=debug, mylib=debug")?
        .log_to_file(
            FileSpec::default()
                .basename("zee")
                .suffix("log")
                .suppress_timestamp(),
        )
        .start()
        .map(|_handle| ())
        .map_err(anyhow::Error::from)
}

fn start_editor() -> Result<()> {
    let args = Args::parse();
    if args.enable_logging {
        configure_logging()?;
    }
    let current_dir = env::current_dir()?;

    // Read the current settings. If we cannot for any reason, we'll use the
    // default ones to ensure the editor opens in any environment.
    let settings = args
        .settings_path
        .or_else(|| settings::settings_path().map(Some).unwrap_or(None))
        .map_or_else(Default::default, settings::read_settings);

    // Create a default settings file if requested by the user
    if args.create_settings {
        let settings_path = settings::settings_path()?;
        if !settings_path.exists() {
            settings::create_default_file(&settings_path)?;
        } else {
            log::warn!(
                "Default settings file won't be created; a file already exists `{}`",
                settings_path.display()
            );
        }
    }

    // Instantiate the editor, open any files specified as arguments and start the UI loop
    zi_term::incremental()?.run_event_loop(Editor::with(EditorProperties {
        args_files: args.files,
        current_working_dir: current_dir,
        settings,
        task_pool: TaskPool::new()?,
        clipboard: clipboard::create()?,
    }))?;

    Ok(())
}

fn main() -> Result<()> {
    start_editor().map_err(|error| {
        log::error!("Zee exited with: {}", error);
        error
    })
}
