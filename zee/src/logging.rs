use anyhow::{Context, Result};
use flexi_logger::{DeferredNow, FileSpec, Logger, Record};
use std::io::Write;

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
    write!(writer, "{}", record.args())
}
