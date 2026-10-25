// SPDX-License-Identifier: MIT OR Apache-2.0

//! GPX serialisation.
//!
//! The output is hand-written rather than produced by a generic XML writer,
//! because the document shape is fixed and small and because byte-for-byte
//! determinism matters for the fixture tests. Every value is escaped through
//! [`escape_xml`], so a station name containing `&` or `<` cannot corrupt the
//! document.

use std::fmt::Write as _;

use crate::document::{GpxDocument, GpxTrackPoint, GpxWaypoint};
use crate::error::GpxError;
use crate::{CREATOR, GPX_VERSION};

/// Controls what the writer emits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WriteOptions {
    /// Emit `<wpt>` elements for the document's waypoints.
    pub include_waypoints: bool,
    /// Emit a `<trk>` element for the document's segments.
    pub include_track: bool,
    /// Decimal places for latitude and longitude.
    ///
    /// Seven is about one centimetre at Harz latitudes and matches the
    /// precision of the operator's own file.
    pub coordinate_decimals: u8,
    /// Decimal places for elevation.
    pub elevation_decimals: u8,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            include_waypoints: true,
            include_track: true,
            coordinate_decimals: 7,
            elevation_decimals: 1,
        }
    }
}

/// Serialises a document to GPX 1.1 XML.
pub fn write_gpx(document: &GpxDocument, options: &WriteOptions) -> Result<String, GpxError> {
    let has_waypoints = options.include_waypoints && !document.waypoints.is_empty();
    let has_track = options.include_track && document.segments.iter().any(|s| !s.points.is_empty());
    if !has_waypoints && !has_track {
        return Err(GpxError::EmptyDocument);
    }
    if options.coordinate_decimals > 12 || options.elevation_decimals > 6 {
        return Err(GpxError::InvalidNumber {
            value: options.coordinate_decimals.to_string(),
            context: "write options",
        });
    }

    let mut out = String::with_capacity(4096);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<gpx version=\"{GPX_VERSION}\" creator=\"{}\" xmlns=\"http://www.topografix.com/GPX/1/1\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xsi:schemaLocation=\"http://www.topografix.com/GPX/1/1 \
         http://www.topografix.com/GPX/1/1/gpx.xsd\">",
        escape_xml(CREATOR)
    );

    let has_metadata =
        document.name.is_some() || document.description.is_some() || document.time.is_some();
    if has_metadata {
        out.push_str("  <metadata>\n");
        if let Some(name) = &document.name {
            let _ = writeln!(out, "    <name>{}</name>", escape_xml(name));
        }
        if let Some(description) = &document.description {
            let _ = writeln!(out, "    <desc>{}</desc>", escape_xml(description));
        }
        if let Some(time) = &document.time {
            // Timestamps are emitted verbatim; validating ISO 8601 here would
            // duplicate the schema for no benefit to our own exports.
            let _ = writeln!(out, "    <time>{}</time>", escape_xml(time));
        }
        out.push_str("  </metadata>\n");
    }

    if has_waypoints {
        for waypoint in &document.waypoints {
            write_waypoint(&mut out, waypoint, options);
        }
    }

    if has_track {
        let track_name = document.name.as_deref().unwrap_or("Route");
        out.push_str("  <trk>\n");
        let _ = writeln!(out, "    <name>{}</name>", escape_xml(track_name));
        for segment in &document.segments {
            if segment.points.is_empty() {
                continue;
            }
            out.push_str("    <trkseg>\n");
            for point in &segment.points {
                write_track_point(&mut out, point, options);
            }
            out.push_str("    </trkseg>\n");
        }
        out.push_str("  </trk>\n");
    }

    out.push_str("</gpx>\n");
    Ok(out)
}

fn write_waypoint(out: &mut String, waypoint: &GpxWaypoint, options: &WriteOptions) {
    let _ = writeln!(
        out,
        "  <wpt lat=\"{}\" lon=\"{}\">",
        format_coordinate(waypoint.position.latitude(), options.coordinate_decimals),
        format_coordinate(waypoint.position.longitude(), options.coordinate_decimals)
    );
    if let Some(elevation) = waypoint.elevation_m {
        let _ = writeln!(
            out,
            "    <ele>{}</ele>",
            format_coordinate(elevation, options.elevation_decimals)
        );
    }
    let _ = writeln!(out, "    <name>{}</name>", escape_xml(&waypoint.name));
    if let Some(comment) = &waypoint.comment {
        let _ = writeln!(out, "    <cmt>{}</cmt>", escape_xml(comment));
    }
    if let Some(description) = &waypoint.description {
        let _ = writeln!(out, "    <desc>{}</desc>", escape_xml(description));
    }
    if let Some(symbol) = &waypoint.symbol {
        let _ = writeln!(out, "    <sym>{}</sym>", escape_xml(symbol));
    }
    out.push_str("  </wpt>\n");
}

fn write_track_point(out: &mut String, point: &GpxTrackPoint, options: &WriteOptions) {
    let _ = write!(
        out,
        "      <trkpt lat=\"{}\" lon=\"{}\">",
        format_coordinate(point.position.latitude(), options.coordinate_decimals),
        format_coordinate(point.position.longitude(), options.coordinate_decimals)
    );
    if let Some(elevation) = point.elevation_m {
        let _ = write!(
            out,
            "<ele>{}</ele>",
            format_coordinate(elevation, options.elevation_decimals)
        );
    }
    out.push_str("</trkpt>\n");
}

/// Formats a number with a fixed number of decimals, normalising `-0.0`.
fn format_coordinate(value: f64, decimals: u8) -> String {
    let precision = usize::from(decimals);
    let formatted = format!("{value:.precision$}");
    // `-0.0000000` is valid but confusing in a diff and on some devices.
    if formatted.starts_with('-') && formatted.parse::<f64>().is_ok_and(|parsed| parsed == 0.0) {
        formatted.trim_start_matches('-').to_string()
    } else {
        formatted
    }
}

/// Escapes text for use in XML character data.
///
/// Also strips characters that XML 1.0 forbids outright, so a control character
/// in a station name cannot produce a file that no parser will accept.
#[must_use]
pub fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            // Legal XML whitespace is kept; other C0 controls are dropped.
            '\t' | '\n' | '\r' => escaped.push(character),
            character if (character as u32) < 0x20 => {}
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_engine::LatLon;

    fn position(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    fn sample_document() -> GpxDocument {
        let mut document = GpxDocument::new("Test tour");
        let mut waypoint =
            GpxWaypoint::new("HWN001 Eckertalsperre", position(51.841_649, 10.579_975_7));
        waypoint.elevation_m = Some(555.0);
        waypoint.comment = Some("Eckertalsperre (Staumauer)".to_string());
        waypoint.symbol = Some("Flag, Blue".to_string());
        document.push_waypoint(waypoint);
        document.push_segment_from_positions(&[
            position(51.841_649, 10.579_975_7),
            position(51.830_167_7, 10.602_774_2),
        ]);
        document
    }

    #[test]
    fn output_has_the_gpx_skeleton() {
        let xml = write_gpx(&sample_document(), &WriteOptions::default()).expect("write");
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(xml.contains("<gpx version=\"1.1\""));
        assert!(xml.contains("xmlns=\"http://www.topografix.com/GPX/1/1\""));
        assert!(xml.ends_with("</gpx>\n"));
        assert!(xml.contains(&format!("creator=\"{CREATOR}\"")));
    }

    #[test]
    fn waypoints_and_track_are_written() {
        let xml = write_gpx(&sample_document(), &WriteOptions::default()).expect("write");
        assert!(xml.contains("<wpt lat=\"51.8416490\" lon=\"10.5799757\">"));
        assert!(xml.contains("<ele>555.0</ele>"));
        assert!(xml.contains("<name>HWN001 Eckertalsperre</name>"));
        assert!(xml.contains("<cmt>Eckertalsperre (Staumauer)</cmt>"));
        assert!(xml.contains("<sym>Flag, Blue</sym>"));
        assert_eq!(xml.matches("<trkpt").count(), 2);
        assert_eq!(xml.matches("<trkseg>").count(), 1);
    }

    #[test]
    fn special_characters_are_escaped() {
        let mut document = GpxDocument::new("A & B <test>");
        document.push_waypoint(GpxWaypoint::new(
            "Quotes \" and ' apostrophes",
            position(51.8, 10.6),
        ));
        let xml = write_gpx(&document, &WriteOptions::default()).expect("write");
        assert!(xml.contains("A &amp; B &lt;test&gt;"));
        assert!(xml.contains("Quotes &quot; and &apos; apostrophes"));
        assert!(!xml.contains("<test>"));
    }

    #[test]
    fn control_characters_are_dropped() {
        let mut document = GpxDocument::new("bad\u{0}name");
        document.push_waypoint(GpxWaypoint::new("stamp\u{7}", position(51.8, 10.6)));
        let xml = write_gpx(&document, &WriteOptions::default()).expect("write");
        assert!(!xml.contains('\u{0}'));
        assert!(!xml.contains('\u{7}'));
    }

    #[test]
    fn output_is_deterministic() {
        let document = sample_document();
        let first = write_gpx(&document, &WriteOptions::default()).expect("write");
        let second = write_gpx(&document, &WriteOptions::default()).expect("write");
        assert_eq!(first, second);
    }

    #[test]
    fn no_time_element_is_written_by_default() {
        let xml = write_gpx(&sample_document(), &WriteOptions::default()).expect("write");
        assert!(!xml.contains("<time>"));
    }

    #[test]
    fn supplied_time_is_written() {
        let mut document = sample_document();
        document.time = Some("2026-01-01T00:00:00Z".to_string());
        let xml = write_gpx(&document, &WriteOptions::default()).expect("write");
        assert!(xml.contains("<time>2026-01-01T00:00:00Z</time>"));
    }

    #[test]
    fn empty_document_is_rejected() {
        let document = GpxDocument::default();
        assert!(matches!(
            write_gpx(&document, &WriteOptions::default()),
            Err(GpxError::EmptyDocument)
        ));
    }

    #[test]
    fn waypoints_only_document_is_allowed() {
        let mut document = GpxDocument::new("stamps");
        document.push_waypoint(GpxWaypoint::new("HWN001", position(51.8, 10.6)));
        let options = WriteOptions {
            include_track: false,
            ..Default::default()
        };
        let xml = write_gpx(&document, &options).expect("write");
        assert!(xml.contains("<wpt"));
        assert!(!xml.contains("<trk>"));
    }

    #[test]
    fn track_only_document_is_allowed() {
        let mut document = GpxDocument::new("track");
        document.push_segment_from_positions(&[position(51.8, 10.6), position(51.9, 10.7)]);
        let options = WriteOptions {
            include_waypoints: false,
            ..Default::default()
        };
        let xml = write_gpx(&document, &options).expect("write");
        assert!(!xml.contains("<wpt"));
        assert!(xml.contains("<trk>"));
    }

    #[test]
    fn negative_zero_is_normalised() {
        assert_eq!(format_coordinate(-0.0, 7), "0.0000000");
        assert_eq!(format_coordinate(-0.5, 2), "-0.50");
    }

    #[test]
    fn coordinate_precision_is_configurable() {
        let options = WriteOptions {
            coordinate_decimals: 5,
            ..Default::default()
        };
        let xml = write_gpx(&sample_document(), &options).expect("write");
        assert!(xml.contains("lat=\"51.84165\""));
    }

    #[test]
    fn excessive_precision_is_rejected() {
        let options = WriteOptions {
            coordinate_decimals: 20,
            ..Default::default()
        };
        assert!(write_gpx(&sample_document(), &options).is_err());
    }
}
