//! Fixed physical positions for DER time digits, terminators and UTC padding.
//!
//! Both encodings occupy fifteen rows. Every row carries the same authenticated
//! time-node identity; UTC's final two rows have no source-byte lookup.

use super::{F, PolynomialAirFieldV1};

/// Public byte-position template; fields never depend on witness values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DecimalTemplateV1 {
    pub(super) digit: u64,
    pub(super) group: u64,
    pub(super) offset: u64,
    pub(super) length: u64,
    pub(super) terminator: u64,
}

pub(super) fn template_v1(position: usize, generalized: bool) -> DecimalTemplateV1 {
    assert!(position < 15);
    let digits = if generalized { 14 } else { 12 };
    if position >= digits {
        return DecimalTemplateV1 {
            digit: 0,
            group: 0,
            offset: 0,
            length: 0,
            terminator: u64::from(position == digits),
        };
    }
    let year_digits = if generalized { 4 } else { 2 };
    let (group, offset, length) = if position < year_digits {
        (0, position, year_digits)
    } else {
        let local = position - year_digits;
        (1 + local / 2, local % 2, 2)
    };
    DecimalTemplateV1 {
        digit: 1,
        group: group as u64,
        offset: offset as u64,
        length: length as u64,
        terminator: 0,
    }
}

/// Ten reusable fixed cells: byte position; group/offset/length/digit/Z
/// templates. Length is always twice digit plus twice the generalized-year
/// flag, so its delta needs no extra fixed cell.
pub(super) fn fixed_cells_v1(position: usize) -> [F; 10] {
    let utc = template_v1(position, false);
    let generalized = template_v1(position, true);
    let delta = |left, right| F(right).sub(F(left));
    [
        F(position as u64),
        F(utc.group),
        delta(utc.group, generalized.group),
        F(utc.offset),
        delta(utc.offset, generalized.offset),
        F(u64::from(position < 4)),
        F(utc.digit),
        delta(utc.digit, generalized.digit),
        F(utc.terminator),
        delta(utc.terminator, generalized.terminator),
    ]
}

/// Expected group, offset, length, digit and Z selectors over any AIR field.
/// The templates are verifier-owned columns, so this is polynomial selection.
pub(super) fn expected_v1<A: PolynomialAirFieldV1>(fixed: &[A], generalized: A) -> [A; 5] {
    assert_eq!(fixed.len(), 10);
    let digit = fixed[6].add(generalized.mul(fixed[7]));
    [
        fixed[1].add(generalized.mul(fixed[2])),
        fixed[3].add(generalized.mul(fixed[4])),
        digit
            .mul(A::from_base(F(2)))
            .add(generalized.mul(fixed[5]).mul(A::from_base(F(2)))),
        digit,
        fixed[8].add(generalized.mul(fixed[9])),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_positions_cover_six_components_one_z_and_only_utc_padding() {
        for generalized in [false, true] {
            let mut counts = [0_usize; 6];
            let mut z = Vec::new();
            let mut padding = Vec::new();
            for position in 0..15 {
                let template = template_v1(position, generalized);
                assert_eq!(
                    expected_v1(&fixed_cells_v1(position), F(u64::from(generalized))),
                    [
                        F(template.group),
                        F(template.offset),
                        F(template.length),
                        F(template.digit),
                        F(template.terminator)
                    ]
                );
                if template.digit == 1 {
                    assert_eq!(template.offset, counts[template.group as usize] as u64);
                    counts[template.group as usize] += 1;
                } else if template.terminator == 1 {
                    z.push(position);
                } else {
                    padding.push(position);
                }
            }
            assert_eq!(counts, [if generalized { 4 } else { 2 }, 2, 2, 2, 2, 2]);
            assert_eq!(z, [if generalized { 14 } else { 12 }]);
            assert_eq!(padding, if generalized { vec![] } else { vec![13, 14] });
        }
    }
}
