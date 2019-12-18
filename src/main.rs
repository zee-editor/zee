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
mod utils;

use clap;
use std::{env, path::PathBuf};
use structopt::StructOpt;

use crate::{
    editor::Editor,
    error::Result,
    frontend::{Frontend, Termion},
    task::TaskPool,
    terminal::Screen,
};

#[derive(Debug, StructOpt)]
#[structopt(global_settings(&[clap::AppSettings::ColoredHelp]))]
pub struct Args {
    #[structopt(name = "file", parse(from_os_str))]
    /// Open file to edit
    pub files: Vec<PathBuf>,

    #[structopt(long = "config-file", parse(from_os_str))]
    /// Path to the configuration directory. It's usually ~/.config/zee on Linux.
    pub config: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::from_args();
    let current_dir = env::current_dir()?;
    let mut editor = Editor::new(current_dir, TaskPool::new()?);
    for file_path in args.files.iter() {
        editor.open_file(file_path)?;
    }
    let frontend = Termion::new()?;
    editor.ui_loop(Screen::new(frontend.size()?), frontend)
}
