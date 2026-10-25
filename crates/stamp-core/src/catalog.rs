// SPDX-License-Identifier: MIT OR Apache-2.0

//! The catalogue of stamp stations and its provenance metadata.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use geo_engine::{BoundingBox, LatLon};
use serde::{Deserialize, Serialize};

use crate::{EMBEDDED_CATALOG_JSON, Stamp, StampError, StampId};

/// Schema version this build understands.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Provenance of a catalogue, mirrored into exports and the about dialog so
/// that users can always tell which dataset produced a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogDataset {
    /// Stable dataset identifier, for example `hwn-2025-04-23`.
    pub id: String,
    /// Human readable dataset name.
    pub name: String,
    /// Where the source file was downloaded from.
    pub source_url: String,
    /// Repository-relative path of the committed source file.
    pub source_file: String,
    /// Lowercase hex SHA-256 of the source file, used for drift detection.
    pub source_sha256: String,
    /// Publication date stated by the operator (`YYYY-MM-DD`).
    pub released: String,
    /// Date the file was retrieved (`YYYY-MM-DD`).
    pub retrieved: String,
    /// Pointer to the licensing notes for the dataset.
    pub rights_note: String,
}

impl CatalogDataset {
    fn validate(&self) -> Result<(), StampError> {
        for (field, value) in [
            ("dataset id", &self.id),
            ("dataset name", &self.name),
            ("source url", &self.source_url),
            ("source file", &self.source_file),
            ("source sha256", &self.source_sha256),
            ("rights note", &self.rights_note),
        ] {
            if value.trim().is_empty() {
                return Err(StampError::InvalidJson {
                    message: format!("catalogue {field} must not be empty"),
                });
            }
        }
        Ok(())
    }
}

/// A validated, immutable set of stamp stations.
#[derive(Debug, Clone)]
pub struct StampCatalog {
    dataset: CatalogDataset,
    stamps: Vec<Stamp>,
    index: BTreeMap<StampId, usize>,
}

/// Wire format of a single stamp, with the position flattened into `lat`/`lon`
/// to match the GPX source and keep the JSON readable.
#[derive(Debug, Deserialize)]
struct RawStamp {
    id: u16,
    code: String,
    name: String,
    location: String,
    lat: f64,
    lon: f64,
    #[serde(default)]
    elevation_m: Option<f64>,
}

impl RawStamp {
    fn into_stamp(self) -> Result<Stamp, StampError> {
        let id = StampId::new(self.id)?;
        let position =
            LatLon::new(self.lat, self.lon).map_err(|source| StampError::InvalidPosition {
                number: self.id,
                source,
            })?;
        Ok(Stamp {
            id,
            code: self.code,
            name: self.name,
            location: self.location,
            position,
            elevation_m: self.elevation_m,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    schema_version: u32,
    dataset: CatalogDataset,
    stamps: Vec<RawStamp>,
}

impl StampCatalog {
    /// Parses and validates a catalogue document.
    ///
    /// Every rejected input produces a specific error rather than a partially
    /// loaded catalogue: a plan built from an inconsistent station list would
    /// be worse than no plan at all.
    pub fn from_json(json: &str) -> Result<Self, StampError> {
        let file: CatalogFile =
            serde_json::from_str(json).map_err(|error| StampError::InvalidJson {
                message: error.to_string(),
            })?;

        if file.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(StampError::UnsupportedSchema {
                found: file.schema_version,
                expected: SUPPORTED_SCHEMA_VERSION,
            });
        }
        file.dataset.validate()?;
        if file.stamps.is_empty() {
            return Err(StampError::EmptyCatalog);
        }

        let mut stamps = Vec::with_capacity(file.stamps.len());
        let mut index = BTreeMap::new();
        for raw in file.stamps {
            let stamp = raw.into_stamp()?;
            stamp.validate()?;
            if index.insert(stamp.id, stamps.len()).is_some() {
                return Err(StampError::DuplicateNumber {
                    number: stamp.id.number(),
                });
            }
            stamps.push(stamp);
        }

        Ok(Self {
            dataset: file.dataset,
            stamps,
            index,
        })
    }

    /// The catalogue bundled with the application.
    ///
    /// Panics only if the committed catalogue is invalid, which the test suite
    /// rules out before a release can be cut.
    #[must_use]
    pub fn embedded() -> &'static Self {
        static CATALOG: OnceLock<StampCatalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Self::from_json(EMBEDDED_CATALOG_JSON)
                .expect("the committed catalogue must be valid; run `cargo xtask test`")
        })
    }

    /// Provenance of this catalogue.
    #[must_use]
    pub fn dataset(&self) -> &CatalogDataset {
        &self.dataset
    }

    /// Stations in ascending station-number order.
    #[must_use]
    pub fn stamps(&self) -> &[Stamp] {
        &self.stamps
    }

    /// Number of stations in the catalogue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stamps.len()
    }

    /// Whether the catalogue contains no stations. Always false for a
    /// successfully constructed catalogue; kept for API completeness.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stamps.is_empty()
    }

    /// Looks up a station by number.
    #[must_use]
    pub fn get(&self, id: StampId) -> Option<&Stamp> {
        self.index.get(&id).map(|index| &self.stamps[*index])
    }

    /// Bounding box of every station, for fitting the map viewport.
    pub fn bounding_box(&self) -> Result<BoundingBox, StampError> {
        let points: Vec<LatLon> = self.stamps.iter().map(|stamp| stamp.position).collect();
        BoundingBox::from_points(&points)
            .map_err(|source| StampError::InvalidPosition { number: 0, source })
    }

    /// Rejects a station number that this catalogue does not contain.
    pub fn require(&self, id: StampId) -> Result<&Stamp, StampError> {
        self.get(id).ok_or_else(|| StampError::UnknownStamp {
            number: id.number(),
            dataset: self.dataset.id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(number: u16) -> StampId {
        StampId::new(number).expect("valid id")
    }

    #[test]
    fn embedded_catalogue_has_all_stations() {
        let catalog = StampCatalog::embedded();
        assert_eq!(catalog.len(), 222, "expected the full 2025 station list");
        assert!(!catalog.is_empty());
    }

    #[test]
    fn embedded_catalogue_numbers_are_contiguous() {
        let catalog = StampCatalog::embedded();
        let numbers: Vec<u16> = catalog.stamps().iter().map(|s| s.id.number()).collect();
        assert_eq!(numbers, (1..=222).collect::<Vec<u16>>());
    }

    #[test]
    fn embedded_catalogue_is_sorted_ascending() {
        let catalog = StampCatalog::embedded();
        assert!(catalog.stamps().windows(2).all(|w| w[0].id < w[1].id));
    }

    #[test]
    fn embedded_catalogue_matches_the_published_dataset() {
        let catalog = StampCatalog::embedded();
        let dataset = catalog.dataset();
        assert_eq!(dataset.id, "hwn-2025-04-23");
        assert_eq!(dataset.released, "2025-04-23");
        assert_eq!(dataset.source_sha256.len(), 64);
        assert!(dataset.source_url.contains("harzer-wandernadel.de"));
    }

    #[test]
    fn first_and_last_stations_are_correct() {
        let catalog = StampCatalog::embedded();
        let first = catalog.get(id(1)).expect("HWN001");
        assert_eq!(first.code, "HWN001");
        assert_eq!(first.name, "Eckertalsperre");
        assert!((first.position.latitude() - 51.841_649).abs() < 1e-9);
        assert!((first.position.longitude() - 10.579_975_7).abs() < 1e-9);
        assert_eq!(first.elevation_m, Some(555.0));

        let last = catalog.get(id(222)).expect("HWN222");
        assert_eq!(last.code, "HWN222");
    }

    #[test]
    fn every_station_has_a_name_and_location() {
        for stamp in StampCatalog::embedded().stamps() {
            assert!(!stamp.name.trim().is_empty(), "{} has no name", stamp.code);
            assert!(
                !stamp.location.trim().is_empty(),
                "{} has no location",
                stamp.code
            );
            assert!(
                stamp.position.latitude() > 51.0 && stamp.position.latitude() < 52.5,
                "{} is outside the Harz",
                stamp.code
            );
            assert!(
                stamp.position.longitude() > 9.5 && stamp.position.longitude() < 12.0,
                "{} is outside the Harz",
                stamp.code
            );
        }
    }

    #[test]
    fn bounding_box_covers_the_published_extent() {
        let bbox = StampCatalog::embedded()
            .bounding_box()
            .expect("bounding box");
        assert!((bbox.south_west.latitude() - 51.488_97).abs() < 1e-6);
        assert!((bbox.south_west.longitude() - 10.225_03).abs() < 1e-6);
        assert!((bbox.north_east.latitude() - 51.914_84).abs() < 1e-6);
        assert!((bbox.north_east.longitude() - 11.347_8).abs() < 1e-6);
    }

    #[test]
    fn lookup_of_unknown_station_returns_none() {
        let catalog = StampCatalog::embedded();
        let unknown = id(900);
        assert!(catalog.get(unknown).is_none());
        assert!(matches!(
            catalog.require(unknown),
            Err(StampError::UnknownStamp { number: 900, .. })
        ));
    }

    #[test]
    fn duplicate_numbers_are_rejected() {
        let json = r#"{
            "schema_version": 1,
            "dataset": {
                "id": "test", "name": "test", "source_url": "https://example.invalid",
                "source_file": "x.gpx", "source_sha256": "00", "released": "2025-01-01",
                "retrieved": "2025-01-02", "rights_note": "test"
            },
            "stamps": [
                {"id": 1, "code": "HWN001", "name": "A", "location": "A", "lat": 51.8, "lon": 10.6},
                {"id": 1, "code": "HWN001", "name": "B", "location": "B", "lat": 51.9, "lon": 10.7}
            ]
        }"#;
        assert!(matches!(
            StampCatalog::from_json(json),
            Err(StampError::DuplicateNumber { number: 1 })
        ));
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let json = r#"{
            "schema_version": 99,
            "dataset": {
                "id": "test", "name": "test", "source_url": "https://example.invalid",
                "source_file": "x.gpx", "source_sha256": "00", "released": "2025-01-01",
                "retrieved": "2025-01-02", "rights_note": "test"
            },
            "stamps": []
        }"#;
        assert!(matches!(
            StampCatalog::from_json(json),
            Err(StampError::UnsupportedSchema { found: 99, .. })
        ));
    }

    #[test]
    fn empty_catalogue_is_rejected() {
        let json = r#"{
            "schema_version": 1,
            "dataset": {
                "id": "test", "name": "test", "source_url": "https://example.invalid",
                "source_file": "x.gpx", "source_sha256": "00", "released": "2025-01-01",
                "retrieved": "2025-01-02", "rights_note": "test"
            },
            "stamps": []
        }"#;
        assert!(matches!(
            StampCatalog::from_json(json),
            Err(StampError::EmptyCatalog)
        ));
    }

    #[test]
    fn malformed_code_is_rejected() {
        let json = r#"{
            "schema_version": 1,
            "dataset": {
                "id": "test", "name": "test", "source_url": "https://example.invalid",
                "source_file": "x.gpx", "source_sha256": "00", "released": "2025-01-01",
                "retrieved": "2025-01-02", "rights_note": "test"
            },
            "stamps": [
                {"id": 5, "code": "HWN006", "name": "A", "location": "A", "lat": 51.8, "lon": 10.6}
            ]
        }"#;
        assert!(matches!(
            StampCatalog::from_json(json),
            Err(StampError::MalformedCode { number: 5, .. })
        ));
    }

    #[test]
    fn invalid_json_is_reported_not_panicking() {
        assert!(matches!(
            StampCatalog::from_json("{not json"),
            Err(StampError::InvalidJson { .. })
        ));
    }
}
