//! Shared codec helpers bridging smart contracts and the host runtime.
//!
//! These helpers provide a stable length-prefixed envelope that both the IVM
//! guest and the Rust host understand. They are used by the executor, smart
//! contracts, and utils crates to exchange Norito-encoded payloads without
//! duplicating pointer-handling logic.
#![allow(unsafe_code)]

use core::{convert::TryInto, mem, ops::RangeFrom};

use norito::{NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes};

/// Encode the given value with a `usize` length prefix.
///
/// The returned allocation owns the encoded bytes and is suitable for passing
/// across the IVM FFI boundary where the callee reads the prefix to determine
/// how many bytes follow.
#[must_use]
pub fn encode_with_length_prefix<T: NoritoSerialize>(value: &T) -> Box<[u8]> {
    let len_size_bytes = mem::size_of::<usize>();
    let encoded = to_bytes(value).expect("Norito serialization must succeed");
    let total_len = len_size_bytes
        .checked_add(encoded.len())
        .expect("encoded payload length must fit in usize");

    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(&total_len.to_le_bytes());
    bytes.extend_from_slice(&encoded);
    bytes.into_boxed_slice()
}

/// Consume a pointer previously produced by [`encode_with_length_prefix`].
///
/// # Safety
///
/// - `ptr` must point to a heap allocation created by [`encode_with_length_prefix`].
/// - The caller must not use `ptr` after calling this function.
pub unsafe fn decode_with_length_prefix_from_raw<T>(ptr: *const u8) -> T
where
    for<'de> T: NoritoDeserialize<'de> + norito::core::DecodeFromSlice<'de>,
{
    let len_size_bytes = mem::size_of::<usize>();
    let len_bytes = unsafe { core::slice::from_raw_parts(ptr, len_size_bytes) };
    let len = usize::from_le_bytes(
        len_bytes
            .try_into()
            .expect("length prefix must match usize width"),
    );

    unsafe { decode_from_raw_in_range(ptr, len, len_size_bytes..) }
}

unsafe fn decode_from_raw_in_range<T>(ptr: *const u8, len: usize, range: RangeFrom<usize>) -> T
where
    for<'de> T: NoritoDeserialize<'de> + norito::core::DecodeFromSlice<'de>,
{
    let bytes = unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr.cast_mut(), len)) };
    let payload = &bytes[range];

    #[allow(clippy::expect_fun_call)]
    decode_from_bytes::<T>(payload).expect(
        format!(
            "Decoding of {} failed. This is a bug",
            core::any::type_name::<T>()
        )
        .as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
    struct Dummy {
        a: u32,
        b: bool,
    }

    #[test]
    fn roundtrip_length_prefixed_payload() {
        let value = Dummy { a: 5, b: true };
        let bytes = encode_with_length_prefix(&value);
        let len_size_bytes = mem::size_of::<usize>();
        assert_eq!(
            bytes.len(),
            usize::from_le_bytes(bytes[..len_size_bytes].try_into().unwrap())
        );

        let ptr = Box::into_raw(bytes) as *const u8;
        let decoded = unsafe { decode_with_length_prefix_from_raw::<Dummy>(ptr) };
        assert_eq!(decoded, value);
    }
}
