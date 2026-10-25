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

/// A constraint the plan could not satisfy.
///
/// Warnings are part of the result rather than errors: dropping chosen stations
/// silently would be worse, so the plan is returned and the problem is stated.
///
/// `rename_all` renames the variant tags, `rename_all_fields` the payload
/// fields. Both are camelCase because this enum crosses into JavaScript, where
/// the rest of the plan payload is camelCase as well. Getting only one of them
/// right produced a UI that showed "–" instead of a distance.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
pub enum OptimizerWarning {
    /// The plan needs more days than the user allowed.
    #[error("plan needs {required} days but only {allowed} were allowed")]
    DayLimitExceeded {
        /// Days the plan actually uses.
        required: usize,
        /// Days the user allowed.
        allowed: usize,
    },

    /// A single station cannot be reached within the daily distance limit.
    #[error(
        "day {day} is {distance_m:.0} m long, above the {limit_m:.0} m limit, \
         because station {stamp} cannot be reached in a shorter round trip"
    )]
    OversizedDay {
        /// One-based day number.
        day: usize,
        /// Actual distance in metres.
        distance_m: f64,
        /// Configured limit in metres.
        limit_m: f64,
        /// The station responsible.
        stamp: stamp_core::StampId,
    },
}
