// SPDX-License-Identifier: MIT OR Apache-2.0

//! The distance matrix the optimizers work on.

use geo_engine::LatLon;
use routing_client::{RouteProvider, StraightLineProvider};
use stamp_core::Stamp;

use crate::OptimizerError;

/// A row-major matrix of distances between `node_count` nodes.
///
/// Node `0` is always the car park; node `i` corresponds to
/// `stamps[i - 1]`. Distances need not be symmetric, because a routed leg in
/// one direction can legitimately differ from the other, but the diagonal must
/// be zero.
#[derive(Debug, Clone, PartialEq)]
pub struct DistanceMatrix {
    node_count: usize,
    data: Vec<f64>,
}

impl DistanceMatrix {
    /// Wraps a flat row-major matrix, validating its shape and entries.
    pub fn new(data: Vec<f64>, node_count: usize) -> Result<Self, OptimizerError> {
        let expected = node_count.saturating_mul(node_count);
        if data.len() != expected {
            return Err(OptimizerError::MatrixShape {
                expected,
                found: data.len(),
            });
        }
        for from in 0..node_count {
            for to in 0..node_count {
                let value = data[from * node_count + to];
                if !value.is_finite() || value < 0.0 {
                    return Err(OptimizerError::InvalidDistance { from, to, value });
                }
            }
        }
        Ok(Self { node_count, data })
    }

    /// Builds a matrix from straight-line estimates.
    ///
    /// The distances carry the provider's detour factor, so a plan built this
    /// way is an estimate and must be labelled as one by the caller.
    pub fn straight_line(start: LatLon, stamps: &[Stamp]) -> Result<Self, OptimizerError> {
        let provider = StraightLineProvider::default();
        let node_count = stamps.len() + 1;

        let mut positions = Vec::with_capacity(node_count);
        positions.push(start);
        positions.extend(stamps.iter().map(|stamp| stamp.position));

        let mut data = vec![0.0; node_count * node_count];
        for from in 0..node_count {
            for to in 0..node_count {
                if from == to {
                    continue;
                }
                let leg = provider
                    .route(positions[from], positions[to])
                    .map_err(|_| OptimizerError::InvalidDistance {
                        from,
                        to,
                        value: f64::NAN,
                    })?;
                data[from * node_count + to] = leg.distance_m;
            }
        }
        Self::new(data, node_count)
    }

    /// Number of nodes, including the car park.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Distance between two nodes in metres.
    ///
    /// # Panics
    ///
    /// Panics if an index is out of range. Indices are produced by this crate's
    /// own optimizers and never come from user input.
    #[must_use]
    pub fn get(&self, from: usize, to: usize) -> f64 {
        self.data[from * self.node_count + to]
    }

    /// The raw row-major data, for crossing the WASM boundary.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stamp_core::{StampCatalog, StampId};

    fn stamp(number: u16) -> Stamp {
        StampCatalog::embedded()
            .get(StampId::new(number).expect("id"))
            .expect("station")
            .clone()
    }

    #[test]
    fn wrong_entry_count_is_rejected() {
        assert!(matches!(
            DistanceMatrix::new(vec![0.0; 5], 3),
            Err(OptimizerError::MatrixShape {
                expected: 9,
                found: 5
            })
        ));
    }

    #[test]
    fn negative_entries_are_rejected() {
        let mut data = vec![0.0; 4];
        data[3] = -1.0;
        assert!(matches!(
            DistanceMatrix::new(data, 2),
            Err(OptimizerError::InvalidDistance { from: 1, to: 1, .. })
        ));
    }

    #[test]
    fn nan_entries_are_rejected() {
        let mut data = vec![0.0; 4];
        data[1] = f64::NAN;
        assert!(matches!(
            DistanceMatrix::new(data, 2),
            Err(OptimizerError::InvalidDistance { .. })
        ));
    }

    #[test]
    fn get_reads_row_major() {
        let matrix = DistanceMatrix::new(vec![0.0, 1.0, 2.0, 0.0], 2).expect("matrix");
        assert!((matrix.get(0, 1) - 1.0).abs() < 1e-9);
        assert!((matrix.get(1, 0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn straight_line_matrix_is_symmetric_and_zero_diagonal() {
        let stamps = vec![stamp(1), stamp(2), stamp(3)];
        let start = stamps[0].position;
        let matrix = DistanceMatrix::straight_line(start, &stamps).expect("matrix");
        assert_eq!(matrix.node_count(), 4);
        for node in 0..4 {
            assert_eq!(matrix.get(node, node), 0.0);
        }
        for from in 0..4 {
            for to in 0..4 {
                let difference = (matrix.get(from, to) - matrix.get(to, from)).abs();
                assert!(difference < 1e-6, "asymmetry at {from},{to}");
            }
        }
    }

    #[test]
    fn straight_line_matrix_charges_the_detour_factor() {
        let stamps = vec![stamp(1)];
        let start = stamp(2).position;
        let matrix = DistanceMatrix::straight_line(start, &stamps).expect("matrix");
        let direct = start.distance_m(stamps[0].position);
        assert!((matrix.get(0, 1) - direct * 1.25).abs() < 1e-6);
    }
}
