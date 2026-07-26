use std::{format, string::String, vec::Vec};

use derive_more::Display;

/// Variable length unsigned int. [ref](https://github.com/multiformats/unsigned-varint)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VarUint {
    /// Contains validated varuint number
    payload: Vec<u8>,
}

/// Error which occurs when converting to/from `VarUint`
#[derive(Debug, Clone, Display)]
pub struct ConvertError {
    reason: String,
}

impl ConvertError {
    const fn new(reason: String) -> Self {
        Self { reason }
    }
}

impl std::error::Error for ConvertError {}

macro_rules! try_from_var_uint(
    { $( $ty:ty ),* } => {
        $(
            #[allow(trivial_numeric_casts)]
            impl TryFrom<VarUint> for $ty {
                type Error = ConvertError;

                fn try_from(source: VarUint) -> Result<Self, Self::Error> {
                    let VarUint { payload } = source;
                    let overflow = || {
                        Self::Error::new(String::from(
                            concat!("Number too large for ", stringify!($ty)),
                        ))
                    };

                    let mut number = 0_u128;
                    for (idx, byte) in payload.into_iter().enumerate() {
                        let offset = idx.checked_mul(7).ok_or_else(overflow)?;
                        let offset = u32::try_from(offset).map_err(|_| overflow())?;
                        let part = u128::from(byte & 0b0111_1111);
                        let Some(max_part) = u128::MAX.checked_shr(offset) else {
                            if part == 0 {
                                continue;
                            }
                            return Err(overflow());
                        };
                        if part > max_part {
                            return Err(overflow());
                        }
                        let shifted = part << offset;
                        number = number.checked_add(shifted).ok_or_else(overflow)?;
                    }

                    Self::try_from(number).map_err(|_| overflow())
                }
            }
        )*
    }
);

try_from_var_uint!(u8, u16, u32, u64, u128);

impl From<VarUint> for Vec<u8> {
    fn from(int: VarUint) -> Self {
        int.payload
    }
}

impl AsRef<[u8]> for VarUint {
    fn as_ref(&self) -> &[u8] {
        self.payload.as_ref()
    }
}

macro_rules! from_uint(
    { $( $ty:ty ),* } => {
        $(
            #[allow(trivial_numeric_casts)]
            impl From<$ty> for VarUint {
                fn from(n: $ty) -> Self {
                    if n == 0 {
                        return Self { payload: vec![0] };
                    }
                    let zeros = n.leading_zeros();
                    let end = core::mem::size_of::<$ty>() * 8 - zeros as usize;
                    let chunk_count = end.div_ceil(7);

                    #[allow(clippy::cast_possible_truncation)]
                    let payload = (0..chunk_count)
                        .map(|idx| {
                            let offset = idx * 7;
                            let byte = ((n >> offset) as u8) & 0b0111_1111;
                            if idx + 1 == chunk_count {
                                byte
                            } else {
                                byte | 0b1000_0000
                            }
                        })
                        .collect::<Vec<_>>();

                    Self { payload }
                }
            }
        )*
    }
);

from_uint!(u8, u16, u32, u64, u128);

impl VarUint {
    /// Construct `VarUint` from canonical unsigned-varint bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload is malformed or uses a non-canonical encoding.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, ConvertError> {
        let idx = bytes
            .as_ref()
            .iter()
            .enumerate()
            .find(|&(_, &byte)| (byte & 0b1000_0000) == 0)
            .ok_or_else(|| {
                ConvertError::new(String::from(
                    "Failed to find last byte(byte smaller than 128)",
                ))
            })?
            .0;
        let (payload, empty) = bytes.as_ref().split_at(idx + 1);
        let payload = payload.to_vec();

        if payload.len() > 1 && payload.last().copied().unwrap_or(0).trailing_zeros() >= 7 {
            return Err(ConvertError::new(String::from(
                "Non-canonical varuint encoding",
            )));
        }

        if empty.is_empty() {
            return Ok(Self { payload });
        }

        Err(ConvertError::new(format!(
            "{empty:?}: found these bytes following last byte"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn test_basic_into() {
        let n = 0x4000_u64;
        let varuint: VarUint = n.into();
        let vec: Vec<_> = varuint.into();
        let should_be = vec![0b1000_0000, 0b1000_0000, 0b0000_0001];
        assert_eq!(vec, should_be);
    }

    #[test]
    fn test_basic_from() {
        let n_should: u64 = VarUint::new([0b1000_0000, 0b1000_0000, 0b0000_0001])
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(0x4000_u64, n_should);
    }

    #[test]
    fn test_basic_into_from() {
        let n = 0x4000_u64;
        let varuint: VarUint = n.into();
        let n_new: u64 = varuint.try_into().unwrap();
        assert_eq!(n, n_new);
    }

    #[test]
    fn test_multihash() {
        let n = 0xed;
        let varuint: VarUint = n.into();
        let vec: Vec<_> = varuint.clone().into();
        let n_new: u64 = varuint.try_into().unwrap();
        assert_eq!(n, n_new);
        assert_eq!(vec, vec![0xed, 0x01]);
    }

    #[test]
    fn test_new_returns_err_on_extra_bytes() {
        assert!(VarUint::new([0b1000_0000, 0b0000_0001, 0xFF]).is_err());
    }

    #[test]
    fn varuint_from_zero_encodes_single_byte() {
        let varuint: VarUint = 0u8.into();
        let bytes: Vec<u8> = varuint.into();
        assert_eq!(bytes, vec![0x00]);
    }

    #[test]
    fn varuint_zero_roundtrip() {
        let varuint = VarUint::new([0x00]).unwrap();
        let value: u64 = varuint.try_into().unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn varuint_new_rejects_non_canonical_zero() {
        assert!(VarUint::new([0x80, 0x00]).is_err());
    }

    #[test]
    fn varuint_new_rejects_non_canonical_one() {
        assert!(VarUint::new([0x81, 0x00]).is_err());
    }

    #[test]
    fn u8_try_from_varuint_fails_on_overflow() {
        let value = u16::from(u8::MAX) + 1;
        let varuint: VarUint = value.into();
        assert!(<u8 as core::convert::TryFrom<VarUint>>::try_from(varuint).is_err());
    }

    #[test]
    fn max_values_roundtrip_for_all_integer_widths() {
        let value: u8 = VarUint::from(u8::MAX).try_into().unwrap();
        assert_eq!(value, u8::MAX);

        let value: u16 = VarUint::from(u16::MAX).try_into().unwrap();
        assert_eq!(value, u16::MAX);

        let value: u32 = VarUint::from(u32::MAX).try_into().unwrap();
        assert_eq!(value, u32::MAX);

        let value: u64 = VarUint::from(u64::MAX).try_into().unwrap();
        assert_eq!(value, u64::MAX);

        let value: u128 = VarUint::from(u128::MAX).try_into().unwrap();
        assert_eq!(value, u128::MAX);
    }

    #[test]
    fn varuint_decode_rejects_values_above_u128_without_panicking() {
        let mut bytes = vec![0x80; 19];
        bytes.push(0x01);
        let varuint = VarUint::new(bytes).unwrap();

        assert!(u128::try_from(varuint).is_err());
    }

    #[test]
    fn varuint_decode_rejects_high_bits_above_u128_without_truncating() {
        let mut bytes = vec![0xFF; 18];
        bytes.push(0x04);
        let varuint = VarUint::new(bytes).unwrap();

        assert!(u128::try_from(varuint).is_err());
    }

    #[test]
    fn varuint_from_integer_types_sets_only_required_continuation_bits() {
        for bytes in [
            VarUint::from(u8::MAX).payload,
            VarUint::from(u16::MAX).payload,
            VarUint::from(u32::MAX).payload,
            VarUint::from(u64::MAX).payload,
            VarUint::from(u128::MAX).payload,
        ] {
            assert!(!bytes.is_empty());
            assert_eq!(
                bytes[bytes.len() - 1] & 0b1000_0000,
                0,
                "final byte must not carry a continuation bit",
            );
            for byte in &bytes[..bytes.len() - 1] {
                assert_ne!(
                    byte & 0b1000_0000,
                    0,
                    "non-final byte must carry a continuation bit",
                );
            }
        }
    }
}
