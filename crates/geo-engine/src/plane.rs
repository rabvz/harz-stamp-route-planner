// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local planar projection used by the optimizers.
//!
//! Great-circle distances cannot be summed or compared cheaply inside a
//! tour-construction loop that runs in the browser, so the optimizers work on
//! a local equirectangular projection and convert back to metres through it.
//! Over the extent of the Harz (about 80 km) the distortion is well below the
//! accuracy of the underlying routing data, and it is documented as an
//! approximation rather than a precise geodesic computation.

use crate::{GeoError, LatLon};

/// A point in the local metric plane, in metres relative to the plane origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Easting in metres relative to the origin.
    pub x: f64,
    /// Northing in metres relative to the origin.
    pub y: f64,
}

impl Point {
    /// Euclidean distance between two planar points.
    #[must_use]
    pub fn distance_to(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx.hypot(dy)
    }
}

/// An equirectangular projection centred on a reference position.
#[derive(Debug, Clone, Copy)]
pub struct LocalPlane {
    origin: LatLon,
    /// Metres per degree of latitude (constant).
    m_per_deg_lat: f64,
    /// Metres per degree of longitude at the origin latitude.
    m_per_deg_lon: f64,
}

impl LocalPlane {
    /// Createsa projection centred on `origin`.
    #[must_use]
    pub fn new(origin: LatLon) -> Self {
        // 2 * pi * R / 360, using the mean Earth radius for consistency with
        // the haversine implementation.
        let m_per_deg_lat = crate::EARTH_RADIUS_M * std::f64::consts::PI / 180.0;
        let cos_lat = origin.latitude().to_radians().cos().abs().max(1e-6);
        Self {
            origin,
            m_per_deg_lat,
            m_per_deg_lon: m_per_deg_lat * cos_lat,
        }
    }

    /// The projection origin.
    #[must_use]
    pub fn origin(&self) -> LatLon {
        self.origin
    }

    /// Projects a position onto the plane, in metres relative to the origin.
    #[must_use]
    pub fn project(&self, position: LatLon) -> Point {
        Point {
            x: (position.longitude() - self.origin.longitude()) * self.m_per_deg_lon,
            y: (position.latitude() - self.origin.latitude()) * self.m_per_deg_lat,
        }
    }

    /// Inverse of [`LocalPlane::project`].
    pub fn unproject(&self, point: Point) -> Result<LatLon, GeoError> {
        LatLon::new(
            self.origin.latitude() + point.y / self.m_per_deg_lat,
            self.origin.longitude() + point.x / self.m_per_deg_lon,
        )
    }

    /// Planar distance between two positions, in metres.
    #[must_use]
    pub fn distance_m(&self, a: LatLon, b: LatLon) -> f64 {
        self.project(a).distance_to(self.project(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    #[test]
    fn origin_projects_to_zero() {
        let plane = LocalPlane::new(ll(51.8, 10.6));
        let p = plane.project(ll(51.8, 10.6));
        assert!(p.x.abs() < 1e-9 && p.y.abs() < 1e-9);
    }

    #[test]
    fn one_degree_north_is_about_111_km() {
        let plane = LocalPlane::new(ll(51.8, 10.6));
        let p = plane.project(ll(52.8, 10.6));
        assert!((p.x.abs() < 1e-6), "no northing expected on x");
        assert!((p.y - 111_194.9).abs() < 10.0, "y was {}", p.y);
    }

    #[test]
    fn planar_and_haversine_agree_locally() {
        let plane = LocalPlane::new(ll(51.8, 10.6));
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        let planar = plane.distance_m(a, b);
        let haversine = a.distance_m(b);
        let error = (planar - haversine).abs() / haversine;
        assert!(error < 0.005, "relative error {error} too large");
    }

    #[test]
    fn project_unproject_roundtrips() {
        let plane = LocalPlane::new(ll(51.8, 10.6));
        let original = ll(51.841_649, 10.579_975_7);
        let back = plane.unproject(plane.project(original)).expect("back");
        assert!((back.latitude() - original.latitude()).abs() < 1e-9);
        assert!((back.longitude() - original.longitude()).abs() < 1e-9);
    }

    #[test]
    fn planar_distance_is_symmetric() {
        let plane = LocalPlane::new(ll(51.8, 10.6));
        let a = ll(51.9, 10.7);
        let b = ll(51.5, 10.3);
        assert!((plane.distance_m(a, b) - plane.distance_m(b, a)).abs() < 1e-9);
    }
}
