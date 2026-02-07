use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use crate::error::{Result, TinythisError};

pub fn run_capture(program: &Path, args: &[OsString]) -> Result<std::process::Output> {
    let output = Command::new(program).args(args).output()?;
    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Err(TinythisError::ProcessFailed {
        program: program.display().to_string(),
        code: output.status.code(),
        stderr,
    })
}
