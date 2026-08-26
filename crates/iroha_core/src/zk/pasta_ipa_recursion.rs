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

/// Public-instance opening policy used by an Axiom IPA transcript.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PastaIpaInstanceQueryV1 {
    /// Evaluate public instances directly and omit committed instance openings.
    Direct,
    /// Commit to and open every configured instance query.
    Queried,
}

/// Exact configured proof shape before any key generation or proving work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PastaIpaProofShapeV1 {
    degree: usize,
    advice_columns: usize,
    advice_queries: usize,
    instance_queries: usize,
    fixed_queries: usize,
    selectors: usize,
    lookups: usize,
    permutation_columns: usize,
    permutation_chunks: usize,
    point_sets: usize,
    commitments: usize,
    evaluations: usize,
    transcript_elements: usize,
    augmented_proof_bytes: u32,
}

impl PastaIpaProofShapeV1 {
    /// Maximum constraint degree.
    #[cfg(test)]
    pub(crate) const fn degree(&self) -> usize {
        self.degree
    }

    /// Number of advice columns committed by the prover.
    #[cfg(test)]
    pub(crate) const fn advice_columns(&self) -> usize {
        self.advice_columns
    }

    /// Number of distinct advice query pairs.
    #[cfg(test)]
    pub(crate) const fn advice_queries(&self) -> usize {
        self.advice_queries
    }

    /// Number of configured instance query pairs.
    #[cfg(test)]
    pub(crate) const fn instance_queries(&self) -> usize {
        self.instance_queries
    }

    /// Number of fixed-column query pairs before selector materialization.
    #[cfg(test)]
    pub(crate) const fn fixed_queries(&self) -> usize {
        self.fixed_queries
    }

    /// Number of selectors materialized as current-row fixed queries.
    #[cfg(test)]
    pub(crate) const fn selectors(&self) -> usize {
        self.selectors
    }

    /// Number of lookup arguments.
    #[cfg(test)]
    pub(crate) const fn lookups(&self) -> usize {
        self.lookups
    }

    /// Number of equality-enabled permutation columns.
    #[cfg(test)]
    pub(crate) const fn permutation_columns(&self) -> usize {
        self.permutation_columns
    }

    /// Number of permutation product-polynomial chunks.
    #[cfg(test)]
    pub(crate) const fn permutation_chunks(&self) -> usize {
        self.permutation_chunks
    }

    /// Number of unique multi-opening rotation sets.
    #[cfg(test)]
    pub(crate) const fn point_sets(&self) -> usize {
        self.point_sets
    }

    /// Number of curve commitments in the augmented transcript.
    #[cfg(test)]
    pub(crate) const fn commitments(&self) -> usize {
        self.commitments
    }

    /// Number of scalar evaluations in the augmented transcript.
    #[cfg(test)]
    pub(crate) const fn evaluations(&self) -> usize {
        self.evaluations
    }

    /// Total 32-byte transcript elements, including the folded-generator suffix.
    #[cfg(test)]
    pub(crate) const fn transcript_elements(&self) -> usize {
        self.transcript_elements
    }

    /// Exact augmented proof length in bytes.
    pub(crate) const fn augmented_proof_bytes(&self) -> u32 {
        self.augmented_proof_bytes
    }
}

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

/// Compute the exact Axiom IPA augmented-proof transcript shape.
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
pub(crate) fn pasta_ipa_augmented_proof_shape_v1<F>(
    cs: &ConstraintSystem<F>,
    k: u32,
    instance_query: PastaIpaInstanceQueryV1,
) -> Result<PastaIpaProofShapeV1, String>
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
    if instance_query == PastaIpaInstanceQueryV1::Queried {
        for (column, rotation) in cs.instance_queries() {
            column_queries
                .entry((*column).into())
                .or_default()
                .insert(rotation.0);
        }
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
    let proof_instance_evaluations = match instance_query {
        PastaIpaInstanceQueryV1::Direct => 0,
        PastaIpaInstanceQueryV1::Queried => cs.instance_queries().len(),
    };
    let evaluations = cs
        .advice_queries()
        .len()
        .checked_add(proof_instance_evaluations)
        .and_then(|count| count.checked_add(fixed_query_evaluations))
        .and_then(|count| count.checked_add(lookups.checked_mul(5)?))
        .and_then(|count| count.checked_add(permutation_evaluations))
        .and_then(|count| count.checked_add(permutation_columns))
        .and_then(|count| count.checked_add(1))
        .and_then(|count| count.checked_add(point_sets.len()))
        .and_then(|count| count.checked_add(2))
        .ok_or_else(|| "proof evaluation count overflow".to_owned())?;
    let transcript_elements = commitments
        .checked_add(evaluations)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| "augmented proof element count overflow".to_owned())?;
    let transcript_bytes = transcript_elements
        .checked_mul(32)
        .ok_or_else(|| "augmented proof byte length overflow".to_owned())?;
    let augmented_proof_bytes = u32::try_from(transcript_bytes)
        .map_err(|_| "augmented proof byte length does not fit u32".to_owned())?;

    Ok(PastaIpaProofShapeV1 {
        degree,
        advice_columns: cs.num_advice_columns(),
        advice_queries: cs.advice_queries().len(),
        instance_queries: cs.instance_queries().len(),
        fixed_queries,
        selectors,
        lookups,
        permutation_columns,
        permutation_chunks,
        point_sets: point_sets.len(),
        commitments,
        evaluations,
        transcript_elements,
        augmented_proof_bytes,
    })
}
