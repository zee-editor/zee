#![allow(clippy::reversed_empty_ranges)]

mod clipboard;
mod components;
mod editor;
mod error;
mod mode;
mod settings;
mod syntax;
mod task;
mod undo;
mod utils;

use flexi_logger::Logger;
use std::{env, path::PathBuf, rc::Rc};
use structopt::StructOpt;
use zi::{layout, App};

use crate::{
    editor::{Context, Editor},
    error::Result,
    task::TaskPool,
};

#[derive(Debug, StructOpt)]
#[structopt(global_settings(&[clap::AppSettings::ColoredHelp]))]
struct Args {
    #[structopt(name = "file", parse(from_os_str))]
    /// Open file to edit
    files: Vec<PathBuf>,

    #[structopt(long = "settings-path", parse(from_os_str))]
    /// Path to the configuration file. It's usually ~/.config/zee on Linux.
    settings_path: Option<PathBuf>,

    #[structopt(long = "create-settings")]
    /// Writes the default configuration to file, if the file doesn't exist
    create_settings: bool,

    #[structopt(long = "log")]
    /// Enable debug logging to `zee.log` file
    enable_logging: bool,
}

fn configure_logging() -> Result<()> {
    Logger::with_env_or_str("myprog=debug, mylib=debug")
        .log_to_file()
        .suppress_timestamp()
        .start()
        .map_err(anyhow::Error::from)?;
    Ok(())
}

fn start_editor() -> Result<()> {
    let args = Args::from_args();
    if args.enable_logging {
        configure_logging()?;
    }
    let current_dir = env::current_dir()?;

    // Read the current settings. If we cannot for any reason, we'll use the
    // default ones -- ensure the editor opens in any environment.
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

    // Instantiate editor and open any files specified as arguments
    let context = Rc::new(Context {
        args_files: args.files,
        current_working_dir: current_dir,
        settings,
        task_pool: TaskPool::new()?,
        clipboard: clipboard::create()?,
    });
    let mut app = App::new(layout::component::<Editor>(context));

    // Start the UI loop
    let frontend =
        zi::frontend::crossterm::incremental().map_err(|err| -> zi::Error { err.into() })?;
    app.run_event_loop(frontend)?;

    Ok(())
}

fn main() -> Result<()> {
    start_editor().map_err(|error| {
        log::error!("Zee exited with: {}", error);
        error
    })
}
