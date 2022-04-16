use anyhow::{bail, Context, Result};
use colored::Colorize;
use libloading::{Library, Symbol};
use log;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tree_sitter::Language;

use crate::{
    config::{self, Grammar, GrammarSource},
    git,
    mode::Mode,
};

const GRAMMAR_DIR: &str = "grammars";
const LIBRARY_DIR: &str = "lib";
const SOURCE_DIR: &str = "src";
const QUERY_DIR: &str = "queries";

pub fn load_language(grammar_id: &str) -> Result<Language> {
    let library_path = tree_sitter_library_path(grammar_id)?;
    let library = unsafe { Library::new(&library_path) }
        .with_context(|| format!("Error opening dynamic library {library_path:?}"))?;

    let language_fn_name = format!("tree_sitter_{}", grammar_id.replace('-', "_"));
    let language = unsafe {
        let language_fn: Symbol<unsafe extern "C" fn() -> Language> = library
            .get(language_fn_name.as_bytes())
            .with_context(|| format!("Failed to load symbol {language_fn_name}"))?;
        language_fn()
    };

    std::mem::forget(library);
    Ok(language)
}

pub fn fetch_and_build_tree_sitter_parsers(mode_configs: &[Mode]) -> Result<()> {
    mode_configs.into_par_iter().try_for_each(|config| {
        if let Some(ref grammar) = config.grammar {
            fetch_grammar(grammar)?;
            build_grammar(grammar)?;
            log::info!(
                "{:>12} {} grammar {}",
                "Up to date".bold().bright_green(),
                grammar.grammar_id.bold().bright_blue(),
                "✅".green().dimmed(),
            );
        }
        Ok(())
    })
}

pub fn fetch_grammar(grammar: &Grammar) -> Result<bool> {
    let (remote, revision) = match grammar.source {
        GrammarSource::Git {
            ref remote,
            ref revision,
            ..
        } => (remote, revision),
        _ => return Ok(false),
    };

    let grammar_dir = config::runtime_dir()?
        .join(GRAMMAR_DIR)
        .join(SOURCE_DIR)
        .join(&grammar.grammar_id);

    std::fs::create_dir_all(&grammar_dir).context(format!(
        "Could not create grammar directory {:?}",
        grammar_dir
    ))?;

    // Git init the grammar dir if it doesn't contain a .git directory
    if !grammar_dir.join(".git").is_dir() {
        git::run(&grammar_dir, ["init"])?;
    }

    // Make sure the remote matches the configuration
    let remote_changed = git::get_remote_url(&grammar_dir)
        .map(|current_remote| current_remote != *remote)
        .unwrap_or(true);
    if remote_changed {
        git::set_remote(&grammar_dir, remote)?;
    }

    // Make sure the checked out revision matches the configuration
    let revision_changed = remote_changed
        || git::get_revision(&grammar_dir)
            .map(|current_revision| current_revision != *revision)
            .unwrap_or(true);
    if revision_changed {
        git::run(&grammar_dir, ["fetch", "--depth", "1", "origin", revision])?;
        git::run(&grammar_dir, ["checkout", revision])?;

        log::info!(
            "{:>12} {} grammar {}#{}",
            "Downloading".bold().bright_cyan(),
            grammar.grammar_id.bold().bright_blue(),
            remote.dimmed(),
            revision[0..8].dimmed(),
        );
    }

    Ok(revision_changed)
}

pub fn build_grammar(grammar: &Grammar) -> Result<bool> {
    let (grammar_dir, subpath) = match grammar.source {
        GrammarSource::Local { ref path } => (path.clone(), None),
        GrammarSource::Git {
            path: ref subpath, ..
        } => (
            config::runtime_dir()?
                .join(GRAMMAR_DIR)
                .join(SOURCE_DIR)
                .join(&grammar.grammar_id),
            subpath.clone(),
        ),
    };

    // Make sure the tree sitter library directory exists
    {
        let library_dir = tree_sitter_library_dir()?;
        std::fs::create_dir_all(&library_dir).with_context(|| {
            format!("Could not create tree sitter library directory: {library_dir:?}",)
        })?;
    }

    let grammar_dir_entries = grammar_dir.read_dir().with_context(|| {
        format!("Failed to read directory {grammar_dir:?}. Did you use 'hx --grammar fetch'?",)
    })?;

    if grammar_dir_entries.count() == 0 {
        bail!("Directory {grammar_dir:?} is empty. Did you use 'hx --grammar fetch'?",);
    };

    copy_tree_sitter_queries(&grammar.grammar_id, &grammar_dir)?;

    // Build the tree sitter library
    let paths = TreeSitterPaths::new(grammar_dir.clone(), subpath);
    build_tree_sitter_library(&grammar.grammar_id, &paths).with_context(|| {
        format!(
            "Failed to build tree sitter library for `{grammar_id}` in {grammar_dir}",
            grammar_id = grammar.grammar_id,
            grammar_dir = grammar_dir.display()
        )
    })
}

fn build_tree_sitter_library(grammar_id: &str, paths: &TreeSitterPaths) -> Result<bool> {
    let library_path = tree_sitter_library_path(grammar_id)?;
    let should_recompile = paths.should_recompile(&library_path)?;

    if !should_recompile {
        return Ok(false);
    }

    log::info!(
        "{:>12} {} grammar {}",
        "Building".bold().bright_cyan(),
        grammar_id.bold().bright_blue(),
        library_path.to_string_lossy().dimmed(),
    );

    let mut compiler = cc::Build::new();
    compiler
        .cpp(true)
        .warnings(false)
        .include(&paths.source)
        .opt_level(3)
        .cargo_metadata(false)
        .shared_flag(true)
        .host(BUILD_TARGET)
        .target(BUILD_TARGET);

    let mut command = compiler.try_get_compiler()?.to_command();
    if cfg!(windows) {
        command.arg(&paths.parser);
        if let Some(TreeSitterScannerSource { ref path, .. }) = paths.scanner {
            command.arg(&path);
        }
        command.arg(format!("/out:{}", library_path.to_str().unwrap()));
    } else {
        command
            .arg("-fPIC")
            .arg("-fno-exceptions")
            .arg("-xc")
            .arg(&paths.parser)
            .arg("-o")
            .arg(&library_path);
        if let Some(TreeSitterScannerSource { ref path, cpp }) = paths.scanner {
            if cpp {
                command.arg("-xc++");
            } else {
                command.arg("-xc").arg("-std=c99");
            }
            command.arg(path);
        }
    }

    // Compile the tree sitter library
    let command_str = format!("{command:?}");
    log::debug!("{:>12} {command_str}", "Running".bold().dimmed());
    let output = command
        .output()
        .with_context(|| format!("Failed to run C compiler. Command: {command_str}"))?;
    if !output.status.success() {
        bail!(
            "Parser compilation failed:\nCommand: {command_str}\nStdout: {}\nStderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(true)
}

struct TreeSitterScannerSource {
    path: PathBuf,
    cpp: bool,
}

struct TreeSitterPaths {
    source: PathBuf,
    parser: PathBuf,
    scanner: Option<TreeSitterScannerSource>,
}

impl TreeSitterPaths {
    fn new(repo: PathBuf, relative: Option<PathBuf>) -> Self {
        // Resolve subpath within the repo if any
        let subpath = relative.map(|subpath| repo.join(subpath)).unwrap_or(repo);

        // Source directory
        let source = subpath.join("src");

        // Path to parser source
        let parser = source.join("parser.c");

        // Path to scanner if any
        let mut scanner_path = source.join("scanner.c");
        let scanner = if scanner_path.exists() {
            Some(TreeSitterScannerSource {
                path: scanner_path,
                cpp: false,
            })
        } else {
            scanner_path.set_extension("cc");
            if scanner_path.exists() {
                Some(TreeSitterScannerSource {
                    path: scanner_path,
                    cpp: true,
                })
            } else {
                None
            }
        };

        Self {
            source,
            parser,
            scanner,
        }
    }

    fn should_recompile(&self, library_path: &Path) -> Result<bool> {
        let mtime = |path| mtime(path).context("Failed to compare source and library timestamps");
        if !library_path.exists() {
            return Ok(true);
        };
        let library_mtime = mtime(library_path)?;
        if mtime(&self.parser)? > library_mtime {
            return Ok(true);
        }
        if let Some(TreeSitterScannerSource { ref path, .. }) = self.scanner {
            if mtime(path)? > library_mtime {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn tree_sitter_query_dir() -> Result<PathBuf> {
    Ok(config::runtime_dir()?.join(GRAMMAR_DIR).join(QUERY_DIR))
}

fn tree_sitter_library_dir() -> Result<PathBuf> {
    Ok(config::runtime_dir()?.join(GRAMMAR_DIR).join(LIBRARY_DIR))
}

fn tree_sitter_library_name(grammar_id: &str) -> String {
    format!("tree-sitter-{grammar_id}")
}

fn tree_sitter_library_path(grammar_id: &str) -> Result<PathBuf> {
    let mut library_path = tree_sitter_library_dir()?.join(tree_sitter_library_name(grammar_id));
    library_path.set_extension(LIBRARY_EXTENSION);
    Ok(library_path)
}

fn copy_tree_sitter_queries(grammar_id: &str, path: &Path) -> Result<()> {
    let query_dir = tree_sitter_query_dir()?;
    std::fs::create_dir_all(&query_dir)
        .with_context(|| format!("Could not create grammar queries directory {query_dir:?}",))?;

    let query_src = path.join("queries").join("highlights.scm");
    let query_dest = query_dir.join(&format!("{grammar_id}.scm"));
    std::fs::copy(&query_src, &query_dest)
        .with_context(|| format!("Could not copy {query_src:?} -> {query_dest:?}"))?;

    Ok(())
}

fn mtime(path: &Path) -> Result<SystemTime> {
    Ok(std::fs::metadata(path)?.modified()?)
}

/// Gives the contents of a file from a language's `runtime/queries/<lang>`
/// directory
pub fn load_runtime_file(language: &str, filename: &str) -> Result<String> {
    let path = config::runtime_dir()?
        .join("queries")
        .join(language)
        .join(filename);
    Ok(std::fs::read_to_string(&path)?)
}

const BUILD_TARGET: &str = env!("BUILD_TARGET");
#[cfg(unix)]
const LIBRARY_EXTENSION: &str = "so";
#[cfg(windows)]
const LIBRARY_EXTENSION: &str = "dll";
