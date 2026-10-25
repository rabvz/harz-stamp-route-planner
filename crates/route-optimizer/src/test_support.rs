// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic fixtures shared by the optimizer's unit tests.
//!
//! Everything here is synthetic and clearly labelled as such: no test depends
//! on the real catalogue, so the optimizer tests keep passing when the station
//! dataset is updated.

use geo_engine::LatLon;
use stamp_core::{Stamp, StampId};

use crate::DistanceMatrix;

/// A 4-node matrix built from a small irregular layout around the Harz.
///
/// Node 0 is the car park; nodes 1..=3 are stamps. The distances are deliberately
/// not symmetric and not a line, so both the construction and the improvement
/// step have something to work on.
pub fn synthetic_matrix() -> DistanceMatrix {
    let positions: [f64; 4] = [0.0, 12.0, 7.0, 19.0];
    let mut data = vec![0.0; 16];
    for from in 0..4 {
        for to in 0..4 {
            // Slight asymmetry models one-way trail sections.
            let penalty = if from > to { 0.5 } else { 0.0 };
            data[from * 4 + to] = (positions[from] - positions[to]).abs() + penalty;
        }
    }
    DistanceMatrix::new(data, 4).expect("synthetic matrix is valid")
}

/// A matrix for nodes placed on a line at the given coordinates.
pub fn line_matrix(coordinates: &[f64]) -> DistanceMatrix {
    let count = coordinates.len();
    let mut data = vec![0.0; count * count];
    for from in 0..count {
        for to in 0..count {
            data[from * count + to] = (coordinates[from] - coordinates[to]).abs();
        }
    }
    DistanceMatrix::new(data, count).expect("line matrix is valid")
}

/// Synthetic stamps with plausible Harz coordinates, numbered from 1.
///
/// Used where the tour builder needs station identities but not real data.
pub fn synthetic_stamps(count: usize) -> Vec<Stamp> {
    (1..=count)
        .map(|index| {
            let number = u16::try_from(index).expect("small count");
            Stamp {
                id: StampId::new(number).expect("valid id"),
                code: StampId::new(number).expect("valid id").code(),
                name: format!("Test station {number}"),
                location: "Synthetic fixture".to_string(),
                position: LatLon::new(51.8 + f64::from(number) * 0.001, 10.6)
                    .expect("valid position"),
                elevation_m: None,
            }
        })
        .collect()
}
