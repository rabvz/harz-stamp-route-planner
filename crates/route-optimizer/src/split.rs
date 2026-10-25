// SPDX-License-Identifier: MIT OR Apache-2.0

//! Splitting an order into days that respect the distance limit.

use stamp_core::Stamp;

use crate::{DayPlan, DistanceMatrix, Leg, LegNode, OptimizerOptions, OptimizerWarning};

/// Round-trip distance for a day, in metres.
///
/// A day starts and ends at the car park (node `0`), so the return leg is
/// always included. An empty day costs nothing.
#[must_use]
pub fn day_round_trip_m(matrix: &DistanceMatrix, stamps: &[usize]) -> f64 {
    let Some(first) = stamps.first() else {
        return 0.0;
    };
    let mut total = matrix.get(0, *first);
    for pair in stamps.windows(2) {
        total += matrix.get(pair[0], pair[1]);
    }
    total + matrix.get(*stamps.last().expect("non-empty"), 0)
}

/// Groups the visiting order into days and materialises the itinerary legs.
pub fn build_days(
    order: &[usize],
    matrix: &DistanceMatrix,
    stamps: &[Stamp],
    options: OptimizerOptions,
) -> (Vec<DayPlan>, Vec<OptimizerWarning>) {
    let groups = split_into_days(order, matrix, options);
    let mut warnings = Vec::new();
    let mut days = Vec::with_capacity(groups.len());

    for (index, group) in groups.iter().enumerate() {
        let day_number = index + 1;
        let distance_m = day_round_trip_m(matrix, group);
        let oversized = distance_m > options.max_day_distance_m + 1e-9;

        let ids: Vec<_> = group
            .iter()
            .filter_map(|node| node.checked_sub(1))
            .filter_map(|position| stamps.get(position))
            .map(|stamp| stamp.id)
            .collect();

        let legs = build_legs(group, matrix, stamps);

        if oversized {
            // Only single-station days are unavoidable; a multi-station day
            // above the limit would indicate a bug in the splitter.
            if let Some(first) = ids.first() {
                warnings.push(OptimizerWarning::OversizedDay {
                    day: day_number,
                    distance_m,
                    limit_m: options.max_day_distance_m,
                    stamp: *first,
                });
            }
        }

        days.push(DayPlan {
            day: day_number,
            legs,
            distance_m,
            stamps: ids,
            oversized,
        });
    }

    if days.len() > options.max_days {
        warnings.push(OptimizerWarning::DayLimitExceeded {
            required: days.len(),
            allowed: options.max_days,
        });
    }

    (days, warnings)
}

/// Packs the order into days, starting a new day when the limit would be
/// exceeded. A station that cannot fit in any day is kept in a day of its own.
fn split_into_days(
    order: &[usize],
    matrix: &DistanceMatrix,
    options: OptimizerOptions,
) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();

    for &node in order {
        let mut candidate = current.clone();
        candidate.push(node);
        let cost = day_round_trip_m(matrix, &candidate);

        if !current.is_empty() && cost > options.max_day_distance_m + 1e-9 {
            groups.push(std::mem::take(&mut current));
            current.push(node);
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Turns a day's node indices into legs, parking first and last.
fn build_legs(group: &[usize], matrix: &DistanceMatrix, stamps: &[Stamp]) -> Vec<Leg> {
    let node_of = |index: usize| -> LegNode {
        index
            .checked_sub(1)
            .and_then(|position| stamps.get(position))
            .map_or(LegNode::Parking, |stamp| LegNode::Stamp(stamp.id))
    };

    let mut legs = Vec::with_capacity(group.len() + 1);
    let mut previous = 0_usize;
    for &node in group {
        legs.push(Leg {
            from: node_of(previous),
            to: node_of(node),
            distance_m: matrix.get(previous, node),
        });
        previous = node;
    }
    legs.push(Leg {
        from: node_of(previous),
        to: LegNode::Parking,
        distance_m: matrix.get(previous, 0),
    });
    legs
}

/// Convenience for callers that want only the day count for a selection.
#[must_use]
pub fn days_needed(order: &[usize], matrix: &DistanceMatrix, options: OptimizerOptions) -> usize {
    split_into_days(order, matrix, options).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{line_matrix, synthetic_matrix};

    fn options(limit: f64) -> OptimizerOptions {
        OptimizerOptions {
            max_day_distance_m: limit,
            max_days: 99,
            two_opt_passes: 4,
        }
    }

    #[test]
    fn empty_day_costs_nothing() {
        let matrix = DistanceMatrix::new(vec![0.0], 1).expect("matrix");
        assert_eq!(day_round_trip_m(&matrix, &[]), 0.0);
    }

    #[test]
    fn round_trip_includes_the_return_leg() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0]);
        // Park -> node 1 -> node 2 -> park = 10 + 10 + 20.
        assert!((day_round_trip_m(&matrix, &[1, 2]) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn a_single_station_day_doubles_its_distance() {
        let matrix = line_matrix(&[0.0, 30.0]);
        assert!((day_round_trip_m(&matrix, &[1]) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn split_respects_the_limit() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0, 30.0]);
        let groups = split_into_days(&[1, 2, 3], &matrix, options(45.0));
        assert!(groups.len() > 1, "the limit must force a split");
        for group in &groups {
            let cost = day_round_trip_m(&matrix, group);
            // A single station that is too far away cannot be split further,
            // so it is kept alone and reported instead of dropped.
            if group.len() > 1 {
                assert!(cost <= 45.0 + 1e-9, "multi-station day is {cost} m");
            }
        }
        assert_eq!(groups.iter().map(Vec::len).sum::<usize>(), 3);
    }

    #[test]
    fn split_keeps_everything_in_one_day_when_it_fits() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0, 30.0]);
        let groups = split_into_days(&[1, 2, 3], &matrix, options(1_000.0));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![1, 2, 3]);
    }

    #[test]
    fn split_keeps_an_unreachable_station_in_its_own_day() {
        let matrix = line_matrix(&[0.0, 1_000.0]);
        let groups = split_into_days(&[1], &matrix, options(100.0));
        assert_eq!(groups.len(), 1, "the station must not be dropped");
    }

    #[test]
    fn legs_start_and_end_at_parking() {
        let matrix = synthetic_matrix();
        let stamps = crate::test_support::synthetic_stamps(matrix.node_count() - 1);
        let (days, _) = build_days(&[1, 2, 3], &matrix, &stamps, options(1_000.0));
        for day in &days {
            assert_eq!(day.legs.first().expect("leg").from, LegNode::Parking);
            assert_eq!(day.legs.last().expect("leg").to, LegNode::Parking);
            assert_eq!(day.legs.len(), day.stamps.len() + 1);
        }
    }

    #[test]
    fn oversized_single_day_is_flagged() {
        let matrix = line_matrix(&[0.0, 1_000.0]);
        let stamps = crate::test_support::synthetic_stamps(1);
        let (days, warnings) = build_days(&[1], &matrix, &stamps, options(100.0));
        assert!(days[0].oversized);
        assert!(matches!(
            warnings.as_slice(),
            [OptimizerWarning::OversizedDay { day: 1, .. }]
        ));
    }

    #[test]
    fn day_limit_warning_is_emitted() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0, 30.0]);
        let stamps = crate::test_support::synthetic_stamps(3);
        let mut opts = options(45.0);
        opts.max_days = 1;
        let (days, warnings) = build_days(&[1, 2, 3], &matrix, &stamps, opts);
        assert!(days.len() > 1);
        assert!(warnings.iter().any(|warning| matches!(
            warning,
            OptimizerWarning::DayLimitExceeded { allowed: 1, .. }
        )));
    }

    #[test]
    fn days_needed_matches_build_days() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0, 30.0]);
        let groups = split_into_days(&[1, 2, 3], &matrix, options(45.0));
        assert_eq!(
            days_needed(&[1, 2, 3], &matrix, options(45.0)),
            groups.len()
        );
    }
}
