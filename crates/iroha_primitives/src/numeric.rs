//! Signed decimal number with variable-length mantissa (up to 512 bits) and scale.
//!
//! This replaces the previous fixed-width, non-negative decimal. Mantissas are
//! stored in [`crate::bigint::BigInt`] and allow negative values; scale counts
//! fractional digits (e.g., `1.88` => mantissa `188`, scale `2`).
//!
//! Encoding note: `Numeric` serializes as a helper carrying `(mantissa, scale)`.
//! The mantissa is a raw [`crate::bigint::BigInt`] integer (no decimal scale
//! is embedded in the integer), and the scale is stored separately as a `u32`.

use core::{cmp::Ordering, str::FromStr};
use std::{
    io::Write,
    string::{String, ToString},
    vec::Vec,
};

use derive_more::From;
pub use iroha_primitives_derive::numeric;
use norito::{
    Archived, Error, NoritoDeserialize, NoritoSerialize,
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize},
};

use crate::bigint::{BigInt, MAX_BITS as BIGINT_MAX_BITS};

/// Decimal number with arbitrary precision and scale.
///
/// The finite set of values of type [`Numeric`] are of the form $m / 10^e$,
/// where `m` is a signed integer with up to 512 bits and `e` is in `[0, 28]`.
/// The mantissa `m` is stored as a [`crate::bigint::BigInt`], while the scale
/// `e` is carried separately.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Numeric {
    mantissa: BigInt,
    scale: u32,
}

/// Define maximum precision and scale for given number.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Hash,
    From,
    iroha_schema::IntoSchema,
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
pub struct NumericSpec {
    /// Count of decimal digits in the fractional part.
    /// Currently only positive scale up to 28 decimal points is supported.
    scale: Option<u32>,
}

impl NoritoSerialize for NumericSpec {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        NoritoSerialize::serialize(&self.scale, writer)
    }
}

// Bridge Norito slice-based decoding for Numeric to the codec decoder so that
// containers (Vec/Option) of Numeric can be decoded in data-model queries.
impl<'a> norito::core::DecodeFromSlice<'a> for Numeric {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut s: &'a [u8] = bytes;
        let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
            .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
        let used = bytes.len() - s.len();
        Ok((value, used))
    }
}

impl<'a> NoritoDeserialize<'a> for NumericSpec {
    fn deserialize(archived: &'a Archived<NumericSpec>) -> Self {
        let scale_arch: &Archived<Option<u32>> = archived.cast();
        let scale = <Option<u32> as NoritoDeserialize>::deserialize(scale_arch);
        NumericSpec { scale }
    }
}

impl FastJsonWrite for NumericSpec {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"scale\":");
        if let Some(scale) = self.scale {
            scale.json_serialize(out);
        } else {
            out.push_str("null");
        }
        out.push('}');
    }
}

impl JsonDeserialize for NumericSpec {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut scale: Option<Option<u32>> = None;
        while let Some(key) = visitor.next_key()? {
            match key {
                json::KeyRef::Borrowed("scale") => {
                    let value = visitor.parse_value::<Option<u32>>()?;
                    scale = Some(value);
                }
                json::KeyRef::Owned(ref key) if key == "scale" => {
                    let value = visitor.parse_value::<Option<u32>>()?;
                    scale = Some(value);
                }
                _ => visitor.skip_value()?,
            }
        }
        Ok(NumericSpec {
            scale: scale.unwrap_or(None),
        })
    }
}

/// Error occurred during creation of [`Numeric`]
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error)]
pub enum NumericError {
    /// Mantissa exceeds allowed range
    MantissaTooLarge,
    /// Scale exeeds allowed range
    ScaleTooLarge,
    /// Malformed: expecting number with optional decimal point (10, 10.02)
    Malformed,
}

/// The error type returned when a numeric conversion fails.
#[derive(Debug, Clone, Copy, displaydoc::Display, thiserror::Error)]
pub struct TryFromNumericError;

/// Error occurred while checking if number satisfy given spec
#[derive(Clone, Copy, Debug, displaydoc::Display, thiserror::Error)]
pub enum NumericSpecError {
    /// Given number has scale higher than allowed by spec.
    ScaleTooHigh,
}

/// Error occurred while checking if number satisfy given spec
#[derive(Clone, Debug, displaydoc::Display, thiserror::Error)]
pub enum NumericSpecParseError {
    /// String representation should start with Numeric
    StartWithNumeric,
    /// Numeric should be followed by optional scale wrapped in braces
    WrappedInBraces,
    /// Scale should be valid integer value: {_0}
    InvalidScale(#[source] <u32 as FromStr>::Err),
}

impl Numeric {
    /// Zero numeric value
    pub fn zero() -> Self {
        Self::new(BigInt::zero(), 0)
    }
    /// One numeric value
    pub fn one() -> Self {
        Self::new(BigInt::one(), 0)
    }

    /// Create new numeric given mantissa and scale
    ///
    /// # Panics
    /// Panics in cases where [`Self::try_new`] would return error.
    #[inline]
    pub fn new<T: Into<BigInt>>(mantissa: T, scale: u32) -> Self {
        match Self::try_new(mantissa, scale) {
            Ok(numeric) => numeric,
            Err(NumericError::ScaleTooLarge) => panic!("failed to create numeric: scale too large"),
            Err(NumericError::MantissaTooLarge) => {
                panic!("failed to create numeric: mantissa too large")
            }
            Err(NumericError::Malformed) => unreachable!(),
        }
    }

    /// Try to create numeric given mantissa and scale
    ///
    /// # Errors
    /// - if mantissa exceeds 512 bits
    /// - if scale is greater than 28
    #[inline]
    pub fn try_new<T: Into<BigInt>>(mantissa: T, scale: u32) -> Result<Self, NumericError> {
        if scale > 28 {
            return Err(NumericError::ScaleTooLarge);
        }
        let mantissa = mantissa.into();
        if mantissa.bit_len() > BIGINT_MAX_BITS {
            return Err(NumericError::MantissaTooLarge);
        }

        Ok(Self { mantissa, scale })
    }

    /// Return mantissa of number (signed).
    #[inline]
    pub fn mantissa(&self) -> &BigInt {
        &self.mantissa
    }

    /// Try to view mantissa as u128 (fails on negative or too-wide values).
    #[inline]
    pub fn try_mantissa_u128(&self) -> Option<u128> {
        if self.mantissa.is_negative() {
            None
        } else {
            self.mantissa.to_string().parse::<u128>().ok()
        }
    }

    /// Try to view mantissa as i128 (fails if too wide).
    #[inline]
    pub fn try_mantissa_i128(&self) -> Option<i128> {
        self.mantissa.to_string().parse::<i128>().ok()
    }

    /// Return scale of number
    #[inline]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Reduce the scale by stripping trailing zero fractional digits.
    #[must_use]
    pub fn trim_trailing_zeros(mut self) -> Self {
        let ten = BigInt::from_i128(10);
        while self.scale > 0 {
            match self.mantissa.checked_div_rem(&ten) {
                Ok((quotient, remainder)) if remainder.is_zero() => {
                    self.mantissa = quotient;
                    self.scale -= 1;
                }
                _ => break,
            }
        }
        self
    }

    fn scale_up(mantissa: &BigInt, delta_scale: u32) -> Option<BigInt> {
        if delta_scale == 0 {
            return Some(mantissa.clone());
        }
        let factor = BigInt::pow10(delta_scale)?;
        let product = mantissa.checked_mul(&factor).ok()?;
        Self::enforce_bounds(product)
    }

    fn enforce_bounds(value: BigInt) -> Option<BigInt> {
        (value.bit_len() <= BIGINT_MAX_BITS).then_some(value)
    }

    /// Checked addition. Computes `self + other`, returning `None` if overflow occurred
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let Numeric {
            mantissa: lhs_mantissa,
            scale: lhs_scale,
        } = self;
        let Numeric {
            mantissa: rhs_mantissa,
            scale: rhs_scale,
        } = other;
        let target_scale = lhs_scale.max(rhs_scale);
        let lhs = Self::scale_up(&lhs_mantissa, target_scale - lhs_scale)?;
        let rhs = Self::scale_up(&rhs_mantissa, target_scale - rhs_scale)?;
        let sum = lhs.checked_add(&rhs).ok()?;
        let sum = Self::enforce_bounds(sum)?;
        Numeric::try_new(sum, target_scale).ok()
    }

    /// Checked subtraction. Computes `self - other`, returning `None` if overflow occurred
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        let Numeric {
            mantissa: lhs_mantissa,
            scale: lhs_scale,
        } = self;
        let Numeric {
            mantissa: rhs_mantissa,
            scale: rhs_scale,
        } = other;
        let target_scale = lhs_scale.max(rhs_scale);
        let lhs = Self::scale_up(&lhs_mantissa, target_scale - lhs_scale)?;
        let rhs = Self::scale_up(&rhs_mantissa, target_scale - rhs_scale)?;
        let diff = lhs.checked_sub(&rhs).ok()?;
        let diff = Self::enforce_bounds(diff)?;
        Numeric::try_new(diff, target_scale).ok()
    }

    /// Checked multiplication. Computes `self * other`, returning `None` if overflow occurred
    pub fn checked_mul(self, other: Self, spec: NumericSpec) -> Option<Self> {
        let Numeric {
            mantissa: lhs_mantissa,
            scale: lhs_scale,
        } = self;
        let Numeric {
            mantissa: rhs_mantissa,
            scale: rhs_scale,
        } = other;
        let product = lhs_mantissa.checked_mul(&rhs_mantissa).ok()?;
        let mut scale = lhs_scale + rhs_scale;
        let mut adjusted = product;

        if let Some(target_scale) = spec.scale
            && scale > target_scale
        {
            let trim = scale - target_scale;
            let factor = BigInt::pow10(trim)?;
            let (q, _) = adjusted.checked_div_rem(&factor).ok()?;
            adjusted = q;
            scale = target_scale;
        }

        let adjusted = Self::enforce_bounds(adjusted)?;
        Numeric::try_new(adjusted, scale).ok()
    }

    /// Checked division. Computes `self / other`, returning `None` if overflow occurred.
    pub fn checked_div(self, other: Self, spec: NumericSpec) -> Option<Self> {
        let Numeric {
            mantissa: lhs_mantissa,
            scale: lhs_scale,
        } = self;
        let Numeric {
            mantissa: rhs_mantissa,
            scale: rhs_scale,
        } = other;
        if rhs_mantissa.is_zero() {
            return None;
        }
        let target_scale = spec.scale.unwrap_or_else(|| lhs_scale.max(rhs_scale));
        // a/10^sa / (b/10^sb) = (a * 10^(sb+target_scale)) / (b * 10^sa)
        let num_scale = rhs_scale + target_scale;
        let num = lhs_mantissa.checked_mul(&BigInt::pow10(num_scale)?).ok()?;
        let denom = rhs_mantissa.checked_mul(&BigInt::pow10(lhs_scale)?).ok()?;
        let (quot, _) = num.checked_div_rem(&denom).ok()?;
        let quot = Self::enforce_bounds(quot)?;
        Numeric::try_new(quot, target_scale).ok()
    }

    /// Checked remainder. Computes `self % other`, returning `None` if overflow occurred.
    pub fn checked_rem(self, other: Self, spec: NumericSpec) -> Option<Self> {
        let Numeric {
            mantissa: lhs_mantissa,
            scale: lhs_scale,
        } = self;
        let Numeric {
            mantissa: rhs_mantissa,
            scale: rhs_scale,
        } = other;
        if rhs_mantissa.is_zero() {
            return None;
        }
        let target_scale = lhs_scale.max(rhs_scale);
        let lhs = Self::scale_up(&lhs_mantissa, target_scale - lhs_scale)?;
        let rhs = Self::scale_up(&rhs_mantissa, target_scale - rhs_scale)?;
        let (_, rem) = lhs.checked_div_rem(&rhs).ok()?;
        let mut rem = rem;
        let mut scale = target_scale;
        if let Some(out_scale) = spec.scale
            && scale > out_scale
        {
            let trim = scale - out_scale;
            let factor = BigInt::pow10(trim)?;
            let (q, _) = rem.checked_div_rem(&factor).ok()?;
            rem = q;
            scale = out_scale;
        }
        let rem = Self::enforce_bounds(rem)?;
        Numeric::try_new(rem, scale).ok()
    }

    /// Returns a new `Numeric` number rounded (truncated) to the given scale.
    #[must_use]
    pub fn round(&self, spec: NumericSpec) -> Self {
        if let Some(scale) = spec.scale {
            if scale >= self.scale {
                return Self::new(self.mantissa.clone(), self.scale);
            }
            let delta = self.scale - scale;
            let factor = BigInt::pow10(delta).expect("pow");
            let (trimmed, _) = self.mantissa.checked_div_rem(&factor).expect("div ok");
            return Self::new(trimmed, scale);
        }

        Self::new(self.mantissa.clone(), self.scale)
    }

    /// Convert [`Numeric`] to [`f64`] with possible loss in precision
    pub fn to_f64(&self) -> f64 {
        self.to_string().parse().unwrap_or(f64::NAN)
    }

    /// Check if number is zero
    pub fn is_zero(&self) -> bool {
        self.mantissa.is_zero()
    }
}

impl Numeric {
    /// Encode this `Numeric` into Norito bytes.
    pub fn encode(&self) -> Vec<u8> {
        let helper = scale_::NumericScaleHelper {
            mantissa: self.mantissa.clone(),
            scale: self.scale(),
        };
        norito::codec::Encode::encode(&helper)
    }

    /// Decode `Numeric` from Norito-encoded input.
    ///
    /// # Errors
    /// Returns an error if the input does not contain a valid [`Numeric`]
    /// representation or if its mantissa or scale exceed supported limits.
    pub fn decode<I: norito::codec::Input>(input: &mut I) -> Result<Self, norito::Error> {
        let scale_::NumericScaleHelper { mantissa, scale } =
            <scale_::NumericScaleHelper as norito::codec::Decode>::decode(input)?;
        match Numeric::try_new(mantissa, scale) {
            Ok(numeric) => Ok(numeric),
            Err(NumericError::MantissaTooLarge) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "error decoding numeric: mantissa too large",
            )
            .into()),
            Err(NumericError::ScaleTooLarge) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "error decoding numeric: scale too large",
            )
            .into()),
            Err(NumericError::Malformed) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "error decoding numeric: malformed",
            )
            .into()),
        }
    }
}

impl NoritoSerialize for Numeric {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
        let helper = scale_::NumericScaleHelper {
            mantissa: self.mantissa.clone(),
            scale: self.scale(),
        };
        helper.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        None
    }
}

impl<'a> NoritoDeserialize<'a> for Numeric {
    fn deserialize(archived: &'a Archived<Numeric>) -> Self {
        Self::try_deserialize(archived).expect("invalid numeric")
    }

    fn try_deserialize(archived: &'a Archived<Numeric>) -> Result<Self, Error> {
        let helper_align = core::mem::align_of::<Archived<scale_::NumericScaleHelper>>();
        let numeric_align = core::mem::align_of::<Archived<Numeric>>();
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let aligned = numeric_align >= helper_align || (ptr as usize).is_multiple_of(helper_align);

        if aligned {
            let helper_arch: &Archived<scale_::NumericScaleHelper> = archived.cast();
            let helper = scale_::NumericScaleHelper::try_deserialize(helper_arch)?;
            let value = Numeric::try_new(helper.mantissa, helper.scale)
                .map_err(|err| Error::Message(format!("invalid numeric: {err}")))?;
            Ok(value)
        } else {
            let slice = norito::core::payload_slice_from_ptr(ptr)?;
            let (value, _) = <Numeric as norito::core::DecodeFromSlice>::decode_from_slice(slice)?;
            Ok(value)
        }
    }
}

impl FastJsonWrite for Numeric {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for Numeric {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        value
            .parse::<Numeric>()
            .map_err(|err| json::Error::InvalidField {
                field: "numeric".into(),
                message: format!("invalid numeric `{value}`: {err}"),
            })
    }
}

impl From<u32> for Numeric {
    fn from(value: u32) -> Self {
        Self::new(BigInt::from(i128::from(value)), 0)
    }
}

impl From<u64> for Numeric {
    fn from(value: u64) -> Self {
        Self::new(BigInt::from(i128::from(value)), 0)
    }
}

impl From<i64> for Numeric {
    fn from(value: i64) -> Self {
        Self::new(BigInt::from(i128::from(value)), 0)
    }
}

impl TryFrom<f64> for Numeric {
    type Error = TryFromNumericError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let s = value.to_string();
        s.parse().map_err(|_| TryFromNumericError)
    }
}

impl TryFrom<Numeric> for u32 {
    type Error = TryFromNumericError;

    fn try_from(value: Numeric) -> Result<Self, Self::Error> {
        value
            .to_string()
            .parse::<u32>()
            .map_err(|_| TryFromNumericError)
    }
}

impl TryFrom<Numeric> for u64 {
    type Error = TryFromNumericError;

    fn try_from(value: Numeric) -> Result<Self, Self::Error> {
        value
            .to_string()
            .parse::<u64>()
            .map_err(|_| TryFromNumericError)
    }
}

impl Ord for Numeric {
    fn cmp(&self, other: &Self) -> Ordering {
        let target_scale = self.scale.max(other.scale);
        let lhs = Self::scale_up(&self.mantissa, target_scale - self.scale);
        let rhs = Self::scale_up(&other.mantissa, target_scale - other.scale);
        match (lhs, rhs) {
            (Some(lhs), Some(rhs)) => lhs.cmp(&rhs),
            _ => self
                .mantissa
                .cmp(&other.mantissa)
                .then_with(|| self.scale.cmp(&other.scale)),
        }
    }
}

impl PartialOrd for Numeric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NumericSpec {
    /// Check if given numeric satisfy constrains
    ///
    /// # Errors
    /// If given number has precision or scale higher than specified by spec.
    pub fn check(self, numeric: &Numeric) -> Result<(), NumericSpecError> {
        if let Some(allowed_scale) = self.scale {
            let actual_scale = numeric.scale();
            if actual_scale <= allowed_scale {
                return Ok(());
            }

            // Allow higher-scale representations when the extra fractional digits are all zero
            // (e.g., "1.00" should satisfy an integer-only spec).
            let trim = actual_scale - allowed_scale;
            let factor = BigInt::pow10(trim).ok_or(NumericSpecError::ScaleTooHigh)?;
            if numeric
                .mantissa()
                .clone()
                .checked_div_rem(&factor)
                .is_ok_and(|(_, rem)| rem.is_zero())
            {
                return Ok(());
            }

            return Err(NumericSpecError::ScaleTooHigh);
        }

        Ok(())
    }

    /// Create [`NumericSpec`] which accepts any numeric value
    #[inline]
    pub const fn unconstrained() -> Self {
        NumericSpec { scale: None }
    }

    /// Create [`NumericSpec`] which accepts only integer values
    #[inline]
    pub const fn integer() -> Self {
        Self { scale: Some(0) }
    }

    /// Create [`NumericSpec`] which accepts numeric values with scale up to given decimal places
    #[inline]
    pub const fn fractional(scale: u32) -> Self {
        Self { scale: Some(scale) }
    }

    /// Get the scale
    #[inline]
    pub const fn scale(self) -> Option<u32> {
        self.scale
    }
}

impl core::str::FromStr for Numeric {
    type Err = NumericError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(NumericError::Malformed);
        }
        let negative = trimmed.starts_with('-');
        let digits = trimmed.trim_start_matches(['+', '-']);
        let mut scale = 0u32;
        let mut mantissa_str = String::new();
        let mut seen_dot = false;
        for ch in digits.chars() {
            if ch == '.' {
                if seen_dot {
                    return Err(NumericError::Malformed);
                }
                seen_dot = true;
                continue;
            }
            if !ch.is_ascii_digit() {
                return Err(NumericError::Malformed);
            }
            mantissa_str.push(ch);
            if seen_dot {
                scale = scale.saturating_add(1);
            }
        }
        if scale > 28 {
            return Err(NumericError::ScaleTooLarge);
        }
        let mut mantissa: BigInt = mantissa_str
            .parse::<BigInt>()
            .map_err(|_| NumericError::Malformed)?;
        if negative {
            mantissa = mantissa.neg();
        }
        Numeric::try_new(mantissa, scale)
    }
}

impl core::fmt::Display for NumericSpec {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Numeric")?;
        if let Some(scale) = self.scale {
            write!(f, "({scale})")?;
        }
        Ok(())
    }
}

impl core::fmt::Display for Numeric {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.mantissa);
        }
        let mut s = self.mantissa.abs().to_string();
        while s.len() <= self.scale as usize {
            s.insert(0, '0');
        }
        let (int_part, frac_part) = s.split_at(s.len() - self.scale as usize);
        if self.mantissa.is_negative() {
            write!(f, "-{int_part}.{frac_part}")
        } else {
            write!(f, "{int_part}.{frac_part}")
        }
    }
}

mod scale_ {
    #[allow(unexpected_cfgs)]
    #[derive(norito::Encode, norito::Decode)]
    #[norito(decode_from_slice)]
    /// Internal helper used to encode/decode Numeric as `(mantissa, scale)`.
    pub(super) struct NumericScaleHelper {
        /// Mantissa carried by the numeric helper.
        #[codec(compact)]
        pub(super) mantissa: crate::bigint::BigInt,
        /// Scale carried by the numeric helper.
        #[codec(compact)]
        pub(super) scale: u32,
    }
}

mod schema_ {
    use iroha_schema::{
        Compact, Declaration, Ident, IntoSchema, MetaMap, Metadata, NamedFieldsMeta, TypeId,
    };

    use super::*;

    impl TypeId for Numeric {
        fn id() -> Ident {
            "Numeric".to_string()
        }
    }

    impl IntoSchema for Numeric {
        fn type_name() -> Ident {
            "Numeric".to_string()
        }

        fn update_schema_map(metamap: &mut MetaMap) {
            if !metamap.contains_key::<Self>() {
                <crate::bigint::BigInt as iroha_schema::IntoSchema>::update_schema_map(metamap);
                <Compact<u32> as iroha_schema::IntoSchema>::update_schema_map(metamap);

                metamap.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                    declarations: vec![
                        Declaration {
                            name: "mantissa".to_string(),
                            ty: core::any::TypeId::of::<crate::bigint::BigInt>(),
                        },
                        Declaration {
                            name: "scale".to_string(),
                            ty: core::any::TypeId::of::<Compact<u32>>(),
                        },
                    ],
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use super::*;

    #[test]
    fn check_add() {
        let a = Numeric::new(10, 0);
        let b = Numeric::new(9, 3);

        assert_eq!(a.checked_add(b), Some(Numeric::new(10009, 3)));

        let a = Numeric::new(1, 2);
        let b = Numeric::new(999, 2);

        assert_eq!(a.checked_add(b), Some(Numeric::new(1000, 2)));
    }

    #[test]
    fn numeric_ordering_compares_value_not_repr() {
        let ten = Numeric::new(10, 0);
        let nine_point_eight = Numeric::new(98, 1);
        let nine_point_eight_fine = Numeric::new(9_800, 3);

        assert!(nine_point_eight < ten);
        assert!(nine_point_eight_fine < ten);
        assert_eq!(
            nine_point_eight.partial_cmp(&nine_point_eight_fine),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn check_json_roundtrip() {
        let num1 = Numeric::new(1002, 2);

        let s = norito::json::to_json(&num1).expect("failed to serialize numeric");

        assert_eq!(s, "\"10.02\"");

        let num2 = norito::json::from_str(&s).expect("failed to deserialize numeric");

        assert_eq!(num1, num2);
    }

    #[test]
    fn numeric_spec_json_roundtrip() {
        let specs = [NumericSpec::unconstrained(), NumericSpec::fractional(5)];
        let mut serialized = Vec::new();
        for spec in specs {
            serialized.push(norito::json::to_json(&spec).expect("serialize spec"));
        }
        assert_eq!(serialized[0], "{\"scale\":null}");
        assert_eq!(serialized[1], "{\"scale\":5}");
        for json in serialized {
            let decoded: NumericSpec = norito::json::from_json(&json).expect("deserialize spec");
            let reencoded = norito::json::to_json(&decoded).expect("re-serialize spec");
            assert_eq!(reencoded, json);
        }
    }

    #[test]
    fn numeric_spec_allows_trailing_zero_scale_reduction() {
        let integer_spec = NumericSpec::integer();
        assert!(integer_spec.check(&Numeric::new(100, 2)).is_ok());
        assert!(matches!(
            integer_spec.check(&Numeric::new(101, 2)),
            Err(NumericSpecError::ScaleTooHigh)
        ));

        let fractional_spec = NumericSpec::fractional(1);
        assert!(fractional_spec.check(&Numeric::new(120, 2)).is_ok());
        assert!(matches!(
            fractional_spec.check(&Numeric::new(121, 2)),
            Err(NumericSpecError::ScaleTooHigh)
        ));
    }

    #[test]
    fn trim_trailing_zeros_normalises_scale() {
        assert_eq!(
            Numeric::new(1000, 3).trim_trailing_zeros(),
            Numeric::new(1, 0)
        );
        assert_eq!(
            Numeric::new(1230, 2).trim_trailing_zeros(),
            Numeric::new(123, 1)
        );
        assert_eq!(
            Numeric::new(1234, 2).trim_trailing_zeros(),
            Numeric::new(1234, 2)
        );
    }

    // Ensure Norito codec round-trips the value without loss.
    #[test]
    fn check_norito_roundtrip() {
        let num1 = Numeric::new(1002, 2);

        let s = num1.encode();

        let num2 = Numeric::decode(&mut s.as_slice()).expect("failed to decode numeric");

        assert_eq!(num1, num2);
    }

    #[test]
    fn numeric_canonical_roundtrip() {
        let value = Numeric::new(12345, 3);
        let payload = norito::codec::Encode::encode(&value);
        let (decoded, used) = norito::core::decode_field_canonical::<Numeric>(&payload)
            .expect("decode canonical numeric");
        assert_eq!(decoded, value);
        assert_eq!(used, payload.len());
    }
}
