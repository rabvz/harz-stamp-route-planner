// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors from routing providers and response parsing.

use thiserror::Error;

/// Reasons a route could not be produced or understood.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum RoutingError {
    /// A provider returned a response that is not valid JSON.
    #[error("routing response is not valid JSON: {message}")]
    InvalidJson {
        /// Parser message.
        message: String,
    },

    /// A provider returned JSON that is valid but not a shape we understand.
    #[error("routing response has an unsupported shape: {message}")]
    UnsupportedShape {
        /// What was expected versus found.
        message: String,
    },

    /// A geometry contained no usable coordinates.
    #[error("routing response contained no coordinates")]
    EmptyGeometry,

    /// A coordinate in a response was out of range or not finite.
    #[error("routing response contains an invalid coordinate: {message}")]
    InvalidCoordinate {
        /// Detail about the rejected coordinate.
        message: String,
    },

    /// A provider reported a failure of its own.
    #[error("routing provider failed: {message}")]
    ProviderFailure {
        /// The provider's message, passed through for the user to see.
        message: String,
    },

    /// The provider was configured with an unusable parameter.
    #[error("routing provider misconfigured: {message}")]
    Configuration {
        /// What is wrong with the configuration.
        message: String,
    },

    /// A cached leg was requested for a key that is not present.
    #[error("no cached route for the requested leg")]
    CacheMiss,
}
