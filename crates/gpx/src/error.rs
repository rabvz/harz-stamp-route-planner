// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors from GPX parsing and writing.

use thiserror::Error;

/// Reasons a GPX document could not be read or written.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum GpxError {
    /// The XML itself was malformed.
    #[error("invalid XML at position {position}: {message}")]
    InvalidXml {
        /// What the XML parser reported.
        message: String,
        /// Byte position in the input, when known.
        position: u64,
    },

    /// The document was well-formed XML but not a GPX document.
    #[error("not a GPX document: {message}")]
    NotGpx {
        /// What was expected versus found.
        message: String,
    },

    /// A required element or attribute was missing.
    #[error("missing required {what} in {context}")]
    MissingElement {
        /// The element or attribute that was absent.
        what: &'static str,
        /// Where it was expected.
        context: &'static str,
    },

    /// A coordinate could not be parsed or was out of range.
    #[error("invalid coordinate {value:?} in {context}")]
    InvalidCoordinate {
        /// The raw value found.
        value: String,
        /// Where it appeared.
        context: &'static str,
    },

    /// A numeric field such as elevation was not a number.
    #[error("invalid number {value:?} in {context}")]
    InvalidNumber {
        /// The raw value found.
        value: String,
        /// Where it appeared.
        context: &'static str,
    },

    /// A waypoint had an empty name.
    #[error("waypoint at index {index} has no name")]
    UnnamedWaypoint {
        /// Zero-based index among parsed waypoints.
        index: usize,
    },

    /// A document had no content worth exporting or importing.
    #[error("document contains neither waypoints nor track points")]
    EmptyDocument,
}
