//! Simultaneous multiplication for four consecutive public generator folds.

use super::straus::{Addition, add_digits, straus_block};
use super::*;

/// Rows of 128 output lanes keep the fifteen width-5 odd-multiple tables
/// within the per-worker scratch size of the ordinary single-round kernel.
const LANES: usize = 128;

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
            add_digits::<15>(&mut columns, j, endomorphism, digits, false);
        }
    }
    Some(columns)
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
            let len = block.len();
            let terms =
                std::array::from_fn(|j| &bases[(j + 1) * n + start..(j + 1) * n + start + len]);
            if columns
                .as_ref()
                .is_some_and(|columns| straus_block::<C, 15, TABLE>(terms, block, columns))
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
