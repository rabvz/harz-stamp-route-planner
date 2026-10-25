// SPDX-License-Identifier: MIT OR Apache-2.0

//! Routing abstraction, provider implementations and caching.
//!
//! # Why there is no HTTP client here
//!
//! The application ships as a static site on GitHub Pages, so it has no server
//! that could hold API keys or proxy requests. Network access therefore lives
//! wherever it is cheapest and most transparent:
//!
//! * in the browser, where the web app calls a routing service with `fetch` and
//!   hands the raw response to [`parse_geojson_route`];
//! * or in a future optional backend that uses exactly the same parsing code.
//!
//! Keeping I/O out of this crate is what allows the identical logic to run
//! natively in tests and inside WebAssembly, and it makes every routing
//! decision from untrusted JSON testable without a network.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod cache;
mod error;
mod geojson;
mod provider;

pub use cache::RouteCache;
pub use error::RoutingError;
pub use geojson::parse_geojson_route;
pub use provider::{RouteLeg, RouteProvider, StraightLineProvider};

/// Default factor applied to the great-circle distance when no routing service
/// is available.
///
/// Trail networks are not straight lines; 1.25 is a deliberately conservative
/// multiplier that keeps the offline fallback from underestimating a hike. It
/// is an estimate, and every result derived from it is flagged as such.
pub const DEFAULT_DETOUR_FACTOR: f64 = 1.25;

/// Default walking speed used to turn a distance into a duration, in metres per
/// second (4.5 km/h, a common planning value for hiking with breaks excluded).
pub const DEFAULT_WALKING_SPEED_MPS: f64 = 1.25;
