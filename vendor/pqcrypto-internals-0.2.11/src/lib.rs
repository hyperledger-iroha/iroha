#![no_std]

use core::{ffi::c_int, slice};

/// Get random bytes; exposed for PQClean implementations.
///
/// # Safety
/// Assumes inputs are valid and may panic over FFI boundary if rng failed.
///
/// # Example
/// ```rust
/// use pqcrypto_internals::*;
/// let mut buf = [0u8;10];
/// unsafe {
///   PQCRYPTO_RUST_randombytes(buf.as_mut_ptr(), buf.len());
/// }
/// ```
#[no_mangle]
pub unsafe extern "C" fn PQCRYPTO_RUST_randombytes(buf: *mut u8, len: usize) -> c_int {
    let buf = slice::from_raw_parts_mut(buf, len);
    getrandom::fill(buf).expect("RNG Failed");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn randombytes_preserves_c_abi_and_exact_write_bounds() {
        let randombytes: unsafe extern "C" fn(*mut u8, usize) -> c_int = PQCRYPTO_RUST_randombytes;
        let mut bytes = [0_u8; 66];
        bytes[0] = 0xa5;
        bytes[65] = 0x5a;
        // SAFETY: the middle 64 bytes are writable and exclusively borrowed.
        let status = unsafe { randombytes(bytes.as_mut_ptr().add(1), 64) };
        assert_eq!(status, 0);
        assert_eq!((bytes[0], bytes[65]), (0xa5, 0x5a));
        // A real entropy fill must not silently leave the initial all-zero buffer.
        assert!(bytes[1..65].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn randombytes_accepts_an_empty_valid_buffer() {
        let mut sentinel = 0xa5;
        // SAFETY: the pointer is non-null and aligned; the requested slice is empty.
        assert_eq!(unsafe { PQCRYPTO_RUST_randombytes(&mut sentinel, 0) }, 0);
        assert_eq!(sentinel, 0xa5);
    }
}
