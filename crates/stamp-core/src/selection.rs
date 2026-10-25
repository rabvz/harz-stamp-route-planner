// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which stamp stations a user wants to collect on this trip.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{Stamp, StampCatalog, StampError, StampId};

/// An ordered set of chosen stamp stations.
///
/// The set is intentionally independent of a catalogue: it can hold numbers the
/// current catalogue does not know (for example after an import from an older
/// dataset). [`StampSelection::resolve`] and
/// [`StampSelection::validate_against`] are the points where a catalogue is
/// applied, so stale selections surface as clear errors instead of silently
/// dropping stations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampSelection {
    selected: BTreeSet<StampId>,
}

impl StampSelection {
    /// Creates an empty selection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a selection from station numbers, rejecting duplicates eagerly
    /// is unnecessary because the set deduplicates; invalid numbers are still
    /// rejected.
    pub fn from_numbers(numbers: impl IntoIterator<Item = u16>) -> Result<Self, StampError> {
        let mut selected = BTreeSet::new();
        for number in numbers {
            selected.insert(StampId::new(number)?);
        }
        Ok(Self { selected })
    }

    /// Number of selected stations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Whether a station is selected.
    #[must_use]
    pub fn contains(&self, id: StampId) -> bool {
        self.selected.contains(&id)
    }

    /// Adds a station; returns `true` if it was not present before.
    pub fn insert(&mut self, id: StampId) -> bool {
        self.selected.insert(id)
    }

    /// Removes a station; returns `true` if it was present before.
    pub fn remove(&mut self, id: StampId) -> bool {
        self.selected.remove(&id)
    }

    /// Flips a station on or off; returns the new state.
    pub fn toggle(&mut self, id: StampId) -> bool {
        if self.selected.remove(&id) {
            false
        } else {
            self.selected.insert(id);
            true
        }
    }

    /// Removes every station.
    pub fn clear(&mut self) {
        self.selected.clear();
    }

    /// Selects every station of a catalogue.
    pub fn select_all(&mut self, catalog: &StampCatalog) {
        self.selected = catalog.stamps().iter().map(|stamp| stamp.id).collect();
    }

    /// Selected station numbers in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = StampId> + '_ {
        self.selected.iter().copied()
    }

    /// Selected stations resolved against a catalogue, in station-number order.
    ///
    /// Unknown station numbers are skipped here so that rendering stays
    /// robust; call [`StampSelection::validate_against`] when the selection
    /// must be complete before planning.
    pub fn resolve<'a>(&'a self, catalog: &'a StampCatalog) -> Vec<&'a Stamp> {
        self.selected
            .iter()
            .filter_map(|id| catalog.get(*id))
            .collect()
    }

    /// Fails if any selected station is missing from the catalogue.
    pub fn validate_against(&self, catalog: &StampCatalog) -> Result<(), StampError> {
        for id in &self.selected {
            if catalog.get(*id).is_none() {
                return Err(StampError::UnknownStamp {
                    number: id.number(),
                    dataset: catalog.dataset().id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Number of selected stations that the catalogue does not contain.
    #[must_use]
    pub fn unknown_count(&self, catalog: &StampCatalog) -> usize {
        self.selected
            .iter()
            .filter(|id| catalog.get(**id).is_none())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(number: u16) -> StampId {
        StampId::new(number).expect("valid id")
    }

    #[test]
    fn new_selection_is_empty() {
        let selection = StampSelection::new();
        assert!(selection.is_empty());
        assert_eq!(selection.len(), 0);
    }

    #[test]
    fn insert_remove_and_toggle_report_state_changes() {
        let mut selection = StampSelection::new();
        assert!(selection.insert(id(1)), "first insert is a change");
        assert!(!selection.insert(id(1)), "second insert is not");
        assert!(selection.contains(id(1)));

        assert!(selection.remove(id(1)));
        assert!(!selection.remove(id(1)));
        assert!(!selection.contains(id(1)));

        assert!(selection.toggle(id(2)), "toggle on");
        assert!(!selection.toggle(id(2)), "toggle off");
    }

    #[test]
    fn ids_are_returned_in_ascending_order_regardless_of_insertion() {
        let mut selection = StampSelection::new();
        for number in [42_u16, 7, 222, 1] {
            selection.insert(id(number));
        }
        let numbers: Vec<u16> = selection.ids().map(StampId::number).collect();
        assert_eq!(numbers, vec![1, 7, 42, 222]);
    }

    #[test]
    fn select_all_covers_the_catalogue() {
        let catalog = StampCatalog::embedded();
        let mut selection = StampSelection::new();
        selection.select_all(catalog);
        assert_eq!(selection.len(), catalog.len());
        assert!(selection.validate_against(catalog).is_ok());
    }

    #[test]
    fn resolve_returns_stamps_in_order() {
        let catalog = StampCatalog::embedded();
        let selection = StampSelection::from_numbers([3, 1, 2]).expect("selection");
        let stamps = selection.resolve(catalog);
        assert_eq!(stamps.len(), 3);
        assert_eq!(
            stamps.iter().map(|s| s.id.number()).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(stamps[0].name, "Eckertalsperre");
    }

    #[test]
    fn resolve_skips_unknown_but_validate_reports_them() {
        let catalog = StampCatalog::embedded();
        let selection = StampSelection::from_numbers([1, 900]).expect("selection");
        assert_eq!(selection.resolve(catalog).len(), 1);
        assert_eq!(selection.unknown_count(catalog), 1);
        assert!(matches!(
            selection.validate_against(catalog),
            Err(StampError::UnknownStamp { number: 900, .. })
        ));
    }

    #[test]
    fn from_numbers_rejects_zero() {
        assert!(StampSelection::from_numbers([0]).is_err());
    }

    #[test]
    fn clear_empties_the_selection() {
        let mut selection = StampSelection::from_numbers([1, 2, 3]).expect("selection");
        selection.clear();
        assert!(selection.is_empty());
    }

    #[test]
    fn serde_roundtrip_preserves_selection() {
        let selection = StampSelection::from_numbers([5, 1, 222]).expect("selection");
        let json = serde_json::to_string(&selection).expect("serialise");
        let back: StampSelection = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(selection, back);
        assert_eq!(back.len(), 3);
    }
}
