// SPDX-License-Identifier: MIT OR Apache-2.0

//! GPX 1.1 reading and writing.
//!
//! This crate is a pure format layer: it knows waypoints and track segments,
//! but nothing about planning, routing or the user interface. Composing an
//! itinerary into a document happens one layer up, which keeps the dependency
//! graph one-way and makes both directions testable against fixed fixtures.
//!
//! Writing follows the GPX 1.1 schema with the `topografix` namespace only.
//! Vendor extensions (Garmin `gpxx`, for example) are ignored on read and never
//! emitted on write, so files stay portable.
//!
//! # Determinism
//!
//! The writer never emits a `<time>` element unless the caller supplies one.
//! That keeps export output byte-identical for identical input, which the test
//! suite relies on and which makes diffing two exports meaningful.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod document;
mod error;
mod reader;
mod writer;

pub use document::{GpxDocument, GpxTrackPoint, GpxTrackSegment, GpxWaypoint};
pub use error::GpxError;
pub use reader::parse_gpx;
pub use writer::{WriteOptions, write_gpx};

/// The GPX schema version this crate produces.
pub const GPX_VERSION: &str = "1.1";

/// Creator string embedded in exported files.
///
/// Includes the crate version so a bug report can identify the generating
/// build; the project name matches the repository.
pub const CREATOR: &str = concat!("harz-stamp-route-planner/", env!("CARGO_PKG_VERSION"));
