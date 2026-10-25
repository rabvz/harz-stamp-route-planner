// SPDX-License-Identifier: MIT OR Apache-2.0

//! Path measurement and simplification.

use crate::{GeoError, LatLon};

/// Total length of a path in metres, summing consecutive great-circle legs.
#[must_use]
pub fn path_length_m(points: &[LatLon]) -> f64 {
    points
        .windows(2)
        .map(|pair| pair[0].distance_m(pair[1]))
        .sum()
}

/// Reduces a path with the Douglas-Peucker algorithm.
///
/// `tolerance_m` is the maximum permitted perpendicular deviation. Used to keep
/// exported GPX files small without visibly changing the track, and to make
/// unit tests compare shapes instead of every source vertex.
///
/// The first and last points are always kept. A tolerance of zero keeps every
/// point that is not exactly collinear.
pub fn simplify(points: &[LatLon], tolerance_m: f64) -> Result<Vec<LatLon>, GeoError> {
    if !tolerance_m.is_finite() || tolerance_m < 0.0 {
        return Err(GeoError::InvalidMagnitude {
            name: "tolerance_m",
            value: tolerance_m,
        });
    }
    if points.len() <= 2 {
        return Ok(points.to_vec());
    }

    // Work in a local plane centred on the first point so perpendicular
    // distances are meaningful in metres.
    let plane = crate::LocalPlane::new(points[0]);
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;

    // Iterative stack instead of recursion: imported tracks can be long and
    // the recursion depth would depend on untrusted input.
    let mut stack = vec![(0_usize, points.len() - 1)];
    while let Some((start, end)) = stack.pop() {
        if end <= start + 1 {
            continue;
        }
        let a = plane.project(points[start]);
        let b = plane.project(points[end]);

        let mut worst_index = start;
        let mut worst_distance = -1.0_f64;
        for (offset, point) in points[start + 1..end].iter().enumerate() {
            let distance = perpendicular_distance(plane.project(*point), a, b);
            if distance > worst_distance {
                worst_distance = distance;
                worst_index = start + 1 + offset;
            }
        }

        if worst_distance > tolerance_m {
            keep[worst_index] = true;
            stack.push((start, worst_index));
            stack.push((worst_index, end));
        }
    }

    Ok(points
        .iter()
        .zip(keep)
        .filter_map(|(point, keep)| keep.then_some(*point))
        .collect())
}

/// Perpendicular distance from `p` to the segment `a`-`b`, in plane units.
fn perpendicular_distance(p: crate::Point, a: crate::Point, b: crate::Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= f64::EPSILON {
        return p.distance_to(a);
    }
    // Projection parameter of p onto the segment, clamped to the segment.
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / length_sq).clamp(0.0, 1.0);
    let closest = crate::Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    };
    p.distance_to(closest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    #[test]
    fn empty_and_single_point_paths_have_zero_length() {
        assert_eq!(path_length_m(&[]), 0.0);
        assert_eq!(path_length_m(&[ll(51.8, 10.6)]), 0.0);
    }

    #[test]
    fn length_sums_the_legs() {
        let a = ll(51.8, 10.6);
        let b = ll(51.9, 10.6);
        let c = ll(52.0, 10.6);
        let expected = a.distance_m(b) + b.distance_m(c);
        assert!((path_length_m(&[a, b, c]) - expected).abs() < 1e-6);
    }

    #[test]
    fn collinear_points_collapse() {
        let points: Vec<_> = (0..=10)
            .map(|step| ll(51.8 + f64::from(step) * 0.01, 10.6))
            .collect();
        let simplified = simplify(&points, 1.0).expect("simplify");
        assert_eq!(simplified.len(), 2, "expected only the endpoints to remain");
        assert_eq!(simplified[0], points[0]);
        assert_eq!(simplified[1], points[10]);
    }

    #[test]
    fn a_real_detour_survives_simplification() {
        // A point roughly 500 m east of a north-south line must be kept when
        // the tolerance is small enough to notice it.
        let points = [ll(51.80, 10.60), ll(51.85, 10.6072), ll(51.90, 10.60)];
        let simplified = simplify(&points, 10.0).expect("simplify");
        assert_eq!(simplified.len(), 3);
    }

    #[test]
    fn small_deviation_is_dropped() {
        // The middle point deviates by roughly 3 m, below a 50 m tolerance.
        let points = [ll(51.80, 10.60), ll(51.85, 10.600_04), ll(51.90, 10.60)];
        let simplified = simplify(&points, 50.0).expect("simplify");
        assert_eq!(simplified.len(), 2);
    }

    #[test]
    fn zero_tolerance_keeps_detours() {
        let points = [ll(51.80, 10.60), ll(51.85, 10.600_04), ll(51.90, 10.60)];
        let simplified = simplify(&points, 0.0).expect("simplify");
        assert_eq!(simplified.len(), 3);
    }

    #[test]
    fn simplify_rejects_invalid_tolerance() {
        let points = [ll(51.80, 10.60), ll(51.90, 10.60)];
        assert!(simplify(&points, -1.0).is_err());
        assert!(simplify(&points, f64::NAN).is_err());
    }

    #[test]
    fn simplify_never_invents_points() {
        let points = [ll(51.80, 10.60), ll(51.85, 10.63), ll(51.90, 10.60)];
        let simplified = simplify(&points, 100.0).expect("simplify");
        for point in &simplified {
            assert!(points.contains(point), "{point:?} was not in the input");
        }
    }
}
