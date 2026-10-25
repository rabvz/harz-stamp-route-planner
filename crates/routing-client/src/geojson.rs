// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parsing of GeoJSON routing responses.
//!
//! Several freely usable hiking routers (for example OpenRouteService and
//! Valhalla in GeoJSON mode) return a GeoJSON geometry plus a summary. Rather
//! than binding to one vendor's response type, this module accepts the shared
//! subset and reports anything else as [`RoutingError::UnsupportedShape`], so a
//! changed upstream schema fails loudly instead of silently producing a wrong
//! distance.
//!
//! Recognised shapes:
//!
//! ```text
//! FeatureCollection { features: [ { geometry: { LineString }, properties } ] }
//! Feature            { geometry: { LineString }, properties }
//! LineString         { coordinates: [[lon, lat], ...] }
//! ```
//!
//! Distance and duration are taken from `properties.summary.*` or
//! `properties.*` when present; the summary distance is authoritative because
//! it comes from the routing engine, while the geometry may be simplified. When
//! no summary is present the distance is measured along the geometry and the
//! duration derived from the configured walking speed.

use geo_engine::LatLon;
use serde_json::Value;

use crate::RoutingError;
use crate::provider::RouteLeg;

/// Parses a GeoJSON route response into a [`RouteLeg`].
///
/// `speed_mps` is only used when the response carries no duration; pass
/// [`crate::DEFAULT_WALKING_SPEED_MPS`] unless you have a better estimate.
pub fn parse_geojson_route(json: &str, speed_mps: f64) -> Result<RouteLeg, RoutingError> {
    let root: Value = serde_json::from_str(json).map_err(|error| RoutingError::InvalidJson {
        message: error.to_string(),
    })?;

    let (geometry_value, properties) = locate_geometry_and_properties(&root)?;
    let coordinates =
        geometry_value
            .get("coordinates")
            .ok_or_else(|| RoutingError::UnsupportedShape {
                message: "geometry has no `coordinates` member".to_string(),
            })?;
    let geometry = parse_linestring_coordinates(coordinates)?;

    let distance = property_number(properties.as_ref(), "distance");
    let duration = property_number(properties.as_ref(), "duration")
        .or_else(|| property_number(properties.as_ref(), "time"));

    let mut leg = RouteLeg::from_geometry(geometry, duration, speed_mps, false)?;
    if let Some(distance) = distance {
        if distance < 0.0 || !distance.is_finite() {
            return Err(RoutingError::UnsupportedShape {
                message: format!("provider reported an implausible distance: {distance}"),
            });
        }
        // A routed distance is authoritative: it comes from the trail network,
        // while the geometry may be simplified. Only use it when it is usable,
        // so a missing or zero summary falls back to the measured geometry.
        if distance > 0.0 {
            leg.distance_m = distance;
        }
    }
    Ok(leg)
}

/// Finds the geometry object and the properties object to read summaries from.
fn locate_geometry_and_properties(root: &Value) -> Result<(Value, Option<Value>), RoutingError> {
    let kind = root.get("type").and_then(Value::as_str).unwrap_or_default();

    match kind {
        "FeatureCollection" => {
            let feature = root
                .get("features")
                .and_then(Value::as_array)
                .and_then(|features| features.first())
                .ok_or_else(|| RoutingError::UnsupportedShape {
                    message: "FeatureCollection contains no features".to_string(),
                })?;
            let geometry =
                feature
                    .get("geometry")
                    .cloned()
                    .ok_or_else(|| RoutingError::UnsupportedShape {
                        message: "feature has no `geometry` member".to_string(),
                    })?;
            Ok((geometry, feature.get("properties").cloned()))
        }
        "Feature" => {
            let geometry =
                root.get("geometry")
                    .cloned()
                    .ok_or_else(|| RoutingError::UnsupportedShape {
                        message: "feature has no `geometry` member".to_string(),
                    })?;
            Ok((geometry, root.get("properties").cloned()))
        }
        "LineString" => Ok((root.clone(), root.get("properties").cloned())),
        other => Err(RoutingError::UnsupportedShape {
            message: format!("unsupported GeoJSON object type {other:?}"),
        }),
    }
}

/// Reads a number from `properties.summary.<key>` or `properties.<key>`.
fn property_number(properties: Option<&Value>, key: &str) -> Option<f64> {
    let properties = properties?;
    properties
        .get("summary")
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_f64)
        .or_else(|| properties.get(key).and_then(Value::as_f64))
}

/// Converts GeoJSON `[lon, lat]` pairs into validated positions.
fn parse_linestring_coordinates(coordinates: &Value) -> Result<Vec<LatLon>, RoutingError> {
    let pairs = coordinates
        .as_array()
        .ok_or_else(|| RoutingError::UnsupportedShape {
            message: "`coordinates` is not an array".to_string(),
        })?;

    if pairs.is_empty() {
        return Err(RoutingError::EmptyGeometry);
    }

    let mut geometry = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let values = pair
            .as_array()
            .ok_or_else(|| RoutingError::UnsupportedShape {
                message: "a coordinate is not an array".to_string(),
            })?;
        let (Some(lon), Some(lat)) = (
            values.first().and_then(Value::as_f64),
            values.get(1).and_then(Value::as_f64),
        ) else {
            return Err(RoutingError::InvalidCoordinate {
                message: "coordinate is missing a numeric longitude or latitude".to_string(),
            });
        };
        // GeoJSON stores longitude first; getting this backwards silently puts
        // the route in the wrong country, so validate before constructing.
        let position = LatLon::new(lat, lon).map_err(|error| RoutingError::InvalidCoordinate {
            message: error.to_string(),
        })?;
        geometry.push(position);
    }

    if geometry.len() < 2 {
        return Err(RoutingError::EmptyGeometry);
    }
    Ok(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE_STRING: &str = r#"{
        "type": "LineString",
        "coordinates": [[10.5799757, 51.841649], [10.6027742, 51.8301677]]
    }"#;

    #[test]
    fn parses_a_bare_linestring() {
        let leg = parse_geojson_route(LINE_STRING, 1.25).expect("leg");
        assert_eq!(leg.geometry.len(), 2);
        assert!(!leg.estimated);
        assert!((leg.geometry[0].latitude() - 51.841_649).abs() < 1e-9);
        assert!((leg.geometry[0].longitude() - 10.579_975_7).abs() < 1e-9);
        assert!(leg.distance_m > 2_000.0 && leg.distance_m < 2_100.0);
    }

    #[test]
    fn longitude_and_latitude_are_not_swapped() {
        let leg = parse_geojson_route(LINE_STRING, 1.25).expect("leg");
        // The Harz is at ~51.8 N / 10.6 E; a swap would fail validation.
        assert!(leg.geometry[0].latitude() > 51.0);
        assert!(leg.geometry[0].longitude() < 11.0);
    }

    #[test]
    fn parses_a_feature_collection_with_summary() {
        let json = r#"{
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "LineString",
                    "coordinates": [[10.5799757, 51.841649], [10.6027742, 51.8301677]]
                },
                "properties": { "summary": { "distance": 2450.0, "duration": 1800.0 } }
            }]
        }"#;
        let leg = parse_geojson_route(json, 1.25).expect("leg");
        assert!(
            (leg.distance_m - 2450.0).abs() < 1e-9,
            "provider distance wins"
        );
        assert!((leg.duration_s - 1800.0).abs() < 1e-9);
    }

    #[test]
    fn derives_duration_when_absent() {
        let leg = parse_geojson_route(LINE_STRING, 1.0).expect("leg");
        assert!((leg.duration_s - leg.distance_m).abs() < 1e-6);
    }

    #[test]
    fn rejects_unsupported_object_type() {
        let json = r#"{"type": "Polygon", "coordinates": [[[1,2],[3,4],[5,6]]]}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::UnsupportedShape { .. })
        ));
    }

    #[test]
    fn rejects_empty_feature_collection() {
        let json = r#"{"type": "FeatureCollection", "features": []}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::UnsupportedShape { .. })
        ));
    }

    #[test]
    fn rejects_empty_coordinates() {
        let json = r#"{"type": "LineString", "coordinates": []}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::EmptyGeometry)
        ));
    }

    #[test]
    fn rejects_single_point_geometry() {
        let json = r#"{"type": "LineString", "coordinates": [[10.6, 51.8]]}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::EmptyGeometry)
        ));
    }

    #[test]
    fn rejects_out_of_range_coordinates() {
        let json = r#"{"type": "LineString", "coordinates": [[10.6, 91.0], [10.7, 51.8]]}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::InvalidCoordinate { .. })
        ));
    }

    #[test]
    fn rejects_non_numeric_coordinates() {
        let json = r#"{"type": "LineString", "coordinates": [["a", 51.8], [10.7, 51.8]]}"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::InvalidCoordinate { .. })
        ));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(
            parse_geojson_route("{", 1.25),
            Err(RoutingError::InvalidJson { .. })
        ));
    }

    #[test]
    fn rejects_implausible_provider_distance() {
        let json = r#"{
            "type": "LineString",
            "properties": { "distance": -50.0 },
            "coordinates": [[10.5799757, 51.841649], [10.6027742, 51.8301677]]
        }"#;
        assert!(matches!(
            parse_geojson_route(json, 1.25),
            Err(RoutingError::UnsupportedShape { .. })
        ));
    }
}
