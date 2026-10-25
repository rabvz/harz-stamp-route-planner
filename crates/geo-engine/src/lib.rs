// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure, dependency-light geometry primitives for the Harz stamp route planner.
//!
//! This crate is the bottom of the dependency graph: it knows nothing about
//! stamps, routing providers, GPX or the user interface. Everything here is
//! deterministic and side-effect free so that higher layers stay testable.
//!
//! All distances are metres on the WGS84 ellipsoid approximation used by the
//! haversine formula (mean Earth radius). That is accurate to well under 0.5 %
//! for the distances involved in day hikes, which is sufficient for planning
//! and documented as an approximation rather than a survey-grade measurement.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bbox;
mod error;
mod latlon;
mod plane;
mod polyline;

pub use bbox::BoundingBox;
pub use error::GeoError;
pub use latlon::{EARTH_RADIUS_M, LatLon, LatLonRepr, haversine_m};
pub use plane::{LocalPlane, Point};
pub use polyline::{path_length_m, simplify};

/// Maximum plausible distance for a single leg inside the Harz region, in
/// metres. Used to sanity-check optimizer output and imported tracks.
pub const MAX_SANE_LEG_M: f64 = 500_000.0;
