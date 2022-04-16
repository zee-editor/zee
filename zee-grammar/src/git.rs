use anyhow::{anyhow, Result};
use std::{path::Path, process::Command};

pub fn get_revision(current_dir: impl AsRef<Path>) -> Option<String> {
    run(current_dir, ["rev-parse", "HEAD"]).ok()
}

pub fn get_remote_url(current_dir: impl AsRef<Path>) -> Option<String> {
    run(current_dir, ["remote", "get-url", REMOTE_NAME]).ok()
}

pub fn set_remote(current_dir: impl AsRef<Path>, remote_url: &str) -> Result<String> {
    run(
        current_dir.as_ref(),
        ["remote", "set-url", REMOTE_NAME, remote_url],
    )
    .or_else(|_| run(current_dir, ["remote", "add", REMOTE_NAME, remote_url]))
}

pub fn run(
    current_dir: impl AsRef<Path>,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<String> {
    let mut command = Command::new(GIT_COMMAND);
    command
        .args(args)
        .current_dir(current_dir)
        .env("GIT_TERMINAL_PROMPT", "0"); // non-interactive
    let command_str = format!("{command:?}");

    let output = command.output()?;
    output
        .status
        .success()
        .then(|| {
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_owned()
        })
        .ok_or_else(|| {
            anyhow!(
                "Git command failed: {}\nStdout: {}\nStderr: {}",
                command_str,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            )
        })
}

const REMOTE_NAME: &str = "origin";
const GIT_COMMAND: &str = "git";
