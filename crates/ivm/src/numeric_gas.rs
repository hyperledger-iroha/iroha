//! Consensus gas formulas for Kotodama V1 exact numbers.
//!
//! Work is expressed in logical 64-bit limbs. The formulas never inspect host
//! bigint allocation strategies or hardware instructions, so equal inputs cost
//! the same on every validator architecture.

use crate::error::VMError;
use iroha_primitives::numeric::NumericWorkStep;

/// Version of the complete consensus numeric-gas formula and staged phase map.
///
/// This value is included in the gas-schedule descriptor. Any change to a
/// logical-work formula, charge-point ordering, or stable staged-phase tag
/// MUST increment it and regenerate the gas-schedule golden hash.
pub const NUMERIC_GAS_FORMULA_VERSION_V1: u64 = 3;
/// Fixed staged-syscall entry charge.
pub const NUMERIC_ENTRY_GAS: u64 = 16;
/// Gas charged for each logical 64-bit limb of arithmetic work.
pub const NUMERIC_GAS_PER_LIMB_WORK: u64 = 4;
/// Pointer-ABI header bytes read before trusting an envelope length.
pub const POINTER_HEADER_BYTES: u64 = 7;
/// Authentication digest bytes at the end of every pointer envelope.
pub const POINTER_HASH_BYTES: u64 = iroha_crypto::Hash::LENGTH as u64;
/// Bytes covered by one logical numeric-frame validation work unit.
pub const NUMERIC_VALIDATION_WORD_BYTES: u64 = 8;
/// Maximum public decimal scale.
pub const MAX_DECIMAL_SCALE: u8 = 28;
/// Maximum scale of an exact multiplication intermediate.
pub const MAX_PRODUCT_SCALE: u8 = MAX_DECIMAL_SCALE * 2;
/// Maximum signed integer limb width (`512 / 64`).
pub const MAX_VALUE_LIMBS: u64 = (iroha_primitives::numeric::MAX_MANTISSA_BITS / 64) as u64;
/// Maximum schoolbook multiplication intermediate width.
pub const MAX_PRODUCT_LIMBS: u64 = MAX_VALUE_LIMBS * 2;

// Exact bit length of 10^n for n=0..=56. Keeping the table in the consensus
// module avoids floating-point logarithms and their cross-platform edge cases.
const POW10_BIT_LENGTH: [u16; 57] = [
    1, 4, 7, 10, 14, 17, 20, 24, 27, 30, 34, 37, 40, 44, 47, 50, 54, 57, 60, 64, 67, 70, 74, 77,
    80, 84, 87, 90, 94, 97, 100, 103, 107, 110, 113, 117, 120, 123, 127, 130, 133, 137, 140, 143,
    147, 150, 153, 157, 160, 163, 167, 170, 173, 177, 180, 183, 187,
];

/// Checked addition in the consensus gas domain.
#[inline]
pub fn checked_add(lhs: u64, rhs: u64) -> Result<u64, VMError> {
    lhs.checked_add(rhs).ok_or(VMError::GasCostOverflow)
}

/// Checked multiplication in the consensus gas domain.
#[inline]
pub fn checked_mul(lhs: u64, rhs: u64) -> Result<u64, VMError> {
    lhs.checked_mul(rhs).ok_or(VMError::GasCostOverflow)
}

/// Convert a bounded byte count to the consensus gas domain.
pub fn checked_bytes(bytes: usize) -> Result<u64, VMError> {
    u64::try_from(bytes).map_err(|_| VMError::GasCostOverflow)
}

/// Charge one envelope exactly once: header first, then digest and payload.
pub fn envelope_tail_bytes(payload_bytes: usize) -> Result<u64, VMError> {
    checked_add(POINTER_HASH_BYTES, checked_bytes(payload_bytes)?)
}

/// Payload-authentication gas for one input frame.
///
/// The fixed digest term covers reading/comparing the supplied digest. The
/// frame term covers the complete traversal performed by `Hash::new(frame)`;
/// snapshot transport is charged separately by the pointer-envelope phases.
pub fn payload_hash_gas(frame_bytes: usize) -> Result<u64, VMError> {
    checked_add(POINTER_HASH_BYTES, checked_bytes(frame_bytes)?)
}

/// Output byte work for canonical framing, authentication, and publication.
///
/// `envelope_bytes` covers building/publishing the complete envelope. Two
/// additional frame traversals cover Norito checksum/framing and the outer
/// authentication hash.
pub fn output_serialization_gas(envelope_bytes: usize, frame_bytes: usize) -> Result<u64, VMError> {
    checked_add(
        checked_bytes(envelope_bytes)?,
        checked_mul(2, checked_bytes(frame_bytes)?)?,
    )
}

/// Logical work needed to decode and validate one numeric frame.
///
/// Structural Norito validation scans every complete or partial eight-byte
/// word, including the payload CRC. Canonical value decoding then scans every
/// complete or partial body word. Counting both passes prevents the nested
/// checksum traversal from disappearing behind the pointer-envelope byte
/// charge. Scaled decimal/quantity values additionally charge their observed
/// quotient/remainder canonicality probe. Frame bytes themselves are still
/// charged exactly once as transport.
pub fn numeric_frame_validation_work(frame_bytes: usize) -> Result<u64, VMError> {
    let (decode, canonical) = numeric_frame_validation_phase_work(frame_bytes)?;
    checked_add(decode, canonical)
}

/// Split frame validation into stable decode and canonicality phases.
///
/// The sum of the two values is always [`numeric_frame_validation_work`].
pub fn numeric_frame_validation_phase_work(frame_bytes: usize) -> Result<(u64, u64), VMError> {
    let decode = checked_bytes(frame_bytes)?
        .div_ceil(NUMERIC_VALIDATION_WORD_BYTES)
        .max(1);
    let body_bytes =
        frame_bytes.saturating_sub(iroha_primitives::numeric_abi::NUMERIC_FRAME_HEADER_BYTES_V1);
    let canonical = numeric_frame_body_validation_work(body_bytes)?;
    Ok((decode, canonical))
}

/// Logical word work for decoding and validating a canonical numeric body,
/// excluding the separately observed decimal divide-by-ten probe.
pub fn numeric_frame_body_validation_work(body_bytes: usize) -> Result<u64, VMError> {
    Ok(checked_bytes(body_bytes)?
        .div_ceil(NUMERIC_VALIDATION_WORD_BYTES)
        .max(1))
}

/// Logical limb count for a bit width. Zero still occupies one logical limb.
#[must_use]
pub const fn limbs_for_bits(bits: u64) -> u64 {
    if bits == 0 { 1 } else { bits.div_ceil(64) }
}

/// Exact bit width of `10^exponent` for every supported intermediate scale.
pub fn pow10_bit_length(exponent: u8) -> Result<u64, VMError> {
    POW10_BIT_LENGTH
        .get(usize::from(exponent))
        .copied()
        .map(u64::from)
        .ok_or(VMError::GasCostOverflow)
}

/// Logical limb width of `10^exponent`.
pub fn pow10_limbs(exponent: u8) -> Result<u64, VMError> {
    Ok(limbs_for_bits(pow10_bit_length(exponent)?))
}

/// Conservative exact width bound for a nonzero `value * 10^exponent`.
///
/// `value_bits` is the exact magnitude bit width. For a nonzero exponent the
/// product uses at most `value_bits + bit_length(10^exponent)` bits. The
/// seemingly tighter `- 1` bound is not valid for every operand: for example,
/// a 61-bit value multiplied by ten can require 65 bits. Exponent zero is
/// handled separately so multiplying by one retains the original width.
/// Passing zero returns one logical limb regardless of exponent.
pub fn scaled_limbs(value_bits: u64, exponent: u8) -> Result<u64, VMError> {
    if value_bits == 0 {
        return Ok(1);
    }
    if exponent == 0 {
        return Ok(limbs_for_bits(value_bits));
    }
    let bits = checked_add(value_bits, pow10_bit_length(exponent)?)?;
    Ok(limbs_for_bits(bits))
}

/// Logical work for multiplying a value by a decimal power.
pub fn scale_work(value_limbs: u64, exponent: u8) -> Result<u64, VMError> {
    if exponent == 0 {
        return Ok(0);
    }
    checked_add(
        power_construction_work(exponent)?,
        checked_mul(value_limbs.max(1), pow10_limbs(exponent)?)?,
    )
}

/// Work for copying an unchanged value into one owned conceptual temporary.
pub const fn materialization_work(value_limbs: u64) -> u64 {
    if value_limbs == 0 { 1 } else { value_limbs }
}

fn alignment_operand_work(value_limbs: u64, exponent: u8) -> Result<u64, VMError> {
    if exponent == 0 {
        Ok(materialization_work(value_limbs))
    } else {
        scale_work(value_limbs, exponent)
    }
}

/// Logical work for deterministically constructing `10^exponent`.
///
/// The primitive implementation starts at one and performs one multiplication
/// by the one-limb constant ten for each decimal place. The operand before step
/// `k + 1` is `10^k`, so the exact schoolbook work is the sum of those widths.
pub fn power_construction_work(exponent: u8) -> Result<u64, VMError> {
    let mut work = 0_u64;
    for power in 0..exponent {
        work = checked_add(work, pow10_limbs(power)?)?;
    }
    Ok(work)
}

/// Aligned add/subtract/compare work, including both decimal scale shifts.
pub fn aligned_work(
    lhs_limbs: u64,
    lhs_scale_delta: u8,
    lhs_aligned_limbs: u64,
    rhs_limbs: u64,
    rhs_scale_delta: u8,
    rhs_aligned_limbs: u64,
) -> Result<u64, VMError> {
    let lhs_scale = alignment_operand_work(lhs_limbs, lhs_scale_delta)?;
    let rhs_scale = alignment_operand_work(rhs_limbs, rhs_scale_delta)?;
    checked_add(
        checked_add(lhs_scale, rhs_scale)?,
        lhs_aligned_limbs.max(rhs_aligned_limbs).max(1),
    )
}

/// Schoolbook multiplication work. Inputs are always at least one limb.
pub fn multiplication_work(lhs_limbs: u64, rhs_limbs: u64) -> Result<u64, VMError> {
    checked_mul(lhs_limbs.max(1), rhs_limbs.max(1))
}

/// Deterministic Knuth-style long-division bound.
///
/// Let `q = max(1, dividend_limbs - divisor_limbs + 1)`, with subtraction
/// clamped at zero. Work is `dividend + divisor + divisor * q`: one dividend
/// scan, one divisor normalization pass, and one divisor-width trial per
/// candidate quotient limb.
pub fn division_work(dividend_limbs: u64, divisor_limbs: u64) -> Result<u64, VMError> {
    let dividend = dividend_limbs.max(1);
    let divisor = divisor_limbs.max(1);
    let quotient_limbs = if dividend < divisor {
        1
    } else {
        checked_add(dividend - divisor, 1)?
    };
    checked_add(
        checked_add(dividend, divisor)?,
        checked_mul(quotient_limbs, divisor)?,
    )
}

/// Conservative logical width of a truncating quotient.
///
/// The zero quotient still occupies one logical limb. For all other inputs,
/// base-`2^64` long division cannot produce more than
/// `dividend_limbs - divisor_limbs + 1` quotient limbs.
pub fn quotient_limb_bound(dividend_limbs: u64, divisor_limbs: u64) -> Result<u64, VMError> {
    let dividend = dividend_limbs.max(1);
    let divisor = divisor_limbs.max(1);
    if dividend < divisor {
        Ok(1)
    } else {
        checked_add(dividend - divisor, 1)
    }
}

/// Work for one quotient/remainder operation implemented with one division.
///
/// The bigint layer computes `q = dividend / divisor` once and derives the
/// remainder as `dividend - q * divisor`. Account for all three operations so
/// the consensus charge remains an upper bound for the actual backend work.
pub fn quotient_remainder_work(dividend_limbs: u64, divisor_limbs: u64) -> Result<u64, VMError> {
    let dividend = dividend_limbs.max(1);
    let divisor = divisor_limbs.max(1);
    let quotient = quotient_limb_bound(dividend, divisor)?;
    checked_add(
        checked_add(
            division_work(dividend, divisor)?,
            multiplication_work(quotient, divisor)?,
        )?,
        dividend,
    )
}

/// Conservative all-rounding-mode work for a rounded quotient.
///
/// In addition to the quotient/remainder operation, the implementation scans
/// and doubles the remainder, scans the absolute denominator, compares the
/// doubled remainder, probes quotient parity for nearest-even, and may add one
/// to the quotient. Charging the all-mode bound keeps gas independent of the
/// selected rounding tag and of whether the remainder is a tie.
pub fn rounded_division_work(dividend_limbs: u64, divisor_limbs: u64) -> Result<u64, VMError> {
    let dividend = dividend_limbs.max(1);
    let divisor = divisor_limbs.max(1);
    let remainder = dividend.min(divisor);
    let doubled_remainder = checked_add(remainder, 1)?;
    let quotient = quotient_limb_bound(dividend, divisor)?;
    let ancillary = checked_add(
        checked_add(remainder, remainder)?,
        checked_add(
            checked_add(divisor, doubled_remainder.max(divisor))?,
            checked_add(1, checked_add(quotient, 1)?)?,
        )?,
    )?;
    checked_add(quotient_remainder_work(dividend, divisor)?, ancillary)
}

/// Work for absolute-value preparation before exact denominator classification.
///
/// The numerator is copied once. The denominator is copied once for the GCD
/// state and once for the later reduced-denominator state.
pub fn classification_prepare_work(
    numerator_limbs: u64,
    denominator_limbs: u64,
) -> Result<u64, VMError> {
    checked_add(
        numerator_limbs.max(1),
        checked_mul(2, denominator_limbs.max(1))?,
    )
}

/// Work for the final signed-domain scan of a conceptual result.
pub const fn finalization_work(value_limbs: u64) -> u64 {
    if value_limbs == 0 { 1 } else { value_limbs }
}

/// Work for converting/truncating/sign-filling an intermediate modulo `2^512`.
pub fn wrapping_reduction_work(source_limbs: u64) -> Result<u64, VMError> {
    let source = source_limbs.max(1);
    checked_add(
        source,
        checked_add(
            checked_mul(3, MAX_VALUE_LIMBS)?,
            source.min(MAX_VALUE_LIMBS),
        )?,
    )
}

/// Checked integer negation including generic and V1 signed-domain scans.
pub fn checked_int_unary_work(value_limbs: u64) -> Result<u64, VMError> {
    let value = value_limbs.max(1);
    let result = checked_add(value, 1)?;
    checked_add(value, checked_mul(2, result)?)
}

/// Checked integer add/subtract including generic and V1 domain scans.
pub fn checked_int_additive_work(lhs_limbs: u64, rhs_limbs: u64) -> Result<u64, VMError> {
    let operands = lhs_limbs.max(rhs_limbs).max(1);
    let result = checked_add(operands, 1)?;
    checked_add(operands, checked_mul(2, result)?)
}

/// Checked integer multiplication including generic and V1 domain scans.
pub fn checked_int_multiplication_work(lhs_limbs: u64, rhs_limbs: u64) -> Result<u64, VMError> {
    let lhs = lhs_limbs.max(1);
    let rhs = rhs_limbs.max(1);
    let result = checked_add(lhs, rhs)?;
    checked_add(multiplication_work(lhs, rhs)?, checked_mul(2, result)?)
}

/// Checked integer quotient/remainder including both generic and V1 scans.
pub fn checked_int_division_work(dividend_limbs: u64, divisor_limbs: u64) -> Result<u64, VMError> {
    let dividend = dividend_limbs.max(1);
    let divisor = divisor_limbs.max(1);
    let quotient = quotient_limb_bound(dividend, divisor)?;
    let remainder = dividend.min(divisor);
    checked_add(
        quotient_remainder_work(dividend, divisor)?,
        checked_mul(2, checked_add(quotient, remainder)?)?,
    )
}

/// Generic wrapping arithmetic before the explicit 512-bit reduction.
pub fn wrapping_unary_work(value_limbs: u64) -> Result<u64, VMError> {
    let value = value_limbs.max(1);
    checked_add(value, checked_add(value, 1)?)
}

/// Generic wrapping add/subtract before the explicit 512-bit reduction.
pub fn wrapping_additive_work(lhs_limbs: u64, rhs_limbs: u64) -> Result<u64, VMError> {
    let operands = lhs_limbs.max(rhs_limbs).max(1);
    checked_add(operands, checked_add(operands, 1)?)
}

/// Generic wrapping multiplication before the explicit 512-bit reduction.
pub fn wrapping_multiplication_work(lhs_limbs: u64, rhs_limbs: u64) -> Result<u64, VMError> {
    let lhs = lhs_limbs.max(1);
    let rhs = rhs_limbs.max(1);
    checked_add(multiplication_work(lhs, rhs)?, checked_add(lhs, rhs)?)
}

/// Work for one exact or rounded division scale attempt.
pub fn division_attempt_work(
    numerator_limbs: u64,
    numerator_scale_delta: u8,
    scaled_numerator_limbs: u64,
    denominator_limbs: u64,
    denominator_scale_delta: u8,
    scaled_denominator_limbs: u64,
) -> Result<u64, VMError> {
    checked_add(
        checked_add(
            alignment_operand_work(numerator_limbs, numerator_scale_delta)?,
            alignment_operand_work(denominator_limbs, denominator_scale_delta)?,
        )?,
        quotient_remainder_work(scaled_numerator_limbs, scaled_denominator_limbs)?,
    )
}

/// Convert logical limb work to gas using checked `u64` arithmetic.
pub fn work_gas(limb_work: u64) -> Result<u64, VMError> {
    checked_mul(NUMERIC_GAS_PER_LIMB_WORK, limb_work)
}

/// Gas for one core-reported arithmetic step.
///
/// The primitive layer invokes the observer immediately before performing each
/// bounded work step. This conversion is the sole VM mapping from that logical
/// work protocol into gas.
pub fn work_step_gas(step: NumericWorkStep) -> Result<u64, VMError> {
    let work = match step {
        NumericWorkStep::CanonicalityProbe { mantissa_limbs, .. } => {
            // Quotient/remainder by the one-limb constant ten.
            quotient_remainder_work(u64::from(mantissa_limbs), 1)?
        }
        NumericWorkStep::ScaleByPowerOfTen {
            value_limbs,
            exponent,
        } => scale_work(u64::from(value_limbs), exponent)?,
        NumericWorkStep::Materialize { value_limbs } => {
            materialization_work(u64::from(value_limbs))
        }
        NumericWorkStep::Negate { value_limbs } => u64::from(value_limbs).max(1),
        NumericWorkStep::Add {
            lhs_limbs,
            rhs_limbs,
        }
        | NumericWorkStep::Subtract {
            lhs_limbs,
            rhs_limbs,
        } => u64::from(lhs_limbs.max(rhs_limbs)).max(1),
        NumericWorkStep::Multiply {
            lhs_limbs,
            rhs_limbs,
        } => multiplication_work(u64::from(lhs_limbs), u64::from(rhs_limbs))?,
        NumericWorkStep::Normalize { mantissa_limbs, .. } => {
            quotient_remainder_work(u64::from(mantissa_limbs), 1)?
        }
        NumericWorkStep::ExactDivisionAttempt {
            numerator_limbs,
            denominator_limbs,
            ..
        } => quotient_remainder_work(u64::from(numerator_limbs), u64::from(denominator_limbs))?,
        NumericWorkStep::RoundedDivision {
            numerator_limbs,
            denominator_limbs,
            ..
        } => rounded_division_work(u64::from(numerator_limbs), u64::from(denominator_limbs))?,
        NumericWorkStep::DivisionClassification {
            dividend_limbs,
            divisor_limbs,
        } => quotient_remainder_work(u64::from(dividend_limbs), u64::from(divisor_limbs))?,
        NumericWorkStep::DivisionClassificationPrepare {
            numerator_limbs,
            denominator_limbs,
        } => classification_prepare_work(u64::from(numerator_limbs), u64::from(denominator_limbs))?,
        NumericWorkStep::Finalize { value_limbs } => finalization_work(u64::from(value_limbs)),
    };
    work_gas(work)
}

/// Complete successful-call formula used by golden tests and documentation.
///
/// Input and output lengths include the complete pointer envelopes; frame-byte
/// arguments account for authentication and output framing traversals. The
/// input count adds one fixed schema-frame decode charge per value; control
/// booleans add their stable validation phases. Canonical validation,
/// output-length, and normalization work remain explicit so they cannot
/// disappear into a codec or bigint backend.
#[expect(
    clippy::too_many_arguments,
    reason = "the explicit arguments mirror every independently audited term in the public gas formula"
)]
pub fn successful_call_gas(
    input_envelope_bytes: u64,
    input_hash_frame_bytes: u64,
    output_envelope_bytes: u64,
    output_frame_bytes: u64,
    output_length_limb_work: u64,
    arithmetic_limb_work: u64,
    validation_limb_work: u64,
    normalization_limb_work: u64,
) -> Result<u64, VMError> {
    let bytes = checked_add(
        checked_add(input_envelope_bytes, input_hash_frame_bytes)?,
        checked_add(output_envelope_bytes, checked_mul(2, output_frame_bytes)?)?,
    )?;
    let work = checked_add(
        checked_add(
            checked_add(arithmetic_limb_work, validation_limb_work)?,
            normalization_limb_work,
        )?,
        output_length_limb_work,
    )?;
    checked_add(checked_add(NUMERIC_ENTRY_GAS, bytes)?, work_gas(work)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow10_widths_pin_scale_boundaries() {
        assert_eq!(pow10_bit_length(0), Ok(1));
        assert_eq!(pow10_bit_length(19), Ok(64));
        assert_eq!(pow10_bit_length(28), Ok(94));
        assert_eq!(pow10_bit_length(56), Ok(187));
        assert_eq!(pow10_limbs(28), Ok(2));
        assert_eq!(pow10_limbs(56), Ok(3));
        assert_eq!(pow10_bit_length(57), Err(VMError::GasCostOverflow));
        assert_eq!(power_construction_work(0), Ok(0));
        assert_eq!(power_construction_work(1), Ok(1));
        assert_eq!(power_construction_work(19), Ok(19));
        assert_eq!(power_construction_work(20), Ok(20));
        assert_eq!(power_construction_work(28), Ok(36));
        assert_eq!(power_construction_work(56), Ok(109));
    }

    #[test]
    fn limb_boundaries_are_monotonic_and_zero_is_one_limb() {
        assert_eq!(limbs_for_bits(0), 1);
        assert_eq!(limbs_for_bits(1), 1);
        assert_eq!(limbs_for_bits(64), 1);
        assert_eq!(limbs_for_bits(65), 2);
        assert_eq!(limbs_for_bits(512), 8);
        assert_eq!(MAX_VALUE_LIMBS, 8);
        assert_eq!(scaled_limbs(511, 28), Ok(10));
    }

    #[test]
    fn frame_validation_accounts_for_structural_and_body_passes_without_transport_double_counting()
    {
        assert_eq!(numeric_frame_validation_work(0), Ok(2));
        assert_eq!(numeric_frame_validation_phase_work(1), Ok((1, 1)));
        assert_eq!(numeric_frame_validation_phase_work(8), Ok((1, 1)));
        assert_eq!(numeric_frame_validation_phase_work(9), Ok((2, 1)));
        assert_eq!(numeric_frame_validation_phase_work(44), Ok((6, 1)));
        assert_eq!(numeric_frame_validation_work(44), Ok(7));
        assert_eq!(numeric_frame_validation_phase_work(109), Ok((14, 9)));
        assert_eq!(envelope_tail_bytes(44), Ok(76));
        assert_eq!(payload_hash_gas(44), Ok(76));
        assert_eq!(payload_hash_gas(108), Ok(140));
        assert_eq!(output_serialization_gas(83, 44), Ok(171));
        assert_eq!(output_serialization_gas(147, 108), Ok(363));
    }

    #[test]
    fn scaled_limb_bound_covers_decimal_power_boundary_products() {
        assert_eq!(scaled_limbs(0, 28), Ok(1));
        assert_eq!(scaled_limbs(64, 0), Ok(1));
        assert_eq!(scaled_limbs(65, 0), Ok(2));

        // (2^61 - 1) * 10 has 65 bits. Using
        // `value_bits + bit_length(10) - 1` would incorrectly charge one limb.
        assert_eq!(scaled_limbs(61, 1), Ok(2));
        assert_eq!(scaled_limbs(60, 1), Ok(1));
    }

    #[test]
    fn multiplication_and_scale_intermediates_exceed_value_width() {
        assert_eq!(multiplication_work(8, 8), Ok(64));
        assert_eq!(aligned_work(8, 28, 10, 1, 0, 1), Ok(63));
        assert_eq!(MAX_PRODUCT_SCALE, 56);
        assert_eq!(MAX_PRODUCT_LIMBS, 16);
    }

    #[test]
    fn division_formula_pins_all_width_branches() {
        assert_eq!(division_work(1, 2), Ok(5));
        assert_eq!(division_work(1, 1), Ok(3));
        assert_eq!(division_work(8, 3), Ok(29));
        assert_eq!(division_work(10, 8), Ok(42));
    }

    #[test]
    fn quotient_remainder_formula_covers_derived_remainder_work() {
        assert_eq!(quotient_limb_bound(1, 2), Ok(1));
        assert_eq!(quotient_limb_bound(8, 3), Ok(6));
        assert_eq!(quotient_remainder_work(1, 2), Ok(8));
        assert_eq!(quotient_remainder_work(1, 1), Ok(5));
        assert_eq!(quotient_remainder_work(8, 3), Ok(55));
        assert_eq!(quotient_remainder_work(10, 8), Ok(76));
        assert_eq!(rounded_division_work(1, 1), Ok(13));
        assert_eq!(rounded_division_work(2, 1), Ok(18));
        assert_eq!(rounded_division_work(10, 8), Ok(114));
        assert_eq!(classification_prepare_work(8, 3), Ok(14));
        assert_eq!(finalization_work(16), 16);
    }

    #[test]
    fn integer_domain_and_wrapping_passes_are_pinned() {
        assert_eq!(checked_int_unary_work(8), Ok(26));
        assert_eq!(checked_int_additive_work(8, 8), Ok(26));
        assert_eq!(checked_int_multiplication_work(8, 8), Ok(96));
        assert_eq!(checked_int_division_work(10, 8), Ok(98));
        assert_eq!(wrapping_unary_work(8), Ok(17));
        assert_eq!(wrapping_additive_work(8, 8), Ok(17));
        assert_eq!(wrapping_multiplication_work(8, 8), Ok(80));
        assert_eq!(wrapping_reduction_work(9), Ok(41));
        assert_eq!(wrapping_reduction_work(16), Ok(48));
    }

    #[test]
    fn observed_steps_map_to_stable_work_formulas() {
        assert_eq!(
            work_step_gas(NumericWorkStep::CanonicalityProbe {
                mantissa_limbs: 3,
                scale: 28,
            }),
            Ok(52)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::ScaleByPowerOfTen {
                value_limbs: 8,
                exponent: 56,
            }),
            Ok(532)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::Materialize { value_limbs: 8 }),
            Ok(32)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::Multiply {
                lhs_limbs: 8,
                rhs_limbs: 8,
            }),
            Ok(256)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::Normalize {
                mantissa_limbs: 3,
                remaining_scale: 28,
            }),
            Ok(52)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::Normalize {
                mantissa_limbs: 16,
                remaining_scale: 56,
            }),
            Ok(260),
            "a maximum-width multiplication intermediate is pinned explicitly"
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::ExactDivisionAttempt {
                numerator_limbs: 8,
                denominator_limbs: 3,
                output_scale: 28,
            }),
            Ok(220)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::RoundedDivision {
                numerator_limbs: 2,
                denominator_limbs: 1,
                output_scale: 28,
            }),
            Ok(72)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::DivisionClassificationPrepare {
                numerator_limbs: 8,
                denominator_limbs: 3,
            }),
            Ok(56)
        );
        assert_eq!(
            work_step_gas(NumericWorkStep::Finalize { value_limbs: 16 }),
            Ok(64)
        );
    }

    #[test]
    fn checked_formulas_reject_host_integer_overflow() {
        assert_eq!(checked_add(u64::MAX, 1), Err(VMError::GasCostOverflow));
        assert_eq!(checked_mul(u64::MAX, 2), Err(VMError::GasCostOverflow));
        assert_eq!(work_gas(u64::MAX), Err(VMError::GasCostOverflow));
        assert_eq!(
            successful_call_gas(u64::MAX, 1, 0, 0, 0, 0, 0, 0),
            Err(VMError::GasCostOverflow)
        );
    }

    #[test]
    fn zero_work_is_not_artificially_rounded_up() {
        assert_eq!(work_gas(0), Ok(0));
        assert_eq!(successful_call_gas(39, 0, 0, 0, 0, 0, 0, 0), Ok(55));
    }
}
