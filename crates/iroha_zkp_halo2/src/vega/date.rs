//! Gregorian date, RFC 3339, expiry, and completed-age R1CS gadgets.

use super::{
    VegaT256ScalarV1 as Scalar,
    circuit::{Bit, CircuitBuilder, CircuitError, LinearCombination},
    sha256::{ByteVar, enforce_byte_constant},
};

#[derive(Clone)]
pub(super) struct DateVar {
    year: LinearCombination,
    month: LinearCombination,
    day: LinearCombination,
    year_bits: Vec<Bit>,
    month_bits: Vec<Bit>,
    day_bits: Vec<Bit>,
}

pub(super) fn parse_full_date(
    builder: &mut CircuitBuilder,
    bytes: &[ByteVar],
) -> Result<DateVar, CircuitError> {
    if bytes.len() != 10 {
        return Err(CircuitError::InvalidDimension);
    }
    enforce_byte_constant(builder, bytes[4], b'-')?;
    enforce_byte_constant(builder, bytes[7], b'-')?;
    let year = decimal(builder, &bytes[..4])?;
    let month = decimal(builder, &bytes[5..7])?;
    let day = decimal(builder, &bytes[8..])?;
    let date = date_from_lcs(builder, year, month, day)?;
    enforce_valid_date(builder, &date, 1)?;
    Ok(date)
}

pub(super) fn parse_rfc3339_seconds(
    builder: &mut CircuitBuilder,
    bytes: &[ByteVar],
) -> Result<DateVar, CircuitError> {
    if bytes.len() != 20 {
        return Err(CircuitError::InvalidDimension);
    }
    for (index, expected) in [(10, b'T'), (13, b':'), (16, b':'), (19, b'Z')] {
        enforce_byte_constant(builder, bytes[index], expected)?;
    }
    let date = parse_full_date(builder, &bytes[..10])?;
    let hour = decimal(builder, &bytes[11..13])?;
    let minute = decimal(builder, &bytes[14..16])?;
    let second = decimal(builder, &bytes[17..19])?;
    enforce_small_range(builder, hour, 7, 24, false)?;
    enforce_small_range(builder, minute, 7, 60, false)?;
    enforce_small_range(builder, second, 7, 60, false)?;
    Ok(date)
}

pub(super) fn public_date(
    builder: &mut CircuitBuilder,
    year_index: usize,
    month_index: usize,
    day_index: usize,
) -> Result<DateVar, CircuitError> {
    let year = builder.public(year_index)?;
    let month = builder.public(month_index)?;
    let day = builder.public(day_index)?;
    let date = date_from_lcs(builder, year.into(), month.into(), day.into())?;
    enforce_valid_date(builder, &date, 1970)?;
    enforce_less_than_constant(builder, &date.year_bits, 10_000, true)?;
    Ok(date)
}

pub(super) fn public_age_threshold(
    builder: &mut CircuitBuilder,
    index: usize,
) -> Result<(LinearCombination, Vec<Bit>), CircuitError> {
    let threshold: LinearCombination = builder.public(index)?.into();
    let bits = decompose(builder, threshold.clone(), 8)?;
    enforce_nonzero(builder, threshold.clone())?;
    enforce_less_than_constant(builder, &bits, 151, true)?;
    Ok((threshold, bits))
}

pub(super) fn enforce_completed_age(
    builder: &mut CircuitBuilder,
    birth: &DateVar,
    presentation: &DateVar,
    threshold: LinearCombination,
) -> Result<(), CircuitError> {
    let month_before_birthday = less_than(builder, &presentation.month_bits, &birth.month_bits)?;
    let same_month = builder.is_zero(presentation.month.clone().minus(&birth.month))?;
    let day_before_birthday = less_than(builder, &presentation.day_bits, &birth.day_bits)?;
    let same_month_and_day_before = builder.and(same_month, day_before_birthday)?;
    let before_birthday = builder.or(month_before_birthday, same_month_and_day_before)?;

    let presentation_year = scalar_to_u64(builder.evaluate(&presentation.year))?;
    let birth_year = scalar_to_u64(builder.evaluate(&birth.year))?;
    let threshold_value = scalar_to_u64(builder.evaluate(&threshold))?;
    let before_value = u64::from(builder.evaluate(&before_birthday.lc()) == Scalar::one());
    let required = birth_year
        .checked_add(threshold_value)
        .and_then(|value| value.checked_add(before_value));
    let slack = required
        .and_then(|required| presentation_year.checked_sub(required))
        .unwrap_or(0);
    let (_, slack_lc) = allocate_unsigned(builder, slack, 14)?;
    builder.enforce_equal(
        presentation.year.clone(),
        birth
            .year
            .clone()
            .plus(&threshold)
            .plus(&before_birthday.lc())
            .plus(&slack_lc),
    )
}

pub(super) fn enforce_strictly_after(
    builder: &mut CircuitBuilder,
    later: &DateVar,
    earlier: &DateVar,
) -> Result<(), CircuitError> {
    let later_code = date_code(later);
    let earlier_code = date_code(earlier);
    let later_value = scalar_to_u64(builder.evaluate(&later_code))?;
    let earlier_value = scalar_to_u64(builder.evaluate(&earlier_code))?;
    let slack = later_value
        .checked_sub(earlier_value)
        .and_then(|difference| difference.checked_sub(1))
        .unwrap_or(0);
    let (_, slack_lc) = allocate_unsigned(builder, slack, 24)?;
    builder.enforce_equal(
        later_code,
        earlier_code.plus(&LinearCombination::one()).plus(&slack_lc),
    )
}

pub(super) fn enforce_not_before(
    builder: &mut CircuitBuilder,
    later: &DateVar,
    earlier: &DateVar,
) -> Result<(), CircuitError> {
    let later_code = date_code(later);
    let earlier_code = date_code(earlier);
    let later_value = scalar_to_u64(builder.evaluate(&later_code))?;
    let earlier_value = scalar_to_u64(builder.evaluate(&earlier_code))?;
    let slack = later_value.saturating_sub(earlier_value);
    let (_, slack_lc) = allocate_unsigned(builder, slack, 24)?;
    builder.enforce_equal(later_code, earlier_code.plus(&slack_lc))
}

fn date_from_lcs(
    builder: &mut CircuitBuilder,
    year: LinearCombination,
    month: LinearCombination,
    day: LinearCombination,
) -> Result<DateVar, CircuitError> {
    Ok(DateVar {
        year_bits: decompose(builder, year.clone(), 14)?,
        month_bits: decompose(builder, month.clone(), 4)?,
        day_bits: decompose(builder, day.clone(), 6)?,
        year,
        month,
        day,
    })
}

fn enforce_valid_date(
    builder: &mut CircuitBuilder,
    date: &DateVar,
    minimum_year: u64,
) -> Result<(), CircuitError> {
    enforce_nonzero(builder, date.year.clone())?;
    if minimum_year > 1 {
        let below_minimum =
            enforce_less_than_constant(builder, &date.year_bits, minimum_year, false)?;
        builder.enforce_zero(below_minimum.lc())?;
    }
    enforce_less_than_constant(builder, &date.year_bits, 10_000, true)?;

    let month_selectors = equality_selectors(builder, date.month.clone(), 1..=12)?;
    let day_selectors = equality_selectors(builder, date.day.clone(), 1..=31)?;

    for month in [4_usize, 6, 9, 11] {
        builder.enforce(
            month_selectors[month - 1].lc(),
            day_selectors[30].lc(),
            LinearCombination::zero(),
        )?;
    }
    for day in [30_usize, 31] {
        builder.enforce(
            month_selectors[1].lc(),
            day_selectors[day - 1].lc(),
            LinearCombination::zero(),
        )?;
    }
    let divisible_by_4 = divisible(builder, date.year.clone(), 4, 12, 2)?;
    let divisible_by_100 = divisible(builder, date.year.clone(), 100, 7, 7)?;
    let divisible_by_400 = divisible(builder, date.year.clone(), 400, 5, 9)?;
    let not_divisible_by_100 = builder.not(divisible_by_100)?;
    let century_rule = builder.or(not_divisible_by_100, divisible_by_400)?;
    let leap_year = builder.and(divisible_by_4, century_rule)?;
    let february_29 = builder.and(month_selectors[1], day_selectors[28])?;
    builder.enforce(
        february_29.lc(),
        LinearCombination::one().minus(&leap_year.lc()),
        LinearCombination::zero(),
    )
}

fn equality_selectors(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
    range: core::ops::RangeInclusive<u64>,
) -> Result<Vec<Bit>, CircuitError> {
    let selectors = range
        .map(|candidate| {
            builder.is_zero(
                value
                    .clone()
                    .minus(&LinearCombination::constant(Scalar::from_u64(candidate))),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sum = selectors
        .iter()
        .fold(LinearCombination::zero(), |sum, selector| {
            sum.plus(&selector.lc())
        });
    builder.enforce_equal(sum, LinearCombination::one())?;
    Ok(selectors)
}

fn divisible(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
    divisor: u64,
    quotient_width: usize,
    remainder_width: usize,
) -> Result<Bit, CircuitError> {
    let assigned = scalar_to_u64(builder.evaluate(&value))?;
    let (_, quotient) = allocate_unsigned(builder, assigned / divisor, quotient_width)?;
    let (remainder_bits, remainder) =
        allocate_unsigned(builder, assigned % divisor, remainder_width)?;
    enforce_less_than_constant(builder, &remainder_bits, divisor, true)?;
    builder.enforce_equal(
        value,
        quotient.scaled(Scalar::from_u64(divisor)).plus(&remainder),
    )?;
    builder.is_zero(remainder)
}

fn decimal(
    builder: &mut CircuitBuilder,
    bytes: &[ByteVar],
) -> Result<LinearCombination, CircuitError> {
    if bytes.is_empty() {
        return Err(CircuitError::InvalidDimension);
    }
    let mut result = LinearCombination::zero();
    for byte in bytes {
        let digit = decimal_digit(builder, *byte)?;
        result = result.scaled(Scalar::from_u64(10)).plus(&digit);
    }
    Ok(result)
}

fn decimal_digit(
    builder: &mut CircuitBuilder,
    byte: ByteVar,
) -> Result<LinearCombination, CircuitError> {
    let byte_value = scalar_to_u64(builder.evaluate(&byte.lc()))?;
    let digit_value = byte_value.saturating_sub(u64::from(b'0'));
    let (bits, digit) = allocate_unsigned(builder, digit_value & 0xf, 4)?;
    builder.enforce_equal(
        byte.lc(),
        digit
            .clone()
            .plus(&LinearCombination::constant(Scalar::from_u64(u64::from(
                b'0',
            )))),
    )?;
    let high_and_middle = builder.or(bits[2], bits[1])?;
    builder.enforce(
        bits[3].lc(),
        high_and_middle.lc(),
        LinearCombination::zero(),
    )?;
    Ok(digit)
}

fn enforce_small_range(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
    width: usize,
    exclusive_maximum: u64,
    nonzero: bool,
) -> Result<Vec<Bit>, CircuitError> {
    let bits = decompose(builder, value.clone(), width)?;
    enforce_less_than_constant(builder, &bits, exclusive_maximum, true)?;
    if nonzero {
        enforce_nonzero(builder, value)?;
    }
    Ok(bits)
}

fn enforce_nonzero(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
) -> Result<(), CircuitError> {
    let zero = builder.is_zero(value)?;
    builder.enforce_zero(zero.lc())
}

fn decompose(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
    width: usize,
) -> Result<Vec<Bit>, CircuitError> {
    let assigned = scalar_to_u64(builder.evaluate(&value))?;
    let (bits, reconstructed) = allocate_unsigned(builder, assigned, width)?;
    builder.enforce_equal(value, reconstructed)?;
    Ok(bits)
}

fn allocate_unsigned(
    builder: &mut CircuitBuilder,
    value: u64,
    width: usize,
) -> Result<(Vec<Bit>, LinearCombination), CircuitError> {
    if width == 0 || width > 63 || value >= (1_u64 << width) {
        return Err(CircuitError::InvalidAssignment);
    }
    let bits = (0..width)
        .map(|bit| builder.alloc_bit(value & (1 << bit) != 0))
        .collect::<Result<Vec<_>, _>>()?;
    let reconstructed = bits_to_lc(&bits);
    Ok((bits, reconstructed))
}

fn enforce_less_than_constant(
    builder: &mut CircuitBuilder,
    value: &[Bit],
    constant: u64,
    require_less: bool,
) -> Result<Bit, CircuitError> {
    if value.is_empty() || value.len() > 63 {
        return Err(CircuitError::InvalidDimension);
    }
    if constant == (1_u64 << value.len()) {
        let less = constant_bit(builder, true)?;
        if require_less {
            builder.enforce_equal(less.lc(), LinearCombination::one())?;
        }
        return Ok(less);
    }
    if constant > (1_u64 << value.len()) {
        return Err(CircuitError::InvalidDimension);
    }
    let constant_bits = (0..value.len())
        .map(|bit| constant & (1 << bit) != 0)
        .collect::<Vec<_>>();
    let (_, borrow) = subtract(builder, value, &constant_bits)?;
    if require_less {
        builder.enforce_equal(borrow.lc(), LinearCombination::one())?;
    }
    Ok(borrow)
}

fn less_than(
    builder: &mut CircuitBuilder,
    left: &[Bit],
    right: &[Bit],
) -> Result<Bit, CircuitError> {
    if left.len() != right.len() {
        return Err(CircuitError::InvalidDimension);
    }
    let mut borrow = constant_bit(builder, false)?;
    for (left_bit, right_bit) in left.iter().copied().zip(right.iter().copied()) {
        let raw = i8::from(builder.evaluate(&left_bit.lc()) == Scalar::one())
            - i8::from(builder.evaluate(&right_bit.lc()) == Scalar::one())
            - i8::from(builder.evaluate(&borrow.lc()) == Scalar::one());
        let (difference_value, borrow_value) = if raw < 0 {
            (raw + 2 == 1, true)
        } else {
            (raw == 1, false)
        };
        let difference_bit = builder.alloc_bit(difference_value)?;
        let next_borrow = builder.alloc_bit(borrow_value)?;
        builder.enforce_zero(
            left_bit
                .lc()
                .minus(&right_bit.lc())
                .minus(&borrow.lc())
                .minus(&difference_bit.lc())
                .plus(&next_borrow.lc().scaled(Scalar::from_u64(2))),
        )?;
        borrow = next_borrow;
    }
    Ok(borrow)
}

fn subtract(
    builder: &mut CircuitBuilder,
    value: &[Bit],
    subtrahend: &[bool],
) -> Result<(Vec<Bit>, Bit), CircuitError> {
    if value.len() != subtrahend.len() {
        return Err(CircuitError::InvalidDimension);
    }
    let mut borrow = constant_bit(builder, false)?;
    let mut difference = Vec::with_capacity(value.len());
    for (value_bit, subtrahend_bit) in value.iter().copied().zip(subtrahend.iter().copied()) {
        let raw = i8::from(builder.evaluate(&value_bit.lc()) == Scalar::one())
            - i8::from(subtrahend_bit)
            - i8::from(builder.evaluate(&borrow.lc()) == Scalar::one());
        let (difference_value, borrow_value) = if raw < 0 {
            (raw + 2 == 1, true)
        } else {
            (raw == 1, false)
        };
        let difference_bit = builder.alloc_bit(difference_value)?;
        let next_borrow = builder.alloc_bit(borrow_value)?;
        let mut equation = value_bit
            .lc()
            .minus(&borrow.lc())
            .minus(&difference_bit.lc())
            .plus(&next_borrow.lc().scaled(Scalar::from_u64(2)));
        if subtrahend_bit {
            equation = equation.minus(&LinearCombination::one());
        }
        builder.enforce_zero(equation)?;
        difference.push(difference_bit);
        borrow = next_borrow;
    }
    Ok((difference, borrow))
}

fn constant_bit(builder: &mut CircuitBuilder, value: bool) -> Result<Bit, CircuitError> {
    let bit = builder.alloc_bit(value)?;
    builder.enforce_equal(
        bit.lc(),
        LinearCombination::constant(Scalar::from_u64(u64::from(value))),
    )?;
    Ok(bit)
}

fn bits_to_lc(bits: &[Bit]) -> LinearCombination {
    let mut coefficient = Scalar::one();
    bits.iter().fold(LinearCombination::zero(), |result, bit| {
        let next = result.add_term(bit.variable(), coefficient);
        coefficient += coefficient;
        next
    })
}

fn date_code(date: &DateVar) -> LinearCombination {
    date.year
        .clone()
        .scaled(Scalar::from_u64(512))
        .plus(&date.month.clone().scaled(Scalar::from_u64(32)))
        .plus(&date.day)
}

fn scalar_to_u64(value: Scalar) -> Result<u64, CircuitError> {
    let bytes = value.to_be_bytes();
    if bytes[..24].iter().any(|byte| *byte != 0) {
        return Err(CircuitError::InvalidAssignment);
    }
    Ok(u64::from_be_bytes(
        bytes[24..]
            .try_into()
            .map_err(|_| CircuitError::InvalidAssignment)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::sha256::allocate_bytes;

    fn check_date(value: &str, expected: bool) {
        let mut builder = CircuitBuilder::new(vec![Scalar::one()]).expect("public");
        let bytes = allocate_bytes(&mut builder, value.as_bytes()).expect("bytes");
        let result = parse_full_date(&mut builder, &bytes);
        let assignment = builder.finalize().expect("shape");
        let satisfied = assignment
            .shape
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); assignment.shape.constraint_count()],
            )
            .is_ok();
        assert_eq!(result.is_ok() && satisfied, expected, "{value}");
    }

    #[test]
    fn gregorian_boundaries_and_leap_rules_are_exact() {
        for valid in ["2000-02-29", "2024-02-29", "2026-07-26", "9999-12-31"] {
            check_date(valid, true);
        }
        for invalid in [
            "1900-02-29",
            "2025-02-29",
            "2026-04-31",
            "2026-13-01",
            "0000-01-01",
            "2026/07/26",
        ] {
            check_date(invalid, false);
        }
    }

    #[test]
    fn completed_age_and_strict_expiry_reject_boundary_attacks() {
        for (birth, expiry, accepted) in [
            ("2008-07-26", "2026-07-27", true),
            ("2008-07-27", "2026-07-27", false),
            ("2008-07-26", "2026-07-26", false),
        ] {
            let public = vec![
                Scalar::from_u64(2026),
                Scalar::from_u64(7),
                Scalar::from_u64(26),
                Scalar::from_u64(18),
            ];
            let mut builder = CircuitBuilder::new(public).expect("public");
            let birth_bytes = allocate_bytes(&mut builder, birth.as_bytes()).expect("birth bytes");
            let expiry_bytes =
                allocate_bytes(&mut builder, expiry.as_bytes()).expect("expiry bytes");
            let birth = parse_full_date(&mut builder, &birth_bytes).expect("birth shape");
            let expiry = parse_full_date(&mut builder, &expiry_bytes).expect("expiry shape");
            let presentation = public_date(&mut builder, 0, 1, 2).expect("public date");
            let (threshold, _) = public_age_threshold(&mut builder, 3).expect("threshold");
            enforce_completed_age(&mut builder, &birth, &presentation, threshold)
                .expect("age shape");
            enforce_strictly_after(&mut builder, &expiry, &presentation).expect("expiry shape");
            let assignment = builder.finalize().expect("shape");
            let satisfied = assignment
                .shape
                .validate_relaxed_assignment(
                    &assignment.witness,
                    Scalar::one(),
                    &assignment.public_inputs,
                    &vec![Scalar::zero(); assignment.shape.constraint_count()],
                )
                .is_ok();
            assert_eq!(satisfied, accepted);
        }
    }
}
