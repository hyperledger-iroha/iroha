//! Crate with utilities for implementing smart contract FFI
// This crate relies on the standard library.
#![allow(unsafe_code)]

pub use dbg::*;
pub use getrandom;
pub use iroha_smart_contract_codec::{
    decode_with_length_prefix_from_raw, encode_with_length_prefix,
};
pub use norito::{NoritoDeserialize, NoritoSerialize};

mod dbg;
pub mod log;

/// Registers a custom `getrandom` function that:
///
/// 1. prints `error` message to the log
/// 2. panics with the same message
#[doc(hidden)]
#[macro_export]
macro_rules! register_getrandom_err_callback {
    () => {
        /// Prints a log message with [`error!`] and panics.
        ///
        /// # Panics
        ///
        /// Never panics when compiled with `test`; fills the buffer with a
        /// deterministic pattern instead. In production builds this logs an
        /// error and fills deterministically to preserve peer parity.
        ///
        /// # Errors
        ///
        /// Returns `Ok(())` after populating the destination buffer with a
        /// deterministic, non-cryptographic pattern.
        #[unsafe(no_mangle)]
        unsafe extern "Rust" fn __getrandom_v03_custom(
            dest: *mut u8,
            len: usize,
        ) -> Result<(), $crate::getrandom::Error> {
            const ERROR_MESSAGE: &str =
                "`getrandom()` is not implemented. To provide your custom function \
                 see https://docs.rs/getrandom/0.3/getrandom/#custom-backend. \
                 Be aware that your function must give the same result on different peers at the same execution round,
                 and keep in mind the consequences of purely implemented random function.";

            // we don't support logging in our current IVM test runner implementation
            #[cfg(not(test))]
            $crate::error!(ERROR_MESSAGE);
            #[cfg(test)]
            let _ = ERROR_MESSAGE;
            if dest.is_null() {
                return Err($crate::getrandom::Error::UNSUPPORTED);
            }
            // Fill with a deterministic, non-cryptographic pattern so peers
            // remain in sync even without a true RNG.
            let slice = unsafe { core::slice::from_raw_parts_mut(dest, len) };
            for (idx, byte) in slice.iter_mut().enumerate() {
                // Wrap the index to a single byte to keep the pattern stable for long buffers.
                let offset = u8::try_from(idx).unwrap_or_else(|_| {
                    u8::try_from(idx.wrapping_rem(u8::MAX as usize + 1))
                        .expect("wrapped to u8 range")
                });
                *byte = offset.wrapping_mul(0x5A).wrapping_add(0xA5);
            }
            Ok(())
        }
    };
}

/// Encode the given object and call the given function with the pointer and length of the allocation
///
/// # Warning
///
/// Ownership of the returned allocation is transferred to the caller
///
/// # Safety
///
/// The given function must not take ownership of the pointer argument
#[doc(hidden)]
pub unsafe fn encode_and_execute<T: NoritoSerialize, O>(
    obj: &T,
    fun: unsafe extern "C" fn(*const u8, usize) -> O,
) -> O {
    // NOTE: It's imperative that encoded object is stored on the heap
    // because heap corresponds to linear memory when compiled for the IVM
    let bytes = norito::to_bytes(obj).expect("Norito serialization must succeed");

    unsafe { fun(bytes.as_ptr(), bytes.len()) }
}

#[cfg(test)]
register_getrandom_err_callback!();

#[cfg(test)]
mod tests {
    // The Norito derive macros may reference cfg(feature = "packed-struct")
    // internally. Our workspace enables strict check-cfg, so allow the
    // unexpected-cfgs lint in this test module only.
    #![allow(unexpected_cfgs)]
    use core::{convert::TryInto, ptr};

    // Silence unexpected-cfgs emitted from norito derives inside tests where
    // the workspace enables strict cfg checking for feature names.
    #[allow(unexpected_cfgs)]
    #[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
    struct Dummy {
        a: u32,
        b: bool,
    }

    #[test]
    fn encode_decode_roundtrip() {
        let value = Dummy { a: 5, b: true };
        let bytes = super::encode_with_length_prefix(&value);
        let len_size_bytes = core::mem::size_of::<usize>();
        assert_eq!(
            bytes.len(),
            usize::from_le_bytes(bytes[..len_size_bytes].try_into().unwrap())
        );

        // SAFETY: `decode_with_length_prefix_from_raw` takes ownership of the pointer.
        let ptr = Box::into_raw(bytes) as *const u8;
        let decoded = unsafe { super::decode_with_length_prefix_from_raw::<Dummy>(ptr) };
        assert_eq!(decoded, value);
    }

    #[test]
    fn getrandom_callback_fills_deterministically() {
        // Safety: destination buffer is valid for writes.
        let mut buf = [0u8; 8];
        unsafe {
            super::__getrandom_v03_custom(buf.as_mut_ptr(), buf.len()).expect("callback ok");
        }
        assert_eq!(
            buf,
            [0xA5, 0xFF, 0x59, 0x53, 0xAD, 0x07, 0x61, 0x5B],
            "deterministic pattern must remain stable"
        );

        // Null pointer should be rejected.
        let err = unsafe { super::__getrandom_v03_custom(ptr::null_mut(), 1) }
            .expect_err("expected error on null dest");
        assert_eq!(err, crate::getrandom::Error::UNSUPPORTED);
    }
}
