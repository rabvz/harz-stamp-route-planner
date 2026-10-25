// SPDX-License-Identifier: MIT OR Apache-2.0

//! Axis-aligned bounding boxes in degrees.

use serde::{Deserialize, Serialize};

use crate::{GeoError, LatLon};

/// An axis-aligned box on the coordinate grid, used for spatial pre-filtering
/// and for fitting the map viewport.
///
/// Antimeridian-crossing boxes are deliberately unsupported: the application
/// is scoped to the Harz, so a plain min/max box is correct and far simpler.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// South-west corner (minimum latitude, minimum longitude).
    pub south_west: LatLon,
    /// North-east corner (maximum latitude, maximum longitude).
    pub north_east: LatLon,
}

impl BoundingBox {
    /// Builds a box from two opposite corners in any order.
    pub fn from_corners(a: LatLon, b: LatLon) -> Result<Self, GeoError> {
        let south = a.latitude().min(b.latitude());
        let north = a.latitude().max(b.latitude());
        let west = a.longitude().min(b.longitude());
        let east = a.longitude().max(b.longitude());
        Ok(Self {
            south_west: LatLon::new(south, west)?,
            north_east: LatLon::new(north, east)?,
        })
    }

    /// Builds the smallest box containing every given point.
    pub fn from_points(points: &[LatLon]) -> Result<Self, GeoError> {
        let first = *points.first().ok_or(GeoError::Empty)?;
        let mut south = first.latitude();
        let mut north = first.latitude();
        let mut west = first.longitude();
        let mut east = first.longitude();

        for p in points {
            south = south.min(p.latitude());
            north = north.max(p.latitude());
            west = west.min(p.longitude());
            east = east.max(p.longitude());
        }

        Ok(Self {
            south_west: LatLon::new(south, west)?,
            north_east: LatLon::new(north, east)?,
        })
    }

    /// Geometric centre of the box.
    pub fn center(self) -> Result<LatLon, GeoError> {
        LatLon::new(
            (self.south_west.latitude() + self.north_east.latitude()) / 2.0,
            (self.south_west.longitude() + self.north_east.longitude()) / 2.0,
        )
    }

    /// Whether `point` lies inside the box (inclusive edges).
    #[must_use]
    pub fn contains(self, point: LatLon) -> bool {
        point.latitude() >= self.south_west.latitude()
            && point.latitude() <= self.north_east.latitude()
            && point.longitude() >= self.south_west.longitude()
            && point.longitude() <= self.north_east.longitude()
    }

    /// Whether two boxes share any area (touching edges count as overlap).
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.south_west.latitude() <= other.north_east.latitude()
            && other.south_west.latitude() <= self.north_east.latitude()
            && self.south_west.longitude() <= other.north_east.longitude()
            && other.south_west.longitude() <= self.north_east.longitude()
    }

    /// Returns a box grown by `margin_m` metres in every direction.
    pub fn expanded_m(self, margin_m: f64) -> Result<Self, GeoError> {
        if !margin_m.is_finite() || margin_m < 0.0 {
            return Err(GeoError::InvalidMagnitude {
                name: "margin_m",
                value: margin_m,
            });
        }
        // Degrees of latitude per metre, and the same for longitude at the
        // box's own latitude (longitude degrees shrink towards the poles).
        let d_lat = margin_m / 111_320.0;
        let mean_lat = (self.south_west.latitude() + self.north_east.latitude()) / 2.0;
        let cos_lat = mean_lat.to_radians().cos().abs().max(1e-6);
        let d_lon = d_lat / cos_lat;

        let south = (self.south_west.latitude() - d_lat).max(-90.0);
        let north = (self.north_east.latitude() + d_lat).min(90.0);
        let west = (self.south_west.longitude() - d_lon).max(-180.0);
        let east = (self.north_east.longitude() + d_lon).min(180.0);

        Ok(Self {
            south_west: LatLon::new(south, west)?,
            north_east: LatLon::new(north, east)?,
        })
    }

    /// Longest great-circle distance between any two points of the box,
    /// approximated by its diagonal. Used for clustering diameters.
    pub fn diagonal_m(self) -> f64 {
        self.south_west.distance_m(self.north_east)
    }

    /// Width in metres at the box's mean latitude.
    pub fn width_m(self) -> f64 {
        let mean_lat = (self.south_west.latitude() + self.north_east.latitude()) / 2.0;
        LatLon::new(mean_lat, self.south_west.longitude())
            .and_then(|w| {
                LatLon::new(mean_lat, self.north_east.longitude()).map(|e| w.distance_m(e))
            })
            .unwrap_or(0.0)
    }

    /// Height in metres along the meridian.
    pub fn height_m(self) -> f64 {
        LatLon::new(self.south_west.latitude(), 0.0)
            .and_then(|s| LatLon::new(self.north_east.latitude(), 0.0).map(|n| s.distance_m(n)))
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    #[test]
    fn from_corners_normalises_ordering() {
        let a = BoundingBox::from_corners(ll(52.0, 11.0), ll(51.0, 10.0)).expect("box");
        let b = BoundingBox::from_corners(ll(51.0, 10.0), ll(52.0, 11.0)).expect("box");
        assert_eq!(a, b);
        assert_eq!(a.south_west.latitude(), 51.0);
        assert_eq!(a.north_east.longitude(), 11.0);
    }

    #[test]
    fn from_points_matches_known_extent() {
        let points = [
            ll(51.841_649, 10.579_975_7),
            ll(51.488_97, 10.225_03),
            ll(51.914_84, 11.347_799_9),
        ];
        let bbox = BoundingBox::from_points(&points).expect("box");
        assert!((bbox.south_west.latitude() - 51.488_97).abs() < 1e-9);
        assert!((bbox.north_east.longitude() - 11.347_799_9).abs() < 1e-9);
        for p in points {
            assert!(bbox.contains(p));
        }
    }

    #[test]
    fn from_points_rejects_empty_input() {
        assert!(matches!(
            BoundingBox::from_points(&[]),
            Err(GeoError::Empty)
        ));
    }

    #[test]
    fn contains_respects_edges() {
        let bbox = BoundingBox::from_corners(ll(51.0, 10.0), ll(52.0, 11.0)).expect("box");
        assert!(bbox.contains(ll(51.0, 10.0)), "south-west corner");
        assert!(bbox.contains(ll(52.0, 11.0)), "north-east corner");
        assert!(!bbox.contains(ll(52.000_1, 11.0)));
    }

    #[test]
    fn intersects_detects_overlap_and_separation() {
        let a = BoundingBox::from_corners(ll(51.0, 10.0), ll(52.0, 11.0)).expect("box");
        let overlapping = BoundingBox::from_corners(ll(51.5, 10.5), ll(53.0, 12.0)).expect("box");
        let separate = BoundingBox::from_corners(ll(50.0, 8.0), ll(50.5, 9.0)).expect("box");
        assert!(a.intersects(overlapping));
        assert!(overlapping.intersects(a));
        assert!(!a.intersects(separate));
    }

    #[test]
    fn expanded_grows_and_clamps() {
        let bbox = BoundingBox::from_corners(ll(51.0, 10.0), ll(51.1, 10.1)).expect("box");
        let grown = bbox.expanded_m(1_000.0).expect("grown");
        assert!(grown.south_west.latitude() < bbox.south_west.latitude());
        assert!(grown.north_east.longitude() > bbox.north_east.longitude());
        assert!(grown.contains(ll(51.0, 10.0)));
    }

    #[test]
    fn expanded_rejects_invalid_margin() {
        let bbox = BoundingBox::from_corners(ll(51.0, 10.0), ll(51.1, 10.1)).expect("box");
        assert!(bbox.expanded_m(-5.0).is_err());
        assert!(bbox.expanded_m(f64::NAN).is_err());
    }

    #[test]
    fn centre_is_the_midpoint() {
        let bbox = BoundingBox::from_corners(ll(51.0, 10.0), ll(52.0, 11.0)).expect("box");
        let c = bbox.center().expect("centre");
        assert!((c.latitude() - 51.5).abs() < 1e-9);
        assert!((c.longitude() - 10.5).abs() < 1e-9);
    }

    #[test]
    fn width_and_height_are_plausible_for_the_harz() {
        // Extent of the official station dataset.
        let bbox = BoundingBox::from_corners(ll(51.488_97, 10.225_03), ll(51.914_84, 11.347_8))
            .expect("box");
        let width = bbox.width_m();
        let height = bbox.height_m();
        // Roughly 79 km east-west and 47 km north-south.
        assert!((75_000.0..83_000.0).contains(&width), "width {width}");
        assert!((45_000.0..49_000.0).contains(&height), "height {height}");
    }
}
