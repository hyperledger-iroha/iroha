//! Bounded variable-length signed integer with two's-complement encoding.
//!
//! The type wraps `num_bigint::BigInt`, enforces a signed 4,096-bit domain, and provides Norito and
//! JSON codecs that use a length-prefixed two's-complement byte representation. Small values stay
//! compact; larger values are allowed until the hard limit is reached.
//!
//! Norito encoding: a little-endian `u32` byte length (not compact) followed by
//! that many little-endian two's-complement bytes. A length of `0` represents
//! zero. When used by [`crate::numeric::Numeric`], this type stores the mantissa
//! only; the decimal scale is carried separately in `Numeric`.
use core::fmt;
use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::{
    Archived, Error as NoritoError, NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, DecodeFromSlice},
    json::{self, FastJsonWrite, JsonDeserialize},
};
use num_bigint::BigInt as InnerBigInt;
use num_traits::{One, Signed, Zero};
/// Width of the signed two's-complement domain represented by [`BigInt`].
///
/// Values are in `-2^4095..=2^4095-1`. This is deliberately a signed-width
/// bound, not a magnitude-bit bound.
pub const MAX_BITS: usize = 4_096;
/// Maximum canonical two's-complement payload length.
pub const MAX_ENCODED_BYTES: usize = MAX_BITS / 8;
/// Errors returned by [`BigInt`] operations.
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error, PartialEq, Eq)]
pub enum BigIntError {
    /// Value exceeds configured bit cap
    Overflow,
    /// Two's-complement byte representation is not minimal
    NonCanonical,
    /// Division by zero
    DivisionByZero,
}
/// Bounded signed integer with adaptive width in `-2^4095..=2^4095-1`.
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
    /// Bit length of the unsigned magnitude.
    pub fn bit_len(&self) -> usize {
        usize::try_from(self.inner.bits()).unwrap_or(usize::MAX)
    }
    /// Compute `10^exp` with signed-domain checking.
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
    /// the signed [`MAX_BITS`]-bit domain. This low-level constructor accepts fixed-width sign
    /// extension; the Norito decoder separately enforces minimal encoding.
    pub fn from_twos_bytes(bytes: &[u8]) -> Result<Self, BigIntError> {
        if bytes.len() > MAX_ENCODED_BYTES {
            return Err(BigIntError::Overflow);
        }
        let inner = InnerBigInt::from_signed_bytes_le(bytes);
        Self::from_inner(inner)
    }
    /// Emit minimal little-endian two's-complement byte representation.
    pub fn to_twos_bytes(&self) -> Vec<u8> {
        if self.inner.is_zero() {
            Vec::new()
        } else {
            self.inner.to_signed_bytes_le()
        }
    }
    /// Exact length of [`Self::to_twos_bytes`] without allocating the byte representation.
    #[must_use]
    pub fn twos_byte_len(&self) -> usize {
        signed_twos_byte_len(&self.inner)
    }
    /// Checked addition.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the sum leaves the signed domain.
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, BigIntError> {
        Self::from_inner(&self.inner + &rhs.inner)
    }
    /// Checked subtraction.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the difference leaves the signed domain.
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, BigIntError> {
        Self::from_inner(&self.inner - &rhs.inner)
    }
    /// Checked multiplication.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] if the product leaves the signed domain.
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
        // Compute the expensive quotient once. `%` on `num_bigint::BigInt`
        // performs another division, which would make runtime work disagree
        // with the VM's single quotient/remainder gas unit. Truncating division
        // guarantees `q * rhs` is no larger in magnitude than the dividend, so
        // deriving the remainder in the unbounded backend is exact before the
        // signed-domain checks below.
        let q = &self.inner / &rhs.inner;
        let r = &self.inner - (&q * &rhs.inner);
        Ok((Self::from_inner(q)?, Self::from_inner(r)?))
    }
    /// Checked absolute value.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] for the minimum value, whose positive
    /// counterpart is outside the signed domain.
    pub fn checked_abs(&self) -> Result<Self, BigIntError> {
        Self::from_inner(self.inner.abs())
    }
    /// Checked negation.
    ///
    /// # Errors
    /// Returns [`BigIntError::Overflow`] for the minimum value.
    pub fn checked_neg(&self) -> Result<Self, BigIntError> {
        Self::from_inner(-&self.inner)
    }
    /// Negation modulo `2^4096`, interpreted back in the signed domain.
    #[must_use]
    pub fn wrapping_neg(&self) -> Self {
        Self::from_wrapped_inner(-&self.inner)
    }
    /// Addition modulo `2^4096`, interpreted back in the signed domain.
    #[must_use]
    pub fn wrapping_add(&self, rhs: &Self) -> Self {
        Self::from_wrapped_inner(&self.inner + &rhs.inner)
    }
    /// Subtraction modulo `2^4096`, interpreted back in the signed domain.
    #[must_use]
    pub fn wrapping_sub(&self, rhs: &Self) -> Self {
        Self::from_wrapped_inner(&self.inner - &rhs.inner)
    }
    /// Multiplication modulo `2^4096`, interpreted back in the signed domain.
    #[must_use]
    pub fn wrapping_mul(&self, rhs: &Self) -> Self {
        Self::from_wrapped_inner(&self.inner * &rhs.inner)
    }
    /// Convert to `i64` if the value is representable.
    #[must_use]
    pub fn try_to_i64(&self) -> Option<i64> {
        num_traits::ToPrimitive::to_i64(&self.inner)
    }
    /// Convert to `u64` if the value is non-negative and representable.
    #[must_use]
    pub fn try_to_u64(&self) -> Option<u64> {
        num_traits::ToPrimitive::to_u64(&self.inner)
    }
    /// Convert to `u128` if the value is non-negative and representable.
    #[must_use]
    pub fn try_to_u128(&self) -> Option<u128> {
        num_traits::ToPrimitive::to_u128(&self.inner)
    }
    pub(crate) fn from_inner(inner: InnerBigInt) -> Result<Self, BigIntError> {
        if signed_twos_byte_len(&inner) > MAX_ENCODED_BYTES {
            return Err(BigIntError::Overflow);
        }
        Ok(Self { inner })
    }
    fn from_wrapped_inner(inner: InnerBigInt) -> Self {
        let modulus = InnerBigInt::one() << MAX_BITS;
        let sign_bit = InnerBigInt::one() << (MAX_BITS - 1);
        let mut residue = inner % &modulus;
        if residue.is_negative() {
            residue += &modulus;
        }
        if residue >= sign_bit {
            residue -= modulus;
        }
        Self::from_inner(residue).expect("modulo reduction produces a signed 4096-bit value")
    }
    pub(crate) fn inner(&self) -> &InnerBigInt {
        &self.inner
    }
}
fn signed_twos_byte_len(inner: &InnerBigInt) -> usize {
    if inner.is_zero() {
        return 0;
    }
    let magnitude = inner.magnitude();
    let magnitude_bits = magnitude.bits();
    let signed_bits =
        if inner.is_negative() && magnitude.trailing_zeros() == Some(magnitude_bits - 1) {
            // A negative power of two uses the sign bit itself as the top bit
            // (`-128` is exactly one byte, unlike `-129`).
            magnitude_bits
        } else {
            magnitude_bits.saturating_add(1)
        };
    usize::try_from(signed_bits.div_ceil(8)).unwrap_or(usize::MAX)
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), NoritoError> {
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
    fn encoded_len_exact(&self) -> Option<usize> {
        core::mem::size_of::<u32>().checked_add(self.twos_byte_len())
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
    fn write_json_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        // BigInt is bounded to 4,096 signed bits, so this scalar scratch string
        // has a fixed protocol-derived ceiling independent of response size.
        json::write_json_string_to(&self.to_string(), out)
    }
}
impl JsonDeserialize for BigInt {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        let parsed = value
            .parse::<BigInt>()
            .map_err(|err| json::Error::InvalidField {
                field: "bigint".into(),
                message: format!("invalid bigint `{value}`: {err}"),
            })?;
        if parsed.to_string() != value {
            return Err(json::Error::InvalidField {
                field: "bigint".into(),
                message: format!("noncanonical bigint `{value}`"),
            });
        }
        Ok(parsed)
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
        if value.to_twos_bytes() != payload {
            return Err(ncore::Error::Message(BigIntError::NonCanonical.to_string()));
        }
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
    fn minimal_twos_complement_transition_vectors_are_pinned() {
        for (value, expected) in [
            (0_i128, &[][..]),
            (127, &[0x7f][..]),
            (128, &[0x80, 0x00][..]),
            (-128, &[0x80][..]),
            (-129, &[0x7f, 0xff][..]),
        ] {
            let integer = BigInt::from_i128(value);
            assert_eq!(integer.to_twos_bytes(), expected, "value={value}");
            assert_eq!(BigInt::from_twos_bytes(expected), Ok(integer));
        }
    }
    #[test]
    fn allocation_free_twos_length_matches_canonical_encoding() {
        for value in [
            -65_537_i128,
            -65_536,
            -32_769,
            -32_768,
            -257,
            -256,
            -255,
            -129,
            -128,
            -127,
            -2,
            -1,
            0,
            1,
            2,
            127,
            128,
            129,
            255,
            256,
            32_767,
            32_768,
            65_535,
            65_536,
        ] {
            let value = BigInt::from_i128(value);
            assert_eq!(value.twos_byte_len(), value.to_twos_bytes().len());
        }
        let maximum: BigInt = ((InnerBigInt::one() << (MAX_BITS - 1)) - 1_u8)
            .to_string()
            .parse()
            .expect("maximum");
        let minimum: BigInt = (-(InnerBigInt::one() << (MAX_BITS - 1)))
            .to_string()
            .parse()
            .expect("minimum");
        assert_eq!(maximum.twos_byte_len(), MAX_ENCODED_BYTES);
        assert_eq!(minimum.twos_byte_len(), MAX_ENCODED_BYTES);
        assert_eq!(maximum.twos_byte_len(), maximum.to_twos_bytes().len());
        assert_eq!(minimum.twos_byte_len(), minimum.to_twos_bytes().len());
    }
    #[test]
    fn nonallocating_signed_length_preserves_from_inner_boundaries() {
        let signed_limit = InnerBigInt::one() << (MAX_BITS - 1);
        let values = [
            -signed_limit.clone() - 1_u8,
            -signed_limit.clone(),
            -signed_limit.clone() + 1_u8,
            InnerBigInt::from(-129_i16),
            InnerBigInt::from(-128_i16),
            InnerBigInt::from(0_u8),
            InnerBigInt::from(127_u8),
            InnerBigInt::from(128_u16),
            signed_limit.clone() - 1_u8,
            signed_limit.clone(),
            signed_limit + 1_u8,
        ];
        for value in values {
            let encoded_len = if value.is_zero() {
                0
            } else {
                value.to_signed_bytes_le().len()
            };
            assert_eq!(signed_twos_byte_len(&value), encoded_len);
            assert_eq!(
                BigInt::from_inner(value).is_ok(),
                encoded_len <= MAX_ENCODED_BYTES
            );
        }
    }
    #[test]
    fn exact_norito_length_matches_canonical_payload() {
        let assert_exact_length = |value: &BigInt| {
            assert_eq!(
                value.encoded_len_exact(),
                Some(norito::core::encoded_payload_len(value).expect("encode bigint payload"))
            );
        };
        for value in [
            -65_537_i128,
            -32_768,
            -129,
            -128,
            -1,
            0,
            1,
            127,
            128,
            32_767,
        ] {
            let value = BigInt::from_i128(value);
            assert_exact_length(&value);
        }
        let signed_limit = InnerBigInt::one() << (MAX_BITS - 1);
        for value in [
            BigInt::from_inner(-signed_limit.clone()).expect("minimum"),
            BigInt::from_inner(signed_limit - 1_u8).expect("maximum"),
        ] {
            assert_eq!(value.twos_byte_len(), MAX_ENCODED_BYTES);
            assert_exact_length(&value);
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
    #[test]
    fn zero_one_sign_checked_abs_neg_and_bit_len() {
        let zero = BigInt::zero();
        assert!(zero.is_zero());
        assert!(!zero.is_negative());
        assert_eq!(zero.bit_len(), 0);
        let one = BigInt::one();
        assert!(!one.is_zero());
        assert!(!one.is_negative());
        assert_eq!(one.bit_len(), 1);
        let negative = BigInt::from_i128(-42);
        assert!(negative.is_negative());
        assert_eq!(negative.checked_abs(), Ok(BigInt::from_i128(42)));
        assert_eq!(negative.checked_neg(), Ok(BigInt::from_i128(42)));
    }
    #[test]
    fn checked_sub_and_div_rem_basic() {
        let a = BigInt::from_i128(10);
        let b = BigInt::from_i128(13);
        assert_eq!(a.checked_sub(&b).unwrap(), BigInt::from_i128(-3));
        let dividend = BigInt::from_i128(17);
        let divisor = BigInt::from_i128(5);
        let (quotient, remainder) = dividend.checked_div_rem(&divisor).unwrap();
        assert_eq!(quotient, BigInt::from_i128(3));
        assert_eq!(remainder, BigInt::from_i128(2));
    }
    #[test]
    fn checked_div_rem_rejects_zero_divisor() {
        let err = BigInt::from_i128(17)
            .checked_div_rem(&BigInt::zero())
            .expect_err("division by zero should be rejected");
        assert_eq!(err, BigIntError::DivisionByZero);
    }
    #[test]
    fn checked_div_rem_obeys_truncating_identity_for_all_signs() {
        for dividend in [-257_i128, -17, -1, 0, 1, 17, 257] {
            for divisor in [-19_i128, -5, -1, 1, 5, 19] {
                let lhs = BigInt::from_i128(dividend);
                let rhs = BigInt::from_i128(divisor);
                let (quotient, remainder) = lhs
                    .checked_div_rem(&rhs)
                    .expect("small quotient and remainder fit");
                assert_eq!(
                    quotient
                        .checked_mul(&rhs)
                        .and_then(|product| product.checked_add(&remainder)),
                    Ok(lhs.clone()),
                    "identity failed for {dividend} / {divisor}"
                );
                assert!(
                    remainder.is_zero() || remainder.is_negative() == lhs.is_negative(),
                    "remainder sign must follow the dividend for {dividend} / {divisor}"
                );
                assert!(
                    remainder.inner.abs() < rhs.inner.abs(),
                    "remainder magnitude must be below the divisor for {dividend} / {divisor}"
                );
            }
        }
    }
    #[test]
    fn pow10_obeys_bit_limit() {
        assert_eq!(BigInt::pow10(0).unwrap(), BigInt::one());
        assert_eq!(BigInt::pow10(2).unwrap(), BigInt::from_i128(100));
        assert!(BigInt::pow10(1_232).is_some());
        assert!(BigInt::pow10(1_233).is_none());
    }
    #[test]
    fn from_twos_bytes_rejects_overflow() {
        let mut bytes = vec![0_u8; MAX_ENCODED_BYTES + 1];
        bytes[MAX_ENCODED_BYTES] = 0x01;
        let err = BigInt::from_twos_bytes(&bytes).expect_err("4097-bit value must overflow");
        assert_eq!(err, BigIntError::Overflow);
    }
    #[test]
    fn signed_4096_bit_endpoints_roundtrip_and_neighbors_overflow() {
        let positive_bytes = vec![0xff_u8; MAX_ENCODED_BYTES - 1]
            .into_iter()
            .chain([0x7f])
            .collect::<Vec<_>>();
        let positive = BigInt::from_twos_bytes(&positive_bytes).expect("signed maximum must fit");
        assert_eq!(positive.bit_len(), MAX_BITS - 1);
        assert_eq!(positive.to_twos_bytes(), positive_bytes);
        assert_eq!(
            positive.checked_add(&BigInt::one()),
            Err(BigIntError::Overflow)
        );
        let negative_bytes = vec![0_u8; MAX_ENCODED_BYTES - 1]
            .into_iter()
            .chain([0x80])
            .collect::<Vec<_>>();
        let negative = BigInt::from_twos_bytes(&negative_bytes).expect("signed minimum must fit");
        assert_eq!(negative.bit_len(), MAX_BITS);
        assert_eq!(negative.to_twos_bytes(), negative_bytes);
        assert_eq!(
            negative.checked_sub(&BigInt::one()),
            Err(BigIntError::Overflow)
        );
        assert_eq!(negative.checked_abs(), Err(BigIntError::Overflow));
        assert_eq!(negative.checked_neg(), Err(BigIntError::Overflow));
    }
    #[test]
    fn wrapping_arithmetic_is_modulo_two_to_4096() {
        let max: BigInt = ((InnerBigInt::one() << (MAX_BITS - 1)) - 1_u8)
            .to_string()
            .parse()
            .expect("maximum");
        let min: BigInt = (-(InnerBigInt::one() << (MAX_BITS - 1)))
            .to_string()
            .parse()
            .expect("minimum");
        assert_eq!(max.wrapping_add(&BigInt::one()), min);
        assert_eq!(min.wrapping_sub(&BigInt::one()), max);
        assert_eq!(min.wrapping_neg(), min);
        assert_eq!(
            max.wrapping_mul(&BigInt::from_i128(2)),
            BigInt::from_i128(-2)
        );
        for seed in 0_i128..=256 {
            let lhs = BigInt::from_i128(seed * seed - 12_345);
            let rhs = BigInt::from_i128(seed * 97 - 4_321);
            assert_eq!(
                lhs.wrapping_add(&rhs),
                lhs.checked_add(&rhs).expect("small sum")
            );
            assert_eq!(
                lhs.wrapping_sub(&rhs),
                lhs.checked_sub(&rhs).expect("small difference")
            );
            assert_eq!(
                lhs.wrapping_mul(&rhs),
                lhs.checked_mul(&rhs).expect("small product")
            );
        }
    }
    #[test]
    fn signed_conversion_boundaries_are_exact() {
        assert_eq!(BigInt::from(i64::MIN).try_to_i64(), Some(i64::MIN));
        assert_eq!(BigInt::from(i64::MAX).try_to_i64(), Some(i64::MAX));
        assert_eq!(BigInt::from(-1_i64).try_to_u64(), None);
        assert_eq!(BigInt::from(u64::MAX).try_to_u64(), Some(u64::MAX));
        assert_eq!(BigInt::from(u64::MAX).try_to_i64(), None);
    }
    #[test]
    fn unsigned_128_conversion_boundaries_are_exact() {
        assert_eq!(BigInt::from(-1_i64).try_to_u128(), None);
        assert_eq!(BigInt::from(u128::MAX).try_to_u128(), Some(u128::MAX));

        let above_max: BigInt = "340282366920938463463374607431768211456"
            .parse()
            .expect("u128::MAX + 1 fits the adaptive-width domain");
        assert_eq!(above_max.try_to_u128(), None);
    }
    #[test]
    fn norito_decode_rejects_redundant_sign_extension() {
        for bytes in [&[0_u8][..], &[1, 0], &[0xff, 0xff]] {
            let mut encoded =
                norito::codec::Encode::encode(&u32::try_from(bytes.len()).expect("small length"));
            encoded.extend_from_slice(bytes);
            let error = <BigInt as DecodeFromSlice>::decode_from_slice(&encoded)
                .expect_err("redundant sign extension must fail");
            assert!(error.to_string().contains("not minimal"), "{error}");
        }
        assert_eq!(BigInt::from_twos_bytes(&[]), Ok(BigInt::zero()));
    }
    #[test]
    fn decode_from_slice_rejects_short_payload() {
        let mut bytes = norito::codec::Encode::encode(&4_u32);
        bytes.extend([1_u8, 2]);
        let err = <BigInt as DecodeFromSlice>::decode_from_slice(&bytes)
            .expect_err("declared payload length should be enforced");
        match err {
            ncore::Error::Message(message) => assert_eq!(message, "buffer too short"),
            other => panic!("unexpected decode error: {other:?}"),
        }
    }
    #[test]
    fn decode_from_slice_reports_used_bytes() {
        let value = BigInt::from_i128(-9_876_543_210);
        let mut bytes = norito::codec::Encode::encode(&value);
        let encoded_len = bytes.len();
        bytes.extend([0xaa, 0xbb]);
        let (decoded, used) =
            <BigInt as DecodeFromSlice>::decode_from_slice(&bytes).expect("decode");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded_len);
    }
    #[test]
    fn json_roundtrip_and_invalid_error_field() {
        let value = BigInt::from_i128(-123_456_789);
        let json = norito::json::to_json(&value).expect("serialize");
        assert_eq!(json, "\"-123456789\"");
        let decoded: BigInt = norito::json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, value);
        let err = norito::json::from_str::<BigInt>("\"not-a-number\"")
            .expect_err("invalid bigint string should be rejected");
        match err {
            json::Error::InvalidField { field, message } => {
                assert_eq!(field, "bigint");
                assert!(message.contains("invalid bigint `not-a-number`"));
            }
            other => panic!("unexpected JSON error: {other:?}"),
        }
        for noncanonical in ["+1", "01", "-0", " 1"] {
            let source = format!("\"{noncanonical}\"");
            let error = norito::json::from_str::<BigInt>(&source)
                .expect_err("alternate bigint spelling must be rejected");
            assert!(
                matches!(error, json::Error::InvalidField { ref field, .. } if field == "bigint"),
                "source={source} error={error:?}"
            );
        }
    }
}
