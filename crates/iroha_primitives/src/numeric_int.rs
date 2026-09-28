//! Checked signed 512-bit integer helpers with deterministic, pre-work observation.
//!
//! Compiler folding and VM execution share these algorithms. Intermediate values
//! use the bounded 4,096-bit integer backend; only inputs and final results must
//! fit the signed 512-bit language domain. Observers run before every arithmetic
//! operation, copy, comparison, and domain scan, independently of hardware.

use crate::{
    bigint::{BigInt, BigIntError},
    numeric::{MAX_MANTISSA_BYTES, NumericOperationError, NumericWorkStep, ObservedNumericError},
};
use core::{cmp::Ordering, convert::Infallible};

/// Unary checked operations on the Kotodama integer domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntUnaryOperation {
    /// Floor square root of a nonnegative integer.
    Isqrt,
    /// Absolute value, rejecting the minimum signed integer.
    Abs,
}

/// Binary checked operations on the Kotodama integer domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntBinaryOperation {
    /// Smaller operand in signed order.
    Min,
    /// Larger operand in signed order.
    Max,
    /// Mathematical ceiling of the exact quotient.
    DivCeil,
    /// Nonnegative greatest common divisor; the gcd of two zeros is zero.
    Gcd,
    /// Truncating arithmetic mean with a full-width intermediate sum.
    Mean,
}

impl IntUnaryOperation {
    /// Evaluate without an external work observer, as used by constant folding.
    ///
    /// # Errors
    /// Rejects out-of-domain operands/results and a negative square-root input.
    pub fn evaluate(self, value: &BigInt) -> Result<BigInt, NumericOperationError> {
        unobserved(self.evaluate_observed(value, &mut |_| Ok::<_, Infallible>(())))
    }

    /// Evaluate while allowing the caller to reject each work step before it runs.
    ///
    /// # Errors
    /// Returns a numeric domain failure or the observer's original error.
    pub fn evaluate_observed<E>(
        self,
        value: &BigInt,
        observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
    ) -> Result<BigInt, ObservedNumericError<E>> {
        validate(value, observer)?;
        let result = match self {
            Self::Abs => absolute(value, observer)?,
            Self::Isqrt => {
                if value.is_negative() {
                    return Err(ObservedNumericError::Numeric(
                        NumericOperationError::NegativeSquareRoot,
                    ));
                }
                square_root(value, observer)?
            }
        };
        validate(&result, observer)?;
        Ok(result)
    }
}

impl IntBinaryOperation {
    /// Evaluate without an external work observer, as used by constant folding.
    ///
    /// # Errors
    /// Rejects out-of-domain operands/results and division by zero.
    pub fn evaluate(self, lhs: &BigInt, rhs: &BigInt) -> Result<BigInt, NumericOperationError> {
        unobserved(self.evaluate_observed(lhs, rhs, &mut |_| Ok::<_, Infallible>(())))
    }

    /// Evaluate while allowing the caller to reject each work step before it runs.
    ///
    /// # Errors
    /// Returns a numeric domain failure or the observer's original error.
    pub fn evaluate_observed<E>(
        self,
        lhs: &BigInt,
        rhs: &BigInt,
        observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
    ) -> Result<BigInt, ObservedNumericError<E>> {
        validate(lhs, observer)?;
        validate(rhs, observer)?;
        let result = match self {
            Self::Min | Self::Max => {
                let order = compare(lhs, rhs, observer)?;
                let take_left = if self == Self::Min {
                    order != Ordering::Greater
                } else {
                    order != Ordering::Less
                };
                copy(if take_left { lhs } else { rhs }, observer)?
            }
            Self::DivCeil => {
                let (quotient, remainder) = divide(lhs, rhs, observer)?;
                if !remainder.is_zero() && lhs.is_negative() == rhs.is_negative() {
                    add(&quotient, &BigInt::one(), observer)?
                } else {
                    quotient
                }
            }
            Self::Gcd => {
                let mut left = absolute(lhs, observer)?;
                let mut right = absolute(rhs, observer)?;
                while !right.is_zero() {
                    let (_, remainder) = divide(&left, &right, observer)?;
                    left = right;
                    right = remainder;
                }
                left
            }
            Self::Mean => {
                let sum = add(lhs, rhs, observer)?;
                divide(&sum, &BigInt::from(2_u64), observer)?.0
            }
        };
        validate(&result, observer)?;
        Ok(result)
    }
}

fn unobserved<T>(
    result: Result<T, ObservedNumericError<Infallible>>,
) -> Result<T, NumericOperationError> {
    match result {
        Ok(value) => Ok(value),
        Err(ObservedNumericError::Numeric(error)) => Err(error),
        Err(ObservedNumericError::Observer(never)) => match never {},
    }
}

fn limbs(value: &BigInt) -> u16 {
    // BigInt bounds every magnitude, including intermediates, to 64 limbs.
    u16::try_from(value.bit_len().max(1).div_ceil(64))
        .expect("the bounded BigInt magnitude has at most 64 limbs")
}

fn work<E>(
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
    step: NumericWorkStep,
) -> Result<(), ObservedNumericError<E>> {
    observer(step).map_err(ObservedNumericError::Observer)
}

fn validate<E>(
    value: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<(), ObservedNumericError<E>> {
    work(
        observer,
        NumericWorkStep::Finalize {
            value_limbs: limbs(value),
        },
    )?;
    if value.twos_byte_len() > MAX_MANTISSA_BYTES {
        Err(ObservedNumericError::Numeric(
            NumericOperationError::MantissaOverflow,
        ))
    } else {
        Ok(())
    }
}

fn arithmetic<E>(result: Result<BigInt, BigIntError>) -> Result<BigInt, ObservedNumericError<E>> {
    result.map_err(|error| ObservedNumericError::Numeric(arithmetic_error(error)))
}

fn arithmetic_error(error: BigIntError) -> NumericOperationError {
    match error {
        BigIntError::Overflow => NumericOperationError::MantissaOverflow,
        BigIntError::DivisionByZero => NumericOperationError::DivisionByZero,
        BigIntError::NonCanonical => NumericOperationError::NonCanonical,
    }
}

fn copy<E>(
    value: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<BigInt, ObservedNumericError<E>> {
    work(
        observer,
        NumericWorkStep::Materialize {
            value_limbs: limbs(value),
        },
    )?;
    Ok(value.clone())
}

fn absolute<E>(
    value: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<BigInt, ObservedNumericError<E>> {
    if value.is_negative() {
        work(
            observer,
            NumericWorkStep::Negate {
                value_limbs: limbs(value),
            },
        )?;
        arithmetic(value.checked_neg())
    } else {
        copy(value, observer)
    }
}

fn compare<E>(
    lhs: &BigInt,
    rhs: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<Ordering, ObservedNumericError<E>> {
    work(
        observer,
        NumericWorkStep::Compare {
            lhs_limbs: limbs(lhs),
            rhs_limbs: limbs(rhs),
        },
    )?;
    Ok(lhs.cmp(rhs))
}

fn add<E>(
    lhs: &BigInt,
    rhs: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<BigInt, ObservedNumericError<E>> {
    work(
        observer,
        NumericWorkStep::Add {
            lhs_limbs: limbs(lhs),
            rhs_limbs: limbs(rhs),
        },
    )?;
    arithmetic(lhs.checked_add(rhs))
}

fn divide<E>(
    lhs: &BigInt,
    rhs: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<(BigInt, BigInt), ObservedNumericError<E>> {
    if rhs.is_zero() {
        return Err(ObservedNumericError::Numeric(
            NumericOperationError::DivisionByZero,
        ));
    }
    work(
        observer,
        NumericWorkStep::DivisionClassification {
            dividend_limbs: limbs(lhs),
            divisor_limbs: limbs(rhs),
        },
    )?;
    lhs.checked_div_rem(rhs)
        .map_err(|error| ObservedNumericError::Numeric(arithmetic_error(error)))
}

fn square_root<E>(
    value: &BigInt,
    observer: &mut impl FnMut(NumericWorkStep) -> Result<(), E>,
) -> Result<BigInt, ObservedNumericError<E>> {
    if value.is_zero() {
        return copy(value, observer);
    }
    // Newton iteration begins at a power of two strictly above sqrt(value).
    // For a 511-bit positive operand this needs at most 33 encoded bytes.
    let exponent = value.bit_len().div_ceil(2);
    work(
        observer,
        NumericWorkStep::Materialize {
            value_limbs: u16::try_from((exponent + 1).div_ceil(64))
                .expect("a validated 511-bit square-root input needs at most five initial limbs"),
        },
    )?;
    let mut bytes = [0_u8; MAX_MANTISSA_BYTES];
    bytes[exponent / 8] = 1 << (exponent % 8);
    let mut root = arithmetic(BigInt::from_twos_bytes(&bytes[..exponent / 8 + 2]))?;
    let two = BigInt::from(2_u64);
    loop {
        let quotient = divide(value, &root, observer)?.0;
        let sum = add(&root, &quotient, observer)?;
        let next = divide(&sum, &two, observer)?.0;
        if compare(&next, &root, observer)? != Ordering::Less {
            return Ok(root);
        }
        root = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extrema() -> (BigInt, BigInt) {
        let mut bytes = [0xff; MAX_MANTISSA_BYTES];
        bytes[MAX_MANTISSA_BYTES - 1] = 0x7f;
        let maximum = BigInt::from_twos_bytes(&bytes).expect("maximum");
        bytes.fill(0);
        bytes[MAX_MANTISSA_BYTES - 1] = 0x80;
        (BigInt::from_twos_bytes(&bytes).expect("minimum"), maximum)
    }

    #[test]
    fn limb_counts_cover_every_backend_word_boundary() {
        use crate::bigint::MAX_ENCODED_BYTES;

        assert_eq!(limbs(&BigInt::zero()), 1);
        for count in 1_u16..=64 {
            for bit in [usize::from(count - 1) * 64, usize::from(count) * 64 - 1] {
                let mut bytes = [0_u8; MAX_ENCODED_BYTES];
                bytes[bit / 8] = 1 << (bit % 8);
                let value = BigInt::from_twos_bytes(&bytes).expect("bounded backend word boundary");
                assert_eq!(limbs(&value), count);
            }
        }
    }

    #[test]
    fn square_root_initial_materialization_is_charged_before_work() {
        let (_, maximum) = extrema();
        let mut visited = Vec::new();
        let result = IntUnaryOperation::Isqrt.evaluate_observed(&maximum, &mut |step| {
            visited.push(step);
            if visited.len() == 2 {
                Err("initial root refused")
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result,
            Err(ObservedNumericError::Observer("initial root refused"))
        );
        assert_eq!(
            visited,
            [
                NumericWorkStep::Finalize { value_limbs: 8 },
                NumericWorkStep::Materialize { value_limbs: 5 },
            ]
        );
    }

    #[test]
    fn unary_helpers_cover_signed_domain_and_square_boundaries() {
        let (minimum, maximum) = extrema();
        assert_eq!(
            IntUnaryOperation::Abs.evaluate(&minimum),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            IntUnaryOperation::Isqrt.evaluate(&minimum),
            Err(NumericOperationError::NegativeSquareRoot)
        );
        for value in 0_u64..=1_024 {
            let integer = BigInt::from(value);
            assert_eq!(
                IntUnaryOperation::Isqrt.evaluate(&integer).unwrap(),
                BigInt::from(value.isqrt())
            );
            assert_eq!(
                IntUnaryOperation::Abs
                    .evaluate(&integer.checked_neg().unwrap())
                    .unwrap(),
                integer
            );
        }
        let root = IntUnaryOperation::Isqrt.evaluate(&maximum).unwrap();
        assert!(root.checked_mul(&root).unwrap() <= maximum);
        let successor = root.checked_add(&BigInt::one()).unwrap();
        assert!(successor.checked_mul(&successor).unwrap() > maximum);
    }

    #[test]
    fn binary_helpers_match_signed_reference_exhaustively() {
        for left in -32_i64..=32 {
            for right in -32_i64..=32 {
                let lhs = BigInt::from(left);
                let rhs = BigInt::from(right);
                for (operation, expected) in [
                    (IntBinaryOperation::Min, left.min(right)),
                    (IntBinaryOperation::Max, left.max(right)),
                    (IntBinaryOperation::Mean, (left + right) / 2),
                ] {
                    assert_eq!(
                        operation.evaluate(&lhs, &rhs).unwrap(),
                        BigInt::from(expected)
                    );
                }
                let (mut a, mut b) = (left.abs(), right.abs());
                while b != 0 {
                    (a, b) = (b, a % b);
                }
                assert_eq!(
                    IntBinaryOperation::Gcd.evaluate(&lhs, &rhs).unwrap(),
                    BigInt::from(a)
                );
                if right == 0 {
                    assert_eq!(
                        IntBinaryOperation::DivCeil.evaluate(&lhs, &rhs),
                        Err(NumericOperationError::DivisionByZero)
                    );
                } else {
                    let expected =
                        left / right + i64::from(left % right != 0 && (left < 0) == (right < 0));
                    assert_eq!(
                        IntBinaryOperation::DivCeil.evaluate(&lhs, &rhs).unwrap(),
                        BigInt::from(expected)
                    );
                }
            }
        }
    }

    #[test]
    fn intermediates_may_exceed_language_domain_but_results_must_fit() {
        let (minimum, maximum) = extrema();
        assert_eq!(
            IntBinaryOperation::Mean
                .evaluate(&maximum, &maximum)
                .unwrap(),
            maximum
        );
        assert_eq!(
            IntBinaryOperation::Mean
                .evaluate(&minimum, &minimum)
                .unwrap(),
            minimum
        );
        assert_eq!(
            IntBinaryOperation::Mean
                .evaluate(&minimum, &maximum)
                .unwrap(),
            BigInt::zero()
        );
        assert_eq!(
            IntBinaryOperation::Gcd
                .evaluate(&minimum, &BigInt::from(2_u64))
                .unwrap(),
            BigInt::from(2_u64)
        );
        assert_eq!(
            IntBinaryOperation::Gcd.evaluate(&minimum, &BigInt::zero()),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            IntBinaryOperation::DivCeil.evaluate(&minimum, &BigInt::from(-1_i64)),
            Err(NumericOperationError::MantissaOverflow)
        );
        let oversized = maximum.checked_add(&BigInt::one()).unwrap();
        assert_eq!(
            IntBinaryOperation::Mean.evaluate(&oversized, &BigInt::zero()),
            Err(NumericOperationError::MantissaOverflow)
        );
        assert_eq!(
            IntUnaryOperation::Isqrt.evaluate(&oversized),
            Err(NumericOperationError::MantissaOverflow)
        );
    }

    #[test]
    fn observers_can_refuse_every_stage_without_running_later_work() {
        let (_, maximum) = extrema();
        let mut steps = Vec::new();
        let expected = IntUnaryOperation::Isqrt
            .evaluate_observed(&maximum, &mut |step| {
                steps.push(step);
                Ok::<_, usize>(())
            })
            .unwrap();
        assert_eq!(
            IntUnaryOperation::Isqrt.evaluate(&maximum).unwrap(),
            expected
        );
        for stop in 0..steps.len() {
            let mut visited = Vec::new();
            let result = IntUnaryOperation::Isqrt.evaluate_observed(&maximum, &mut |step| {
                visited.push(step);
                if visited.len() == stop + 1 {
                    Err(stop)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result, Err(ObservedNumericError::Observer(stop)));
            assert_eq!(visited, steps[..=stop]);
        }
        for operation in [
            IntBinaryOperation::Min,
            IntBinaryOperation::Max,
            IntBinaryOperation::DivCeil,
            IntBinaryOperation::Gcd,
            IntBinaryOperation::Mean,
        ] {
            let mut steps = Vec::new();
            operation
                .evaluate_observed(&maximum, &BigInt::from(17_u64), &mut |step| {
                    steps.push(step);
                    Ok::<_, usize>(())
                })
                .unwrap();
            for stop in 0..steps.len() {
                let mut visited = 0;
                let result =
                    operation.evaluate_observed(&maximum, &BigInt::from(17_u64), &mut |_| {
                        visited += 1;
                        if visited == stop + 1 {
                            Err(stop)
                        } else {
                            Ok(())
                        }
                    });
                assert_eq!(result, Err(ObservedNumericError::Observer(stop)));
                assert_eq!(visited, stop + 1);
            }
        }
    }
}
