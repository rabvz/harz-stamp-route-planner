// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tour construction and improvement for multi-day stamp collecting hikes.
//!
//! # Shape of the problem
//!
//! A hike starts and ends at the same car park and visits a set of stamp
//! stations. A multi-day plan is therefore a sequence of closed days, each of
//! which is an open path from the car park back to itself. This is a vehicle
//! routing problem with a working-time (distance) limit per day, which is
//! NP-hard. The planner uses:
//!
//! 1. **Distance matrix** over the car park and the chosen stations. The caller
//!    supplies it, typically from a real routing service; when it does not, the
//!    straight-line provider fills it in and the result is flagged as an
//!    estimate.
//! 2. **Nearest-neighbour construction** to obtain an initial order, breaking
//!    ties on station number so the result is reproducible.
//! 3. **2-opt improvement** on the open path with the car park pinned at the
//!    start. This removes the most visible inefficiencies at negligible cost.
//! 4. **Greedy day splitting** under the per-day distance limit.
//!
//! The result is deterministic, explainable, and never silently drops a chosen
//! station: when a constraint cannot be met, the itinerary reports a warning
//! instead of pretending otherwise.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod matrix;
mod order;
mod split;

#[cfg(test)]
mod test_support;

pub use error::{OptimizerError, OptimizerWarning};
pub use matrix::DistanceMatrix;
pub use order::optimize_order;
pub use split::days_needed;

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};
use stamp_core::{Stamp, StampId};

use crate::split::{build_days, day_round_trip_m};

/// Default upper bound for one hiking day, in metres.
pub const DEFAULT_MAX_DAY_DISTANCE_M: f64 = 20_000.0;

/// Default ceiling on the number of days a plan may span.
pub const DEFAULT_MAX_DAYS: usize = 14;

/// Planning limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OptimizerOptions {
    /// Maximum walking distance per day in metres, including the return leg.
    pub max_day_distance_m: f64,
    /// Maximum number of days the itinerary may span.
    pub max_days: usize,
    /// How many 2-opt improvement passes to attempt.
    pub two_opt_passes: usize,
}

impl Default for OptimizerOptions {
    fn default() -> Self {
        Self {
            max_day_distance_m: DEFAULT_MAX_DAY_DISTANCE_M,
            max_days: DEFAULT_MAX_DAYS,
            two_opt_passes: 8,
        }
    }
}

impl OptimizerOptions {
    /// Validates the options.
    pub fn validate(&self) -> Result<(), OptimizerError> {
        if !self.max_day_distance_m.is_finite() || self.max_day_distance_m <= 0.0 {
            return Err(OptimizerError::InvalidOptions {
                message: format!(
                    "max_day_distance_m must be positive and finite, got {}",
                    self.max_day_distance_m
                ),
            });
        }
        if self.max_days == 0 {
            return Err(OptimizerError::InvalidOptions {
                message: "max_days must be at least 1".to_string(),
            });
        }
        Ok(())
    }
}

/// One end of a leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "stamp")]
pub enum LegNode {
    /// The day's car park, where every day starts and ends.
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

/// One hiking day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPlan {
    /// One-based day number, for display.
    pub day: usize,
    /// Legs in walking order, starting and ending at the car park.
    pub legs: Vec<Leg>,
    /// Total distance of the day in metres, including the return leg.
    pub distance_m: f64,
    /// Stations visited that day, in walking order.
    pub stamps: Vec<StampId>,
    /// Whether this day exceeds the configured limit because a single station
    /// cannot be reached within it.
    pub oversized: bool,
}

/// The finished plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Itinerary {
    /// Days in order.
    pub days: Vec<DayPlan>,
    /// Sum of all day distances in metres.
    pub total_distance_m: f64,
    /// Every visited station, in visiting order across days.
    pub stamps_visited: Vec<StampId>,
    /// Position the plan starts and ends at each day.
    pub start: LatLon,
    /// Name of the distance source, for attribution.
    pub provider_name: String,
    /// True when distances are estimates rather than routed results.
    pub estimated: bool,
    /// Constraints that could not be satisfied.
    pub warnings: Vec<OptimizerWarning>,
}

impl Itinerary {
    /// Number of days in the plan.
    #[must_use]
    pub fn day_count(&self) -> usize {
        self.days.len()
    }

    /// Number of stations visited.
    #[must_use]
    pub fn stamp_count(&self) -> usize {
        self.stamps_visited.len()
    }

    /// Total distance in kilometres, for display.
    #[must_use]
    pub fn total_distance_km(&self) -> f64 {
        self.total_distance_m / 1000.0
    }
}

/// Input for [`plan`].
#[derive(Debug, Clone)]
pub struct PlanRequest {
    /// Car park where every day starts and ends.
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

/// Builds an itinerary from a request.
pub fn plan(request: PlanRequest) -> Result<Itinerary, OptimizerError> {
    request.options.validate()?;
    if request.stamps.is_empty() {
        return Err(OptimizerError::NoStamps);
    }

    // Reject duplicate stations: visiting the same box twice would produce a
    // plan that looks valid but wastes a day.
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
    let (days, warnings) = build_days(&order, &matrix, &request.stamps, request.options);

    let total_distance_m = days.iter().map(|day| day.distance_m).sum();
    let stamps_visited = days
        .iter()
        .flat_map(|day| day.stamps.iter().copied())
        .collect();

    Ok(Itinerary {
        days,
        total_distance_m,
        stamps_visited,
        start: request.start,
        provider_name: request.provider_name,
        estimated,
        warnings,
    })
}

/// Computes the round-trip distance for a set of stations from a car park.
///
/// Exposed for previews ("this selection needs about 14 km per day") without
/// building a full itinerary.
#[must_use]
pub fn round_trip_m(matrix: &DistanceMatrix, stamp_indices: &[usize]) -> f64 {
    day_round_trip_m(matrix, stamp_indices)
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
            max_day_distance_m: 0.0,
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
        let itinerary = plan(PlanRequest {
            start,
            stamps: vec![station.clone()],
            matrix: None,
            options: OptimizerOptions::default(),
            provider_name: "test".to_string(),
        })
        .expect("itinerary");

        assert_eq!(itinerary.day_count(), 1);
        assert_eq!(itinerary.stamp_count(), 1);
        let day = &itinerary.days[0];
        assert_eq!(day.legs.len(), 2);
        assert_eq!(day.legs[0].from, LegNode::Parking);
        assert_eq!(day.legs[1].to, LegNode::Parking);
        assert!(day.distance_m.abs() < 1e-6, "zero-length round trip");
    }

    #[test]
    fn every_station_is_visited_exactly_once() {
        let numbers = [1_u16, 2, 3, 4, 5, 9, 10, 20];
        let itinerary = plan(request(&numbers, OptimizerOptions::default())).expect("itinerary");
        let mut visited: Vec<u16> = itinerary
            .stamps_visited
            .iter()
            .map(|id| id.number())
            .collect();
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
    fn day_limit_is_respected_when_possible() {
        let options = OptimizerOptions {
            max_day_distance_m: 6_000.0,
            max_days: 99,
            two_opt_passes: 8,
        };
        let numbers: Vec<u16> = (1..=10).collect();
        let itinerary = plan(request(&numbers, options)).expect("itinerary");
        for day in &itinerary.days {
            assert!(
                day.distance_m <= options.max_day_distance_m + 1e-6 || day.oversized,
                "day {} is {} m without being flagged",
                day.day,
                day.distance_m
            );
        }
    }

    #[test]
    fn each_day_starts_and_ends_at_the_car_park() {
        let options = OptimizerOptions {
            max_day_distance_m: 5_000.0,
            max_days: 99,
            two_opt_passes: 4,
        };
        let numbers: Vec<u16> = (1..=8).collect();
        let itinerary = plan(request(&numbers, options)).expect("itinerary");
        for day in &itinerary.days {
            assert_eq!(day.legs.first().map(|leg| leg.from), Some(LegNode::Parking));
            assert_eq!(day.legs.last().map(|leg| leg.to), Some(LegNode::Parking));
        }
    }

    #[test]
    fn day_numbers_are_one_based_and_contiguous() {
        let itinerary = plan(request(&[1, 2, 3], OptimizerOptions::default())).expect("itinerary");
        let numbers: Vec<usize> = itinerary.days.iter().map(|day| day.day).collect();
        assert_eq!(numbers, (1..=itinerary.days.len()).collect::<Vec<usize>>());
    }

    #[test]
    fn too_many_days_produces_a_warning_not_a_lost_station() {
        let options = OptimizerOptions {
            max_day_distance_m: 1_000.0,
            max_days: 1,
            two_opt_passes: 2,
        };
        let numbers: Vec<u16> = (1..=6).collect();
        let itinerary = plan(request(&numbers, options)).expect("itinerary");
        assert_eq!(itinerary.stamp_count(), 6, "no station may be dropped");
        assert!(itinerary.days.len() > options.max_days);
        assert!(
            itinerary
                .warnings
                .iter()
                .any(|warning| matches!(warning, OptimizerWarning::DayLimitExceeded { .. })),
            "expected a day-limit warning, got {:?}",
            itinerary.warnings
        );
    }

    #[test]
    fn unreachable_station_is_flagged_as_oversized() {
        let options = OptimizerOptions {
            max_day_distance_m: 100.0,
            max_days: 10,
            two_opt_passes: 1,
        };
        // HWN001 and HWN222 are roughly 50 km apart. The plan starts at HWN001,
        // so only the HWN222 day can be oversized - and it must be flagged
        // rather than silently shortened.
        let itinerary = plan(request(&[1, 222], options)).expect("itinerary");
        let far_day = itinerary
            .days
            .iter()
            .find(|day| day.stamps.contains(&StampId::new(222).expect("id")))
            .expect("a day must visit HWN222");
        assert!(far_day.oversized, "the far station day must be flagged");
        assert!(
            itinerary.warnings.iter().any(|warning| matches!(
                warning,
                OptimizerWarning::OversizedDay { stamp, .. }
                    if stamp.number() == 222
            )),
            "expected an oversized-day warning for HWN222, got {:?}",
            itinerary.warnings
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
        let itinerary = plan(PlanRequest {
            start,
            stamps: stations,
            matrix: Some(matrix),
            options: OptimizerOptions::default(),
            provider_name: "test-matrix".to_string(),
        })
        .expect("itinerary");
        assert_eq!(itinerary.provider_name, "test-matrix");
        assert!((itinerary.total_distance_m - 2_000.0).abs() < 1e-6);
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
    fn total_distance_equals_the_sum_of_days() {
        let numbers: Vec<u16> = (1..=12).collect();
        let itinerary = plan(request(&numbers, OptimizerOptions::default())).expect("itinerary");
        let sum: f64 = itinerary.days.iter().map(|day| day.distance_m).sum();
        assert!((itinerary.total_distance_m - sum).abs() < 1e-6);
    }

    #[test]
    fn leg_distances_sum_to_the_day_distance() {
        let numbers: Vec<u16> = (1..=6).collect();
        let itinerary = plan(request(&numbers, OptimizerOptions::default())).expect("itinerary");
        for day in &itinerary.days {
            let sum: f64 = day.legs.iter().map(|leg| leg.distance_m).sum();
            assert!((sum - day.distance_m).abs() < 1e-6);
        }
    }
}
