//! Exact decimal arithmetic with a signed 4,096-bit mantissa and bounded scale.
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
use num_bigint::BigInt as UnboundedBigInt;
use num_traits::{One as _, Signed as _, Zero as _};

use crate::bigint::{BigInt, MAX_BITS as BIGINT_MAX_BITS};

/// Maximum number of fractional decimal digits in a canonical decimal.
pub const MAX_DECIMAL_SCALE: u32 = 28;

/// Canonical exact decimal with a bounded signed mantissa and scale.
///
/// The finite set of values of type [`Numeric`] are of the form $m / 10^e$,
/// where `m` is in `-2^4095..=2^4095-1` and `e` is in `[0, 28]`.
/// The mantissa `m` is stored as a [`crate::bigint::BigInt`], while the scale
/// `e` is carried separately. Public constructors strip fractional trailing
/// zeroes, including reducing every zero to scale zero, so equality, ordering,
/// hashing, map keys, and serialization all observe one representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Numeric {
    mantissa: BigInt,
    scale: u32,
}

/// Canonical non-negative decimal used for quantities such as money.
///
/// `Quantity` is nominal: it cannot contain negative values or noncanonical
/// decimal representations, so ledger-domain mistakes are rejected before a
/// value reaches storage or hashing.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Quantity(Numeric);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum NumericError {
    /// Mantissa exceeds allowed range
    MantissaTooLarge,
    /// Scale exeeds allowed range
    ScaleTooLarge,
    /// Malformed: expecting number with optional decimal point (10, 10.02)
    Malformed,
}

/// Consensus-visible failures produced by exact decimal and quantity operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum NumericOperationError {
    /// Canonical result mantissa is outside `-2^4095..=2^4095-1`
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
        /// Conservative logical bound for the scaled result width.
        result_limbs: u16,
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
    /// One quotient/remainder operation used for explicit rounding or conversion.
    RoundedDivision {
        /// Width of the conceptual numerator.
        numerator_limbs: u16,
        /// Width of the conceptual denominator.
        denominator_limbs: u16,
        /// Requested output scale.
        output_scale: u8,
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

// TODO: Remove the pre-release Amount bridge after all in-tree VM and ledger
// callers have migrated to the nominal Quantity API in this cutover.
/// Failure produced by the temporary pre-release Kotodama `Amount` bridge.
///
/// `Amount` is a nominal source-language type whose payload is a canonical,
/// non-negative [`Numeric`].  Keeping these failures distinct lets the VM map
/// malformed input, arithmetic overflow, underflow, and deliberately inexact
/// division to stable deterministic traps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, displaydoc::Display, thiserror::Error)]
pub enum AmountError {
    /// Amount payload is negative
    Negative,
    /// Amount payload is not in its unique canonical representation
    NonCanonical,
    /// Amount subtraction would produce a negative value
    Underflow,
    /// Amount mantissa exceeds the signed 4,096-bit domain
    MantissaOverflow,
    /// Amount's canonical scale exceeds 28
    ScaleOverflow,
    /// Amount divisor is zero
    DivisionByZero,
    /// Division has no exact finite decimal result at scale 28 or less
    InexactDivision,
}

/// Explicit deterministic rounding policy for the temporary `Amount` bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmountRoundingMode {
    /// Round toward negative infinity (identical to truncation for Amounts).
    Floor,
    /// Round toward positive infinity.
    Ceil,
    /// Round to the nearest value, resolving exact ties to an even mantissa.
    NearestEven,
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
    /// - if mantissa leaves the signed 4,096-bit domain
    /// - if the canonical scale remains greater than 28 after trailing-zero removal
    #[inline]
    pub fn try_new<T: Into<BigInt>>(mantissa: T, scale: u32) -> Result<Self, NumericError> {
        let mantissa = mantissa.into();
        if mantissa.bit_len() > BIGINT_MAX_BITS {
            return Err(NumericError::MantissaTooLarge);
        }
        if mantissa.is_zero() {
            return Ok(Self::zero());
        }

        let mut value = Self { mantissa, scale }.trim_trailing_zeros();
        if value.scale > MAX_DECIMAL_SCALE {
            return Err(NumericError::ScaleTooLarge);
        }
        // Keep this assignment local to construction so strict decoders can
        // continue to use `try_new_raw` and reject alternate representations.
        value.scale = value.scale.min(MAX_DECIMAL_SCALE);
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
    /// The conceptual product may be wider than 4,096 bits and may initially
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
    /// Candidate scales are attempted in ascending order. If none succeeds,
    /// reduced-denominator classification distinguishes a repeating quotient
    /// from a terminating quotient requiring more than 28 digits.
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
        for output_scale in 0..=MAX_DECIMAL_SCALE {
            if let Some(value) =
                exact_division_at_scale_observed(self, divisor, output_scale, observer)?
            {
                return Ok(value);
            }
        }
        let class = classify_exact_division_inner(self, divisor, observer)?;
        Err(ObservedNumericError::Numeric(match class {
            ExactDivisionClass::Repeating => NumericOperationError::RepeatingDecimal,
            ExactDivisionClass::ScaleOverflow => NumericOperationError::ExactDivisionScaleOverflow,
            ExactDivisionClass::Representable { .. } => {
                unreachable!("a representable quotient succeeds in one of the attempted scales")
            }
        }))
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
        BigInt::from_inner(quotient)
            .map_err(|_| ObservedNumericError::Numeric(NumericOperationError::MantissaOverflow))
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
            return Ok(self.mantissa.clone());
        }
        let divisor = scale_unbounded_observed(&UnboundedBigInt::one(), self.scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(self.mantissa.inner()),
            denominator_limbs: logical_limbs(&divisor),
            output_scale: 0,
        })
        .map_err(ObservedNumericError::Observer)?;
        BigInt::from_inner(quotient_remainder(self.mantissa.inner(), &divisor).0)
            .map_err(|_| ObservedNumericError::Numeric(NumericOperationError::MantissaOverflow))
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
            return Ok(self.mantissa.clone());
        }
        let divisor = scale_unbounded_observed(&UnboundedBigInt::one(), self.scale, observer)?;
        observer(NumericWorkStep::RoundedDivision {
            numerator_limbs: logical_limbs(self.mantissa.inner()),
            denominator_limbs: logical_limbs(&divisor),
            output_scale: 0,
        })
        .map_err(ObservedNumericError::Observer)?;
        BigInt::from_inner(rounded_quotient(self.mantissa.inner(), &divisor, mode))
            .map_err(|_| ObservedNumericError::Numeric(NumericOperationError::MantissaOverflow))
    }

    // TODO: Remove this compatibility block with AmountError and
    // AmountRoundingMode once all in-tree callers use Quantity.
    /// Convert a `Numeric` into the unique payload representation accepted for
    /// a Kotodama `Amount`.
    ///
    /// Negative values are rejected. Fractional trailing zeros are removed and
    /// zero is always returned with scale zero.
    ///
    /// # Errors
    ///
    /// Returns [`AmountError::Negative`] when the mantissa is negative.
    pub fn canonicalize_amount(self) -> Result<Self, AmountError> {
        if self.mantissa.is_negative() {
            return Err(AmountError::Negative);
        }
        Ok(self.trim_trailing_zeros())
    }

    /// Validate that this value is a canonical, non-negative `Amount` payload.
    ///
    /// # Errors
    ///
    /// Returns [`AmountError::Negative`] for a negative mantissa or
    /// [`AmountError::NonCanonical`] when a fractional trailing zero remains.
    pub fn validate_amount(&self) -> Result<(), AmountError> {
        if self.mantissa.is_negative() {
            return Err(AmountError::Negative);
        }
        if self.scale > 0 {
            let ten = BigInt::from_i128(10);
            if self
                .mantissa
                .checked_div_rem(&ten)
                .is_ok_and(|(_, remainder)| remainder.is_zero())
            {
                return Err(AmountError::NonCanonical);
            }
        }
        Ok(())
    }

    /// Add two canonical `Amount` payloads.
    ///
    /// # Errors
    ///
    /// Returns an [`AmountError`] when either operand is not canonical or the
    /// canonical result exceeds Amount's mantissa or scale limits.
    pub fn checked_amount_add(&self, other: &Self) -> Result<Self, AmountError> {
        self.validate_amount()?;
        other.validate_amount()?;
        let target_scale = self.scale.max(other.scale);
        let lhs = scale_unbounded(self.mantissa.inner(), target_scale - self.scale);
        let rhs = scale_unbounded(other.mantissa.inner(), target_scale - other.scale);
        canonical_amount_from_unbounded(lhs + rhs, target_scale)
    }

    /// Subtract two canonical `Amount` payloads, rejecting underflow.
    ///
    /// # Errors
    ///
    /// Returns an [`AmountError`] when either operand is not canonical, the
    /// result would be negative, or the canonical result exceeds Amount's
    /// mantissa or scale limits.
    pub fn checked_amount_sub(&self, other: &Self) -> Result<Self, AmountError> {
        self.validate_amount()?;
        other.validate_amount()?;
        let target_scale = self.scale.max(other.scale);
        let lhs = scale_unbounded(self.mantissa.inner(), target_scale - self.scale);
        let rhs = scale_unbounded(other.mantissa.inner(), target_scale - other.scale);
        let difference = lhs - rhs;
        if difference.is_negative() {
            return Err(AmountError::Underflow);
        }
        canonical_amount_from_unbounded(difference, target_scale)
    }

    /// Multiply two canonical `Amount` payloads without implicit rounding.
    ///
    /// # Errors
    ///
    /// Returns an [`AmountError`] when either operand is not canonical or the
    /// exact canonical product exceeds Amount's mantissa or scale limits.
    pub fn checked_amount_mul(&self, other: &Self) -> Result<Self, AmountError> {
        self.validate_amount()?;
        other.validate_amount()?;
        let scale = self
            .scale
            .checked_add(other.scale)
            .ok_or(AmountError::ScaleOverflow)?;
        canonical_amount_from_unbounded(self.mantissa.inner() * other.mantissa.inner(), scale)
    }

    /// Divide two canonical `Amount` payloads exactly.
    ///
    /// The smallest representable decimal scale is selected. A repeating
    /// decimal, a result requiring more than 28 fractional digits, or a result
    /// whose canonical mantissa exceeds the signed 4,096-bit domain is rejected.
    ///
    /// # Errors
    ///
    /// Returns an [`AmountError`] when either operand is not canonical, the
    /// divisor is zero, the quotient is not an exact finite decimal through
    /// scale 28, or the canonical quotient exceeds the mantissa limit.
    pub fn checked_amount_div_exact(&self, divisor: &Self) -> Result<Self, AmountError> {
        self.validate_amount()?;
        divisor.validate_amount()?;
        if divisor.mantissa.is_zero() {
            return Err(AmountError::DivisionByZero);
        }

        for scale in 0..=28 {
            let (numerator, denominator) = amount_division_operands(self, divisor, scale);
            let quotient = &numerator / &denominator;
            let remainder = numerator % denominator;
            if remainder.is_zero() {
                return canonical_amount_from_unbounded(quotient, scale);
            }
        }
        Err(AmountError::InexactDivision)
    }

    /// Divide two canonical `Amount` payloads with an explicit output scale and
    /// rounding policy.
    ///
    /// # Errors
    ///
    /// Returns an [`AmountError`] when either operand is not canonical, the
    /// divisor is zero, `scale` exceeds 28, or the rounded canonical quotient
    /// exceeds the mantissa limit.
    pub fn checked_amount_div_round(
        &self,
        divisor: &Self,
        scale: u32,
        mode: AmountRoundingMode,
    ) -> Result<Self, AmountError> {
        self.validate_amount()?;
        divisor.validate_amount()?;
        if scale > MAX_DECIMAL_SCALE {
            return Err(AmountError::ScaleOverflow);
        }
        if divisor.mantissa.is_zero() {
            return Err(AmountError::DivisionByZero);
        }

        let (numerator, denominator) = amount_division_operands(self, divisor, scale);
        let mut quotient = &numerator / &denominator;
        let remainder = numerator % &denominator;
        if !remainder.is_zero() {
            let round_up = match mode {
                AmountRoundingMode::Floor => false,
                AmountRoundingMode::Ceil => true,
                AmountRoundingMode::NearestEven => {
                    let doubled = &remainder * UnboundedBigInt::from(2_u8);
                    match doubled.cmp(&denominator) {
                        Ordering::Less => false,
                        Ordering::Greater => true,
                        Ordering::Equal => {
                            (&quotient % UnboundedBigInt::from(2_u8)) != UnboundedBigInt::zero()
                        }
                    }
                }
            };
            if round_up {
                quotient += UnboundedBigInt::from(1_u8);
            }
        }
        canonical_amount_from_unbounded(quotient, scale)
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

    /// Validate an already canonical decimal as a non-negative quantity.
    ///
    /// # Errors
    /// Rejects noncanonical or negative input.
    pub fn from_canonical_numeric(value: Numeric) -> Result<Self, NumericOperationError> {
        value.validate_decimal()?;
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
        if self < other {
            return Err(NumericOperationError::QuantityUnderflow);
        }
        Self::from_canonical_numeric(self.0.try_decimal_sub(&other.0)?)
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
    fn serialize<W: Write>(&self, writer: W) -> Result<(), Error> {
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
    let mantissa = BigInt::from_inner(mantissa)
        .map_err(|_| ObservedNumericError::Numeric(NumericOperationError::MantissaOverflow))?;
    Ok(Numeric { mantissa, scale })
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
    let (numerator, denominator) = if numerator_scale >= dividend.scale {
        (
            scale_unbounded_observed(
                dividend.mantissa.inner(),
                numerator_scale - dividend.scale,
                observer,
            )?,
            divisor.mantissa.inner().clone(),
        )
    } else {
        (
            dividend.mantissa.inner().clone(),
            scale_unbounded_observed(
                divisor.mantissa.inner(),
                dividend.scale - numerator_scale,
                observer,
            )?,
        )
    };
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
    let mut lhs = numerator.abs();
    let mut rhs = denominator.abs();
    while !rhs.is_zero() {
        let (_, remainder) = classification_division(&lhs, &rhs, observer)?;
        lhs = rhs;
        rhs = remainder;
    }

    let mut reduced_denominator = if lhs.is_one() {
        denominator.abs()
    } else {
        classification_division(&denominator.abs(), &lhs, observer)?.0
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
        return Ok(value.clone());
    }
    // `10^e < 2^(4e)`. This pre-computable bound deliberately avoids doing
    // the multiplication merely to discover its result width before debit.
    let result_bits_bound = value
        .bits()
        .saturating_add(u64::from(decimal_places).saturating_mul(4));
    let result_limbs = u16::try_from(result_bits_bound.max(1).div_ceil(64)).unwrap_or(u16::MAX);
    observer(NumericWorkStep::ScaleByPowerOfTen {
        value_limbs: logical_limbs(value),
        exponent: u8::try_from(decimal_places).unwrap_or(u8::MAX),
        result_limbs,
    })
    .map_err(ObservedNumericError::Observer)?;
    Ok(value * UnboundedBigInt::from(10_u8).pow(decimal_places))
}

fn scale_unbounded(value: &UnboundedBigInt, decimal_places: u32) -> UnboundedBigInt {
    if decimal_places == 0 {
        return value.clone();
    }
    value * UnboundedBigInt::from(10_u8).pow(decimal_places)
}

fn canonical_amount_from_unbounded(
    mut mantissa: UnboundedBigInt,
    mut scale: u32,
) -> Result<Numeric, AmountError> {
    if mantissa.is_negative() {
        return Err(AmountError::Underflow);
    }
    let ten = UnboundedBigInt::from(10_u8);
    while scale > 0 && (&mantissa % &ten).is_zero() {
        mantissa /= &ten;
        scale -= 1;
    }
    if scale > 28 {
        return Err(AmountError::ScaleOverflow);
    }
    let mantissa = BigInt::from_inner(mantissa).map_err(|_| AmountError::MantissaOverflow)?;
    Numeric::try_new(mantissa, scale).map_err(|error| match error {
        NumericError::MantissaTooLarge => AmountError::MantissaOverflow,
        NumericError::ScaleTooLarge => AmountError::ScaleOverflow,
        NumericError::Malformed => unreachable!("validated mantissa and scale are well formed"),
    })
}

fn amount_division_operands(
    dividend: &Numeric,
    divisor: &Numeric,
    output_scale: u32,
) -> (UnboundedBigInt, UnboundedBigInt) {
    let numerator_scale = divisor
        .scale
        .checked_add(output_scale)
        .expect("validated Amount scales cannot overflow u32");
    let mut numerator = dividend.mantissa.inner().clone();
    let mut denominator = divisor.mantissa.inner().clone();
    if numerator_scale >= dividend.scale {
        numerator *= UnboundedBigInt::from(10_u8).pow(numerator_scale - dividend.scale);
    } else {
        denominator *= UnboundedBigInt::from(10_u8).pow(dividend.scale - numerator_scale);
    }
    (numerator, denominator)
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
        if negative {
            mantissa_str.insert(0, '-');
        }
        let mantissa: BigInt = mantissa_str
            .parse::<BigInt>()
            .map_err(|_| NumericError::Malformed)?;
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
        let mut minimum_bytes = vec![0_u8; crate::bigint::MAX_ENCODED_BYTES];
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

    fn amount(source: &str) -> Numeric {
        source
            .parse::<Numeric>()
            .expect("valid test numeric")
            .canonicalize_amount()
            .expect("non-negative test amount")
    }

    #[test]
    fn amount_canonicalization_has_one_zero_and_strips_fractional_zeros() {
        for (source, expected) in [
            ("0", Numeric::zero()),
            ("0.000", Numeric::zero()),
            ("1.2500", Numeric::new(125, 2)),
            ("10.000000", Numeric::new(10, 0)),
        ] {
            let canonical = source
                .parse::<Numeric>()
                .expect("valid numeric")
                .canonicalize_amount()
                .expect("non-negative");
            assert_eq!(canonical, expected, "source {source}");
            canonical.validate_amount().expect("canonical amount");
        }

        assert_eq!(
            "-0.1"
                .parse::<Numeric>()
                .expect("valid signed numeric")
                .canonicalize_amount(),
            Err(AmountError::Negative)
        );
        assert_eq!(
            Numeric::try_new_raw(10, 1)
                .expect("raw test value")
                .validate_amount(),
            Err(AmountError::NonCanonical)
        );
    }

    #[test]
    fn amount_canonicalization_properties_hold_for_deterministic_samples() {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..10_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let base = state % 1_000_000_000_000;
            let extra_zeroes = u32::try_from((state >> 48) % 7).expect("bounded zero count");
            let base_scale = u32::try_from((state >> 32) % 22).expect("bounded scale");
            let factor = 10_u64.pow(extra_zeroes);
            let encoded = Numeric::new(base.saturating_mul(factor), base_scale + extra_zeroes);
            let canonical = encoded
                .clone()
                .canonicalize_amount()
                .expect("sample is nonnegative");
            canonical.validate_amount().expect("canonical sample");
            assert_eq!(
                canonical.clone().canonicalize_amount(),
                Ok(canonical.clone()),
                "canonicalization must be idempotent"
            );
            assert_eq!(
                encoded.cmp(&canonical),
                Ordering::Equal,
                "canonicalization must preserve the represented value"
            );
            if canonical.is_zero() {
                assert_eq!(canonical.scale(), 0, "zero has one representation");
            }
        }
    }

    #[test]
    fn amount_arithmetic_is_exact_and_canonical() {
        assert_eq!(
            amount("1.2").checked_amount_add(&amount("0.03")),
            Ok(amount("1.23"))
        );
        assert_eq!(
            amount("1.2").checked_amount_sub(&amount("0.03")),
            Ok(amount("1.17"))
        );
        assert_eq!(
            amount("1.2").checked_amount_mul(&amount("0.5")),
            Ok(amount("0.6"))
        );
        assert_eq!(
            amount("0.00000000000000000002").checked_amount_mul(&amount("0.000000005")),
            Ok(amount("0.0000000000000000000000000001"))
        );
        assert_eq!(
            amount("1").checked_amount_sub(&amount("1.01")),
            Err(AmountError::Underflow)
        );
    }

    #[test]
    fn amount_mantissa_overflow_is_rejected_after_canonicalization() {
        let mut bytes = vec![0xff_u8; BIGINT_MAX_BITS / 8 - 1];
        bytes.push(0x7f);
        let largest = Numeric::new(
            BigInt::from_twos_bytes(&bytes).expect("positive signed-domain maximum"),
            0,
        );
        largest.validate_amount().expect("canonical maximum");
        assert_eq!(largest.mantissa().bit_len(), BIGINT_MAX_BITS - 1);
        assert!(
            largest > Numeric::new(largest.mantissa().clone(), 28),
            "comparison must use unbounded intermediates when scale alignment exceeds 4096 bits"
        );
        assert_eq!(
            largest.checked_amount_mul(&Numeric::new(2, 0)),
            Err(AmountError::MantissaOverflow)
        );
    }

    #[test]
    fn amount_exact_division_accepts_only_finite_scale_28_results() {
        assert_eq!(
            amount("1").checked_amount_div_exact(&amount("8")),
            Ok(amount("0.125"))
        );
        assert_eq!(
            amount("1.2").checked_amount_div_exact(&amount("0.03")),
            Ok(amount("40"))
        );
        assert_eq!(
            amount("1").checked_amount_div_exact(&amount("3")),
            Err(AmountError::InexactDivision)
        );
        assert_eq!(
            amount("0.0000000000000000000000000001").checked_amount_div_exact(&amount("10")),
            Err(AmountError::InexactDivision)
        );
        assert_eq!(
            amount("1").checked_amount_div_exact(&Numeric::zero()),
            Err(AmountError::DivisionByZero)
        );
    }

    #[test]
    fn amount_exact_division_recovers_canonical_factors_deterministically() {
        const FINITE_DIVISORS: [u64; 10] = [1, 2, 4, 5, 8, 10, 20, 25, 40, 50];

        for quotient_mantissa in 0_u64..=100 {
            for quotient_scale in 0..=8 {
                let quotient = Numeric::new(quotient_mantissa, quotient_scale)
                    .canonicalize_amount()
                    .expect("nonnegative quotient");
                for divisor_mantissa in FINITE_DIVISORS {
                    for divisor_scale in 0..=8 {
                        let divisor = Numeric::new(divisor_mantissa, divisor_scale)
                            .canonicalize_amount()
                            .expect("positive divisor");
                        let dividend = quotient
                            .checked_amount_mul(&divisor)
                            .expect("small exact product");
                        let recovered = dividend
                            .checked_amount_div_exact(&divisor)
                            .expect("a product divided by its factor is exact");

                        assert_eq!(recovered, quotient);
                        recovered.validate_amount().expect("canonical quotient");
                        assert!(recovered.scale() <= 28);
                    }
                }
            }
        }

        for nonterminating_prime in [3_u64, 7, 11, 13, 17, 19, 23, 29, 31] {
            assert_eq!(
                amount("1").checked_amount_div_exact(&Numeric::from(nonterminating_prime)),
                Err(AmountError::InexactDivision),
                "a reduced denominator containing a prime other than 2 or 5 must not terminate"
            );
        }
    }

    #[test]
    fn amount_rounded_division_implements_all_tie_rules() {
        assert_eq!(
            amount("1").checked_amount_div_round(&amount("8"), 2, AmountRoundingMode::Floor),
            Ok(amount("0.12"))
        );
        assert_eq!(
            amount("1").checked_amount_div_round(&amount("8"), 2, AmountRoundingMode::Ceil),
            Ok(amount("0.13"))
        );
        assert_eq!(
            amount("1").checked_amount_div_round(&amount("8"), 2, AmountRoundingMode::NearestEven,),
            Ok(amount("0.12"))
        );
        assert_eq!(
            amount("3").checked_amount_div_round(&amount("8"), 2, AmountRoundingMode::NearestEven,),
            Ok(amount("0.38"))
        );
        assert_eq!(
            amount("1").checked_amount_div_round(&amount("6"), 2, AmountRoundingMode::NearestEven,),
            Ok(amount("0.17"))
        );
        assert_eq!(
            amount("1").checked_amount_div_round(&amount("2"), 29, AmountRoundingMode::Floor),
            Err(AmountError::ScaleOverflow)
        );
    }

    #[test]
    fn amount_rounding_small_domain_invariants_hold_exhaustively() {
        for dividend in 0_u64..=50 {
            for divisor in 1_u64..=20 {
                let dividend = Numeric::from(dividend);
                let divisor = Numeric::from(divisor);
                for scale in 0..=4 {
                    let floor = dividend
                        .checked_amount_div_round(&divisor, scale, AmountRoundingMode::Floor)
                        .expect("bounded floor");
                    let ceil = dividend
                        .checked_amount_div_round(&divisor, scale, AmountRoundingMode::Ceil)
                        .expect("bounded ceil");
                    let nearest = dividend
                        .checked_amount_div_round(&divisor, scale, AmountRoundingMode::NearestEven)
                        .expect("bounded nearest");
                    floor.validate_amount().expect("canonical floor");
                    ceil.validate_amount().expect("canonical ceil");
                    nearest.validate_amount().expect("canonical nearest");
                    assert!(floor <= nearest && nearest <= ceil);
                    assert!(floor.checked_amount_mul(&divisor).expect("small product") <= dividend);
                    assert!(ceil.checked_amount_mul(&divisor).expect("small product") >= dividend);
                }
            }
        }
    }

    fn decimal(source: &str) -> Numeric {
        source
            .parse::<Numeric>()
            .expect("valid decimal source")
            .canonicalize_decimal()
            .expect("representable canonical decimal")
    }

    fn signed_maximum() -> BigInt {
        let mut bytes = vec![0xff_u8; BIGINT_MAX_BITS / 8 - 1];
        bytes.push(0x7f);
        BigInt::from_twos_bytes(&bytes).expect("signed maximum")
    }

    fn signed_minimum() -> BigInt {
        let mut bytes = vec![0_u8; BIGINT_MAX_BITS / 8 - 1];
        bytes.push(0x80);
        BigInt::from_twos_bytes(&bytes).expect("signed minimum")
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
    fn decimal_multiplication_uses_unbounded_intermediate_then_normalizes() {
        let maximum = Numeric::new(signed_maximum(), MAX_DECIMAL_SCALE);
        maximum
            .validate_decimal()
            .expect("maximum is not divisible by ten");
        let decimal_power = Numeric::new(BigInt::pow10(MAX_DECIMAL_SCALE).expect("10^28 fits"), 0);
        assert_eq!(
            decimal_power.try_decimal_mul(&maximum),
            Ok(Numeric::new(signed_maximum(), 0)),
            "the conceptual product is wider than 4096 bits but the canonical result fits"
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
    fn observer_is_called_before_every_division_and_can_abort_without_later_work() {
        let mut normalization_steps = Vec::new();
        let normalized = Numeric::try_new_raw(10_000, 4)
            .expect("raw value for observed normalization")
            .canonicalize_decimal_observed(&mut |step| {
                normalization_steps.push(step);
                Ok::<_, ()>(())
            })
            .expect("normalize");
        assert_eq!(normalized, Numeric::one());
        assert_eq!(normalization_steps.len(), 4);
        assert!(
            normalization_steps
                .iter()
                .all(|step| matches!(step, NumericWorkStep::Normalize { .. }))
        );

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
            29
        );
        assert!(
            attempts
                .iter()
                .any(|step| matches!(step, NumericWorkStep::DivisionClassification { .. }))
        );

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
    fn public_numeric_construction_makes_equality_ordering_and_hash_canonical() {
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
                result_limbs: 1,
            }
        );
        assert!(matches!(add_steps[2], NumericWorkStep::Add { .. }));
        assert!(matches!(add_steps[3], NumericWorkStep::Normalize { .. }));

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
                .all(|step| matches!(step, NumericWorkStep::Normalize { .. }))
        );

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
    fn quantity_is_nominal_nonnegative_and_reports_underflow() {
        assert_eq!(
            Quantity::try_from_numeric(decimal("-0.01")),
            Err(NumericOperationError::NegativeQuantity)
        );
        let lhs: Quantity = "1.20".parse().expect("quantity");
        let rhs: Quantity = "0.03".parse().expect("quantity");
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
        assert_eq!(lhs.try_ratio_exact(&rhs), Ok(decimal("40")));
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
}
