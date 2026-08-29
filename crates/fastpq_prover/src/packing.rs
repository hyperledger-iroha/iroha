//! Byte packing helpers for the FASTPQ trace builder.
//!
//! The FASTPQ AIR packs variable-length keys and values into Goldilocks field elements using 7-byte
//! limbs (little-endian). Seven bytes always fit in a canonical Goldilocks element.
/// Number of bytes stored per packed limb (little-endian).
pub const LIMB_BYTES: usize = 7;
/// Packed representation of an arbitrary byte slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedBytes {
    /// Packed Goldilocks limbs storing the canonical bytes.
    pub limbs: Vec<u64>,
    /// Original uncompressed length in bytes.
    pub length: usize,
}
/// Pack an arbitrary byte slice into 7-byte little-endian limbs.
#[must_use]
pub fn pack_bytes(bytes: &[u8]) -> PackedBytes {
    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(LIMB_BYTES));
    for bytes in bytes.chunks(LIMB_BYTES) {
        let mut chunk = [0u8; 8];
        chunk[..bytes.len()].copy_from_slice(bytes);
        limbs.push(u64::from_le_bytes(chunk));
    }
    PackedBytes {
        limbs,
        length: bytes.len(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::TryFrom;
    #[test]
    fn roundtrip_empty() {
        let packed = pack_bytes(&[]);
        assert!(packed.limbs.is_empty());
        assert_eq!(packed.length, 0);
    }
    #[test]
    fn roundtrip_various_lengths() {
        for len in 1..=32 {
            let data: Vec<u8> = (0..len)
                .map(|i| u8::try_from(i).expect("index fits u8").wrapping_mul(7))
                .collect();
            let packed = pack_bytes(&data);
            assert_eq!(packed.length, len);
            let expected_limbs = data
                .chunks(LIMB_BYTES)
                .map(|chunk| {
                    let mut bytes = [0_u8; 8];
                    bytes[..chunk.len()].copy_from_slice(chunk);
                    u64::from_le_bytes(bytes)
                })
                .collect::<Vec<_>>();
            assert_eq!(packed.limbs, expected_limbs);
        }
    }
    #[test]
    fn padding_is_trimmed() {
        let data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let packed = pack_bytes(&data);
        assert_eq!(packed.limbs.len(), 1);
        assert_eq!(packed.limbs, vec![0xDDCC_BBAA]);
    }
    #[test]
    fn limbs_always_canonical() {
        for len in [0usize, 1, 6, 7, 15, 31, 64, 127, 191] {
            let len_u8 = u8::try_from(len).expect("length fits u8");
            let data: Vec<u8> = (0..len)
                .map(|i| {
                    let seed = u8::try_from(i).expect("index fits u8");
                    seed.wrapping_mul(37).wrapping_add(len_u8).rotate_left(1)
                })
                .collect();
            let packed = pack_bytes(&data);
            assert!(packed.limbs.iter().all(|&limb| limb < crate::FIELD_MODULUS));
        }
    }
}
