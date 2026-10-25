// SPDX-License-Identifier: MIT OR Apache-2.0

//! The raw WebAssembly ABI.
//!
//! Four exported functions, all of them deliberately small:
//!
//! | export | signature | purpose |
//! | --- | --- | --- |
//! | `hsr_alloc` | `(len: usize) -> *mut u8` | allocate a request buffer |
//! | `hsr_free` | `(ptr: *mut u8, len: usize)` | release a buffer |
//! | `hsr_invoke` | `(ptr: *const u8, len: usize) -> u64` | handle one request |
//! | `hsr_abi_version` | `() -> u32` | ABI version, for a clear mismatch error |
//!
//! `hsr_invoke` returns the pointer and length of the response buffer packed
//! into one 64-bit value; see [`pack`]. Packing them avoids a second call and
//! any hidden global state. This encoding relies on 32-bit pointers, so the
//! exports are compiled for `wasm32` only; the logic they wrap lives in
//! [`crate::api`] and is tested natively.
//!
//! # Allocation contract
//!
//! Both directions use one explicit layout, `len` bytes with alignment 1.
//! Allocation and release therefore agree by construction. An earlier revision
//! handed out `String`/`Vec` buffers and reconstructed them with a guessed
//! capacity; when the guess differed, the deallocation aborted the module with
//! an `unreachable` trap that surfaced in the browser as a failed start. That is
//! why `scripts/smoke-test.mjs` exercises the real artefact in a real engine:
//! `cargo test` cannot see it.

/// Version of this ABI. Bump only on an incompatible change.
pub const ABI_VERSION: u32 = 1;

/// Packs a pointer and a length into the single 64-bit return value.
///
/// The pointer occupies the high 32 bits and the length the low 32 bits, which
/// is what the JavaScript loader decodes. Exposed and tested here so the two
/// sides cannot drift apart unnoticed.
#[must_use]
pub const fn pack(pointer: u32, length: u32) -> u64 {
    (pointer as u64) << 32 | length as u64
}

/// Inverse of [`pack`].
#[must_use]
pub const fn unpack(packed: u64) -> (u32, u32) {
    (
        ((packed >> 32) & 0xffff_ffff) as u32,
        (packed & 0xffff_ffff) as u32,
    )
}

#[cfg(target_arch = "wasm32")]
mod exports {
    use super::{ABI_VERSION, pack};
    use crate::api;
    use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    /// The single layout used for every buffer crossing the boundary.
    fn layout(length: usize) -> Option<Layout> {
        Layout::from_size_align(length, 1).ok()
    }

    /// Allocates `len` zeroed bytes for the caller to fill.
    ///
    /// Returns null when the length is zero or the allocation fails; the caller
    /// must treat null as an error and not call `hsr_invoke` with it.
    #[unsafe(no_mangle)]
    pub extern "C" fn hsr_alloc(len: usize) -> *mut u8 {
        if len == 0 {
            return std::ptr::null_mut();
        }
        let Some(layout) = layout(len) else {
            return std::ptr::null_mut();
        };
        // SAFETY: `layout` has non-zero size, which is the documented
        // requirement for `alloc_zeroed`.
        unsafe { alloc_zeroed(layout) }
    }

    /// Releases a buffer returned by [`hsr_alloc`] or `hsr_invoke`.
    ///
    /// # Safety
    ///
    /// `pointer` must have come from this module and `len` must be the exact
    /// length it was allocated with. A null pointer or zero length is ignored,
    /// which makes a redundant release from a confused caller harmless.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn hsr_free(pointer: *mut u8, len: usize) {
        if pointer.is_null() || len == 0 {
            return;
        }
        let Some(layout) = layout(len) else {
            return;
        };
        // SAFETY: the caller guarantees that `pointer` was allocated with this
        // exact layout and has not been released yet.
        unsafe { dealloc(pointer, layout) };
    }

    /// Handles one UTF-8 JSON request and returns a packed response buffer.
    ///
    /// # Safety
    ///
    /// `pointer` must reference `len` readable bytes inside this module's
    /// memory, as produced by [`hsr_alloc`].
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn hsr_invoke(pointer: *const u8, len: usize) -> u64 {
        if pointer.is_null() || len == 0 {
            return 0;
        }
        // SAFETY: the caller guarantees a readable buffer of `len` bytes.
        let bytes = unsafe { std::slice::from_raw_parts(pointer, len) };
        let request = String::from_utf8_lossy(bytes);

        // A panic must never cross the boundary: it would abort the module and
        // leave the page with no way to report what happened. With
        // `panic = "abort"` in the release profile this guard only helps debug
        // builds; the release artefact is covered by the browser smoke test.
        let response = catch_unwind(AssertUnwindSafe(|| api::handle(&request))).unwrap_or_else(
            |_| {
                r#"{"ok":false,"error":{"code":"internal","message":"the planner crashed; please report this with your selection"}}"#
                    .to_string()
            },
        );

        let bytes = response.as_bytes();
        let Ok(length) = u32::try_from(bytes.len()) else {
            return 0;
        };
        if length == 0 {
            return 0;
        }
        let Some(layout) = layout(bytes.len()) else {
            return 0;
        };
        // SAFETY: the layout has non-zero size, so the allocation is valid.
        let output = unsafe { alloc(layout) };
        if output.is_null() {
            return 0;
        }
        // SAFETY: `output` points to `bytes.len()` writable bytes and the
        // source is a distinct, valid slice of the same length.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, bytes.len());
        }
        pack(output as u32, length)
    }

    /// Returns the ABI version so the loader can refuse an incompatible module.
    #[unsafe(no_mangle)]
    pub extern "C" fn hsr_abi_version() -> u32 {
        ABI_VERSION
    }
}

#[cfg(target_arch = "wasm32")]
pub use exports::{hsr_abi_version, hsr_alloc, hsr_free, hsr_invoke};

#[cfg(test)]
mod tests {
    use super::{ABI_VERSION, pack, unpack};

    #[test]
    fn packing_roundtrips() {
        for (pointer, length) in [
            (0_u32, 0_u32),
            (1, 1),
            (0xdead_beef, 0x0bad_f00d),
            (u32::MAX, 1),
        ] {
            let packed = pack(pointer, length);
            assert_eq!(
                unpack(packed),
                (pointer, length),
                "failed for {pointer}/{length}"
            );
        }
    }

    #[test]
    fn pointer_and_length_do_not_overlap() {
        // A bug here would let the loader read a length where it expects a
        // pointer, so the halves must stay independent at the extremes.
        let high = pack(u32::MAX, 0);
        let low = pack(0, u32::MAX);
        assert_eq!(high >> 32, u64::from(u32::MAX));
        assert_eq!(low & 0xffff_ffff, u64::from(u32::MAX));
        assert_eq!(low >> 32, 0);
    }

    #[test]
    fn a_zero_word_means_no_response() {
        // `hsr_invoke` returns 0 on any failure; the loader relies on that.
        assert_eq!(unpack(0), (0, 0));
    }

    #[test]
    fn abi_version_is_the_documented_value() {
        // The JavaScript loader in web/js/wasm.js hard-codes the same number.
        // Changing it is an ABI break and must be a deliberate edit in both.
        assert_eq!(ABI_VERSION, 1);
    }
}
