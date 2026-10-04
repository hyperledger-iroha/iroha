//! The halo2 permuted lookup (spec section 2 "Lookup", section 7 rows 2 and
//! 4, BlindingScheduleV1 items 2 and 4).
//!
//! For each lookup the prover compresses the input and table expressions
//! with `theta` (`A = fold(acc * theta + a_i)`, likewise `S`), permutes the
//! usable rows into `(A', S')` exactly as the vendored prover does, pads
//! rows `u..n` of `A'` and then of `S'` with random values, draws the `A'`
//! and `S'` blinds and writes both commitments. After `beta` and `gamma` it
//! builds the grand product
//!
//! ```text
//! z(omega^0) = 1,
//! z(omega^(i+1)) = z(omega^i) (A_i + beta)(S_i + gamma) / ((A'_i + beta)(S'_i + gamma))
//! ```
//!
//! for `i < u`, fills rows `n-b..n` with random values, draws its blind and
//! writes its commitment.
//!
//! # The permutation
//!
//! The usable input rows are grouped by value in ascending canonical-integer
//! order. Group `g` (value `v`, count `c`) emits `(v, v)` and then `c - 1`
//! pairs `(v, t)` with `t` taken in order from the slice
//! `[start_g - g, end_g - g - 1)` of the leftover table values: the sorted
//! usable table values that repeat their predecessor or do not occur in the
//! input. An input value missing from the table is a prover error. The
//! permutation runs in time that depends on the (secret) values, like the
//! vendored one.

use std::collections::BTreeMap;

use ff::Field;
use iroha_pasta::{PastaCurve, PastaField, field::batch_invert, msm::MemoryBudget};
use rand_core_06::RngCore;

use super::{ProverError, quotient::CompiledExpressions, random_values, write_point};
use crate::{
    keys::ProvingKey,
    pcs::ipa::{PinnedParams, commit::Secrecy},
    protocol::Shape,
    transcript::TranscriptWrite,
};

/// A lookup after its permuted columns are committed.
pub(super) struct Permuted<F> {
    compressed_input: Vec<F>,
    compressed_table: Vec<F>,
    permuted_input: Vec<F>,
    permuted_table: Vec<F>,
    input_poly: Vec<F>,
    input_blind: F,
    table_poly: Vec<F>,
    table_blind: F,
}

/// A lookup after its product is committed.
pub(super) struct Committed<F> {
    /// `A'` in coefficient form.
    pub(super) input_poly: Vec<F>,
    /// The blind of `A'`.
    pub(super) input_blind: F,
    /// `S'` in coefficient form.
    pub(super) table_poly: Vec<F>,
    /// The blind of `S'`.
    pub(super) table_blind: F,
    /// `z` in coefficient form.
    pub(super) product_poly: Vec<F>,
    /// The blind of `z`.
    pub(super) product_blind: F,
}

/// The vendored permutation of the usable rows of `input` and `table`
/// (see the module documentation), padded to `n` rows: `A'` padding first,
/// then `S'` padding.
///
/// # Errors
///
/// [`ProverError::LookupInputMissing`] when an input value is not in the
/// table.
pub(super) fn permute<F: PastaField, R: RngCore>(
    input: &[F],
    table: &[F],
    usable_rows: usize,
    n: usize,
    lookup: usize,
    rng: &mut R,
) -> Result<(Vec<F>, Vec<F>), ProverError> {
    let missing = ProverError::LookupInputMissing { lookup };
    let input = input.get(..usable_rows).ok_or(missing.clone())?;
    let table = table.get(..usable_rows).ok_or(missing.clone())?;
    let mut counts: BTreeMap<F, usize> = BTreeMap::new();
    for value in input {
        *counts.entry(*value).or_insert(0) += 1;
    }
    let mut sorted = table.to_vec();
    sorted.sort_unstable();
    if counts.keys().any(|value| sorted.binary_search(value).is_err()) {
        return Err(missing);
    }
    let leftover: Vec<F> = sorted
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            (*index != 0 && sorted[index - 1] == **value) || !counts.contains_key(*value)
        })
        .map(|(_, value)| *value)
        .collect();
    let mut permuted_input = Vec::with_capacity(n);
    let mut permuted_table = Vec::with_capacity(n);
    let mut start = 0_usize;
    for (group, (value, count)) in counts.iter().enumerate() {
        permuted_input.push(*value);
        permuted_table.push(*value);
        // [start - g, end - g - 1) with end = start + count.
        let from = start.checked_sub(group).ok_or(missing.clone())?;
        let to = (start + count)
            .checked_sub(group + 1)
            .ok_or(missing.clone())?;
        let extra = leftover.get(from..to).ok_or(missing.clone())?;
        for table_value in extra {
            permuted_input.push(*value);
            permuted_table.push(*table_value);
        }
        start += count;
    }
    if permuted_input.len() != usable_rows {
        return Err(missing);
    }
    let pad = n - usable_rows;
    permuted_input.extend(random_values::<F, _>(rng, pad));
    permuted_table.extend(random_values::<F, _>(rng, pad));
    Ok((permuted_input, permuted_table))
}

/// Compresses, permutes, blinds, commits and writes every lookup's `A'` and
/// `S'` (BlindingScheduleV1 item 2).
///
/// # Errors
///
/// [`ProverError::LookupInputMissing`], or MSM, FFT and transcript errors.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_permuted<C, T, R>(
    params: &PinnedParams<C>,
    pk: &ProvingKey<C>,
    shape: &Shape,
    compiled: &CompiledExpressions<C::ScalarExt>,
    advice_values: &[Vec<C::ScalarExt>],
    instance_values: &[Vec<C::ScalarExt>],
    theta: C::ScalarExt,
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<Vec<Permuted<C::ScalarExt>>, ProverError>
where
    C: PastaCurve,
    T: TranscriptWrite<C>,
    R: RngCore,
{
    if shape.lookups == 0 {
        return Ok(Vec::new());
    }
    let compressed = compiled.compress_lookups(
        pk.fixed_values(),
        advice_values,
        instance_values,
        theta,
        shape.n,
    )?;
    let tables = pk.commitment_tables();
    let mut permuted = Vec::with_capacity(compressed.len());
    for (lookup, (compressed_input, compressed_table)) in compressed.into_iter().enumerate() {
        let (permuted_input, permuted_table) = permute(
            &compressed_input,
            &compressed_table,
            shape.usable_rows,
            shape.n,
            lookup,
            rng,
        )?;
        let input_blind = C::ScalarExt::random(&mut *rng);
        let table_blind = C::ScalarExt::random(&mut *rng);
        let input_commitment = tables
            .commit_lagrange(
                params.params(),
                &permuted_input,
                &input_blind,
                Secrecy::Secret,
                budget,
            )?
            .to_affine();
        let table_commitment = tables
            .commit_lagrange(
                params.params(),
                &permuted_table,
                &table_blind,
                Secrecy::Secret,
                budget,
            )?
            .to_affine();
        write_point(transcript, &input_commitment)?;
        write_point(transcript, &table_commitment)?;
        let mut input_poly = permuted_input.clone();
        pk.domain().ifft(&mut input_poly)?;
        let mut table_poly = permuted_table.clone();
        pk.domain().ifft(&mut table_poly)?;
        permuted.push(Permuted {
            compressed_input,
            compressed_table,
            permuted_input,
            permuted_table,
            input_poly,
            input_blind,
            table_poly,
            table_blind,
        });
    }
    Ok(permuted)
}

/// Builds, blinds, commits and writes every lookup's product `z`
/// (BlindingScheduleV1 item 4).
///
/// # Errors
///
/// MSM, FFT and transcript errors.
#[allow(clippy::too_many_arguments)]
pub(super) fn commit_products<C, T, R>(
    params: &PinnedParams<C>,
    pk: &ProvingKey<C>,
    shape: &Shape,
    permuted: Vec<Permuted<C::ScalarExt>>,
    beta: C::ScalarExt,
    gamma: C::ScalarExt,
    rng: &mut R,
    transcript: &mut T,
    budget: MemoryBudget,
) -> Result<Vec<Committed<C::ScalarExt>>, ProverError>
where
    C: PastaCurve,
    T: TranscriptWrite<C>,
    R: RngCore,
{
    let tables = pk.commitment_tables();
    let u = shape.usable_rows;
    let mut committed = Vec::with_capacity(permuted.len());
    for lookup in permuted {
        // Denominators (A'_i + beta)(S'_i + gamma) of the usable rows.
        let mut fractions: Vec<C::ScalarExt> = lookup.permuted_input[..u]
            .iter()
            .zip(&lookup.permuted_table[..u])
            .map(|(a, s)| (beta + a) * (gamma + s))
            .collect();
        batch_invert(&mut fractions);
        for ((fraction, a), s) in fractions
            .iter_mut()
            .zip(&lookup.compressed_input[..u])
            .zip(&lookup.compressed_table[..u])
        {
            *fraction *= (*a + beta) * (*s + gamma);
        }
        let mut product = Vec::with_capacity(shape.n);
        let mut running = C::ScalarExt::ONE;
        product.push(running);
        for fraction in &fractions {
            running *= fraction;
            product.push(running);
        }
        product.extend(random_values::<C::ScalarExt, _>(
            rng,
            shape.blinding_factors,
        ));
        let product_blind = C::ScalarExt::random(&mut *rng);
        let commitment = tables
            .commit_lagrange(
                params.params(),
                &product,
                &product_blind,
                Secrecy::Secret,
                budget,
            )?
            .to_affine();
        write_point(transcript, &commitment)?;
        pk.domain().ifft(&mut product)?;
        committed.push(Committed {
            input_poly: lookup.input_poly,
            input_blind: lookup.input_blind,
            table_poly: lookup.table_poly,
            table_blind: lookup.table_blind,
            product_poly: product,
            product_blind,
        });
    }
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use iroha_pasta::Fp;
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    use super::*;

    fn values(raw: &[u64]) -> Vec<Fp> {
        raw.iter().map(|value| Fp::from(*value)).collect()
    }

    #[test]
    fn permutation_matches_the_vendored_layout() {
        // usable = 6, n = 8. Inputs {3, 1, 3, 2, 1, 3}; table {1, 2, 3, 4, 3, 5}.
        let input = values(&[3, 1, 3, 2, 1, 3, 0, 0]);
        let table = values(&[1, 2, 3, 4, 3, 5, 0, 0]);
        let mut rng = ChaCha20Rng::seed_from_u64(5);
        let (a, s) = permute(&input, &table, 6, 8, 0, &mut rng).expect("permute");
        // Groups 1 (x2), 2 (x1), 3 (x3); sorted table 1 2 3 3 4 5 gives the
        // leftovers [3, 4, 5].
        assert_eq!(&a[..6], values(&[1, 1, 2, 3, 3, 3]).as_slice());
        assert_eq!(&s[..6], values(&[1, 3, 2, 3, 4, 5]).as_slice());
        // A' and S' are permutations of the usable inputs and table values.
        let mut sorted_a = a[..6].to_vec();
        sorted_a.sort_unstable();
        let mut sorted_input = input[..6].to_vec();
        sorted_input.sort_unstable();
        assert_eq!(sorted_a, sorted_input);
        // Each A' row equals its predecessor or the S' row beside it.
        for row in 0..6 {
            assert!(a[row] == s[row] || (row > 0 && a[row] == a[row - 1]));
        }
        // Padding: A' rows first, then S' rows, from the same stream.
        let mut replay = ChaCha20Rng::seed_from_u64(5);
        let pads: Vec<Fp> = random_values(&mut replay, 4);
        assert_eq!(&a[6..], &pads[..2]);
        assert_eq!(&s[6..], &pads[2..]);
    }

    #[test]
    fn missing_inputs_are_prover_errors() {
        let input = values(&[1, 9, 0, 0]);
        let table = values(&[1, 2, 0, 0]);
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        assert_eq!(
            permute(&input, &table, 2, 4, 3, &mut rng).err(),
            Some(ProverError::LookupInputMissing { lookup: 3 })
        );
        assert_eq!(
            permute(&input, &table, 5, 4, 0, &mut rng).err(),
            Some(ProverError::LookupInputMissing { lookup: 0 })
        );
    }
}
