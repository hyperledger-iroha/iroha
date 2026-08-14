//! Exact decimal arithmetic with a signed 512-bit mantissa and bounded scale.
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
    string::{String, ToString},
    vec::Vec,
};

pub use iroha_primitives_derive::numeric;
use norito::{
    Archived, Error, NoritoDeserialize, NoritoSerialize,
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize},
};
use num_bigint::BigInt as UnboundedBigInt;
use num_traits::{One as _, Signed as _, Zero as _};

use crate::bigint::BigInt;

/// Width of the signed two's-complement domain shared by Kotodama `int`,
/// `decimal` mantissas, and `quantity` mantissas.
pub const MAX_MANTISSA_BITS: usize = 512;
/// Maximum canonical two's-complement mantissa payload length.
pub const MAX_MANTISSA_BYTES: usize = MAX_MANTISSA_BITS / 8;

/// Maximum number of fractional decimal digits in a canonical decimal.
pub const MAX_DECIMAL_SCALE: u32 = 28;

/// Maximum number of factors accepted by aggregate decimal-product helpers.
///
/// Each factor is individually bounded to a 512-bit canonical mantissa, but
/// the helpers deliberately retain an unbounded conceptual intermediate until
/// their single final normalization or rounding step. Bounding the factor
/// inventory prevents attacker-controlled iterators from growing that
/// intermediate without limit.
pub const MAX_DECIMAL_PRODUCT_FACTORS: usize = 64;

/// Canonical exact decimal with a bounded signed mantissa and scale.
///
/// The finite set of values of type [`Numeric`] are of the form $m / 10^e$,
/// where `m` is in `-2^511..=2^511-1` and `e` is in `[0, 28]`.
/// The mantissa `m` is stored as a [`crate::bigint::BigInt`], while the scale
/// `e` is carried separately. Public constructors strip fractional trailing
/// zeroes, including reducing every zero to scale zero, so equality, ordering,
/// hashing, map keys, and serialization all observe one representation.
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Numeric {
    mantissa: BigInt,
    scale: u32,
}

/// Canonical non-negative decimal for asset and resource quantities.
///
/// `Quantity` is nominal: it cannot contain negative values or noncanonical
/// decimal representations, so ledger-domain mistakes are rejected before a
/// value reaches storage or hashing. The name deliberately does not imply
/// currency: an Iroha asset may represent money, a commodity, a vote, or a
/// right.
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Quantity(Numeric);

/// Maximum number of fractional digits accepted for XOR-denominated values.
///
/// XOR's ledger definition permits nanounit precision.  Keeping this limit in
/// the nominal type prevents independent services from silently choosing
/// incompatible fixed-unit conventions.
pub const XOR_QUANTITY_SCALE: u32 = 9;

/// Canonical XOR-denominated quantity.
///
/// This wrapper carries the same exact decimal value as [`Quantity`] while
/// enforcing XOR's scale policy at every construction and wire-decoding
/// boundary. It is intentionally unit-neutral: callers exchange decimal XOR
/// values, never an implicit micro- or nano-unit integer.
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct XorQuantity(Quantity);

/// Define maximum precision and scale for given number.
///
/// Runtime-supplied fractional scales must use [`NumericSpec::try_fractional`];
/// there is deliberately no infallible `From<Option<u32>>` escape hatch.
///
/// ```compile_fail
/// use iroha_primitives::numeric::NumericSpec;
///
/// let _invalid: NumericSpec = Some(29_u32).into();
/// ```
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Hash, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct NumericSpec {
    /// Count of decimal digits in the fractional part.
    /// Currently only positive scale up to 28 decimal points is supported.
    scale: Option<u32>,
}

impl NoritoSerialize for NumericSpec {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), Error> {
        NoritoSerialize::serialize(&self.scale, writer)
    }
}

// Bridge Norito slice-based decoding for Numeric through the same helper used
// by its codec implementation. Decoding the helper directly preserves the
// exact prefix length when Numeric is embedded in a larger packed record.
impl<'a> norito::core::DecodeFromSlice<'a> for Numeric {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (helper, used) =
            <scale_::NumericScaleHelper as norito::core::DecodeFromSlice>::decode_from_slice(
                bytes,
            )?;
        let value = Self::try_new_raw(helper.mantissa, helper.scale)
            .map_err(|error| norito::core::Error::Message(format!("invalid numeric: {error}")))?;
        value
            .validate_decimal()
            .map_err(|error| norito::core::Error::Message(format!("invalid numeric: {error}")))?;
        Ok((value, used))
    }
}

impl<'a> NoritoDeserialize<'a> for NumericSpec {
    fn deserialize(archived: &'a Archived<NumericSpec>) -> Self {
        Self::try_deserialize(archived).expect("invalid numeric specification")
    }

    fn try_deserialize(archived: &'a Archived<NumericSpec>) -> Result<Self, Error> {
        let scale_arch: &Archived<Option<u32>> = archived.cast();
        let scale = <Option<u32> as NoritoDeserialize>::deserialize(scale_arch);
        Self::try_from_scale(scale)
            .map_err(|error| Error::Message(format!("invalid numeric specification: {error}")))
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
                    if scale.is_some() {
                        return Err(json::Error::duplicate_field("scale"));
                    }
                    let value = visitor.parse_value::<Option<u32>>()?;
                    scale = Some(value);
                }
                json::KeyRef::Owned(ref key) if key == "scale" => {
                    if scale.is_some() {
                        return Err(json::Error::duplicate_field("scale"));
                    }
                    let value = visitor.parse_value::<Option<u32>>()?;
                    scale = Some(value);
                }
                _ => visitor.skip_value()?,
            }
        }
        visitor.finish()?;
        NumericSpec::try_from_scale(scale.unwrap_or(None)).map_err(|error| {
            json::Error::InvalidField {
                field: "scale".into(),
                message: error.to_string(),
            }
        })
    }
}

/// Error occurred during creation of [`Numeric`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum NumericError {
    /// Mantissa exceeds allowed range
    MantissaTooLarge,
    /// Scale exceeds allowed range
    ScaleTooLarge,
    /// Malformed: expecting number with optional decimal point (10, 10.02)
    Malformed,
}

/// Consensus-visible failures produced by exact decimal and quantity operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum NumericOperationError {
    /// Canonical result mantissa is outside `-2^511..=2^511-1`
    MantissaOverflow,
    /// Canonical exact result needs a scale greater than 28
    ScaleOverflow,
    /// Divisor is zero
    DivisionByZero,
    /// Exact quotient is a repeating decimal
    RepeatingDecimal,
    /// Exact terminating quotient needs more than 28 fractional digits
    ExactDivisionScaleOverflow,
    /// Requested output scale is outside `0..=28`
    InvalidScale,
    /// Conversion would discard a nonzero fractional part
    InexactConversion,
    /// Decimal is not in its unique canonical representation
    NonCanonical,
    /// Quantity cannot be negative
    NegativeQuantity,
    /// Quantity subtraction would produce a negative result
    QuantityUnderflow,
}

/// Failures produced while multiplying a quantity by aggregate decimal factors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecimalProductError {
    /// Aggregate decimal product contains more than 64 factors.
    #[error("aggregate decimal product contains more than 64 factors")]
    TooManyFactors,
    /// An exact decimal or quantity operation failed.
    #[error(transparent)]
    Numeric(#[from] NumericOperationError),
}

/// Errors raised while constructing or manipulating XOR quantities.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum XorQuantityError {
    /// Arithmetic result exceeds the bounded exact-decimal domain.
    #[error("XOR quantity overflow")]
    Overflow,
    /// Subtraction would produce a negative quantity.
    #[error("XOR quantity underflow")]
    Underflow,
    /// A signed decimal cannot be used as an XOR quantity.
    #[error("XOR quantity cannot be negative")]
    NegativeQuantity,
    /// Projection to an explicitly requested micro-XOR representation is inexact.
    #[error("XOR quantity cannot be represented exactly in micro-XOR")]
    InexactMicroProjection,
    /// XOR values may carry at most nine fractional digits.
    #[error("XOR quantity scale {scale} exceeds maximum {max}")]
    ScaleOverflow {
        /// Observed fractional digit count.
        scale: u32,
        /// Maximum accepted fractional digit count.
        max: u32,
    },
}

/// Deterministic rounding policies supported by decimal operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoundingMode {
    /// Discard the fractional remainder.
    TowardZero = 0,
    /// Increase the absolute value whenever a remainder exists.
    AwayFromZero = 1,
    /// Round toward negative infinity.
    Floor = 2,
    /// Round toward positive infinity.
    Ceil = 3,
    /// Round to nearest, resolving ties to an even output mantissa.
    NearestEven = 4,
    /// Round to nearest, resolving ties away from zero.
    NearestAway = 5,
    /// Round to nearest, resolving ties toward zero.
    NearestTowardZero = 6,
}

/// A division-like work unit reported before arithmetic begins.
///
/// The VM uses these callbacks to debit deterministic logical work before it
/// performs the corresponding division. Widths count 64-bit logical limbs and
/// are never derived from a host bigint implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericWorkStep {
    /// One canonicality probe dividing a nonzero scaled mantissa by ten.
    CanonicalityProbe {
        /// Width of the mantissa before the probe.
        mantissa_limbs: u16,
        /// Scale carried by the value being validated.
        scale: u8,
    },
    /// Scale a conceptual integer by a decimal power before alignment.
    ScaleByPowerOfTen {
        /// Width of the unscaled value.
        value_limbs: u16,
        /// Decimal exponent.
        exponent: u8,
    },
    /// Materialize one unchanged conceptual integer into an owned temporary.
    Materialize {
        /// Width of the value being copied.
        value_limbs: u16,
    },
    /// Negate one conceptual integer.
    Negate {
        /// Operand width.
        value_limbs: u16,
    },
    /// Add two aligned conceptual integers.
    Add {
        /// Left operand width.
        lhs_limbs: u16,
        /// Right operand width.
        rhs_limbs: u16,
    },
    /// Subtract two aligned conceptual integers.
    Subtract {
        /// Left operand width.
        lhs_limbs: u16,
        /// Right operand width.
        rhs_limbs: u16,
    },
    /// Multiply two conceptual integers.
    Multiply {
        /// Left operand width.
        lhs_limbs: u16,
        /// Right operand width.
        rhs_limbs: u16,
    },
    /// One canonical trailing-zero probe and, when divisible, division by ten.
    Normalize {
        /// Width of the mantissa before the division.
        mantissa_limbs: u16,
        /// Scale before the division.
        remaining_scale: u8,
    },
    /// One exact-division attempt at a candidate output scale.
    ExactDivisionAttempt {
        /// Width of the conceptual scaled numerator.
        numerator_limbs: u16,
        /// Width of the conceptual scaled denominator.
        denominator_limbs: u16,
        /// Candidate output scale.
        output_scale: u8,
    },
    /// One Euclidean or prime-factor classification division.
    DivisionClassification {
        /// Width of the dividend before the division.
        dividend_limbs: u16,
        /// Width of the divisor before the division.
        divisor_limbs: u16,
    },
    /// Prepare absolute numerator/denominator values for exact classification.
    DivisionClassificationPrepare {
        /// Width of the numerator copied into the Euclidean state.
        numerator_limbs: u16,
        /// Width of the denominator copied into both the Euclidean state and
        /// the later reduced-denominator state.
        denominator_limbs: u16,
    },
    /// One quotient/remainder operation used for explicit rounding or conversion.
    RoundedDivision {
        /// Width of the conceptual numerator.
        numerator_limbs: u16,
        /// Width of the conceptual denominator.
        denominator_limbs: u16,
        /// Requested output scale.
        output_scale: u8,
    },
    /// Scan and validate one final conceptual value before bounding it.
    Finalize {
        /// Width of the final conceptual value.
        value_limbs: u16,
    },
}

/// Error from an observed numeric operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedNumericError<E> {
    /// Arithmetic or domain failure.
    Numeric(NumericOperationError),
    /// Observer rejected the work before it began.
    Observer(E),
}

/// Mathematical classification of an exact decimal quotient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactDivisionClass {
    /// The quotient has an exact representation at or below scale 28.
    Representable {
        /// Minimum scale of the reduced terminating quotient before final
        /// trailing-zero canonicalization.
        minimum_scale: u8,
    },
    /// The reduced denominator has a prime factor other than two or five.
    Repeating,
    /// The quotient terminates, but its minimum scale is greater than 28.
    ScaleOverflow,
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

    /// Create a canonical numeric from a mantissa and scale.
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

    /// Try to create a canonical numeric from a mantissa and scale.
    ///
    /// # Errors
    /// - if mantissa leaves the signed 512-bit domain
    /// - if the canonical scale remains greater than 28 after trailing-zero removal
    #[inline]
    pub fn try_new<T: Into<BigInt>>(mantissa: T, scale: u32) -> Result<Self, NumericError> {
        let mantissa = mantissa.into();
        if mantissa.is_zero() {
            return Ok(Self { mantissa, scale: 0 });
        }

        let value = Self { mantissa, scale }.trim_trailing_zeros();
        if value.scale > MAX_DECIMAL_SCALE {
            return Err(NumericError::ScaleTooLarge);
        }
        // The input type is already bounded, but keeping the final-width check
        // after normalization mirrors the conceptual-unbounded arithmetic and
        // text-construction paths and makes the consensus ordering explicit.
        if !mantissa_fits_numeric_domain(&value.mantissa) {
            return Err(NumericError::MantissaTooLarge);
        }
        Ok(value)
    }

    /// Construct raw fields for strict decoders that must detect and reject a
    /// noncanonical representation rather than silently normalize it.
    pub(crate) fn try_new_raw<T: Into<BigInt>>(
        mantissa: T,
        scale: u32,
    ) -> Result<Self, NumericError> {
        if scale > MAX_DECIMAL_SCALE {
            return Err(NumericError::ScaleTooLarge);
        }
        let mantissa = mantissa.into();
        if !mantissa_fits_numeric_domain(&mantissa) {
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

    /// Return this value in its unique canonical decimal representation.
    ///
    /// Canonicalization strips fractional trailing zeroes and represents every
    /// zero as `(0, 0)`.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::MantissaOverflow`] if the normalized
    /// result leaves the signed domain. (Values created through [`Numeric`]
    /// cannot currently trigger this error, but conceptual intermediates can.)
    pub fn canonicalize_decimal(self) -> Result<Self, NumericOperationError> {
        infallible_observed(canonical_decimal_from_unbounded_observed(
            self.mantissa.inner().clone(),
            self.scale,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Canonicalize while reporting every normalization division before it begins.
    ///
    /// # Errors
    /// Returns an arithmetic error or propagates an observer rejection before
    /// the corresponding division is performed.
    pub fn canonicalize_decimal_observed<E, F>(
        self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        canonical_decimal_from_unbounded_observed(
            self.mantissa.inner().clone(),
            self.scale,
            observer,
        )
    }

    /// Validate the unique canonical decimal representation.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::NonCanonical`] for zero at nonzero
    /// scale or for a mantissa divisible by ten while scale is nonzero.
    pub fn validate_decimal(&self) -> Result<(), NumericOperationError> {
        infallible_observed(
            self.validate_decimal_observed(&mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Validate canonicality while reporting the divisibility probe first.
    ///
    /// Zero at nonzero scale is rejected without bigint division. Every other
    /// nonzero-scale value emits exactly one [`NumericWorkStep::CanonicalityProbe`]
    /// before its quotient/remainder-by-ten operation.
    ///
    /// # Errors
    /// Returns noncanonical input or propagates an observer rejection before
    /// the divisibility probe begins.
    pub fn validate_decimal_observed<E, F>(
        &self,
        observer: &mut F,
    ) -> Result<(), ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        if self.scale == 0 {
            return Ok(());
        }
        if self.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::NonCanonical,
            ));
        }
        observer(NumericWorkStep::CanonicalityProbe {
            mantissa_limbs: logical_limbs(self.mantissa.inner()),
            scale: u8::try_from(self.scale).expect("validated scale fits u8"),
        })
        .map_err(ObservedNumericError::Observer)?;
        let ten = UnboundedBigInt::from(10_u8);
        let (_, remainder) = quotient_remainder(self.mantissa.inner(), &ten);
        if remainder.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::NonCanonical,
            ));
        }
        Ok(())
    }

    /// Checked canonical decimal negation.
    ///
    /// # Errors
    /// Rejects noncanonical input and a result outside the signed domain.
    pub fn try_decimal_neg(&self) -> Result<Self, NumericOperationError> {
        infallible_observed(
            self.try_decimal_neg_observed(&mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Negate while reporting normalization divisions before work.
    ///
    /// # Errors
    /// Returns an arithmetic error or propagates an observer rejection.
    pub fn try_decimal_neg_observed<E, F>(
        &self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        observer(NumericWorkStep::Negate {
            value_limbs: logical_limbs(self.mantissa.inner()),
        })
        .map_err(ObservedNumericError::Observer)?;
        canonical_decimal_from_unbounded_observed(-self.mantissa.inner(), self.scale, observer)
    }

    /// Add two canonical decimals using an unbounded conceptual intermediate.
    ///
    /// # Errors
    /// Rejects noncanonical operands or an unrepresentable canonical result.
    pub fn try_decimal_add(&self, other: &Self) -> Result<Self, NumericOperationError> {
        infallible_observed(
            self.try_decimal_add_observed(other, &mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Add while reporting normalization divisions before work.
    ///
    /// # Errors
    /// Returns an arithmetic error or propagates an observer rejection.
    pub fn try_decimal_add_observed<E, F>(
        &self,
        other: &Self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        other.validate_decimal_observed(observer)?;
        let target_scale = self.scale.max(other.scale);
        let lhs =
            scale_unbounded_observed(self.mantissa.inner(), target_scale - self.scale, observer)?;
        let rhs =
            scale_unbounded_observed(other.mantissa.inner(), target_scale - other.scale, observer)?;
        observer(NumericWorkStep::Add {
            lhs_limbs: logical_limbs(&lhs),
            rhs_limbs: logical_limbs(&rhs),
        })
        .map_err(ObservedNumericError::Observer)?;
        canonical_decimal_from_unbounded_observed(lhs + rhs, target_scale, observer)
    }

    /// Subtract two canonical decimals using an unbounded conceptual intermediate.
    ///
    /// # Errors
    /// Rejects noncanonical operands or an unrepresentable canonical result.
    pub fn try_decimal_sub(&self, other: &Self) -> Result<Self, NumericOperationError> {
        infallible_observed(
            self.try_decimal_sub_observed(other, &mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Subtract while reporting normalization divisions before work.
    ///
    /// # Errors
    /// Returns an arithmetic error or propagates an observer rejection.
    pub fn try_decimal_sub_observed<E, F>(
        &self,
        other: &Self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        other.validate_decimal_observed(observer)?;
        let target_scale = self.scale.max(other.scale);
        let lhs =
            scale_unbounded_observed(self.mantissa.inner(), target_scale - self.scale, observer)?;
        let rhs =
            scale_unbounded_observed(other.mantissa.inner(), target_scale - other.scale, observer)?;
        observer(NumericWorkStep::Subtract {
            lhs_limbs: logical_limbs(&lhs),
            rhs_limbs: logical_limbs(&rhs),
        })
        .map_err(ObservedNumericError::Observer)?;
        canonical_decimal_from_unbounded_observed(lhs - rhs, target_scale, observer)
    }

    /// Multiply two canonical decimals exactly.
    ///
    /// The conceptual product may be wider than 512 bits and may initially
    /// have scale 56. Trailing decimal zeroes are removed before the final
    /// signed-width and scale bounds are checked.
    ///
    /// # Errors
    /// Rejects noncanonical operands or an unrepresentable canonical result.
    pub fn try_decimal_mul(&self, other: &Self) -> Result<Self, NumericOperationError> {
        infallible_observed(
            self.try_decimal_mul_observed(other, &mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Multiply two decimals while reporting normalization divisions before work.
    ///
    /// # Errors
    /// Returns an arithmetic error or propagates an observer rejection.
    pub fn try_decimal_mul_observed<E, F>(
        &self,
        other: &Self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        other.validate_decimal_observed(observer)?;
        let scale = self
            .scale
            .checked_add(other.scale)
            .ok_or(ObservedNumericError::Numeric(
                NumericOperationError::ScaleOverflow,
            ))?;
        observer(NumericWorkStep::Multiply {
            lhs_limbs: logical_limbs(self.mantissa.inner()),
            rhs_limbs: logical_limbs(other.mantissa.inner()),
        })
        .map_err(ObservedNumericError::Observer)?;
        canonical_decimal_from_unbounded_observed(
            self.mantissa.inner() * other.mantissa.inner(),
            scale,
            observer,
        )
    }

    /// Multiply by one decimal and divide exactly by another using one
    /// conceptual intermediate.
    ///
    /// The mathematical product remains unbounded until after the quotient is
    /// reduced and canonicalized. A wide temporary therefore cannot reject an
    /// exact final result that fits the public decimal domain.
    ///
    /// # Errors
    /// Rejects noncanonical operands, a zero divisor, a repeating or over-scale
    /// quotient, or a final result outside the canonical decimal domain.
    pub fn try_decimal_mul_div_exact(
        &self,
        multiplier: &Self,
        divisor: &Self,
    ) -> Result<Self, NumericOperationError> {
        infallible_observed(self.try_decimal_mul_div_exact_observed(
            multiplier,
            divisor,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Fused exact multiply/divide while reporting every logical work phase
    /// before it begins.
    ///
    /// # Errors
    /// Returns an arithmetic failure or propagates an observer rejection.
    pub fn try_decimal_mul_div_exact_observed<E, F>(
        &self,
        multiplier: &Self,
        divisor: &Self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        multiplier.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }

        let product_scale =
            self.scale
                .checked_add(multiplier.scale)
                .ok_or(ObservedNumericError::Numeric(
                    NumericOperationError::ScaleOverflow,
                ))?;
        observer(NumericWorkStep::Multiply {
            lhs_limbs: logical_limbs(self.mantissa.inner()),
            rhs_limbs: logical_limbs(multiplier.mantissa.inner()),
        })
        .map_err(ObservedNumericError::Observer)?;
        let product = self.mantissa.inner() * multiplier.mantissa.inner();
        let (numerator, denominator) = decimal_product_division_operands_observed(
            &product,
            product_scale,
            divisor,
            0,
            observer,
        )?;
        let class = classify_exact_rational_observed(&numerator, &denominator, observer)?;
        match class {
            ExactDivisionClass::Representable { minimum_scale } => {
                exact_product_division_at_scale_observed(
                    &product,
                    product_scale,
                    divisor,
                    u32::from(minimum_scale),
                    observer,
                )?
                .ok_or_else(|| ObservedNumericError::Numeric(NumericOperationError::NonCanonical))
            }
            ExactDivisionClass::Repeating => Err(ObservedNumericError::Numeric(
                NumericOperationError::RepeatingDecimal,
            )),
            ExactDivisionClass::ScaleOverflow => Err(ObservedNumericError::Numeric(
                NumericOperationError::ExactDivisionScaleOverflow,
            )),
        }
    }

    /// Multiply by one decimal and divide by another with a single rounded
    /// conceptual intermediate.
    ///
    /// This operation is intentionally fused: the mathematical product is
    /// kept unbounded until after division, so a temporary wider than 512 bits
    /// cannot make an otherwise representable final result fail.
    ///
    /// # Errors
    /// Rejects noncanonical operands, a zero divisor, an invalid output scale,
    /// or a final result outside the canonical decimal domain.
    pub fn try_decimal_mul_div_round(
        &self,
        multiplier: &Self,
        divisor: &Self,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        infallible_observed(self.try_decimal_mul_div_round_observed(
            multiplier,
            divisor,
            output_scale,
            mode,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Fused multiply/divide while reporting every logical work phase before
    /// it begins.
    ///
    /// # Errors
    /// Returns an arithmetic failure or propagates an observer rejection.
    pub fn try_decimal_mul_div_round_observed<E, F>(
        &self,
        multiplier: &Self,
        divisor: &Self,
        output_scale: u32,
        mode: RoundingMode,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        multiplier.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::InvalidScale,
            ));
        }
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }

        let product_scale =
            self.scale
                .checked_add(multiplier.scale)
                .ok_or(ObservedNumericError::Numeric(
                    NumericOperationError::ScaleOverflow,
                ))?;
        observer(NumericWorkStep::Multiply {
            lhs_limbs: logical_limbs(self.mantissa.inner()),
            rhs_limbs: logical_limbs(multiplier.mantissa.inner()),
        })
        .map_err(ObservedNumericError::Observer)?;
        let product = self.mantissa.inner() * multiplier.mantissa.inner();

        let (numerator, denominator) = decimal_product_division_operands_observed(
            &product,
            product_scale,
            divisor,
            output_scale,
            observer,
        )?;

        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(&numerator),
            denominator_limbs: logical_limbs(&denominator),
            output_scale: u8::try_from(output_scale).expect("validated scale fits u8"),
        })
        .map_err(ObservedNumericError::Observer)?;
        let quotient = rounded_quotient(&numerator, &denominator, mode);
        canonical_decimal_from_unbounded_observed(quotient, output_scale, observer)
    }

    /// Attempt exact division at one explicit output scale.
    ///
    /// `Ok(None)` means the quotient has a nonzero remainder at this scale.
    /// This method is useful to runtimes that stage one metered attempt at a
    /// time.
    ///
    /// # Errors
    /// Rejects invalid scale, noncanonical operands, division by zero, observer
    /// rejection, or an exact result outside the canonical domain.
    pub fn try_decimal_div_exact_at_scale_observed<E, F>(
        &self,
        divisor: &Self,
        output_scale: u32,
        observer: &mut F,
    ) -> Result<Option<Self>, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::InvalidScale,
            ));
        }
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }
        exact_division_at_scale_observed(self, divisor, output_scale, observer)
    }

    /// Attempt exact division at one explicit output scale without an observer.
    ///
    /// # Errors
    /// See [`Self::try_decimal_div_exact_at_scale_observed`].
    pub fn try_decimal_div_exact_at_scale(
        &self,
        divisor: &Self,
        output_scale: u32,
    ) -> Result<Option<Self>, NumericOperationError> {
        infallible_observed(self.try_decimal_div_exact_at_scale_observed(
            divisor,
            output_scale,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Classify the mathematical quotient after reducing its denominator.
    ///
    /// Every Euclidean and prime-factor division is reported to `observer`
    /// before it begins.
    ///
    /// # Errors
    /// Rejects noncanonical operands, division by zero, or observer rejection.
    pub fn classify_exact_division_observed<E, F>(
        &self,
        divisor: &Self,
        observer: &mut F,
    ) -> Result<ExactDivisionClass, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }
        classify_exact_division_inner(self, divisor, observer)
    }

    /// Classify an exact quotient without an observer.
    ///
    /// # Errors
    /// Rejects noncanonical operands or division by zero.
    pub fn classify_exact_division(
        &self,
        divisor: &Self,
    ) -> Result<ExactDivisionClass, NumericOperationError> {
        infallible_observed(self.classify_exact_division_observed(divisor, &mut |_| {
            Ok::<_, core::convert::Infallible>(())
        }))
    }

    /// Divide exactly, selecting the smallest representable output scale.
    ///
    /// The reduced denominator is classified first. A terminating quotient is
    /// then attempted exactly once at its proven minimum scale; repeating and
    /// over-scale quotients fail without speculative division attempts.
    ///
    /// # Errors
    /// Returns the precise arithmetic failure or an observer rejection.
    pub fn try_decimal_div_exact_observed<E, F>(
        &self,
        divisor: &Self,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }
        let class = classify_exact_division_inner(self, divisor, observer)?;
        match class {
            ExactDivisionClass::Representable { minimum_scale } => {
                exact_division_at_scale_observed(self, divisor, u32::from(minimum_scale), observer)?
                    .ok_or_else(|| {
                        // Classification reduced the exact mathematical quotient
                        // and proved this scale sufficient. A remainder here would
                        // indicate an internal arithmetic invariant violation, not
                        // a user-triggerable inexact result.
                        ObservedNumericError::Numeric(NumericOperationError::NonCanonical)
                    })
            }
            ExactDivisionClass::Repeating => Err(ObservedNumericError::Numeric(
                NumericOperationError::RepeatingDecimal,
            )),
            ExactDivisionClass::ScaleOverflow => Err(ObservedNumericError::Numeric(
                NumericOperationError::ExactDivisionScaleOverflow,
            )),
        }
    }

    /// Divide exactly without an observer.
    ///
    /// # Errors
    /// See [`Self::try_decimal_div_exact_observed`].
    pub fn try_decimal_div_exact(&self, divisor: &Self) -> Result<Self, NumericOperationError> {
        infallible_observed(self.try_decimal_div_exact_observed(divisor, &mut |_| {
            Ok::<_, core::convert::Infallible>(())
        }))
    }

    /// Divide with an explicit output scale and deterministic rounding mode.
    ///
    /// # Errors
    /// Returns the precise arithmetic failure or an observer rejection.
    pub fn try_decimal_div_round_observed<E, F>(
        &self,
        divisor: &Self,
        output_scale: u32,
        mode: RoundingMode,
        observer: &mut F,
    ) -> Result<Self, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        divisor.validate_decimal_observed(observer)?;
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::InvalidScale,
            ));
        }
        if divisor.mantissa.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::DivisionByZero,
            ));
        }
        let (numerator, denominator) =
            decimal_division_operands_observed(self, divisor, output_scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(&numerator),
            denominator_limbs: logical_limbs(&denominator),
            output_scale: u8::try_from(output_scale).expect("validated scale fits u8"),
        })
        .map_err(ObservedNumericError::Observer)?;
        let quotient = rounded_quotient(&numerator, &denominator, mode);
        canonical_decimal_from_unbounded_observed(quotient, output_scale, observer)
    }

    /// Divide with explicit rounding without an observer.
    ///
    /// # Errors
    /// See [`Self::try_decimal_div_round_observed`].
    pub fn try_decimal_div_round(
        &self,
        divisor: &Self,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        infallible_observed(self.try_decimal_div_round_observed(
            divisor,
            output_scale,
            mode,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Convert to an integer only when the decimal has no fractional value.
    ///
    /// # Errors
    /// Rejects noncanonical input or a nonzero fractional remainder.
    pub fn try_decimal_to_int_exact(&self) -> Result<BigInt, NumericOperationError> {
        infallible_observed(
            self.try_decimal_to_int_exact_observed(&mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Convert exactly while reporting the quotient/remainder work first.
    ///
    /// # Errors
    /// Returns the precise conversion failure or an observer rejection.
    pub fn try_decimal_to_int_exact_observed<E, F>(
        &self,
        observer: &mut F,
    ) -> Result<BigInt, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        if self.scale == 0 {
            observer(NumericWorkStep::Finalize {
                value_limbs: logical_limbs(self.mantissa.inner()),
            })
            .map_err(ObservedNumericError::Observer)?;
            return Ok(self.mantissa.clone());
        }
        let divisor = scale_unbounded_observed(&UnboundedBigInt::one(), self.scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(self.mantissa.inner()),
            denominator_limbs: logical_limbs(&divisor),
            output_scale: 0,
        })
        .map_err(ObservedNumericError::Observer)?;
        let (quotient, remainder) = quotient_remainder(self.mantissa.inner(), &divisor);
        if !remainder.is_zero() {
            return Err(ObservedNumericError::Numeric(
                NumericOperationError::InexactConversion,
            ));
        }
        finalize_bigint_observed(quotient, observer)
    }

    /// Convert to an integer by truncating toward zero.
    ///
    /// # Errors
    /// Rejects noncanonical input or an unrepresentable result.
    pub fn decimal_to_int_trunc(&self) -> Result<BigInt, NumericOperationError> {
        infallible_observed(
            self.decimal_to_int_trunc_observed(&mut |_| Ok::<_, core::convert::Infallible>(())),
        )
    }

    /// Truncate to an integer while reporting the division before work.
    ///
    /// # Errors
    /// Returns the precise conversion failure or an observer rejection.
    pub fn decimal_to_int_trunc_observed<E, F>(
        &self,
        observer: &mut F,
    ) -> Result<BigInt, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        if self.scale == 0 {
            observer(NumericWorkStep::Finalize {
                value_limbs: logical_limbs(self.mantissa.inner()),
            })
            .map_err(ObservedNumericError::Observer)?;
            return Ok(self.mantissa.clone());
        }
        let divisor = scale_unbounded_observed(&UnboundedBigInt::one(), self.scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(self.mantissa.inner()),
            denominator_limbs: logical_limbs(&divisor),
            output_scale: 0,
        })
        .map_err(ObservedNumericError::Observer)?;
        finalize_bigint_observed(
            quotient_remainder(self.mantissa.inner(), &divisor).0,
            observer,
        )
    }

    /// Convert to an integer using an explicit deterministic rounding mode.
    ///
    /// # Errors
    /// Rejects noncanonical input or an unrepresentable rounded result.
    pub fn decimal_to_int_round(
        &self,
        mode: RoundingMode,
    ) -> Result<BigInt, NumericOperationError> {
        infallible_observed(
            self.decimal_to_int_round_observed(mode, &mut |_| {
                Ok::<_, core::convert::Infallible>(())
            }),
        )
    }

    /// Round to an integer while reporting the division before work.
    ///
    /// # Errors
    /// Returns the precise conversion failure or an observer rejection.
    pub fn decimal_to_int_round_observed<E, F>(
        &self,
        mode: RoundingMode,
        observer: &mut F,
    ) -> Result<BigInt, ObservedNumericError<E>>
    where
        F: FnMut(NumericWorkStep) -> Result<(), E>,
    {
        self.validate_decimal_observed(observer)?;
        if self.scale == 0 {
            observer(NumericWorkStep::Finalize {
                value_limbs: logical_limbs(self.mantissa.inner()),
            })
            .map_err(ObservedNumericError::Observer)?;
            return Ok(self.mantissa.clone());
        }
        let divisor = scale_unbounded_observed(&UnboundedBigInt::one(), self.scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(self.mantissa.inner()),
            denominator_limbs: logical_limbs(&divisor),
            output_scale: 0,
        })
        .map_err(ObservedNumericError::Observer)?;
        finalize_bigint_observed(
            rounded_quotient(self.mantissa.inner(), &divisor, mode),
            observer,
        )
    }

    /// Checked addition. Computes `self + other`, returning `None` if overflow occurred
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the consuming API deliberately mirrors primitive checked operator methods"
    )]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.try_decimal_add(&other).ok()
    }

    /// Checked subtraction. Computes `self - other`, returning `None` if overflow occurred
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the consuming API deliberately mirrors primitive checked operator methods"
    )]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.try_decimal_sub(&other).ok()
    }

    /// Quantize this value to at most `output_scale` fractional digits using
    /// an explicit deterministic rounding mode.
    ///
    /// Canonical values do not preserve insignificant trailing zeroes, so a
    /// requested scale greater than the current canonical scale returns the
    /// same value rather than manufacturing a second representation.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::InvalidScale`] above scale 28 or a
    /// canonical result-domain failure.
    pub fn try_quantize(
        &self,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        self.validate_decimal()?;
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(NumericOperationError::InvalidScale);
        }
        if output_scale >= self.scale {
            return Ok(self.clone());
        }
        let factor = UnboundedBigInt::from(10_u8).pow(self.scale - output_scale);
        infallible_observed(canonical_decimal_from_unbounded_observed(
            rounded_quotient(self.mantissa.inner(), &factor, mode),
            output_scale,
            &mut |_| Ok::<_, core::convert::Infallible>(()),
        ))
    }

    /// Quantize to an optional asset scale with an explicit rounding mode.
    ///
    /// An unconstrained specification returns the value unchanged.
    ///
    /// # Errors
    /// Returns the same failures as [`Self::try_quantize`].
    pub fn try_quantize_to_spec(
        &self,
        spec: NumericSpec,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        spec.scale
            .map_or_else(|| Ok(self.clone()), |scale| self.try_quantize(scale, mode))
    }

    /// Convert [`Numeric`] to [`f64`] with possible loss of precision.
    ///
    /// This conversion is intended only for non-consensus consumers such as
    /// telemetry and approximate presentation metrics. Consensus and ledger
    /// code must retain the exact decimal representation.
    #[must_use]
    pub fn to_f64_lossy(&self) -> f64 {
        self.to_string()
            .parse()
            .expect("every bounded canonical Numeric value is representable as a finite f64")
    }

    /// Check if number is zero
    pub fn is_zero(&self) -> bool {
        self.mantissa.is_zero()
    }
}

impl Quantity {
    /// Zero quantity.
    #[must_use]
    pub fn zero() -> Self {
        Self(Numeric::zero())
    }

    /// One quantity.
    #[must_use]
    pub fn one() -> Self {
        Self(Numeric::one())
    }

    /// Canonicalize and validate a decimal as a non-negative quantity.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::NegativeQuantity`] for a negative
    /// value or a canonicalization domain failure.
    pub fn try_from_numeric(value: Numeric) -> Result<Self, NumericOperationError> {
        let value = value.canonicalize_decimal()?;
        Self::from_canonical_numeric(value)
    }

    /// Wrap an already canonical decimal as a non-negative quantity.
    ///
    /// Every publicly constructible [`Numeric`] already carries the canonical
    /// representation invariant. Strict wire decoders validate that invariant
    /// before constructing `Numeric`, so this boundary only needs to enforce
    /// the additional nominal sign rule and performs no hidden bigint pass.
    ///
    /// # Errors
    /// Rejects a negative input.
    pub fn from_canonical_numeric(value: Numeric) -> Result<Self, NumericOperationError> {
        if value.mantissa.is_negative() {
            return Err(NumericOperationError::NegativeQuantity);
        }
        Ok(Self(value))
    }

    /// Borrow the canonical decimal representation.
    #[must_use]
    pub fn as_numeric(&self) -> &Numeric {
        &self.0
    }

    /// Borrow the signed-domain mantissa (always non-negative for a quantity).
    #[must_use]
    pub fn mantissa(&self) -> &BigInt {
        self.0.mantissa()
    }

    /// Return the canonical decimal scale.
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.0.scale()
    }

    /// Return whether this quantity is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Consume this quantity and return its canonical decimal representation.
    #[must_use]
    pub fn into_numeric(self) -> Numeric {
        self.0
    }

    /// Add two quantities exactly.
    ///
    /// # Errors
    /// Returns a canonical result-domain failure.
    pub fn try_add(&self, other: &Self) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_add(&other.0)?)
    }

    /// Alias for [`Self::try_add`] emphasizing checked domain arithmetic.
    ///
    /// # Errors
    /// Returns a canonical result-domain failure.
    pub fn checked_add(&self, other: &Self) -> Result<Self, NumericOperationError> {
        self.try_add(other)
    }

    /// Subtract quantities, rejecting a negative result as underflow.
    ///
    /// # Errors
    /// Returns [`NumericOperationError::QuantityUnderflow`] when `other` is
    /// greater than `self`, or another result-domain failure.
    pub fn try_sub(&self, other: &Self) -> Result<Self, NumericOperationError> {
        let result = self.0.try_decimal_sub(&other.0)?;
        if result.mantissa().is_negative() {
            return Err(NumericOperationError::QuantityUnderflow);
        }
        Self::from_canonical_numeric(result)
    }

    /// Alias for [`Self::try_sub`] emphasizing checked domain arithmetic.
    ///
    /// # Errors
    /// Returns quantity underflow or another canonical result-domain failure.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, NumericOperationError> {
        self.try_sub(other)
    }

    /// Multiply a quantity by an exact decimal factor.
    ///
    /// # Errors
    /// Rejects negative or unrepresentable results.
    pub fn try_mul_decimal(&self, factor: &Numeric) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_mul(factor)?)
    }

    /// Multiply and divide exactly with one unbounded conceptual intermediate.
    ///
    /// # Errors
    /// Returns the precise exact-decimal failure and rejects negative results.
    pub fn try_mul_div_decimal_exact(
        &self,
        multiplier: &Numeric,
        divisor: &Numeric,
    ) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_mul_div_exact(multiplier, divisor)?)
    }

    /// Multiply and divide with one unbounded conceptual intermediate and an
    /// explicit final rounding policy.
    ///
    /// # Errors
    /// Returns the precise decimal failure and rejects negative results.
    pub fn try_mul_div_decimal_round(
        &self,
        multiplier: &Numeric,
        divisor: &Numeric,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_mul_div_round(
            multiplier,
            divisor,
            output_scale,
            mode,
        )?)
    }

    /// Compute a weighted average with unbounded conceptual intermediates.
    ///
    /// Each input contributes `value * weight` to the numerator. Products and
    /// their sum are not narrowed to the public 512-bit mantissa domain before
    /// division, so a representable average cannot fail merely because an
    /// intermediate weighted sum is wider than the final result. Zero-weight
    /// entries are accepted but do not make an otherwise empty denominator
    /// valid.
    ///
    /// This is a domain-aggregation primitive rather than a Kotodama numeric
    /// operator; VM opcodes continue to use the observed scalar operations.
    ///
    /// # Errors
    /// Rejects an output scale above 28, a zero total weight, a noncanonical
    /// input, or a final result outside the canonical quantity domain.
    pub fn try_weighted_average_round<'a, I>(
        values: I,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError>
    where
        I: IntoIterator<Item = (&'a Self, u64)>,
    {
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(NumericOperationError::InvalidScale);
        }

        let mut common_scale = 0_u32;
        let mut weighted_sum = UnboundedBigInt::zero();
        let mut total_weight = UnboundedBigInt::zero();
        for (value, weight) in values {
            value.0.validate_decimal()?;
            if weight == 0 {
                continue;
            }

            if value.scale() > common_scale {
                weighted_sum *= UnboundedBigInt::from(10_u8).pow(value.scale() - common_scale);
                common_scale = value.scale();
            }
            let mut contribution = value.mantissa().inner() * UnboundedBigInt::from(weight);
            if value.scale() < common_scale {
                contribution *= UnboundedBigInt::from(10_u8).pow(common_scale - value.scale());
            }
            weighted_sum += contribution;
            total_weight += UnboundedBigInt::from(weight);
        }

        if total_weight.is_zero() {
            return Err(NumericOperationError::DivisionByZero);
        }

        let (numerator, denominator) = if output_scale >= common_scale {
            (
                weighted_sum * UnboundedBigInt::from(10_u8).pow(output_scale - common_scale),
                total_weight,
            )
        } else {
            (
                weighted_sum,
                total_weight * UnboundedBigInt::from(10_u8).pow(common_scale - output_scale),
            )
        };
        let quotient = rounded_quotient(&numerator, &denominator, mode);
        Self::from_canonical_numeric(infallible_observed(
            canonical_decimal_from_unbounded_observed(quotient, output_scale, &mut |_| {
                Ok::<_, core::convert::Infallible>(())
            }),
        )?)
    }

    /// Multiply this quantity by a sequence of decimal factors using one
    /// unbounded conceptual product.
    ///
    /// This helper is for domain formulas whose factors are defined as one
    /// aggregate product. It deliberately differs from evaluating a source
    /// expression as repeated `quantity * decimal` operators, where every
    /// operator produces and checks its own public-domain result. Exact
    /// trailing-zero normalization is allowed between factors because it does
    /// not change the mathematical product.
    ///
    /// # Errors
    /// Rejects more than 64 factors, a noncanonical factor, or a canonical
    /// final result outside the decimal scale, signed-mantissa, or non-negative
    /// quantity domain.
    pub fn try_product_decimals<'a, I>(&self, factors: I) -> Result<Self, DecimalProductError>
    where
        I: IntoIterator<Item = &'a Numeric>,
    {
        self.0.validate_decimal()?;
        let ten = UnboundedBigInt::from(10_u8);
        let mut product = self.mantissa().inner().clone();
        let mut scale = u128::from(self.scale());

        for (index, factor) in factors.into_iter().enumerate() {
            if index >= MAX_DECIMAL_PRODUCT_FACTORS {
                return Err(DecimalProductError::TooManyFactors);
            }
            factor.validate_decimal()?;
            product *= factor.mantissa().inner();
            if product.is_zero() {
                scale = 0;
                continue;
            }
            scale = scale
                .checked_add(u128::from(factor.scale()))
                .ok_or(NumericOperationError::ScaleOverflow)?;
            while scale > 0 {
                let (quotient, remainder) = quotient_remainder(&product, &ten);
                if !remainder.is_zero() {
                    break;
                }
                product = quotient;
                scale -= 1;
            }
        }

        if scale > u128::from(MAX_DECIMAL_SCALE) {
            return Err(NumericOperationError::ScaleOverflow.into());
        }
        let scale = u32::try_from(scale).expect("validated decimal scale fits u32");
        Ok(Self::from_canonical_numeric(infallible_observed(
            canonical_decimal_from_unbounded_observed(product, scale, &mut |_| {
                Ok::<_, core::convert::Infallible>(())
            }),
        )?)?)
    }

    /// Multiply this quantity by decimal factors as one unbounded conceptual
    /// product, then round the aggregate once at `output_scale`.
    ///
    /// This is the rounded counterpart to [`Self::try_product_decimals`].
    /// Intermediate products are never narrowed to the public mantissa or
    /// scale domain, so deterministic final rounding is independent of factor
    /// grouping.
    ///
    /// # Errors
    /// Rejects more than 64 factors, a scale above 28, a noncanonical factor,
    /// an aggregate scale that cannot be bounded safely, or a rounded result
    /// outside the canonical non-negative quantity domain.
    pub fn try_product_decimals_round<'a, I>(
        &self,
        factors: I,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, DecimalProductError>
    where
        I: IntoIterator<Item = &'a Numeric>,
    {
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(NumericOperationError::InvalidScale.into());
        }
        self.0.validate_decimal()?;
        let ten = UnboundedBigInt::from(10_u8);
        let mut product = self.mantissa().inner().clone();
        let mut scale = u128::from(self.scale());

        for (index, factor) in factors.into_iter().enumerate() {
            if index >= MAX_DECIMAL_PRODUCT_FACTORS {
                return Err(DecimalProductError::TooManyFactors);
            }
            factor.validate_decimal()?;
            product *= factor.mantissa().inner();
            if product.is_zero() {
                scale = 0;
                continue;
            }
            scale = scale
                .checked_add(u128::from(factor.scale()))
                .ok_or(NumericOperationError::ScaleOverflow)?;
            while scale > 0 {
                let (quotient, remainder) = quotient_remainder(&product, &ten);
                if !remainder.is_zero() {
                    break;
                }
                product = quotient;
                scale -= 1;
            }
        }

        let output_scale_wide = u128::from(output_scale);
        let rounded = if scale > output_scale_wide {
            let reduction = u32::try_from(scale - output_scale_wide)
                .map_err(|_| NumericOperationError::ScaleOverflow)?;
            rounded_quotient(&product, &ten.pow(reduction), mode)
        } else {
            let expansion = u32::try_from(output_scale_wide - scale)
                .expect("validated output scale difference fits u32");
            product * ten.pow(expansion)
        };
        Ok(Self::from_canonical_numeric(infallible_observed(
            canonical_decimal_from_unbounded_observed(rounded, output_scale, &mut |_| {
                Ok::<_, core::convert::Infallible>(())
            }),
        )?)?)
    }

    /// Compare `self * self_multiplier` with `other * other_multiplier`.
    ///
    /// Products and decimal alignment are conceptual unbounded intermediates,
    /// so comparisons at the public mantissa boundary remain exact instead of
    /// failing merely because one side cannot be materialized as a standalone
    /// [`Quantity`].
    #[must_use]
    pub fn cmp_mul_u64(
        &self,
        self_multiplier: u64,
        other: &Self,
        other_multiplier: u64,
    ) -> Ordering {
        let common_scale = self.scale().max(other.scale());
        let lhs = self.mantissa().inner()
            * UnboundedBigInt::from(self_multiplier)
            * UnboundedBigInt::from(10_u8).pow(common_scale - self.scale());
        let rhs = other.mantissa().inner()
            * UnboundedBigInt::from(other_multiplier)
            * UnboundedBigInt::from(10_u8).pow(common_scale - other.scale());
        lhs.cmp(&rhs)
    }

    /// Divide a quantity by a decimal factor exactly.
    ///
    /// # Errors
    /// Returns the precise decimal failure and rejects negative results.
    pub fn try_div_decimal_exact(&self, divisor: &Numeric) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_div_exact(divisor)?)
    }

    /// Divide a quantity by a decimal factor with explicit rounding.
    ///
    /// # Errors
    /// Returns the precise decimal failure and rejects negative results.
    pub fn try_div_decimal_round(
        &self,
        divisor: &Numeric,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Self, NumericOperationError> {
        Self::from_canonical_numeric(self.0.try_decimal_div_round(divisor, output_scale, mode)?)
    }

    /// Compute the exact dimensionless ratio of two quantities.
    ///
    /// # Errors
    /// Returns the precise exact-decimal division failure.
    pub fn try_ratio_exact(&self, divisor: &Self) -> Result<Numeric, NumericOperationError> {
        self.0.try_decimal_div_exact(&divisor.0)
    }

    /// Compute a rounded dimensionless ratio of two quantities.
    ///
    /// # Errors
    /// Returns the precise rounded-decimal division failure.
    pub fn try_ratio_round(
        &self,
        divisor: &Self,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<Numeric, NumericOperationError> {
        self.0.try_decimal_div_round(&divisor.0, output_scale, mode)
    }
}

impl Default for Quantity {
    fn default() -> Self {
        Self::zero()
    }
}

impl PartialOrd for Quantity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Quantity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl core::fmt::Display for Quantity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::str::FromStr for Quantity {
    type Err = NumericOperationError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let value = source.parse::<Numeric>().map_err(|error| match error {
            NumericError::ScaleTooLarge => NumericOperationError::ScaleOverflow,
            NumericError::MantissaTooLarge | NumericError::Malformed => {
                NumericOperationError::MantissaOverflow
            }
        })?;
        Self::try_from_numeric(value)
    }
}

impl TryFrom<Numeric> for Quantity {
    type Error = NumericOperationError;

    fn try_from(value: Numeric) -> Result<Self, Self::Error> {
        Self::try_from_numeric(value)
    }
}

impl From<Quantity> for Numeric {
    fn from(value: Quantity) -> Self {
        value.0
    }
}

impl From<u32> for Quantity {
    fn from(value: u32) -> Self {
        Self(Numeric::from(value))
    }
}

impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self(Numeric::from(value))
    }
}

impl From<u128> for Quantity {
    fn from(value: u128) -> Self {
        Self(Numeric::new(BigInt::from(value), 0))
    }
}

impl NoritoSerialize for Quantity {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for Quantity {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("invalid canonical quantity")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        let numeric = Numeric::try_deserialize(archived.cast::<Numeric>())?;
        Self::from_canonical_numeric(numeric)
            .map_err(|error| Error::Message(format!("invalid quantity: {error}")))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for Quantity {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (numeric, used) = <Numeric as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        let quantity = Self::from_canonical_numeric(numeric)
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        Ok((quantity, used))
    }
}

impl FastJsonWrite for Quantity {
    fn write_json(&self, out: &mut String) {
        json::write_json_string(&self.to_string(), out);
    }
}

impl JsonDeserialize for Quantity {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        let parsed = value
            .parse::<Self>()
            .map_err(|error| json::Error::InvalidField {
                field: "quantity".into(),
                message: format!("invalid quantity `{value}`: {error}"),
            })?;
        if parsed.to_string() != value {
            return Err(json::Error::InvalidField {
                field: "quantity".into(),
                message: format!("noncanonical quantity `{value}`"),
            });
        }
        Ok(parsed)
    }
}

impl XorQuantity {
    /// Validate and wrap a canonical non-negative XOR quantity.
    ///
    /// # Errors
    /// Rejects values carrying more than [`XOR_QUANTITY_SCALE`] fractional
    /// digits.
    pub fn try_from_quantity(quantity: Quantity) -> Result<Self, XorQuantityError> {
        if quantity.scale() > XOR_QUANTITY_SCALE {
            return Err(XorQuantityError::ScaleOverflow {
                scale: quantity.scale(),
                max: XOR_QUANTITY_SCALE,
            });
        }
        Ok(Self(quantity))
    }

    /// Construct from an exact micro-XOR projection.
    ///
    /// This is an explicit adapter for versioned external formats that define
    /// their value in micro-XOR. New public APIs should accept decimal XOR
    /// quantities directly.
    ///
    /// # Errors
    /// Returns an error if construction exceeds the bounded decimal domain.
    pub fn try_from_micro(micro: u128) -> Result<Self, XorQuantityError> {
        let numeric = Numeric::try_new(micro, 6).map_err(|_| XorQuantityError::Overflow)?;
        Quantity::from_canonical_numeric(numeric)
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }

    /// Borrow the canonical quantity.
    #[must_use]
    pub const fn as_quantity(&self) -> &Quantity {
        &self.0
    }

    /// Consume the nominal wrapper.
    #[must_use]
    pub fn into_quantity(self) -> Quantity {
        self.0
    }

    /// Return the zero amount.
    #[must_use]
    pub fn zero() -> Self {
        Self(Quantity::zero())
    }

    /// Whether the amount is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Project to micro-XOR exactly.
    ///
    /// This is an explicit adapter for versioned external formats. It never
    /// rounds or saturates.
    ///
    /// # Errors
    /// Rejects sub-micro precision and values wider than `u128`.
    pub fn try_to_micro(&self) -> Result<u128, XorQuantityError> {
        let scaled = self
            .0
            .try_mul_decimal(&Numeric::from(1_000_000_u64))
            .map_err(XorQuantityError::from)?;
        if scaled.scale() != 0 {
            return Err(XorQuantityError::InexactMicroProjection);
        }
        scaled
            .as_numeric()
            .try_mantissa_u128()
            .ok_or(XorQuantityError::Overflow)
    }

    /// Add two XOR amounts exactly.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, XorQuantityError> {
        self.0
            .checked_add(&rhs.0)
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }

    /// Subtract two XOR amounts exactly.
    ///
    /// # Errors
    /// Returns underflow when `rhs` is greater than `self`.
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, XorQuantityError> {
        self.0
            .checked_sub(&rhs.0)
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }

    /// Return the smaller amount.
    #[must_use]
    pub fn min(&self, other: &Self) -> Self {
        if self <= other {
            self.clone()
        } else {
            other.clone()
        }
    }

    /// Multiply by an unsigned 64-bit scalar.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_u64(&self, multiplier: u64) -> Result<Self, XorQuantityError> {
        self.checked_mul_u128(u128::from(multiplier))
    }

    /// Multiply by an unsigned 128-bit scalar.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_u128(&self, multiplier: u128) -> Result<Self, XorQuantityError> {
        self.0
            .try_mul_decimal(&Numeric::new(multiplier, 0))
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }

    /// Divide by a positive integer with explicit output scale and rounding.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_div_u64_round(
        &self,
        divisor: core::num::NonZeroU64,
        output_scale: u32,
        rounding: RoundingMode,
    ) -> Result<Self, XorQuantityError> {
        if output_scale > XOR_QUANTITY_SCALE {
            return Err(XorQuantityError::ScaleOverflow {
                scale: output_scale,
                max: XOR_QUANTITY_SCALE,
            });
        }
        self.0
            .try_div_decimal_round(&Numeric::from(divisor.get()), output_scale, rounding)
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }

    /// Apply a basis-point ratio (`basis_points / 10_000`) toward zero.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_basis_points(&self, basis_points: u16) -> Result<Self, XorQuantityError> {
        self.checked_mul_basis_points_u32(u32::from(basis_points))
    }

    /// Apply a basis-point ratio whose numerator may exceed `u16`.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_basis_points_u32(
        &self,
        basis_points: u32,
    ) -> Result<Self, XorQuantityError> {
        self.checked_mul_ratio(
            u64::from(basis_points),
            core::num::NonZeroU64::new(10_000).expect("basis-point denominator is non-zero"),
        )
    }

    /// Multiply by an unsigned rational factor, rounding toward zero at XOR's
    /// maximum scale.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_ratio(
        &self,
        numerator: u64,
        denominator: core::num::NonZeroU64,
    ) -> Result<Self, XorQuantityError> {
        self.checked_mul_ratio_round(
            numerator,
            denominator,
            XOR_QUANTITY_SCALE,
            RoundingMode::TowardZero,
        )
    }

    /// Multiply by an unsigned rational factor using an explicit rounding
    /// policy and output scale.
    ///
    /// # Errors
    /// Returns a bounded-domain or XOR-scale failure.
    pub fn checked_mul_ratio_round(
        &self,
        numerator: u64,
        denominator: core::num::NonZeroU64,
        output_scale: u32,
        rounding: RoundingMode,
    ) -> Result<Self, XorQuantityError> {
        if output_scale > XOR_QUANTITY_SCALE {
            return Err(XorQuantityError::ScaleOverflow {
                scale: output_scale,
                max: XOR_QUANTITY_SCALE,
            });
        }
        self.0
            .try_mul_div_decimal_round(
                &Numeric::from(numerator),
                &Numeric::from(denominator.get()),
                output_scale,
                rounding,
            )
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }
}

impl Default for XorQuantity {
    fn default() -> Self {
        Self::zero()
    }
}

impl PartialOrd for XorQuantity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for XorQuantity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl core::fmt::Display for XorQuantity {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl core::str::FromStr for XorQuantity {
    type Err = XorQuantityError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        source
            .parse::<Quantity>()
            .map_err(XorQuantityError::from)
            .and_then(Self::try_from_quantity)
    }
}

impl TryFrom<Quantity> for XorQuantity {
    type Error = XorQuantityError;

    fn try_from(value: Quantity) -> Result<Self, Self::Error> {
        Self::try_from_quantity(value)
    }
}

impl From<XorQuantity> for Quantity {
    fn from(value: XorQuantity) -> Self {
        value.0
    }
}

impl NoritoSerialize for XorQuantity {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for XorQuantity {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("invalid canonical XOR quantity")
    }

    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        let quantity = Quantity::try_deserialize(archived.cast::<Quantity>())?;
        Self::try_from_quantity(quantity)
            .map_err(|error| Error::Message(format!("invalid XOR quantity: {error}")))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for XorQuantity {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (quantity, used) =
            <Quantity as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        let value = Self::try_from_quantity(quantity)
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        Ok((value, used))
    }
}

impl FastJsonWrite for XorQuantity {
    fn write_json(&self, out: &mut String) {
        self.0.write_json(out);
    }
}

impl JsonDeserialize for XorQuantity {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let quantity = Quantity::json_deserialize(parser)?;
        Self::try_from_quantity(quantity).map_err(|error| json::Error::InvalidField {
            field: "xor_quantity".into(),
            message: error.to_string(),
        })
    }
}

impl From<NumericOperationError> for XorQuantityError {
    fn from(value: NumericOperationError) -> Self {
        match value {
            NumericOperationError::QuantityUnderflow => Self::Underflow,
            NumericOperationError::NegativeQuantity => Self::NegativeQuantity,
            NumericOperationError::InexactConversion => Self::InexactMicroProjection,
            NumericOperationError::MantissaOverflow
            | NumericOperationError::ScaleOverflow
            | NumericOperationError::DivisionByZero
            | NumericOperationError::RepeatingDecimal
            | NumericOperationError::ExactDivisionScaleOverflow
            | NumericOperationError::InvalidScale
            | NumericOperationError::NonCanonical => Self::Overflow,
        }
    }
}

fn infallible_observed<T>(
    result: Result<T, ObservedNumericError<core::convert::Infallible>>,
) -> Result<T, NumericOperationError> {
    match result {
        Ok(value) => Ok(value),
        Err(ObservedNumericError::Numeric(error)) => Err(error),
        Err(ObservedNumericError::Observer(never)) => match never {},
    }
}

fn logical_limbs(value: &UnboundedBigInt) -> u16 {
    let bits = value.bits();
    let limbs = bits.max(1).div_ceil(64);
    u16::try_from(limbs).unwrap_or(u16::MAX)
}

fn quotient_remainder(
    numerator: &UnboundedBigInt,
    denominator: &UnboundedBigInt,
) -> (UnboundedBigInt, UnboundedBigInt) {
    let quotient = numerator / denominator;
    let remainder = numerator - (&quotient * denominator);
    (quotient, remainder)
}

fn canonical_decimal_from_unbounded_observed<E, F>(
    mut mantissa: UnboundedBigInt,
    mut scale: u32,
    observer: &mut F,
) -> Result<Numeric, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let ten = UnboundedBigInt::from(10_u8);
    if mantissa.is_zero() {
        // Zero has a dedicated canonicalization rule and needs no
        // divide-by-ten probe: `(0, s)` becomes `(0, 0)` directly.
        scale = 0;
    }
    while scale > 0 {
        observer(NumericWorkStep::Normalize {
            mantissa_limbs: logical_limbs(&mantissa),
            remaining_scale: u8::try_from(scale).unwrap_or(u8::MAX),
        })
        .map_err(ObservedNumericError::Observer)?;
        let (quotient, remainder) = quotient_remainder(&mantissa, &ten);
        if !remainder.is_zero() {
            break;
        }
        mantissa = quotient;
        scale -= 1;
    }
    if scale > MAX_DECIMAL_SCALE {
        return Err(ObservedNumericError::Numeric(
            NumericOperationError::ScaleOverflow,
        ));
    }
    let mantissa = finalize_bigint_observed(mantissa, observer)?;
    Numeric::try_new_raw(mantissa, scale).map_err(|error| {
        ObservedNumericError::Numeric(match error {
            NumericError::MantissaTooLarge => NumericOperationError::MantissaOverflow,
            NumericError::ScaleTooLarge => NumericOperationError::ScaleOverflow,
            NumericError::Malformed => unreachable!("structured numeric fields are well formed"),
        })
    })
}

fn finalize_bigint_observed<E, F>(
    value: UnboundedBigInt,
    observer: &mut F,
) -> Result<BigInt, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    observer(NumericWorkStep::Finalize {
        value_limbs: logical_limbs(&value),
    })
    .map_err(ObservedNumericError::Observer)?;
    BigInt::from_inner(value)
        .map_err(|_| ObservedNumericError::Numeric(NumericOperationError::MantissaOverflow))
}

fn mantissa_fits_numeric_domain(value: &BigInt) -> bool {
    value.twos_byte_len() <= MAX_MANTISSA_BYTES
}

fn decimal_division_operands_observed<E, F>(
    dividend: &Numeric,
    divisor: &Numeric,
    output_scale: u32,
    observer: &mut F,
) -> Result<(UnboundedBigInt, UnboundedBigInt), ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let numerator_scale = divisor.scale + output_scale;
    let (numerator_delta, denominator_delta) = if numerator_scale >= dividend.scale {
        (numerator_scale - dividend.scale, 0)
    } else {
        (0, dividend.scale - numerator_scale)
    };
    let numerator = scale_unbounded_observed(dividend.mantissa.inner(), numerator_delta, observer)?;
    let denominator =
        scale_unbounded_observed(divisor.mantissa.inner(), denominator_delta, observer)?;
    Ok((numerator, denominator))
}

fn decimal_product_division_operands_observed<E, F>(
    product: &UnboundedBigInt,
    product_scale: u32,
    divisor: &Numeric,
    output_scale: u32,
    observer: &mut F,
) -> Result<(UnboundedBigInt, UnboundedBigInt), ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let numerator_scale =
        divisor
            .scale
            .checked_add(output_scale)
            .ok_or(ObservedNumericError::Numeric(
                NumericOperationError::ScaleOverflow,
            ))?;
    let (numerator_delta, denominator_delta) = if numerator_scale >= product_scale {
        (numerator_scale - product_scale, 0)
    } else {
        (0, product_scale - numerator_scale)
    };
    let numerator = scale_unbounded_observed(product, numerator_delta, observer)?;
    let denominator =
        scale_unbounded_observed(divisor.mantissa.inner(), denominator_delta, observer)?;
    Ok((numerator, denominator))
}

fn exact_division_at_scale_observed<E, F>(
    dividend: &Numeric,
    divisor: &Numeric,
    output_scale: u32,
    observer: &mut F,
) -> Result<Option<Numeric>, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let (numerator, denominator) =
        decimal_division_operands_observed(dividend, divisor, output_scale, observer)?;
    observer(NumericWorkStep::ExactDivisionAttempt {
        numerator_limbs: logical_limbs(&numerator),
        denominator_limbs: logical_limbs(&denominator),
        output_scale: u8::try_from(output_scale).expect("validated scale fits u8"),
    })
    .map_err(ObservedNumericError::Observer)?;
    let (quotient, remainder) = quotient_remainder(&numerator, &denominator);
    if !remainder.is_zero() {
        return Ok(None);
    }
    canonical_decimal_from_unbounded_observed(quotient, output_scale, observer).map(Some)
}

fn exact_product_division_at_scale_observed<E, F>(
    product: &UnboundedBigInt,
    product_scale: u32,
    divisor: &Numeric,
    output_scale: u32,
    observer: &mut F,
) -> Result<Option<Numeric>, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let (numerator, denominator) = decimal_product_division_operands_observed(
        product,
        product_scale,
        divisor,
        output_scale,
        observer,
    )?;
    observer(NumericWorkStep::ExactDivisionAttempt {
        numerator_limbs: logical_limbs(&numerator),
        denominator_limbs: logical_limbs(&denominator),
        output_scale: u8::try_from(output_scale).expect("validated scale fits u8"),
    })
    .map_err(ObservedNumericError::Observer)?;
    let (quotient, remainder) = quotient_remainder(&numerator, &denominator);
    if !remainder.is_zero() {
        return Ok(None);
    }
    canonical_decimal_from_unbounded_observed(quotient, output_scale, observer).map(Some)
}

fn classification_division<E, F>(
    dividend: &UnboundedBigInt,
    divisor: &UnboundedBigInt,
    observer: &mut F,
) -> Result<(UnboundedBigInt, UnboundedBigInt), ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    observer(NumericWorkStep::DivisionClassification {
        dividend_limbs: logical_limbs(dividend),
        divisor_limbs: logical_limbs(divisor),
    })
    .map_err(ObservedNumericError::Observer)?;
    Ok(quotient_remainder(dividend, divisor))
}

fn classify_exact_division_inner<E, F>(
    dividend: &Numeric,
    divisor: &Numeric,
    observer: &mut F,
) -> Result<ExactDivisionClass, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    let (numerator, denominator) =
        decimal_division_operands_observed(dividend, divisor, 0, observer)?;
    classify_exact_rational_observed(&numerator, &denominator, observer)
}

fn classify_exact_rational_observed<E, F>(
    numerator: &UnboundedBigInt,
    denominator: &UnboundedBigInt,
    observer: &mut F,
) -> Result<ExactDivisionClass, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    observer(NumericWorkStep::DivisionClassificationPrepare {
        numerator_limbs: logical_limbs(numerator),
        denominator_limbs: logical_limbs(denominator),
    })
    .map_err(ObservedNumericError::Observer)?;
    let absolute_denominator = denominator.abs();
    let mut lhs = numerator.abs();
    let mut rhs = absolute_denominator.clone();
    while !rhs.is_zero() {
        let (_, remainder) = classification_division(&lhs, &rhs, observer)?;
        lhs = rhs;
        rhs = remainder;
    }

    let mut reduced_denominator = if lhs.is_one() {
        absolute_denominator
    } else {
        classification_division(&absolute_denominator, &lhs, observer)?.0
    };
    let mut factors_two = 0_u32;
    let mut factors_five = 0_u32;
    for (prime, count) in [
        (UnboundedBigInt::from(2_u8), &mut factors_two),
        (UnboundedBigInt::from(5_u8), &mut factors_five),
    ] {
        while reduced_denominator > UnboundedBigInt::one() {
            let (quotient, remainder) =
                classification_division(&reduced_denominator, &prime, observer)?;
            if !remainder.is_zero() {
                break;
            }
            reduced_denominator = quotient;
            *count += 1;
        }
    }

    if reduced_denominator != UnboundedBigInt::one() {
        return Ok(ExactDivisionClass::Repeating);
    }
    let minimum_scale = factors_two.max(factors_five);
    if minimum_scale > MAX_DECIMAL_SCALE {
        return Ok(ExactDivisionClass::ScaleOverflow);
    }
    Ok(ExactDivisionClass::Representable {
        minimum_scale: u8::try_from(minimum_scale).expect("bounded minimum scale"),
    })
}

fn rounded_quotient(
    numerator: &UnboundedBigInt,
    denominator: &UnboundedBigInt,
    mode: RoundingMode,
) -> UnboundedBigInt {
    let (mut quotient, remainder) = quotient_remainder(numerator, denominator);
    if remainder.is_zero() {
        return quotient;
    }

    let direction = if numerator.is_negative() == denominator.is_negative() {
        UnboundedBigInt::one()
    } else {
        -UnboundedBigInt::one()
    };
    let increment = match mode {
        RoundingMode::TowardZero => false,
        RoundingMode::AwayFromZero => true,
        RoundingMode::Floor => direction.is_negative(),
        RoundingMode::Ceil => direction.is_positive(),
        RoundingMode::NearestEven | RoundingMode::NearestAway | RoundingMode::NearestTowardZero => {
            let doubled_remainder: UnboundedBigInt = remainder.abs() << 1_usize;
            match doubled_remainder.cmp(&denominator.abs()) {
                Ordering::Less => false,
                Ordering::Greater => true,
                Ordering::Equal => match mode {
                    RoundingMode::NearestEven => !(&quotient & UnboundedBigInt::one()).is_zero(),
                    RoundingMode::NearestAway => true,
                    RoundingMode::NearestTowardZero => false,
                    _ => unreachable!("matched nearest rounding modes"),
                },
            }
        }
    };
    if increment {
        quotient += direction;
    }
    quotient
}

fn scale_unbounded_observed<E, F>(
    value: &UnboundedBigInt,
    decimal_places: u32,
    observer: &mut F,
) -> Result<UnboundedBigInt, ObservedNumericError<E>>
where
    F: FnMut(NumericWorkStep) -> Result<(), E>,
{
    if decimal_places == 0 || value.is_zero() {
        observer(NumericWorkStep::Materialize {
            value_limbs: logical_limbs(value),
        })
        .map_err(ObservedNumericError::Observer)?;
        return Ok(value.clone());
    }
    observer(NumericWorkStep::ScaleByPowerOfTen {
        value_limbs: logical_limbs(value),
        exponent: u8::try_from(decimal_places).unwrap_or(u8::MAX),
    })
    .map_err(ObservedNumericError::Observer)?;
    Ok(value * decimal_power_unbounded(decimal_places))
}

fn scale_unbounded(value: &UnboundedBigInt, decimal_places: u32) -> UnboundedBigInt {
    if decimal_places == 0 || value.is_zero() {
        return value.clone();
    }
    value * decimal_power_unbounded(decimal_places)
}

fn decimal_power_unbounded(decimal_places: u32) -> UnboundedBigInt {
    let ten = UnboundedBigInt::from(10_u8);
    let mut power = UnboundedBigInt::one();
    for _ in 0..decimal_places {
        power *= &ten;
    }
    power
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
        match Numeric::try_new_raw(mantissa, scale) {
            Ok(numeric) => {
                numeric.validate_decimal().map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "error decoding numeric: noncanonical representation",
                    )
                })?;
                Ok(numeric)
            }
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
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), Error> {
        let helper = scale_::NumericScaleHelper {
            mantissa: self.mantissa.clone(),
            scale: self.scale(),
        };
        helper.serialize(writer)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        scale_::NumericScaleHelperView {
            mantissa: scale_::BigIntView(&self.mantissa),
            scale: self.scale(),
        }
        .encoded_len_exact()
    }
}

impl<'a> NoritoDeserialize<'a> for Numeric {
    fn deserialize(archived: &'a Archived<Numeric>) -> Self {
        Self::try_deserialize(archived).expect("invalid numeric")
    }

    fn try_deserialize(archived: &'a Archived<Numeric>) -> Result<Self, Error> {
        let helper_align = norito::core::archived_payload_align::<scale_::NumericScaleHelper>();
        let numeric_align = norito::core::archived_payload_align::<Numeric>();
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let aligned = numeric_align >= helper_align || (ptr as usize).is_multiple_of(helper_align);

        if aligned {
            let helper_arch: &Archived<scale_::NumericScaleHelper> = archived.cast();
            let helper = scale_::NumericScaleHelper::try_deserialize(helper_arch)?;
            let value = Numeric::try_new_raw(helper.mantissa, helper.scale)
                .map_err(|err| Error::Message(format!("invalid numeric: {err}")))?;
            value
                .validate_decimal()
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
        let parsed = value
            .parse::<Numeric>()
            .map_err(|err| json::Error::InvalidField {
                field: "numeric".into(),
                message: format!("invalid numeric `{value}`: {err}"),
            })?;
        if parsed.to_string() != value {
            return Err(json::Error::InvalidField {
                field: "numeric".into(),
                message: format!("noncanonical numeric `{value}`"),
            });
        }
        Ok(parsed)
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
        let lhs = scale_unbounded(self.mantissa.inner(), target_scale - self.scale);
        let rhs = scale_unbounded(other.mantissa.inner(), target_scale - other.scale);
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for Numeric {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NumericSpec {
    fn try_from_scale(scale: Option<u32>) -> Result<Self, NumericSpecError> {
        if scale.is_some_and(|scale| scale > MAX_DECIMAL_SCALE) {
            return Err(NumericSpecError::ScaleTooHigh);
        }
        Ok(Self { scale })
    }

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

    /// Try to create a specification accepting at most `scale` decimal places.
    ///
    /// # Errors
    /// Returns [`NumericSpecError::ScaleTooHigh`] when `scale` exceeds the V1
    /// numeric-domain maximum of 28.
    #[inline]
    pub const fn try_fractional(scale: u32) -> Result<Self, NumericSpecError> {
        if scale > MAX_DECIMAL_SCALE {
            return Err(NumericSpecError::ScaleTooHigh);
        }
        Ok(Self { scale: Some(scale) })
    }

    /// Create [`NumericSpec`] which accepts numeric values with scale up to given decimal places.
    ///
    /// # Panics
    /// Panics when `scale` exceeds the V1 numeric-domain maximum of 28. Use
    /// [`Self::try_fractional`] for untrusted or runtime-supplied scales.
    #[inline]
    pub const fn fractional(scale: u32) -> Self {
        assert!(
            scale <= MAX_DECIMAL_SCALE,
            "numeric specification scale exceeds the V1 maximum of 28"
        );
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
        let (negative, digits) = if let Some(digits) = trimmed.strip_prefix('-') {
            (true, digits)
        } else if let Some(digits) = trimmed.strip_prefix('+') {
            (false, digits)
        } else {
            (false, trimmed)
        };
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
        while scale > 0 && mantissa_str.ends_with('0') {
            mantissa_str.pop();
            scale -= 1;
        }
        if mantissa_str.is_empty() {
            return Err(NumericError::Malformed);
        }
        if mantissa_str.bytes().all(|byte| byte == b'0') {
            return Ok(Numeric::zero());
        }
        if negative {
            mantissa_str.insert(0, '-');
        }
        let unbounded = mantissa_str
            .parse::<UnboundedBigInt>()
            .map_err(|_| NumericError::Malformed)?;
        let mantissa = BigInt::from_inner(unbounded).map_err(|_| NumericError::MantissaTooLarge)?;
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
        let rendered = self.mantissa.to_string();
        let negative = self.mantissa.is_negative();
        let mut s = rendered.strip_prefix('-').unwrap_or(&rendered).to_owned();
        while s.len() <= self.scale as usize {
            s.insert(0, '0');
        }
        let (int_part, frac_part) = s.split_at(s.len() - self.scale as usize);
        if negative {
            write!(f, "-{int_part}.{frac_part}")
        } else {
            write!(f, "{int_part}.{frac_part}")
        }
    }
}

mod scale_ {
    /// Borrowed wire-compatible view of a numeric mantissa.
    pub(super) struct BigIntView<'a>(
        /// Canonical bounded integer serialized by the view.
        pub(super) &'a crate::bigint::BigInt,
    );

    impl norito::core::NoritoSerialize for BigIntView<'_> {
        fn schema_hash() -> [u8; 16] {
            <crate::bigint::BigInt as norito::core::NoritoSerialize>::schema_hash()
        }

        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            norito::core::NoritoSerialize::serialize(self.0, writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            norito::core::NoritoSerialize::encoded_len_hint(self.0)
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            norito::core::NoritoSerialize::encoded_len_exact(self.0)
        }
    }

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

    #[allow(unexpected_cfgs)]
    #[derive(norito::Encode)]
    /// Borrowed wire-compatible view used to size a canonical numeric.
    pub(super) struct NumericScaleHelperView<'a> {
        /// Borrowed canonical mantissa.
        #[codec(compact)]
        pub(super) mantissa: BigIntView<'a>,
        /// Canonical decimal scale.
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

    impl TypeId for Quantity {
        fn id() -> Ident {
            "Quantity".to_string()
        }
    }

    impl IntoSchema for Quantity {
        fn type_name() -> Ident {
            "Quantity".to_string()
        }

        fn update_schema_map(metamap: &mut MetaMap) {
            if !metamap.contains_key::<Self>() {
                <Numeric as IntoSchema>::update_schema_map(metamap);
                metamap.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                    declarations: vec![Declaration {
                        name: "value".to_string(),
                        ty: core::any::TypeId::of::<Numeric>(),
                    }],
                }));
            }
        }
    }

    impl TypeId for XorQuantity {
        fn id() -> Ident {
            "XorQuantity".to_string()
        }
    }

    impl IntoSchema for XorQuantity {
        fn type_name() -> Ident {
            "XorQuantity".to_string()
        }

        fn update_schema_map(metamap: &mut MetaMap) {
            if !metamap.contains_key::<Self>() {
                <Quantity as IntoSchema>::update_schema_map(metamap);
                metamap.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                    declarations: vec![Declaration {
                        name: "value".to_string(),
                        ty: core::any::TypeId::of::<Quantity>(),
                    }],
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::cmp::Ordering;

    use num_bigint::BigInt as ReferenceInt;
    use num_traits::{One as _, Signed as _, Zero as _};

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
    fn numeric_decode_from_slice_reports_the_exact_prefix_length() {
        let expected: Numeric = "1.25".parse().expect("numeric");
        let canonical = expected.encode();
        let mut followed_by_next_field = canonical.clone();
        followed_by_next_field.extend_from_slice(b"next-field");

        let (decoded, used) =
            <Numeric as norito::core::DecodeFromSlice>::decode_from_slice(&followed_by_next_field)
                .expect("decode numeric prefix");

        assert_eq!(decoded, expected);
        assert_eq!(used, canonical.len());
    }

    #[test]
    fn numeric_exact_norito_length_matches_canonical_payload() {
        let assert_exact_length = |value: &Numeric| {
            let owned = scale_::NumericScaleHelper {
                mantissa: value.mantissa.clone(),
                scale: value.scale(),
            };
            let borrowed = scale_::NumericScaleHelperView {
                mantissa: scale_::BigIntView(&value.mantissa),
                scale: value.scale(),
            };
            let owned_payload = norito::codec::Encode::encode(&owned);
            let borrowed_payload = norito::codec::Encode::encode(&borrowed);

            assert_eq!(borrowed_payload, owned_payload);
            assert_eq!(
                norito::codec::Encode::encode(value),
                owned_payload,
                "Numeric must retain the helper payload layout"
            );
            assert_eq!(owned.encoded_len_exact(), Some(owned_payload.len()));
            assert_eq!(borrowed.encoded_len_exact(), Some(owned_payload.len()));
            assert_eq!(value.encoded_len_exact(), Some(owned_payload.len()));
        };

        for value in [
            "-327.69",
            "-128",
            "-1",
            "0",
            "0.000000001",
            "1.25",
            "127",
            "128",
            "327.68",
        ] {
            let value: Numeric = value.parse().expect("canonical numeric");
            assert_exact_length(&value);
        }

        let signed_limit = ReferenceInt::one() << (MAX_MANTISSA_BITS - 1);
        for mantissa in [-signed_limit.clone(), signed_limit - 1_u8] {
            let mantissa = BigInt::from_inner(mantissa).expect("bounded numeric mantissa");
            let value =
                Numeric::try_new(mantissa, MAX_DECIMAL_SCALE).expect("canonical numeric extremum");
            assert_eq!(value.scale(), MAX_DECIMAL_SCALE);
            assert_eq!(value.mantissa.twos_byte_len(), MAX_MANTISSA_BYTES);
            assert_exact_length(&value);
        }
    }

    #[test]
    fn check_json_roundtrip() {
        let num1 = Numeric::new(1002, 2);

        let s = norito::json::to_json(&num1).expect("failed to serialize numeric");

        assert_eq!(s, "\"10.02\"");

        let num2 = norito::json::from_str(&s).expect("failed to deserialize numeric");

        assert_eq!(num1, num2);

        for noncanonical in ["+1", "01", "-0", "1.0", ".5", "1."] {
            let source = format!("\"{noncanonical}\"");
            assert!(
                norito::json::from_str::<Numeric>(&source).is_err(),
                "alternate decimal spelling must be rejected: {source}"
            );
        }
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

        let duplicate = norito::json::from_json::<NumericSpec>(r#"{"scale":1,"scale":2}"#)
            .expect_err("duplicate numeric-spec scale must fail closed");
        assert!(matches!(
            duplicate,
            norito::json::Error::DuplicateField { ref field } if field == "scale"
        ));
    }

    #[test]
    fn numeric_spec_rejects_scales_outside_the_v1_domain_on_every_boundary() {
        assert_eq!(
            NumericSpec::try_fractional(MAX_DECIMAL_SCALE).expect("the V1 maximum scale is valid"),
            NumericSpec::fractional(MAX_DECIMAL_SCALE)
        );
        assert!(matches!(
            NumericSpec::try_fractional(MAX_DECIMAL_SCALE + 1),
            Err(NumericSpecError::ScaleTooHigh)
        ));

        let hostile_json = format!(r#"{{"scale":{}}}"#, MAX_DECIMAL_SCALE + 1);
        assert!(
            norito::json::from_json::<NumericSpec>(&hostile_json).is_err(),
            "JSON must not construct an asset precision policy outside the numeric domain"
        );

        let invalid = NumericSpec {
            scale: Some(MAX_DECIMAL_SCALE + 1),
        };
        let encoded = norito::to_bytes(&invalid).expect("encode hostile archived fixture");
        assert!(
            norito::decode_from_bytes::<NumericSpec>(&encoded).is_err(),
            "Norito must not construct an asset precision policy outside the numeric domain"
        );
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

    #[test]
    fn signed_domain_minimum_parses_and_formats_at_fractional_scale() {
        let mut minimum_bytes = vec![0_u8; MAX_MANTISSA_BYTES];
        *minimum_bytes.last_mut().expect("nonempty signed domain") = 0x80;
        let minimum = BigInt::from_twos_bytes(&minimum_bytes).expect("signed minimum");
        let integer = minimum.to_string();
        let magnitude = integer.strip_prefix('-').expect("negative minimum");
        let split = magnitude.len() - 1;
        let source = format!("-{}.{}", &magnitude[..split], &magnitude[split..]);

        let numeric = source
            .parse::<Numeric>()
            .expect("fractional signed minimum");
        assert_eq!(numeric.mantissa(), &minimum);
        assert_eq!(numeric.scale(), 1);
        assert_eq!(numeric.to_string(), source);
    }

    fn decimal(source: &str) -> Numeric {
        source
            .parse::<Numeric>()
            .expect("valid decimal source")
            .canonicalize_decimal()
            .expect("representable canonical decimal")
    }

    fn quantity(source: &str) -> Quantity {
        source.parse().expect("valid quantity source")
    }

    fn signed_maximum() -> BigInt {
        let mut bytes = vec![0xff_u8; MAX_MANTISSA_BYTES - 1];
        bytes.push(0x7f);
        BigInt::from_twos_bytes(&bytes).expect("signed maximum")
    }

    fn signed_minimum() -> BigInt {
        let mut bytes = vec![0_u8; MAX_MANTISSA_BYTES - 1];
        bytes.push(0x80);
        BigInt::from_twos_bytes(&bytes).expect("signed minimum")
    }

    #[test]
    fn lossy_f64_conversion_is_explicit_and_finite_across_the_domain() {
        let beyond_exact_binary_range = decimal("9007199254740993");
        assert_eq!(
            beyond_exact_binary_range.to_f64_lossy().to_bits(),
            9_007_199_254_740_992.0_f64.to_bits()
        );

        for endpoint in [
            Numeric::try_new(signed_minimum(), 0).expect("signed minimum"),
            Numeric::try_new(signed_maximum(), 0).expect("signed maximum"),
            Numeric::try_new(signed_minimum(), MAX_DECIMAL_SCALE)
                .expect("fractional signed minimum"),
            Numeric::try_new(signed_maximum(), MAX_DECIMAL_SCALE)
                .expect("fractional signed maximum"),
        ] {
            assert!(endpoint.to_f64_lossy().is_finite(), "{endpoint}");
        }
    }

    #[test]
    fn decimal_canonicalization_is_unique_for_signed_zeroes_and_trailing_zeroes() {
        for (source, expected) in [
            ("0", Numeric::zero()),
            ("-0.000", Numeric::zero()),
            ("1.2300", Numeric::new(123, 2)),
            ("-1.2300", Numeric::new(-123, 2)),
            ("100.000", Numeric::new(100, 0)),
        ] {
            let parsed = source.parse::<Numeric>().expect("parse");
            let canonical = parsed.canonicalize_decimal().expect("canonicalize");
            assert_eq!(canonical, expected, "source={source}");
            canonical.validate_decimal().expect("canonical output");
        }
        for noncanonical in [
            Numeric::try_new_raw(0, 28).expect("raw zero"),
            Numeric::try_new_raw(10, 1).expect("raw trailing zero"),
        ] {
            assert_eq!(
                noncanonical.validate_decimal(),
                Err(NumericOperationError::NonCanonical)
            );
        }

        assert_eq!(
            Numeric::try_new(10, 29),
            Ok(Numeric::new(1, 28)),
            "normalization precedes the canonical scale bound"
        );
        assert_eq!(Numeric::try_new(1, 29), Err(NumericError::ScaleTooLarge));
        assert_eq!(Numeric::try_new(0, u32::MAX), Ok(Numeric::zero()));

        let removable = format!("0.{}10", "0".repeat(27));
        assert_eq!(
            removable.parse::<Numeric>(),
            Ok(Numeric::new(1, 28)),
            "source parsing must normalize a removable 29th digit"
        );
        let nonremovable = format!("0.{}1", "0".repeat(28));
        assert_eq!(
            nonremovable.parse::<Numeric>(),
            Err(NumericError::ScaleTooLarge)
        );

        let maximum = signed_maximum();
        let oversized_but_removable = format!("{maximum}.0");
        assert_eq!(
            oversized_but_removable.parse::<Numeric>(),
            Ok(Numeric::new(maximum.clone(), 0)),
            "mantissa bounds are checked after textual normalization"
        );
        let oversized_and_nonremovable = format!("{maximum}.1");
        assert_eq!(
            oversized_and_nonremovable.parse::<Numeric>(),
            Err(NumericError::MantissaTooLarge)
        );
    }

    #[test]
    fn numeric_parser_rejects_multiple_leading_signs() {
        for malformed in ["++1", "+-1", "-+1", "--1"] {
            assert_eq!(
                malformed.parse::<Numeric>(),
                Err(NumericError::Malformed),
                "multiple leading signs must be rejected: {malformed}"
            );
        }
    }

    #[test]
    fn decimal_endpoints_and_negation_enforce_signed_domain_after_normalization() {
        let maximum = Numeric::new(signed_maximum(), 0);
        let minimum = Numeric::new(signed_minimum(), 0);
        assert_eq!(
            maximum.try_decimal_add(&Numeric::one()),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            minimum.try_decimal_sub(&Numeric::one()),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            minimum.try_decimal_neg(),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            maximum
                .try_decimal_neg()
                .expect("negate max")
                .try_decimal_neg(),
            Ok(maximum)
        );
    }

    #[test]
    fn numeric_construction_rejects_both_signed_512_bit_neighbors() {
        let above_maximum = signed_maximum()
            .checked_add(&BigInt::one())
            .expect("generic bigint can represent the upper neighbor");
        let below_minimum = signed_minimum()
            .checked_sub(&BigInt::one())
            .expect("generic bigint can represent the lower neighbor");

        assert_eq!(
            Numeric::try_new(above_maximum.clone(), 0),
            Err(NumericError::MantissaTooLarge)
        );
        assert_eq!(
            Numeric::try_new(below_minimum.clone(), 0),
            Err(NumericError::MantissaTooLarge)
        );
        assert_eq!(
            Numeric::try_new_raw(above_maximum, 0),
            Err(NumericError::MantissaTooLarge)
        );
        assert_eq!(
            Numeric::try_new_raw(below_minimum, 0),
            Err(NumericError::MantissaTooLarge)
        );
    }

    #[test]
    fn decimal_multiplication_uses_unbounded_intermediate_then_normalizes() {
        let maximum = Numeric::new(signed_maximum(), MAX_DECIMAL_SCALE);
        maximum
            .validate_decimal()
            .expect("maximum is not divisible by ten");
        let decimal_power = Numeric::new(BigInt::pow10(MAX_DECIMAL_SCALE).expect("10^28 fits"), 0);
        assert_eq!(
            decimal_power.try_decimal_mul(&maximum),
            Ok(Numeric::new(signed_maximum(), 0)),
            "the conceptual product is wider than 512 bits but the canonical result fits"
        );

        assert_eq!(
            decimal("0.0000000000000000000000000001")
                .try_decimal_mul(&decimal("0.0000000000000000000000000001")),
            Err(NumericOperationError::ScaleOverflow)
        );
        assert_eq!(
            decimal("0.0000000000000000000000000002").try_decimal_mul(&decimal("0.5")),
            Ok(decimal("0.0000000000000000000000000001"))
        );
        assert_eq!(
            maximum.try_decimal_mul(&maximum),
            Err(NumericOperationError::ScaleOverflow),
            "after normalization, scale failure precedes simultaneous mantissa overflow"
        );
    }

    #[test]
    fn fused_multiply_divide_bounds_only_the_final_result() {
        let maximum = Numeric::new(signed_maximum(), 0);
        let factor = Numeric::from(10_000_u64);

        assert_eq!(
            maximum.try_decimal_mul(&factor),
            Err(NumericOperationError::MantissaOverflow),
            "the deliberately staged product does not fit the public domain"
        );
        assert_eq!(
            maximum.try_decimal_mul_div_round(&factor, &factor, 0, RoundingMode::NearestEven,),
            Ok(maximum.clone()),
            "the fused operation must bound only its mathematical final result"
        );
        assert_eq!(
            maximum.try_decimal_mul_div_exact(&factor, &factor),
            Ok(maximum.clone()),
            "exact fused arithmetic must also bound only its final result"
        );

        let maximum_quantity =
            Quantity::from_canonical_numeric(maximum).expect("signed maximum is non-negative");
        let maximum_xor =
            XorQuantity::try_from_quantity(maximum_quantity).expect("scale-zero XOR quantity");
        assert_eq!(
            maximum_xor.checked_mul_ratio_round(
                10_000,
                core::num::NonZeroU64::new(10_000).expect("nonzero fixture"),
                XOR_QUANTITY_SCALE,
                RoundingMode::Ceil,
            ),
            Ok(maximum_xor),
            "domain wrappers must retain the fused-intermediate guarantee"
        );
    }

    #[test]
    fn weighted_average_bounds_only_the_final_result_and_total_weight_is_unbounded() {
        let maximum = Quantity::from_canonical_numeric(Numeric::new(signed_maximum(), 0))
            .expect("signed maximum is a quantity");
        assert_eq!(
            maximum.try_mul_decimal(&Numeric::from(u64::MAX)),
            Err(NumericOperationError::MantissaOverflow),
            "a staged weighted product deliberately exceeds the public domain"
        );

        let entries = [(&maximum, u64::MAX), (&maximum, u64::MAX)];
        assert_eq!(
            Quantity::try_weighted_average_round(entries, 0, RoundingMode::NearestEven,),
            Ok(maximum),
            "neither the product, sum, nor total weight is narrowed before division"
        );
    }

    #[test]
    fn weighted_average_aligns_scales_and_has_explicit_failure_boundaries() {
        let one = Quantity::try_from_numeric(decimal("1")).expect("quantity");
        let two = Quantity::try_from_numeric(decimal("2")).expect("quantity");
        let fractional =
            Quantity::try_from_numeric(decimal("0.000001")).expect("fractional quantity");
        let entries = [(&one, 1), (&two, 1), (&fractional, 2)];

        assert_eq!(
            Quantity::try_weighted_average_round(entries, 6, RoundingMode::TowardZero),
            Quantity::try_from_numeric(decimal("0.75")),
        );
        assert_eq!(
            Quantity::try_weighted_average_round(
                [(&one, 0), (&two, 0)],
                0,
                RoundingMode::TowardZero,
            ),
            Err(NumericOperationError::DivisionByZero)
        );
        assert_eq!(
            Quantity::try_weighted_average_round(
                [(&one, 1)],
                MAX_DECIMAL_SCALE + 1,
                RoundingMode::TowardZero,
            ),
            Err(NumericOperationError::InvalidScale)
        );
    }

    #[test]
    fn aggregate_decimal_product_bounds_only_its_canonical_final_result() {
        let maximum_even = Quantity::from_canonical_numeric(Numeric::new(
            signed_maximum()
                .checked_sub(&BigInt::one())
                .expect("maximum minus one fits"),
            0,
        ))
        .expect("largest even positive quantity");
        assert_eq!(
            maximum_even.try_mul_decimal(&Numeric::from(2_u32)),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            maximum_even.try_product_decimals([&Numeric::from(2_u32), &decimal("0.5")]),
            Ok(maximum_even.clone()),
            "a later exact factor may cancel a wide conceptual product"
        );

        let tiny = decimal("0.0000000000000000000000000001");
        assert_eq!(
            Quantity::one()
                .try_mul_decimal(&tiny)
                .and_then(|value| value.try_mul_decimal(&tiny)),
            Err(NumericOperationError::ScaleOverflow)
        );
        assert_eq!(
            Quantity::one().try_product_decimals([
                &tiny,
                &tiny,
                &decimal("10000000000000000000000000000"),
            ]),
            Ok(Quantity::try_from_numeric(tiny.clone()).expect("tiny is a quantity")),
            "final normalization, rather than the scale-56 temporary, controls success"
        );
        assert_eq!(
            Quantity::one().try_product_decimals([&decimal("-1")]),
            Err(DecimalProductError::Numeric(
                NumericOperationError::NegativeQuantity
            ))
        );
    }

    #[test]
    fn aggregate_decimal_product_rejects_more_than_sixty_four_factors() {
        let one = Numeric::one();
        assert_eq!(
            Quantity::one()
                .try_product_decimals(core::iter::repeat_n(&one, MAX_DECIMAL_PRODUCT_FACTORS)),
            Ok(Quantity::one())
        );
        assert_eq!(
            Quantity::one()
                .try_product_decimals(core::iter::repeat_n(&one, MAX_DECIMAL_PRODUCT_FACTORS + 1)),
            Err(DecimalProductError::TooManyFactors)
        );
    }

    #[test]
    fn aggregate_decimal_product_rounds_only_its_final_result() {
        let panel_multiplier = decimal("0.7142857142857142857142857143");
        let result = Quantity::from(150_u32)
            .try_product_decimals_round(
                [
                    &decimal("1.06"),
                    &decimal("1.08"),
                    &decimal("1.2"),
                    &panel_multiplier,
                    &Numeric::one(),
                ],
                MAX_DECIMAL_SCALE,
                RoundingMode::NearestEven,
            )
            .expect("aggregate appeal-pricing product");
        assert_eq!(result.to_string(), "147.1885714285714285714285714315");
        assert_eq!(
            Quantity::from(150_u32)
                .try_product_decimals_round(
                    [
                        &decimal("1.06"),
                        &decimal("1.08"),
                        &decimal("1.2"),
                        &panel_multiplier,
                        &Numeric::one(),
                    ],
                    XOR_QUANTITY_SCALE,
                    RoundingMode::NearestEven,
                )
                .expect("nano-XOR aggregate appeal-pricing product")
                .to_string(),
            "147.188571429"
        );
        assert_eq!(
            Quantity::one()
                .try_product_decimals_round(
                    [&decimal("1.25"), &decimal("1.25")],
                    1,
                    RoundingMode::NearestEven,
                )
                .expect("aggregate final-only rounding")
                .to_string(),
            "1.6",
            "one aggregate rounding must differ from staged tie-to-even rounding"
        );
        assert_eq!(
            Quantity::one().try_product_decimals_round(
                [&Numeric::one()],
                MAX_DECIMAL_SCALE + 1,
                RoundingMode::NearestEven,
            ),
            Err(DecimalProductError::Numeric(
                NumericOperationError::InvalidScale
            ))
        );
    }

    #[test]
    fn rounded_aggregate_decimal_product_rejects_more_than_sixty_four_factors() {
        let one = Numeric::one();
        assert_eq!(
            Quantity::one().try_product_decimals_round(
                core::iter::repeat_n(&one, MAX_DECIMAL_PRODUCT_FACTORS),
                MAX_DECIMAL_SCALE,
                RoundingMode::NearestEven,
            ),
            Ok(Quantity::one())
        );
        assert_eq!(
            Quantity::one().try_product_decimals_round(
                core::iter::repeat_n(&one, MAX_DECIMAL_PRODUCT_FACTORS + 1),
                MAX_DECIMAL_SCALE,
                RoundingMode::NearestEven,
            ),
            Err(DecimalProductError::TooManyFactors)
        );
    }

    #[test]
    fn multiplied_quantity_comparison_never_materializes_bounded_products() {
        let maximum = Quantity::from_canonical_numeric(Numeric::new(signed_maximum(), 0))
            .expect("signed maximum is a quantity");

        assert_eq!(maximum.cmp_mul_u64(3, &maximum, 2), Ordering::Greater);
        assert_eq!(
            Quantity::try_from_numeric(decimal("0.02"))
                .expect("quantity")
                .cmp_mul_u64(
                    3,
                    &Quantity::try_from_numeric(decimal("0.03")).expect("quantity"),
                    2,
                ),
            Ordering::Equal,
            "scale alignment preserves an exact two-thirds boundary"
        );
    }

    #[test]
    fn fused_multiply_divide_matches_bounded_reference_across_signs_and_rounding_modes() {
        let values = ["-12.5", "-1", "0", "0.125", "7.75"];
        let multipliers = ["-3.2", "-1", "0", "0.5", "4"];
        let divisors = ["-2.5", "-1", "0.25", "3"];
        let modes = [
            RoundingMode::TowardZero,
            RoundingMode::AwayFromZero,
            RoundingMode::Floor,
            RoundingMode::Ceil,
            RoundingMode::NearestEven,
            RoundingMode::NearestAway,
            RoundingMode::NearestTowardZero,
        ];

        for value in values.map(decimal) {
            for multiplier in multipliers.map(decimal) {
                for divisor in divisors.map(decimal) {
                    let exact_reference = value
                        .try_decimal_mul(&multiplier)
                        .and_then(|product| product.try_decimal_div_exact(&divisor));
                    assert_eq!(
                        value.try_decimal_mul_div_exact(&multiplier, &divisor),
                        exact_reference,
                        "exact value={value}, multiplier={multiplier}, divisor={divisor}"
                    );
                    for output_scale in 0..=4 {
                        for mode in modes {
                            let reference =
                                value.try_decimal_mul(&multiplier).and_then(|product| {
                                    product.try_decimal_div_round(&divisor, output_scale, mode)
                                });
                            assert_eq!(
                                value.try_decimal_mul_div_round(
                                    &multiplier,
                                    &divisor,
                                    output_scale,
                                    mode,
                                ),
                                reference,
                                "value={value}, multiplier={multiplier}, divisor={divisor}, scale={output_scale}, mode={mode:?}"
                            );
                        }
                    }
                }
            }
        }

        assert_eq!(
            decimal("1").try_decimal_mul_div_round(
                &decimal("2"),
                &Numeric::zero(),
                0,
                RoundingMode::TowardZero,
            ),
            Err(NumericOperationError::DivisionByZero)
        );
        assert_eq!(
            decimal("1").try_decimal_mul_div_round(
                &decimal("2"),
                &decimal("3"),
                MAX_DECIMAL_SCALE + 1,
                RoundingMode::TowardZero,
            ),
            Err(NumericOperationError::InvalidScale)
        );
        assert_eq!(
            decimal("1").try_decimal_mul_div_exact(&decimal("1"), &decimal("3")),
            Err(NumericOperationError::RepeatingDecimal)
        );
        assert_eq!(
            decimal("0.0000000000000000000000000001")
                .try_decimal_mul_div_exact(&decimal("1"), &decimal("10")),
            Err(NumericOperationError::ExactDivisionScaleOverflow)
        );
    }

    #[test]
    fn exact_division_distinguishes_repeating_and_over_scale_terminating_results() {
        assert_eq!(
            decimal("1").try_decimal_div_exact(&decimal("8")),
            Ok(decimal("0.125"))
        );
        assert_eq!(
            decimal("1.2").try_decimal_div_exact(&decimal("0.03")),
            Ok(decimal("40"))
        );
        assert_eq!(
            decimal("1").try_decimal_div_exact(&decimal("3")),
            Err(NumericOperationError::RepeatingDecimal)
        );
        assert_eq!(
            decimal("0.0000000000000000000000000001").try_decimal_div_exact(&decimal("10")),
            Err(NumericOperationError::ExactDivisionScaleOverflow)
        );
        assert_eq!(
            decimal("1").classify_exact_division(&decimal("3")),
            Ok(ExactDivisionClass::Repeating)
        );
        assert_eq!(
            decimal("0.0000000000000000000000000001").classify_exact_division(&decimal("10")),
            Ok(ExactDivisionClass::ScaleOverflow)
        );
        assert_eq!(
            decimal("1").try_decimal_div_exact(&Numeric::zero()),
            Err(NumericOperationError::DivisionByZero)
        );
    }

    #[test]
    fn exact_division_at_scale_reports_inexact_without_conflating_failure_classes() {
        let one = decimal("1");
        let eight = decimal("8");
        for scale in 0..3 {
            assert_eq!(
                one.try_decimal_div_exact_at_scale(&eight, scale),
                Ok(None),
                "scale={scale}"
            );
        }
        assert_eq!(
            one.try_decimal_div_exact_at_scale(&eight, 3),
            Ok(Some(decimal("0.125")))
        );
        assert_eq!(
            one.try_decimal_div_exact_at_scale(&eight, 29),
            Err(NumericOperationError::InvalidScale)
        );
    }

    #[test]
    fn all_rounding_modes_are_correct_for_positive_and_negative_ties() {
        let two = decimal("2");
        let positive = decimal("1");
        let negative = decimal("-1");
        let expectations = [
            (RoundingMode::TowardZero, "0", "0"),
            (RoundingMode::AwayFromZero, "1", "-1"),
            (RoundingMode::Floor, "0", "-1"),
            (RoundingMode::Ceil, "1", "0"),
            (RoundingMode::NearestEven, "0", "0"),
            (RoundingMode::NearestAway, "1", "-1"),
            (RoundingMode::NearestTowardZero, "0", "0"),
        ];
        for (mode, expected_positive, expected_negative) in expectations {
            assert_eq!(
                positive.try_decimal_div_round(&two, 0, mode),
                Ok(decimal(expected_positive)),
                "positive mode={mode:?}"
            );
            assert_eq!(
                negative.try_decimal_div_round(&two, 0, mode),
                Ok(decimal(expected_negative)),
                "negative mode={mode:?}"
            );
        }
        assert_eq!(
            decimal("3").try_decimal_div_round(&two, 0, RoundingMode::NearestEven),
            Ok(decimal("2"))
        );
        assert_eq!(
            decimal("-3").try_decimal_div_round(&two, 0, RoundingMode::NearestEven),
            Ok(decimal("-2"))
        );
    }

    #[test]
    fn quantization_requires_an_explicit_mode_and_preserves_canonical_form() {
        let expectations = [
            (RoundingMode::TowardZero, "1.2", "-1.2"),
            (RoundingMode::AwayFromZero, "1.3", "-1.3"),
            (RoundingMode::Floor, "1.2", "-1.3"),
            (RoundingMode::Ceil, "1.3", "-1.2"),
            (RoundingMode::NearestEven, "1.2", "-1.2"),
            (RoundingMode::NearestAway, "1.3", "-1.3"),
            (RoundingMode::NearestTowardZero, "1.2", "-1.2"),
        ];
        for (mode, positive, negative) in expectations {
            assert_eq!(
                decimal("1.25").try_quantize(1, mode),
                Ok(decimal(positive)),
                "positive mode={mode:?}"
            );
            assert_eq!(
                decimal("-1.25").try_quantize(1, mode),
                Ok(decimal(negative)),
                "negative mode={mode:?}"
            );
        }

        let no_padding = decimal("1.2")
            .try_quantize(5, RoundingMode::NearestEven)
            .expect("a larger requested scale does not manufacture trailing zeros");
        assert_eq!(no_padding, decimal("1.2"));
        assert_eq!(no_padding.scale(), 1);

        let canonical_integer = decimal("1.004")
            .try_quantize(2, RoundingMode::TowardZero)
            .expect("truncated result is representable");
        assert_eq!(canonical_integer, decimal("1"));
        assert_eq!(canonical_integer.scale(), 0);
        assert_eq!(
            decimal("1").try_quantize(29, RoundingMode::TowardZero),
            Err(NumericOperationError::InvalidScale)
        );
    }

    #[test]
    fn quantization_to_spec_preserves_unconstrained_values_and_applies_scale() {
        let value = decimal("1.25");
        assert_eq!(
            value.try_quantize_to_spec(NumericSpec::unconstrained(), RoundingMode::NearestEven,),
            Ok(value.clone())
        );
        assert_eq!(
            value.try_quantize_to_spec(NumericSpec::fractional(1), RoundingMode::NearestEven),
            Ok(decimal("1.2"))
        );
    }

    #[test]
    fn exact_truncating_and_rounded_integer_conversions_are_distinct() {
        assert_eq!(
            decimal("42").try_decimal_to_int_exact(),
            Ok(BigInt::from(42_i32))
        );
        assert_eq!(
            decimal("42.01").try_decimal_to_int_exact(),
            Err(NumericOperationError::InexactConversion)
        );
        assert_eq!(
            decimal("-42.99").decimal_to_int_trunc(),
            Ok(BigInt::from(-42_i32))
        );
        assert_eq!(
            decimal("-42.5").decimal_to_int_round(RoundingMode::Floor),
            Ok(BigInt::from(-43_i32))
        );
        assert_eq!(
            decimal("42.5").decimal_to_int_round(RoundingMode::NearestEven),
            Ok(BigInt::from(42_i32))
        );
        assert_eq!(
            decimal("43.5").decimal_to_int_round(RoundingMode::NearestEven),
            Ok(BigInt::from(44_i32))
        );
    }

    #[test]
    fn observed_canonicalization_reports_each_normalization_and_zero_finalization() {
        let mut normalization_steps = Vec::new();
        let normalized = Numeric::try_new_raw(10_000, 4)
            .expect("raw value for observed normalization")
            .canonicalize_decimal_observed(&mut |step| {
                normalization_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("normalize");
        assert_eq!(normalized, Numeric::one());
        assert_eq!(normalization_steps.len(), 5);
        assert!(
            normalization_steps[..4]
                .iter()
                .all(|step| matches!(step, NumericWorkStep::Normalize { .. }))
        );
        assert!(matches!(
            normalization_steps[4],
            NumericWorkStep::Finalize { value_limbs: 1 }
        ));

        let mut zero_steps = Vec::new();
        let zero = Numeric::try_new_raw(0, MAX_DECIMAL_SCALE)
            .expect("raw scaled zero")
            .canonicalize_decimal_observed(&mut |step| {
                zero_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("canonicalize zero");
        assert_eq!(zero, Numeric::zero());
        assert_eq!(
            zero_steps,
            [NumericWorkStep::Finalize { value_limbs: 1 }],
            "zero performs no division but still validates its final domain"
        );
    }

    #[test]
    fn observed_validation_reports_canonicality_probe_before_division_and_can_abort() {
        let mut validation_steps = Vec::new();
        decimal("1.2")
            .validate_decimal_observed(&mut |step| {
                validation_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("canonical validation");
        assert_eq!(
            validation_steps,
            [NumericWorkStep::CanonicalityProbe {
                mantissa_limbs: 1,
                scale: 1,
            }]
        );
        assert_eq!(
            decimal("1.2").validate_decimal_observed(&mut |_| Err("out-of-gas")),
            Err(ObservedNumericError::Observer("out-of-gas"))
        );
    }

    #[test]
    fn observed_repeating_division_classifies_without_speculative_attempts() {
        let mut attempts = Vec::new();
        let error = decimal("1")
            .try_decimal_div_exact_observed(&decimal("3"), &mut |step| {
                attempts.push(step);
                Ok::<_, ()>(())
            })
            .expect_err("repeating");
        assert_eq!(
            error,
            ObservedNumericError::Numeric(NumericOperationError::RepeatingDecimal)
        );
        assert_eq!(
            attempts
                .iter()
                .filter(|step| matches!(step, NumericWorkStep::ExactDivisionAttempt { .. }))
                .count(),
            0,
            "a repeating quotient must fail after classification without speculative attempts"
        );
        assert!(
            attempts
                .iter()
                .any(|step| matches!(step, NumericWorkStep::DivisionClassification { .. }))
        );
    }

    #[test]
    fn observed_terminating_division_attempts_only_the_proven_scale() {
        let mut terminating_steps = Vec::new();
        assert_eq!(
            decimal("1")
                .try_decimal_div_exact_observed(&decimal("8"), &mut |step| {
                    terminating_steps.push(step);
                    Ok::<_, ()>(())
                })
                .expect("terminating quotient"),
            decimal("0.125")
        );
        assert_eq!(
            terminating_steps
                .iter()
                .filter(|step| matches!(step, NumericWorkStep::ExactDivisionAttempt { .. }))
                .count(),
            1,
            "a terminating quotient must perform exactly one proven-scale attempt"
        );
        assert!(terminating_steps.iter().any(|step| matches!(
            step,
            NumericWorkStep::ExactDivisionAttempt {
                output_scale: 3,
                ..
            }
        )));
    }

    #[test]
    fn observed_division_stops_before_arithmetic_after_observer_rejection() {
        let mut callbacks = 0;
        let aborted = decimal("1").try_decimal_div_exact_observed(&decimal("3"), &mut |_| {
            callbacks += 1;
            Err("out-of-gas")
        });
        assert_eq!(aborted, Err(ObservedNumericError::Observer("out-of-gas")));
        assert_eq!(
            callbacks, 1,
            "no arithmetic phase is entered after observer rejection"
        );
    }

    #[test]
    fn public_quantity_construction_makes_equality_ordering_and_hash_canonical() {
        use core::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let representations = [
            Numeric::new(1, 0),
            Numeric::new(10, 1),
            Numeric::new(100, 2),
            "1.0000".parse::<Numeric>().expect("parse"),
        ];
        for value in &representations {
            assert_eq!(value, &representations[0]);
            assert_eq!(value.cmp(&representations[0]), Ordering::Equal);
            assert_eq!(value.scale(), 0);
        }
        let hashes = representations.map(|value| {
            let mut hasher = DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        });
        assert!(hashes.iter().all(|hash| *hash == hashes[0]));
    }

    #[test]
    fn observed_alignment_and_multiplication_steps_precede_bigint_work() {
        let mut add_steps = Vec::new();
        let sum = decimal("1")
            .try_decimal_add_observed(&decimal("0.1"), &mut |step| {
                add_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("add");
        assert_eq!(sum, decimal("1.1"));
        assert!(matches!(
            add_steps[0],
            NumericWorkStep::CanonicalityProbe { .. }
        ));
        assert_eq!(
            add_steps[1],
            NumericWorkStep::ScaleByPowerOfTen {
                value_limbs: 1,
                exponent: 1,
            }
        );
        assert!(matches!(
            add_steps[2],
            NumericWorkStep::Materialize { value_limbs: 1 }
        ));
        assert!(matches!(add_steps[3], NumericWorkStep::Add { .. }));
        assert!(matches!(add_steps[4], NumericWorkStep::Normalize { .. }));
        assert!(matches!(add_steps[5], NumericWorkStep::Finalize { .. }));

        let mut zero_steps = Vec::new();
        let zero_sum = decimal("0")
            .try_decimal_add_observed(&decimal("0.1"), &mut |step| {
                zero_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("zero alignment add");
        assert_eq!(zero_sum, decimal("0.1"));
        assert!(matches!(
            zero_steps.first(),
            Some(NumericWorkStep::CanonicalityProbe { .. })
        ));
        assert!(matches!(
            zero_steps.get(1),
            Some(NumericWorkStep::Materialize { value_limbs: 1 })
        ));
        assert!(
            zero_steps
                .iter()
                .all(|step| !matches!(step, NumericWorkStep::ScaleByPowerOfTen { .. })),
            "zero alignment must not construct or multiply by a decimal power"
        );

        let mut multiply_steps = Vec::new();
        let product = decimal("0.2")
            .try_decimal_mul_observed(&decimal("0.5"), &mut |step| {
                multiply_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("multiply");
        assert_eq!(product, decimal("0.1"));
        let multiply_index = multiply_steps
            .iter()
            .position(|step| matches!(step, NumericWorkStep::Multiply { .. }))
            .expect("multiply work step");
        assert!(
            multiply_steps[..multiply_index]
                .iter()
                .all(|step| matches!(step, NumericWorkStep::CanonicalityProbe { .. }))
        );
        assert!(
            multiply_steps[multiply_index + 1..]
                .iter()
                .all(|step| matches!(
                    step,
                    NumericWorkStep::Normalize { .. } | NumericWorkStep::Finalize { .. }
                ))
        );
        assert!(matches!(
            multiply_steps.last(),
            Some(NumericWorkStep::Finalize { .. })
        ));

        let mut saw_add = false;
        let aborted =
            decimal("1").try_decimal_add_observed(&decimal("0.1"), &mut |step| match step {
                NumericWorkStep::ScaleByPowerOfTen { .. } => Err("out-of-gas"),
                NumericWorkStep::Add { .. } => {
                    saw_add = true;
                    Ok(())
                }
                _ => Ok(()),
            });
        assert_eq!(aborted, Err(ObservedNumericError::Observer("out-of-gas")));
        assert!(
            !saw_add,
            "aligned multiplication and addition did not begin after rejection"
        );
    }

    #[test]
    fn core_numeric_decoder_rejects_noncanonical_raw_payloads() {
        for value in [
            Numeric::try_new_raw(0, 1).expect("raw zero"),
            Numeric::try_new_raw(10, 1).expect("raw trailing zero"),
        ] {
            let encoded = value.encode();
            assert!(Numeric::decode(&mut encoded.as_slice()).is_err());
        }
    }

    #[test]
    fn quantity_canonicalization_is_unique_and_rejects_invalid_payloads() {
        for (source, expected, expected_scale) in [
            ("0", "0", 0),
            ("0.000", "0", 0),
            ("1.2500", "1.25", 2),
            ("10.000000", "10", 0),
        ] {
            let value = quantity(source);
            assert_eq!(value.to_string(), expected, "source={source}");
            assert_eq!(value.scale(), expected_scale, "source={source}");
            value
                .as_numeric()
                .validate_decimal()
                .expect("quantity contains a canonical decimal");
        }

        assert_eq!(
            Quantity::try_from_numeric(decimal("-0.01")),
            Err(NumericOperationError::NegativeQuantity)
        );

        let raw = Numeric::try_new_raw(10, 1).expect("representable noncanonical decimal");
        assert_eq!(
            raw.validate_decimal(),
            Err(NumericOperationError::NonCanonical)
        );
        assert_eq!(
            Quantity::try_from_numeric(raw),
            Ok(quantity("1")),
            "the canonicalizing constructor must produce the unique representation"
        );

        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let base = state % 1_000_000_000_000;
            let extra_zeroes = u32::try_from((state >> 48) % 7).expect("bounded zero count");
            let base_scale = u32::try_from((state >> 32) % 22).expect("bounded scale");
            let factor = 10_u64.pow(extra_zeroes);
            let encoded =
                Numeric::try_new_raw(base.saturating_mul(factor), base_scale + extra_zeroes)
                    .expect("bounded raw decimal");
            let value = Quantity::try_from_numeric(encoded.clone()).expect("nonnegative sample");
            value
                .as_numeric()
                .validate_decimal()
                .expect("canonical sample");
            assert_eq!(
                encoded.cmp(value.as_numeric()),
                Ordering::Equal,
                "canonicalization must preserve the represented value"
            );
            assert_eq!(
                Quantity::try_from_numeric(value.as_numeric().clone()),
                Ok(value.clone()),
                "canonicalization must be idempotent"
            );
            if value.is_zero() {
                assert_eq!(value.scale(), 0, "zero has one representation");
            }
        }
    }

    #[test]
    fn quantity_arithmetic_is_exact_and_underflow_is_explicit() {
        let lhs = quantity("1.20");
        let rhs = quantity("0.03");
        assert_eq!(lhs.checked_add(&rhs).expect("add").to_string(), "1.23");
        assert_eq!(lhs.checked_sub(&rhs).expect("sub").to_string(), "1.17");
        assert_eq!(
            rhs.checked_sub(&lhs),
            Err(NumericOperationError::QuantityUnderflow)
        );
        assert_eq!(
            lhs.try_mul_decimal(&decimal("-1")),
            Err(NumericOperationError::NegativeQuantity)
        );
        assert_eq!(lhs.try_mul_decimal(&decimal("0.5")), Ok(quantity("0.6")));
        assert_eq!(lhs.try_ratio_exact(&rhs), Ok(decimal("40")));

        let maximum = Quantity::from_canonical_numeric(Numeric::new(signed_maximum(), 0))
            .expect("signed maximum is a quantity");
        assert_eq!(
            maximum.try_mul_decimal(&Numeric::from(2_u32)),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            maximum.try_mul_decimal(&decimal("-2")),
            Err(NumericOperationError::MantissaOverflow),
            "result-domain overflow precedes the nominal negative-quantity check"
        );
    }

    #[test]
    fn quantity_exact_division_distinguishes_all_failure_classes() {
        assert_eq!(
            quantity("1").try_div_decimal_exact(quantity("8").as_numeric()),
            Ok(quantity("0.125"))
        );
        assert_eq!(
            quantity("1.2").try_div_decimal_exact(quantity("0.03").as_numeric()),
            Ok(quantity("40"))
        );
        assert_eq!(
            quantity("1").try_div_decimal_exact(quantity("3").as_numeric()),
            Err(NumericOperationError::RepeatingDecimal)
        );
        assert_eq!(
            quantity("0.0000000000000000000000000001")
                .try_div_decimal_exact(quantity("10").as_numeric()),
            Err(NumericOperationError::ExactDivisionScaleOverflow)
        );
        assert_eq!(
            quantity("1").try_div_decimal_exact(&Numeric::zero()),
            Err(NumericOperationError::DivisionByZero)
        );
        assert_eq!(
            quantity("1").try_div_decimal_exact(&decimal("-2")),
            Err(NumericOperationError::NegativeQuantity)
        );
    }

    #[test]
    fn quantity_rounded_division_obeys_modes_and_small_domain_invariants() {
        assert_eq!(
            quantity("1")
                .try_div_decimal_round(quantity("8").as_numeric(), 2, RoundingMode::Floor,),
            Ok(quantity("0.12"))
        );
        assert_eq!(
            quantity("1").try_div_decimal_round(quantity("8").as_numeric(), 2, RoundingMode::Ceil,),
            Ok(quantity("0.13"))
        );
        assert_eq!(
            quantity("1").try_div_decimal_round(
                quantity("8").as_numeric(),
                2,
                RoundingMode::NearestEven,
            ),
            Ok(quantity("0.12"))
        );
        assert_eq!(
            quantity("3").try_div_decimal_round(
                quantity("8").as_numeric(),
                2,
                RoundingMode::NearestEven,
            ),
            Ok(quantity("0.38"))
        );
        assert_eq!(
            quantity("1").try_div_decimal_round(
                quantity("2").as_numeric(),
                29,
                RoundingMode::Floor,
            ),
            Err(NumericOperationError::InvalidScale)
        );
        assert_eq!(
            quantity("1").try_div_decimal_round(&Numeric::zero(), 2, RoundingMode::NearestEven,),
            Err(NumericOperationError::DivisionByZero)
        );
        assert_eq!(
            quantity("1").try_div_decimal_round(&decimal("-2"), 2, RoundingMode::NearestEven),
            Err(NumericOperationError::NegativeQuantity)
        );

        for dividend in 0_u64..=50 {
            for divisor in 1_u64..=20 {
                let dividend = Quantity::from(dividend);
                let divisor = Quantity::from(divisor);
                for scale in 0..=4 {
                    let floor = dividend
                        .try_div_decimal_round(divisor.as_numeric(), scale, RoundingMode::Floor)
                        .expect("bounded floor");
                    let ceil = dividend
                        .try_div_decimal_round(divisor.as_numeric(), scale, RoundingMode::Ceil)
                        .expect("bounded ceil");
                    let nearest = dividend
                        .try_div_decimal_round(
                            divisor.as_numeric(),
                            scale,
                            RoundingMode::NearestEven,
                        )
                        .expect("bounded nearest-even");
                    for value in [&floor, &ceil, &nearest] {
                        value
                            .as_numeric()
                            .validate_decimal()
                            .expect("rounded quantity remains canonical");
                    }
                    assert!(floor <= nearest && nearest <= ceil);
                    assert!(
                        floor
                            .try_mul_decimal(divisor.as_numeric())
                            .expect("small product")
                            <= dividend
                    );
                    assert!(
                        ceil.try_mul_decimal(divisor.as_numeric())
                            .expect("small product")
                            >= dividend
                    );
                }
            }
        }
    }

    #[test]
    fn quantity_codec_json_and_schema_roundtrip_preserve_invariant() {
        let value: Quantity = "123.4500".parse().expect("quantity");
        assert_eq!(value.to_string(), "123.45");
        let encoded = norito::codec::Encode::encode(&value);
        let (decoded, used) =
            <Quantity as norito::core::DecodeFromSlice>::decode_from_slice(&encoded)
                .expect("decode quantity");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
        let mut followed_by_next_field = encoded.clone();
        followed_by_next_field.extend_from_slice(b"next-field");
        let (decoded, used) =
            <Quantity as norito::core::DecodeFromSlice>::decode_from_slice(&followed_by_next_field)
                .expect("decode quantity prefix");
        assert_eq!(decoded, value);
        assert_eq!(used, encoded.len());
        let json = norito::json::to_json(&value).expect("json");
        assert_eq!(json, "\"123.45\"");
        assert_eq!(
            norito::json::from_str::<Quantity>(&json).expect("json decode"),
            value
        );
        for noncanonical in ["+1", "01", "-0", "1.0", "123.4500"] {
            let source = format!("\"{noncanonical}\"");
            assert!(
                norito::json::from_str::<Quantity>(&source).is_err(),
                "alternate quantity spelling must be rejected: {source}"
            );
        }
        let schema = <Quantity as iroha_schema::IntoSchema>::schema();
        assert!(schema.contains_key::<Quantity>());
    }

    #[test]
    fn small_domain_arithmetic_matches_integer_reference_exhaustively() {
        for lhs in -100_i64..=100 {
            for rhs in -100_i64..=100 {
                let lhs_decimal = Numeric::from(lhs);
                let rhs_decimal = Numeric::from(rhs);
                assert_eq!(
                    lhs_decimal.try_decimal_add(&rhs_decimal),
                    Ok(Numeric::from(lhs + rhs))
                );
                assert_eq!(
                    lhs_decimal.try_decimal_sub(&rhs_decimal),
                    Ok(Numeric::from(lhs - rhs))
                );
                assert_eq!(
                    lhs_decimal.try_decimal_mul(&rhs_decimal),
                    Ok(Numeric::from(lhs * rhs))
                );
                if rhs != 0 && lhs % rhs == 0 {
                    assert_eq!(
                        lhs_decimal.try_decimal_div_exact(&rhs_decimal),
                        Ok(Numeric::from(lhs / rhs))
                    );
                }
            }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ReferenceDecimal {
        mantissa: ReferenceInt,
        scale: u32,
    }

    impl ReferenceDecimal {
        fn read(value: &Numeric) -> Self {
            Self {
                mantissa: value
                    .mantissa()
                    .to_string()
                    .parse()
                    .expect("bounded mantissa parses as num_bigint::BigInt"),
                scale: value.scale(),
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ReferenceExactClass {
        Representable { minimum_scale: u8 },
        Repeating,
        ScaleOverflow,
    }

    fn reference_pow10(exponent: u32) -> ReferenceInt {
        ReferenceInt::from(10_u8).pow(exponent)
    }

    fn reference_normalize(
        mut mantissa: ReferenceInt,
        mut scale: u32,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        if mantissa.is_zero() {
            return Ok(ReferenceDecimal { mantissa, scale: 0 });
        }
        let ten = ReferenceInt::from(10_u8);
        while scale > 0 && (&mantissa % &ten).is_zero() {
            mantissa /= &ten;
            scale -= 1;
        }
        if scale > MAX_DECIMAL_SCALE {
            return Err(NumericOperationError::ScaleOverflow);
        }
        let signed_limit = ReferenceInt::one() << (MAX_MANTISSA_BITS - 1);
        if mantissa < -signed_limit.clone() || mantissa >= signed_limit {
            return Err(NumericOperationError::MantissaOverflow);
        }
        Ok(ReferenceDecimal { mantissa, scale })
    }

    fn reference_add_or_sub(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
        subtract: bool,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        let scale = lhs.scale.max(rhs.scale);
        let lhs_aligned = &lhs.mantissa * reference_pow10(scale - lhs.scale);
        let rhs_aligned = &rhs.mantissa * reference_pow10(scale - rhs.scale);
        let mantissa = if subtract {
            lhs_aligned - rhs_aligned
        } else {
            lhs_aligned + rhs_aligned
        };
        reference_normalize(mantissa, scale)
    }

    fn reference_multiply(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        reference_normalize(&lhs.mantissa * &rhs.mantissa, lhs.scale + rhs.scale)
    }

    fn reference_gcd(mut lhs: ReferenceInt, mut rhs: ReferenceInt) -> ReferenceInt {
        lhs = lhs.abs();
        rhs = rhs.abs();
        while !rhs.is_zero() {
            let remainder = &lhs % &rhs;
            lhs = rhs;
            rhs = remainder;
        }
        lhs
    }

    fn reference_reduced_ratio(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
    ) -> Result<(ReferenceInt, ReferenceInt), NumericOperationError> {
        if rhs.mantissa.is_zero() {
            return Err(NumericOperationError::DivisionByZero);
        }
        // This is a direct rational construction: (lm / 10^ls) /
        // (rm / 10^rs) = (lm * 10^rs) / (rm * 10^ls). It intentionally does
        // not use any Numeric division, scale-alignment, or classification
        // helper.
        let mut numerator = &lhs.mantissa * reference_pow10(rhs.scale);
        let mut denominator = &rhs.mantissa * reference_pow10(lhs.scale);
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let gcd = reference_gcd(numerator.clone(), denominator.clone());
        Ok((numerator / &gcd, denominator / gcd))
    }

    fn reference_exact_class(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
    ) -> Result<ReferenceExactClass, NumericOperationError> {
        let (_, mut denominator) = reference_reduced_ratio(lhs, rhs)?;
        let mut factors_two = 0_u32;
        let mut factors_five = 0_u32;
        for (factor, count) in [
            (ReferenceInt::from(2_u8), &mut factors_two),
            (ReferenceInt::from(5_u8), &mut factors_five),
        ] {
            while (&denominator % &factor).is_zero() {
                denominator /= &factor;
                *count += 1;
            }
        }
        if denominator != ReferenceInt::one() {
            return Ok(ReferenceExactClass::Repeating);
        }
        let minimum_scale = factors_two.max(factors_five);
        if minimum_scale > MAX_DECIMAL_SCALE {
            return Ok(ReferenceExactClass::ScaleOverflow);
        }
        Ok(ReferenceExactClass::Representable {
            minimum_scale: u8::try_from(minimum_scale).expect("reference scale is at most 28"),
        })
    }

    fn reference_exact_divide(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        let class = reference_exact_class(lhs, rhs)?;
        let ReferenceExactClass::Representable { minimum_scale } = class else {
            return Err(match class {
                ReferenceExactClass::Repeating => NumericOperationError::RepeatingDecimal,
                ReferenceExactClass::ScaleOverflow => {
                    NumericOperationError::ExactDivisionScaleOverflow
                }
                ReferenceExactClass::Representable { .. } => unreachable!(),
            });
        };
        let (numerator, denominator) = reference_reduced_ratio(lhs, rhs)?;
        let scaled = numerator * reference_pow10(u32::from(minimum_scale));
        let quotient = &scaled / &denominator;
        assert!(
            (&scaled % &denominator).is_zero(),
            "independent classification must prove exact divisibility"
        );
        reference_normalize(quotient, u32::from(minimum_scale))
    }

    fn reference_rounded_divide(
        lhs: &ReferenceDecimal,
        rhs: &ReferenceDecimal,
        output_scale: u32,
        mode: RoundingMode,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        if output_scale > MAX_DECIMAL_SCALE {
            return Err(NumericOperationError::InvalidScale);
        }
        let (numerator, denominator) = reference_reduced_ratio(lhs, rhs)?;
        let scaled = numerator * reference_pow10(output_scale);
        let magnitude = scaled.abs();
        let mut quotient = &magnitude / &denominator;
        let remainder = &magnitude % &denominator;
        let negative = scaled.is_negative();
        let increment = if remainder.is_zero() {
            false
        } else {
            match mode {
                RoundingMode::TowardZero => false,
                RoundingMode::AwayFromZero => true,
                RoundingMode::Floor => negative,
                RoundingMode::Ceil => !negative,
                RoundingMode::NearestEven
                | RoundingMode::NearestAway
                | RoundingMode::NearestTowardZero => {
                    match (&remainder * ReferenceInt::from(2_u8)).cmp(&denominator) {
                        Ordering::Less => false,
                        Ordering::Greater => true,
                        Ordering::Equal => match mode {
                            RoundingMode::NearestEven => {
                                !(&quotient % ReferenceInt::from(2_u8)).is_zero()
                            }
                            RoundingMode::NearestAway => true,
                            RoundingMode::NearestTowardZero => false,
                            _ => unreachable!("matched a nearest rounding mode"),
                        },
                    }
                }
            }
        };
        if increment {
            quotient += ReferenceInt::one();
        }
        if negative {
            quotient = -quotient;
        }
        reference_normalize(quotient, output_scale)
    }

    fn reference_result(
        result: Result<Numeric, NumericOperationError>,
    ) -> Result<ReferenceDecimal, NumericOperationError> {
        result.map(|value| ReferenceDecimal::read(&value))
    }

    fn reference_class_result(
        result: Result<ExactDivisionClass, NumericOperationError>,
    ) -> Result<ReferenceExactClass, NumericOperationError> {
        result.map(|class| match class {
            ExactDivisionClass::Representable { minimum_scale } => {
                ReferenceExactClass::Representable { minimum_scale }
            }
            ExactDivisionClass::Repeating => ReferenceExactClass::Repeating,
            ExactDivisionClass::ScaleOverflow => ReferenceExactClass::ScaleOverflow,
        })
    }

    #[test]
    fn randomized_decimal_arithmetic_matches_independent_rational_reference() {
        const ROUNDING_MODES: [RoundingMode; 7] = [
            RoundingMode::TowardZero,
            RoundingMode::AwayFromZero,
            RoundingMode::Floor,
            RoundingMode::Ceil,
            RoundingMode::NearestEven,
            RoundingMode::NearestAway,
            RoundingMode::NearestTowardZero,
        ];

        // Fixed xorshift seed makes failures reproducible without coupling the
        // oracle to a random-number crate or host entropy.
        let mut random = 0x6a09_e667_f3bc_c909_u64;
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random
        };

        for case in 0..2_048 {
            let lhs_mantissa =
                i64::try_from(next() % 2_000_001).expect("bounded sample") - 1_000_000;
            let rhs_mantissa =
                i64::try_from(next() % 2_000_001).expect("bounded sample") - 1_000_000;
            let lhs_scale = u32::try_from(next() % 29).expect("bounded scale");
            let rhs_scale = u32::try_from(next() % 29).expect("bounded scale");
            let output_scale = u32::try_from(next() % 29).expect("bounded scale");
            let lhs = Numeric::new(lhs_mantissa, lhs_scale);
            let rhs = Numeric::new(rhs_mantissa, rhs_scale);
            let lhs_reference = ReferenceDecimal::read(&lhs);
            let rhs_reference = ReferenceDecimal::read(&rhs);
            let context = format!("case={case}, lhs={lhs}, rhs={rhs}, output_scale={output_scale}");

            assert_eq!(
                reference_result(lhs.try_decimal_add(&rhs)),
                reference_add_or_sub(&lhs_reference, &rhs_reference, false),
                "add: {context}"
            );
            assert_eq!(
                reference_result(lhs.try_decimal_sub(&rhs)),
                reference_add_or_sub(&lhs_reference, &rhs_reference, true),
                "subtract: {context}"
            );
            assert_eq!(
                reference_result(lhs.try_decimal_mul(&rhs)),
                reference_multiply(&lhs_reference, &rhs_reference),
                "multiply: {context}"
            );
            assert_eq!(
                reference_class_result(lhs.classify_exact_division(&rhs)),
                reference_exact_class(&lhs_reference, &rhs_reference),
                "exact classification: {context}"
            );
            assert_eq!(
                reference_result(lhs.try_decimal_div_exact(&rhs)),
                reference_exact_divide(&lhs_reference, &rhs_reference),
                "exact division: {context}"
            );
            for mode in ROUNDING_MODES {
                assert_eq!(
                    reference_result(lhs.try_decimal_div_round(&rhs, output_scale, mode)),
                    reference_rounded_divide(&lhs_reference, &rhs_reference, output_scale, mode,),
                    "rounded division ({mode:?}): {context}"
                );
            }
        }
    }

    #[allow(clippy::too_many_lines)] // The full-width matrix stays together so its corpus cannot drift between helpers.
    #[test]
    fn full_width_decimal_arithmetic_matches_independent_rational_reference() {
        const ROUNDING_MODES: [RoundingMode; 7] = [
            RoundingMode::TowardZero,
            RoundingMode::AwayFromZero,
            RoundingMode::Floor,
            RoundingMode::Ceil,
            RoundingMode::NearestEven,
            RoundingMode::NearestAway,
            RoundingMode::NearestTowardZero,
        ];

        fn numeric_from_reference(mantissa: &ReferenceInt, scale: u32) -> Numeric {
            let bounded = mantissa
                .to_string()
                .parse::<BigInt>()
                .expect("edge-biased reference mantissa fits the generic bigint domain");
            Numeric::try_new(bounded, scale)
                .expect("edge-biased input is canonicalizable in the V1 domain")
        }

        let signed_limit = ReferenceInt::one() << (MAX_MANTISSA_BITS - 1);
        let maximum = &signed_limit - ReferenceInt::one();
        let minimum = -signed_limit;
        let powers = [63_usize, 64, 127, 128, 255, 256, 447, 448, 510];
        let mut mantissas = vec![
            ReferenceInt::zero(),
            ReferenceInt::one(),
            -ReferenceInt::one(),
            maximum.clone(),
            minimum.clone(),
            &maximum - ReferenceInt::from(8_u8),
            &minimum + ReferenceInt::from(9_u8),
        ];
        for bit in powers {
            let power = ReferenceInt::one() << bit;
            mantissas.extend([
                &power - ReferenceInt::one(),
                power.clone(),
                &power + ReferenceInt::one(),
                -&power - ReferenceInt::one(),
                -power.clone(),
                -power + ReferenceInt::one(),
            ]);
        }

        let divisors = [
            ReferenceInt::zero(),
            ReferenceInt::from(2_u8),
            ReferenceInt::from(-2_i8),
            ReferenceInt::from(5_u8),
            ReferenceInt::from(25_u8),
            ReferenceInt::from(3_u8),
            ReferenceInt::from(-7_i8),
            (ReferenceInt::one() << 255) + ReferenceInt::one(),
        ];
        let scales = [0_u32, 1, 27, 28];
        let output_scales = [0_u32, 1, 27, 28];

        let mut case = 0_usize;
        for (index, lhs_mantissa) in mantissas.iter().enumerate() {
            let rhs_mantissa = if index % 3 == 0 {
                &mantissas[mantissas.len() - 1 - index]
            } else {
                &divisors[index % divisors.len()]
            };
            for &lhs_scale in &scales {
                let rhs_scale =
                    scales[(index + usize::try_from(lhs_scale).unwrap_or(0)) % scales.len()];
                let lhs = numeric_from_reference(lhs_mantissa, lhs_scale);
                let rhs = numeric_from_reference(rhs_mantissa, rhs_scale);
                let lhs_reference = ReferenceDecimal::read(&lhs);
                let rhs_reference = ReferenceDecimal::read(&rhs);
                let context = format!(
                    "case={case}, lhs={lhs}, rhs={rhs}, lhs_scale={lhs_scale}, rhs_scale={rhs_scale}"
                );

                assert_eq!(
                    reference_result(lhs.try_decimal_add(&rhs)),
                    reference_add_or_sub(&lhs_reference, &rhs_reference, false),
                    "add: {context}"
                );
                assert_eq!(
                    reference_result(lhs.try_decimal_sub(&rhs)),
                    reference_add_or_sub(&lhs_reference, &rhs_reference, true),
                    "subtract: {context}"
                );
                assert_eq!(
                    reference_result(lhs.try_decimal_mul(&rhs)),
                    reference_multiply(&lhs_reference, &rhs_reference),
                    "multiply: {context}"
                );
                assert_eq!(
                    reference_class_result(lhs.classify_exact_division(&rhs)),
                    reference_exact_class(&lhs_reference, &rhs_reference),
                    "exact classification: {context}"
                );
                assert_eq!(
                    reference_result(lhs.try_decimal_div_exact(&rhs)),
                    reference_exact_divide(&lhs_reference, &rhs_reference),
                    "exact division: {context}"
                );
                for &output_scale in &output_scales {
                    for mode in ROUNDING_MODES {
                        assert_eq!(
                            reference_result(lhs.try_decimal_div_round(&rhs, output_scale, mode),),
                            reference_rounded_divide(
                                &lhs_reference,
                                &rhs_reference,
                                output_scale,
                                mode,
                            ),
                            "rounded division ({mode:?}, scale={output_scale}): {context}"
                        );
                    }
                }
                case += 1;
            }
        }
        assert!(case >= 200, "full-width corpus unexpectedly shrank");
    }
}
