// SPDX-License-Identifier: MIT OR Apache-2.0

//! Coordinate type and great-circle distance.

use serde::{Deserialize, Serialize};

use crate::GeoError;

/// Mean Earth radius in metres (IUGG mean radius).
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

/// A WGS84 position in decimal degrees.
///
/// Construction through [`LatLon::new`] rejects non-finite values and
/// out-of-range coordinates, so a value of this type is always a usable
/// position. Deserialisation also validates, which matters because the
/// catalogue and imported GPX files are untrusted input.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LatLonRepr", into = "LatLonRepr")]
pub struct LatLon {
    latitude: f64,
    longitude: f64,
}

/// Plain data representation used only for serde and for crossing the WASM
/// boundary. Field names match the convention used in catalogues and GPX.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatLonRepr {
    /// Latitude in decimal degrees.
    pub lat: f64,
    /// Longitude in decimal degrees.
    pub lon: f64,
}

impl From<LatLon> for LatLonRepr {
    fn from(value: LatLon) -> Self {
        Self {
            lat: value.latitude,
            lon: value.longitude,
        }
    }
}

impl TryFrom<LatLonRepr> for LatLon {
    type Error = GeoError;

    fn try_from(value: LatLonRepr) -> Result<Self, Self::Error> {
        Self::new(value.lat, value.lon)
    }
}

impl LatLon {
    /// Creates a position, checking that the coordinates are usable.
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, GeoError> {
        if !latitude.is_finite() {
            return Err(GeoError::NotFinite { axis: "latitude" });
        }
        if !longitude.is_finite() {
            return Err(GeoError::NotFinite { axis: "longitude" });
        }
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(GeoError::LatitudeOutOfRange { value: latitude });
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(GeoError::LongitudeOutOfRange { value: longitude });
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }

    /// Latitude in decimal degrees.
    #[must_use]
    pub fn latitude(self) -> f64 {
        self.latitude
    }

    /// Longitude in decimal degrees.
    #[must_use]
    pub fn longitude(self) -> f64 {
        self.longitude
    }

    /// Great-circle distance to `other` in metres.
    #[must_use]
    pub fn distance_m(self, other: Self) -> f64 {
        haversine_m(self, other)
    }

    /// Initial bearing from `self` to `other`, in degrees clockwise from north.
    #[must_use]
    pub fn bearing_deg(self, other: Self) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let dlon = (other.longitude - self.longitude).to_radians();
        let y = dlon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
        y.atan2(x).to_degrees().rem_euclid(360.0)
    }
}

/// Great-circle distance in metres between two positions.
///
/// Uses the haversine form, which is numerically stable for the short
/// distances typical of hiking legs (unlike the spherical law of cosines).
#[must_use]
pub fn haversine_m(a: LatLon, b: LatLon) -> f64 {
    let lat1 = a.latitude.to_radians();
    let lat2 = b.latitude.to_radians();
    let dlat = lat2 - lat1;
    let dlon = (b.longitude - a.longitude).to_radians();

    let sin_dlat = (dlat / 2.0).sin();
    let sin_dlon = (dlon / 2.0).sin();
    let h = sin_dlat * sin_dlat + lat1.cos() * lat2.cos() * sin_dlon * sin_dlon;
    // Clamp guards against a haversine slightly above 1 from rounding.
    2.0 * EARTH_RADIUS_M * h.sqrt().clamp(0.0, 1.0).asin()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid coordinate")
    }

    #[test]
    fn rejects_invalid_coordinates() {
        assert!(matches!(
            LatLon::new(f64::NAN, 10.0),
            Err(GeoError::NotFinite { axis: "latitude" })
        ));
        assert!(matches!(
            LatLon::new(10.0, f64::INFINITY),
            Err(GeoError::NotFinite { axis: "longitude" })
        ));
        assert!(matches!(
            LatLon::new(90.5, 10.0),
            Err(GeoError::LatitudeOutOfRange { .. })
        ));
        assert!(matches!(
            LatLon::new(10.0, 181.0),
            Err(GeoError::LongitudeOutOfRange { .. })
        ));
    }

    #[test]
    fn accepts_boundary_coordinates() {
        assert!(LatLon::new(90.0, 180.0).is_ok());
        assert!(LatLon::new(-90.0, -180.0).is_ok());
        assert!(LatLon::new(0.0, 0.0).is_ok());
    }

    #[test]
    fn distance_is_zero_for_identical_points() {
        let p = ll(51.841_649, 10.579_975_7);
        assert_eq!(p.distance_m(p), 0.0);
    }

    #[test]
    fn distance_is_symmetric() {
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        let ab = a.distance_m(b);
        let ba = b.distance_m(a);
        assert!((ab - ba).abs() < 1e-9, "asymmetric: {ab} vs {ba}");
    }

    #[test]
    fn one_degree_of_latitude_is_about_111_kilometres() {
        let d = ll(51.0, 10.0).distance_m(ll(52.0, 10.0));
        // Meridian arc per degree at the mean radius: pi * R / 180 = 111 195 m.
        assert!((d - 111_194.9).abs() < 5.0, "unexpected distance: {d}");
    }

    #[test]
    fn one_degree_of_longitude_shrinks_with_latitude() {
        let at_equator = ll(0.0, 10.0).distance_m(ll(0.0, 11.0));
        let at_harz = ll(51.8, 10.0).distance_m(ll(51.8, 11.0));
        assert!(at_harz < at_equator);
        // cos(51.8 deg) = 0.6184, so roughly 68.8 km.
        assert!((at_harz - 68_770.0).abs() < 100.0, "unexpected: {at_harz}");
    }

    #[test]
    fn known_harz_leg_matches_reference() {
        // Eckertalsperre -> Scharfenstein, two official stamp stations.
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        let d = a.distance_m(b);
        assert!(
            (2_000.0..2_050.0).contains(&d),
            "expected roughly 2.02 km, got {d}"
        );
    }

    #[test]
    fn bearing_points_north_and_east() {
        let origin = ll(51.8, 10.6);
        let north = origin.bearing_deg(ll(51.9, 10.6));
        assert!(north < 0.01 || north > 359.99, "bearing was {north}");
        let east = origin.bearing_deg(ll(51.8, 10.7));
        assert!((east - 90.0).abs() < 0.5, "bearing was {east}");
    }

    #[test]
    fn serde_roundtrip_preserves_coordinates() {
        let p = ll(51.841_649, 10.579_975_7);
        let json = serde_json::to_string(&p).expect("serialise");
        let back: LatLon = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(p, back);
    }

    #[test]
    fn serde_rejects_out_of_range_coordinates() {
        let result = serde_json::from_str::<LatLon>(r#"{"lat":95.0,"lon":10.0}"#);
        assert!(result.is_err());
    }
}
