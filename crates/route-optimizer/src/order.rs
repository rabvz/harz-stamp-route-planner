// SPDX-License-Identifier: MIT OR Apache-2.0

//! Construction and improvement of the visiting order.

use crate::DistanceMatrix;

/// Produces a visiting order for the stamps in `matrix`.
///
/// Returns node indices (always `>= 1`; node `0` is the car park) in the order
/// they should be visited. The car park is implicit at both ends of the path.
///
/// The initial order comes from nearest-neighbour construction, which is fast
/// and usually within a quarter of the optimum. It is then improved with 2-opt,
/// which reverses path segments whenever that shortens the path. Both steps
/// break ties on the lower node index, so identical input always produces an
/// identical order.
#[must_use]
pub fn optimize_order(matrix: &DistanceMatrix, passes: usize) -> Vec<usize> {
    let node_count = matrix.node_count();
    if node_count <= 1 {
        return Vec::new();
    }

    let mut order = nearest_neighbour(matrix);
    two_opt(&mut order, matrix, passes);
    order
}

/// Greedy nearest-neighbour construction starting at the car park.
fn nearest_neighbour(matrix: &DistanceMatrix) -> Vec<usize> {
    let node_count = matrix.node_count();
    let mut unvisited: Vec<usize> = (1..node_count).collect();
    let mut order = Vec::with_capacity(unvisited.len());
    let mut current = 0_usize;

    while !unvisited.is_empty() {
        let mut best_position = 0;
        let mut best_distance = f64::INFINITY;
        for (position, candidate) in unvisited.iter().enumerate() {
            let distance = matrix.get(current, *candidate);
            // Strict comparison keeps the earliest candidate on ties, and the
            // list is ascending, so ties resolve to the lower node index.
            if distance < best_distance {
                best_distance = distance;
                best_position = position;
            }
        }
        current = unvisited.remove(best_position);
        order.push(current);
    }
    order
}

/// Improves a path with repeated 2-opt passes.
///
/// The path is `[car park, ...order]`; the car park stays first. Reversing a
/// segment is only applied when it strictly shortens the path, which guarantees
/// termination and makes the result independent of pass count beyond
/// convergence.
fn two_opt(order: &mut [usize], matrix: &DistanceMatrix, passes: usize) {
    if order.len() < 2 || passes == 0 {
        return;
    }

    for _ in 0..passes {
        let mut improved = false;
        // Positions in the implicit path including the car park at index 0.
        let path_len = order.len() + 1;

        for start in 1..path_len.saturating_sub(1) {
            for end in (start + 1)..path_len {
                // Nodes in path coordinates.
                let node = |path_index: usize| -> usize {
                    if path_index == 0 {
                        0
                    } else {
                        order[path_index - 1]
                    }
                };

                let a = node(start - 1);
                let b = node(start);
                let c = node(end);
                let next = if end + 1 < path_len {
                    Some(node(end + 1))
                } else {
                    None
                };

                let removed = matrix.get(a, b) + next.map_or(0.0, |next| matrix.get(c, next));
                let added = matrix.get(a, c) + next.map_or(0.0, |next| matrix.get(b, next));

                if added < removed - 1e-9 {
                    order[(start - 1)..end].reverse();
                    improved = true;
                }
            }
        }

        if !improved {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::synthetic_matrix;

    #[test]
    fn empty_and_single_node_matrices_produce_empty_order() {
        let matrix = DistanceMatrix::new(vec![0.0], 1).expect("matrix");
        assert!(optimize_order(&matrix, 4).is_empty());

        let matrix = DistanceMatrix::new(vec![0.0, 5.0, 5.0, 0.0], 2).expect("matrix");
        assert_eq!(optimize_order(&matrix, 4), vec![1]);
    }

    #[test]
    fn visiting_order_contains_every_node_once() {
        let matrix = synthetic_matrix();
        let order = optimize_order(&matrix, 8);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..matrix.node_count()).collect::<Vec<usize>>());
    }

    #[test]
    fn points_on_a_line_are_visited_in_order() {
        // Nodes 1..=4 on the x axis at 10, 20, 30, 40 metres from the car park.
        let positions: [f64; 5] = [0.0, 10.0, 20.0, 30.0, 40.0];
        let mut data = vec![0.0; positions.len() * positions.len()];
        for from in 0..positions.len() {
            for to in 0..positions.len() {
                data[from * positions.len() + to] = (positions[from] - positions[to]).abs();
            }
        }
        let matrix = DistanceMatrix::new(data, positions.len()).expect("matrix");
        let order = optimize_order(&matrix, 8);
        assert_eq!(
            order,
            vec![1, 2, 3, 4],
            "a line must be walked monotonically"
        );
    }

    #[test]
    fn nearest_neighbour_tie_breaks_on_lower_index() {
        // Two equidistant nodes; the lower index must come first.
        let matrix = DistanceMatrix::new(vec![0.0, 5.0, 5.0, 5.0, 0.0, 1.0, 5.0, 1.0, 0.0], 3)
            .expect("matrix");
        let order = optimize_order(&matrix, 0);
        assert_eq!(order, vec![1, 2]);
    }

    #[test]
    fn two_opt_never_worsens_the_path() {
        let matrix = synthetic_matrix();
        let nearest = optimize_order(&matrix, 0);
        let improved = optimize_order(&matrix, 8);

        let cost = |order: &[usize]| -> f64 {
            let mut total = matrix.get(0, order[0]);
            for pair in order.windows(2) {
                total += matrix.get(pair[0], pair[1]);
            }
            total
        };

        assert!(
            cost(&improved) <= cost(&nearest) + 1e-9,
            "2-opt made the path longer: {} vs {}",
            cost(&improved),
            cost(&nearest)
        );
    }

    #[test]
    fn improvement_is_deterministic() {
        let matrix = synthetic_matrix();
        assert_eq!(optimize_order(&matrix, 8), optimize_order(&matrix, 8));
    }

    #[test]
    fn result_is_stable_for_more_passes() {
        let matrix = synthetic_matrix();
        let eight = optimize_order(&matrix, 8);
        let many = optimize_order(&matrix, 64);
        assert_eq!(eight, many, "the search must converge");
    }
}
