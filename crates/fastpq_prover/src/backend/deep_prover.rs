//! Complete bounded masked DEEP construction from a prepared public relation.
//!
//! Fresh explicitly supplied entropy feeds vanishing-masked base trace replay,
//! exact full-numerator division, randomized quotient chunks and independent R.
//! Every commitment and opening uses the same bounded replay owners. The final
//! canonical frame is checked by the independently implemented bounded verifier.
//! The caller binds authenticated ledger context before invoking this owner.
//! TODO: Complete the full resource diagnostic and independent soundness/hiding
//! review; construction alone does not establish those qualification results.

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use rand::TryCryptoRng;

use super::{
    compact_public_columns::COMMITTED_COLUMN_COUNT,
    deep_binding::{BindingError, Context, Message, Oracle, Transcript},
    deep_coefficient_commitment::{CoefficientCommitment, CoefficientCommitmentPlan},
    deep_coefficient_replay::{
        CoefficientLimits, CoefficientReplay, CoefficientReplayPlan, fold_coefficients,
    },
    deep_composition::OodPair,
    deep_engine,
    deep_geometry::{CONSTRAINTS, DeepGeometry, FRI_DEGREES, LDE_ROWS, QUERY_COUNT, TRACE_ROWS},
    deep_masked_quotient::{DeepQuotientPlan, QuotientLimits},
    deep_masked_replay::{
        MaskedReplayPlan, MaskedTraceReplay, ReplayLimits, TRACE_MASK_COEFFICIENTS,
    },
    deep_node_cache::{CommittedNodes, NodeCachePlan, PendingNodes, opening_payload_bytes},
    deep_polynomial::{DeepPolynomialSource, WORKSPACE_BYTES},
    deep_proof::{
        self, DeepProof, FriGroup, FriRound, FriValues, OodAnswers, OpeningPlans,
        QuotientMaskOpening,
    },
    deep_relation::DeepRelation,
    deep_striped_merkle::{RowCommitmentPlan, StreamLimits, open_cached_rows},
    deep_trace_source::OwnedTraceSource,
    masked_quotient::{checked_add as add, checked_mul as mul},
    secret_polynomial::SecretPolynomial,
};
use crate::{DigestExecutionV1, Error, Result, VerifyLimits, field::GoldilocksFp4V1 as F};

/// Explicit caller resource and execution budget, fixed before private work.
#[derive(Clone, Copy, Debug)]
pub(super) struct ConstructionLimits {
    pub(super) digest_execution: DigestExecutionV1,
    pub(super) max_payload_bytes: usize,
    pub(super) max_work_units: usize,
    pub(super) max_hash_calls: usize,
    pub(super) max_proof_bytes: usize,
}

/// One attempt's fixed pass schedule and conservative, checked payload/work bound.
///
/// Includes the consumed physical source, every retained private polynomial,
/// active stripe/tree/frame buffers, all public prefix-cache slots, proof/codec
/// storage and the self-check decode allowance. Phase buffers are sometimes
/// deliberately summed, including the source after its early release. Excludes allocator metadata, thread stacks, process-wide
/// constants, caller-owned AIR/RNG internals and unrelated caller allocations.
/// It is neither peak RSS nor a timing estimate.
pub(super) struct ProducerPlan<'a, R: DeepRelation> {
    relation: &'a R,
    binding: Context,
    replay: MaskedReplayPlan,
    replay_limits: ReplayLimits,
    quotient: DeepQuotientPlan<'a>,
    coefficient: CoefficientReplayPlan,
    fri: [CoefficientReplayPlan; 5],
    terminal: CoefficientReplayPlan,
    limits: ConstructionLimits,
    #[cfg(test)]
    pub(super) payload_bytes: usize,
    #[cfg(test)]
    pub(super) work_units: usize,
    #[cfg(test)]
    pub(super) hash_calls: usize,
}

impl<'a, R: DeepRelation> ProducerPlan<'a, R> {
    /// Derive the whole attempt before reading private columns or consuming RNG.
    /// Only the bounded public statement and small frontier plans allocate here.
    pub(super) fn new(relation: &'a R, limits: ConstructionLimits) -> Result<Self> {
        limit(
            "max_deep_proof_bytes",
            deep_proof::MAX_FRAME_BYTES,
            limits.max_proof_bytes,
        )?;
        let replay_limits = ReplayLimits {
            max_payload_bytes: limits.max_payload_bytes,
            max_work_units: limits.max_work_units,
            // Row root, the four-stripe numerator, and row openings. Charging
            // the numerator as a full pass is a conservative structural bound.
            max_full_passes: 3,
        };
        let replay = MaskedReplayPlan::new(replay_limits)?;
        let quotient = DeepQuotientPlan::new(
            relation.deep_relation(),
            replay,
            QuotientLimits {
                max_payload_bytes: limits.max_payload_bytes,
                max_work_units: limits.max_work_units,
            },
        )?;
        let binding = Context::for_relation(relation).map_err(binding_error)?;
        let coefficient_limits = CoefficientLimits {
            max_payload_bytes: limits.max_payload_bytes,
            max_work_units: limits.max_work_units,
            // Each scoped coefficient owner makes exactly one traversal. This
            // enclosing plan charges root and opening traversals separately.
            max_full_passes: 1,
        };
        let coefficient = CoefficientReplayPlan::quotient_and_mask(coefficient_limits)?;
        let fri: [CoefficientReplayPlan; 5] = (0..5)
            .map(|round| CoefficientReplayPlan::fri(round, coefficient_limits))
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .ok()
            .expect("five fixed coefficient layers");
        let terminal = CoefficientReplayPlan::terminal(coefficient_limits)?;
        let stream = stream_limits(limits);
        let queries = maximal_queries();
        let openings = OpeningPlans::new(&queries)?;
        let row = RowCommitmentPlan::new(replay, &binding, &queries, stream)?;
        let paired = CoefficientCommitmentPlan::new(
            coefficient,
            &binding,
            Oracle::QuotientAndMask,
            &queries,
            stream,
        )?;
        let mut coefficient_peak = paired.payload_bytes;
        let mut tree_hashes = mul(2, 2 * LDE_ROWS - 1)?;
        let mut cache_payload = 0;
        let mut cache_work = 0;
        let mut opening_payload = 0;
        for oracle in [
            Oracle::Row,
            Oracle::QuotientAndMask,
            Oracle::Fri(0),
            Oracle::Fri(1),
            Oracle::Fri(2),
            Oracle::Fri(3),
            Oracle::Fri(4),
        ] {
            let plan = NodeCachePlan::new(oracle)?;
            cache_payload = add(cache_payload, plan.payload_bytes)?;
            cache_work = add(cache_work, plan.work_units)?;
            opening_payload = opening_payload.max(opening_payload_bytes(oracle)?);
        }

        // The five-slot retained FRI cache Vec and the two scalar cache
        // owners are included explicitly. Also sum every pending owner even
        // though pending/completed phases cannot all coexist. Context clones
        // share the pre-existing immutable Arc prefix rather than its payload.
        let cache_owners = mul(
            7,
            add(
                size_of::<CommittedNodes<'static>>(),
                size_of::<PendingNodes>(),
            )?,
        )?;
        cache_payload = add(cache_payload, cache_owners)?;
        cache_work = add(cache_work, mul(cache_owners, 8)?)?;
        let mut coefficient_work = mul(2, coefficient.work_units)?;
        for (round, &layer) in fri.iter().enumerate() {
            let commitment = CoefficientCommitmentPlan::new(
                layer,
                &binding,
                Oracle::Fri(round as u8),
                &openings.round_indices[round],
                stream,
            )?;
            coefficient_peak = coefficient_peak.max(commitment.payload_bytes);
            tree_hashes = add(
                tree_hashes,
                add(commitment.leaf_hashes, commitment.parent_hashes)?,
            )?;
            coefficient_work = add(coefficient_work, mul(2, layer.work_units)?)?;
        }
        let terminal_commitment =
            CoefficientCommitmentPlan::new(terminal, &binding, Oracle::Terminal, &[], stream)?;
        coefficient_peak = coefficient_peak.max(terminal_commitment.payload_bytes);
        tree_hashes = add(tree_hashes, 2)?;
        coefficient_work = add(coefficient_work, terminal.work_units)?;
        // Retain both blinded chunks (each conservatively <2N) and every FRI
        // coefficient vector until final queries. No full-domain oracle survives.
        let retained_coefficients = mul(
            2 * FRI_DEGREES[0] + FRI_DEGREES.iter().sum::<usize>(),
            F::BYTES,
        )?;
        let active = active_phase_payload(
            quotient.payload_bytes,
            row.payload_bytes,
            add(replay.payload_bytes, coefficient_peak)?,
        )?;
        // The proof's bounded decode charge dominates its DTO's cells/Vec owners,
        // canonical output, frontier-plan storage and public transcript buffers.
        // Charge three separate allowances rather than relying on their lifetimes.
        // A second Context is created only by the independent final self-check.
        let public_and_codec = add(
            mul(
                2,
                binding
                    .maximum_retained_payload_bytes()
                    .map_err(binding_error)?,
            )?,
            mul(3, deep_proof::MAX_ALLOCATION_CHARGES)?,
        )?;
        let payload_bytes = add(
            add(active, add(cache_payload, opening_payload)?)?,
            add(
                retained_coefficients,
                add(WORKSPACE_BYTES, public_and_codec)?,
            )?,
        )?;
        // Horner OOD answers, coefficient batching/divisions/folds, selected row
        // packing and oracle packing all have fixed extents. These are structural
        // field/byte work units, separate from the explicit hash-call budget.
        let polynomial_work = mul(
            32,
            add(
                mul(COMMITTED_COLUMN_COUNT, TRACE_ROWS + TRACE_MASK_COEFFICIENTS)?,
                FRI_DEGREES.iter().sum(),
            )?,
        )?;
        let packing_work = mul(2 * LDE_ROWS, 8 * COMMITTED_COLUMN_COUNT + 3 * F::BYTES)?;
        // A second complete quotient work allowance safely covers the bounded
        // independent verifier/AIR check; it performs no quotient FFT or replay.
        let work_units = add(
            add(
                mul(2, quotient.work_units)?,
                add(cache_work, mul(opening_payload, 8)?)?,
            )?,
            add(coefficient_work, add(polynomial_work, packing_work)?)?,
        )?;
        let verifier_tree_hashes = add(
            mul(2, add(QUERY_COUNT, openings.initial.work().parent_hashes)?)?,
            add(
                openings
                    .rounds
                    .iter()
                    .map(|p| p.work().queried_leaves + p.work().parent_hashes)
                    .sum(),
                2,
            )?,
        )?;
        // Cached openings regenerate queried leaves and leaf-level siblings,
        // then reconstruct each original root once before DTO publication.
        let opening_hashes = |plan: &super::merkle_multiproof::MultiproofPlan| -> Result<usize> {
            add(
                plan.work().queried_leaves
                    + plan
                        .sibling_positions()
                        .iter()
                        .filter(|p| p.level == 0)
                        .count(),
                plan.work().parent_hashes,
            )
        };
        let mut cached_opening_hashes = mul(2, opening_hashes(&openings.initial)?)?;
        for plan in &openings.rounds {
            cached_opening_hashes = add(cached_opening_hashes, opening_hashes(plan)?)?;
        }
        // 637 whole-tape blocks, nine chain commits and one OOD hash per side.
        let hash_calls = add(
            crate::digest384_batch::MAX_PREFLIGHT_HASH_CALLS,
            add(
                tree_hashes,
                add(
                    cached_opening_hashes,
                    add(verifier_tree_hashes, 2 * (637 + 9 + 1))?,
                )?,
            )?,
        )?;
        limit(
            "max_deep_producer_payload_bytes",
            payload_bytes,
            limits.max_payload_bytes,
        )?;
        limit(
            "max_deep_producer_work_units",
            work_units,
            limits.max_work_units,
        )?;
        limit(
            "max_deep_producer_hash_calls",
            hash_calls,
            limits.max_hash_calls,
        )?;
        limit(
            "max_deep_producer_addressable_bytes",
            payload_bytes,
            isize::MAX as usize,
        )?;
        Ok(Self {
            relation,
            binding,
            replay,
            replay_limits,
            quotient,
            coefficient,
            fri,
            terminal,
            limits,
            #[cfg(test)]
            payload_bytes,
            #[cfg(test)]
            work_units,
            #[cfg(test)]
            hash_calls,
        })
    }

    /// Execute exactly the preflighted attempt; aborts never reuse entropy.
    pub(super) fn build(
        self,
        columns: OwnedTraceSource,
        rng: &mut impl TryCryptoRng,
    ) -> Result<Vec<u8>> {
        self.build_with_replay(rng, |limits, rng| columns.into_replay(limits, rng))
    }

    /// Preserve malformed borrowed-source controls without a second normal API.
    #[cfg(test)]
    fn build_from_borrowed_for_test(
        self,
        columns: &[&[u64]],
        rng: &mut impl TryCryptoRng,
    ) -> Result<Vec<u8>> {
        self.build_with_replay(rng, |limits, rng| {
            MaskedTraceReplay::new(limits, columns, rng)
        })
    }

    fn build_with_replay<T: TryCryptoRng>(
        self,
        rng: &mut T,
        initialize: impl FnOnce(ReplayLimits, &mut T) -> Result<MaskedTraceReplay>,
    ) -> Result<Vec<u8>> {
        let Self {
            relation,
            binding,
            replay: replay_plan,
            replay_limits,
            quotient: quotient_plan,
            coefficient,
            fri,
            terminal,
            limits,
            ..
        } = self;
        crate::digest384_batch::preflight_last_fields_execution(limits.digest_execution)?;
        let stream = stream_limits(limits);
        let geometry = DeepGeometry::new()?;
        let mut transcript = Transcript::new(binding.clone());
        if transcript.challenge().map_err(binding_error)? != Message::Dummy {
            return Err(invalid(
                "DEEP producer did not begin with dummy transcript message",
            ));
        }
        // Initialization consumes and clears the physical source before any tree
        // cache is allocated, preserving the original transcript/RNG order.
        let mut replay = initialize(replay_limits, rng)?;
        if replay.plan() != replay_plan {
            return Err(invalid("DEEP producer replay plan drift"));
        }
        let mut row_commitment = RowCommitmentPlan::new(replay_plan, &binding, &[], stream)?
            .commit(&mut replay, &binding)?;
        let row_root = row_commitment.root;
        let row_cache = row_commitment
            .cache
            .take()
            .ok_or_else(|| invalid("row root has no complete node cache"))?
            .bind(&binding, Oracle::Row, row_root)?;
        transcript
            .commit_root(Oracle::Row, row_root)
            .map_err(binding_error)?;
        let alphas = fields(&mut transcript, CONSTRAINTS)?;
        let quotient = quotient_plan.build(&mut replay, &alphas)?;
        let chunks = quotient.chunks();
        let mut quotient_commitment = commit(
            coefficient,
            &binding,
            Oracle::QuotientAndMask,
            &[],
            &[chunks[0], chunks[1], replay.composition_mask()],
            stream,
        )?;
        let quotient_root = quotient_commitment.root;
        let quotient_cache = quotient_commitment
            .cache
            .take()
            .ok_or_else(|| invalid("quotient root has no complete node cache"))?
            .bind(&binding, Oracle::QuotientAndMask, quotient_root)?;
        transcript
            .commit_root(Oracle::QuotientAndMask, quotient_root)
            .map_err(binding_error)?;
        let z = fields(&mut transcript, 1)?[0];
        let prepared = DeepPolynomialSource::from_replay(&replay, chunks)?
            .prepare(OodPair::new(z, geometry.trace_generator())?);
        let answers = prepared.trace_answers();
        let ood = OodAnswers {
            current: answers[0].to_vec(),
            next: answers[1].to_vec(),
            quotient: prepared.quotient_answers().to_vec(),
        };
        geometry.check_ood(
            relation.deep_relation(),
            &alphas,
            z,
            &ood.current,
            &ood.next,
            &ood.quotient,
        )?;
        transcript
            .commit_ood(&ood.current, &ood.next, &ood.quotient)
            .map_err(binding_error)?;
        let lambda = fields(&mut transcript, 1)?[0];
        let composition = prepared.compose(lambda, WORKSPACE_BYTES)?;
        drop(prepared);
        let mut roots = Vec::with_capacity(6);
        let mut fri_caches = Vec::with_capacity(5);
        let mut folded: Vec<SecretPolynomial<F>> = Vec::with_capacity(5);
        for (round, &plan) in fri.iter().enumerate() {
            let source = if round == 0 {
                composition.coefficients()
            } else {
                &folded[round - 1]
            };
            let oracle = Oracle::Fri(round as u8);
            let mut committed = commit(plan, &binding, oracle, &[], &[source], stream)?;
            let root = committed.root;
            fri_caches.push(
                committed
                    .cache
                    .take()
                    .ok_or_else(|| invalid("FRI root has no complete node cache"))?
                    .bind(&binding, oracle, root)?,
            );
            roots.push(root);
            transcript
                .commit_root(oracle, root)
                .map_err(binding_error)?;
            let beta = fields(&mut transcript, 1)?[0];
            folded.push(fold_coefficients(
                round,
                source,
                beta,
                limits.max_payload_bytes,
            )?);
        }
        let terminal_commitment = commit(
            terminal,
            &binding,
            Oracle::Terminal,
            &[],
            &[&folded[4]],
            stream,
        )?;
        roots.push(terminal_commitment.root);
        transcript
            .commit_root(Oracle::Terminal, terminal_commitment.root)
            .map_err(binding_error)?;
        let Message::Queries(queries) = transcript.challenge().map_err(binding_error)? else {
            return Err(invalid(
                "DEEP producer final message has no complete queries",
            ));
        };
        let queries: Vec<usize> = queries.into_iter().map(|v| v as usize).collect();
        let plans = OpeningPlans::new(&queries)?;
        let row = open_cached_rows(row_cache, &mut replay, &queries, limits.digest_execution)?;
        same_root(row_root, row.root)?;
        let paired = open(
            quotient_cache,
            coefficient,
            &queries,
            &[chunks[0], chunks[1], replay.composition_mask()],
            limits.digest_execution,
        )?;
        same_root(quotient_root, paired.root)?;
        let quotients = queries
            .iter()
            .zip(paired.openings())
            .map(|(&index, values)| QuotientMaskOpening {
                index: index as u32,
                low: values[0],
                high: values[1],
                composition_mask: values[2],
            })
            .collect();
        let mut rounds = Vec::with_capacity(5);
        for ((round, &plan), cache) in fri.iter().enumerate().zip(fri_caches) {
            let source = if round == 0 {
                composition.coefficients()
            } else {
                &folded[round - 1]
            };
            let opened = open(
                cache,
                plan,
                &plans.round_indices[round],
                &[source],
                limits.digest_execution,
            )?;
            same_root(roots[round], opened.root)?;
            let groups = plans.round_indices[round]
                .iter()
                .zip(opened.openings())
                .map(|(&index, values)| {
                    Ok(FriGroup {
                        index: index as u32,
                        values: FriValues::new(values.to_vec())?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            rounds.push(FriRound {
                groups,
                siblings: opened.siblings.into_iter().map(Into::into).collect(),
            });
        }
        let proof = DeepProof {
            row_root: row_root.into(),
            quotient_root: quotient_root.into(),
            fri_roots: roots.into_iter().map(Into::into).collect(),
            ood,
            rows: row.rows,
            quotients,
            row_siblings: row.siblings.into_iter().map(Into::into).collect(),
            quotient_siblings: paired.siblings.into_iter().map(Into::into).collect(),
            rounds,
            terminal: terminal_commitment.terminal()?.to_vec(),
        };
        deep_proof::preflight(&proof, &queries)?;
        let bytes = encode_bounded(&proof, limits.max_proof_bytes)?;
        // Erase all private coefficient owners before the independent decoder and
        // AIR verifier run. Only intentional proof disclosures survive this point.
        drop(folded);
        drop(composition);
        drop(quotient);
        drop(replay);
        deep_engine::verify_committed(
            relation,
            &bytes,
            VerifyLimits {
                max_batch_bytes: relation.statement_bytes().len(),
                max_proof_bytes: limits.max_proof_bytes,
                ..VerifyLimits::default()
            },
            deep_proof::MAX_ALLOCATION_CHARGES,
        )?;
        Ok(bytes)
    }
}

fn commit(
    plan: CoefficientReplayPlan,
    binding: &Context,
    oracle: Oracle,
    queries: &[usize],
    sources: &[&[F]],
    limits: StreamLimits,
) -> Result<CoefficientCommitment> {
    let commitment = CoefficientCommitmentPlan::new(plan, binding, oracle, queries, limits)?;
    let mut replay = CoefficientReplay::new(plan, sources)?;
    commitment.commit(&mut replay, binding)
}
fn fields(transcript: &mut Transcript, count: usize) -> Result<Vec<F>> {
    match transcript.challenge().map_err(binding_error)? {
        Message::Fields(values) if values.len() == count => Ok(values),
        _ => Err(invalid("DEEP producer transcript field count differs")),
    }
}
fn open(
    cache: CommittedNodes<'_>,
    plan: CoefficientReplayPlan,
    queries: &[usize],
    sources: &[&[F]],
    execution: DigestExecutionV1,
) -> Result<CoefficientCommitment> {
    let mut replay = CoefficientReplay::new(plan, sources)?;
    super::deep_coefficient_commitment::open_cached(cache, &mut replay, queries, execution)
}
fn same_root(expected: Digest, actual: Digest) -> Result<()> {
    if actual != expected {
        return Err(invalid(
            "DEEP replay opening root differs from committed root",
        ));
    }
    Ok(())
}
fn encode_bounded(proof: &DeepProof, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::canonical_frame_len(proof)
        .map_err(|error| invalid_owned(format!("DEEP canonical size: {error}")))?;
    limit(
        "max_deep_canonical_proof_bytes",
        bytes,
        maximum
            .min(deep_proof::PROOF_BYTE_TARGET)
            .min(deep_proof::MAX_FRAME_BYTES),
    )?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| invalid("DEEP canonical output allocation failed"))?;
    output.resize(bytes, 0);
    let mut target = &mut output[..];
    norito::core::write_canonical_to_writer(proof, &mut target)
        .map_err(|error| invalid_owned(format!("DEEP canonical encoding: {error}")))?;
    if !target.is_empty() {
        return Err(invalid("DEEP canonical writer did not fill exact frame"));
    }
    Ok(output)
}
/// Digest stages already charge both live staging and the complete Metal pool.
/// The CPU quotient follows the row commitment, so those idle pool allocations
/// also survive that phase even though it performs no device dispatch.
fn active_phase_payload(quotient: usize, rows: usize, coefficients: usize) -> Result<usize> {
    Ok(
        add(quotient, crate::gpu_memory::METAL_POOL_MAX_CACHED_BYTES)?
            .max(rows)
            .max(coefficients),
    )
}

fn stream_limits(limits: ConstructionLimits) -> StreamLimits {
    StreamLimits {
        digest_execution: limits.digest_execution,
        max_payload_bytes: limits.max_payload_bytes,
        max_hashes: limits.max_hash_calls,
    }
}
fn maximal_queries() -> Vec<usize> {
    // A preflight-only subset attaining all fixed tree frontier maxima together;
    // proof queries are always sampled later from the complete transcript.
    let mut indices: Vec<_> = (0..QUERY_COUNT)
        .map(|i| (i | i << 6 | i << 12 | i << 18) & (LDE_ROWS - 1))
        .collect();
    indices.sort_unstable();
    indices
}
fn limit(name: &'static str, actual: usize, max: usize) -> Result<()> {
    if actual > max {
        return Err(Error::VerifierLimitExceeded {
            limit: name,
            actual,
            max,
        });
    }
    Ok(())
}
fn binding_error(error: BindingError) -> Error {
    invalid_owned(format!("DEEP producer binding: {error}"))
}
fn invalid(details: &'static str) -> Error {
    invalid_owned(details.to_owned())
}
fn invalid_owned(details: String) -> Error {
    Error::InvalidTraceShape { details }
}

#[cfg(test)]
#[path = "deep_prover/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "deep_prover/integration_tests.rs"]
mod integration_tests;
