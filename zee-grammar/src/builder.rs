use anyhow::{bail, Context, Result};
use colored::Colorize;
use include_dir::Dir;
use libloading::{Library, Symbol};
use log;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};
use tree_sitter::{Language, Query};

use crate::{
    config::{self, GrammarConfig, GrammarSource, ModeConfig},
    git, Grammar, Mode,
};

const BUILD_DIR: &str = "build";
const GRAMMAR_DIR: &str = "grammars";
const LIBRARY_DIR: &str = "lib";
const QUERY_DIR: &str = "queries";

pub fn load_mode(config: ModeConfig) -> Result<Mode> {
    let ModeConfig {
        name,
        scope,
        injection_regex,
        patterns,
        comment,
        indentation,
        grammar: grammar_config,
    } = config;

    let grammar = grammar_config.and_then(|grammar_config| {
        let grammar_id = grammar_config.grammar_id.clone();
        log_on_error(&grammar_id, load_grammar(grammar_config.grammar_id))
    });

    Ok(Mode {
        name,
        scope,
        injection_regex,
        patterns,
        comment,
        indentation,
        grammar,
    })
}

fn load_grammar(grammar_id: String) -> Result<Grammar> {
    let language = load_language(&grammar_id)?;
    let make_query = |name| log_on_error(&grammar_id, load_query(language, &grammar_id, name));
    let [highlights, indents, injections, locals] =
        ["highlights", "indents", "injections", "locals"].map(make_query);

    Ok(Grammar {
        id: grammar_id,
        language,
        highlights,
        indents,
        injections,
        locals,
    })
}

fn load_query(language: Language, grammar_id: &str, name: &str) -> Result<Query> {
    let query_path = tree_sitter_query_dir(grammar_id)
        .map(|path| path.join(&format!("{}.scm", name)))
        .with_context(|| {
            format!(
                "Failed to build path to query grammar_id={} name={}",
                grammar_id, name
            )
        })?;
    let query_src = std::fs::read_to_string(&query_path)
        .with_context(|| format!("Failed to read query at {}", query_path.display()))?;
    Ok(Query::new(language, &query_src)?)
}

fn load_language(grammar_id: &str) -> Result<Language> {
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

pub fn fetch_and_build_tree_sitter_parsers(
    mode_configs: &[ModeConfig],
    defaults: &Dir,
) -> Result<()> {
    mode_configs.into_par_iter().try_for_each(|config| {
        if let Some(ref grammar) = config.grammar {
            let fetched = fetch_grammar(grammar)?;
            let built = build_grammar(grammar, defaults)?;
            log::info!(
                "{:>12} {} grammar {}",
                if fetched || built {
                    "Installed"
                } else {
                    "Up to date"
                }
                .bold()
                .bright_green(),
                grammar.grammar_id.bold().bright_blue(),
                "✅".green().dimmed(),
            );
        }
        Ok(())
    })
}

fn fetch_grammar(grammar: &GrammarConfig) -> Result<bool> {
    let (remote, revision) = match grammar.source {
        GrammarSource::Git {
            ref remote,
            ref revision,
            ..
        } => (remote, revision),
        _ => return Ok(false),
    };

    let grammar_dir = tree_sitter_source_dir(&grammar.grammar_id)?;
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

fn build_grammar(grammar: &GrammarConfig, defaults: &Dir) -> Result<bool> {
    let (grammar_dir, subpath) = match grammar.source {
        GrammarSource::Local { ref path } => (path.clone(), None),
        GrammarSource::Git {
            path: ref subpath, ..
        } => (
            tree_sitter_source_dir(&grammar.grammar_id)?,
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

    let grammar_dir_entries = grammar_dir
        .read_dir()
        .with_context(|| format!("Failed to read directory {grammar_dir:?}.",))?;

    if grammar_dir_entries.count() == 0 {
        bail!("Directory {grammar_dir:?} is empty.",);
    };

    copy_tree_sitter_queries(&grammar.grammar_id, &grammar_dir, defaults)?;

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
    log::debug!(
        "{:>12} {} {command_str}",
        "Running".bold().dimmed(),
        grammar_id.bold().bright_blue(),
    );
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

fn tree_sitter_source_dir(grammar_id: &str) -> Result<PathBuf> {
    Ok(config::config_dir()?
        .join(BUILD_DIR)
        .join(&format!("tree-sitter-{}", grammar_id)))
}

fn tree_sitter_query_dir(grammar_id: &str) -> Result<PathBuf> {
    Ok(config::config_dir()?
        .join(GRAMMAR_DIR)
        .join(QUERY_DIR)
        .join(grammar_id))
}

fn tree_sitter_library_dir() -> Result<PathBuf> {
    Ok(config::config_dir()?.join(GRAMMAR_DIR).join(LIBRARY_DIR))
}

fn tree_sitter_library_name(grammar_id: &str) -> String {
    format!("tree-sitter-{grammar_id}")
}

fn tree_sitter_library_path(grammar_id: &str) -> Result<PathBuf> {
    let mut library_path = tree_sitter_library_dir()?.join(tree_sitter_library_name(grammar_id));
    library_path.set_extension(LIBRARY_EXTENSION);
    Ok(library_path)
}

fn copy_tree_sitter_queries(grammar_id: &str, source: &Path, defaults: &Dir) -> Result<()> {
    let query_dir_dest = tree_sitter_query_dir(grammar_id)?;
    std::fs::create_dir_all(&query_dir_dest).with_context(|| {
        format!(
            "Could not create grammar queries directory {}",
            query_dir_dest.display()
        )
    })?;

    for query_name in ["highlights", "indents", "locals", "injections"] {
        let query_filename = PathBuf::from(format!("{}.scm", query_name));

        // Query destination path
        let query_dest = query_dir_dest.join(&query_filename);

        // If query file already exists at destination, don't overwrite it
        if query_dest.exists() {
            log::debug!(
                "{:>12} {} {} query; already exists {}",
                "Skip".bold().dimmed(),
                grammar_id.bold().bright_blue(),
                query_name,
                format!("{}", query_dest.display()).dimmed(),
            );
            continue;
        }

        // If the packaged default overrides contain the query, use it
        let query_override = defaults
            .get_file(
                &PathBuf::from(QUERY_DIR)
                    .join(grammar_id)
                    .join(&query_filename),
            )
            .map(|file| file.contents());
        if let Some(query_source) = query_override {
            log::debug!(
                "{:>12} {} query {}; using packaged override for {}",
                "Copying".bold().dimmed(),
                grammar_id.bold().bright_blue(),
                query_name,
                query_dest.display(),
            );
            // log::info!("Using packaged query override for {}", query_dest.display());
            log_on_error(grammar_id, std::fs::write(query_dest, query_source));
            continue;
        }

        // Otherwise, copy it from the git repo, if available
        let query_src = source.join(QUERY_DIR).join(query_filename);
        if query_src.exists() {
            log_on_error(
                grammar_id,
                std::fs::copy(&query_src, &query_dest).with_context(|| {
                    format!(
                        "Could not copy {} -> {}",
                        query_src.display(),
                        query_dest.display()
                    )
                }),
            );
        } else {
            log::debug!(
                "{:>12} {} {} query {}",
                "Missing".bold().dimmed(),
                grammar_id.bold().bright_blue(),
                query_name,
                format!("{}", query_src.display()).dimmed(),
            );
        }
    }

    Ok(())
}

fn log_on_error<T, E: std::fmt::Display>(
    grammar_id: &str,
    result: std::result::Result<T, E>,
) -> Option<T> {
    match result {
        Err(error) => {
            log::error!(
                "{:>12} {} {}",
                "Error".bold().bright_red(),
                grammar_id.bold().bright_blue(),
                error
            );
            None
        }
        Ok(value) => Some(value),
    }
}

fn mtime(path: &Path) -> Result<SystemTime> {
    Ok(std::fs::metadata(path)?.modified()?)
}

const BUILD_TARGET: &str = env!("BUILD_TARGET");

#[cfg(unix)]
const LIBRARY_EXTENSION: &str = "so";
#[cfg(windows)]
const LIBRARY_EXTENSION: &str = "dll";
