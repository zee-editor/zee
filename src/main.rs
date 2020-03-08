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

    #[structopt(long = "config-file", parse(from_os_str))]
    /// Path to the configuration directory. It's usually ~/.config/zee on Linux.
    config: Option<PathBuf>,

    #[structopt(long = "frontend", default_value = DEFAULT_FRONTEND_STR)]
    /// What frontend to use. Depending on how features enabled at compile time,
    /// one of: termion, crossterm
    frontend_kind: FrontendKind,
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

fn main() -> Result<()> {
    configure_logging()?;
    let args = Args::from_args();
    let current_dir = env::current_dir()?;
    let mut editor = Editor::new(current_dir, TaskPool::new()?);
    for file_path in args.files.iter() {
        editor.open_file(file_path)?;
    }
    run_editor_ui_loop(&args.frontend_kind, editor)
}
