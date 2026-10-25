// SPDX-License-Identifier: MIT OR Apache-2.0

//! The typed request/response façade.
//!
//! Everything here is plain Rust: no FFI, no raw pointers. [`handle`] takes a
//! JSON string and returns a JSON string, which makes the entire browser-facing
//! surface testable in a normal unit test.

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use stamp_core::{Stamp, StampCatalog, StampId};

/// An error the caller can act on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    /// Stable machine-readable code.
    pub code: String,
    /// Human readable explanation.
    pub message: String,
}

impl ApiError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

/// The envelope returned to JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// Whether the request succeeded.
    pub ok: bool,
    /// Result payload when `ok` is true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    /// Error detail when `ok` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl Response {
    fn success(data: Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn failure(error: ApiError) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Request {
    op: String,
    #[serde(default)]
    payload: Value,
}

/// Handles one JSON request and returns one JSON response.
///
/// This function never panics and never returns malformed JSON: any internal
/// failure becomes an error response, including a serialisation failure.
#[must_use]
pub fn handle(request: &str) -> String {
    let response = match serde_json::from_str::<Request>(request) {
        Ok(request) => match dispatch(&request.op, &request.payload) {
            Ok(data) => Response::success(data),
            Err(error) => Response::failure(error),
        },
        Err(error) => Response::failure(ApiError::new(
            "bad_request",
            format!("request is not valid JSON: {error}"),
        )),
    };

    serde_json::to_string(&response).unwrap_or_else(|error| {
        // Serialising a Response of owned strings cannot fail in practice; if it
        // somehow does, still return valid JSON rather than nothing.
        format!(
            r#"{{"ok":false,"error":{{"code":"internal","message":"could not serialise the response: {}"}}}}"#,
            error.to_string().replace('"', "'")
        )
    })
}

fn dispatch(op: &str, payload: &Value) -> Result<Value, ApiError> {
    let catalog = StampCatalog::embedded();
    match op {
        "version" => Ok(version(catalog)),
        "catalog" => Ok(catalog_json(catalog)),
        "clusters" => clusters(catalog, payload),
        "plan" => plan(catalog, payload),
        "exportGpx" => export_gpx(catalog, payload),
        "importGpx" => import_gpx(catalog, payload),
        "parking" => parking(catalog, payload),
        other => Err(ApiError::new(
            "unknown_op",
            format!("unknown operation {other:?}"),
        )),
    }
}

fn version(catalog: &StampCatalog) -> Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "catalog": {
            "id": catalog.dataset().id,
            "name": catalog.dataset().name,
            "stampCount": catalog.len(),
            "retrieved": catalog.dataset().retrieved,
            "sourceUrl": catalog.dataset().source_url,
        }
    })
}

fn catalog_json(catalog: &StampCatalog) -> Value {
    let stamps: Vec<Value> = catalog
        .stamps()
        .iter()
        .map(|stamp| {
            json!({
                "id": stamp.id.number(),
                "code": stamp.code,
                "name": stamp.name,
                "location": stamp.location,
                "lat": stamp.position.latitude(),
                "lon": stamp.position.longitude(),
                "elevationM": stamp.elevation_m,
            })
        })
        .collect();
    json!({
        "dataset": {
            "id": catalog.dataset().id,
            "name": catalog.dataset().name,
            "sourceUrl": catalog.dataset().source_url,
            "released": catalog.dataset().released,
            "retrieved": catalog.dataset().retrieved,
            "rightsNote": catalog.dataset().rights_note,
        },
        "stamps": stamps,
    })
}

/// Reads a list of station numbers from a payload.
fn stamp_ids(payload: &Value) -> Result<Vec<StampId>, ApiError> {
    let raw = payload
        .get("stampIds")
        .ok_or_else(|| ApiError::new("bad_request", "missing `stampIds`"))?;
    let numbers: Vec<u16> = serde_json::from_value(raw.clone())
        .map_err(|error| ApiError::new("bad_request", format!("invalid `stampIds`: {error}")))?;
    let mut ids = Vec::with_capacity(numbers.len());
    for number in numbers {
        ids.push(
            StampId::new(number)
                .map_err(|error| ApiError::new("invalid_selection", error.to_string()))?,
        );
    }
    Ok(ids)
}

/// Resolves station numbers into catalogue entries.
fn resolve(catalog: &StampCatalog, ids: &[StampId]) -> Result<Vec<Stamp>, ApiError> {
    ids.iter()
        .map(|id| {
            catalog
                .get(*id)
                .cloned()
                .ok_or_else(|| ApiError::new("invalid_selection", format!("unknown station {id}")))
        })
        .collect()
}

fn clusters(catalog: &StampCatalog, payload: &Value) -> Result<Value, ApiError> {
    let ids = stamp_ids(payload)?;
    let stamps = resolve(catalog, &ids)?;
    let options = stamp_clustering::ClusterOptions {
        max_stamps: payload
            .get("maxStamps")
            .and_then(Value::as_u64)
            .map_or(8, |value| usize::try_from(value).unwrap_or(8)),
        max_diameter_m: payload
            .get("maxDiameterM")
            .and_then(Value::as_f64)
            .unwrap_or(8_000.0),
    };
    let references: Vec<&Stamp> = stamps.iter().collect();
    let clusters = stamp_clustering::cluster_stamps(&references, options)
        .map_err(|error| ApiError::new("cluster_error", error.to_string()))?;

    let items: Vec<Value> = clusters
        .iter()
        .map(|cluster| {
            json!({
                "id": cluster.id,
                "stampIds": cluster.stamps.iter().map(|id| id.number()).collect::<Vec<_>>(),
                "centroid": {
                    "lat": cluster.centroid.latitude(),
                    "lon": cluster.centroid.longitude(),
                },
                "diameterM": cluster.diameter_m,
            })
        })
        .collect();
    Ok(json!({ "clusters": items }))
}

fn plan(catalog: &StampCatalog, payload: &Value) -> Result<Value, ApiError> {
    let ids = stamp_ids(payload)?;
    let stamps = resolve(catalog, &ids)?;

    let start = match payload.get("start") {
        Some(value) => {
            let representation: geo_engine::LatLonRepr = serde_json::from_value(value.clone())
                .map_err(|error| {
                    ApiError::new("bad_request", format!("invalid `start`: {error}"))
                })?;
            LatLon::try_from(representation)
                .map_err(|error| ApiError::new("bad_request", error.to_string()))?
        }
        None => stamps
            .first()
            .map(|stamp| stamp.position)
            .ok_or_else(|| ApiError::new("invalid_selection", "no stations selected"))?,
    };

    let options = route_optimizer::OptimizerOptions {
        max_tour_distance_m: payload
            .get("maxTourDistanceM")
            .and_then(Value::as_f64)
            .unwrap_or(route_optimizer::DEFAULT_MAX_TOUR_DISTANCE_M),
        two_opt_passes: payload
            .get("twoOptPasses")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(8),
    };

    // A matrix supplied by the web app carries real routed distances; without
    // one the planner falls back to straight-line estimates and says so.
    let matrix =
        match payload.get("matrix") {
            Some(value) => Some(serde_json::from_value::<Vec<f64>>(value.clone()).map_err(
                |error| ApiError::new("bad_request", format!("invalid `matrix`: {error}")),
            )?),
            None => None,
        };
    let provider_name = payload
        .get("providerName")
        .and_then(Value::as_str)
        .unwrap_or("straight-line-estimate")
        .to_string();

    let tour = route_optimizer::plan(route_optimizer::PlanRequest {
        start,
        stamps,
        matrix,
        options,
        provider_name,
    })
    .map_err(|error| ApiError::new("plan_failed", error.to_string()))?;

    serde_json::to_value(&tour).map_err(|error| ApiError::new("internal", error.to_string()))
}

fn export_gpx(catalog: &StampCatalog, payload: &Value) -> Result<Value, ApiError> {
    let ids = stamp_ids(payload)?;
    let stamps = resolve(catalog, &ids)?;
    let name = payload
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Harzer Wandernadel tour")
        .to_string();

    let mut document = gpx::GpxDocument::new(name);
    document.description = payload
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    for stamp in &stamps {
        let mut waypoint =
            gpx::GpxWaypoint::new(format!("{} — {}", stamp.code, stamp.name), stamp.position);
        waypoint.elevation_m = stamp.elevation_m;
        waypoint.comment = Some(stamp.location.clone());
        waypoint.symbol = Some("Flag, Blue".to_string());
        document.push_waypoint(waypoint);
    }

    // Track geometry is only included when the caller supplies real routed
    // geometry. Writing straight lines between stations as a "route" would
    // produce a file that looks like a hiking trail but is not one. Elevation
    // is optional per point: BRouter returns it, an estimate does not.
    if let Some(track) = payload.get("trackPoints") {
        #[derive(Deserialize)]
        struct RawTrackPoint {
            lat: f64,
            lon: f64,
            #[serde(default)]
            ele: Option<f64>,
        }

        let raw: Vec<RawTrackPoint> = serde_json::from_value(track.clone()).map_err(|error| {
            ApiError::new("bad_request", format!("invalid `trackPoints`: {error}"))
        })?;
        let mut points = Vec::with_capacity(raw.len());
        for point in raw {
            let position = LatLon::new(point.lat, point.lon)
                .map_err(|error| ApiError::new("bad_request", error.to_string()))?;
            points.push(gpx::GpxTrackPoint {
                position,
                elevation_m: point.ele.filter(|value| value.is_finite()),
            });
        }
        if points.len() >= 2 {
            document.segments.push(gpx::GpxTrackSegment { points });
        }
    }

    let options = gpx::WriteOptions {
        include_waypoints: payload
            .get("includeStamps")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        include_track: payload
            .get("includeTrack")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        ..gpx::WriteOptions::default()
    };

    let xml = gpx::write_gpx(&document, &options)
        .map_err(|error| ApiError::new("gpx_error", error.to_string()))?;
    Ok(json!({
        "gpx": xml,
        "waypointCount": document.waypoints.len(),
        "trackPointCount": document.track_point_count(),
    }))
}

fn import_gpx(catalog: &StampCatalog, payload: &Value) -> Result<Value, ApiError> {
    let xml = payload
        .get("gpx")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::new("bad_request", "missing `gpx`"))?;
    let document =
        gpx::parse_gpx(xml).map_err(|error| ApiError::new("gpx_error", error.to_string()))?;

    // Match by the "HWNnnn" prefix that the operator's own files use, so an
    // imported collection can preselect stamps without uploading anything.
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();
    for waypoint in &document.waypoints {
        match stamp_id_from_name(&waypoint.name) {
            Some(id) if catalog.get(id).is_some() => {
                if !matched.contains(&id) {
                    matched.push(id);
                }
            }
            _ => unmatched.push(waypoint.name.clone()),
        }
    }

    Ok(json!({
        "matchedStampIds": matched.iter().map(|id| id.number()).collect::<Vec<_>>(),
        "unmatchedNames": unmatched,
        "waypointCount": document.waypoints.len(),
        "trackPointCount": document.track_point_count(),
    }))
}

/// Extracts a station number from a name like `HWN001 Eckertalsperre`.
fn stamp_id_from_name(name: &str) -> Option<StampId> {
    let trimmed = name.trim();
    let digits: String = trimmed
        .strip_prefix("HWN")
        .or_else(|| trimmed.strip_prefix("hwn"))?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.len() != 3 {
        return None;
    }
    digits
        .parse::<u16>()
        .ok()
        .and_then(|number| StampId::new(number).ok())
}

fn parking(catalog: &StampCatalog, payload: &Value) -> Result<Value, ApiError> {
    let ids = stamp_ids(payload)?;
    let stamps = resolve(catalog, &ids)?;

    let raw_candidates = payload
        .get("candidates")
        .ok_or_else(|| ApiError::new("bad_request", "missing `candidates`"))?;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawCandidate {
        id: String,
        name: String,
        lat: f64,
        lon: f64,
        #[serde(default = "default_kind")]
        kind: String,
        #[serde(default)]
        capacity: Option<u16>,
        #[serde(default)]
        source: String,
    }
    fn default_kind() -> String {
        "unknown".to_string()
    }

    let raw: Vec<RawCandidate> = serde_json::from_value(raw_candidates.clone())
        .map_err(|error| ApiError::new("bad_request", format!("invalid `candidates`: {error}")))?;

    let mut candidates = Vec::with_capacity(raw.len());
    for candidate in raw {
        let position = LatLon::new(candidate.lat, candidate.lon)
            .map_err(|error| ApiError::new("bad_request", error.to_string()))?;
        let kind = match candidate.kind.as_str() {
            "trailhead" => parking_search::ParkingKind::Trailhead,
            "public" => parking_search::ParkingKind::Public,
            "roadside" => parking_search::ParkingKind::Roadside,
            _ => parking_search::ParkingKind::Unknown,
        };
        candidates.push(parking_search::ParkingCandidate {
            id: candidate.id,
            name: candidate.name,
            position,
            kind,
            capacity: candidate.capacity,
            source: candidate.source,
        });
    }

    let options = parking_search::ParkingSearchOptions {
        max_walk_m: payload
            .get("maxWalkM")
            .and_then(Value::as_f64)
            .unwrap_or(1_500.0),
        max_candidates: payload
            .get("maxCandidates")
            .and_then(Value::as_u64)
            .map_or(10, |value| usize::try_from(value).unwrap_or(10)),
        min_stamps: payload
            .get("minStamps")
            .and_then(Value::as_u64)
            .map_or(1, |value| usize::try_from(value).unwrap_or(1)),
    };

    let references: Vec<&Stamp> = stamps.iter().collect();
    let ranked = parking_search::rank_parking(&candidates, &references, options)
        .map_err(|error| ApiError::new("parking_error", error.to_string()))?;

    let suggestions: Vec<Value> = ranked
        .iter()
        .map(|suggestion| {
            json!({
                "candidate": {
                    "id": suggestion.candidate.id,
                    "name": suggestion.candidate.name,
                    "lat": suggestion.candidate.position.latitude(),
                    "lon": suggestion.candidate.position.longitude(),
                    "kind": suggestion.candidate.kind.as_str(),
                    "capacity": suggestion.candidate.capacity,
                    "source": suggestion.candidate.source,
                },
                "servedStampIds": suggestion.served_stamps.iter().map(|id| id.number()).collect::<Vec<_>>(),
                "nearestWalkM": suggestion.nearest_walk_m,
                "meanWalkM": suggestion.mean_walk_m,
                "score": suggestion.score,
            })
        })
        .collect();

    Ok(json!({ "suggestions": suggestions }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(request: &str) -> Value {
        let response = handle(request);
        serde_json::from_str(&response).expect("the boundary must always return JSON")
    }

    #[test]
    fn version_reports_the_catalogue() {
        let value = call(r#"{"op":"version"}"#);
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["catalog"]["stampCount"], 222);
        assert_eq!(value["data"]["catalog"]["id"], "hwn-2025-04-23");
    }

    #[test]
    fn catalog_returns_every_station() {
        let value = call(r#"{"op":"catalog"}"#);
        assert_eq!(value["ok"], true);
        let stamps = value["data"]["stamps"].as_array().expect("array");
        assert_eq!(stamps.len(), 222);
        assert_eq!(stamps[0]["code"], "HWN001");
        assert_eq!(stamps[0]["name"], "Eckertalsperre");
        assert!(stamps[0]["lat"].as_f64().expect("lat") > 51.0);
    }

    #[test]
    fn unknown_operation_is_reported() {
        let value = call(r#"{"op":"explode"}"#);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "unknown_op");
    }

    #[test]
    fn malformed_request_is_reported() {
        let value = call("this is not json");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "bad_request");
    }

    #[test]
    fn unknown_station_is_rejected() {
        let value = call(r#"{"op":"plan","payload":{"stampIds":[900]}}"#);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "invalid_selection");
    }

    #[test]
    fn plan_produces_one_day_tour_and_totals() {
        let value = call(r#"{"op":"plan","payload":{"stampIds":[1,2,3]}}"#);
        assert_eq!(value["ok"], true, "response: {value}");
        assert!(
            value["data"]["legs"]
                .as_array()
                .is_some_and(|legs| legs.len() >= 4),
            "a tour needs a leg per station plus the return leg"
        );
        assert!(value["data"]["distanceM"].as_f64().expect("total") > 0.0);
        assert_eq!(
            value["data"]["stampsVisited"]
                .as_array()
                .expect("visited")
                .len(),
            3
        );
        assert_eq!(value["data"]["estimated"], true);
        assert!(
            value["data"].get("days").is_none(),
            "the wire format must not carry multi-day planning"
        );
    }

    #[test]
    fn plan_accepts_a_supplied_matrix() {
        let matrix = vec![0.0, 0.0, 1_000.0, 0.0, 0.0, 1_000.0, 1_000.0, 1_000.0, 0.0];
        let request = json!({
            "op": "plan",
            "payload": {
                "stampIds": [1, 2],
                "start": { "lat": 51.841649, "lon": 10.5799757 },
                "matrix": matrix,
                "providerName": "test-router",
            }
        });
        let value = call(&request.to_string());
        assert_eq!(value["ok"], true, "response: {value}");
        assert_eq!(value["data"]["providerName"], "test-router");
        assert_eq!(value["data"]["estimated"], false);
        assert!((value["data"]["distanceM"].as_f64().expect("total") - 2_000.0).abs() < 1e-6);
    }

    #[test]
    fn warnings_expose_camel_case_fields() {
        // The UI renders these values directly. A snake_case field name would
        // surface as "–" in the tour summary, which is how this regression was
        // found; pin the wire format instead of trusting the rename attribute.
        let value = call(r#"{"op":"plan","payload":{"stampIds":[1,222],"maxTourDistanceM":100}}"#);
        assert_eq!(value["ok"], true, "response: {value}");
        let warnings = value["data"]["warnings"].as_array().expect("warnings");
        assert!(!warnings.is_empty(), "expected an oversized-tour warning");

        let warning = &warnings[0];
        assert_eq!(warning["kind"], "oversized_tour");
        assert!(
            warning["distanceM"]
                .as_f64()
                .is_some_and(|value| value > 0.0),
            "distanceM must be a number, got {:?}",
            warning.get("distanceM")
        );
        assert!(
            warning["limitM"].as_f64().is_some_and(|value| value > 0.0),
            "limitM must be a number, got {:?}",
            warning.get("limitM")
        );
        assert!(
            warning.get("distance_m").is_none(),
            "the wire format must not leak snake_case fields"
        );
    }

    #[test]
    fn clusters_partition_the_selection() {
        let value = call(r#"{"op":"clusters","payload":{"stampIds":[1,2,3,200,201]}}"#);
        assert_eq!(value["ok"], true, "response: {value}");
        let clusters = value["data"]["clusters"].as_array().expect("clusters");
        let total: usize = clusters
            .iter()
            .map(|cluster| cluster["stampIds"].as_array().map_or(0, Vec::len))
            .sum();
        assert_eq!(total, 5);
    }

    #[test]
    fn export_produces_parseable_gpx() {
        let value = call(r#"{"op":"exportGpx","payload":{"stampIds":[1,2],"name":"Test"}}"#);
        assert_eq!(value["ok"], true, "response: {value}");
        let xml = value["data"]["gpx"].as_str().expect("gpx");
        assert!(xml.contains("<gpx version=\"1.1\""));
        assert!(xml.contains("HWN001"));
        assert_eq!(value["data"]["waypointCount"], 2);
    }

    #[test]
    fn export_includes_track_only_when_geometry_is_supplied() {
        let without = call(r#"{"op":"exportGpx","payload":{"stampIds":[1]}}"#);
        assert_eq!(without["data"]["trackPointCount"], 0);

        let with = call(
            r#"{"op":"exportGpx","payload":{"stampIds":[1],"trackPoints":[
                {"lat":51.841649,"lon":10.5799757,"ele":612.4},
                {"lat":51.8301677,"lon":10.6027742}]}}"#,
        );
        assert_eq!(with["ok"], true, "response: {with}");
        assert_eq!(with["data"]["trackPointCount"], 2);
        // Elevation from the routing service must survive into the GPX track.
        assert!(
            with["data"]["gpx"]
                .as_str()
                .expect("gpx")
                .contains("<ele>612.4</ele>")
        );
    }

    #[test]
    fn import_matches_station_codes() {
        let request = json!({
            "op": "importGpx",
            "payload": { "gpx": r#"<gpx version="1.1">
                <wpt lat="51.841649" lon="10.5799757"><name>HWN001 Eckertalsperre</name></wpt>
                <wpt lat="51.8301677" lon="10.6027742"><name>HWN002 Scharfenstein</name></wpt>
                <wpt lat="51.8" lon="10.6"><name>Somewhere else</name></wpt>
            </gpx>"# }
        });
        let value = call(&request.to_string());
        assert_eq!(value["ok"], true, "response: {value}");
        assert_eq!(value["data"]["matchedStampIds"], json!([1, 2]));
        assert_eq!(value["data"]["unmatchedNames"], json!(["Somewhere else"]));
    }

    #[test]
    fn import_rejects_invalid_gpx() {
        let value = call(r#"{"op":"importGpx","payload":{"gpx":"<not-gpx"}}"#);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "gpx_error");
    }

    #[test]
    fn parking_ranks_supplied_candidates() {
        let request = json!({
            "op": "parking",
            "payload": {
                "stampIds": [1],
                "candidates": [{
                    "id": "node/1", "name": "Wanderparkplatz", "lat": 51.841649,
                    "lon": 10.5799757, "kind": "trailhead", "source": "openstreetmap"
                }]
            }
        });
        let value = call(&request.to_string());
        assert_eq!(value["ok"], true, "response: {value}");
        let suggestions = value["data"]["suggestions"]
            .as_array()
            .expect("suggestions");
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0]["servedStampIds"], json!([1]));
    }

    #[test]
    fn station_name_parsing_is_strict() {
        assert_eq!(
            stamp_id_from_name("HWN001 Eckertalsperre").map(StampId::number),
            Some(1)
        );
        assert_eq!(stamp_id_from_name("hwn222").map(StampId::number), Some(222));
        assert_eq!(stamp_id_from_name("HWN01 short"), None);
        assert_eq!(stamp_id_from_name("Eckertalsperre"), None);
        assert_eq!(stamp_id_from_name("HWN000"), None);
    }
}
