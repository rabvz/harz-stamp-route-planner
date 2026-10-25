// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ranks trailhead parking candidates for a chosen set of stamp stations.
//!
//! # There is no bundled parking dataset
//!
//! Parking data is not part of the operator's stamp file, and shipping a
//! hand-copied list of car parks would mean redistributing data whose licence
//! and accuracy this project cannot vouch for. Instead this crate ranks
//! candidates that the caller supplies:
//!
//! * the web app can load them from OpenStreetMap via the Overpass API at the
//!   user's request (documented in `docs/data-sources.md`), or
//! * the user can import their own list.
//!
//! Ranking is deterministic and explainable: for every candidate the crate
//! reports which stations it serves within the walking limit and why it scored
//! as it did.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};
use stamp_core::{Stamp, StampId};
use thiserror::Error;

/// What kind of place a candidate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParkingKind {
    /// A dedicated hiker car park.
    Trailhead,
    /// A public car park that hikers can use.
    Public,
    /// A lay-by or roadside parking spot.
    Roadside,
    /// Kind not known from the source data.
    Unknown,
}

impl ParkingKind {
    fn bonus(self) -> f64 {
        match self {
            Self::Trailhead => 300.0,
            Self::Public => 150.0,
            Self::Roadside => 50.0,
            Self::Unknown => 0.0,
        }
    }

    /// Stable lowercase identifier for the JSON boundary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trailhead => "trailhead",
            Self::Public => "public",
            Self::Roadside => "roadside",
            Self::Unknown => "unknown",
        }
    }
}

/// A place where a hike could start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParkingCandidate {
    /// Stable identifier from the source (for example an OSM element id).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Position of the parking area.
    pub position: LatLon,
    /// What kind of parking this is.
    pub kind: ParkingKind,
    /// Number of spaces, when known.
    pub capacity: Option<u16>,
    /// Where this candidate came from, for attribution in the UI.
    pub source: String,
}

/// Tuning for the ranking.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParkingSearchOptions {
    /// Stations further than this from a candidate are not counted as served.
    pub max_walk_m: f64,
    /// How many suggestions to return.
    pub max_candidates: usize,
    /// A candidate must serve at least this many stations to be suggested.
    pub min_stamps: usize,
}

impl Default for ParkingSearchOptions {
    fn default() -> Self {
        Self {
            max_walk_m: 1_500.0,
            max_candidates: 10,
            min_stamps: 1,
        }
    }
}

impl ParkingSearchOptions {
    /// Validates the options.
    pub fn validate(&self) -> Result<(), ParkingError> {
        if !self.max_walk_m.is_finite() || self.max_walk_m <= 0.0 {
            return Err(ParkingError::InvalidOptions {
                message: format!(
                    "max_walk_m must be positive and finite, got {}",
                    self.max_walk_m
                ),
            });
        }
        if self.max_candidates == 0 {
            return Err(ParkingError::InvalidOptions {
                message: "max_candidates must be at least 1".to_string(),
            });
        }
        if self.min_stamps == 0 {
            return Err(ParkingError::InvalidOptions {
                message: "min_stamps must be at least 1".to_string(),
            });
        }
        Ok(())
    }
}

/// One ranked candidate plus the reasoning behind its score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParkingSuggestion {
    /// The candidate itself.
    pub candidate: ParkingCandidate,
    /// Stations reachable within the walking limit, nearest first.
    pub served_stamps: Vec<StampId>,
    /// Shortest walking distance to a served station, in metres.
    pub nearest_walk_m: f64,
    /// Mean walking distance over the served stations, in metres.
    pub mean_walk_m: f64,
    /// Score used for ordering; higher is better. Not a distance.
    pub score: f64,
}

impl ParkingSuggestion {
    /// Share of the requested stations that this candidate serves, in 0..=1.
    #[must_use]
    pub fn coverage(&self, selected_total: usize) -> f64 {
        if selected_total == 0 {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            {
                self.served_stamps.len() as f64 / selected_total as f64
            }
        }
    }
}

/// Reasons ranking could not run.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ParkingError {
    /// The options were unusable.
    #[error("invalid parking options: {message}")]
    InvalidOptions {
        /// What is wrong with the options.
        message: String,
    },

    /// No stations were supplied.
    #[error("no stamps selected for parking search")]
    NoStamps,

    /// Two candidates shared an id, which would make results ambiguous.
    #[error("duplicate parking candidate id {id:?}")]
    DuplicateCandidate {
        /// The repeated identifier.
        id: String,
    },
}

/// Ranks candidates against the chosen stations.
///
/// Scoring favours candidates that serve many stations, by a short walk, and
/// that are a real trailhead rather than a roadside spot. Ties break on the
/// candidate id so that the same input always yields the same order.
pub fn rank_parking(
    candidates: &[ParkingCandidate],
    stamps: &[&Stamp],
    options: ParkingSearchOptions,
) -> Result<Vec<ParkingSuggestion>, ParkingError> {
    options.validate()?;
    if stamps.is_empty() {
        return Err(ParkingError::NoStamps);
    }
    let mut seen = std::collections::BTreeSet::new();
    for candidate in candidates {
        if !seen.insert(candidate.id.as_str()) {
            return Err(ParkingError::DuplicateCandidate {
                id: candidate.id.clone(),
            });
        }
    }

    let mut suggestions = Vec::new();
    for candidate in candidates {
        let mut served: Vec<(StampId, f64)> = stamps
            .iter()
            .filter_map(|stamp| {
                let distance = candidate.position.distance_m(stamp.position);
                (distance <= options.max_walk_m).then_some((stamp.id, distance))
            })
            .collect();

        if served.len() < options.min_stamps {
            continue;
        }

        served.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));

        let nearest = served.first().map_or(0.0, |(_, distance)| *distance);
        #[allow(clippy::cast_precision_loss)]
        let mean = served.iter().map(|(_, distance)| distance).sum::<f64>() / served.len() as f64;

        // Each served station is worth a fixed amount; a shorter average walk
        // is worth up to 500 points, so coverage dominates but proximity breaks
        // ties sensibly.
        #[allow(clippy::cast_precision_loss)]
        let coverage_score = served.len() as f64 * 1_000.0;
        let proximity_score = 500.0 * (1.0 - (mean / options.max_walk_m).clamp(0.0, 1.0));
        let score = coverage_score + proximity_score + candidate.kind.bonus();

        suggestions.push(ParkingSuggestion {
            candidate: candidate.clone(),
            served_stamps: served.iter().map(|(id, _)| *id).collect(),
            nearest_walk_m: nearest,
            mean_walk_m: mean,
            score,
        });
    }

    suggestions.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.nearest_walk_m.total_cmp(&b.nearest_walk_m))
            .then(a.candidate.id.cmp(&b.candidate.id))
    });
    suggestions.truncate(options.max_candidates);
    Ok(suggestions)
}

/// Greedily picks parking candidates until every station is covered.
///
/// Used for multi-day planning, where one car park per day is usually not
/// enough. Returns the chosen suggestions in the order they were picked; the
/// caller can see from `served_stamps` how the coverage was built up. Stations
/// that no candidate can reach are reported in [`ParkingCover::uncovered`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParkingCover {
    /// Chosen candidates, in greedy order.
    pub chosen: Vec<ParkingSuggestion>,
    /// Stations no candidate could serve within the walking limit.
    pub uncovered: Vec<StampId>,
}

/// Builds a greedy cover of `stamps` using `candidates`.
pub fn cover_stamps(
    candidates: &[ParkingCandidate],
    stamps: &[&Stamp],
    options: ParkingSearchOptions,
) -> Result<ParkingCover, ParkingError> {
    // Ranking with a large candidate budget so the greedy loop sees everything.
    let mut pool_options = options;
    pool_options.max_candidates = usize::MAX;
    let ranked = rank_parking(candidates, stamps, pool_options)?;

    let wanted: std::collections::BTreeSet<StampId> = stamps.iter().map(|stamp| stamp.id).collect();
    let mut covered: std::collections::BTreeSet<StampId> = std::collections::BTreeSet::new();
    let mut chosen: Vec<ParkingSuggestion> = Vec::new();

    while covered.len() < wanted.len() {
        let next = ranked
            .iter()
            .filter(|suggestion| {
                !chosen
                    .iter()
                    .any(|c| c.candidate.id == suggestion.candidate.id)
            })
            .map(|suggestion| {
                let gain = suggestion
                    .served_stamps
                    .iter()
                    .filter(|id| !covered.contains(*id))
                    .count();
                (gain, suggestion)
            })
            .filter(|(gain, _)| *gain > 0)
            .max_by(|(left_gain, left), (right_gain, right)| {
                left_gain
                    .cmp(right_gain)
                    .then(right.score.total_cmp(&left.score))
                    .then(right.candidate.id.cmp(&left.candidate.id))
            });

        match next {
            Some((_, suggestion)) => {
                covered.extend(suggestion.served_stamps.iter().copied());
                chosen.push(suggestion.clone());
                if chosen.len() > candidates.len() {
                    break; // defensive: cannot happen with the gain filter
                }
            }
            None => break,
        }
    }

    let uncovered = wanted.difference(&covered).copied().collect();
    Ok(ParkingCover { chosen, uncovered })
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

    fn candidate(id: &str, kind: ParkingKind, lat: f64, lon: f64) -> ParkingCandidate {
        ParkingCandidate {
            id: id.to_string(),
            name: format!("Test parking {id}"),
            position: LatLon::new(lat, lon).expect("position"),
            kind,
            capacity: None,
            source: "unit-test".to_string(),
        }
    }

    fn at_stamp(id: &str, number: u16) -> ParkingCandidate {
        let station = stamp(number);
        candidate(
            id,
            ParkingKind::Trailhead,
            station.position.latitude(),
            station.position.longitude(),
        )
    }

    #[test]
    fn options_are_validated() {
        assert!(ParkingSearchOptions::default().validate().is_ok());
        let bad = ParkingSearchOptions {
            max_walk_m: 0.0,
            ..Default::default()
        };
        assert!(matches!(
            rank_parking(&[], &[], bad),
            Err(ParkingError::InvalidOptions { .. })
        ));
    }

    #[test]
    fn no_stamps_is_an_error() {
        assert!(matches!(
            rank_parking(&[], &[], ParkingSearchOptions::default()),
            Err(ParkingError::NoStamps)
        ));
    }

    #[test]
    fn duplicate_candidate_ids_are_rejected() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("a", 1), at_stamp("a", 1)];
        assert!(matches!(
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()),
            Err(ParkingError::DuplicateCandidate { .. })
        ));
    }

    #[test]
    fn a_candidate_right_at_a_station_serves_it() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("trailhead-hwn001", 1)];
        let ranked =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].served_stamps, vec![StampId::new(1).expect("id")]);
        assert!(ranked[0].nearest_walk_m < 1.0);
    }

    #[test]
    fn distant_candidates_are_excluded() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        // HWN222 is far from HWN001.
        let candidates = [at_stamp("far-away", 222)];
        let ranked =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        assert!(ranked.is_empty(), "a 50 km walk must not count as served");
    }

    #[test]
    fn candidates_serving_more_stations_rank_higher() {
        let stations = [stamp(1), stamp(2), stamp(3)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [
            at_stamp("serves-one", 1),
            // Between HWN001 and HWN002, about 1 km from each.
            candidate("serves-two", ParkingKind::Trailhead, 51.835_9, 10.591_4),
        ];
        let ranked =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].candidate.id, "serves-two");
        assert!(ranked[0].served_stamps.len() >= 2);
    }

    #[test]
    fn trailheads_beats_roadside_on_a_tie() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let mut roadside = at_stamp("z-roadside", 1);
        roadside.kind = ParkingKind::Roadside;
        let mut trailhead = at_stamp("a-trailhead", 1);
        trailhead.kind = ParkingKind::Trailhead;
        let ranked = rank_parking(
            &[roadside, trailhead],
            &refs,
            ParkingSearchOptions::default(),
        )
        .expect("ranked");
        assert_eq!(ranked[0].candidate.id, "a-trailhead");
    }

    #[test]
    fn ranking_is_deterministic() {
        let stations = [stamp(1), stamp(2)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("a", 1), at_stamp("b", 2)];
        let first =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        let second =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        assert_eq!(first, second);
    }

    #[test]
    fn result_count_is_capped() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates: Vec<ParkingCandidate> = (0..10)
            .map(|index| at_stamp(&format!("p{index}"), 1))
            .collect();
        let options = ParkingSearchOptions {
            max_candidates: 3,
            ..Default::default()
        };
        let ranked = rank_parking(&candidates, &refs, options).expect("ranked");
        assert_eq!(ranked.len(), 3);
    }

    #[test]
    fn min_stamps_filters_candidates() {
        let stations = [stamp(1), stamp(222)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("only-one", 1)];
        let options = ParkingSearchOptions {
            min_stamps: 2,
            ..Default::default()
        };
        let ranked = rank_parking(&candidates, &refs, options).expect("ranked");
        assert!(ranked.is_empty());
    }

    #[test]
    fn coverage_is_a_ratio() {
        let stations = [stamp(1)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("a", 1)];
        let ranked =
            rank_parking(&candidates, &refs, ParkingSearchOptions::default()).expect("ranked");
        assert!((ranked[0].coverage(1) - 1.0).abs() < 1e-9);
        assert!((ranked[0].coverage(2) - 0.5).abs() < 1e-9);
        assert!((ranked[0].coverage(0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn cover_reports_unreachable_stations() {
        let stations = [stamp(1), stamp(222)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("near-hwn001", 1)];
        let cover =
            cover_stamps(&candidates, &refs, ParkingSearchOptions::default()).expect("cover");
        assert_eq!(cover.chosen.len(), 1);
        assert_eq!(cover.uncovered, vec![StampId::new(222).expect("id")]);
    }

    #[test]
    fn cover_uses_several_candidates_when_needed() {
        let stations = [stamp(1), stamp(222)];
        let refs: Vec<&Stamp> = stations.iter().collect();
        let candidates = [at_stamp("near-hwn001", 1), at_stamp("near-hwn222", 222)];
        let cover =
            cover_stamps(&candidates, &refs, ParkingSearchOptions::default()).expect("cover");
        assert_eq!(cover.chosen.len(), 2);
        assert!(cover.uncovered.is_empty());
    }
}
