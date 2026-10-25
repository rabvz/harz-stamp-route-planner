// SPDX-License-Identifier: MIT OR Apache-2.0

//! GPX parsing.
//!
//! The parser is a small event-driven state machine over `quick-xml`. It reads
//! only the elements this project needs (`wpt`, `trkseg`/`trkpt`, `metadata`)
//! and ignores everything else, including vendor extension blocks. Unknown
//! elements are skipped structurally rather than treated as errors, because
//! real-world GPX files from Garmin devices and phones carry plenty of them.
//!
//! Text content is accumulated per element so that entity references, CDATA
//! sections and split text nodes all end up in the same field.

use geo_engine::LatLon;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::document::{GpxDocument, GpxTrackPoint, GpxTrackSegment, GpxWaypoint};
use crate::error::GpxError;

/// Parses a GPX document.
pub fn parse_gpx(xml: &str) -> Result<GpxDocument, GpxError> {
    let mut reader = Reader::from_str(xml);
    {
        let config = reader.config_mut();
        // Trimming is done on the assembled value in `apply_field`, not per
        // text event: entity references split a text node into several events,
        // and trimming each fragment would delete the spaces around them
        // ("A &amp; B" would collapse to "A&B").
        config.trim_text_start = false;
        config.trim_text_end = false;
        config.check_end_names = true;
    }

    let mut document = GpxDocument::default();
    let mut stack: Vec<String> = Vec::new();
    let mut current_waypoint: Option<GpxWaypoint> = None;
    let mut current_segment: Option<GpxTrackSegment> = None;
    let mut pending_field: Option<String> = None;
    let mut text_buffer = String::new();
    let mut saw_gpx_root = false;

    loop {
        let event = reader.read_event().map_err(|error| GpxError::InvalidXml {
            message: error.to_string(),
            position: 0,
        })?;

        match event {
            Event::Start(start) => {
                let name = local_name(&start);
                if name == "gpx" {
                    saw_gpx_root = true;
                }
                stack.push(name.clone());

                if name == "wpt" {
                    current_waypoint = Some(waypoint_from_attributes(&start)?);
                } else if name == "trkseg" {
                    current_segment = Some(GpxTrackSegment::default());
                } else if name == "trkpt" {
                    let point = track_point_from_attributes(&start)?;
                    current_segment
                        .get_or_insert_with(GpxTrackSegment::default)
                        .points
                        .push(point);
                } else if is_captured_field(&name) {
                    pending_field = Some(name);
                    text_buffer.clear();
                }
            }
            Event::Empty(start) => {
                let name = local_name(&start);
                if name == "gpx" {
                    saw_gpx_root = true;
                }
                // Self-closing elements never contain text, so they are handled
                // completely and do not touch the element stack.
                if name == "wpt" {
                    let waypoint = waypoint_from_attributes(&start)?;
                    if waypoint.name.trim().is_empty() {
                        return Err(GpxError::UnnamedWaypoint {
                            index: document.waypoints.len(),
                        });
                    }
                    document.waypoints.push(waypoint);
                } else if name == "trkpt" {
                    let point = track_point_from_attributes(&start)?;
                    current_segment
                        .get_or_insert_with(GpxTrackSegment::default)
                        .points
                        .push(point);
                }
            }
            Event::End(end) => {
                let name = end.name().local_name().as_ref().to_string();
                let closing_position = stack.iter().rposition(|open| *open == name);
                if let Some(position) = closing_position {
                    stack.truncate(position);
                }

                match name.as_str() {
                    "wpt" => {
                        if let Some(mut waypoint) = current_waypoint.take() {
                            if waypoint.name.trim().is_empty() {
                                return Err(GpxError::UnnamedWaypoint {
                                    index: document.waypoints.len(),
                                });
                            }
                            // A Garmin <name> like "HWN001 Eckertalsperre" is
                            // kept verbatim; splitting it is the caller's job.
                            if waypoint.description.is_none() && waypoint.comment.is_some() {
                                waypoint.description = waypoint.comment.clone();
                            }
                            document.waypoints.push(waypoint);
                        }
                    }
                    "trkseg" => {
                        if let Some(segment) = current_segment.take() {
                            if !segment.points.is_empty() {
                                document.segments.push(segment);
                            }
                        }
                    }
                    "name" | "desc" | "cmt" | "sym" | "ele" | "time"
                        if pending_field.as_deref() == Some(name.as_str()) =>
                    {
                        let value = std::mem::take(&mut text_buffer);
                        apply_field(
                            &name,
                            &value,
                            &stack,
                            &mut current_waypoint,
                            &mut current_segment,
                            &mut document,
                        )?;
                        pending_field = None;
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if pending_field.is_some() {
                    text_buffer.push_str(text.xml10_content().as_ref());
                }
            }
            Event::CData(data) => {
                if pending_field.is_some() {
                    text_buffer.push_str(data.into_inner().as_ref());
                }
            }
            Event::GeneralRef(reference) => {
                if pending_field.is_some() {
                    if let Some(character) = resolve_reference(&reference.into_inner()) {
                        text_buffer.push(character);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if !saw_gpx_root {
        return Err(GpxError::NotGpx {
            message: "no <gpx> root element".to_string(),
        });
    }
    if document.waypoints.is_empty() && document.segments.is_empty() {
        return Err(GpxError::EmptyDocument);
    }
    Ok(document)
}

/// Applies a captured text value to the right target.
fn apply_field(
    field: &str,
    value: &str,
    stack: &[String],
    waypoint: &mut Option<GpxWaypoint>,
    segment: &mut Option<GpxTrackSegment>,
    document: &mut GpxDocument,
) -> Result<(), GpxError> {
    let value = value.trim().to_string();
    let in_waypoint = stack.iter().any(|name| name == "wpt");
    let in_trackpoint = stack.iter().any(|name| name == "trkpt");

    match field {
        "ele" => {
            if value.is_empty() {
                return Ok(());
            }
            let elevation: f64 = value.parse().map_err(|_| GpxError::InvalidNumber {
                value: value.clone(),
                context: "ele",
            })?;
            if !elevation.is_finite() {
                return Err(GpxError::InvalidNumber {
                    value,
                    context: "ele",
                });
            }
            if in_waypoint {
                if let Some(waypoint) = waypoint.as_mut() {
                    waypoint.elevation_m = Some(elevation);
                }
            } else if in_trackpoint {
                if let Some(point) = segment
                    .as_mut()
                    .and_then(|segment| segment.points.last_mut())
                {
                    point.elevation_m = Some(elevation);
                }
            }
        }
        "name" | "desc" | "cmt" | "sym" => {
            if in_waypoint {
                if let Some(waypoint) = waypoint.as_mut() {
                    set_waypoint_field(waypoint, field, value);
                }
            } else if !in_trackpoint && stack.iter().any(|name| name == "metadata") {
                match field {
                    "name" => document.name = Some(value),
                    "desc" => document.description = Some(value),
                    _ => {}
                }
            }
        }
        "time" if !in_waypoint && !in_trackpoint && stack.iter().any(|name| name == "metadata") => {
            document.time = Some(value);
        }
        _ => {}
    }
    Ok(())
}

fn set_waypoint_field(waypoint: &mut GpxWaypoint, field: &str, value: String) {
    match field {
        "name" => waypoint.name = value,
        "desc" => waypoint.description = Some(value),
        "cmt" => waypoint.comment = Some(value),
        "sym" => waypoint.symbol = Some(value),
        _ => {}
    }
}

/// Builds a waypoint from the `lat`/`lon` attributes of a `<wpt>` element.
fn waypoint_from_attributes(start: &BytesStart<'_>) -> Result<GpxWaypoint, GpxError> {
    let position = read_position(start, "wpt")?;
    Ok(GpxWaypoint::new(String::new(), position))
}

/// Builds a track point from the `lat`/`lon` attributes of a `<trkpt>`.
fn track_point_from_attributes(start: &BytesStart<'_>) -> Result<GpxTrackPoint, GpxError> {
    Ok(GpxTrackPoint {
        position: read_position(start, "trkpt")?,
        elevation_m: None,
    })
}

fn read_position(start: &BytesStart<'_>, context: &'static str) -> Result<LatLon, GpxError> {
    let lat = attribute(start, "lat")?.ok_or(GpxError::MissingElement {
        what: "lat attribute",
        context,
    })?;
    let lon = attribute(start, "lon")?.ok_or(GpxError::MissingElement {
        what: "lon attribute",
        context,
    })?;
    let latitude: f64 = lat
        .trim()
        .parse()
        .map_err(|_| GpxError::InvalidCoordinate {
            value: lat.clone(),
            context: "lat",
        })?;
    let longitude: f64 = lon
        .trim()
        .parse()
        .map_err(|_| GpxError::InvalidCoordinate {
            value: lon.clone(),
            context: "lon",
        })?;
    LatLon::new(latitude, longitude).map_err(|error| GpxError::InvalidCoordinate {
        value: format!("{lat},{lon} ({error})"),
        context,
    })
}

/// Reads an attribute by local name.
fn attribute(start: &BytesStart<'_>, name: &str) -> Result<Option<String>, GpxError> {
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| GpxError::InvalidXml {
            message: error.to_string(),
            position: 0,
        })?;
        if attribute.key.local_name().as_ref() == name {
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| GpxError::InvalidXml {
                    message: error.to_string(),
                    position: 0,
                })?;
            return Ok(Some(value.into_owned()));
        }
    }
    Ok(None)
}

/// Local element name without namespace prefix.
fn local_name(start: &BytesStart<'_>) -> String {
    start.name().local_name().as_ref().to_string()
}

/// Whether text content of this element is worth capturing.
fn is_captured_field(name: &str) -> bool {
    matches!(name, "name" | "desc" | "cmt" | "sym" | "ele" | "time")
}

/// Resolves a predefined or numeric entity reference.
fn resolve_reference(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = if let Some(hex) = digits
                .strip_prefix('x')
                .or_else(|| digits.strip_prefix('X'))
            {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                digits.parse().ok()?
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <metadata>
    <name>Tour</name>
    <desc>A test document</desc>
    <time>2026-01-01T10:00:00Z</time>
  </metadata>
  <wpt lat="51.8416490" lon="10.5799757">
    <ele>555.0</ele>
    <name>HWN001 Eckertalsperre</name>
    <cmt>Eckertalsperre (Staumauer)</cmt>
    <sym>Flag, Blue</sym>
  </wpt>
  <trk>
    <trkseg>
      <trkpt lat="51.8416490" lon="10.5799757"><ele>555.0</ele></trkpt>
      <trkpt lat="51.8301677" lon="10.6027742"/>
    </trkseg>
  </trk>
</gpx>
"#;

    #[test]
    fn parses_metadata_waypoints_and_track() {
        let document = parse_gpx(MINIMAL).expect("parse");
        assert_eq!(document.name.as_deref(), Some("Tour"));
        assert_eq!(document.description.as_deref(), Some("A test document"));
        assert_eq!(document.time.as_deref(), Some("2026-01-01T10:00:00Z"));
        assert_eq!(document.waypoints.len(), 1);
        assert_eq!(document.segments.len(), 1);
        assert_eq!(document.track_point_count(), 2);

        let waypoint = &document.waypoints[0];
        assert_eq!(waypoint.name, "HWN001 Eckertalsperre");
        assert_eq!(
            waypoint.comment.as_deref(),
            Some("Eckertalsperre (Staumauer)")
        );
        assert_eq!(waypoint.symbol.as_deref(), Some("Flag, Blue"));
        assert_eq!(waypoint.elevation_m, Some(555.0));
        assert!((waypoint.position.latitude() - 51.841_649).abs() < 1e-9);
    }

    #[test]
    fn track_point_elevation_is_attached_to_the_right_point() {
        let document = parse_gpx(MINIMAL).expect("parse");
        let points = document.track_points();
        assert_eq!(points[0].elevation_m, Some(555.0));
        assert_eq!(points[1].elevation_m, None);
    }

    #[test]
    fn missing_metadata_is_fine() {
        let xml = r#"<gpx version="1.1"><wpt lat="51.8" lon="10.6"><name>A</name></wpt></gpx>"#;
        let document = parse_gpx(xml).expect("parse");
        assert!(document.name.is_none());
        assert_eq!(document.waypoints.len(), 1);
    }

    #[test]
    fn self_closing_waypoint_is_parsed() {
        // A waypoint with no name is rejected, because a nameless stamp is not
        // something the UI can display.
        let xml = r#"<gpx version="1.1"><wpt lat="51.8" lon="10.6"/></gpx>"#;
        assert!(matches!(
            parse_gpx(xml),
            Err(GpxError::UnnamedWaypoint { index: 0 })
        ));
    }

    #[test]
    fn entities_in_text_are_resolved() {
        let xml = r#"<gpx version="1.1"><wpt lat="51.8" lon="10.6"><name>A &amp; B &lt;x&gt;</name></wpt></gpx>"#;
        let document = parse_gpx(xml).expect("parse");
        assert_eq!(document.waypoints[0].name, "A & B <x>");
    }

    #[test]
    fn cdata_is_read() {
        let xml = r#"<gpx version="1.1"><wpt lat="51.8" lon="10.6"><name><![CDATA[Kirche & Co]]></name></wpt></gpx>"#;
        let document = parse_gpx(xml).expect("parse");
        assert_eq!(document.waypoints[0].name, "Kirche & Co");
    }

    #[test]
    fn vendor_extensions_are_ignored() {
        let xml = r#"<gpx version="1.1" xmlns:gpxx="http://www.garmin.com/xmlschemas/GpxExtensions/v3">
  <wpt lat="51.8" lon="10.6">
    <name>A</name>
    <extensions><gpxx:WaypointExtension><gpxx:DisplayMode>SymbolAndName</gpxx:DisplayMode></gpxx:WaypointExtension></extensions>
  </wpt>
</gpx>"#;
        let document = parse_gpx(xml).expect("parse");
        assert_eq!(document.waypoints.len(), 1);
        assert_eq!(document.waypoints[0].name, "A");
    }

    #[test]
    fn missing_gpx_root_is_rejected() {
        let xml = r#"<kml><wpt lat="51.8" lon="10.6"><name>A</name></wpt></kml>"#;
        assert!(matches!(parse_gpx(xml), Err(GpxError::NotGpx { .. })));
    }

    #[test]
    fn empty_document_is_rejected() {
        let xml = r#"<gpx version="1.1"></gpx>"#;
        assert!(matches!(parse_gpx(xml), Err(GpxError::EmptyDocument)));
    }

    #[test]
    fn malformed_xml_is_reported() {
        assert!(matches!(
            parse_gpx("<gpx><wpt"),
            Err(GpxError::InvalidXml { .. })
        ));
    }

    #[test]
    fn missing_coordinates_are_rejected() {
        let xml = r#"<gpx version="1.1"><wpt lat="51.8"><name>A</name></wpt></gpx>"#;
        assert!(matches!(
            parse_gpx(xml),
            Err(GpxError::MissingElement {
                what: "lon attribute",
                ..
            })
        ));
    }

    #[test]
    fn out_of_range_coordinates_are_rejected() {
        let xml = r#"<gpx version="1.1"><wpt lat="91.0" lon="10.6"><name>A</name></wpt></gpx>"#;
        assert!(matches!(
            parse_gpx(xml),
            Err(GpxError::InvalidCoordinate { .. })
        ));
    }

    #[test]
    fn non_numeric_elevation_is_rejected() {
        let xml = r#"<gpx version="1.1"><wpt lat="51.8" lon="10.6"><ele>high</ele><name>A</name></wpt></gpx>"#;
        assert!(matches!(
            parse_gpx(xml),
            Err(GpxError::InvalidNumber { context: "ele", .. })
        ));
    }

    #[test]
    fn write_then_read_roundtrips() {
        use crate::writer::{WriteOptions, write_gpx};

        let mut document = GpxDocument::new("Roundtrip");
        let mut waypoint = GpxWaypoint::new(
            "HWN001 Eckertalsperre",
            LatLon::new(51.841_649, 10.579_975_7).expect("pos"),
        );
        waypoint.elevation_m = Some(555.0);
        waypoint.comment = Some("Eckertalsperre (Staumauer)".to_string());
        document.push_waypoint(waypoint);
        document.push_segment_from_positions(&[
            LatLon::new(51.841_649, 10.579_975_7).expect("pos"),
            LatLon::new(51.830_167_7, 10.602_774_2).expect("pos"),
        ]);

        let xml = write_gpx(&document, &WriteOptions::default()).expect("write");
        let parsed = parse_gpx(&xml).expect("parse");
        assert_eq!(parsed.name.as_deref(), Some("Roundtrip"));
        assert_eq!(parsed.waypoints.len(), 1);
        assert_eq!(parsed.waypoints[0].name, "HWN001 Eckertalsperre");
        assert_eq!(parsed.track_point_count(), 2);
    }
}
