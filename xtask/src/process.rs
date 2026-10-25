// SPDX-License-Identifier: MIT OR Apache-2.0

//! Process helpers for the developer tasks.

use std::path::Path;
use std::process::Command;

/// Runs a program in the workspace root, streaming its output.
pub fn run(program: &str, arguments: &[&str]) -> Result<(), String> {
    run_in(program, arguments, &crate::workspace_root())
}

/// Runs a program in a specific directory, streaming its output.
pub fn run_in(program: &str, arguments: &[&str], directory: &Path) -> Result<(), String> {
    println!("$ {program} {}", arguments.join(" "));
    let status = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .status()
        .map_err(|error| format!("could not start {program}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {} exited with {status}",
            arguments.join(" ")
        ))
    }
}
