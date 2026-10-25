// SPDX-License-Identifier: MIT OR Apache-2.0

//! Building the WebAssembly module and the deployable static site.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::process;
use crate::workspace_root;

/// Target triple the browser build uses.
const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// Cargo profile used for the browser build.
const WASM_PROFILE: &str = "wasm-release";

/// File name of the compiled module inside `web/pkg/`.
const WASM_ARTEFACT: &str = "route_planner.wasm";

/// Builds the WebAssembly module into `web/pkg/`.
pub fn build_wasm() -> Result<PathBuf, String> {
    let root = workspace_root();
    process::run(
        "cargo",
        &[
            "build",
            "--package",
            "wasm-bindings",
            "--target",
            WASM_TARGET,
            "--profile",
            WASM_PROFILE,
        ],
    )?;

    let compiled = root
        .join("target")
        .join(WASM_TARGET)
        .join(WASM_PROFILE)
        .join("wasm_bindings.wasm");
    if !compiled.is_file() {
        return Err(format!(
            "expected the module at {}, but it was not produced",
            compiled.display()
        ));
    }

    let destination_directory = root.join("web/pkg");
    std::fs::create_dir_all(&destination_directory).map_err(|error| {
        format!(
            "could not create {}: {error}",
            destination_directory.display()
        )
    })?;
    let destination = destination_directory.join(WASM_ARTEFACT);
    std::fs::copy(&compiled, &destination).map_err(|error| {
        format!(
            "could not copy {} to {}: {error}",
            compiled.display(),
            destination.display()
        )
    })?;

    let size = std::fs::metadata(&destination)
        .map_err(|error| format!("could not stat {}: {error}", destination.display()))?
        .len();
    println!("module size: {} KiB", size / 1024);
    Ok(destination)
}

/// Builds the static site into `dist/`.
pub fn build_web() -> Result<PathBuf, String> {
    let root = workspace_root();
    build_wasm()?;

    let source = root.join("web");
    let destination = root.join("dist");

    if destination.exists() {
        std::fs::remove_dir_all(&destination)
            .map_err(|error| format!("could not clear {}: {error}", destination.display()))?;
    }
    copy_tree(&source, &destination)?;

    // Legal texts travel with the deployed site so attribution is available
    // from the page footer without a trip to the repository.
    for file in ["LICENSE-MIT", "LICENSE-APACHE", "ATTRIBUTION.md"] {
        let from = root.join(file);
        if from.is_file() {
            std::fs::copy(&from, destination.join(file))
                .map_err(|error| format!("could not copy {file}: {error}"))?;
        }
    }

    // GitHub Pages serves 404.html for unknown paths; the single-page app
    // needs the entry document instead.
    let index = destination.join("index.html");
    if index.is_file() {
        std::fs::copy(&index, destination.join("404.html"))
            .map_err(|error| format!("could not write 404.html: {error}"))?;
    }

    // The site is useless without the module, so fail the build rather than
    // shipping a page that cannot start.
    let module = destination.join("pkg").join(WASM_ARTEFACT);
    if !module.is_file() {
        return Err(format!("the built site has no {}", module.display()));
    }

    println!("site written to {}", destination.display());
    Ok(destination)
}

/// Serves `dist/` for manual testing.
pub fn serve(port: u16) -> Result<(), String> {
    let root = workspace_root();
    let directory = root.join("dist");
    if !directory.is_dir() {
        return Err("dist/ does not exist; run `cargo xtask web` first".to_string());
    }

    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&address)
        .map_err(|error| format!("could not bind {address}: {error}"))?;
    println!("serving {} on http://{address}/", directory.display());
    println!("press Ctrl+C to stop");

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut buffer = [0_u8; 8192];
        let read = stream.read(&mut buffer).unwrap_or(0);
        if read == 0 {
            continue;
        }
        let request = String::from_utf8_lossy(&buffer[..read]);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");

        let (status, content_type, body) = response_for(&directory, path);
        let header = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }
    Ok(())
}

/// Builds a response for one request path.
fn response_for(directory: &Path, path: &str) -> (&'static str, &'static str, Vec<u8>) {
    let path = path.split('?').next().unwrap_or("/");
    let path = if path == "/" { "/index.html" } else { path };
    let decoded = percent_decode(path);

    // Reject traversal outright rather than normalising it.
    if decoded.contains("..") {
        return not_found();
    }
    let candidate = directory.join(decoded.trim_start_matches('/'));
    if !candidate.is_file() {
        return not_found();
    }
    match std::fs::read(&candidate) {
        Ok(body) => ("200 OK", content_type(&candidate), body),
        Err(_) => not_found(),
    }
}

fn not_found() -> (&'static str, &'static str, Vec<u8>) {
    (
        "404 Not Found",
        "text/plain; charset=utf-8",
        b"not found".to_vec(),
    )
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("webmanifest") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("md") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
            if let Ok(value) = u8::from_str_radix(hex, 16) {
                output.push(value);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination)
        .map_err(|error| format!("could not create {}: {error}", destination.display()))?;
    let entries = std::fs::read_dir(source)
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read an entry: {error}"))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|error| format!("could not copy {}: {error}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_handles_escapes() {
        assert_eq!(percent_decode("/a%20b"), "/a b");
        assert_eq!(percent_decode("/plain"), "/plain");
        assert_eq!(percent_decode("/%2e%2e/x"), "/../x");
    }

    #[test]
    fn content_types_are_assigned() {
        assert_eq!(content_type(Path::new("a.wasm")), "application/wasm");
        assert_eq!(
            content_type(Path::new("a.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type(Path::new("a.unknown")),
            "application/octet-stream"
        );
    }

    #[test]
    fn traversal_is_refused() {
        let (status, _, _) = response_for(Path::new("."), "/../Cargo.toml");
        assert_eq!(status, "404 Not Found");
    }
}
