//! Bounded variable-length signed integer with two's-complement encoding.
//!
//! The type wraps `num_bigint::BigInt`, enforces a 512-bit cap, and provides
//! Norito and JSON codecs that use a length-prefixed two's-complement byte
//! representation. Small values stay compact; larger values are allowed until
//! the hard limit is reached.
//!
//! Norito encoding: a little-endian `u32` byte length (not compact) followed by
//! that many little-endian two's-complement bytes. A length of `0` represents
//! zero. When used by [`crate::numeric::Numeric`], this type stores the mantissa
//! only; the decimal scale is carried separately in `Numeric`.

use core::fmt;
use std::io::Write;

use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::{
    Archived, Error as NoritoError, NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, DecodeFromSlice},
    json::{self, FastJsonWrite, JsonDeserialize},
};
use num_bigint::BigInt as InnerBigInt;
use num_traits::{One, Signed, Zero};

/// Maximum number of bits representable by [`BigInt`].
pub const MAX_BITS: usize = 512;
const MAX_BYTES: usize = MAX_BITS / 8;

/// Errors returned by [`BigInt`] operations.
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error, PartialEq, Eq)]
pub enum BigIntError {
    /// Value exceeds configured bit cap
    Overflow,
    /// Division by zero
    DivisionByZero,
}

/// Bounded signed integer with adaptive width (up to 512 bits).
///
/// This is a raw integer. [`crate::numeric::Numeric`] uses it as a mantissa
/// alongside a separate scale value.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct BigInt {
    inner: InnerBigInt,
}

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl BigInt {
    /// Zero.
    pub fn zero() -> Self {
        Self {
            inner: InnerBigInt::zero(),
        }
    }

    /// One.
    pub fn one() -> Self {
        Self {
            inner: InnerBigInt::one(),
        }
    }

    /// Returns `true` when the value is zero.
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Returns `true` when the value is negative.
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Bit length of the minimal two's-complement representation.
    pub fn bit_len(&self) -> usize {
        let bytes = self.inner.to_signed_bytes_le();
        if bytes.is_empty() {
            0
        } else {
            (bytes.len() - 1) * 8 + 8 - bytes.last().unwrap().leading_zeros() as usize
        }
    }

    /// Compute 10^exp with bound checking.
    pub fn pow10(exp: u32) -> Option<Self> {
        let val = InnerBigInt::from(10u8).pow(exp);
        BigInt::from_inner(val).ok()
    }

    /// Construct from a signed 128-bit value.
    pub fn from_i128(value: i128) -> Self {
        Self::from_inner(InnerBigInt::from(value)).expect("i128 always fits")
    }

    /// Attempt to construct from a little-endian two's-complement byte slice.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the decoded value exceeds
    /// [`MAX_BITS`].
    pub fn from_twos_bytes(bytes: &[u8]) -> Result<Self, BigIntError> {
        let inner = InnerBigInt::from_signed_bytes_le(bytes);
        Self::from_inner(inner)
    }

    /// Emit minimal little-endian two's-complement byte representation.
    pub fn to_twos_bytes(&self) -> Vec<u8> {
        self.inner.to_signed_bytes_le()
    }

    /// Checked addition.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the sum exceeds [`MAX_BITS`].
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, BigIntError> {
        Self::from_inner(&self.inner + &rhs.inner)
    }

    /// Checked subtraction.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the difference exceeds
    /// [`MAX_BITS`].
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, BigIntError> {
        Self::from_inner(&self.inner - &rhs.inner)
    }

    /// Checked multiplication.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the product exceeds [`MAX_BITS`].
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self, BigIntError> {
        Self::from_inner(&self.inner * &rhs.inner)
    }

    /// Checked division returning `(quotient, remainder)`.
    ///
    /// # Errors
    /// Returns [`BigIntError::DivisionByZero`] if `rhs` is zero or
    /// [`BigIntError::Overflow`] if either result exceeds [`MAX_BITS`].
    pub fn checked_div_rem(&self, rhs: &Self) -> Result<(Self, Self), BigIntError> {
        if rhs.is_zero() {
            return Err(BigIntError::DivisionByZero);
        }
        let q = &self.inner / &rhs.inner;
        let r = &self.inner % &rhs.inner;
        Ok((Self::from_inner(q)?, Self::from_inner(r)?))
    }

    /// Absolute value.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self {
            inner: self.inner.abs(),
        }
    }

    /// Negation.
    #[must_use]
    pub fn neg(&self) -> Self {
        Self {
            inner: -&self.inner,
        }
    }

    fn from_inner(inner: InnerBigInt) -> Result<Self, BigIntError> {
        let bytes = inner.to_signed_bytes_le();
        if bytes.len() > MAX_BYTES {
            return Err(BigIntError::Overflow);
        }
        Ok(Self { inner })
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl NoritoSerialize for BigInt {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), NoritoError> {
        let bytes = self.to_twos_bytes();
        let len: u32 = bytes
            .len()
            .try_into()
            .map_err(|_| NoritoError::Message("length overflow".into()))?;
        let encoded_len = norito::codec::Encode::encode(&len);
        writer
            .write_all(&encoded_len)
            .map_err(|e| NoritoError::Message(e.to_string()))?;
        writer
            .write_all(&bytes)
            .map_err(|e| NoritoError::Message(e.to_string()))
    }
}

impl<'a> NoritoDeserialize<'a> for BigInt {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        let slice = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast())
            .expect("payload slice");
        let (value, _) =
            <BigInt as DecodeFromSlice>::decode_from_slice(slice).expect("deserialize bigint");
        value
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, NoritoError> {
        let slice = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast())
            .map_err(|e| NoritoError::Message(e.to_string()))?;
        let (value, _) = <BigInt as DecodeFromSlice>::decode_from_slice(slice)
            .map_err(|e| NoritoError::Message(e.to_string()))?;
        Ok(value)
    }
}

impl FastJsonWrite for BigInt {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for BigInt {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<BigInt>()
            .map_err(|err| json::Error::InvalidField {
                field: "bigint".into(),
                message: format!("invalid bigint `{value}`: {err}"),
            })
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl TypeId for BigInt {
    fn id() -> Ident {
        "BigInt".to_string()
    }
}

impl IntoSchema for BigInt {
    fn type_name() -> Ident {
        "BigInt".to_string()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        if !metamap.contains_key::<Self>() {
            metamap.insert::<Self>(Metadata::Struct(iroha_schema::NamedFieldsMeta {
                declarations: Vec::new(),
            }));
        }
    }
}

impl From<i128> for BigInt {
    fn from(value: i128) -> Self {
        BigInt::from_i128(value)
    }
}

impl From<u128> for BigInt {
    fn from(value: u128) -> Self {
        BigInt::from_inner(InnerBigInt::from(value)).expect("u128 fits within MAX_BITS")
    }
}

impl From<u64> for BigInt {
    fn from(value: u64) -> Self {
        BigInt::from_inner(InnerBigInt::from(value)).expect("u64 fits within MAX_BITS")
    }
}

impl From<u32> for BigInt {
    fn from(value: u32) -> Self {
        BigInt::from_inner(InnerBigInt::from(value)).expect("u32 fits within MAX_BITS")
    }
}

impl From<i64> for BigInt {
    fn from(value: i64) -> Self {
        BigInt::from_inner(InnerBigInt::from(value)).expect("i64 fits within MAX_BITS")
    }
}

impl From<i32> for BigInt {
    fn from(value: i32) -> Self {
        BigInt::from_inner(InnerBigInt::from(value)).expect("i32 fits within MAX_BITS")
    }
}

impl core::str::FromStr for BigInt {
    type Err = BigIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner: InnerBigInt = s.parse().map_err(|_| BigIntError::Overflow)?;
        Self::from_inner(inner)
    }
}

impl<'a> DecodeFromSlice<'a> for BigInt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
        let (len_u32, used_len) = <u32 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let len_usize: usize = len_u32
            .try_into()
            .map_err(|_| ncore::Error::Message("length overflow".into()))?;
        let end = used_len
            .checked_add(len_usize)
            .ok_or_else(|| ncore::Error::Message("length overflow".into()))?;
        if end > bytes.len() {
            return Err(ncore::Error::Message("buffer too short".into()));
        }
        let payload = &bytes[used_len..end];
        let value = BigInt::from_twos_bytes(payload)
            .map_err(|_| ncore::Error::Message("invalid bigint".into()))?;
        Ok((value, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_twos_bytes_positive() {
        let values = [0i128, 1, 42, i128::from(u64::MAX), i128::MAX];
        for val in values {
            let bigint = BigInt::from_i128(val);
            let bytes = bigint.to_twos_bytes();
            let decoded = BigInt::from_twos_bytes(&bytes).expect("decode");
            assert_eq!(bigint, decoded);
        }
    }

    #[test]
    fn roundtrip_twos_bytes_negative() {
        let values = [-1i128, -2, -42, -i128::from(u64::MAX)];
        for val in values {
            let bigint = BigInt::from_i128(val);
            let bytes = bigint.to_twos_bytes();
            let decoded = BigInt::from_twos_bytes(&bytes).expect("decode");
            assert_eq!(bigint, decoded);
        }
    }

    #[test]
    fn checked_add_basic() {
        let a = BigInt::from_i128(10);
        let b = BigInt::from_i128(-3);
        assert_eq!(a.checked_add(&b).unwrap(), BigInt::from_i128(7));
    }

    #[test]
    fn checked_mul_basic() {
        let a = BigInt::from_i128(12);
        let b = BigInt::from_i128(-4);
        assert_eq!(a.checked_mul(&b).unwrap(), BigInt::from_i128(-48));
    }

    #[test]
    fn display_and_parse() {
        let v = BigInt::from_i128(-1_234_567_890);
        let s = v.to_string();
        let parsed: BigInt = s.parse().expect("parse");
        assert_eq!(v, parsed);
    }
}
