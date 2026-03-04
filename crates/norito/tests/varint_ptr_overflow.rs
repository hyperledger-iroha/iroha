//! Ensure `try_read_len_ptr_unchecked` rejects overlong varints (>10 bytes).

use norito::core::{self, header_flags};

#[test]
fn try_read_len_ptr_unchecked_overflow_errors() {
    // Enable compact-len so varint reader is active
    let _fg = core::DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

    // Construct an overlong varint: 11 bytes with continuation bits set
    let bytes = [0xFFu8; 11];
    unsafe {
        core::set_payload_ctx(bytes.as_slice());
        let res = core::try_read_len_ptr_unchecked(bytes.as_ptr());
        core::clear_payload_ctx();
        assert!(matches!(res, Err(norito::Error::LengthMismatch)));
    }
}
