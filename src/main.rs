mod components;
mod editor;
mod error;
mod frontend;
mod mode;
mod settings;
mod smallstring;
mod syntax;
mod task;
mod terminal;
mod undo;
mod utils;

use clap;
use flexi_logger::{opt_format, Logger};
use std::{env, path::PathBuf};
use structopt::StructOpt;

use crate::{
    editor::Editor,
    error::Result,
    frontend::{Frontend, FrontendKind, DEFAULT_FRONTEND_STR},
    task::TaskPool,
    terminal::Screen,
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

    #[structopt(long = "frontend", default_value = DEFAULT_FRONTEND_STR)]
    /// What frontend to use. Depending on how features enabled at compile time,
    /// one of: termion, crossterm
    frontend_kind: FrontendKind,

    #[structopt(long = "log")]
    /// Enable debug logging to `zee.log` file
    enable_logging: bool,
}

fn run_editor_ui_loop(frontend_kind: &FrontendKind, mut editor: Editor) -> Result<()> {
    match frontend_kind {
        #[cfg(feature = "frontend-termion")]
        FrontendKind::Termion => {
            let frontend = frontend::termion::Termion::new()?;
            editor.ui_loop(Screen::new(frontend.size()?), frontend)
        }

        #[cfg(feature = "frontend-crossterm")]
        FrontendKind::Crossterm => {
            let frontend = frontend::crossterm::Crossterm::new()?;
            editor.ui_loop(Screen::new(frontend.size()?), frontend)
        }
    }
}

fn configure_logging() -> Result<()> {
    Logger::with_env_or_str("myprog=debug, mylib=debug")
        .log_to_file()
        .format(opt_format)
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

    // Create a default settings file if user requested it
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
    let mut editor = Editor::new(settings, current_dir, TaskPool::new()?);
    for file_path in args.files.iter() {
        editor.open_file(file_path)?;
    }

    // Start the UI loop
    run_editor_ui_loop(&args.frontend_kind, editor)
}

fn main() -> Result<()> {
    start_editor().map_err(|error| {
        log::error!("Zee exited with: {}", error);
        error
    })
}
