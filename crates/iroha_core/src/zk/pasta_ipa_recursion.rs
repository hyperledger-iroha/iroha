//! Shared fixed-profile accounting for Pasta IPA recursion.
//!
//! This module contains no proof-acceptance authority. It fixes the reviewed
//! Poseidon/direct-instance compilation profile and computes the exact Axiom
//! IPA transcript shape from a configured Halo2 constraint system. Callers
//! still have to authenticate the circuit, verifier key, and compiled protocol.

use ff::Field;
use halo2_proofs::plonk::{Any, Column, ConstraintSystem};

/// Width of the reviewed Pasta IPA Poseidon transcript.
pub(crate) const PASTA_IPA_POSEIDON_WIDTH_V1: usize = 3;
/// Rate of the reviewed Pasta IPA Poseidon transcript.
pub(crate) const PASTA_IPA_POSEIDON_RATE_V1: usize = 2;
/// Full rounds in the reviewed Pasta IPA Poseidon transcript.
pub(crate) const PASTA_IPA_POSEIDON_FULL_ROUNDS_V1: usize = 8;
/// Partial rounds in the reviewed Pasta IPA Poseidon transcript.
pub(crate) const PASTA_IPA_POSEIDON_PARTIAL_ROUNDS_V1: usize = 57;
/// `secure_mds` selector in the reviewed Pasta IPA Poseidon transcript.
pub(crate) const PASTA_IPA_POSEIDON_SECURE_MDS_V1: usize = 0;

/// Build the reviewed one-column, direct-instance IPA compilation profile.
///
/// Public instances are evaluated directly from the supplied field elements;
/// they are not committed and opened as proof polynomials. The matching prover
/// must therefore set `QUERY_INSTANCE = false`.
pub(crate) fn pasta_ipa_direct_instance_compile_config_v1(
    public_len: usize,
) -> snark_verifier::system::halo2::Config {
    snark_verifier::system::halo2::Config::ipa()
        .set_query_instance(false)
        .with_num_instance(vec![public_len])
}

/// Compute the exact Axiom IPA augmented-proof length for the direct-instance profile.
///
/// The accounting mirrors Halo2 Axiom and `snark-verifier`: selector
/// polynomials are current-row fixed queries, permutation products are chunked
/// by `degree - 2`, and the final BGH19 folded generator contributes one extra
/// 32-byte transcript element.
///
/// # Errors
///
/// Returns an error for an invalid degree or any arithmetic/representation
/// overflow while deriving the transcript shape.
pub(crate) fn pasta_ipa_augmented_proof_bytes_v1<F>(
    cs: &ConstraintSystem<F>,
    k: u32,
) -> Result<u32, String>
where
    F: Field,
{
    let degree = cs.degree();
    let permutation_chunk_size = degree
        .checked_sub(2)
        .filter(|size| *size != 0)
        .ok_or_else(|| "proof-size preflight has invalid degree".to_owned())?;
    let permutation_columns = cs.permutation().get_columns().len();
    let permutation_chunks = permutation_columns.div_ceil(permutation_chunk_size);
    let selectors = cs.num_selectors();
    let fixed_queries = cs.fixed_queries().len();
    let fixed_query_evaluations = fixed_queries
        .checked_add(selectors)
        .ok_or_else(|| "fixed-query count overflow".to_owned())?;

    let mut column_queries =
        std::collections::BTreeMap::<Column<Any>, std::collections::BTreeSet<i32>>::new();
    for (column, rotation) in cs.advice_queries() {
        column_queries
            .entry((*column).into())
            .or_default()
            .insert(rotation.0);
    }
    for (column, rotation) in cs.fixed_queries() {
        column_queries
            .entry((*column).into())
            .or_default()
            .insert(rotation.0);
    }
    for column in cs.permutation().get_columns() {
        column_queries.entry(column).or_default().insert(0);
    }
    let mut point_sets = column_queries
        .into_values()
        .map(|rotations| rotations.into_iter().collect::<Vec<_>>())
        .collect::<std::collections::BTreeSet<_>>();
    if selectors != 0 {
        point_sets.insert(vec![0]);
    }
    if !cs.lookups().is_empty() {
        point_sets.insert(vec![0, 1]);
        point_sets.insert(vec![-1, 0]);
        point_sets.insert(vec![0]);
    }
    if permutation_columns != 0 {
        point_sets.insert(vec![0, 1]);
        if permutation_columns > permutation_chunk_size {
            let chained_rotation = i32::try_from(
                cs.blinding_factors()
                    .checked_add(1)
                    .ok_or_else(|| "blinding-factor overflow".to_owned())?,
            )
            .map_err(|_| "blinding factor does not fit i32".to_owned())?;
            point_sets.insert(vec![-chained_rotation, 0, 1]);
        }
    }

    let lookups = cs.lookups().len();
    let ipa_rounds = usize::try_from(k).map_err(|_| "IPA degree does not fit usize".to_owned())?;
    let ipa_commitments = ipa_rounds
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| "IPA commitment count overflow".to_owned())?;
    let commitments = cs
        .num_advice_columns()
        .checked_add(
            lookups
                .checked_mul(3)
                .ok_or_else(|| "lookup commitment count overflow".to_owned())?,
        )
        .and_then(|count| count.checked_add(permutation_chunks))
        .and_then(|count| count.checked_add(degree))
        .and_then(|count| count.checked_add(1))
        .and_then(|count| count.checked_add(ipa_commitments))
        .ok_or_else(|| "proof commitment count overflow".to_owned())?;
    let permutation_evaluations = if permutation_chunks == 0 {
        0
    } else {
        permutation_chunks
            .checked_mul(3)
            .and_then(|count| count.checked_sub(1))
            .ok_or_else(|| "permutation evaluation count overflow".to_owned())?
    };
    let evaluations = cs
        .advice_queries()
        .len()
        .checked_add(fixed_query_evaluations)
        .and_then(|count| count.checked_add(lookups.checked_mul(5)?))
        .and_then(|count| count.checked_add(permutation_evaluations))
        .and_then(|count| count.checked_add(permutation_columns))
        .and_then(|count| count.checked_add(1))
        .and_then(|count| count.checked_add(point_sets.len()))
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| "proof evaluation count overflow".to_owned())?;
    let transcript_elements = commitments
        .checked_add(evaluations)
        // The raw Halo2 IPA transcript ends with the final scalar `f`; KAGEMUSHA's
        // augmented shape then appends the 32-byte folded SRS generator derived by
        // `augment_halo2_ipa_proof_v1`.
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| "augmented proof element count overflow".to_owned())?;
    let transcript_bytes = transcript_elements
        .checked_mul(32)
        .ok_or_else(|| "augmented proof byte length overflow".to_owned())?;
    let augmented_proof_bytes = u32::try_from(transcript_bytes)
        .map_err(|_| "augmented proof byte length does not fit u32".to_owned())?;

    Ok(augmented_proof_bytes)
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{halo2curves::pasta::Fp, poly::Rotation};

    use super::*;

    #[test]
    fn direct_instance_queries_do_not_add_committed_openings() {
        let mut baseline = ConstraintSystem::<Fp>::default();
        let baseline_advice = baseline.advice_column();
        baseline.create_gate("query advice", |meta| {
            vec![meta.query_advice(baseline_advice, Rotation::cur())]
        });

        let mut with_instance = ConstraintSystem::<Fp>::default();
        let advice = with_instance.advice_column();
        let instance = with_instance.instance_column();
        with_instance.create_gate("bind advice to public instance", |meta| {
            let advice = meta.query_advice(advice, Rotation::cur());
            let instance = meta.query_instance(instance, Rotation::cur());
            vec![advice - instance]
        });

        let baseline_bytes =
            pasta_ipa_augmented_proof_bytes_v1(&baseline, 4).expect("baseline proof size");
        let direct_instance_bytes = pasta_ipa_augmented_proof_bytes_v1(&with_instance, 4)
            .expect("direct-instance proof size");

        assert_eq!(baseline_bytes, 672);
        assert_eq!(direct_instance_bytes, baseline_bytes);
    }
}
