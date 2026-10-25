// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors produced by the stamp model.

use thiserror::Error;

/// Reasons a stamp, catalogue or selection could not be constructed.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum StampError {
    /// A station number was zero or above [`crate::MAX_STAMP_NUMBER`].
    #[error("stamp number {value} is outside 1..={max}")]
    NumberOutOfRange {
        /// The rejected number.
        value: u16,
        /// The inclusive upper bound.
        max: u16,
    },

    /// The catalogue JSON could not be parsed.
    #[error("catalogue is not valid JSON: {message}")]
    InvalidJson {
        /// Parser message.
        message: String,
    },

    /// The catalogue declared an unsupported schema version.
    #[error("unsupported catalogue schema version {found}, expected {expected}")]
    UnsupportedSchema {
        /// Version found in the file.
        found: u32,
        /// Version this build understands.
        expected: u32,
    },

    /// The catalogue contained no stations at all.
    #[error("catalogue contains no stamps")]
    EmptyCatalog,

    /// Two stations shared the same number.
    #[error("duplicate stamp number {number}")]
    DuplicateNumber {
        /// The number that appeared twice.
        number: u16,
    },

    /// A station code did not follow the `HWNnnn` convention.
    #[error("stamp {number} has malformed code {code:?}, expected HWN{number:0>3}")]
    MalformedCode {
        /// The numeric station number.
        number: u16,
        /// The code as found in the file.
        code: String,
    },

    /// A station had an empty name or location.
    #[error("stamp {number} has an empty {field}")]
    EmptyField {
        /// The numeric station number.
        number: u16,
        /// Which field was empty.
        field: &'static str,
    },

    /// A selection referenced a station that the catalogue does not contain.
    #[error("stamp {number} is not part of catalogue {dataset}")]
    UnknownStamp {
        /// The unknown station number.
        number: u16,
        /// Identifier of the catalogue that was consulted.
        dataset: String,
    },

    /// A coordinate from the catalogue was rejected by the geometry layer.
    #[error("stamp {number} has an invalid position: {source}")]
    InvalidPosition {
        /// The numeric station number.
        number: u16,
        /// The underlying geometry error.
        source: geo_engine::GeoError,
    },
}
