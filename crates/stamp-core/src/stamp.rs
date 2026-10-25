// SPDX-License-Identifier: MIT OR Apache-2.0

//! A single stamp station.

use geo_engine::LatLon;
use serde::{Deserialize, Serialize};

use crate::{MAX_STAMP_NUMBER, StampError};

/// The numeric identifier of a stamp station (1-based).
///
/// Wrapping the raw number in a newtype keeps station numbers from being
/// confused with distances, indices or day numbers, and centralises the range
/// check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct StampId(u16);

impl StampId {
    /// Creates an identifier, rejecting zero and numbers above
    /// [`MAX_STAMP_NUMBER`].
    pub fn new(number: u16) -> Result<Self, StampError> {
        if number == 0 || number > MAX_STAMP_NUMBER {
            return Err(StampError::NumberOutOfRange {
                value: number,
                max: MAX_STAMP_NUMBER,
            });
        }
        Ok(Self(number))
    }

    /// The raw station number.
    #[must_use]
    pub fn number(self) -> u16 {
        self.0
    }

    /// The canonical station code, for example `HWN001`.
    #[must_use]
    pub fn code(self) -> String {
        format!("HWN{:03}", self.0)
    }
}

impl TryFrom<u16> for StampId {
    type Error = StampError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<StampId> for u16 {
    fn from(value: StampId) -> Self {
        value.0
    }
}

impl std::fmt::Display for StampId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HWN{:03}", self.0)
    }
}

/// One stamp station of the Harzer Wandernadel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// Numeric station number.
    pub id: StampId,
    /// Canonical code such as `HWN001`.
    pub code: String,
    /// Short station name, for example `Eckertalsperre`.
    pub name: String,
    /// Descriptive location, often naming the feature the box stands at.
    pub location: String,
    /// Exact position of the stamp box.
    pub position: LatLon,
    /// Elevation in metres, when the source provides one.
    pub elevation_m: Option<f64>,
}

impl Stamp {
    /// Validates the fields that `serde` cannot express (code/name invariants).
    pub(crate) fn validate(&self) -> Result<(), StampError> {
        let number = self.id.number();
        let expected_code = self.id.code();
        if self.code != expected_code {
            return Err(StampError::MalformedCode {
                number,
                code: self.code.clone(),
            });
        }
        if self.name.trim().is_empty() {
            return Err(StampError::EmptyField {
                number,
                field: "name",
            });
        }
        if self.location.trim().is_empty() {
            return Err(StampError::EmptyField {
                number,
                field: "location",
            });
        }
        if let Some(elevation) = self.elevation_m {
            if !elevation.is_finite() {
                return Err(StampError::InvalidPosition {
                    number,
                    source: geo_engine::GeoError::NotFinite { axis: "elevation" },
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_numbers_are_accepted() {
        assert!(StampId::new(1).is_ok());
        assert!(StampId::new(222).is_ok());
        assert!(StampId::new(MAX_STAMP_NUMBER).is_ok());
    }

    #[test]
    fn zero_and_oversized_numbers_are_rejected() {
        assert!(matches!(
            StampId::new(0),
            Err(StampError::NumberOutOfRange { value: 0, .. })
        ));
        assert!(StampId::new(MAX_STAMP_NUMBER + 1).is_err());
    }

    #[test]
    fn code_is_zero_padded() {
        assert_eq!(StampId::new(1).expect("id").code(), "HWN001");
        assert_eq!(StampId::new(42).expect("id").code(), "HWN042");
        assert_eq!(StampId::new(222).expect("id").code(), "HWN222");
    }

    #[test]
    fn display_matches_code() {
        let id = StampId::new(7).expect("id");
        assert_eq!(id.to_string(), "HWN007");
    }

    #[test]
    fn serde_roundtrip_uses_bare_number() {
        let id = StampId::new(222).expect("id");
        let json = serde_json::to_string(&id).expect("serialise");
        assert_eq!(json, "222");
        let back: StampId = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, id);
    }

    #[test]
    fn serde_rejects_zero() {
        assert!(serde_json::from_str::<StampId>("0").is_err());
    }

    #[test]
    fn ids_order_numerically() {
        let mut ids = [
            StampId::new(10).expect("id"),
            StampId::new(2).expect("id"),
            StampId::new(1).expect("id"),
        ];
        ids.sort();
        assert_eq!(
            ids.iter().map(|id| id.number()).collect::<Vec<_>>(),
            vec![1, 2, 10]
        );
    }

    fn sample_stamp() -> Stamp {
        Stamp {
            id: StampId::new(1).expect("id"),
            code: "HWN001".to_string(),
            name: "Eckertalsperre".to_string(),
            location: "Eckertalsperre (Staumauer)".to_string(),
            position: LatLon::new(51.841_649, 10.579_975_7).expect("position"),
            elevation_m: Some(555.0),
        }
    }

    #[test]
    fn a_well_formed_stamp_validates() {
        assert!(sample_stamp().validate().is_ok());
    }

    #[test]
    fn mismatched_code_is_rejected() {
        let mut stamp = sample_stamp();
        stamp.code = "HWN002".to_string();
        assert!(matches!(
            stamp.validate(),
            Err(StampError::MalformedCode { number: 1, .. })
        ));
    }

    #[test]
    fn blank_name_is_rejected() {
        let mut stamp = sample_stamp();
        stamp.name = "   ".to_string();
        assert!(matches!(
            stamp.validate(),
            Err(StampError::EmptyField { field: "name", .. })
        ));
    }
}
