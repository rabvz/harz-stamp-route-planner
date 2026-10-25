// SPDX-License-Identifier: MIT OR Apache-2.0

//! Regenerates the station catalogue from the operator's published GPX file.
//!
//! The catalogue committed in `data/catalog/` is generated, never hand-edited.
//! `cargo xtask data --check` (part of `cargo xtask ci`) fails when the file no
//! longer matches its source, which keeps the dataset and its provenance from
//! drifting apart.

use std::fmt::Write as _;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::workspace_root;

/// Provenance metadata committed next to the source file.
#[derive(Debug, Deserialize)]
struct Provenance {
    dataset_id: String,
    dataset_name: String,
    source_url: String,
    source_file: String,
    source_sha256: String,
    released: String,
    retrieved: String,
    rights_note: String,
    expected_station_count: usize,
}

/// One generated catalogue entry.
struct Entry {
    id: u16,
    code: String,
    name: String,
    location: String,
    lat: f64,
    lon: f64,
    elevation_m: Option<f64>,
}

/// Generates the catalogue, or verifies it when `check_only` is set.
pub fn generate(check_only: bool) -> Result<(), String> {
    let root = workspace_root();
    let provenance_path = root.join("data/official/PROVENANCE.json");
    let provenance_text = std::fs::read_to_string(&provenance_path)
        .map_err(|error| format!("could not read {}: {error}", provenance_path.display()))?;
    let provenance: Provenance = serde_json::from_str(&provenance_text)
        .map_err(|error| format!("{} is not valid: {error}", provenance_path.display()))?;

    let source_path = root.join(&provenance.source_file);
    let source_bytes = std::fs::read(&source_path)
        .map_err(|error| format!("could not read {}: {error}", source_path.display()))?;

    // Verify the source is byte-identical to what the provenance record claims.
    let digest = hex(&Sha256::digest(&source_bytes));
    if digest != provenance.source_sha256.to_lowercase() {
        return Err(format!(
            "{} has SHA-256 {digest}, but PROVENANCE.json records {}\n\
             Update the source file and its provenance together.",
            provenance.source_file, provenance.source_sha256
        ));
    }

    let source_text = String::from_utf8(source_bytes)
        .map_err(|error| format!("{} is not valid UTF-8: {error}", provenance.source_file))?;
    let document = gpx::parse_gpx(&source_text)
        .map_err(|error| format!("{} could not be parsed: {error}", provenance.source_file))?;

    let entries = build_entries(&document)?;
    if entries.len() != provenance.expected_station_count {
        return Err(format!(
            "{} contains {} stations, but PROVENANCE.json expects {}",
            provenance.source_file,
            entries.len(),
            provenance.expected_station_count
        ));
    }
    verify_contiguous(&entries)?;

    let rendered = render(&provenance, &entries);
    let target = root.join("data/catalog/hwn-2025-stamps.json");

    if check_only {
        let existing = std::fs::read_to_string(&target).unwrap_or_default();
        if normalize_newlines(&existing) == rendered {
            println!("catalogue is up to date ({} stations)", entries.len());
            return Ok(());
        }
        return Err(format!(
            "{} is out of date. Run `cargo xtask data` and commit the result.",
            target.display()
        ));
    }

    std::fs::write(&target, rendered)
        .map_err(|error| format!("could not write {}: {error}", target.display()))?;
    println!("wrote {} ({} stations)", target.display(), entries.len());
    Ok(())
}

/// Turns parsed waypoints into catalogue entries.
fn build_entries(document: &gpx::GpxDocument) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::with_capacity(document.waypoints.len());
    for waypoint in &document.waypoints {
        let raw = waypoint.name.trim();
        // Source names look like "HWN001 Eckertalsperre".
        let (code, name) = raw.split_once(char::is_whitespace).ok_or_else(|| {
            format!("waypoint {raw:?} does not follow the \"HWNnnn Name\" convention")
        })?;
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("waypoint {code} has an empty name"));
        }
        if !code.starts_with("HWN") || code.len() != 6 {
            return Err(format!("waypoint code {code:?} is not of the form HWNnnn"));
        }
        let number: u16 = code[3..]
            .parse()
            .map_err(|_| format!("waypoint code {code:?} has a non-numeric suffix"))?;

        let location = waypoint
            .comment
            .as_deref()
            .or(waypoint.description.as_deref())
            .map(normalize_whitespace)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| name.to_string());

        entries.push(Entry {
            id: number,
            code: code.to_string(),
            name: normalize_whitespace(name),
            location,
            lat: waypoint.position.latitude(),
            lon: waypoint.position.longitude(),
            elevation_m: waypoint.elevation_m,
        });
    }
    entries.sort_by_key(|entry| entry.id);
    Ok(entries)
}

/// Station numbers must run from 1 to N without gaps or duplicates.
fn verify_contiguous(entries: &[Entry]) -> Result<(), String> {
    for (index, entry) in entries.iter().enumerate() {
        let expected = u16::try_from(index + 1).map_err(|_| "too many stations")?;
        if entry.id != expected {
            return Err(format!(
                "station numbering is not contiguous: expected HWN{expected:03}, found {}",
                entry.code
            ));
        }
    }
    Ok(())
}

/// Renders the catalogue document.
fn render(provenance: &Provenance, entries: &[Entry]) -> String {
    let mut out = String::with_capacity(entries.len() * 160 + 512);
    out.push_str("{\n  \"schema_version\": 1,\n  \"dataset\": {\n");
    let _ = writeln!(
        out,
        "    \"id\": \"{}\",",
        json_escape(&provenance.dataset_id)
    );
    let _ = writeln!(
        out,
        "    \"name\": \"{}\",",
        json_escape(&provenance.dataset_name)
    );
    let _ = writeln!(
        out,
        "    \"source_url\": \"{}\",",
        json_escape(&provenance.source_url)
    );
    let _ = writeln!(
        out,
        "    \"source_file\": \"{}\",",
        json_escape(&provenance.source_file)
    );
    let _ = writeln!(
        out,
        "    \"source_sha256\": \"{}\",",
        json_escape(&provenance.source_sha256.to_lowercase())
    );
    let _ = writeln!(
        out,
        "    \"released\": \"{}\",",
        json_escape(&provenance.released)
    );
    let _ = writeln!(
        out,
        "    \"retrieved\": \"{}\",",
        json_escape(&provenance.retrieved)
    );
    let _ = writeln!(
        out,
        "    \"rights_note\": \"{}\"",
        json_escape(&provenance.rights_note)
    );
    out.push_str("  },\n  \"stamps\": [\n");

    for (index, entry) in entries.iter().enumerate() {
        let elevation = match entry.elevation_m {
            Some(value) => format_decimal(value, 1),
            None => "null".to_string(),
        };
        let _ = write!(
            out,
            "    {{ \"id\": {}, \"code\": \"{}\", \"name\": \"{}\", \"location\": \"{}\", \
             \"lat\": {}, \"lon\": {}, \"elevation_m\": {} }}",
            entry.id,
            json_escape(&entry.code),
            json_escape(&entry.name),
            json_escape(&entry.location),
            format_decimal(entry.lat, 7),
            format_decimal(entry.lon, 7),
            elevation
        );
        if index + 1 < entries.len() {
            out.push(',');
        }
        out.push('\n');
    }

    out.push_str("  ]\n}\n");
    out
}

/// Formats a decimal with at most `decimals` places, dropping trailing zeros.
///
/// Coordinates are stored as published (seven decimals) and shortened to their
/// shortest exact representation so the file stays readable.
fn format_decimal(value: f64, decimals: usize) -> String {
    // Normalise negative zero: `-0.0` is a valid float but looks like a bug in
    // a diff, and `0.0 == -0.0` is true, so one branch covers both.
    if value == 0.0 {
        return "0.0".to_string();
    }
    let mut formatted = format!("{value:.decimals$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            // Keep one decimal so the value stays visibly a float, matching
            // "555.0" in the dataset and json_serde's float rendering.
            formatted.push('0');
        }
    }
    formatted
}

/// Collapses any run of whitespace into a single space and trims the result.
fn normalize_whitespace(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character);
        }
    }
    normalized
}

/// Escapes a string for a JSON string literal.
fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                let _ = write!(escaped, "\\u{:04x}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(normalize_whitespace("  a  b \n c  "), "a b c");
        assert_eq!(
            normalize_whitespace("Wasserscheide\r\n„Hohe Tür“"),
            "Wasserscheide „Hohe Tür“"
        );
    }

    #[test]
    fn decimals_are_trimmed_but_keep_one_place() {
        assert_eq!(format_decimal(51.844_15, 7), "51.84415");
        assert_eq!(format_decimal(555.0, 1), "555.0");
        assert_eq!(format_decimal(51.841_649, 7), "51.841649");
        assert_eq!(format_decimal(-0.0, 7), "0.0");
    }

    #[test]
    fn json_escaping_handles_quotes_and_control_characters() {
        assert_eq!(json_escape("a\"b"), "a\\\"b");
        assert_eq!(json_escape("a\\b"), "a\\\\b");
        assert_eq!(json_escape("a\u{1}b"), "a\\u0001b");
    }

    #[test]
    fn hex_encodes_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }
}
