use core::convert::TryFrom;
use fastpq_isi::{CANONICAL_PARAMETER_SETS, StarkParameterSet, find_by_name};
use iroha_crypto::Hash;
use norito::{NoritoDeserialize, NoritoSerialize};

use crate::{
    Error, Result, TransitionBatch,
    backend::{
        self, AIR_COMPOSITION_ALPHA_COUNT, Backend, BackendArtifact, BackendConfig, ExecutionMode,
        LOOKUP_PRODUCT_DOMAIN, PoseidonExecutionMode, StarkBackend, TRANSCRIPT_TAG_AIR_ROOTS,
        TRANSCRIPT_TAG_ALPHA_PREFIX, TRANSCRIPT_TAG_GAMMA, TRANSCRIPT_TAG_INIT,
        TRANSCRIPT_TAG_ROOTS,
    },
    ordering, trace, trace_commitment,
};

/// Protocol version advertised by the Stage 2 prover implementation.
const PROTOCOL_VERSION: u16 = 1;
/// Domain tag for permission root fallback commitments.
const PERM_ROOT_DOMAIN: &[u8] = b"fastpq:v1:perm_root";
/// Domain tag for transaction set hash fallback commitments.
const TX_SET_DOMAIN: &[u8] = b"fastpq:v1:tx_set";
/// Default maximum transitions accepted by the replay verifier.
const DEFAULT_MAX_VERIFY_TRANSITIONS: usize = 256;
/// Default maximum batch payload bytes accepted by the replay verifier.
const DEFAULT_MAX_VERIFY_BATCH_BYTES: usize = 256 * 1024;
/// Default maximum FRI layers accepted by the replay verifier.
const DEFAULT_MAX_VERIFY_FRI_LAYERS: usize = 16;
/// Default maximum query openings accepted by the replay verifier.
const DEFAULT_MAX_VERIFY_QUERIES: usize = 128;
/// Default maximum LDE values carried by a single query chunk.
const DEFAULT_MAX_VERIFY_QUERY_CHUNK_VALUES: usize = 128;
/// Default maximum Merkle siblings carried by a single query opening.
const DEFAULT_MAX_VERIFY_QUERY_PATH_LEN: usize = 64;
/// Default maximum FRI values carried by a single round opening.
const DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES: usize = 16;
/// Default maximum AIR row values carried by a sampled opening.
const DEFAULT_MAX_VERIFY_AIR_ROW_VALUES: usize = 512;

/// Public inputs committed by the prover and replayed by the verifier.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, Default)]
#[repr(C)]
pub struct PublicIO {
    /// Data-space identifier (little-endian UUID).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Permission table Poseidon commitment for this slot.
    pub perm_root: [u8; 32],
    /// Transaction set hash recorded by the scheduler.
    pub tx_set_hash: [u8; 32],
    /// Deterministic ordering hash over canonicalised transitions.
    pub ordering_hash: [u8; 32],
    /// Poseidon hashes for each permission lookup row included in the batch.
    pub permission_hashes: Vec<[u8; 32]>,
}

/// Evaluation opening for the verifier queries into the FRI domain.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct QueryOpening {
    /// Domain index opened by the prover.
    pub index: u32,
    /// Evaluation value at the queried index.
    pub value: u64,
    /// Full LDE leaf chunk containing `value`.
    pub chunk_values: Vec<u64>,
    /// Merkle authentication path from the LDE leaf chunk to `lookup_root`.
    pub merkle_path: Vec<u64>,
}

/// Opened FRI fold group for one query at one round.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct FriRoundOpening {
    /// FRI round number.
    pub round: u32,
    /// Index inside the current round domain.
    pub index: u32,
    /// Full arity-sized group opened at this round.
    pub values: Vec<u64>,
    /// Folded value carried into the next round.
    pub folded_value: u64,
    /// Merkle authentication path for `values` under `fri_layers[round]`.
    pub merkle_path: Vec<u64>,
}

/// Per-query FRI opening chain across all committed rounds.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct FriQueryOpening {
    /// Initial evaluation-domain index sampled by the transcript.
    pub initial_index: u32,
    /// Round openings from the initial layer through the last fold.
    pub rounds: Vec<FriRoundOpening>,
    /// Index inside the final FRI layer.
    pub final_index: u32,
    /// Final FRI leaf values authenticated under the terminal root.
    pub final_values: Vec<u64>,
    /// Merkle authentication path for `final_values` under the terminal root.
    pub final_merkle_path: Vec<u64>,
}

/// Sampled AIR row and composition opening.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct AirConstraintOpening {
    /// Evaluation-domain index sampled by the verifier transcript.
    pub index: u32,
    /// AIR trace row values at `index`.
    pub current_row: Vec<u64>,
    /// AIR trace row values at `(index + 1) mod domain_size`.
    pub next_row: Vec<u64>,
    /// Merkle authentication path for `current_row` under `air_trace_root`.
    pub current_row_path: Vec<u64>,
    /// Merkle authentication path for `next_row` under `air_trace_root`.
    pub next_row_path: Vec<u64>,
    /// Sampled AIR composition value folded by FRI.
    pub composition_value: u64,
    /// Merkle authentication path for `composition_value` under `air_composition_root`.
    pub composition_path: Vec<u64>,
}

/// Proof artifact produced by the FASTPQ prover.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct Proof {
    /// Protocol version used to derive Fiat–Shamir challenges.
    pub protocol_version: u16,
    /// Canonical parameter catalogue version.
    pub params_version: u16,
    /// Parameter set name used by the prover.
    pub parameter: String,
    /// Deterministic commitment over the canonicalised batch.
    pub trace_commitment: Hash,
    /// Stage 2 public inputs.
    pub public_io: PublicIO,
    /// Poseidon commitment over the trace columns.
    pub trace_root: [u8; 32],
    /// Poseidon commitment over row-major AIR trace openings.
    pub air_trace_root: [u8; 32],
    /// Poseidon commitment over the AIR composition evaluation vector.
    pub air_composition_root: [u8; 32],
    /// Poseidon commitment over the lookup witness LDE leaves.
    pub lookup_root: [u8; 32],
    /// Number of evaluation rows committed by `lookup_root`.
    pub lde_domain_size: u32,
    /// Lookup grand-product accumulator evaluated by the prover.
    pub lookup_grand_product: u64,
    /// Lookup Fiat–Shamir challenge (`γ`).
    pub lookup_challenge: u64,
    /// Composition challenges sampled after `γ`.
    pub alphas: Vec<u64>,
    /// FRI folding challenges (`β_ℓ`).
    pub betas: Vec<u64>,
    /// Poseidon roots for each FRI layer (last element is the terminal root).
    pub fri_layers: Vec<[u8; 32]>,
    /// Openings into the evaluation domain sampled by the verifier.
    pub queries: Vec<QueryOpening>,
    /// AIR constraint openings for the same sampled query indices.
    pub air_openings: Vec<AirConstraintOpening>,
    /// Per-round FRI openings for the same sampled query indices.
    pub fri_queries: Vec<FriQueryOpening>,
}

impl Proof {
    /// Access the commitment hash.
    pub fn commitment(&self) -> Hash {
        self.trace_commitment
    }
}

/// Limits applied before FASTPQ V1 replay verification performs prover-scale work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyLimits {
    /// Maximum transition rows accepted in the batch supplied to the verifier.
    pub max_transitions: usize,
    /// Maximum approximate batch payload size accepted by the verifier.
    pub max_batch_bytes: usize,
    /// Maximum number of FRI layer commitments accepted in the proof.
    pub max_fri_layers: usize,
    /// Maximum number of verifier query openings accepted in the proof.
    pub max_queries: usize,
    /// Maximum number of LDE values carried by each query chunk.
    pub max_query_chunk_values: usize,
    /// Maximum Merkle authentication path length accepted for each query.
    pub max_query_path_len: usize,
    /// Maximum values opened in one FRI round group.
    pub max_fri_round_values: usize,
    /// Maximum values opened in one AIR row.
    pub max_air_row_values: usize,
}

impl Default for VerifyLimits {
    fn default() -> Self {
        Self {
            max_transitions: DEFAULT_MAX_VERIFY_TRANSITIONS,
            max_batch_bytes: DEFAULT_MAX_VERIFY_BATCH_BYTES,
            max_fri_layers: DEFAULT_MAX_VERIFY_FRI_LAYERS,
            max_queries: DEFAULT_MAX_VERIFY_QUERIES,
            max_query_chunk_values: DEFAULT_MAX_VERIFY_QUERY_CHUNK_VALUES,
            max_query_path_len: DEFAULT_MAX_VERIFY_QUERY_PATH_LEN,
            max_fri_round_values: DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES,
            max_air_row_values: DEFAULT_MAX_VERIFY_AIR_ROW_VALUES,
        }
    }
}

/// FASTPQ prover wiring canonical STARK parameters to the backend.
#[derive(Debug, Clone)]
pub struct Prover {
    params: StarkParameterSet,
    backend: StarkBackend,
}

impl Prover {
    fn new(params: StarkParameterSet) -> Self {
        Self::from_backend_config(params, BackendConfig::new(params))
    }

    fn from_backend_config(params: StarkParameterSet, config: BackendConfig) -> Self {
        Self {
            backend: StarkBackend::new(config),
            params,
        }
    }

    /// Construct a prover using a canonical parameter set.
    ///
    /// The returned prover initialises the production backend.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownParameter`] when the supplied name is not part
    /// of the canonical FASTPQ catalogue.
    pub fn canonical(parameter_name: &str) -> Result<Self> {
        let params = find_by_name(parameter_name)
            .copied()
            .ok_or_else(|| Error::UnknownParameter(parameter_name.to_string()))?;
        Ok(Self::new(params))
    }

    /// Construct a prover using a canonical parameter set and explicit execution mode.
    ///
    /// This helper mirrors [`Prover::canonical`] but forces the backend to use the
    /// specified [`ExecutionMode`], allowing operators to pin the prover to CPU or GPU
    /// execution explicitly.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownParameter`] when the supplied name is not part of
    /// the canonical FASTPQ catalogue.
    pub fn canonical_with_execution_mode(
        parameter_name: &str,
        mode: ExecutionMode,
    ) -> Result<Self> {
        Self::canonical_with_modes(parameter_name, mode, PoseidonExecutionMode::Auto)
    }

    /// Construct a prover using explicit execution and Poseidon pipeline modes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownParameter`] when the named parameter set is not
    /// part of the canonical FASTPQ catalogue.
    pub fn canonical_with_modes(
        parameter_name: &str,
        execution_mode: ExecutionMode,
        poseidon_mode: PoseidonExecutionMode,
    ) -> Result<Self> {
        let params = find_by_name(parameter_name)
            .copied()
            .ok_or_else(|| Error::UnknownParameter(parameter_name.to_string()))?;
        let config = BackendConfig::new(params)
            .with_execution_mode(execution_mode)
            .with_poseidon_mode(poseidon_mode);
        Ok(Self::from_backend_config(params, config))
    }

    /// Iterate over canonical parameter sets exposed by this crate.
    pub fn canonical_parameter_sets() -> &'static [StarkParameterSet] {
        &CANONICAL_PARAMETER_SETS
    }

    /// Produce a proof for the provided batch.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`trace_commitment`] and from the configured
    /// backend implementation.
    pub fn prove(&self, batch: &TransitionBatch) -> Result<Proof> {
        let commitment = trace_commitment(&self.params, batch)?;
        let ordering = ordering::ordering_hash(batch)?;
        let permission_hashes = collect_permission_hashes(batch)?;
        let public_io = build_public_io(batch, ordering, permission_hashes);
        let params_version = canonical_params_version(&self.params)
            .ok_or_else(|| Error::UnknownParameter(self.params.name.to_string()))?;
        let artifact = self
            .backend
            .prove(batch, &public_io, PROTOCOL_VERSION, params_version)?;
        materialise_proof(commitment, public_io, artifact, params_version)
    }
}

/// Verify a proof with default V1 replay limits.
///
/// # Errors
///
/// Returns [`Error::UnknownParameter`] when the proof references an unknown
/// parameter set or an appropriate [`Error`] variant when validation fails.
pub fn verify(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
    verify_with_limits(batch, proof, VerifyLimits::default())
}

/// Verify a proof by replaying the public transcript, authenticating sampled
/// LDE openings against the prover's Merkle root, and recomputing the canonical
/// batch commitments inside explicit replay limits.
///
/// # Errors
///
/// Returns [`Error::UnknownParameter`] when the proof references an unknown
/// parameter set, [`Error::VerifierLimitExceeded`] when inputs exceed the
/// supplied replay limits, or another [`Error`] variant when validation fails.
#[allow(clippy::too_many_lines)]
pub fn verify_with_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    if proof.protocol_version != PROTOCOL_VERSION {
        return Err(Error::UnsupportedProtocolVersion {
            version: proof.protocol_version,
        });
    }

    let params = find_by_name(&proof.parameter)
        .copied()
        .ok_or_else(|| Error::UnknownParameter(proof.parameter.clone()))?;
    let expected_version = canonical_params_version(&params)
        .ok_or_else(|| Error::UnknownParameter(proof.parameter.clone()))?;
    if proof.params_version != expected_version {
        return Err(Error::ParameterVersionMismatch {
            parameter: proof.parameter.clone(),
            expected: expected_version,
            actual: proof.params_version,
        });
    }
    enforce_verify_limits(batch, proof, limits)?;

    let expected_commitment = trace_commitment(&params, batch)?;
    if expected_commitment != proof.trace_commitment {
        return Err(Error::CommitmentMismatch);
    }

    let expected_permission_hashes = collect_permission_hashes(batch)?;
    let expected_ordering = ordering::ordering_hash(batch)?;
    let expected_public_io =
        build_public_io(batch, expected_ordering, expected_permission_hashes.clone());
    ensure_public_io_matches(&expected_public_io, &proof.public_io)?;

    let trace_root =
        field_norito::core::from_bytes(&proof.trace_root).ok_or(Error::TraceRootMismatch)?;
    let lde_root =
        field_norito::core::from_bytes(&proof.lookup_root).ok_or(Error::LookupRootMismatch)?;
    let air_trace_root =
        field_norito::core::from_bytes(&proof.air_trace_root).ok_or(Error::AirTraceRootMismatch)?;
    let air_composition_root = field_norito::core::from_bytes(&proof.air_composition_root)
        .ok_or(Error::AirCompositionRootMismatch)?;

    let mut transcript = backend::Transcript::initialise(
        &proof.public_io,
        &proof.parameter,
        proof.protocol_version,
        proof.params_version,
        TRANSCRIPT_TAG_INIT,
    )?;
    transcript.append_message(
        TRANSCRIPT_TAG_ROOTS,
        &[lde_root.to_le_bytes(), trace_root.to_le_bytes()].concat(),
    );
    let gamma = transcript.challenge_field(TRANSCRIPT_TAG_GAMMA);
    if gamma != proof.lookup_challenge {
        return Err(Error::LookupChallengeMismatch);
    }

    if proof.alphas.len() != AIR_COMPOSITION_ALPHA_COUNT {
        return Err(Error::AirChallengeCountMismatch {
            expected: AIR_COMPOSITION_ALPHA_COUNT,
            actual: proof.alphas.len(),
        });
    }
    for (idx, &alpha) in proof.alphas.iter().enumerate() {
        let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
        let expected = transcript.challenge_field(&tag);
        if expected != alpha {
            return Err(Error::FriChallengeMismatch { round: idx });
        }
    }
    transcript.append_message(
        TRANSCRIPT_TAG_AIR_ROOTS,
        &[
            air_trace_root.to_le_bytes(),
            air_composition_root.to_le_bytes(),
        ]
        .concat(),
    );
    transcript.append_message(
        LOOKUP_PRODUCT_DOMAIN,
        &proof.lookup_grand_product.to_le_bytes(),
    );

    if proof.fri_layers.is_empty() {
        return Err(Error::FriLayerLengthMismatch {
            expected: 1,
            actual: 0,
        });
    }
    let round_count = proof.fri_layers.len().saturating_sub(1);
    let max_rounds = usize::try_from(params.fri.max_reductions).expect("FRI rounds fit usize");
    if round_count > max_rounds {
        return Err(Error::FriLayerLengthMismatch {
            expected: max_rounds + 1,
            actual: proof.fri_layers.len(),
        });
    }
    let mut expected_betas = Vec::with_capacity(round_count);
    for (round, root_bytes) in proof.fri_layers.iter().take(round_count).enumerate() {
        let root =
            field_norito::core::from_bytes(root_bytes).ok_or(Error::FriLayerMismatch { round })?;
        transcript.append_fri_layer(round, root);
        expected_betas.push(transcript.challenge_beta(round));
    }
    let final_root = field_norito::core::from_bytes(
        proof
            .fri_layers
            .last()
            .expect("non-empty FRI layer commitments"),
    )
    .ok_or(Error::FriLayerMismatch { round: round_count })?;
    transcript.append_fri_final(final_root);

    if expected_betas.len() != proof.betas.len() {
        return Err(Error::FriChallengeLengthMismatch {
            expected: expected_betas.len(),
            actual: proof.betas.len(),
        });
    }
    for (round, (&expected, &actual)) in expected_betas.iter().zip(proof.betas.iter()).enumerate() {
        if expected != actual {
            return Err(Error::FriChallengeMismatch { round });
        }
    }

    let lde_domain_size = usize::try_from(proof.lde_domain_size)
        .map_err(|_| Error::QueryIndexOverflow { index: usize::MAX })?;
    if lde_domain_size == 0 {
        return Err(Error::QueryIndexOutOfRange { index: 0, len: 0 });
    }
    let expected_queries = backend::sample_queries(
        lde_domain_size,
        usize::try_from(params.fri.queries).expect("query count fits usize"),
        &mut transcript,
    );
    if expected_queries.len() != proof.queries.len() {
        return Err(Error::QueryCountMismatch {
            expected: expected_queries.len(),
            actual: proof.queries.len(),
        });
    }
    if proof.fri_queries.len() != proof.queries.len() {
        return Err(Error::QueryCountMismatch {
            expected: proof.queries.len(),
            actual: proof.fri_queries.len(),
        });
    }
    if proof.air_openings.len() != proof.queries.len() {
        return Err(Error::AirOpeningCountMismatch {
            expected: proof.queries.len(),
            actual: proof.air_openings.len(),
        });
    }
    let column_names = trace::column_names_for_batch(batch);
    for (pos, (&expected_idx, query)) in expected_queries.iter().zip(&proof.queries).enumerate() {
        let expected_index =
            u32::try_from(expected_idx).map_err(|_| Error::QueryIndexOverflow {
                index: expected_idx,
            })?;
        if expected_index != query.index {
            return Err(Error::QueryMismatch { index: pos });
        }
        let chunk_size = backend::lde_chunk_size(params.fri.arity).max(1);
        let leaf_index = expected_idx / chunk_size;
        let chunk_offset = expected_idx % chunk_size;
        if query.chunk_values.get(chunk_offset).copied() != Some(query.value) {
            return Err(Error::QueryMismatch { index: pos });
        }
        let leaf = backend::hash_lde_chunk(leaf_index, &query.chunk_values)?;
        if !backend::verify_merkle_path(lde_root, leaf, leaf_index, &query.merkle_path)? {
            return Err(Error::QueryMerklePathMismatch { index: pos });
        }
        let air_opening = &proof.air_openings[pos];
        if usize::try_from(air_opening.index).ok() != Some(expected_idx)
            || air_opening.current_row.len() != column_names.len()
            || air_opening.next_row.len() != column_names.len()
        {
            return Err(Error::AirOpeningMismatch { index: pos });
        }
        let current_leaf = backend::hash_air_trace_row(expected_idx, &air_opening.current_row)?;
        if !backend::verify_merkle_path(
            air_trace_root,
            current_leaf,
            expected_idx,
            &air_opening.current_row_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        let next_idx = (expected_idx + 1) % lde_domain_size;
        let next_leaf = backend::hash_air_trace_row(next_idx, &air_opening.next_row)?;
        if !backend::verify_merkle_path(
            air_trace_root,
            next_leaf,
            next_idx,
            &air_opening.next_row_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        let expected_composition = backend::air_composition_value_for_rows(
            &column_names,
            &air_opening.current_row,
            &air_opening.next_row,
            &proof.alphas,
        )?;
        if expected_composition != air_opening.composition_value {
            return Err(Error::AirConstraintMismatch { index: pos });
        }
        let composition_leaf =
            backend::hash_air_composition_leaf(expected_idx, air_opening.composition_value)?;
        if !backend::verify_merkle_path(
            air_composition_root,
            composition_leaf,
            expected_idx,
            &air_opening.composition_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        verify_fri_query_chain(
            pos,
            expected_idx,
            air_opening.composition_value,
            &proof.fri_queries[pos],
            &proof.fri_layers,
            &proof.betas,
            params.fri.arity,
        )?;
    }

    Ok(())
}

fn enforce_verify_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    let batch_bytes = batch_size_hint(batch);
    if batch_bytes > limits.max_batch_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_batch_bytes",
            actual: batch_bytes,
            max: limits.max_batch_bytes,
        });
    }
    if proof.fri_layers.len() > limits.max_fri_layers {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_fri_layers",
            actual: proof.fri_layers.len(),
            max: limits.max_fri_layers,
        });
    }
    if proof.queries.len() > limits.max_queries {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_queries",
            actual: proof.queries.len(),
            max: limits.max_queries,
        });
    }
    for query in &proof.queries {
        if query.chunk_values.len() > limits.max_query_chunk_values {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_query_chunk_values",
                actual: query.chunk_values.len(),
                max: limits.max_query_chunk_values,
            });
        }
        if query.merkle_path.len() > limits.max_query_path_len {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual: query.merkle_path.len(),
                max: limits.max_query_path_len,
            });
        }
    }
    if proof.fri_queries.len() > limits.max_queries {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_queries",
            actual: proof.fri_queries.len(),
            max: limits.max_queries,
        });
    }
    if proof.air_openings.len() > limits.max_queries {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_queries",
            actual: proof.air_openings.len(),
            max: limits.max_queries,
        });
    }
    for air_opening in &proof.air_openings {
        for row_len in [air_opening.current_row.len(), air_opening.next_row.len()] {
            if row_len > limits.max_air_row_values {
                return Err(Error::VerifierLimitExceeded {
                    limit: "max_air_row_values",
                    actual: row_len,
                    max: limits.max_air_row_values,
                });
            }
        }
        for path_len in [
            air_opening.current_row_path.len(),
            air_opening.next_row_path.len(),
            air_opening.composition_path.len(),
        ] {
            if path_len > limits.max_query_path_len {
                return Err(Error::VerifierLimitExceeded {
                    limit: "max_query_path_len",
                    actual: path_len,
                    max: limits.max_query_path_len,
                });
            }
        }
    }
    for fri_query in &proof.fri_queries {
        if fri_query.final_values.len() > limits.max_fri_round_values {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_fri_round_values",
                actual: fri_query.final_values.len(),
                max: limits.max_fri_round_values,
            });
        }
        if fri_query.final_merkle_path.len() > limits.max_query_path_len {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual: fri_query.final_merkle_path.len(),
                max: limits.max_query_path_len,
            });
        }
        for round in &fri_query.rounds {
            if round.values.len() > limits.max_fri_round_values {
                return Err(Error::VerifierLimitExceeded {
                    limit: "max_fri_round_values",
                    actual: round.values.len(),
                    max: limits.max_fri_round_values,
                });
            }
            if round.merkle_path.len() > limits.max_query_path_len {
                return Err(Error::VerifierLimitExceeded {
                    limit: "max_query_path_len",
                    actual: round.merkle_path.len(),
                    max: limits.max_query_path_len,
                });
            }
        }
    }
    Ok(())
}

fn verify_fri_query_chain(
    query_pos: usize,
    initial_index: usize,
    initial_value: u64,
    fri_query: &FriQueryOpening,
    fri_layers: &[[u8; 32]],
    betas: &[u64],
    arity: u32,
) -> Result<()> {
    let arity = usize::try_from(arity).map_err(|_| Error::FriArity(arity))?;
    if arity == 0 {
        return Err(Error::FriArity(0));
    }
    if usize::try_from(fri_query.initial_index).ok() != Some(initial_index) {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    if fri_query.rounds.len() != betas.len() {
        return Err(Error::FriChallengeLengthMismatch {
            expected: betas.len(),
            actual: fri_query.rounds.len(),
        });
    }
    if fri_layers.len() != betas.len() + 1 {
        return Err(Error::FriLayerLengthMismatch {
            expected: betas.len() + 1,
            actual: fri_layers.len(),
        });
    }

    let mut index = initial_index;
    let mut value = initial_value;
    for (round, opening) in fri_query.rounds.iter().enumerate() {
        if usize::try_from(opening.round).ok() != Some(round)
            || usize::try_from(opening.index).ok() != Some(index)
            || opening.values.is_empty()
        {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let leaf_index = index / arity;
        let offset = index % arity;
        if opening.values.get(offset).copied() != Some(value) {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let root = field_norito::core::from_bytes(&fri_layers[round])
            .ok_or(Error::FriLayerMismatch { round })?;
        let leaf = backend::hash_fri_chunk(round, leaf_index, &opening.values)?;
        if !backend::verify_merkle_path(root, leaf, leaf_index, &opening.merkle_path)? {
            return Err(Error::QueryMerklePathMismatch { index: query_pos });
        }
        let folded = fold_fri_values(&opening.values, betas[round]);
        if folded != opening.folded_value {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        index = leaf_index;
        value = folded;
    }

    if usize::try_from(fri_query.final_index).ok() != Some(index)
        || fri_query.final_values.is_empty()
    {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let final_leaf_index = index / arity;
    let final_offset = index % arity;
    if fri_query.final_values.get(final_offset).copied() != Some(value) {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let final_round = betas.len();
    let final_root = field_norito::core::from_bytes(&fri_layers[final_round])
        .ok_or(Error::FriLayerMismatch { round: final_round })?;
    let final_leaf =
        backend::hash_fri_chunk(final_round, final_leaf_index, &fri_query.final_values)?;
    if !backend::verify_merkle_path(
        final_root,
        final_leaf,
        final_leaf_index,
        &fri_query.final_merkle_path,
    )? {
        return Err(Error::QueryMerklePathMismatch { index: query_pos });
    }
    Ok(())
}

const GOLDILOCKS_MODULUS: u64 = 0xffff_ffff_0000_0001;

fn fold_fri_values(values: &[u64], challenge: u64) -> u64 {
    let mut acc = 0u64;
    let mut power = 1u64;
    for &value in values {
        acc = add_mod(acc, mul_mod(value, power));
        power = mul_mod(power, challenge);
    }
    acc
}

fn add_mod(a: u64, b: u64) -> u64 {
    let sum = u128::from(a) + u128::from(b);
    u64::try_from(sum % u128::from(GOLDILOCKS_MODULUS)).expect("modulus reduction fits in u64")
}

fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    u64::try_from(product % u128::from(GOLDILOCKS_MODULUS)).expect("modulus reduction fits in u64")
}

fn batch_size_hint(batch: &TransitionBatch) -> usize {
    let mut total = batch.parameter.len();
    for transition in &batch.transitions {
        total = total
            .saturating_add(transition.key.len())
            .saturating_add(transition.pre_value.len())
            .saturating_add(transition.post_value.len())
            .saturating_add(operation_size_hint(&transition.operation));
    }
    for (key, value) in &batch.metadata {
        total = total.saturating_add(key.len()).saturating_add(value.len());
    }
    total
}

fn operation_size_hint(operation: &crate::OperationKind) -> usize {
    match operation {
        crate::OperationKind::RoleGrant {
            role_id,
            permission_id,
            ..
        }
        | crate::OperationKind::RoleRevoke {
            role_id,
            permission_id,
            ..
        } => role_id.len().saturating_add(permission_id.len()),
        crate::OperationKind::Transfer
        | crate::OperationKind::Mint
        | crate::OperationKind::Burn
        | crate::OperationKind::MetaSet => 0,
    }
}

fn materialise_proof(
    commitment: Hash,
    public_io: PublicIO,
    artifact: BackendArtifact,
    params_version: u16,
) -> Result<Proof> {
    if artifact.query_openings.len() != artifact.query_chunks.len() {
        return Err(Error::QueryCountMismatch {
            expected: artifact.query_openings.len(),
            actual: artifact.query_chunks.len(),
        });
    }
    if artifact.query_openings.len() != artifact.query_paths.len() {
        return Err(Error::QueryCountMismatch {
            expected: artifact.query_openings.len(),
            actual: artifact.query_paths.len(),
        });
    }
    if artifact.query_openings.len() != artifact.fri_query_openings.len() {
        return Err(Error::QueryCountMismatch {
            expected: artifact.query_openings.len(),
            actual: artifact.fri_query_openings.len(),
        });
    }
    if artifact.query_openings.len() != artifact.air_openings.len() {
        return Err(Error::AirOpeningCountMismatch {
            expected: artifact.query_openings.len(),
            actual: artifact.air_openings.len(),
        });
    }
    let fri_layers = artifact
        .fri_layers
        .into_iter()
        .map(field_norito::core::to_bytes)
        .collect();
    let queries = artifact
        .query_openings
        .into_iter()
        .zip(artifact.query_chunks)
        .zip(artifact.query_paths)
        .map(
            |(((index, value), chunk_values), merkle_path)| QueryOpening {
                index,
                value,
                chunk_values,
                merkle_path,
            },
        )
        .collect();
    Ok(Proof {
        protocol_version: PROTOCOL_VERSION,
        params_version,
        parameter: artifact.parameter,
        trace_commitment: commitment,
        public_io,
        trace_root: field_norito::core::to_bytes(artifact.trace_root),
        air_trace_root: field_norito::core::to_bytes(artifact.air_trace_root),
        air_composition_root: field_norito::core::to_bytes(artifact.air_composition_root),
        lookup_root: field_norito::core::to_bytes(artifact.lookup_root),
        lde_domain_size: artifact.lde_domain_size,
        lookup_grand_product: artifact.lookup_grand_product,
        lookup_challenge: artifact.lookup_challenge,
        alphas: artifact.alphas,
        betas: artifact.fri_betas,
        fri_layers,
        queries,
        air_openings: artifact.air_openings,
        fri_queries: artifact.fri_query_openings,
    })
}

fn build_public_io(
    batch: &TransitionBatch,
    ordering_hash: Hash,
    permission_hashes: Vec<[u8; 32]>,
) -> PublicIO {
    let inputs = &batch.public_inputs;
    let perm_root = if is_zero_bytes(&inputs.perm_root) {
        perm_root_from_permission_hashes(&permission_hashes)
    } else {
        inputs.perm_root
    };
    let tx_set_hash = if is_zero_bytes(&inputs.tx_set_hash) {
        tx_set_hash_from_ordering(&ordering_hash)
    } else {
        inputs.tx_set_hash
    };
    PublicIO {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root,
        tx_set_hash,
        ordering_hash: hash_norito::core::to_bytes(&ordering_hash),
        permission_hashes,
    }
}

fn ensure_public_io_matches(expected: &PublicIO, actual: &PublicIO) -> Result<()> {
    if actual.dsid != expected.dsid {
        return Err(Error::PublicIoMismatch { field: "dsid" });
    }
    if actual.slot != expected.slot {
        return Err(Error::PublicIoMismatch { field: "slot" });
    }
    if actual.old_root != expected.old_root {
        return Err(Error::PublicIoMismatch { field: "old_root" });
    }
    if actual.new_root != expected.new_root {
        return Err(Error::PublicIoMismatch { field: "new_root" });
    }
    if actual.perm_root != expected.perm_root {
        return Err(Error::PublicIoMismatch { field: "perm_root" });
    }
    if actual.tx_set_hash != expected.tx_set_hash {
        return Err(Error::PublicIoMismatch {
            field: "tx_set_hash",
        });
    }
    if actual.ordering_hash != expected.ordering_hash {
        return Err(Error::OrderingHashMismatch);
    }
    if actual.permission_hashes != expected.permission_hashes {
        return Err(Error::PermissionHashMismatch);
    }
    Ok(())
}

fn is_zero_bytes(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| b == 0)
}

fn perm_root_from_permission_hashes(hashes: &[[u8; 32]]) -> [u8; 32] {
    if hashes.is_empty() {
        return [0u8; 32];
    }
    let mut payload = Vec::with_capacity(PERM_ROOT_DOMAIN.len() + hashes.len() * 32);
    payload.extend_from_slice(PERM_ROOT_DOMAIN);
    for hash in hashes {
        payload.extend_from_slice(hash);
    }
    hash_norito::core::to_bytes(&Hash::new(payload))
}

fn tx_set_hash_from_ordering(ordering_hash: &Hash) -> [u8; 32] {
    let mut payload = Vec::with_capacity(TX_SET_DOMAIN.len() + Hash::LENGTH);
    payload.extend_from_slice(TX_SET_DOMAIN);
    payload.extend_from_slice(ordering_hash.as_ref());
    hash_norito::core::to_bytes(&Hash::new(payload))
}

fn collect_permission_hashes(batch: &TransitionBatch) -> Result<Vec<[u8; 32]>> {
    let mut canonical = batch.clone();
    canonical.sort();
    let mut hashes = Vec::new();
    for transition in &canonical.transitions {
        match &transition.operation {
            crate::OperationKind::RoleGrant {
                role_id,
                permission_id,
                epoch,
            }
            | crate::OperationKind::RoleRevoke {
                role_id,
                permission_id,
                epoch,
            } => {
                let digest = trace::permission_hash(role_id, permission_id, *epoch)?;
                hashes.push(field_norito::core::to_bytes(digest));
            }
            _ => {}
        }
    }
    Ok(hashes)
}

fn canonical_params_version(params: &StarkParameterSet) -> Option<u16> {
    CANONICAL_PARAMETER_SETS
        .iter()
        .position(|candidate| candidate.name == params.name)
        .and_then(|idx| u16::try_from(idx + 1).ok())
}

mod hash_norito {
    pub mod core {
        use iroha_crypto::Hash;

        pub fn to_bytes(hash: &Hash) -> [u8; 32] {
            let mut out = [0u8; 32];
            out.copy_from_slice(hash.as_ref());
            out
        }
    }
}

mod field_norito {
    pub mod core {
        pub fn to_bytes(value: u64) -> [u8; 32] {
            let mut out = [0u8; 32];
            out[..8].copy_from_slice(&value.to_le_bytes());
            out
        }

        pub fn from_bytes(bytes: &[u8; 32]) -> Option<u64> {
            if bytes[8..].iter().any(|byte| *byte != 0) {
                return None;
            }
            Some(u64::from_le_bytes(
                bytes[..8].try_into().expect("slice length is 8"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, PublicInputs, StateTransition};

    fn annotate_batch(batch: &mut TransitionBatch) {
        batch.public_inputs.dsid = [0x11; 16];
        batch.public_inputs.slot = 42;
        batch.public_inputs.old_root = [0xAA; 32];
        batch.public_inputs.new_root = [0xBB; 32];
        batch.public_inputs.perm_root = [0xCC; 32];
        batch.public_inputs.tx_set_hash = [0xDD; 32];
    }

    fn sample_batch() -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.push(StateTransition::new(
            b"account/alice".to_vec(),
            vec![1],
            vec![2],
            OperationKind::Mint,
        ));
        batch.push(StateTransition::new(
            b"asset/xor".to_vec(),
            vec![10, 0, 0, 0],
            vec![11, 0, 0, 0],
            OperationKind::Mint,
        ));
        batch.sort();
        annotate_batch(&mut batch);
        batch
    }

    fn sample_batch_with_size(rows: usize) -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        for idx in 0..rows {
            let key = format!("asset/xor/account/{idx:04}").into_bytes();
            let idx_u64 = u64::try_from(idx).expect("sample row index fits u64");
            let pre = idx_u64.to_le_bytes().to_vec();
            let post = idx_u64.wrapping_add(1).to_le_bytes().to_vec();
            let op = match idx % 3 {
                0 => OperationKind::Mint,
                1 => OperationKind::Burn,
                _ => OperationKind::MetaSet,
            };
            batch.push(StateTransition::new(key, pre, post, op));
        }
        batch.sort();
        annotate_batch(&mut batch);
        batch
    }

    fn sample_batch_with_permission() -> TransitionBatch {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.push(StateTransition::new(
            b"role/sora-admin/permission/transfer".to_vec(),
            0u64.to_le_bytes().to_vec(),
            1u64.to_le_bytes().to_vec(),
            OperationKind::RoleGrant {
                role_id: vec![0xAB; 32],
                permission_id: vec![0xCD; 32],
                epoch: 7,
            },
        ));
        batch.sort();
        annotate_batch(&mut batch);
        batch
    }

    #[test]
    fn public_io_falls_back_to_derived_inputs() {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.public_inputs.dsid = [0x11; 16];
        batch.public_inputs.slot = 7;
        batch.public_inputs.old_root = [0xAA; 32];
        batch.public_inputs.new_root = [0xBB; 32];
        batch.push(StateTransition::new(
            b"role/sora-admin/permission/transfer".to_vec(),
            0u64.to_le_bytes().to_vec(),
            1u64.to_le_bytes().to_vec(),
            OperationKind::RoleGrant {
                role_id: vec![0xAB; 32],
                permission_id: vec![0xCD; 32],
                epoch: 7,
            },
        ));
        batch.sort();
        let ordering = ordering::ordering_hash(&batch).expect("ordering");
        let permission_hashes = collect_permission_hashes(&batch).expect("permission hashes");
        let public_io = build_public_io(&batch, ordering, permission_hashes.clone());
        assert_eq!(
            public_io.perm_root,
            perm_root_from_permission_hashes(&permission_hashes)
        );
        assert_eq!(public_io.tx_set_hash, tx_set_hash_from_ordering(&ordering));
    }

    #[test]
    fn prove_and_verify_roundtrip() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let proof = prover.prove(&batch).unwrap();
        verify(&batch, &proof).unwrap();
    }

    #[test]
    fn canonical_prover_rejects_unknown_parameter() {
        let err = Prover::canonical("does-not-exist").unwrap_err();
        assert!(matches!(err, Error::UnknownParameter(_)));
    }

    #[test]
    fn canonical_with_execution_mode_overrides_backend() {
        let prover =
            Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
                .expect("prover");
        assert_eq!(prover.backend.execution_mode(), ExecutionMode::Cpu);
    }

    #[test]
    fn verify_rejects_modified_roots() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.trace_root[0] ^= 0xAA;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::TraceRootMismatch | Error::LookupChallengeMismatch
        ));
    }

    #[test]
    fn verify_rejects_tampered_commitment() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.trace_commitment = Hash::new(b"tampered-fastpq-commitment");
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::CommitmentMismatch));
    }

    #[test]
    fn verify_rejects_ordering_hash_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.ordering_hash[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::OrderingHashMismatch));
    }

    #[test]
    fn verify_rejects_dsid_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.dsid[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "dsid" }));
    }

    #[test]
    fn verify_rejects_slot_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.slot = proof.public_io.slot.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "slot" }));
    }

    #[test]
    fn verify_rejects_old_root_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.old_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "old_root" }));
    }

    #[test]
    fn verify_rejects_new_root_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.new_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "new_root" }));
    }

    #[test]
    fn verify_rejects_perm_root_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.perm_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::PublicIoMismatch { field: "perm_root" }
        ));
    }

    #[test]
    fn verify_rejects_tx_set_hash_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.tx_set_hash[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::PublicIoMismatch {
                field: "tx_set_hash"
            }
        ));
    }

    #[test]
    fn verify_rejects_permission_hash_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_permission();
        let mut proof = prover.prove(&batch).unwrap();
        assert!(
            !proof.public_io.permission_hashes.is_empty(),
            "expected at least one permission hash for mutation test"
        );
        proof.public_io.permission_hashes[0][0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PermissionHashMismatch));
    }

    #[test]
    fn verify_rejects_wrong_betas() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        if let Some(beta) = proof.betas.first_mut() {
            *beta = beta.wrapping_add(1);
        }
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeMismatch { round: 0 }));
    }

    #[test]
    fn verify_rejects_wrong_lookup_challenge() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(8);
        let mut proof = prover.prove(&batch).unwrap();
        proof.lookup_challenge = proof.lookup_challenge.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::LookupChallengeMismatch));
    }

    #[test]
    fn verify_rejects_wrong_lookup_grand_product() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(8);
        let mut proof = prover.prove(&batch).unwrap();
        proof.lookup_grand_product = proof.lookup_grand_product.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeMismatch { .. }));
    }

    #[test]
    fn verify_rejects_modified_lookup_root() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(16);
        let mut proof = prover.prove(&batch).unwrap();
        proof.lookup_root[0] ^= 0xAA;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::LookupRootMismatch | Error::LookupChallengeMismatch
        ));
    }

    #[test]
    fn verify_rejects_fri_layer_length_mismatch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        assert!(
            proof.fri_layers.len() > 1,
            "expected at least one FRI layer plus terminal root"
        );
        proof.fri_layers.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::FriLayerLengthMismatch { .. } | Error::FriChallengeLengthMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_fri_layer_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        assert!(
            !proof.fri_layers.is_empty(),
            "expected non-empty FRI layer list"
        );
        proof.fri_layers[0][0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::FriLayerMismatch { round: 0 } | Error::FriChallengeMismatch { round: 0 }
        ));
    }

    #[test]
    fn verify_rejects_fri_challenge_length_mismatch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        assert!(!proof.betas.is_empty(), "expected at least one FRI beta");
        proof.betas.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeLengthMismatch { .. }));
    }

    #[test]
    fn verify_rejects_query_count_mismatch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        assert!(!proof.queries.is_empty(), "expected queries in proof");
        proof.queries.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryCountMismatch { .. }));
    }

    #[test]
    fn verify_rejects_wrong_query_value() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        first.value = first.value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { .. }));
    }

    #[test]
    fn verify_rejects_wrong_query_chunk_value() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        let chunk_value = first
            .chunk_values
            .first_mut()
            .expect("expected query chunk values");
        *chunk_value = chunk_value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryMismatch { .. } | Error::QueryMerklePathMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_wrong_query_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        let sibling = first
            .merkle_path
            .first_mut()
            .expect("expected query Merkle path");
        *sibling = sibling.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { .. }));
    }

    #[test]
    fn verify_rejects_wrong_air_composition_root() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.air_composition_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirCompositionRootMismatch | Error::FriChallengeMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_missing_air_challenges() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.alphas.clear();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirChallengeCountMismatch {
                expected: 2,
                actual: 0
            }
        ));
    }

    #[test]
    fn verify_rejects_wrong_air_row_opening() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected at least one AIR opening");
        let value = first
            .current_row
            .first_mut()
            .expect("expected sampled AIR row values");
        *value = value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirMerklePathMismatch { .. } | Error::AirConstraintMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_wrong_air_composition_opening() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected at least one AIR opening");
        first.composition_value = first.composition_value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirConstraintMismatch { .. } | Error::AirMerklePathMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_mismatched_large_batch_by_commitment() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let small_batch = sample_batch();
        let proof = prover.prove(&small_batch).unwrap();
        let large_batch = sample_batch_with_size(DEFAULT_MAX_VERIFY_TRANSITIONS + 1);
        let err = verify(&large_batch, &proof).unwrap_err();
        assert!(matches!(err, Error::CommitmentMismatch));
    }

    #[test]
    fn verify_allows_large_batch_without_replay_transition_window() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(DEFAULT_MAX_VERIFY_TRANSITIONS + 1);
        let proof = prover.prove(&batch).unwrap();
        verify(&batch, &proof).unwrap();
    }

    #[test]
    fn verify_rejects_stale_version() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.params_version = proof.params_version.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::ParameterVersionMismatch { .. }));
    }

    #[test]
    fn proof_roundtrip_smoke() {
        let proof = Proof {
            protocol_version: PROTOCOL_VERSION,
            params_version: 1,
            parameter: "fastpq-lane-balanced".to_string(),
            trace_commitment: Hash::new([0u8; 4]),
            public_io: PublicIO {
                dsid: [0; 16],
                slot: 42,
                old_root: [1; 32],
                new_root: [2; 32],
                perm_root: [3; 32],
                tx_set_hash: [4; 32],
                ordering_hash: [5; 32],
                permission_hashes: vec![[6; 32]],
            },
            trace_root: [7; 32],
            air_trace_root: [8; 32],
            air_composition_root: [9; 32],
            lookup_root: [10; 32],
            lde_domain_size: 1,
            lookup_grand_product: 11,
            lookup_challenge: 12,
            alphas: vec![13, 14],
            betas: vec![15, 16],
            fri_layers: vec![[17; 32], [18; 32]],
            queries: vec![QueryOpening {
                index: 0,
                value: 123,
                chunk_values: vec![123],
                merkle_path: Vec::new(),
            }],
            air_openings: vec![AirConstraintOpening {
                index: 0,
                current_row: vec![1, 2],
                next_row: vec![3, 4],
                current_row_path: Vec::new(),
                next_row_path: Vec::new(),
                composition_value: 456,
                composition_path: Vec::new(),
            }],
            fri_queries: vec![FriQueryOpening {
                initial_index: 0,
                rounds: vec![FriRoundOpening {
                    round: 0,
                    index: 0,
                    values: vec![456],
                    folded_value: 456,
                    merkle_path: Vec::new(),
                }],
                final_index: 0,
                final_values: vec![456],
                final_merkle_path: Vec::new(),
            }],
        };
        let first = norito::core::to_bytes(&proof).expect("encode proof");
        let second = norito::core::to_bytes(&proof).expect("re-encode proof deterministically");
        assert_eq!(first, second);
    }

    #[test]
    fn hash_norito_to_bytes_matches_hash_bytes() {
        let raw = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x01, 0x23, 0x45, 0x67, 0x89, 0x10, 0x32, 0x54, 0x76, 0x98,
            0xBA, 0xDC, 0xFE, 0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01, 0xFF, 0xEE, 0xDD,
            0xCC, 0xBB, 0xAA, 0x99,
        ];
        let hash = Hash::prehashed(raw);
        assert_eq!(hash_norito::core::to_bytes(&hash), raw);
    }

    #[test]
    fn field_norito_to_bytes_encodes_le() {
        let value = 0x0123_4567_89AB_CDEFu64;
        let encoded = field_norito::core::to_bytes(value);
        assert_eq!(encoded[..8], value.to_le_bytes());
        assert!(encoded[8..].iter().all(|byte| *byte == 0));
    }
}
