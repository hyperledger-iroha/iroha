//! Bounded Gregorian calendar arithmetic shared by base-field and extension AIR.
//!
//! The caller binds the six decimal components, DER time encoding and timestamp
//! uses through the RFC lookup arguments. This kernel constrains the conversion
//! itself, without inspecting or branching on a field element.

use super::{F, PolynomialAirFieldV1};

pub(super) const WIDTH_V1: usize = 164;
pub(super) const MONTH: usize = 0;
const Q4: usize = MONTH + 12;
const Q100: usize = Q4 + 12;
const Q400: usize = Q100 + 7;
const R4: usize = Q400 + 5;
const R100: usize = R4 + 2;
const R100_SLACK: usize = R100 + 7;
const R400: usize = R100_SLACK + 7;
const R400_SLACK: usize = R400 + 9;
const YEAR_LOWER: usize = R400_SLACK + 9;
const YEAR_UPPER: usize = YEAR_LOWER + 13;
const DAY_MINUS_ONE: usize = YEAR_UPPER + 13;
const MONTH_SLACK: usize = DAY_MINUS_ONE + 5;
const HOUR: usize = MONTH_SLACK + 5;
const HOUR_SLACK: usize = HOUR + 5;
const MINUTE: usize = HOUR_SLACK + 5;
const MINUTE_SLACK: usize = MINUTE + 6;
const SECOND: usize = MINUTE_SLACK + 6;
const SECOND_SLACK: usize = SECOND + 6;
const UTC_YEAR: usize = SECOND_SLACK + 6;
const UTC_YEAR_SLACK: usize = UTC_YEAR + 7;
const ZERO: usize = UTC_YEAR_SLACK + 7;
const INVERSE: usize = ZERO + 3;
const LEAP: usize = INVERSE + 3;
pub(super) const GENERALIZED: usize = LEAP + 1;
const CENTURY_2000: usize = GENERALIZED + 1;
pub(super) const RAW_YEAR: usize = CENTURY_2000 + 1;
const _: () = assert!(RAW_YEAR + 1 == WIDTH_V1);
const MONTH_PREFIX: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const BIT_RANGES: [(usize, usize); 21] = [
    (MONTH, 12),
    (Q4, 12),
    (Q100, 7),
    (Q400, 5),
    (R4, 2),
    (R100, 7),
    (R100_SLACK, 7),
    (R400, 9),
    (R400_SLACK, 9),
    (YEAR_LOWER, 13),
    (YEAR_UPPER, 13),
    (DAY_MINUS_ONE, 5),
    (MONTH_SLACK, 5),
    (HOUR, 5),
    (HOUR_SLACK, 5),
    (MINUTE, 6),
    (MINUTE_SLACK, 6),
    (SECOND, 6),
    (SECOND_SLACK, 6),
    (UTC_YEAR, 7),
    (UTC_YEAR_SLACK, 7),
];
/// Exact arithmetic-only residue count, before the caller's phase constraints.
pub(super) const RESIDUES_V1: usize = 193;

fn packed<A: PolynomialAirFieldV1>(witness: &[A], offset: usize, width: usize) -> A {
    witness[offset..offset + width]
        .iter()
        .copied()
        .enumerate()
        .fold(A::ZERO, |sum, (bit, value)| {
            sum.add(value.mul(A::from_base(F(1 << bit))))
        })
}

/// Append gated constraints for year/month/day/hour/minute/second, timestamp,
/// and days before the year. Every ungated polynomial has degree at most two.
pub(super) fn append_residues_v1<A: PolynomialAirFieldV1>(
    values: [A; 8],
    witness: &[A],
    gate: A,
    residues: &mut Vec<A>,
) {
    assert_eq!(witness.len(), WIDTH_V1);
    let start = residues.len();
    let [
        year,
        month,
        day,
        hour,
        minute,
        second,
        timestamp,
        days_before_year,
    ] = values;
    let constant = |value| A::from_base(F(value));
    for (offset, width) in BIT_RANGES {
        for bit in &witness[offset..offset + width] {
            residues.push(gate.mul(bit.mul(bit.sub(A::ONE))));
        }
    }
    let selectors = &witness[MONTH..MONTH + 12];
    residues.push(gate.mul(selectors.iter().copied().fold(A::ZERO, A::add).sub(A::ONE)));
    residues.push(
        gate.mul(
            month.sub(
                selectors
                    .iter()
                    .copied()
                    .enumerate()
                    .fold(A::ZERO, |sum, (index, selector)| {
                        sum.add(selector.mul(constant((index + 1) as u64)))
                    }),
            ),
        ),
    );
    let q4 = packed(witness, Q4, 12);
    let q100 = packed(witness, Q100, 7);
    let q400 = packed(witness, Q400, 5);
    let r4 = packed(witness, R4, 2);
    let r100 = packed(witness, R100, 7);
    let r400 = packed(witness, R400, 9);
    for (quotient, remainder, divisor) in [(q4, r4, 4), (q100, r100, 100), (q400, r400, 400)] {
        residues.push(gate.mul(year.sub(quotient.mul(constant(divisor))).sub(remainder)));
    }
    residues.push(gate.mul(r100.add(packed(witness, R100_SLACK, 7)).sub(constant(99))));
    residues.push(gate.mul(r400.add(packed(witness, R400_SLACK, 9)).sub(constant(399))));
    for (index, remainder) in [r4, r100, r400].into_iter().enumerate() {
        let zero = witness[ZERO + index];
        residues.push(gate.mul(zero.mul(zero.sub(A::ONE))));
        residues.push(
            gate.mul(
                remainder
                    .mul(witness[INVERSE + index])
                    .sub(A::ONE.sub(zero)),
            ),
        );
        residues.push(gate.mul(zero.mul(remainder)));
        // The zero case has one canonical inverse, including under masking.
        residues.push(gate.mul(zero.mul(witness[INVERSE + index])));
    }
    let leap = witness[LEAP];
    let generalized = witness[GENERALIZED];
    let century = witness[CENTURY_2000];
    let raw_year = witness[RAW_YEAR];
    for bit in [leap, generalized, century] {
        residues.push(gate.mul(bit.mul(bit.sub(A::ONE))));
    }
    residues.push(
        gate.mul(
            leap.sub(
                witness[ZERO]
                    .mul(A::ONE.sub(witness[ZERO + 1]))
                    .add(witness[ZERO + 2]),
            ),
        ),
    );
    // floor((y-1)/4)-floor((y-1)/100)+floor((y-1)/400)
    // equals q4-q100+q400-leap. The corresponding value at 1969 is 477.
    residues.push(
        gate.mul(
            days_before_year.sub(
                year.sub(constant(1970))
                    .mul(constant(365))
                    .add(q4)
                    .sub(q100)
                    .add(q400)
                    .sub(leap)
                    .sub(constant(477)),
            ),
        ),
    );
    residues.push(
        gate.mul(
            year.sub(constant(1970))
                .sub(generalized.mul(constant(80)))
                .sub(packed(witness, YEAR_LOWER, 13)),
        ),
    );
    residues.push(
        gate.mul(
            constant(2049)
                .add(generalized.mul(constant(7950)))
                .sub(year)
                .sub(packed(witness, YEAR_UPPER, 13)),
        ),
    );
    residues.push(
        gate.mul(
            year.sub(raw_year).sub(
                A::ONE
                    .sub(generalized)
                    .mul(constant(1900).add(century.mul(constant(100)))),
            ),
        ),
    );
    residues.push(gate.mul(generalized.mul(century)));
    residues.push(
        gate.mul(
            A::ONE
                .sub(generalized)
                .mul(raw_year)
                .sub(packed(witness, UTC_YEAR, 7)),
        ),
    );
    residues.push(
        gate.mul(
            A::ONE
                .sub(generalized)
                .mul(constant(99).sub(raw_year))
                .sub(packed(witness, UTC_YEAR_SLACK, 7)),
        ),
    );
    let prefix = selectors
        .iter()
        .copied()
        .enumerate()
        .fold(A::ZERO, |sum, (index, selector)| {
            sum.add(
                selector.mul(constant(MONTH_PREFIX[index]).add(if index >= 2 {
                    leap
                } else {
                    A::ZERO
                })),
            )
        });
    let month_days =
        selectors
            .iter()
            .copied()
            .enumerate()
            .fold(A::ZERO, |sum, (index, selector)| {
                sum.add(selector.mul(constant(MONTH_DAYS[index]).add(if index == 1 {
                    leap
                } else {
                    A::ZERO
                })))
            });
    let day_minus_one = packed(witness, DAY_MINUS_ONE, 5);
    residues.push(gate.mul(day.sub(day_minus_one).sub(A::ONE)));
    residues.push(gate.mul(month_days.sub(day).sub(packed(witness, MONTH_SLACK, 5))));
    for (value, bits, slack_bits, width, maximum) in [
        (hour, HOUR, HOUR_SLACK, 5, 23),
        (minute, MINUTE, MINUTE_SLACK, 6, 59),
        (second, SECOND, SECOND_SLACK, 6, 59),
    ] {
        residues.push(gate.mul(value.sub(packed(witness, bits, width))));
        residues.push(
            gate.mul(
                value
                    .add(packed(witness, slack_bits, width))
                    .sub(constant(maximum)),
            ),
        );
    }
    let computed = days_before_year
        .add(prefix)
        .add(day_minus_one)
        .mul(constant(24))
        .add(hour)
        .mul(constant(60))
        .add(minute)
        .mul(constant(60))
        .add(second);
    residues.push(gate.mul(timestamp.sub(computed)));
    debug_assert_eq!(residues.len() - start, RESIDUES_V1);
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn populate_v1(
    operands: super::CalendarOperandsV1,
    witness: &mut [F],
) -> Result<u64, super::ZkX509Rfc5280StarkErrorV1> {
    use super::ZkX509Rfc5280StarkErrorV1 as Error;
    assert_eq!(witness.len(), WIDTH_V1);
    let y = operands.year;
    let generalized = operands.generalized;
    let lower = if generalized { 2050 } else { 1970 };
    let upper = if generalized { 9999 } else { 2049 };
    if !(lower..=upper).contains(&y)
        || !(1..=12).contains(&operands.month)
        || operands.hour > 23
        || operands.minute > 59
        || operands.second > 59
    {
        return Err(Error::Semantic);
    }
    witness.fill(F::ZERO);
    let month = usize::try_from(operands.month - 1).map_err(|_| Error::Resource)?;
    let r4 = y % 4;
    let r100 = y % 100;
    let r400 = y % 400;
    let leap = r4 == 0 && r100 != 0 || r400 == 0;
    let days = MONTH_DAYS[month] + u64::from(leap && month == 1);
    if operands.day == 0 || operands.day > days {
        return Err(Error::Semantic);
    }
    let century = !generalized && y >= 2000;
    let raw_year = if generalized { y } else { y % 100 };
    let mut set_bits = |offset, width, value: u64| {
        for bit in 0..width {
            witness[offset + bit] = F((value >> bit) & 1);
        }
    };
    for (offset, width, value) in [
        (Q4, 12, y / 4),
        (Q100, 7, y / 100),
        (Q400, 5, y / 400),
        (R4, 2, r4),
        (R100, 7, r100),
        (R100_SLACK, 7, 99 - r100),
        (R400, 9, r400),
        (R400_SLACK, 9, 399 - r400),
        (YEAR_LOWER, 13, y - lower),
        (YEAR_UPPER, 13, upper - y),
        (DAY_MINUS_ONE, 5, operands.day - 1),
        (MONTH_SLACK, 5, days - operands.day),
        (HOUR, 5, operands.hour),
        (HOUR_SLACK, 5, 23 - operands.hour),
        (MINUTE, 6, operands.minute),
        (MINUTE_SLACK, 6, 59 - operands.minute),
        (SECOND, 6, operands.second),
        (SECOND_SLACK, 6, 59 - operands.second),
        (UTC_YEAR, 7, if generalized { 0 } else { raw_year }),
        (
            UTC_YEAR_SLACK,
            7,
            if generalized { 0 } else { 99 - raw_year },
        ),
    ] {
        set_bits(offset, width, value);
    }
    witness[MONTH + month] = F::ONE;
    for (index, remainder) in [r4, r100, r400].into_iter().enumerate() {
        witness[ZERO + index] = F(u64::from(remainder == 0));
        witness[INVERSE + index] = if remainder == 0 {
            F::ZERO
        } else {
            F(remainder).inv().ok_or(Error::Semantic)?
        };
    }
    witness[LEAP] = F(u64::from(leap));
    witness[GENERALIZED] = F(u64::from(generalized));
    witness[CENTURY_2000] = F(u64::from(century));
    witness[RAW_YEAR] = F(raw_year);
    Ok(365 * (y - 1970) + y / 4 - y / 100 + y / 400 - u64::from(leap) - 477)
}

#[cfg(test)]
mod tests {
    use super::super::{CalendarOperandsV1, ZkX509Rfc5280StarkErrorV1};
    use super::*;
    use time::{Date, Month, PrimitiveDateTime, Time};

    fn sample(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
    ) -> (CalendarOperandsV1, [F; WIDTH_V1], [F; 8]) {
        let timestamp = PrimitiveDateTime::new(
            Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap(),
            Time::from_hms(hour, minute, second).unwrap(),
        )
        .assume_utc()
        .unix_timestamp();
        let operands = CalendarOperandsV1 {
            generalized: year >= 2050,
            timestamp: u64::try_from(timestamp).unwrap(),
            year: year as u64,
            month: u64::from(month),
            day: u64::from(day),
            hour: u64::from(hour),
            minute: u64::from(minute),
            second: u64::from(second),
        };
        let mut witness = [F::ZERO; WIDTH_V1];
        let days = populate_v1(operands, &mut witness).unwrap();
        let values = [
            F(operands.year),
            F(operands.month),
            F(operands.day),
            F(operands.hour),
            F(operands.minute),
            F(operands.second),
            F(operands.timestamp),
            F(days),
        ];
        (operands, witness, values)
    }
    fn residues(values: [F; 8], witness: &[F; WIDTH_V1]) -> Vec<F> {
        let mut result = Vec::new();
        append_residues_v1(values, witness, F::ONE, &mut result);
        assert_eq!(result.len(), RESIDUES_V1);
        result
    }
    fn put_bits(witness: &mut [F; WIDTH_V1], offset: usize, width: usize, value: u64) {
        for bit in 0..width {
            witness[offset + bit] = F((value >> bit) & 1);
        }
    }

    #[test]
    fn gregorian_dates_match_independent_time_library_across_century_and_encoding_boundaries() {
        for year in [
            1970, 1971, 1972, 1999, 2000, 2001, 2048, 2049, 2050, 2099, 2100, 2101, 2399, 2400,
            9999,
        ] {
            for ordinal in 1..=366 {
                let Ok(date) = Date::from_ordinal_date(year, ordinal) else {
                    continue;
                };
                for (hour, minute, second) in [(0, 0, 0), (23, 59, 59)] {
                    let (_, witness, values) = sample(
                        year,
                        u8::from(date.month()),
                        date.day(),
                        hour,
                        minute,
                        second,
                    );
                    assert!(
                        residues(values, &witness)
                            .iter()
                            .all(|value| *value == F::ZERO),
                        "date={date} time={hour}:{minute}:{second}"
                    );
                }
            }
        }
        let (operands, expected, _) = sample(2000, 11, 2, 3, 4, 5);
        let mut reused = [F::ONE; WIDTH_V1];
        populate_v1(operands, &mut reused).unwrap();
        assert_eq!(reused, expected, "every reused witness cell is overwritten");
        assert_eq!(sample(1970, 1, 1, 0, 0, 0).2[6], F::ZERO);
        assert_eq!(sample(2000, 3, 1, 0, 0, 0).2[6], F(951_868_800));
        assert_eq!(sample(2100, 3, 1, 0, 0, 0).2[6], F(4_107_542_400));
        assert_eq!(sample(9999, 12, 31, 23, 59, 59).2[6], F(253_402_300_799));
    }

    #[test]
    fn calendar_rejects_alternate_divisions_out_of_range_fields_and_wrong_year_encoding() {
        let (_, witness, values) = sample(2100, 3, 1, 0, 0, 0);
        // Same integer identity y=100q+r with an invalid remainder. Seven
        // remainder bits alone would accept this alternate Euclidean division.
        let mut changed = witness;
        put_bits(&mut changed, Q100, 7, 20);
        put_bits(&mut changed, R100, 7, 100);
        assert!(residues(values, &changed).iter().any(|v| *v != F::ZERO));
        let mut changed = witness;
        put_bits(&mut changed, Q400, 5, 4);
        put_bits(&mut changed, R400, 9, 500);
        assert!(residues(values, &changed).iter().any(|v| *v != F::ZERO));
        for (index, replacement) in [
            (0, 1969),
            (0, 10000),
            (1, 0),
            (1, 13),
            (2, 0),
            (2, 32),
            (3, 24),
            (4, 60),
            (5, 60),
            (6, 0),
        ] {
            let mut changed = values;
            changed[index] = F(replacement);
            assert!(
                residues(changed, &witness).iter().any(|v| *v != F::ZERO),
                "index={index}, value={replacement}"
            );
        }
        for year in [1970, 1999, 2000, 2049, 2050, 9999] {
            let (_, witness, values) = sample(year, 11, 2, 3, 4, 5);
            for column in [GENERALIZED, CENTURY_2000, RAW_YEAR, LEAP, ZERO, INVERSE] {
                let mut changed = witness;
                changed[column] = changed[column].add(F::ONE);
                assert!(
                    residues(values, &changed).iter().any(|v| *v != F::ZERO),
                    "year={year}, column={column}"
                );
            }
        }
        for (year, month, day, hour, minute, second) in [
            (2100, 2, 29, 0, 0, 0),
            (2001, 2, 29, 0, 0, 0),
            (2000, 4, 31, 0, 0, 0),
            (2000, 1, 1, 24, 0, 0),
            (2000, 1, 1, 0, 60, 0),
            (2000, 1, 1, 0, 0, 60),
        ] {
            let operands = CalendarOperandsV1 {
                generalized: year >= 2050,
                timestamp: 0,
                year,
                month,
                day,
                hour,
                minute,
                second,
            };
            assert_eq!(
                populate_v1(operands, &mut [F::ZERO; WIDTH_V1]),
                Err(ZkX509Rfc5280StarkErrorV1::Semantic)
            );
        }
    }

    #[test]
    fn calendar_all_affine_inputs_have_declared_degree_four_with_a_quadratic_gate() {
        let mut evaluations = Vec::new();
        for t in 0..6 {
            let values = core::array::from_fn(|i| F((i as u64 + 3) * t + 7));
            let witness: [F; WIDTH_V1] = core::array::from_fn(|i| F((i as u64 + 5) * t + 11));
            let mut result = Vec::new();
            append_residues_v1(values, &witness, F(t + 13).mul(F(2 * t + 17)), &mut result);
            evaluations.push(result);
        }
        let mut degree_four_seen = false;
        for column in 0..RESIDUES_V1 {
            let mut differences = evaluations
                .iter()
                .map(|row| row[column])
                .collect::<Vec<_>>();
            for degree in 1..=5 {
                differences = differences
                    .windows(2)
                    .map(|pair| pair[1].sub(pair[0]))
                    .collect();
                if degree == 4 {
                    degree_four_seen |= differences.iter().any(|value| *value != F::ZERO);
                }
            }
            assert!(
                differences.iter().all(|value| *value == F::ZERO),
                "residue {column} exceeds degree four"
            );
        }
        assert!(degree_four_seen);
    }
}
