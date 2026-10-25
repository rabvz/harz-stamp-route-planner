// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repository automation.
//!
//! `cargo xtask <command>` is the single entry point for every check and build
//! step this project has. Continuous integration calls exactly one command,
//! `cargo xtask ci`, so a contributor never has to rediscover the pipeline from
//! a workflow file.
//!
//! Commands:
//!
//! | command | purpose |
//! | --- | --- |
//! | `ci` | every quality gate, in the order CI runs them |
//! | `fmt` | `cargo fmt` (`--check` for verification) |
//! | `clippy` | clippy with warnings denied |
//! | `test` | the whole workspace test suite |
//! | `wasm` | build the WebAssembly artefact into `web/pkg/` |
//! | `web` | build the deployable static site into `dist/` |
//! | `data` | regenerate the station catalogue (`--check` for drift) |
//! | `check-architecture` | enforce the documented crate layering |
//! | `check-licenses` | enforce SPDX headers and licence metadata |
//! | `check-gpx` | round-trip and fixture checks for GPX output |
//! | `serve` | serve `dist/` on localhost for manual testing |
//! | `help` | this table |

mod assets;
mod checks;
mod data;
mod examples;
mod process;

use std::process::ExitCode;

/// Repository root, derived from the location of this crate.
#[must_use]
pub fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(command) = arguments.next() else {
        print_help();
        return ExitCode::FAILURE;
    };
    let flags: Vec<String> = arguments.collect();
    let check_only = flags.iter().any(|flag| flag == "--check");

    let result = match command.as_str() {
        "ci" => run_ci(),
        "fmt" => run_fmt(check_only),
        "clippy" => run_clippy(),
        "test" => run_test(),
        "wasm" => assets::build_wasm().map(|path| {
            println!("wasm artefact: {}", path.display());
        }),
        "web" => assets::build_web().map(|path| {
            println!("web build: {}", path.display());
        }),
        "data" => data::generate(check_only),
        "examples" => examples::generate().map(|path| {
            println!("examples written to {}", path.display());
        }),
        "check-architecture" => checks::check_architecture(),
        "check-licenses" => checks::check_licenses(),
        "check-gpx" => checks::check_gpx(),
        "check-secrets" => checks::check_secrets(),
        "serve" => assets::serve(port_from_flags(&flags)),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        unknown => {
            eprintln!("unknown command {unknown:?}");
            print_help();
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("\nFAILED: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Every gate CI runs, in the same order, failing at the first problem.
fn run_ci() -> Result<(), String> {
    step("format", || run_fmt(true))?;
    step("clippy", run_clippy)?;
    step("tests", run_test)?;
    step("station data drift", || data::generate(true))?;
    step("architecture", checks::check_architecture)?;
    step("licences", checks::check_licenses)?;
    step("gpx", checks::check_gpx)?;
    step("secrets", checks::check_secrets)?;
    step("wasm build", || assets::build_wasm().map(|_| ()))?;
    step("web build", || assets::build_web().map(|_| ()))?;
    println!("\nAll checks passed.");
    Ok(())
}

fn step(name: &str, run: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    println!("\n=== {name} ===");
    run()
}

/// Reads `--port <n>` from the flags, defaulting to 8080.
///
/// The port is configurable because 8080 is a popular port; a local preview
/// that refuses to start because something else holds it is an unnecessary
/// obstacle. Our own test run hit exactly that.
fn port_from_flags(flags: &[String]) -> u16 {
    flags
        .iter()
        .position(|flag| flag == "--port")
        .and_then(|index| flags.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080)
}

fn run_fmt(check: bool) -> Result<(), String> {
    let mut arguments = vec!["fmt", "--all"];
    if check {
        arguments.push("--");
        arguments.push("--check");
    }
    process::run("cargo", &arguments)
}

fn run_clippy() -> Result<(), String> {
    process::run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn run_test() -> Result<(), String> {
    process::run("cargo", &["test", "--workspace", "--all-features"])
}

fn print_help() {
    println!(
        "harz-stamp-route-planner developer tasks

Usage: cargo xtask <command> [--check]

Commands:
  ci                  run every check in the order CI does
  fmt [--check]       format the workspace (or verify formatting)
  clippy              run clippy with warnings denied
  test                run the workspace test suite
  wasm                build the WebAssembly module into web/pkg/
  web                 build the deployable site into dist/
  data [--check]      regenerate the station catalogue (or verify it)
  examples            write the example GPX files to examples/
  check-architecture  enforce the documented crate layering
  check-licenses      enforce SPDX headers and licence metadata
  check-gpx           round-trip and fixture checks for GPX
  check-secrets       scan the tree for committed credentials
  serve [--port N]    serve dist/ (default http://127.0.0.1:8080)
  help                show this table"
    );
}
