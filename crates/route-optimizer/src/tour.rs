// SPDX-License-Identifier: MIT OR Apache-2.0

//! Materialising the single day tour from a visiting order.

use stamp_core::{Stamp, StampId};

use crate::{DistanceMatrix, Leg, LegNode};

/// Round-trip distance for a visiting order, in metres.
///
/// The tour starts and ends at the car park (node `0`), so the return leg is
/// always included. An empty order costs nothing.
#[must_use]
pub fn round_trip_m(matrix: &DistanceMatrix, order: &[usize]) -> f64 {
    let Some(first) = order.first() else {
        return 0.0;
    };
    let mut total = matrix.get(0, *first);
    for pair in order.windows(2) {
        total += matrix.get(pair[0], pair[1]);
    }
    total + matrix.get(*order.last().expect("non-empty"), 0)
}

/// Turns the visiting order into legs and collects the visited stations.
///
/// Returns the legs (car park first and last), the total distance, and the
/// station ids in walking order.
pub fn build_tour(
    order: &[usize],
    matrix: &DistanceMatrix,
    stamps: &[Stamp],
) -> (Vec<Leg>, f64, Vec<StampId>) {
    let node_of = |index: usize| -> LegNode {
        index
            .checked_sub(1)
            .and_then(|position| stamps.get(position))
            .map_or(LegNode::Parking, |stamp| LegNode::Stamp(stamp.id))
    };

    let mut legs = Vec::with_capacity(order.len() + 1);
    let mut visited = Vec::with_capacity(order.len());
    let mut distance_m = 0.0;
    let mut previous = 0_usize;

    for &node in order {
        let distance = matrix.get(previous, node);
        distance_m += distance;
        legs.push(Leg {
            from: node_of(previous),
            to: node_of(node),
            distance_m: distance,
        });
        if let Some(stamp) = node
            .checked_sub(1)
            .and_then(|position| stamps.get(position))
        {
            visited.push(stamp.id);
        }
        previous = node;
    }

    // The return leg closes the round trip.
    let return_m = matrix.get(previous, 0);
    distance_m += return_m;
    legs.push(Leg {
        from: node_of(previous),
        to: LegNode::Parking,
        distance_m: return_m,
    });

    (legs, distance_m, visited)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{line_matrix, synthetic_matrix, synthetic_stamps};

    #[test]
    fn empty_order_costs_nothing() {
        let matrix = DistanceMatrix::new(vec![0.0], 1).expect("matrix");
        assert_eq!(round_trip_m(&matrix, &[]), 0.0);
    }

    #[test]
    fn round_trip_includes_the_return_leg() {
        let matrix = line_matrix(&[0.0, 10.0, 20.0]);
        // Park -> node 1 -> node 2 -> park = 10 + 10 + 20.
        assert!((round_trip_m(&matrix, &[1, 2]) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn a_single_station_visit_doubles_its_distance() {
        let matrix = line_matrix(&[0.0, 30.0]);
        assert!((round_trip_m(&matrix, &[1]) - 60.0).abs() < 1e-9);
    }

    #[test]
    fn legs_start_and_end_at_parking() {
        let matrix = synthetic_matrix();
        let stamps = synthetic_stamps(matrix.node_count() - 1);
        let (legs, _, _) = build_tour(&[1, 2, 3], &matrix, &stamps);
        assert_eq!(legs.first().expect("leg").from, LegNode::Parking);
        assert_eq!(legs.last().expect("leg").to, LegNode::Parking);
        assert_eq!(legs.len(), stamps.len() + 1);
    }

    #[test]
    fn distance_is_the_sum_of_the_legs() {
        let matrix = synthetic_matrix();
        let stamps = synthetic_stamps(3);
        let (legs, distance_m, _) = build_tour(&[1, 2, 3], &matrix, &stamps);
        let sum: f64 = legs.iter().map(|leg| leg.distance_m).sum();
        assert!((distance_m - sum).abs() < 1e-9);
    }

    #[test]
    fn visited_stations_keep_the_walking_order() {
        let matrix = synthetic_matrix();
        let stamps = synthetic_stamps(3);
        let (_, _, visited) = build_tour(&[3, 1, 2], &matrix, &stamps);
        assert_eq!(
            visited.iter().map(|id| id.number()).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
    }
}
