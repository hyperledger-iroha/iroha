//! Bounded verification of the exact DEEP compact replacement candidate.
//!
//! This owner joins the full statement, typed whole-message transcript, one OOD
//! AIR identity, exact authenticated fibers and the complete constant terminal.
//! It constructs neither a witness nor a trace/FFT/LDE. Its inputs are the
//! caller-prepared transfer relation and the canonical bounded proof frame.
//! The same-profile producer is joined through the offline Quantity facade.
//! TODO: Qualify complete generated artifacts, source/finality authentication,
//! privacy and cryptographic/resource bounds before production admission.

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use iroha_data_model::privacy::GoldilocksDigest384V1 as WireDigest;

use super::{
    deep_binding::{BindingError, Context, Message, Oracle, Transcript},
    deep_composition::DeepComposition,
    deep_geometry::{CONSTRAINTS, DeepGeometry, FRI_ARITIES, FRI_LENGTHS, QUERY_COUNT},
    deep_proof::{self, DeepProof, OpeningPlans},
    deep_relation::DeepRelation,
    fri_fold::FriFoldPlan,
    merkle_multiproof::MultiproofPlan,
};
use crate::{Error, Result, VerifyLimits, field::GoldilocksFp4V1 as F};

/// Actual bounded work of one fully accepted candidate; no admission authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VerificationWork {
    /// Complete canonical input frame length.
    pub(super) proof_bytes: usize,
    /// Exactly one evaluation of all 923 AIR slots at the sampled OOD point.
    pub(super) air_evaluations: usize,
    /// Authenticated row, quotient, FRI-fiber and terminal leaves.
    pub(super) leaf_hashes: usize,
    /// All reconstructed binary Merkle parents, including the sole terminal parent.
    pub(super) parent_hashes: usize,
    /// All commitment, OOD and transcript-chain H calls.
    pub(super) h_calls: usize,
    /// Complete indivisible verifier messages.
    pub(super) verifier_messages: usize,
    /// Materialized six-lane G blocks including every unused suffix word.
    pub(super) g_blocks: usize,
    /// Checked initial-query fold edges, including repeated shared fibers.
    pub(super) fold_checks: usize,
    /// Every terminal value checked against the degree-zero polynomial.
    pub(super) terminal_values: usize,
}

/// Authenticated row commitment and measured work from one complete verification.
/// Private fields prevent decoded or partially checked proofs from minting success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VerifiedDeepProof {
    row_root: WireDigest,
    work: VerificationWork,
}

impl VerifiedDeepProof {
    /// The row root of the same decoded proof which passed every check.
    pub(super) const fn row_root(&self) -> WireDigest {
        self.row_root
    }

    /// Measured verification work for that authenticated proof.
    pub(super) const fn work(&self) -> VerificationWork {
        self.work
    }
}

/// Check caller ceilings against fixed geometry before decoding or private proving.
///
/// `proof_bytes` may be MAX_FRAME_BYTES for conservative producer preflight.
/// Transition count belongs to the enclosing prepared public bundle and remains
/// checked there before constructing a segment; it cannot be read from proof bytes.
/// AIR rows have 301 retained values, quotient chunks have two Fp4 values, and
/// FRI groups have at most 16 values. The complete 128-value terminal has its own
/// immutable DTO/engine bound; it is not a queried FRI group.
pub(super) fn preflight(
    relation: &impl DeepRelation,
    proof_bytes: usize,
    limits: VerifyLimits,
) -> Result<()> {
    for (name, actual, maximum) in [
        (
            "max_proof_bytes",
            proof_bytes,
            limits.max_proof_bytes.min(deep_proof::MAX_FRAME_BYTES),
        ),
        (
            "max_compact_statement_bytes",
            relation.statement_bytes().len(),
            limits.max_batch_bytes,
        ),
        ("max_fri_layers", FRI_LENGTHS.len(), limits.max_fri_layers),
        ("max_queries", QUERY_COUNT, limits.max_queries),
        ("max_query_chunk_values", 2, limits.max_query_chunk_values),
        (
            "max_query_path_len",
            super::deep_geometry::LDE_ROWS.ilog2() as usize,
            limits.max_query_path_len,
        ),
        (
            "max_fri_round_values",
            *FRI_ARITIES.iter().max().expect("fixed nonempty arities"),
            limits.max_fri_round_values,
        ),
        (
            "max_air_row_values",
            super::compact_public_columns::COMMITTED_COLUMN_COUNT,
            limits.max_air_row_values,
        ),
    ] {
        if actual > maximum {
            return Err(Error::VerifierLimitExceeded {
                limit: name,
                actual,
                max: maximum,
            });
        }
    }
    Context::preflight_relation(relation).map_err(binding_error)
}

/// Verify with all segment policies and return only its authenticated commitment.
///
/// One canonical decode remains within the caller's allocation ceiling and any
/// stricter enclosing Norito scope. The same object then receives exactly one
/// full OOD AIR evaluation and all transcript, Merkle, FRI and terminal checks.
pub(super) fn verify_committed(
    relation: &impl DeepRelation,
    bytes: &[u8],
    limits: VerifyLimits,
    max_decode_allocation_charges: usize,
) -> Result<VerifiedDeepProof> {
    preflight(relation, bytes.len(), limits)?;
    let proof = deep_proof::decode_with_allocation(
        bytes,
        limits.max_proof_bytes,
        max_decode_allocation_charges,
    )?;
    verify_decoded(relation, &proof, bytes.len())
}

/// Test diagnostic using the same decoder and complete verifier implementation.
#[cfg(test)]
pub(super) fn verify(
    relation: &impl DeepRelation,
    bytes: &[u8],
    max_proof_bytes: usize,
) -> Result<VerificationWork> {
    // Decode enforces byte, aggregate allocation/element and shape ceilings
    // before any transcript expansion or public AIR preparation below.
    let proof = deep_proof::decode(bytes, max_proof_bytes)?;
    Ok(verify_decoded(relation, &proof, bytes.len())?.work())
}

fn verify_decoded(
    relation: &impl DeepRelation,
    proof: &DeepProof,
    proof_bytes: usize,
) -> Result<VerifiedDeepProof> {
    let geometry = DeepGeometry::new()?;
    let binding = Context::for_relation(relation).map_err(binding_error)?;
    let mut transcript = Transcript::new(binding.clone());
    if transcript.challenge().map_err(binding_error)? != Message::Dummy {
        return Err(shape(
            "DEEP transcript must start with its complete dummy message",
        ));
    }
    transcript
        .commit_root(Oracle::Row, proof.row_root.as_fastpq())
        .map_err(binding_error)?;
    let alphas = fields(&mut transcript, CONSTRAINTS)?;
    transcript
        .commit_root(Oracle::QuotientPair, proof.quotient_root.as_fastpq())
        .map_err(binding_error)?;
    let z = fields(&mut transcript, 1)?[0];
    let composition = geometry.check_ood(
        relation.deep_relation(),
        &alphas,
        z,
        &proof.ood.current,
        &proof.ood.next,
        &proof.ood.quotient,
    )?;
    transcript
        .commit_ood(&proof.ood.current, &proof.ood.next, &proof.ood.quotient)
        .map_err(binding_error)?;
    let lambda = fields(&mut transcript, 1)?[0];
    let mut betas = [F::ZERO; 5];
    for (round, beta) in betas.iter_mut().enumerate() {
        transcript
            .commit_root(Oracle::Fri(round as u8), proof.fri_roots[round].as_fastpq())
            .map_err(binding_error)?;
        *beta = fields(&mut transcript, 1)?[0];
    }
    transcript
        .commit_root(Oracle::Terminal, proof.fri_roots[5].as_fastpq())
        .map_err(binding_error)?;
    let Message::Queries(queries) = transcript.challenge().map_err(binding_error)? else {
        return Err(shape(
            "DEEP final transcript message must contain complete queries",
        ));
    };
    let queries: Vec<_> = queries.into_iter().map(|index| index as usize).collect();
    let plans = deep_proof::preflight(proof, &queries)?;
    let (leaf_hashes, parent_hashes) = authenticate(&binding, proof, &plans)?;
    let fold_checks = check_chains(&geometry, &composition, lambda, &betas, &queries, proof)?;
    let work = VerificationWork {
        proof_bytes,
        air_evaluations: 1,
        leaf_hashes,
        parent_hashes,
        h_calls: leaf_hashes + parent_hashes + 9 + 1,
        verifier_messages: 10,
        g_blocks: 637,
        fold_checks,
        terminal_values: proof.terminal.len(),
    };
    Ok(VerifiedDeepProof {
        row_root: proof.row_root,
        work,
    })
}

fn fields(transcript: &mut Transcript, expected: usize) -> Result<Vec<F>> {
    match transcript.challenge().map_err(binding_error)? {
        Message::Fields(values) if values.len() == expected => Ok(values),
        _ => Err(shape("DEEP transcript field message has another dimension")),
    }
}

fn authenticate(
    binding: &Context,
    proof: &DeepProof,
    plans: &OpeningPlans,
) -> Result<(usize, usize)> {
    let rows = proof
        .rows
        .iter()
        .map(|row| {
            let bytes: Vec<_> = row
                .values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            binding
                .hash_leaf(Oracle::Row, row.index, &bytes)
                .map_err(binding_error)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut parents = verify_tree(
        binding,
        Oracle::Row,
        &plans.initial,
        proof.row_root,
        &rows,
        &proof.row_siblings,
    )?;
    let pairs = proof
        .quotients
        .iter()
        .map(|pair| {
            let mut bytes = [0_u8; 64];
            bytes[..32].copy_from_slice(&pair.low.to_le_bytes());
            bytes[32..].copy_from_slice(&pair.high.to_le_bytes());
            binding
                .hash_leaf(Oracle::QuotientPair, pair.index, &bytes)
                .map_err(binding_error)
        })
        .collect::<Result<Vec<_>>>()?;
    parents += verify_tree(
        binding,
        Oracle::QuotientPair,
        &plans.initial,
        proof.quotient_root,
        &pairs,
        &proof.quotient_siblings,
    )?;
    let mut leaves = rows.len() + pairs.len();
    for (round, opening) in proof.rounds.iter().enumerate() {
        let oracle = Oracle::Fri(round as u8);
        let groups = opening
            .groups
            .iter()
            .map(|group| {
                let bytes: Vec<_> = group
                    .values
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect();
                binding
                    .hash_leaf(oracle, group.index, &bytes)
                    .map_err(binding_error)
            })
            .collect::<Result<Vec<_>>>()?;
        parents += verify_tree(
            binding,
            oracle,
            &plans.rounds[round],
            proof.fri_roots[round],
            &groups,
            &opening.siblings,
        )?;
        leaves += groups.len();
    }
    let terminal: Vec<_> = proof
        .terminal
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let terminal_leaf = binding
        .hash_leaf(Oracle::Terminal, 0, &terminal)
        .map_err(binding_error)?;
    parents += verify_tree(
        binding,
        Oracle::Terminal,
        &plans.terminal,
        proof.fri_roots[5],
        &[terminal_leaf],
        &[],
    )?;
    Ok((leaves + 1, parents))
}

fn verify_tree(
    binding: &Context,
    oracle: Oracle,
    plan: &MultiproofPlan,
    root: WireDigest,
    leaves: &[Digest],
    siblings: &[WireDigest],
) -> Result<usize> {
    let siblings: Vec<_> = siblings.iter().map(|value| value.as_fastpq()).collect();
    let work = plan.verify_parallel_with(
        root.as_fastpq(),
        leaves,
        &siblings,
        |level, index, left, right| {
            binding
                .hash_parent(oracle, level as u32, index as u32, left, right)
                .map_err(binding_error)
        },
    )?;
    Ok(work.parent_hashes)
}

// Called only after exact preflight, complete field decoding and authentication.
fn check_chains(
    geometry: &DeepGeometry,
    composition: &DeepComposition,
    lambda: F,
    betas: &[F; 5],
    queries: &[usize],
    proof: &DeepProof,
) -> Result<usize> {
    // The final exclusive degree is one: all 128 values, including unqueried
    // positions, must be the same constant. Sampling this vector is insufficient.
    let first = proof.terminal[0];
    if proof.terminal.iter().any(|&value| value != first) {
        return Err(shape(
            "DEEP terminal does not represent a constant polynomial",
        ));
    }
    let mut domain = geometry.domain();
    let mut domains = Vec::with_capacity(FRI_ARITIES.len());
    let mut folds = Vec::with_capacity(FRI_ARITIES.len());
    for (round, &arity) in FRI_ARITIES.iter().enumerate() {
        domains.push(domain);
        folds.push(FriFoldPlan::new(
            arity,
            domain.coset_generator(FRI_LENGTHS[round + 1]),
        )?);
        domain = domain.folded(arity);
    }
    for (ordinal, &initial) in queries.iter().enumerate() {
        let pair = &proof.quotients[ordinal];
        let mut value = composition.base_value_at(
            geometry.domain().point(initial),
            &proof.rows[ordinal].values,
            &[pair.low, pair.high],
            lambda,
        )?;
        let mut index = initial;
        for round in 0..FRI_ARITIES.len() {
            let next_len = FRI_LENGTHS[round + 1];
            let group_index = index % next_len;
            let coordinate = index / next_len;
            let position = proof.rounds[round]
                .groups
                .binary_search_by_key(&(group_index as u32), |group| group.index)
                .map_err(|_| shape("DEEP authenticated FRI fiber is missing"))?;
            let group = &proof.rounds[round].groups[position];
            if group.values[coordinate] != value {
                return Err(shape(
                    "DEEP composition or folded value differs from its authenticated fiber",
                ));
            }
            value = folds[round].fold_coset(
                &group.values,
                betas[round],
                domains[round].point(group_index),
            )?;
            index = group_index;
        }
        if proof.terminal[index] != value {
            return Err(shape(
                "DEEP final folded value differs from its authenticated terminal",
            ));
        }
    }
    Ok(queries.len() * FRI_ARITIES.len())
}

fn binding_error(error: BindingError) -> Error {
    Error::InvalidTraceShape {
        details: format!("DEEP binding: {error}"),
    }
}

fn shape(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
#[path = "deep_engine/tests.rs"]
mod tests;
