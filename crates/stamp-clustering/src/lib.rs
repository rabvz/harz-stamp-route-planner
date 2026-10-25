// SPDX-License-Identifier: MIT OR Apache-2.0

//! Groups selected stamp stations into spatially coherent day-tour clusters.
//!
//! # Algorithm
//!
//! A deterministic greedy expansion followed by an optional merge pass:
//!
//! 1. Stations are sorted by latitude, longitude and number, so the result does
//!    not depend on the order the user ticked them.
//! 2. The first unassigned station seeds a cluster. The nearest still-unassigned
//!    station is added as long as the cluster keeps its stamp budget and stays
//!    inside the diameter limit. Distances are measured on a local planar
//!    projection centred on the seed.
//! 3. Adjacent clusters are merged when the merged cluster still satisfies both
//!    limits, which avoids a long tail of two-station clusters.
//!
//! Every constraint is checked before a station joins, so the guarantees hold by
//! construction rather than by post-hoc repair. The algorithm is deliberately
//! simple and explainable: a user who sees a strange grouping can reason about
//! why it happened, which matters more here than a marginally better partition.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use geo_engine::{LatLon, LocalPlane};
use serde::{Deserialize, Serialize};
use stamp_core::{Stamp, StampId};
use thiserror::Error;

/// Limits applied while grouping stations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClusterOptions {
    /// Maximum number of stations in one cluster.
    pub max_stamps: usize,
    /// Maximum distance between any two stations of one cluster, in metres.
    pub max_diameter_m: f64,
}

impl Default for ClusterOptions {
    fn default() -> Self {
        Self {
            max_stamps: 8,
            max_diameter_m: 8_000.0,
        }
    }
}

impl ClusterOptions {
    /// Validates the options.
    pub fn validate(&self) -> Result<(), ClusterError> {
        if self.max_stamps == 0 {
            return Err(ClusterError::InvalidOptions {
                message: "max_stamps must be at least 1".to_string(),
            });
        }
        if !self.max_diameter_m.is_finite() || self.max_diameter_m <= 0.0 {
            return Err(ClusterError::InvalidOptions {
                message: format!(
                    "max_diameter_m must be positive and finite, got {}",
                    self.max_diameter_m
                ),
            });
        }
        Ok(())
    }
}

/// One group of stations that belong together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StampCluster {
    /// Zero-based identifier, assigned after sorting by centre.
    pub id: usize,
    /// Member stations in ascending station-number order.
    pub stamps: Vec<StampId>,
    /// Mean position of the members.
    pub centroid: LatLon,
    /// Largest distance between any two members, in metres.
    pub diameter_m: f64,
}

impl StampCluster {
    /// Number of stations in the cluster.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stamps.len()
    }

    /// Whether the cluster is empty. A constructed cluster never is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stamps.is_empty()
    }
}

/// Reasons clustering could not run.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ClusterError {
    /// The options were unusable.
    #[error("invalid cluster options: {message}")]
    InvalidOptions {
        /// What is wrong with the options.
        message: String,
    },

    /// No stations were supplied.
    #[error("no stamps selected for clustering")]
    NoStamps,

    /// Two stations shared a number, which would break the partition guarantee.
    #[error("duplicate stamp {number} in the input")]
    DuplicateStamp {
        /// The repeated station number.
        number: u16,
    },
}

/// Groups `stamps` into clusters that satisfy `options`.
///
/// The input is treated as a set: the result contains every supplied station
/// exactly once, regardless of input order.
pub fn cluster_stamps(
    stamps: &[&Stamp],
    options: ClusterOptions,
) -> Result<Vec<StampCluster>, ClusterError> {
    options.validate()?;
    if stamps.is_empty() {
        return Err(ClusterError::NoStamps);
    }

    // Deterministic input order, independent of the caller's ordering.
    let mut ordered: Vec<&Stamp> = stamps.to_vec();
    ordered.sort_by(|a, b| {
        a.position
            .latitude()
            .total_cmp(&b.position.latitude())
            .then(a.position.longitude().total_cmp(&b.position.longitude()))
            .then(a.id.cmp(&b.id))
    });
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(ClusterError::DuplicateStamp {
                number: pair[0].id.number(),
            });
        }
    }

    let mut groups: Vec<Vec<&Stamp>> = Vec::new();
    let mut assigned = vec![false; ordered.len()];

    for seed_index in 0..ordered.len() {
        if assigned[seed_index] {
            continue;
        }
        assigned[seed_index] = true;
        let mut members = vec![ordered[seed_index]];
        let plane = LocalPlane::new(ordered[seed_index].position);

        loop {
            if members.len() >= options.max_stamps {
                break;
            }
            // Nearest unassigned station that keeps the cluster inside the
            // diameter limit. Ties break on station number for determinism.
            let mut best: Option<(usize, f64)> = None;
            for (index, candidate) in ordered.iter().enumerate() {
                if assigned[index] {
                    continue;
                }
                let diameter = diameter_with(&plane, &members, candidate.position);
                if diameter > options.max_diameter_m {
                    continue;
                }
                let distance = plane.distance_m(ordered[seed_index].position, candidate.position);
                let better = match best {
                    None => true,
                    Some((best_index, best_distance)) => {
                        distance < best_distance
                            || (distance == best_distance
                                && ordered[index].id < ordered[best_index].id)
                    }
                };
                if better {
                    best = Some((index, distance));
                }
            }
            match best {
                Some((index, _)) => {
                    assigned[index] = true;
                    members.push(ordered[index]);
                }
                None => break,
            }
        }
        groups.push(members);
    }

    // Merge pass: repeatedly join the closest pair that still fits the limits.
    let mut merges_possible = true;
    while merges_possible {
        merges_possible = false;
        let mut best: Option<(usize, usize, f64)> = None;
        for left in 0..groups.len() {
            for right in (left + 1)..groups.len() {
                if groups[left].len() + groups[right].len() > options.max_stamps {
                    continue;
                }
                let merged = diameter_of(groups[left].iter().chain(groups[right].iter()).copied());
                if merged > options.max_diameter_m {
                    continue;
                }
                let left_centre = centroid(groups[left].as_slice());
                let right_centre = centroid(groups[right].as_slice());
                let separation = left_centre.distance_m(right_centre);
                let better = match best {
                    None => true,
                    Some((_, _, best_separation)) => separation < best_separation,
                };
                if better {
                    best = Some((left, right, separation));
                }
            }
        }
        if let Some((left, right, _)) = best {
            let absorbed = groups.remove(right);
            groups[left].extend(absorbed);
            merges_possible = true;
        }
    }

    build_clusters(groups)
}

/// Builds the public cluster representation, ordered north to south.
fn build_clusters(mut groups: Vec<Vec<&Stamp>>) -> Result<Vec<StampCluster>, ClusterError> {
    for group in &mut groups {
        group.sort_by_key(|stamp| stamp.id);
    }

    let mut described: Vec<(LatLon, Vec<StampId>, f64)> = groups
        .iter()
        .map(|group| {
            let centre = centroid(group.as_slice());
            let diameter = diameter_of(group.iter().copied());
            (
                centre,
                group.iter().map(|stamp| stamp.id).collect(),
                diameter,
            )
        })
        .collect();

    // Sort by centre, north to south then west to east, so identifiers are
    // stable across runs and meaningful on the map.
    described.sort_by(|a, b| {
        b.0.latitude()
            .total_cmp(&a.0.latitude())
            .then(a.0.longitude().total_cmp(&b.0.longitude()))
    });

    let mut clusters = Vec::with_capacity(described.len());
    for (index, (centre, stamps, diameter)) in described.into_iter().enumerate() {
        clusters.push(StampCluster {
            id: index,
            stamps,
            centroid: centre,
            diameter_m: diameter,
        });
    }
    Ok(clusters)
}

/// Mean position of a non-empty group.
fn centroid(group: &[&Stamp]) -> LatLon {
    debug_assert!(!group.is_empty(), "clusters are never empty");
    let count = group.len() as f64;
    let lat = group
        .iter()
        .map(|stamp| stamp.position.latitude())
        .sum::<f64>()
        / count;
    let lon = group
        .iter()
        .map(|stamp| stamp.position.longitude())
        .sum::<f64>()
        / count;
    // The mean of valid positions is valid, but fall back to the first member
    // rather than panicking if rounding ever produces an invalid value.
    LatLon::new(lat, lon).unwrap_or(group[0].position)
}

/// Largest pairwise distance inside a group, in metres.
fn diameter_of<'a>(group: impl Iterator<Item = &'a Stamp>) -> f64 {
    let positions: Vec<LatLon> = group.map(|stamp| stamp.position).collect();
    if positions.len() < 2 {
        return 0.0;
    }
    // A local plane keeps the O(n^2) loop cheap; the clusters are small.
    let plane = LocalPlane::new(positions[0]);
    let mut largest: f64 = 0.0;
    for left in 0..positions.len() {
        for right in (left + 1)..positions.len() {
            let distance = plane.distance_m(positions[left], positions[right]);
            if distance > largest {
                largest = distance;
            }
        }
    }
    largest
}

/// Diameter of `members` plus one candidate position.
fn diameter_with(plane: &LocalPlane, members: &[&Stamp], candidate: LatLon) -> f64 {
    members
        .iter()
        .map(|stamp| plane.distance_m(stamp.position, candidate))
        .fold(0.0_f64, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stamp_core::StampCatalog;

    fn catalog() -> &'static StampCatalog {
        StampCatalog::embedded()
    }

    fn stamps(numbers: &[u16]) -> Vec<Stamp> {
        numbers
            .iter()
            .map(|number| {
                let id = StampId::new(*number).expect("id");
                catalog().get(id).expect("station").clone()
            })
            .collect()
    }

    fn as_refs(stamps: &[Stamp]) -> Vec<&Stamp> {
        stamps.iter().collect()
    }

    #[test]
    fn default_options_are_sane() {
        let options = ClusterOptions::default();
        assert!(options.validate().is_ok());
        assert!(options.max_stamps > 0);
    }

    #[test]
    fn zero_budget_is_rejected() {
        let options = ClusterOptions {
            max_stamps: 0,
            max_diameter_m: 1_000.0,
        };
        assert!(matches!(
            cluster_stamps(&[], options),
            Err(ClusterError::InvalidOptions { .. })
        ));
    }

    #[test]
    fn negative_diameter_is_rejected() {
        let options = ClusterOptions {
            max_stamps: 4,
            max_diameter_m: -1.0,
        };
        assert!(options.validate().is_err());
    }

    #[test]
    fn no_stamps_is_an_error() {
        assert!(matches!(
            cluster_stamps(&[], ClusterOptions::default()),
            Err(ClusterError::NoStamps)
        ));
    }

    #[test]
    fn duplicate_stamps_are_rejected() {
        let stamps = stamps(&[1, 1]);
        assert!(matches!(
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()),
            Err(ClusterError::DuplicateStamp { number: 1 })
        ));
    }

    #[test]
    fn every_stamp_appears_exactly_once() {
        let stamps = stamps(&[1, 2, 3, 9, 10, 20, 30, 45, 60, 77, 100, 150, 200, 222]);
        let clusters =
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()).expect("clusters");

        let mut collected: Vec<u16> = clusters
            .iter()
            .flat_map(|cluster| cluster.stamps.iter().map(|id| id.number()))
            .collect();
        collected.sort_unstable();

        let mut expected: Vec<u16> = stamps.iter().map(|stamp| stamp.id.number()).collect();
        expected.sort_unstable();
        assert_eq!(
            collected, expected,
            "partition must be complete and disjoint"
        );
    }

    #[test]
    fn limits_are_never_exceeded() {
        let stamps = stamps(&(1..=40).collect::<Vec<u16>>());
        let options = ClusterOptions {
            max_stamps: 5,
            max_diameter_m: 6_000.0,
        };
        let clusters = cluster_stamps(&as_refs(&stamps), options).expect("clusters");
        for cluster in &clusters {
            assert!(
                cluster.len() <= options.max_stamps,
                "cluster {} has {} members",
                cluster.id,
                cluster.len()
            );
            assert!(
                cluster.diameter_m <= options.max_diameter_m + 1e-6,
                "cluster {} diameter {} exceeds limit",
                cluster.id,
                cluster.diameter_m
            );
        }
    }

    #[test]
    fn clustering_is_deterministic_and_order_independent() {
        let stamps = stamps(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let forward =
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()).expect("clusters");

        let mut reversed_stamps = as_refs(&stamps);
        reversed_stamps.reverse();
        let reversed =
            cluster_stamps(&reversed_stamps, ClusterOptions::default()).expect("clusters");

        assert_eq!(forward, reversed);
    }

    #[test]
    fn nearby_stamps_share_a_cluster() {
        // HWN001 and HWN002 are about 2 km apart.
        let stamps = stamps(&[1, 2]);
        let options = ClusterOptions {
            max_stamps: 8,
            max_diameter_m: 5_000.0,
        };
        let clusters = cluster_stamps(&as_refs(&stamps), options).expect("clusters");
        assert_eq!(clusters.len(), 1, "close stations should be grouped");
        assert_eq!(clusters[0].len(), 2);
    }

    #[test]
    fn distant_stamps_are_separated() {
        // HWN001 (Torhaus) and HWN222 are at opposite ends of the Harz.
        let stamps = stamps(&[1, 222]);
        let options = ClusterOptions {
            max_stamps: 8,
            max_diameter_m: 5_000.0,
        };
        let clusters = cluster_stamps(&as_refs(&stamps), options).expect("clusters");
        assert_eq!(clusters.len(), 2, "far apart stations must not be merged");
    }

    #[test]
    fn centroid_lies_inside_the_members_bounding_box() {
        let stamps = stamps(&[1, 2, 3]);
        let clusters =
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()).expect("clusters");
        for cluster in &clusters {
            let lats: Vec<f64> = cluster
                .stamps
                .iter()
                .filter_map(|id| catalog().get(*id))
                .map(|stamp| stamp.position.latitude())
                .collect();
            let min = lats.iter().copied().fold(f64::INFINITY, f64::min);
            let max = lats.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(cluster.centroid.latitude() >= min - 1e-9);
            assert!(cluster.centroid.latitude() <= max + 1e-9);
        }
    }

    #[test]
    fn cluster_ids_are_contiguous_from_zero() {
        let stamps = stamps(&(1..=20).collect::<Vec<u16>>());
        let clusters =
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()).expect("clusters");
        let ids: Vec<usize> = clusters.iter().map(|cluster| cluster.id).collect();
        assert_eq!(ids, (0..clusters.len()).collect::<Vec<usize>>());
    }

    #[test]
    fn member_lists_are_sorted() {
        let stamps = stamps(&[9, 3, 1, 7, 5]);
        let clusters =
            cluster_stamps(&as_refs(&stamps), ClusterOptions::default()).expect("clusters");
        for cluster in &clusters {
            let mut sorted = cluster.stamps.clone();
            sorted.sort();
            assert_eq!(cluster.stamps, sorted);
        }
    }
}
