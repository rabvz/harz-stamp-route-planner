// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tour construction and improvement for single-day stamp collecting hikes.
//!
//! # Shape of the problem
//!
//! A day hike starts and ends at the same car park and visits a set of stamp
//! stations. The plan is therefore one closed tour: an open path from the car
//! park through the chosen stations and back. The planner uses:
//!
//! 1. **Distance matrix** over the car park and the chosen stations. The caller
//!    supplies it, typically from a real routing service; when it does not, the
//!    straight-line provider fills it in and the result is flagged as an
//!    estimate.
//! 2. **Nearest-neighbour construction** to obtain an initial order, breaking
//!    ties on station number so the result is reproducible.
//! 3. **2-opt improvement** on the open path with the car park pinned at the
//!    start. This removes the most visible inefficiencies at negligible cost.
//!
//! The result is deterministic, explainable, and never silently drops a chosen
//! station: when the tour exceeds the configured distance limit, it is returned
//! and flagged instead of being shortened or split into several days.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod matrix;
mod order;
mod tour;

#[cfg(test)]
mod test_support;

pub use error::{OptimizerError, OptimizerWarning};
pub use matrix::DistanceMatrix;
pub use order::optimize_order;
pub use tour::{build_tour, round_trip_m};

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};
use stamp_core::{Stamp, StampId};

/// Default upper bound for one hike, in metres.
pub const DEFAULT_MAX_TOUR_DISTANCE_M: f64 = 20_000.0;

/// Planning limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OptimizerOptions {
    /// Maximum walking distance of the whole tour in metres, including the
    /// return leg.
    pub max_tour_distance_m: f64,
    /// How many 2-opt improvement passes to attempt.
    pub two_opt_passes: usize,
}

impl Default for OptimizerOptions {
    fn default() -> Self {
        Self {
            max_tour_distance_m: DEFAULT_MAX_TOUR_DISTANCE_M,
            two_opt_passes: 8,
        }
    }
}

impl OptimizerOptions {
    /// Validates the options.
    pub fn validate(&self) -> Result<(), OptimizerError> {
        if !self.max_tour_distance_m.is_finite() || self.max_tour_distance_m <= 0.0 {
            return Err(OptimizerError::InvalidOptions {
                message: format!(
                    "max_tour_distance_m must be positive and finite, got {}",
                    self.max_tour_distance_m
                ),
            });
        }
        Ok(())
    }
}

/// One end of a leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "stamp")]
pub enum LegNode {
    /// The car park, where the tour starts and ends.
    Parking,
    /// A stamp station.
    Stamp(StampId),
}

impl LegNode {
    /// Display helper used by the GPX writer and the UI.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Parking => "Parkplatz".to_string(),
            Self::Stamp(id) => id.code(),
        }
    }
}

/// A walked leg between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Leg {
    /// Where the leg starts.
    pub from: LegNode,
    /// Where the leg ends.
    pub to: LegNode,
    /// Distance in metres.
    pub distance_m: f64,
}

/// The finished day tour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tour {
    /// Position the tour starts and ends at.
    pub start: LatLon,
    /// Legs in walking order, starting and ending at the car park.
    pub legs: Vec<Leg>,
    /// Total distance of the tour in metres, including the return leg.
    pub distance_m: f64,
    /// Stations visited, in walking order.
    pub stamps_visited: Vec<StampId>,
    /// Name of the distance source, for attribution.
    pub provider_name: String,
    /// True when distances are estimates rather than routed results.
    pub estimated: bool,
    /// Whether the tour exceeds the configured limit.
    pub oversized: bool,
    /// Constraints that could not be satisfied.
    pub warnings: Vec<OptimizerWarning>,
}

impl Tour {
    /// Number of stations visited.
    #[must_use]
    pub fn stamp_count(&self) -> usize {
        self.stamps_visited.len()
    }

    /// Total distance in kilometres, for display.
    #[must_use]
    pub fn distance_km(&self) -> f64 {
        self.distance_m / 1000.0
    }
}

/// Input for [`plan`].
#[derive(Debug, Clone)]
pub struct PlanRequest {
    /// Car park where the tour starts and ends.
    pub start: LatLon,
    /// Stations to visit; order is irrelevant, ids must be unique.
    pub stamps: Vec<Stamp>,
    /// Optional distance matrix over `[start, stamps...]`, row-major.
    ///
    /// Must contain `(n + 1)^2` entries where `n` is the number of stamps.
    pub matrix: Option<Vec<f64>>,
    /// Planning limits.
    pub options: OptimizerOptions,
    /// How the distances were obtained, for attribution in exports.
    pub provider_name: String,
}

/// Builds a day tour from a request.
pub fn plan(request: PlanRequest) -> Result<Tour, OptimizerError> {
    request.options.validate()?;
    if request.stamps.is_empty() {
        return Err(OptimizerError::NoStamps);
    }

    // Reject duplicate stations: visiting the same box twice would produce a
    // plan that looks valid but wastes part of the day.
    let mut seen = std::collections::BTreeSet::new();
    for stamp in &request.stamps {
        if !seen.insert(stamp.id) {
            return Err(OptimizerError::DuplicateStamp {
                number: stamp.id.number(),
            });
        }
    }

    let node_count = request.stamps.len() + 1;
    // Whether the plan can claim measured distances depends on where the
    // matrix came from, so record that before the matrix is moved.
    let (matrix, estimated) = match request.matrix {
        Some(data) => (DistanceMatrix::new(data, node_count)?, false),
        None => (
            DistanceMatrix::straight_line(request.start, &request.stamps)?,
            true,
        ),
    };

    let order = optimize_order(&matrix, request.options.two_opt_passes);
    let (legs, distance_m, stamps_visited) = build_tour(&order, &matrix, &request.stamps);

    let oversized = distance_m > request.options.max_tour_distance_m + 1e-9;
    let mut warnings = Vec::new();
    if oversized {
        warnings.push(OptimizerWarning::OversizedTour {
            distance_m,
            limit_m: request.options.max_tour_distance_m,
        });
    }

    Ok(Tour {
        start: request.start,
        legs,
        distance_m,
        stamps_visited,
        provider_name: request.provider_name,
        estimated,
        oversized,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use stamp_core::StampCatalog;

    fn catalog() -> &'static StampCatalog {
        StampCatalog::embedded()
    }

    fn stamp(number: u16) -> Stamp {
        catalog()
            .get(StampId::new(number).expect("id"))
            .expect("station")
            .clone()
    }

    fn request(numbers: &[u16], options: OptimizerOptions) -> PlanRequest {
        let stamps: Vec<Stamp> = numbers.iter().map(|number| stamp(*number)).collect();
        PlanRequest {
            start: stamps
                .first()
                .map_or_else(|| LatLon::new(51.8, 10.6).expect("valid"), |s| s.position),
            stamps,
            matrix: None,
            options,
            provider_name: "test".to_string(),
        }
    }

    #[test]
    fn empty_selection_is_an_error() {
        let result = plan(request(&[], OptimizerOptions::default()));
        assert!(matches!(result, Err(OptimizerError::NoStamps)));
    }

    #[test]
    fn duplicate_stations_are_rejected() {
        let result = plan(request(&[1, 1], OptimizerOptions::default()));
        assert!(matches!(
            result,
            Err(OptimizerError::DuplicateStamp { number: 1 })
        ));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let options = OptimizerOptions {
            max_tour_distance_m: 0.0,
            ..Default::default()
        };
        assert!(matches!(
            plan(request(&[1], options)),
            Err(OptimizerError::InvalidOptions { .. })
        ));
    }

    #[test]
    fn single_station_round_trip_has_two_legs() {
        let station = stamp(1);
        let start = station.position;
        let tour = plan(PlanRequest {
            start,
            stamps: vec![station.clone()],
            matrix: None,
            options: OptimizerOptions::default(),
            provider_name: "test".to_string(),
        })
        .expect("tour");

        assert_eq!(tour.stamp_count(), 1);
        assert_eq!(tour.legs.len(), 2);
        assert_eq!(tour.legs[0].from, LegNode::Parking);
        assert_eq!(tour.legs[1].to, LegNode::Parking);
        assert!(tour.distance_m.abs() < 1e-6, "zero-length round trip");
        assert!(!tour.oversized);
    }

    #[test]
    fn every_station_is_visited_exactly_once() {
        let numbers = [1_u16, 2, 3, 4, 5, 9, 10, 20];
        let tour = plan(request(&numbers, OptimizerOptions::default())).expect("tour");
        let mut visited: Vec<u16> = tour.stamps_visited.iter().map(|id| id.number()).collect();
        visited.sort_unstable();
        assert_eq!(visited, numbers.to_vec());
    }

    #[test]
    fn planning_is_deterministic() {
        let numbers = [1_u16, 2, 3, 4, 5];
        let first = plan(request(&numbers, OptimizerOptions::default())).expect("first");
        let second = plan(request(&numbers, OptimizerOptions::default())).expect("second");
        assert_eq!(first, second);
    }

    #[test]
    fn every_station_lies_on_the_tour() {
        let numbers: Vec<u16> = (1..=10).collect();
        let tour = plan(request(&numbers, OptimizerOptions::default())).expect("tour");
        assert_eq!(tour.stamp_count(), numbers.len());
        for id in &tour.stamps_visited {
            assert!(
                tour.legs.iter().any(|leg| leg.to == LegNode::Stamp(*id)),
                "station {id} is not reached by any leg"
            );
        }
    }

    #[test]
    fn tour_starts_and_ends_at_the_car_park() {
        let numbers: Vec<u16> = (1..=8).collect();
        let tour = plan(request(&numbers, OptimizerOptions::default())).expect("tour");
        assert_eq!(
            tour.legs.first().map(|leg| leg.from),
            Some(LegNode::Parking)
        );
        assert_eq!(tour.legs.last().map(|leg| leg.to), Some(LegNode::Parking));
        assert_eq!(tour.legs.len(), tour.stamp_count() + 1);
    }

    #[test]
    fn a_tour_above_the_limit_is_flagged_but_keeps_every_station() {
        let options = OptimizerOptions {
            max_tour_distance_m: 100.0,
            two_opt_passes: 1,
        };
        let tour = plan(request(&[1, 222], options)).expect("tour");
        assert_eq!(tour.stamp_count(), 2, "no station may be dropped");
        assert!(tour.oversized);
        assert!(
            tour.warnings
                .iter()
                .any(|warning| matches!(warning, OptimizerWarning::OversizedTour { .. })),
            "expected an oversized-tour warning, got {:?}",
            tour.warnings
        );
    }

    #[test]
    fn a_large_selection_is_not_split_into_days() {
        // The product plans day tours only. A selection that cannot be walked in
        // one day is returned as one oversized tour with every station, never as
        // several days.
        let options = OptimizerOptions {
            max_tour_distance_m: 1_000.0,
            two_opt_passes: 2,
        };
        let numbers: Vec<u16> = (1..=6).collect();
        let tour = plan(request(&numbers, options)).expect("tour");
        assert_eq!(tour.stamp_count(), 6);
        assert!(tour.oversized);
        assert_eq!(
            tour.warnings
                .iter()
                .filter(|warning| matches!(warning, OptimizerWarning::OversizedTour { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn supplied_matrix_overrides_straight_line_distances() {
        let stations = vec![stamp(1), stamp(2)];
        let start = stations[0].position;
        // Three nodes: 0 = parking, 1 = HWN001, 2 = HWN002.
        // Parking coincides with HWN001, so a symmetric matrix is easy to state.
        let matrix = vec![
            0.0, 0.0, 1_000.0, //
            0.0, 0.0, 1_000.0, //
            1_000.0, 1_000.0, 0.0,
        ];
        let tour = plan(PlanRequest {
            start,
            stamps: stations,
            matrix: Some(matrix),
            options: OptimizerOptions::default(),
            provider_name: "test-matrix".to_string(),
        })
        .expect("tour");
        assert_eq!(tour.provider_name, "test-matrix");
        assert!(!tour.estimated);
        assert!((tour.distance_m - 2_000.0).abs() < 1e-6);
    }

    #[test]
    fn wrong_matrix_size_is_rejected() {
        let stations = vec![stamp(1), stamp(2)];
        let result = plan(PlanRequest {
            start: stations[0].position,
            stamps: stations,
            matrix: Some(vec![0.0; 4]),
            options: OptimizerOptions::default(),
            provider_name: "test".to_string(),
        });
        assert!(matches!(result, Err(OptimizerError::MatrixShape { .. })));
    }

    #[test]
    fn negative_matrix_entry_is_rejected() {
        let stations = vec![stamp(1)];
        let result = plan(PlanRequest {
            start: stations[0].position,
            stamps: stations,
            matrix: Some(vec![0.0, 1.0, 1.0, -5.0]),
            options: OptimizerOptions::default(),
            provider_name: "test".to_string(),
        });
        assert!(matches!(
            result,
            Err(OptimizerError::InvalidDistance { .. })
        ));
    }

    #[test]
    fn distance_equals_the_sum_of_legs() {
        let numbers: Vec<u16> = (1..=12).collect();
        let tour = plan(request(&numbers, OptimizerOptions::default())).expect("tour");
        let sum: f64 = tour.legs.iter().map(|leg| leg.distance_m).sum();
        assert!((tour.distance_m - sum).abs() < 1e-6);
    }
}
