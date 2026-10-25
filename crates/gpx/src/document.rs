// SPDX-License-Identifier: MIT OR Apache-2.0

//! The in-memory GPX document model.

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};

/// A `<wpt>` element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpxWaypoint {
    /// `<name>`, the short label shown on devices.
    pub name: String,
    /// `<desc>`, the longer description.
    pub description: Option<String>,
    /// `<cmt>`, the comment field; often a human readable location.
    pub comment: Option<String>,
    /// `<sym>`, the device symbol name.
    pub symbol: Option<String>,
    /// Position from the `lat`/`lon` attributes.
    pub position: LatLon,
    /// `<ele>`, elevation in metres.
    pub elevation_m: Option<f64>,
}

impl GpxWaypoint {
    /// Builds a waypoint with only the required fields.
    #[must_use]
    pub fn new(name: impl Into<String>, position: LatLon) -> Self {
        Self {
            name: name.into(),
            description: None,
            comment: None,
            symbol: None,
            position,
            elevation_m: None,
        }
    }
}

/// A `<trkpt>` element.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GpxTrackPoint {
    /// Position from the `lat`/`lon` attributes.
    pub position: LatLon,
    /// `<ele>`, elevation in metres.
    pub elevation_m: Option<f64>,
}

/// A `<trkseg>` element.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GpxTrackSegment {
    /// Points in walking order.
    pub points: Vec<GpxTrackPoint>,
}

/// A whole GPX document, limited to the parts this project reads and writes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GpxDocument {
    /// `<metadata><name>`.
    pub name: Option<String>,
    /// `<metadata><desc>`.
    pub description: Option<String>,
    /// `<metadata><time>`, an ISO 8601 timestamp.
    pub time: Option<String>,
    /// `<wpt>` elements in document order.
    pub waypoints: Vec<GpxWaypoint>,
    /// `<trkseg>` elements in document order.
    pub segments: Vec<GpxTrackSegment>,
}

impl GpxDocument {
    /// Creates an empty document with a metadata name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// Appends a waypoint.
    pub fn push_waypoint(&mut self, waypoint: GpxWaypoint) {
        self.waypoints.push(waypoint);
    }

    /// Appends a track segment from plain positions.
    pub fn push_segment_from_positions(&mut self, positions: &[LatLon]) {
        let segment = GpxTrackSegment {
            points: positions
                .iter()
                .map(|position| GpxTrackPoint {
                    position: *position,
                    elevation_m: None,
                })
                .collect(),
        };
        self.segments.push(segment);
    }

    /// Total number of track points across all segments.
    #[must_use]
    pub fn track_point_count(&self) -> usize {
        self.segments
            .iter()
            .map(|segment| segment.points.len())
            .sum()
    }

    /// All track points, flattened in document order.
    #[must_use]
    pub fn track_points(&self) -> Vec<GpxTrackPoint> {
        self.segments
            .iter()
            .flat_map(|segment| segment.points.iter().copied())
            .collect()
    }
}
