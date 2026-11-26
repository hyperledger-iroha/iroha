//! Simultaneous multiplication for four consecutive public generator folds.

use super::*;

/// Rows of 128 output lanes keep the fifteen width-5 odd-multiple tables
/// within the per-worker scratch size of the ordinary single-round kernel.
const LANES: usize = 128;

#[derive(Clone, Copy)]
struct Addition {
    row: usize,
    endomorphism: bool,
    negative: bool,
}

/// Expand the four successive half-vector folds in the order the generators
/// appear in storage: the first challenge weights the most significant bit.
fn weights<F: PastaField>(challenges: &[F; 4]) -> [F; 16] {
    let mut weights = [F::ONE; 16];
    let mut width = 1;
    for u in challenges.iter().rev() {
        for i in 0..width {
            weights[width + i] = weights[i] * u;
        }
        width *= 2;
    }
    weights
}

/// For each bit, collect the odd multiples required by the GLV halves of
/// every scalar. The same additions and doublings apply to every output lane.
fn schedule<C: PastaCurve>(weights: &[C::ScalarExt; 16]) -> Option<Vec<Vec<Addition>>> {
    let mut columns = Vec::new();
    for (j, weight) in weights[1..].iter().enumerate() {
        let challenge = FoldChallenge::new::<C>(weight)?;
        for (endomorphism, digits) in [(false, &challenge.d1), (true, &challenge.d2)] {
            if columns.len() < digits.len() {
                columns.resize_with(digits.len(), Vec::new);
            }
            for (column, &digit) in columns.iter_mut().zip(digits) {
                if digit != 0 {
                    column.push(Addition {
                        row: (usize::from(digit.unsigned_abs()) / 2) * 15 + j,
                        endomorphism,
                        negative: digit < 0,
                    });
                }
            }
        }
    }
    Some(columns)
}

fn straus_block<C: PastaCurve>(
    input: &[C::AffineExt],
    start: usize,
    output: &mut [C::AffineExt],
    columns: &[Vec<Addition>],
) -> bool {
    let m = output.len();
    let stride = input.len() / 16;
    let points = |part: usize| &input[part * stride + start..part * stride + start + m];
    if (0..16).any(|part| points(part).iter().any(|p| bool::from(p.is_identity()))) {
        return false;
    }
    // Table order is (odd multiple, generator block, lane). All fifteen
    // blocks can share a single inversion while building each odd multiple.
    let row = 15 * m;
    let mut scratch = Scratch {
        den: vec![C::Base::ZERO; row],
        prefix: vec![C::Base::ZERO; row],
    };
    let mut xs = vec![C::Base::ZERO; TABLE * row];
    let mut ys = xs.clone();
    for j in 0..15 {
        for (i, point) in points(j + 1).iter().enumerate() {
            xs[j * m + i] = point.x();
            ys[j * m + i] = point.y();
        }
    }
    let mut dx = xs[..row].to_vec();
    let mut dy = ys[..row].to_vec();
    if !ba_double(&mut dx, &mut dy, &mut scratch) {
        return false;
    }
    for index in 1..TABLE {
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
        for (i, p) in points(0).iter().enumerate() {
            dx[i] = p.x();
            dy[i] = p.y();
        }
        if !ba_add(&mut ax, &mut ay, &dx, &dy, false, &mut scratch) {
            return false;
        }
        for (i, p) in output.iter_mut().enumerate() {
            *p = crate::curve::affine_unchecked::<C::AffineExt>(ax[i], ay[i]);
        }
    } else {
        output.copy_from_slice(points(0));
    }
    true
}

/// Apply four public IPA generator challenges to a borrowed basis, in
/// transcript order. Each output is the 16-term weighted sum obtained from
/// four applications of [`fold_generators_vartime`]. Simultaneous GLV
/// multiplication shares the doublings across all fifteen non-unit terms.
///
/// # Panics
/// Panics when the number of generators is not divisible by sixteen.
#[must_use]
pub fn folded_generators_four_vartime<C: PastaCurve>(
    bases: &[C::AffineExt],
    challenges: &[C::ScalarExt; 4],
) -> Vec<C::AffineExt> {
    assert!(
        bases.len().is_multiple_of(16),
        "four complete generator folds"
    );
    let n = bases.len() / 16;
    let weights = weights(challenges);
    let columns = schedule::<C>(&weights);
    let mut output = bases[..n].to_vec();
    output
        .par_chunks_mut(LANES)
        .enumerate()
        .with_max_len(1)
        .for_each(|(index, block)| {
            let start = index * LANES;
            if columns
                .as_ref()
                .is_some_and(|columns| straus_block::<C>(bases, start, block, columns))
            {
                return;
            }
            // Identities and exceptional additions use complete group formulas.
            let sums: Vec<C> = (start..start + block.len())
                .map(|lane| {
                    let mut sum = C::from(bases[lane]);
                    for (j, w) in weights.iter().enumerate().skip(1) {
                        sum += C::from(bases[j * n + lane]).mul_vartime(w);
                    }
                    sum
                })
                .collect();
            C::batch_normalize(&sums, block);
        });
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ep, Eq};

    fn check<C: PastaCurve>() {
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;

        let mut rng = ChaCha20Rng::seed_from_u64(811);
        let challenges = std::array::from_fn(|_| C::ScalarExt::random(&mut rng));
        let mut bases: Vec<_> = (0..16 * (LANES + 1))
            .map(|_| C::random(&mut rng).to_affine())
            .collect();
        let compare = |bases: &[C::AffineExt], us: &[C::ScalarExt; 4]| {
            let mut reference = bases.to_vec();
            for u in us {
                let h = fold_generators_reference::<C>(&mut reference, u);
                reference.truncate(h);
            }
            assert_eq!(folded_generators_four_vartime::<C>(bases, us), reference);
        };
        // Multiple arithmetic blocks, including a one-lane tail.
        compare(&bases, &challenges);
        let one = C::ScalarExt::ONE;
        let zero = C::ScalarExt::ZERO;
        compare(&[], &challenges);
        compare(&bases[..16], &[zero; 4]);
        compare(&bases[..32], &[one, zero, -one, challenges[3]]);
        // Complete formulas for identities and cancelling additions, including
        // a block for which the fast partial sums would reach infinity.
        bases[0] = C::AffineExt::default();
        compare(&bases[..32], &challenges);
        let base = C::generator().to_affine();
        let equal = [base; 16];
        compare(&equal, &[one; 4]);
        compare(&equal, &[one, one, one, -one]);
    }

    #[test]
    fn four_challenges_match_sequential_complete_folds_on_both_curves() {
        check::<Ep>();
        check::<Eq>();
    }
}
