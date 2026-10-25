// SPDX-License-Identifier: MIT OR Apache-2.0

//! Optimizer errors and constraint warnings.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Reasons a plan could not be built.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum OptimizerError {
    /// No stations were supplied.
    #[error("no stamps selected for planning")]
    NoStamps,

    /// The options were unusable.
    #[error("invalid optimizer options: {message}")]
    InvalidOptions {
        /// What is wrong with the options.
        message: String,
    },

    /// A station appeared twice in the request.
    #[error("duplicate stamp {number} in the request")]
    DuplicateStamp {
        /// The repeated station number.
        number: u16,
    },

    /// The supplied distance matrix did not match the node count.
    #[error("distance matrix has {found} entries, expected {expected}")]
    MatrixShape {
        /// Entries required: `(n + 1)^2`.
        expected: usize,
        /// Entries supplied.
        found: usize,
    },

    /// A matrix entry was negative or not a number.
    #[error("distance from node {from} to node {to} is invalid: {value}")]
    InvalidDistance {
        /// Origin node index.
        from: usize,
        /// Destination node index.
        to: usize,
        /// The rejected value.
        value: f64,
    },
}

/// A constraint the tour could not satisfy.
///
/// Warnings are part of the result rather than errors: dropping chosen stations
/// silently would be worse, so the tour is returned and the problem is stated.
///
/// `rename_all` renames the variant tags, `rename_all_fields` the payload
/// fields. Both are camelCase because this enum crosses into JavaScript, where
/// the rest of the tour payload is camelCase as well. Getting only one of them
/// right produced a UI that showed "–" instead of a distance.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
pub enum OptimizerWarning {
    /// The tour is longer than the configured limit.
    ///
    /// It is still returned with every chosen station; the user can remove
    /// stations or raise the limit.
    #[error("the tour is {distance_m:.0} m long, above the {limit_m:.0} m limit")]
    OversizedTour {
        /// Actual distance in metres.
        distance_m: f64,
        /// Configured limit in metres.
        limit_m: f64,
    },
}
