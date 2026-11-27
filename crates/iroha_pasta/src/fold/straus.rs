//! Batched simultaneous multiplication of public generators by public scalars.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Addition {
    row: usize,
    endomorphism: bool,
    negative: bool,
}

/// Insert one signed wNAF stream into a schedule shared by all output lanes.
pub(super) fn add_digits<const BASES: usize>(
    columns: &mut Vec<Vec<Addition>>,
    base: usize,
    endomorphism: bool,
    digits: &[i8],
    negative: bool,
) {
    if columns.len() < digits.len() {
        columns.resize_with(digits.len(), Vec::new);
    }
    for (column, &digit) in columns.iter_mut().zip(digits) {
        if digit != 0 {
            column.push(Addition {
                row: (usize::from(digit.unsigned_abs()) / 2) * BASES + base,
                endomorphism,
                negative: (digit < 0) ^ negative,
            });
        }
    }
}

/// Add the scheduled terms to the original output, which starts as the low
/// generator. An exceptional denominator leaves it intact for the caller's
/// complete-formula fallback.
pub(super) fn straus_block<C: PastaCurve, const BASES: usize, const ODDS: usize>(
    bases: [&[C::AffineExt]; BASES],
    output: &mut [C::AffineExt],
    columns: &[Vec<Addition>],
) -> bool {
    let m = output.len();
    if output
        .iter()
        .chain(bases.iter().flat_map(|base| base.iter()))
        .any(|p| bool::from(p.is_identity()))
    {
        return false;
    }
    // Table order is (odd multiple, generator, lane). All bases share one
    // inversion while building each new odd multiple.
    let row = BASES * m;
    let mut scratch = Scratch {
        den: vec![C::Base::ZERO; row],
        prefix: vec![C::Base::ZERO; row],
    };
    let mut xs = vec![C::Base::ZERO; ODDS * row];
    let mut ys = xs.clone();
    for (j, base) in bases.iter().enumerate() {
        for (i, point) in base.iter().enumerate() {
            xs[j * m + i] = point.x();
            ys[j * m + i] = point.y();
        }
    }
    let mut dx = xs[..row].to_vec();
    let mut dy = ys[..row].to_vec();
    if !ba_double(&mut dx, &mut dy, &mut scratch) {
        return false;
    }
    for index in 1..ODDS {
        let (prev_x, rest_x) = xs.split_at_mut(index * row);
        let (prev_y, rest_y) = ys.split_at_mut(index * row);
        let (x, y) = (&mut rest_x[..row], &mut rest_y[..row]);
        x.copy_from_slice(&prev_x[(index - 1) * row..]);
        y.copy_from_slice(&prev_y[(index - 1) * row..]);
        if !ba_add(x, y, &dx, &dy, false, &mut scratch) {
            return false;
        }
    }
    let beta = C::AffineExt::endo_beta();
    let endo_xs: Vec<_> = xs.iter().map(|&x| x * beta).collect();
    let mut ax = vec![C::Base::ZERO; m];
    let mut ay = ax.clone();
    let mut started = false;
    for column in columns.iter().rev() {
        if started && !ba_double(&mut ax, &mut ay, &mut scratch) {
            return false;
        }
        for add in column {
            let row = add.row * m..(add.row + 1) * m;
            let px = if add.endomorphism { &endo_xs } else { &xs };
            let (x, y) = (&px[row.clone()], &ys[row]);
            if started {
                if !ba_add(&mut ax, &mut ay, x, y, add.negative, &mut scratch) {
                    return false;
                }
            } else {
                ax.copy_from_slice(x);
                ay.copy_from_slice(y);
                if add.negative {
                    for v in &mut ay {
                        *v = -*v;
                    }
                }
                started = true;
            }
        }
    }
    if started {
        for (i, p) in output.iter().enumerate() {
            dx[i] = p.x();
            dy[i] = p.y();
        }
        if !ba_add(&mut ax, &mut ay, &dx, &dy, false, &mut scratch) {
            return false;
        }
        for (i, p) in output.iter_mut().enumerate() {
            *p = crate::curve::affine_unchecked::<C::AffineExt>(ax[i], ay[i]);
        }
    }
    true
}
