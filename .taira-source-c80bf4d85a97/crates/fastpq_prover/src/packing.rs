//! Byte packing helpers for the FASTPQ trace builder.
//!
//! The FASTPQ AIR packs variable-length keys and values into Goldilocks field
//! elements using 7-byte limbs (little-endian). This module provides helper
//! functions for packing/unpacking the representation so higher-level builders
//! can operate on typed structures instead of manual loops. The helpers are
//! intentionally simple and deterministic so the eventual trace builder can be
//! audited easily.

use core::cmp::min;

/// Goldilocks modulus (2^64 - 2^32 + 1) used by the FASTPQ AIR.
const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

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

impl PackedBytes {
    /// Reconstruct the original byte vector using the stored length to trim
    /// padding introduced during packing.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.limbs.len() * LIMB_BYTES);
        for limb in &self.limbs {
            let chunk = limb.to_le_bytes();
            out.extend_from_slice(&chunk[..LIMB_BYTES]);
        }
        out.truncate(self.length);
        out
    }
}

/// Pack an arbitrary byte slice into 7-byte little-endian limbs.
#[must_use]
pub fn pack_bytes(bytes: &[u8]) -> PackedBytes {
    if bytes.is_empty() {
        return PackedBytes {
            limbs: Vec::new(),
            length: 0,
        };
    }

    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(LIMB_BYTES));
    let mut offset = 0;
    while offset < bytes.len() {
        let remaining = bytes.len() - offset;
        let take = min(remaining, LIMB_BYTES);
        let mut chunk = [0u8; 8];
        chunk[..take].copy_from_slice(&bytes[offset..offset + take]);
        let limb = u64::from_le_bytes(chunk);
        assert!(
            limb < GOLDILOCKS_MODULUS,
            "packed limb {limb:#x} exceeds Goldilocks modulus {GOLDILOCKS_MODULUS:#x}"
        );
        limbs.push(limb);
        offset += take;
    }

    PackedBytes {
        limbs,
        length: bytes.len(),
    }
}

/// Unpack a previously packed limb representation into raw bytes.
#[must_use]
pub fn unpack_bytes(packed: &PackedBytes) -> Vec<u8> {
    packed.to_bytes()
}

#[cfg(test)]
mod tests {
    use core::convert::TryFrom;

    use super::*;

    #[test]
    fn roundtrip_empty() {
        let packed = pack_bytes(&[]);
        assert!(packed.limbs.is_empty());
        assert_eq!(packed.length, 0);
        assert_eq!(unpack_bytes(&packed), Vec::<u8>::new());
    }

    #[test]
    fn roundtrip_various_lengths() {
        for len in 1..=32 {
            let data: Vec<u8> = (0..len)
                .map(|i| u8::try_from(i).expect("index fits u8").wrapping_mul(7))
                .collect();
            let packed = pack_bytes(&data);
            assert_eq!(packed.length, len);
            assert_eq!(unpack_bytes(&packed), data);
        }
    }

    #[test]
    fn padding_is_trimmed() {
        let data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let packed = pack_bytes(&data);
        assert_eq!(packed.limbs.len(), 1);
        assert_eq!(unpack_bytes(&packed), data);
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
            assert!(packed.limbs.iter().all(|&limb| limb < GOLDILOCKS_MODULUS));
        }
    }
}
