// SPDX-License-Identifier: MIT OR Apache-2.0

//! Route providers.

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};

use crate::{DEFAULT_DETOUR_FACTOR, DEFAULT_WALKING_SPEED_MPS, RoutingError};

/// A single routed leg between two points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteLeg {
    /// Distance along the route in metres.
    pub distance_m: f64,
    /// Estimated duration in seconds.
    pub duration_s: f64,
    /// Geometry as a sequence of positions, first equal to the origin and last
    /// equal to the destination for well-formed provider responses.
    pub geometry: Vec<LatLon>,
    /// Whether the distance is an estimate rather than a routed result.
    ///
    /// The user interface must surface this: presenting a straight-line
    /// estimate as a measured hiking distance would be misleading.
    pub estimated: bool,
}

impl RouteLeg {
    /// Builds a leg, deriving nothing and trusting the caller's numbers.
    ///
    /// Used by tests and by the cache; provider implementations go through
    /// [`RouteLeg::from_geometry`] instead.
    #[must_use]
    pub fn new(distance_m: f64, duration_s: f64, geometry: Vec<LatLon>, estimated: bool) -> Self {
        Self {
            distance_m,
            duration_s,
            geometry,
            estimated,
        }
    }

    /// Builds a leg from a geometry, measuring the distance along it.
    pub fn from_geometry(
        geometry: Vec<LatLon>,
        duration_s: Option<f64>,
        speed_mps: f64,
        estimated: bool,
    ) -> Result<Self, RoutingError> {
        if geometry.len() < 2 {
            return Err(RoutingError::EmptyGeometry);
        }
        let distance_m = geo_engine::path_length_m(&geometry);
        let duration_s = duration_s.unwrap_or_else(|| {
            if speed_mps > 0.0 {
                distance_m / speed_mps
            } else {
                0.0
            }
        });
        Ok(Self {
            distance_m,
            duration_s,
            geometry,
            estimated,
        })
    }

    /// Duration in whole minutes, rounded up, for display.
    #[must_use]
    pub fn duration_minutes(&self) -> u32 {
        let minutes = self.duration_s / 60.0;
        if minutes.is_finite() && minutes > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                minutes.ceil() as u32
            }
        } else {
            0
        }
    }

    /// Distance in kilometres, for display.
    #[must_use]
    pub fn distance_km(&self) -> f64 {
        self.distance_m / 1000.0
    }
}

/// Anything that can produce a route between two positions.
///
/// Implementations must be deterministic for identical input and must not
/// panic; the browser calls this on every plan recalculation.
pub trait RouteProvider {
    /// Stable identifier used in exports and the UI.
    fn name(&self) -> &'static str;

    /// Whether this provider only estimates distances.
    fn is_estimate(&self) -> bool;

    /// Produces a leg from `from` to `to`.
    fn route(&self, from: LatLon, to: LatLon) -> Result<RouteLeg, RoutingError>;
}

/// Offline fallback: great-circle distance scaled by a detour factor.
///
/// This exists so the planner still produces a usable, clearly labelled plan
/// when no routing service is configured or reachable. It never touches the
/// network.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StraightLineProvider {
    detour_factor: f64,
    speed_mps: f64,
}

impl Default for StraightLineProvider {
    fn default() -> Self {
        Self::new(DEFAULT_DETOUR_FACTOR, DEFAULT_WALKING_SPEED_MPS)
            .expect("default constants are valid")
    }
}

impl StraightLineProvider {
    /// Creates a provider, validating both parameters.
    pub fn new(detour_factor: f64, speed_mps: f64) -> Result<Self, RoutingError> {
        if !detour_factor.is_finite() || detour_factor < 1.0 {
            return Err(RoutingError::Configuration {
                message: format!("detour factor must be >= 1.0 and finite, got {detour_factor}"),
            });
        }
        if !speed_mps.is_finite() || speed_mps <= 0.0 {
            return Err(RoutingError::Configuration {
                message: format!("walking speed must be positive and finite, got {speed_mps}"),
            });
        }
        Ok(Self {
            detour_factor,
            speed_mps,
        })
    }

    /// The configured detour factor.
    #[must_use]
    pub fn detour_factor(&self) -> f64 {
        self.detour_factor
    }

    /// The configured walking speed in metres per second.
    #[must_use]
    pub fn speed_mps(&self) -> f64 {
        self.speed_mps
    }
}

impl RouteProvider for StraightLineProvider {
    fn name(&self) -> &'static str {
        "straight-line-estimate"
    }

    fn is_estimate(&self) -> bool {
        true
    }

    fn route(&self, from: LatLon, to: LatLon) -> Result<RouteLeg, RoutingError> {
        let distance_m = from.distance_m(to) * self.detour_factor;
        Ok(RouteLeg {
            distance_m,
            duration_s: distance_m / self.speed_mps,
            geometry: vec![from, to],
            estimated: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    #[test]
    fn default_provider_is_configured_sensibly() {
        let provider = StraightLineProvider::default();
        assert!((provider.detour_factor() - DEFAULT_DETOUR_FACTOR).abs() < 1e-12);
        assert!((provider.speed_mps() - DEFAULT_WALKING_SPEED_MPS).abs() < 1e-12);
        assert!(provider.is_estimate());
        assert_eq!(provider.name(), "straight-line-estimate");
    }

    #[test]
    fn estimate_grows_the_direct_distance() {
        let provider = StraightLineProvider::default();
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        let leg = provider.route(a, b).expect("route");
        let direct = a.distance_m(b);
        assert!(leg.estimated);
        assert!((leg.distance_m - direct * 1.25).abs() < 1e-6);
        assert_eq!(leg.geometry, vec![a, b]);
    }

    #[test]
    fn duration_follows_walking_speed() {
        let provider = StraightLineProvider::new(1.0, 1.0).expect("provider");
        let leg = provider
            .route(ll(51.8, 10.6), ll(51.81, 10.6))
            .expect("route");
        assert!((leg.duration_s - leg.distance_m).abs() < 1e-6);
    }

    #[test]
    fn identical_points_produce_a_degenerate_but_valid_leg() {
        let provider = StraightLineProvider::default();
        let point = ll(51.8, 10.6);
        let leg = provider.route(point, point).expect("route");
        assert!(leg.distance_m.abs() < 1e-9);
        assert!(leg.duration_s.abs() < 1e-9);
        assert_eq!(leg.geometry.len(), 2);
    }

    #[test]
    fn routing_is_deterministic() {
        let provider = StraightLineProvider::default();
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        assert_eq!(provider.route(a, b), provider.route(a, b));
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        assert!(StraightLineProvider::new(0.5, 1.25).is_err());
        assert!(StraightLineProvider::new(f64::NAN, 1.25).is_err());
        assert!(StraightLineProvider::new(1.25, 0.0).is_err());
        assert!(StraightLineProvider::new(1.25, -1.0).is_err());
    }

    #[test]
    fn duration_minutes_rounds_up() {
        let leg = RouteLeg::new(1000.0, 61.0, vec![ll(51.8, 10.6), ll(51.9, 10.6)], false);
        assert_eq!(leg.duration_minutes(), 2);
        let zero = RouteLeg::new(0.0, 0.0, vec![ll(51.8, 10.6), ll(51.9, 10.6)], false);
        assert_eq!(zero.duration_minutes(), 0);
    }

    #[test]
    fn distance_km_converts() {
        let leg = RouteLeg::new(2_500.0, 1.0, vec![ll(51.8, 10.6), ll(51.9, 10.6)], false);
        assert!((leg.distance_km() - 2.5).abs() < 1e-12);
    }

    #[test]
    fn from_geometry_requires_two_points() {
        assert!(matches!(
            RouteLeg::from_geometry(vec![ll(51.8, 10.6)], None, 1.25, false),
            Err(RoutingError::EmptyGeometry)
        ));
    }

    #[test]
    fn from_geometry_measures_the_path() {
        let a = ll(51.8, 10.6);
        let b = ll(51.85, 10.6);
        let c = ll(51.9, 10.6);
        let leg = RouteLeg::from_geometry(vec![a, b, c], None, 1.25, false).expect("leg");
        let expected = a.distance_m(b) + b.distance_m(c);
        assert!((leg.distance_m - expected).abs() < 1e-6);
        assert!(!leg.estimated);
    }

    #[test]
    fn from_geometry_prefers_provider_duration() {
        let leg = RouteLeg::from_geometry(
            vec![ll(51.8, 10.6), ll(51.9, 10.6)],
            Some(1_800.0),
            1.25,
            false,
        )
        .expect("leg");
        assert!((leg.duration_s - 1_800.0).abs() < 1e-9);
    }
}
