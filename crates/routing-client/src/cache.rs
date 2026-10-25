// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small deterministic route cache.
//!
//! Re-planning a tour revisits the same legs many times (2-opt in particular),
//! and every miss against a public routing service costs a round trip and,
//! usually, quota. Coordinate keys are rounded to a micodegree so that
//! floating-point noise from the optimizers does not defeat the cache.
//!
//! Eviction is a full clear when the capacity is reached: predictable, cheap,
//! and adequate for the few thousand legs a browser session produces.

use std::collections::HashMap;

use geo_engine::LatLon;

use crate::provider::RouteLeg;

/// Multiplier that turns a coordinate into a discrete key.
const KEY_SCALE: f64 = 1_000_000.0;

/// A leg identifier: origin and destination, quantised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    from: (i64, i64),
    to: (i64, i64),
}

impl Key {
    fn new(from: LatLon, to: LatLon) -> Self {
        fn quantise(position: LatLon) -> (i64, i64) {
            // Rounding, not truncation, so keys are symmetric around a point.
            #[allow(clippy::cast_possible_truncation)]
            let lat = (position.latitude() * KEY_SCALE).round() as i64;
            #[allow(clippy::cast_possible_truncation)]
            let lon = (position.longitude() * KEY_SCALE).round() as i64;
            (lat, lon)
        }
        Self {
            from: quantise(from),
            to: quantise(to),
        }
    }
}

/// Caches routed legs by quantised endpoints.
#[derive(Debug, Clone)]
pub struct RouteCache {
    entries: HashMap<Key, RouteLeg>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl RouteCache {
    /// Creates a cache holding at most `capacity` legs.
    ///
    /// A capacity of zero is treated as one so the cache never degrades into a
    /// cache that cannot store anything.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            hits: 0,
            misses: 0,
        }
    }

    /// Looks up a leg, recording a hit or a miss.
    #[must_use]
    pub fn get(&mut self, from: LatLon, to: LatLon) -> Option<RouteLeg> {
        let key = Key::new(from, to);
        match self.entries.get(&key) {
            Some(leg) => {
                self.hits += 1;
                Some(leg.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Stores a leg, evicting everything when the cache is full.
    pub fn insert(&mut self, from: LatLon, to: LatLon, leg: RouteLeg) {
        if self.entries.len() >= self.capacity {
            self.entries.clear();
        }
        self.entries.insert(Key::new(from, to), leg);
    }

    /// Returns the cached leg or computes and stores it.
    pub fn get_or_try_insert_with<F>(
        &mut self,
        from: LatLon,
        to: LatLon,
        compute: F,
    ) -> Result<RouteLeg, crate::RoutingError>
    where
        F: FnOnce() -> Result<RouteLeg, crate::RoutingError>,
    {
        if let Some(leg) = self.get(from, to) {
            return Ok(leg);
        }
        let leg = compute()?;
        self.insert(from, to, leg.clone());
        Ok(leg)
    }

    /// Number of cached legs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of successful lookups so far.
    #[must_use]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Number of unsuccessful lookups so far.
    #[must_use]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Drops every entry while keeping the hit/miss counters.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for RouteCache {
    fn default() -> Self {
        Self::with_capacity(4_096)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StraightLineProvider;
    use crate::provider::RouteProvider;
    use std::cell::Cell;

    fn ll(lat: f64, lon: f64) -> LatLon {
        LatLon::new(lat, lon).expect("valid")
    }

    fn leg(distance: f64) -> RouteLeg {
        RouteLeg::new(
            distance,
            distance / 1.25,
            vec![ll(51.8, 10.6), ll(51.81, 10.6)],
            true,
        )
    }

    #[test]
    fn miss_then_hit() {
        let mut cache = RouteCache::with_capacity(8);
        let a = ll(51.8, 10.6);
        let b = ll(51.9, 10.7);
        assert!(cache.get(a, b).is_none());
        cache.insert(a, b, leg(1_000.0));
        let hit = cache.get(a, b).expect("hit");
        assert!((hit.distance_m - 1_000.0).abs() < 1e-9);
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn keys_are_direction_sensitive() {
        let mut cache = RouteCache::with_capacity(8);
        let a = ll(51.8, 10.6);
        let b = ll(51.9, 10.7);
        cache.insert(a, b, leg(1_000.0));
        assert!(cache.get(b, a).is_none(), "reversed leg must not match");
    }

    #[test]
    fn sub_microdegree_noise_still_hits() {
        let mut cache = RouteCache::with_capacity(8);
        let a = ll(51.8, 10.6);
        let b = ll(51.9, 10.7);
        cache.insert(a, b, leg(1_000.0));
        let noisy = ll(51.9 + 1e-9, 10.7 - 1e-9);
        assert!(cache.get(a, noisy).is_some());
    }

    #[test]
    fn get_or_try_insert_computes_only_once() {
        let mut cache = RouteCache::with_capacity(8);
        let provider = StraightLineProvider::default();
        let a = ll(51.841_649, 10.579_975_7);
        let b = ll(51.830_167_7, 10.602_774_2);
        let calls = Cell::new(0);

        let first = cache
            .get_or_try_insert_with(a, b, || {
                calls.set(calls.get() + 1);
                provider.route(a, b)
            })
            .expect("first");
        let second = cache
            .get_or_try_insert_with(a, b, || {
                calls.set(calls.get() + 1);
                provider.route(a, b)
            })
            .expect("second");

        assert_eq!(calls.get(), 1, "provider must be consulted once");
        assert_eq!(first, second);
    }

    #[test]
    fn eviction_keeps_the_cache_available() {
        let mut cache = RouteCache::with_capacity(2);
        let a = ll(51.0, 10.0);
        for step in 0..5 {
            let b = ll(52.0 + f64::from(step) * 0.001, 11.0);
            cache.insert(a, b, leg(1_000.0));
            assert!(cache.len() <= 2, "capacity must be respected");
        }
        assert!(!cache.is_empty());
    }

    #[test]
    fn zero_capacity_is_coerced_to_one() {
        let mut cache = RouteCache::with_capacity(0);
        let a = ll(51.8, 10.6);
        let b = ll(51.9, 10.7);
        cache.insert(a, b, leg(1.0));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn clear_drops_entries_but_keeps_counters() {
        let mut cache = RouteCache::with_capacity(4);
        let a = ll(51.8, 10.6);
        cache.insert(a, ll(51.9, 10.7), leg(1.0));
        let _ = cache.get(a, ll(51.9, 10.7));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.hits(), 1);
    }
}
