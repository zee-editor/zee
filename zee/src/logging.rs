use anyhow::{Context, Result};
use colored::{ColoredString, Colorize};
use flexi_logger::{DeferredNow, FileSpec, Level, Logger, Record};
use once_cell::sync::Lazy;
use std::{io::Write, ops::Deref};

pub fn configure_for_editor() -> Result<()> {
    Logger::try_with_env_or_str("info")?
        .log_to_file(
            FileSpec::default()
                .basename("zee")
                .suffix("log")
                .suppress_timestamp(),
        )
        .start()
        .map(|_handle| ())
        .context("Could not initialise logging to file")
}

pub fn configure_for_cli(verbose: bool) -> Result<()> {
    Logger::try_with_env_or_str(if verbose { "debug" } else { "info" })?
        .log_to_stderr()
        .format_for_stderr(cli_format)
        .start()
        .map(|_handle| ())
        .context("Could not initialise logging to stderr")
}

fn cli_format(
    writer: &mut dyn Write,
    _now: &mut DeferredNow,
    record: &Record<'_>,
) -> std::io::Result<()> {
    let level = match record.level() {
        Level::Debug => "",
        Level::Info => "",
        Level::Warn => LOG_PREFIX_WARN.deref(),
        Level::Error => LOG_PREFIX_ERROR.deref(),
        Level::Trace => LOG_PREFIX_TRACE.deref(),
    };

    write!(writer, "{}{}", level, record.args())
}

pub static LOG_PREFIX_WARN: Lazy<ColoredString> = Lazy::new(|| "W ".yellow().bold());
pub static LOG_PREFIX_ERROR: Lazy<ColoredString> = Lazy::new(|| "E ".red().bold());
pub static LOG_PREFIX_TRACE: Lazy<ColoredString> = Lazy::new(|| "T ".normal());
