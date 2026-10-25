// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors produced by the geometry layer.

use thiserror::Error;

/// Reasons a geometric value could not be constructed or used.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum GeoError {
    /// A coordinate component was NaN or infinite.
    #[error("{axis} is not a finite number")]
    NotFinite {
        /// Name of the offending component.
        axis: &'static str,
    },

    /// Latitude fell outside [-90, 90].
    #[error("latitude {value} is outside [-90, 90]")]
    LatitudeOutOfRange {
        /// The rejected value.
        value: f64,
    },

    /// Longitude fell outside [-180, 180].
    #[error("longitude {value} is outside [-180, 180]")]
    LongitudeOutOfRange {
        /// The rejected value.
        value: f64,
    },

    /// An operation required at least one point but received none.
    #[error("expected at least one point, got none")]
    Empty,

    /// A distance or tolerance was negative or not finite.
    #[error("{name} must be a finite, non-negative number, got {value}")]
    InvalidMagnitude {
        /// Name of the offending parameter.
        name: &'static str,
        /// The rejected value.
        value: f64,
    },
}
