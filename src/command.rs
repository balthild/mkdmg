//! Running the system tools the build depends on.

use std::{
    ffi::{OsStr, OsString},
    process::Command,
};

use crate::{Error, Result};

/// Turn anything path-like into a command argument.
pub(crate) fn arg(value: impl AsRef<OsStr>) -> OsString {
    value.as_ref().to_owned()
}

/// Run `program`, returning its standard output.
///
/// Failures carry the tool's standard error, which is where `hdiutil` and
/// `ditto` explain themselves.
pub(crate) fn run(program: &str, args: Vec<OsString>) -> Result<String> {
    let output = Command::new(program)
        .args(&args)
        .output()
        .map_err(|error| Error::io(program, error))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }

    Err(Error::Command {
        command: render(program, &args),
        status: output.status,
        details: stderr(&output.stderr),
    })
}

fn render(program: &str, args: &[OsString]) -> String {
    let mut command = program.to_owned();

    for argument in args {
        command.push(' ');
        command.push_str(&argument.to_string_lossy());
    }

    command
}

fn stderr(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim();

    if trimmed.is_empty() {
        String::new()
    } else {
        format!(": {trimmed}")
    }
}
