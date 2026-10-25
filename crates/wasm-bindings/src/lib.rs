// SPDX-License-Identifier: MIT OR Apache-2.0

//! The WebAssembly boundary.
//!
//! # Why a JSON ABI instead of `wasm-bindgen`
//!
//! The browser needs exactly one thing from Rust: a pure function from a JSON
//! request to a JSON response. Generating bindings and a JavaScript loader for
//! that would add a second toolchain (`wasm-bindgen-cli`) whose version must
//! match the crate, for no gain in capability.
//!
//! A four-function C ABI — allocate, free, invoke, version — keeps the build
//! reproducible with `cargo` alone, produces a small module, and makes the
//! whole API callable from any JavaScript runtime without a loader. The typed
//! façade lives in [`api`], which is ordinary Rust and therefore testable
//! natively without a browser.
//!
//! # Safety
//!
//! The exported functions take raw pointers from JavaScript. Every one of them
//! validates its arguments and refuses to read or write outside the buffer it
//! was handed. Panics are caught at the boundary and converted into an error
//! response, because unwinding across the FFI boundary would abort the module.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

pub mod abi;
pub mod api;

pub use api::{ApiError, Response, handle};
