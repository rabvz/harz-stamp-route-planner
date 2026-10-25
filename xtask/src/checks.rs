// SPDX-License-Identifier: MIT OR Apache-2.0

//! Policy checks: crate layering, licence headers and GPX fixtures.

use std::path::{Path, PathBuf};

use crate::workspace_root;

/// The only dependency edges the architecture allows.
///
/// Adding an edge here is an architecture decision and deserves its own pull
/// request; the check exists so an accidental `use` cannot quietly invert the
/// dependency direction.
const ALLOWED_INTERNAL_DEPENDENCIES: &[(&str, &[&str])] = &[
    ("geo-engine", &[]),
    ("stamp-core", &["geo-engine"]),
    ("parking-search", &["geo-engine", "stamp-core"]),
    ("stamp-clustering", &["geo-engine", "stamp-core"]),
    ("routing-client", &["geo-engine"]),
    (
        "route-optimizer",
        &["geo-engine", "stamp-core", "routing-client"],
    ),
    ("gpx", &["geo-engine", "stamp-core"]),
    (
        "wasm-bindings",
        &[
            "geo-engine",
            "gpx",
            "parking-search",
            "route-optimizer",
            "routing-client",
            "stamp-clustering",
            "stamp-core",
        ],
    ),
    ("xtask", &["gpx", "stamp-core"]),
];

/// The SPDX identifier every source file must declare.
const SPDX_HEADER: &str = "// SPDX-License-Identifier: MIT OR Apache-2.0";

/// Verifies the documented crate layering.
pub fn check_architecture() -> Result<(), String> {
    let root = workspace_root();
    let mut problems = Vec::new();

    for (crate_name, allowed) in ALLOWED_INTERNAL_DEPENDENCIES {
        let manifest = manifest_path(&root, crate_name);
        let text = std::fs::read_to_string(&manifest)
            .map_err(|error| format!("could not read {}: {error}", manifest.display()))?;
        let dependencies = declared_dependencies(&text);

        for dependency in &dependencies {
            let is_internal = ALLOWED_INTERNAL_DEPENDENCIES
                .iter()
                .any(|(name, _)| name == dependency);
            if is_internal && !allowed.contains(&dependency.as_str()) {
                problems.push(format!(
                    "{crate_name} must not depend on {dependency}; allowed: [{}]",
                    allowed.join(", ")
                ));
            }
        }

        // Only the boundary crate may produce a C dynamic library.
        if *crate_name != "wasm-bindings" && text.contains("cdylib") {
            problems.push(format!(
                "{crate_name} declares a cdylib target; only wasm-bindings may do that"
            ));
        }
    }

    // The libraries must never depend on the browser boundary.
    for (crate_name, _) in ALLOWED_INTERNAL_DEPENDENCIES {
        if *crate_name == "wasm-bindings" {
            continue;
        }
        let manifest = manifest_path(&root, crate_name);
        let text = std::fs::read_to_string(&manifest)
            .map_err(|error| format!("could not read {}: {error}", manifest.display()))?;
        if declared_dependencies(&text)
            .iter()
            .any(|dependency| dependency == "wasm-bindings")
        {
            problems.push(format!(
                "{crate_name} depends on wasm-bindings, which inverts the layering"
            ));
        }
    }

    report("architecture", problems)
}

/// Verifies licence headers and manifest metadata.
pub fn check_licenses() -> Result<(), String> {
    let root = workspace_root();
    let mut problems = Vec::new();

    for licence in ["LICENSE-MIT", "LICENSE-APACHE", "ATTRIBUTION.md"] {
        if !root.join(licence).is_file() {
            problems.push(format!("{licence} is missing at the repository root"));
        }
    }

    for directory in ["crates", "xtask"] {
        for source in collect_sources(&root.join(directory))? {
            let text = std::fs::read_to_string(&source)
                .map_err(|error| format!("could not read {}: {error}", source.display()))?;
            let first_line = text.lines().next().unwrap_or_default().trim_end();
            if first_line != SPDX_HEADER {
                problems.push(format!(
                    "{} must start with {SPDX_HEADER:?}, found {first_line:?}",
                    relative(&root, &source)
                ));
            }
        }
    }

    let root_manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("could not read the workspace manifest: {error}"))?;
    if !root_manifest.contains("license = \"MIT OR Apache-2.0\"") {
        problems.push("the workspace manifest must declare license = \"MIT OR Apache-2.0\"".into());
    }
    for (crate_name, _) in ALLOWED_INTERNAL_DEPENDENCIES {
        let manifest = manifest_path(&root, crate_name);
        let text = std::fs::read_to_string(&manifest)
            .map_err(|error| format!("could not read {}: {error}", manifest.display()))?;
        if !text.contains("license.workspace = true")
            && !text.contains("license = \"MIT OR Apache-2.0\"")
        {
            problems.push(format!(
                "{crate_name}/Cargo.toml does not inherit the workspace licence"
            ));
        }
    }

    report("licences", problems)
}

/// Verifies GPX reading and writing against real and synthetic data.
pub fn check_gpx() -> Result<(), String> {
    let root = workspace_root();
    let mut problems = Vec::new();

    // 1. The operator's published file must parse completely.
    let source_path = root.join("data/official/HWN2025.gpx");
    let source = std::fs::read_to_string(&source_path)
        .map_err(|error| format!("could not read {}: {error}", source_path.display()))?;
    match gpx::parse_gpx(&source) {
        Ok(document) => {
            if document.waypoints.len() != 222 {
                problems.push(format!(
                    "official GPX yielded {} waypoints, expected 222",
                    document.waypoints.len()
                ));
            }
            if let Some(first) = document.waypoints.first() {
                if !first.name.starts_with("HWN001") {
                    problems.push(format!(
                        "official GPX first waypoint is {:?}, expected HWN001",
                        first.name
                    ));
                }
            }
        }
        Err(error) => problems.push(format!("official GPX failed to parse: {error}")),
    }

    // 2. A catalogue-derived export must round-trip without loss.
    let catalog = stamp_core::StampCatalog::embedded();
    let mut document = gpx::GpxDocument::new("GPX round-trip fixture");
    for stamp in catalog.stamps().iter().take(5) {
        let mut waypoint =
            gpx::GpxWaypoint::new(format!("{} {}", stamp.code, stamp.name), stamp.position);
        waypoint.elevation_m = stamp.elevation_m;
        waypoint.comment = Some(stamp.location.clone());
        document.push_waypoint(waypoint);
    }
    document.push_segment_from_positions(
        &catalog
            .stamps()
            .iter()
            .take(3)
            .map(|stamp| stamp.position)
            .collect::<Vec<_>>(),
    );

    let options = gpx::WriteOptions::default();
    match gpx::write_gpx(&document, &options) {
        Ok(xml) => {
            if xml.contains("gpxx:") {
                problems.push("exported GPX contains vendor extension namespaces".to_string());
            }
            match gpx::parse_gpx(&xml) {
                Ok(parsed) => {
                    if parsed.waypoints.len() != 5 {
                        problems.push(format!(
                            "round-trip changed the waypoint count to {}",
                            parsed.waypoints.len()
                        ));
                    }
                    if parsed.track_point_count() != 3 {
                        problems.push(format!(
                            "round-trip changed the track point count to {}",
                            parsed.track_point_count()
                        ));
                    }
                    for (original, reparsed) in document.waypoints.iter().zip(&parsed.waypoints) {
                        if original.name != reparsed.name {
                            problems.push(format!(
                                "round-trip changed a name: {:?} -> {:?}",
                                original.name, reparsed.name
                            ));
                        }
                        let lat_delta =
                            (original.position.latitude() - reparsed.position.latitude()).abs();
                        let lon_delta =
                            (original.position.longitude() - reparsed.position.longitude()).abs();
                        if lat_delta > 1e-7 || lon_delta > 1e-7 {
                            problems.push(format!(
                                "round-trip moved {:?} by {lat_delta},{lon_delta} degrees",
                                original.name
                            ));
                        }
                    }
                }
                Err(error) => problems.push(format!("exported GPX failed to parse: {error}")),
            }
        }
        Err(error) => problems.push(format!("export failed: {error}")),
    }

    report("gpx", problems)
}

fn report(name: &str, problems: Vec<String>) -> Result<(), String> {
    if problems.is_empty() {
        println!("{name}: ok");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  - {problem}");
    }
    Err(format!("{name} check found {} problem(s)", problems.len()))
}

fn manifest_path(root: &Path, crate_name: &str) -> PathBuf {
    if crate_name == "xtask" {
        root.join("xtask/Cargo.toml")
    } else {
        root.join("crates").join(crate_name).join("Cargo.toml")
    }
}

/// Extracts dependency names from the `[dependencies]` section of a manifest.
///
/// A deliberately small parser: it understands the table form this workspace
/// uses and ignores anything more exotic rather than guessing.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut dependencies = Vec::new();
    let mut in_dependencies = false;

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = trimmed == "[dependencies]";
            continue;
        }
        if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            let key = key.trim();
            if !key.is_empty() {
                dependencies.push(key.to_string());
            }
        }
    }
    dependencies
}

fn collect_sources(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut sources = Vec::new();
    if !directory.is_dir() {
        return Ok(sources);
    }
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read an entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            sources.extend(collect_sources(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
    Ok(sources)
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A credential pattern that must never appear in a tracked file.
struct SecretPattern {
    /// What the pattern detects, for the error message.
    name: &'static str,
    /// Text that must appear before the match for it to count, if any.
    prefix: &'static str,
    /// Number of characters that follow the prefix.
    length: usize,
    /// Characters allowed in the sensitive part.
    allowed: &'static [&'static str],
}

/// PEM banner fragments.
///
/// Split into two constants on purpose: a single literal would make this file a
/// hit for its own scan. That kind of self-reference is exactly why the check
/// needs a suppression-free formulation.
const PEM_BANNER: &str = "-----BEGIN";
const PEM_KEY_LABEL: &str = "PRIVATE KEY";

/// High-signal credential shapes. Deliberately narrow: a false positive in CI
/// is worse than missing an exotic format, and dedicated secret scanning on
/// GitHub covers the rest.
const SECRET_PATTERNS: &[SecretPattern] = &[
    SecretPattern {
        name: "AWS access key id",
        prefix: "AKIA",
        length: 16,
        allowed: &["ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"],
    },
    SecretPattern {
        name: "GitHub token",
        prefix: "ghp_",
        length: 30,
        allowed: &["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"],
    },
    SecretPattern {
        name: "GitHub fine-grained token",
        prefix: "github_pat_",
        length: 20,
        allowed: &["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_"],
    },
    SecretPattern {
        name: "Slack token",
        prefix: "xoxb-",
        length: 12,
        allowed: &["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-"],
    },
    SecretPattern {
        name: "Google API key",
        prefix: "AIza",
        length: 35,
        allowed: &["ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-"],
    },
];

/// Scans the working tree for accidentally committed credentials.
///
/// This complements GitHub's own secret scanning: it runs locally as part of
/// `cargo xtask ci`, so a key is caught before it ever reaches a commit that
/// someone forgets to make private. Matches are redacted in the output — a
/// check that prints the secret into a CI log would leak what it protects.
pub fn check_secrets() -> Result<(), String> {
    let root = workspace_root();
    let mut problems = Vec::new();
    let mut scanned = 0_usize;

    scan_for_secrets(&root, &root, &mut problems, &mut scanned)?;

    if problems.is_empty() {
        println!("secrets: ok ({scanned} files scanned)");
        return Ok(());
    }
    for problem in &problems {
        eprintln!("  - {problem}");
    }
    Err(format!(
        "secret scan found {} potential credential(s); remove them and rotate the key",
        problems.len()
    ))
}

fn scan_for_secrets(
    root: &Path,
    directory: &Path,
    problems: &mut Vec<String>,
    scanned: &mut usize,
) -> Result<(), String> {
    const SKIPPED: &[&str] = &[
        "target",
        ".git",
        ".toolchain",
        "node_modules",
        "dist",
        "pkg",
        "vendor",
    ];

    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read an entry: {error}"))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if SKIPPED.contains(&name.as_str()) {
                continue;
            }
            scan_for_secrets(root, &path, problems, scanned)?;
            continue;
        }

        // Only inspect small text files; binaries and the vendored minified
        // library are not places we would have put a key.
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if metadata.len() > 1_000_000 {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if bytes.iter().take(8192).any(|byte| *byte == 0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        *scanned += 1;

        for (line_number, line) in text.lines().enumerate() {
            // Test fixtures and documentation sometimes need to show a token
            // shape. They opt out explicitly so the marker is visible in review.
            if line.contains("secret-scan: allow") {
                continue;
            }
            if let Some(finding) = find_secret(line) {
                problems.push(format!(
                    "{}:{}: possible {} ({})",
                    relative(root, &path),
                    line_number + 1,
                    finding.0,
                    finding.1
                ));
            }
            if line.contains(PEM_BANNER) && line.contains(PEM_KEY_LABEL) {
                problems.push(format!(
                    "{}:{}: private key block",
                    relative(root, &path),
                    line_number + 1
                ));
            }
        }
    }
    Ok(())
}

/// Returns the pattern name and a redacted excerpt when `line` contains one.
fn find_secret(line: &str) -> Option<(&'static str, String)> {
    for pattern in SECRET_PATTERNS {
        let Some(start) = line.find(pattern.prefix) else {
            continue;
        };
        let tail = &line[start + pattern.prefix.len()..];
        let candidate: String = tail.chars().take(pattern.length).collect();
        if candidate.chars().count() == pattern.length
            && candidate
                .chars()
                .all(|character| pattern.allowed.iter().any(|set| set.contains(character)))
        {
            let redacted = format!("{}… ({} chars)", pattern.prefix, candidate.len());
            return Some((pattern.name, redacted));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_parsing_reads_the_table_form() {
        let manifest = "[package]\nname = \"x\"\n\n[dependencies]\ngeo-engine = { workspace = true }\nserde = \"1.0\"\n\n[dev-dependencies]\nserde_json = \"1\"\n";
        assert_eq!(declared_dependencies(manifest), vec!["geo-engine", "serde"]);
    }

    #[test]
    fn dependency_parsing_ignores_comments_and_blank_lines() {
        let manifest = "[dependencies]\n# a comment\n\nserde = \"1\"\n";
        assert_eq!(declared_dependencies(manifest), vec!["serde"]);
    }

    #[test]
    fn every_allowed_crate_has_an_entry() {
        for (name, allowed) in ALLOWED_INTERNAL_DEPENDENCIES {
            assert!(
                !allowed.contains(name),
                "{name} must not list itself as a dependency"
            );
        }
    }

    #[test]
    fn github_tokens_are_detected_and_redacted() {
        // Assembled at runtime so this file does not contain a token-shaped
        // literal that its own scan would (correctly) report.
        let token = ["ghp_", "abcdefghijklmnopqrstuvwxyz", "0123"].concat();
        let line = format!("const token = \"{token}\";");
        let finding = find_secret(&line).expect("token should be detected");
        assert_eq!(finding.0, "GitHub token");
        assert!(
            !finding.1.contains("abcdefghij"),
            "the excerpt must be redacted"
        );
    }

    #[test]
    fn aws_key_ids_are_detected() {
        let key = ["AKIA", "IOSFODNN7EXAMPLE"].concat();
        assert!(find_secret(&key).is_some());
    }

    #[test]
    fn ordinary_text_is_not_flagged() {
        for line in [
            "this file contains no credentials",
            "AKIA is a prefix, but too short here",
            "ghp_ not followed by thirty characters",
            "https://example.org/path?query=1",
        ] {
            assert!(find_secret(line).is_none(), "false positive for {line:?}");
        }
    }
}
