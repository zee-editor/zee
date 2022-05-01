#![allow(clippy::reversed_empty_ranges)]

mod clipboard;
mod components;
mod config;
mod editor;
mod error;
mod logging;
mod panicking;
mod syntax;
mod task;
mod utils;
mod versioned;

use clap::Parser;
use std::{env, path::PathBuf};
use zi::ComponentExt;

use crate::{
    editor::{Editor, Properties as EditorProperties},
    error::Result,
    task::TaskPool,
};

#[derive(Debug, Parser)]
#[clap(about, version)]
struct Args {
    #[clap(name = "file", parse(from_os_str))]
    /// Open these files to edit after starting zee
    files: Vec<PathBuf>,

    #[clap(long = "config-dir", parse(from_os_str))]
    /// Path to the zee configuration directory. Usually ~/.config/zee on
    /// Linux and `%AppData%/zee` on Windows by default.
    config_dir: Option<PathBuf>,

    #[clap(long = "init")]
    /// Initialises the default configuration directory, if missing. Usually
    /// ~/.config/zee on Linux and %AppData%/zee on Windows by default. This
    /// command will create a default configuration file `config.ron` with
    /// comments that you should edit further to customise zee.
    initialise: bool,

    #[clap(long = "log")]
    /// Enable debug logging to `zee.log` file
    enable_logging: bool,

    #[clap(long = "build")]
    /// Download and build tree-sitter parsers
    build: bool,

    #[clap(short = 'v', long = "verbose")]
    /// Verbose mode. Display extra information when building grammars
    verbose: bool,
}

fn start_editor() -> Result<()> {
    let args = Args::parse();

    if args.initialise || args.build {
        logging::configure_for_cli(args.verbose)?;
    } else if args.enable_logging {
        logging::configure_for_editor()?;
    }

    // Create a default configuration file if requested by the user
    if args.initialise {
        let config_path = zee_grammar::config::config_dir()?.join("config.ron");
        if !config_path.exists() {
            config::create_default_config_file(&config_path)?;
            log::info!(
                "A default configuration file was created at `{}`",
                config_path.display()
            );
        } else {
            log::warn!(
                "Default settings file won't be created; a file already exists `{}`",
                config_path.display()
            );
        }
    }

    // Finds the editor configuration. If we cannot for any reason, we'll use the
    // default ones to ensure the editor opens in any environment.
    let editor_config = config::find_editor_config(args.config_dir);

    // Download and build tree sitter parsers if requested
    if args.build {
        zee_grammar::builder::fetch_and_build_tree_sitter_parsers(
            &editor_config.modes,
            &crate::config::DEFAULT_CONFIG_DIR,
        )?;
    }

    if args.build || args.initialise {
        return Ok(());
    }

    // Instantiate the editor, open any files specified as arguments and start the UI loop
    zi_term::incremental()?.run_event_loop(Editor::with(EditorProperties {
        args_files: args.files,
        current_working_dir: env::current_dir()?,
        config: editor_config,
        task_pool: TaskPool::new()?,
        clipboard: clipboard::create()?,
    }))?;

    Ok(())
}

fn main() -> Result<()> {
    panicking::print_panic_after_unwind(|| {
        start_editor().map_err(|error| {
            log::error!("Zee exited with: {}", error);
            error
        })
    })
}
