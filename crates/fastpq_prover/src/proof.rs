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

/// Protocol version advertised by the V1 prover implementation.
const PROTOCOL_VERSION: u16 = 1;
/// Domain tag for permission root fallback commitments.
const PERM_ROOT_DOMAIN: &[u8] = b"fastpq:v1:perm_root";
/// Domain tag for transaction set hash fallback commitments.
const TX_SET_DOMAIN: &[u8] = b"fastpq:v1:tx_set";
/// Default maximum transitions accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_TRANSITIONS: usize = 256;
/// Default maximum batch payload bytes accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_BATCH_BYTES: usize = 256 * 1024;
/// Default maximum approximate proof payload bytes accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_PROOF_BYTES: usize = 512 * 1024;
/// Default maximum FRI layers accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_FRI_LAYERS: usize = 16;
/// Default maximum query openings accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_QUERIES: usize = 128;
/// Default maximum LDE values carried by a single query chunk.
const DEFAULT_MAX_VERIFY_QUERY_CHUNK_VALUES: usize = 128;
/// Default maximum Merkle siblings carried by a single query opening.
const DEFAULT_MAX_VERIFY_QUERY_PATH_LEN: usize = 64;
/// Default maximum FRI values carried by a single round opening.
const DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES: usize = 16;
/// Default maximum AIR row values carried by a sampled opening.
const DEFAULT_MAX_VERIFY_AIR_ROW_VALUES: usize = 512;

/// Public inputs committed by the prover and checked by the verifier.
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
    /// V1 public inputs.
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

/// Limits applied before FASTPQ V1 proof verification consumes proof-carried openings.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyLimits {
    /// Maximum transition rows accepted in the batch supplied to the verifier.
    pub max_transitions: usize,
    /// Maximum approximate batch payload size accepted by the verifier.
    pub max_batch_bytes: usize,
    /// Maximum approximate proof payload size accepted by the verifier.
    pub max_proof_bytes: usize,
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
            max_proof_bytes: DEFAULT_MAX_VERIFY_PROOF_BYTES,
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
    /// backend implementation. The generated proof is verified against the
    /// canonical CPU verifier path before being returned.
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
        let proof = materialise_proof(commitment, public_io, artifact, params_version)?;
        verify(batch, &proof)?;
        Ok(proof)
    }
}

/// Verify a V1 proof with default proof-size and transcript limits.
///
/// # Errors
///
/// Returns [`Error::UnknownParameter`] when the proof references an unknown
/// parameter set, or an appropriate [`Error`] variant identifying the invalid
/// proof component.
pub fn verify(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
    verify_with_limits(batch, proof, VerifyLimits::default())
}

/// Verify a V1 proof from proof contents, public inputs, commitments, Merkle paths,
/// lookup product binding, AIR openings, FRI query chains, challenges, and
/// parameter/version checks.
///
/// # Errors
///
/// Returns [`Error::UnknownParameter`] when the proof references an unknown
/// parameter set, [`Error::VerifierLimitExceeded`] when inputs exceed the
/// supplied limits, or another [`Error`] variant identifying the invalid proof
/// component.
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
    if batch.parameter != proof.parameter {
        return Err(Error::ParameterMismatch {
            expected: proof.parameter.clone(),
            actual: batch.parameter.clone(),
        });
    }
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
    let expected_artifact = expected_backend_artifact(
        batch,
        &expected_public_io,
        params,
        proof.protocol_version,
        proof.params_version,
    )?;
    ensure_artifact_matches_proof(&expected_artifact, proof)?;

    let trace_root =
        field_norito::core::from_bytes(&proof.trace_root).ok_or(Error::TraceRootMismatch)?;
    let lde_root =
        field_norito::core::from_bytes(&proof.lookup_root).ok_or(Error::LookupRootMismatch)?;
    let air_trace_root =
        field_norito::core::from_bytes(&proof.air_trace_root).ok_or(Error::AirTraceRootMismatch)?;
    let air_composition_root = field_norito::core::from_bytes(&proof.air_composition_root)
        .ok_or(Error::AirCompositionRootMismatch)?;
    let lde_domain_size = usize::try_from(proof.lde_domain_size)
        .map_err(|_| Error::QueryIndexOverflow { index: usize::MAX })?;
    if lde_domain_size == 0 {
        return Err(Error::QueryIndexOutOfRange { index: 0, len: 0 });
    }

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

    let fri_layer_lengths =
        expected_fri_layer_lengths(lde_domain_size, params.fri.arity, params.fri.max_reductions)?;
    if proof.fri_layers.len() != fri_layer_lengths.len() {
        return Err(Error::FriLayerLengthMismatch {
            expected: fri_layer_lengths.len(),
            actual: proof.fri_layers.len(),
        });
    }
    let round_count = proof.fri_layers.len().saturating_sub(1);
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

    let next_step = usize::try_from(params.fri.blowup_factor)
        .expect("FRI blowup factor fits usize")
        .max(1);
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
    let lde_chunk_size = backend::lde_chunk_size(params.fri.arity).max(1);
    let lde_leaf_count = leaf_count_for_values(lde_domain_size, lde_chunk_size)?;
    let lde_path_len = merkle_path_len_for_leaf_count(lde_leaf_count)?;
    let air_path_len = merkle_path_len_for_leaf_count(lde_domain_size)?;
    for (pos, (&expected_idx, query)) in expected_queries.iter().zip(&proof.queries).enumerate() {
        let expected_index =
            u32::try_from(expected_idx).map_err(|_| Error::QueryIndexOverflow {
                index: expected_idx,
            })?;
        if expected_index != query.index {
            return Err(Error::QueryMismatch { index: pos });
        }
        let leaf_index = expected_idx / lde_chunk_size;
        let chunk_offset = expected_idx % lde_chunk_size;
        let expected_chunk_len =
            expected_leaf_value_len(lde_domain_size, lde_chunk_size, leaf_index)?;
        if query.chunk_values.len() != expected_chunk_len {
            return Err(Error::QueryMismatch { index: pos });
        }
        if query.chunk_values.get(chunk_offset).copied() != Some(query.value) {
            return Err(Error::QueryMismatch { index: pos });
        }
        if expected_artifact.query_openings.get(pos).copied() != Some((query.index, query.value)) {
            return Err(Error::QueryMismatch { index: pos });
        }
        if query.merkle_path.len() != lde_path_len {
            return Err(Error::QueryMerklePathMismatch { index: pos });
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
        if air_opening.current_row_path.len() != air_path_len
            || air_opening.next_row_path.len() != air_path_len
            || air_opening.composition_path.len() != air_path_len
        {
            return Err(Error::AirMerklePathMismatch { index: pos });
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
        let next_idx = expected_idx
            .checked_add(next_step)
            .ok_or(Error::QueryIndexOverflow {
                index: expected_idx,
            })?
            % lde_domain_size;
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
            &proof.fri_queries[pos],
            FriQueryVerification {
                query_pos: pos,
                initial_index: expected_idx,
                initial_value: air_opening.composition_value,
                fri_layers: &proof.fri_layers,
                betas: &proof.betas,
                fri_layer_lengths: &fri_layer_lengths,
                arity: params.fri.arity,
            },
        )?;
    }
    Ok(())
}

fn expected_backend_artifact(
    batch: &TransitionBatch,
    public_io: &PublicIO,
    params: StarkParameterSet,
    protocol_version: u16,
    params_version: u16,
) -> Result<BackendArtifact> {
    let config = BackendConfig::new(params)
        .with_execution_mode(ExecutionMode::Cpu)
        .with_poseidon_mode(PoseidonExecutionMode::Cpu);
    StarkBackend::new(config).prove(batch, public_io, protocol_version, params_version)
}

fn ensure_artifact_matches_proof(artifact: &BackendArtifact, proof: &Proof) -> Result<()> {
    if proof.trace_root != field_norito::core::to_bytes(artifact.trace_root) {
        return Err(Error::TraceRootMismatch);
    }
    if proof.lookup_root != field_norito::core::to_bytes(artifact.lookup_root) {
        return Err(Error::LookupRootMismatch);
    }
    if proof.air_trace_root != field_norito::core::to_bytes(artifact.air_trace_root) {
        return Err(Error::AirTraceRootMismatch);
    }
    if proof.air_composition_root != field_norito::core::to_bytes(artifact.air_composition_root) {
        return Err(Error::AirCompositionRootMismatch);
    }
    if proof.lde_domain_size != artifact.lde_domain_size {
        return Err(Error::QueryCountMismatch {
            expected: usize::try_from(artifact.lde_domain_size).unwrap_or(usize::MAX),
            actual: usize::try_from(proof.lde_domain_size).unwrap_or(usize::MAX),
        });
    }
    if proof.lookup_grand_product != artifact.lookup_grand_product {
        return Err(Error::LookupGrandProductMismatch);
    }
    if proof.lookup_challenge != artifact.lookup_challenge {
        return Err(Error::LookupChallengeMismatch);
    }
    if proof.alphas.len() != artifact.alphas.len() {
        return Err(Error::AirChallengeCountMismatch {
            expected: artifact.alphas.len(),
            actual: proof.alphas.len(),
        });
    }
    for (idx, (expected, actual)) in artifact.alphas.iter().zip(&proof.alphas).enumerate() {
        if expected != actual {
            return Err(Error::FriChallengeMismatch { round: idx });
        }
    }
    let expected_layers = artifact
        .fri_layers
        .iter()
        .copied()
        .map(field_norito::core::to_bytes)
        .collect::<Vec<_>>();
    if proof.fri_layers.len() != expected_layers.len() {
        return Err(Error::FriLayerLengthMismatch {
            expected: expected_layers.len(),
            actual: proof.fri_layers.len(),
        });
    }
    for (round, (expected, actual)) in expected_layers.iter().zip(&proof.fri_layers).enumerate() {
        if expected != actual {
            return Err(Error::FriLayerMismatch { round });
        }
    }
    if proof.betas.len() != artifact.fri_betas.len() {
        return Err(Error::FriChallengeLengthMismatch {
            expected: artifact.fri_betas.len(),
            actual: proof.betas.len(),
        });
    }
    for (round, (expected, actual)) in artifact.fri_betas.iter().zip(&proof.betas).enumerate() {
        if expected != actual {
            return Err(Error::FriChallengeMismatch { round });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn enforce_verify_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    if batch.transitions.len() > limits.max_transitions {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_transitions",
            actual: batch.transitions.len(),
            max: limits.max_transitions,
        });
    }
    let batch_bytes = batch_size_hint(batch);
    if batch_bytes > limits.max_batch_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_batch_bytes",
            actual: batch_bytes,
            max: limits.max_batch_bytes,
        });
    }
    let proof_bytes = proof_size_hint(proof);
    if proof_bytes > limits.max_proof_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            actual: proof_bytes,
            max: limits.max_proof_bytes,
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

#[derive(Clone, Copy)]
struct FriQueryVerification<'a> {
    query_pos: usize,
    initial_index: usize,
    initial_value: u64,
    fri_layers: &'a [[u8; 32]],
    betas: &'a [u64],
    fri_layer_lengths: &'a [usize],
    arity: u32,
}

fn verify_fri_query_chain(
    fri_query: &FriQueryOpening,
    context: FriQueryVerification<'_>,
) -> Result<()> {
    let FriQueryVerification {
        query_pos,
        initial_index,
        initial_value,
        fri_layers,
        betas,
        fri_layer_lengths,
        arity,
    } = context;
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
    if fri_layers.len() != betas.len() + 1 || fri_layer_lengths.len() != fri_layers.len() {
        return Err(Error::FriLayerLengthMismatch {
            expected: betas.len() + 1,
            actual: fri_layers.len(),
        });
    }

    let mut index = initial_index;
    let mut value = initial_value;
    for (round, opening) in fri_query.rounds.iter().enumerate() {
        let round_len = fri_layer_lengths[round];
        if index >= round_len {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let leaf_index = index / arity;
        let expected_values_len = expected_leaf_value_len(round_len, arity, leaf_index)?;
        if usize::try_from(opening.round).ok() != Some(round)
            || usize::try_from(opening.index).ok() != Some(index)
            || opening.values.len() != expected_values_len
        {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let offset = index % arity;
        if opening.values.get(offset).copied() != Some(value) {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let round_leaf_count = leaf_count_for_values(round_len, arity)?;
        let round_path_len = merkle_path_len_for_leaf_count(round_leaf_count)?;
        if opening.merkle_path.len() != round_path_len {
            return Err(Error::QueryMerklePathMismatch { index: query_pos });
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

    let final_len = *fri_layer_lengths
        .last()
        .expect("FRI layer lengths checked non-empty");
    if usize::try_from(fri_query.final_index).ok() != Some(index) || index >= final_len {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let final_leaf_index = index / arity;
    let final_offset = index % arity;
    let expected_final_values_len = expected_leaf_value_len(final_len, arity, final_leaf_index)?;
    if fri_query.final_values.len() != expected_final_values_len {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    if fri_query.final_values.get(final_offset).copied() != Some(value) {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let final_round = betas.len();
    let final_leaf_count = leaf_count_for_values(final_len, arity)?;
    let final_path_len = merkle_path_len_for_leaf_count(final_leaf_count)?;
    if fri_query.final_merkle_path.len() != final_path_len {
        return Err(Error::QueryMerklePathMismatch { index: query_pos });
    }
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

fn expected_fri_layer_lengths(
    domain_size: usize,
    arity: u32,
    max_reductions: u32,
) -> Result<Vec<usize>> {
    let arity = usize::try_from(arity).map_err(|_| Error::FriArity(arity))?;
    if arity == 0 {
        return Err(Error::FriArity(0));
    }
    let max_rounds = usize::try_from(max_reductions).expect("FRI reduction bound fits usize");
    let mut current = domain_size;
    let mut rounds = 0usize;
    let mut lengths = Vec::new();
    while current > 1 && rounds < max_rounds {
        current = pad_len_to_arity(current, arity)?;
        lengths.push(current);
        current /= arity;
        rounds += 1;
    }
    lengths.push(current);
    Ok(lengths)
}

fn pad_len_to_arity(len: usize, arity: usize) -> Result<usize> {
    if arity == 0 {
        return Err(Error::FriArity(0));
    }
    let remainder = len % arity;
    if remainder == 0 {
        return Ok(len);
    }
    len.checked_add(arity - remainder)
        .ok_or(Error::TraceLengthOverflow { rows: len })
}

fn leaf_count_for_values(value_count: usize, chunk_size: usize) -> Result<usize> {
    if value_count == 0 || chunk_size == 0 {
        return Err(Error::QueryIndexOutOfRange {
            index: 0,
            len: value_count,
        });
    }
    Ok(value_count.div_ceil(chunk_size))
}

fn expected_leaf_value_len(
    value_count: usize,
    chunk_size: usize,
    leaf_index: usize,
) -> Result<usize> {
    if value_count == 0 || chunk_size == 0 {
        return Err(Error::QueryIndexOutOfRange {
            index: 0,
            len: value_count,
        });
    }
    let start = leaf_index
        .checked_mul(chunk_size)
        .ok_or(Error::QueryIndexOverflow { index: leaf_index })?;
    if start >= value_count {
        return Err(Error::QueryIndexOutOfRange {
            index: start,
            len: value_count,
        });
    }
    Ok(value_count.saturating_sub(start).min(chunk_size))
}

fn merkle_path_len_for_leaf_count(leaf_count: usize) -> Result<usize> {
    if leaf_count == 0 {
        return Err(Error::QueryIndexOutOfRange { index: 0, len: 0 });
    }
    let mut current = leaf_count;
    let mut depth = 0usize;
    loop {
        current = pad_len_to_arity(current, 2)?;
        depth = depth
            .checked_add(1)
            .ok_or(Error::TraceLengthOverflow { rows: leaf_count })?;
        let next = current / 2;
        if next == 1 {
            return Ok(depth);
        }
        current = next;
    }
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

fn proof_size_hint(proof: &Proof) -> usize {
    let mut total = 0usize;
    total = total.saturating_add(2); // protocol_version
    total = total.saturating_add(2); // params_version
    total = total.saturating_add(proof.parameter.len());
    total = total.saturating_add(32); // trace_commitment
    total = total.saturating_add(public_io_size_hint(&proof.public_io));
    total = total.saturating_add(32 * 4); // trace, AIR, composition, and lookup roots
    total = total.saturating_add(4); // lde_domain_size
    total = total.saturating_add(8); // lookup_grand_product
    total = total.saturating_add(8); // lookup_challenge
    total = total.saturating_add(proof.alphas.len().saturating_mul(8));
    total = total.saturating_add(proof.betas.len().saturating_mul(8));
    total = total.saturating_add(proof.fri_layers.len().saturating_mul(32));
    for query in &proof.queries {
        total = total.saturating_add(4); // index
        total = total.saturating_add(8); // value
        total = total.saturating_add(query.chunk_values.len().saturating_mul(8));
        total = total.saturating_add(query.merkle_path.len().saturating_mul(8));
    }
    for opening in &proof.air_openings {
        total = total.saturating_add(4); // index
        total = total.saturating_add(opening.current_row.len().saturating_mul(8));
        total = total.saturating_add(opening.next_row.len().saturating_mul(8));
        total = total.saturating_add(opening.current_row_path.len().saturating_mul(8));
        total = total.saturating_add(opening.next_row_path.len().saturating_mul(8));
        total = total.saturating_add(8); // composition_value
        total = total.saturating_add(opening.composition_path.len().saturating_mul(8));
    }
    for query in &proof.fri_queries {
        total = total.saturating_add(4); // initial_index
        for round in &query.rounds {
            total = total.saturating_add(4); // round
            total = total.saturating_add(4); // index
            total = total.saturating_add(round.values.len().saturating_mul(8));
            total = total.saturating_add(8); // folded_value
            total = total.saturating_add(round.merkle_path.len().saturating_mul(8));
        }
        total = total.saturating_add(4); // final_index
        total = total.saturating_add(query.final_values.len().saturating_mul(8));
        total = total.saturating_add(query.final_merkle_path.len().saturating_mul(8));
    }
    total
}

fn public_io_size_hint(public_io: &PublicIO) -> usize {
    let mut total = 0usize;
    total = total.saturating_add(16); // dsid
    total = total.saturating_add(8); // slot
    total = total.saturating_add(32 * 5); // roots, tx set, and ordering hashes
    total = total.saturating_add(public_io.permission_hashes.len().saturating_mul(32));
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

    fn verify_limits_with_override(apply: impl FnOnce(&mut VerifyLimits)) -> VerifyLimits {
        let mut limits = VerifyLimits::default();
        apply(&mut limits);
        limits
    }

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

    fn sample_proof_with_size(rows: usize) -> (TransitionBatch, Proof) {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(rows);
        let proof = prover.prove(&batch).unwrap();
        (batch, proof)
    }

    fn single_fri_leaf_root_and_path(
        round: usize,
        leaf_index: usize,
        values: &[u64],
    ) -> ([u8; 32], Vec<u64>) {
        let leaf = backend::hash_fri_chunk(round, leaf_index, values).unwrap();
        (
            field_norito::core::to_bytes(trace::merkle_root(&[leaf])),
            vec![leaf],
        )
    }

    fn two_fri_leaf_root_and_path(
        round: usize,
        leaf_index: usize,
        values: &[u64],
        sibling_values: &[u64],
    ) -> ([u8; 32], Vec<u64>) {
        let leaf = backend::hash_fri_chunk(round, leaf_index, values).unwrap();
        let sibling_index = if leaf_index == 0 { 1 } else { leaf_index - 1 };
        let sibling = backend::hash_fri_chunk(round, sibling_index, sibling_values).unwrap();
        let leaves = if leaf_index == 0 {
            vec![leaf, sibling]
        } else {
            vec![sibling, leaf]
        };
        (
            field_norito::core::to_bytes(trace::merkle_root(&leaves)),
            vec![sibling],
        )
    }

    fn assert_verify_rejects<F, M>(
        batch: &TransitionBatch,
        proof: &Proof,
        mutate: F,
        matches_err: M,
    ) where
        F: FnOnce(&mut Proof),
        M: FnOnce(&Error) -> bool,
    {
        let mut tampered = proof.clone();
        mutate(&mut tampered);
        let err = verify(batch, &tampered).unwrap_err();
        assert!(matches_err(&err), "unexpected verifier error: {err:?}");
    }

    fn verify_fri_query_chain_for_test(
        initial_index: usize,
        initial_value: u64,
        fri_query: &FriQueryOpening,
        fri_layers: &[[u8; 32]],
        betas: &[u64],
        fri_layer_lengths: &[usize],
        arity: u32,
    ) -> Result<()> {
        verify_fri_query_chain(
            fri_query,
            FriQueryVerification {
                query_pos: 0,
                initial_index,
                initial_value,
                fri_layers,
                betas,
                fri_layer_lengths,
                arity,
            },
        )
    }

    fn sample_backend_artifact() -> BackendArtifact {
        BackendArtifact {
            parameter: "fastpq-lane-balanced".to_owned(),
            trace_rows: 1,
            trace_root: 11,
            air_trace_root: 12,
            air_composition_root: 13,
            lookup_root: 14,
            lde_domain_size: 1,
            lookup_grand_product: 15,
            lookup_challenge: 16,
            alphas: vec![17, 18],
            fri_arity: 2,
            fri_blowup: 2,
            fri_layers: vec![19],
            fri_betas: Vec::new(),
            query_openings: vec![(0, 20)],
            query_chunks: vec![vec![20]],
            query_paths: vec![Vec::new()],
            air_openings: vec![AirConstraintOpening {
                index: 0,
                current_row: Vec::new(),
                next_row: Vec::new(),
                current_row_path: Vec::new(),
                next_row_path: Vec::new(),
                composition_value: 21,
                composition_path: Vec::new(),
            }],
            fri_query_openings: vec![FriQueryOpening {
                initial_index: 0,
                rounds: Vec::new(),
                final_index: 0,
                final_values: vec![21],
                final_merkle_path: Vec::new(),
            }],
        }
    }

    fn materialise_sample_artifact(artifact: BackendArtifact) -> Result<Proof> {
        materialise_proof(
            Hash::new(b"materialise-proof-test"),
            PublicIO::default(),
            artifact,
            7,
        )
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
    fn public_io_preserves_explicit_roots_and_hashes_claims() {
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.public_inputs.dsid = [0x11; 16];
        batch.public_inputs.slot = 99;
        batch.public_inputs.old_root = [0x22; 32];
        batch.public_inputs.new_root = [0x33; 32];
        batch.public_inputs.perm_root = [0x44; 32];
        batch.public_inputs.tx_set_hash = [0x55; 32];
        let ordering = Hash::new(b"explicit-ordering-hash");
        let permission_hashes = vec![[0x66; 32], [0x77; 32]];

        let public_io = build_public_io(&batch, ordering, permission_hashes.clone());

        assert_eq!(public_io.dsid, batch.public_inputs.dsid);
        assert_eq!(public_io.slot, batch.public_inputs.slot);
        assert_eq!(public_io.old_root, batch.public_inputs.old_root);
        assert_eq!(public_io.new_root, batch.public_inputs.new_root);
        assert_eq!(public_io.perm_root, batch.public_inputs.perm_root);
        assert_eq!(public_io.tx_set_hash, batch.public_inputs.tx_set_hash);
        assert_eq!(
            public_io.ordering_hash,
            hash_norito::core::to_bytes(&ordering)
        );
        assert_eq!(public_io.permission_hashes, permission_hashes);
    }

    #[test]
    fn public_io_matcher_reports_direct_mismatches() {
        let expected = PublicIO {
            dsid: [0x11; 16],
            slot: 42,
            old_root: [0x22; 32],
            new_root: [0x33; 32],
            perm_root: [0x44; 32],
            tx_set_hash: [0x55; 32],
            ordering_hash: [0x66; 32],
            permission_hashes: vec![[0x77; 32]],
        };

        let mut actual = expected.clone();
        actual.dsid[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch { field: "dsid" })
        ));

        let mut actual = expected.clone();
        actual.slot = actual.slot.wrapping_add(1);
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch { field: "slot" })
        ));

        let mut actual = expected.clone();
        actual.old_root[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch { field: "old_root" })
        ));

        let mut actual = expected.clone();
        actual.new_root[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch { field: "new_root" })
        ));

        let mut actual = expected.clone();
        actual.perm_root[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch { field: "perm_root" })
        ));

        let mut actual = expected.clone();
        actual.tx_set_hash[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PublicIoMismatch {
                field: "tx_set_hash"
            })
        ));

        let mut actual = expected.clone();
        actual.ordering_hash[0] ^= 0x01;
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::OrderingHashMismatch)
        ));

        let mut actual = expected.clone();
        actual.permission_hashes.push([0x88; 32]);
        assert!(matches!(
            ensure_public_io_matches(&expected, &actual),
            Err(Error::PermissionHashMismatch)
        ));

        ensure_public_io_matches(&expected, &expected).unwrap();
    }

    #[test]
    fn public_io_hash_helpers_are_domain_separated() {
        assert!(is_zero_bytes(&[0u8; 32]));
        assert!(!is_zero_bytes(&[0, 0, 1]));
        assert_eq!(perm_root_from_permission_hashes(&[]), [0u8; 32]);

        let first = [[0x11; 32]];
        let second = [[0x22; 32]];
        let first_root = perm_root_from_permission_hashes(&first);
        let second_root = perm_root_from_permission_hashes(&second);
        let ordering_root = tx_set_hash_from_ordering(&Hash::new(first[0]));

        assert_ne!(first_root, [0u8; 32]);
        assert_ne!(first_root, second_root);
        assert_ne!(first_root, ordering_root);
    }

    #[test]
    fn collect_permission_hashes_sorts_roles_and_validates_lengths() {
        let grant_role = vec![0x11; 32];
        let grant_permission = vec![0x22; 32];
        let revoke_role = vec![0x33; 32];
        let revoke_permission = vec![0x44; 32];
        let mut batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        batch.push(StateTransition::new(
            b"z/revoke".to_vec(),
            vec![1],
            vec![2],
            OperationKind::RoleRevoke {
                role_id: revoke_role.clone(),
                permission_id: revoke_permission.clone(),
                epoch: 9,
            },
        ));
        batch.push(StateTransition::new(
            b"a/grant".to_vec(),
            vec![3],
            vec![4],
            OperationKind::RoleGrant {
                role_id: grant_role.clone(),
                permission_id: grant_permission.clone(),
                epoch: 7,
            },
        ));
        batch.push(StateTransition::new(
            b"m/mint".to_vec(),
            vec![5],
            vec![6],
            OperationKind::Mint,
        ));

        let hashes = collect_permission_hashes(&batch).unwrap();
        assert_eq!(
            hashes,
            vec![
                field_norito::core::to_bytes(
                    trace::permission_hash(&grant_role, &grant_permission, 7).unwrap()
                ),
                field_norito::core::to_bytes(
                    trace::permission_hash(&revoke_role, &revoke_permission, 9).unwrap()
                ),
            ]
        );

        let mut bad_role = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        bad_role.push(StateTransition::new(
            b"bad-role".to_vec(),
            Vec::new(),
            Vec::new(),
            OperationKind::RoleGrant {
                role_id: vec![0xAA; 31],
                permission_id: vec![0xBB; 32],
                epoch: 1,
            },
        ));
        assert!(matches!(
            collect_permission_hashes(&bad_role),
            Err(Error::InvalidRoleIdLength { length: 31 })
        ));

        let mut bad_permission =
            TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        bad_permission.push(StateTransition::new(
            b"bad-permission".to_vec(),
            Vec::new(),
            Vec::new(),
            OperationKind::RoleRevoke {
                role_id: vec![0xAA; 32],
                permission_id: vec![0xBB; 31],
                epoch: 1,
            },
        ));
        assert!(matches!(
            collect_permission_hashes(&bad_permission),
            Err(Error::InvalidPermissionIdLength { length: 31 })
        ));
    }

    #[test]
    fn batch_size_hint_counts_metadata_and_operation_payloads() {
        let role_id = vec![0xAA; 5];
        let permission_id = vec![0xBB; 7];
        let mut batch = TransitionBatch::new("param", PublicInputs::default());
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![1, 2],
            vec![3, 4, 5],
            OperationKind::RoleRevoke {
                role_id: role_id.clone(),
                permission_id: permission_id.clone(),
                epoch: 99,
            },
        ));
        batch.metadata.insert("meta".to_owned(), vec![9, 8, 7]);

        let expected = "param".len()
            + "key".len()
            + 2
            + 3
            + role_id.len()
            + permission_id.len()
            + "meta".len()
            + 3;
        assert_eq!(batch_size_hint(&batch), expected);
        assert_eq!(operation_size_hint(&OperationKind::Transfer), 0);
        assert_eq!(operation_size_hint(&OperationKind::Mint), 0);
        assert_eq!(operation_size_hint(&OperationKind::Burn), 0);
        assert_eq!(operation_size_hint(&OperationKind::MetaSet), 0);
    }

    #[test]
    fn operation_size_hint_counts_role_grant_and_revoke_payloads() {
        let role_id = vec![0x11; 3];
        let permission_id = vec![0x22; 4];
        let expected = role_id.len() + permission_id.len();

        assert_eq!(
            operation_size_hint(&OperationKind::RoleGrant {
                role_id: role_id.clone(),
                permission_id: permission_id.clone(),
                epoch: 1,
            }),
            expected
        );
        assert_eq!(
            operation_size_hint(&OperationKind::RoleRevoke {
                role_id,
                permission_id,
                epoch: 2,
            }),
            expected
        );
    }

    #[test]
    fn fallback_roots_are_order_and_input_sensitive() {
        let first = [0x11; 32];
        let second = [0x22; 32];
        assert_ne!(
            perm_root_from_permission_hashes(&[first, second]),
            perm_root_from_permission_hashes(&[second, first])
        );
        assert_ne!(
            tx_set_hash_from_ordering(&Hash::new(b"ordering-a")),
            tx_set_hash_from_ordering(&Hash::new(b"ordering-b"))
        );
    }

    #[test]
    fn verify_accepts_canonical_v1_roundtrip() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let proof = prover.prove(&batch).unwrap();
        verify(&batch, &proof).unwrap();
    }

    #[test]
    fn verify_rejects_batch_parameter_mismatch_before_replay() {
        let (mut batch, proof) = sample_proof_with_size(8);
        batch.parameter = "fastpq-lane-latency".to_owned();

        let err = verify(&batch, &proof).unwrap_err();

        assert!(matches!(
            err,
            Error::ParameterMismatch {
                expected,
                actual
            } if expected == "fastpq-lane-balanced" && actual == "fastpq-lane-latency"
        ));
    }

    #[test]
    fn canonical_prover_rejects_unknown_parameter() {
        let err = Prover::canonical("does-not-exist").unwrap_err();
        assert!(matches!(err, Error::UnknownParameter(_)));
    }

    #[test]
    fn canonical_with_modes_rejects_unknown_parameter() {
        let err = Prover::canonical_with_modes(
            "does-not-exist",
            ExecutionMode::Cpu,
            PoseidonExecutionMode::Cpu,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            Error::UnknownParameter(parameter) if parameter == "does-not-exist"
        ));
    }

    #[test]
    fn canonical_with_execution_mode_overrides_backend() {
        let prover =
            Prover::canonical_with_execution_mode("fastpq-lane-balanced", ExecutionMode::Cpu)
                .expect("prover");
        assert_eq!(prover.backend.execution_mode(), ExecutionMode::Cpu);
    }

    #[test]
    fn canonical_params_versions_track_catalogue_order() {
        let sets = Prover::canonical_parameter_sets();
        assert_eq!(sets, CANONICAL_PARAMETER_SETS.as_slice());
        for (idx, params) in sets.iter().enumerate() {
            assert_eq!(
                canonical_params_version(params),
                Some(u16::try_from(idx + 1).expect("test catalogue version fits u16"))
            );
        }

        let mut custom = sets[0];
        custom.name = "fastpq-local-test-parameter";
        assert_eq!(canonical_params_version(&custom), None);
    }

    #[test]
    fn materialise_proof_maps_backend_artifact_fields() {
        let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        assert_eq!(proof.protocol_version, PROTOCOL_VERSION);
        assert_eq!(proof.params_version, 7);
        assert_eq!(proof.parameter, "fastpq-lane-balanced");
        assert_eq!(proof.trace_root, field_norito::core::to_bytes(11));
        assert_eq!(proof.air_trace_root, field_norito::core::to_bytes(12));
        assert_eq!(proof.air_composition_root, field_norito::core::to_bytes(13));
        assert_eq!(proof.lookup_root, field_norito::core::to_bytes(14));
        assert_eq!(proof.lde_domain_size, 1);
        assert_eq!(proof.lookup_grand_product, 15);
        assert_eq!(proof.lookup_challenge, 16);
        assert_eq!(proof.alphas, vec![17, 18]);
        assert_eq!(proof.betas, Vec::<u64>::new());
        assert_eq!(proof.fri_layers, vec![field_norito::core::to_bytes(19)]);
        assert_eq!(proof.queries.len(), 1);
        assert_eq!(proof.queries[0].index, 0);
        assert_eq!(proof.queries[0].value, 20);
        assert_eq!(proof.queries[0].chunk_values, vec![20]);
        assert_eq!(proof.air_openings.len(), 1);
        assert_eq!(proof.fri_queries.len(), 1);
    }

    #[test]
    fn materialise_proof_preserves_commitment_and_public_io() {
        let commitment = Hash::new(b"explicit-materialise-commitment");
        let public_io = PublicIO {
            dsid: [0x01; 16],
            slot: 17,
            old_root: [0x02; 32],
            new_root: [0x03; 32],
            perm_root: [0x04; 32],
            tx_set_hash: [0x05; 32],
            ordering_hash: [0x06; 32],
            permission_hashes: vec![[0x07; 32], [0x08; 32]],
        };

        let proof = materialise_proof(commitment, public_io.clone(), sample_backend_artifact(), 11)
            .expect("materialise proof");

        assert_eq!(proof.commitment(), commitment);
        assert_eq!(proof.trace_commitment, commitment);
        assert_eq!(proof.public_io, public_io);
        assert_eq!(proof.params_version, 11);
    }

    #[test]
    fn materialise_proof_preserves_multiple_query_openings_and_paths() {
        let mut artifact = sample_backend_artifact();
        artifact.query_openings.push((3, 33));
        artifact.query_chunks.push(vec![30, 31, 32, 33]);
        artifact.query_paths.push(vec![101, 102]);
        artifact.air_openings.push(AirConstraintOpening {
            index: 3,
            current_row: vec![1, 2, 3],
            next_row: vec![4, 5, 6],
            current_row_path: vec![201],
            next_row_path: vec![202],
            composition_value: 34,
            composition_path: vec![203],
        });
        artifact.fri_query_openings.push(FriQueryOpening {
            initial_index: 3,
            rounds: Vec::new(),
            final_index: 3,
            final_values: vec![33, 34, 35, 36],
            final_merkle_path: vec![204],
        });

        let proof = materialise_sample_artifact(artifact).unwrap();

        assert_eq!(proof.queries.len(), 2);
        assert_eq!(proof.queries[1].index, 3);
        assert_eq!(proof.queries[1].value, 33);
        assert_eq!(proof.queries[1].chunk_values, vec![30, 31, 32, 33]);
        assert_eq!(proof.queries[1].merkle_path, vec![101, 102]);
        assert_eq!(proof.air_openings[1].index, 3);
        assert_eq!(proof.air_openings[1].composition_path, vec![203]);
        assert_eq!(proof.fri_queries[1].final_merkle_path, vec![204]);
    }

    #[test]
    fn materialise_proof_rejects_backend_artifact_count_mismatches() {
        let mut artifact = sample_backend_artifact();
        artifact.query_chunks.clear();
        let err = materialise_sample_artifact(artifact).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryCountMismatch {
                expected: 1,
                actual: 0
            }
        ));

        let mut artifact = sample_backend_artifact();
        artifact.query_paths.clear();
        let err = materialise_sample_artifact(artifact).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryCountMismatch {
                expected: 1,
                actual: 0
            }
        ));

        let mut artifact = sample_backend_artifact();
        artifact.fri_query_openings.clear();
        let err = materialise_sample_artifact(artifact).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryCountMismatch {
                expected: 1,
                actual: 0
            }
        ));

        let mut artifact = sample_backend_artifact();
        artifact.air_openings.clear();
        let err = materialise_sample_artifact(artifact).unwrap_err();
        assert!(matches!(
            err,
            Error::AirOpeningCountMismatch {
                expected: 1,
                actual: 0
            }
        ));
    }

    #[test]
    fn verify_rejects_bad_protocol_and_parameter_metadata() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.protocol_version = PROTOCOL_VERSION.wrapping_add(1),
            |err| matches!(err, Error::UnsupportedProtocolVersion { version } if *version == PROTOCOL_VERSION + 1),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.parameter = "does-not-exist".to_owned(),
            |err| matches!(err, Error::UnknownParameter(parameter) if parameter == "does-not-exist"),
        );
    }

    #[test]
    fn verify_rejects_malformed_commitment_root_encodings() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.trace_root[8] = 1,
            |err| matches!(err, Error::TraceRootMismatch),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.lookup_root[8] = 1,
            |err| matches!(err, Error::LookupRootMismatch),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.air_trace_root[8] = 1,
            |err| matches!(err, Error::AirTraceRootMismatch),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.air_composition_root[8] = 1,
            |err| matches!(err, Error::AirCompositionRootMismatch),
        );
    }

    #[test]
    fn verify_rejects_modified_roots() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.trace_root[0] ^= 0xAA;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::TraceRootMismatch));
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
    fn verify_rejects_relabelled_proof_from_different_batch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let source = sample_batch_with_size(8);
        let target = sample_batch_with_size(9);
        let mut proof = prover.prove(&source).unwrap();
        let target_proof = prover.prove(&target).unwrap();
        proof.trace_commitment = target_proof.trace_commitment;
        proof.public_io = target_proof.public_io;

        let err = verify(&target, &proof).unwrap_err();

        assert!(matches!(err, Error::TraceRootMismatch));
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
        assert!(matches!(err, Error::LookupGrandProductMismatch));
    }

    #[test]
    fn verify_rejects_modified_lookup_root() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(16);
        let mut proof = prover.prove(&batch).unwrap();
        proof.lookup_root[0] ^= 0xAA;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::LookupRootMismatch));
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
    fn verify_rejects_extra_fri_layer_for_domain_schedule() {
        let (batch, mut proof) = sample_proof_with_size(32);
        let terminal = *proof
            .fri_layers
            .last()
            .expect("expected terminal FRI layer");
        proof.fri_layers.push(terminal);

        let err = verify(&batch, &proof).unwrap_err();

        assert!(matches!(
            err,
            Error::FriLayerLengthMismatch { expected, actual }
                if actual == expected + 1
        ));
    }

    #[test]
    fn verify_rejects_empty_and_malformed_fri_layer_roots() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.fri_layers.clear(),
            |err| {
                matches!(
                    err,
                    Error::FriLayerLengthMismatch {
                        expected: _,
                        actual: 0
                    }
                )
            },
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let root = tampered
                    .fri_layers
                    .last_mut()
                    .expect("expected terminal FRI root");
                root[8] = 1;
            },
            |err| matches!(err, Error::FriLayerMismatch { .. }),
        );
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
    fn verify_rejects_fri_and_air_vector_count_mismatches() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered.fri_queries.pop();
            },
            |err| {
                matches!(
                    err,
                    Error::QueryCountMismatch {
                        expected,
                        actual
                    } if *expected == proof.queries.len() && *actual + 1 == *expected
                )
            },
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let extra = tampered
                    .fri_queries
                    .first()
                    .expect("expected sampled FRI query")
                    .clone();
                tampered.fri_queries.push(extra);
            },
            |err| {
                matches!(
                    err,
                    Error::QueryCountMismatch {
                        expected,
                        actual
                    } if *expected == proof.queries.len() && *actual == *expected + 1
                )
            },
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let extra = tampered
                    .air_openings
                    .first()
                    .expect("expected sampled AIR opening")
                    .clone();
                tampered.air_openings.push(extra);
            },
            |err| {
                matches!(
                    err,
                    Error::AirOpeningCountMismatch {
                        expected,
                        actual
                    } if *expected == proof.queries.len() && *actual == *expected + 1
                )
            },
        );
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
    fn verify_rejects_malformed_query_and_air_openings() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let query = tampered
                    .queries
                    .first_mut()
                    .expect("expected sampled query");
                query.index = query.index.wrapping_add(1);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .queries
                    .first_mut()
                    .expect("expected sampled query")
                    .chunk_values
                    .clear();
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .queries
                    .first_mut()
                    .expect("expected sampled query")
                    .chunk_values
                    .push(0);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .air_openings
                    .first_mut()
                    .expect("expected sampled AIR opening")
                    .current_row
                    .pop();
            },
            |err| matches!(err, Error::AirOpeningMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .air_openings
                    .first_mut()
                    .expect("expected sampled AIR opening")
                    .next_row
                    .pop();
            },
            |err| matches!(err, Error::AirOpeningMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .air_openings
                    .first_mut()
                    .expect("expected sampled AIR opening")
                    .current_row_path
                    .push(0);
            },
            |err| matches!(err, Error::AirMerklePathMismatch { index: 0 }),
        );
    }

    #[test]
    fn verify_rejects_wrong_air_composition_root() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.air_composition_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirCompositionRootMismatch));
    }

    #[test]
    fn verify_rejects_wrong_air_trace_root() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.air_trace_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirTraceRootMismatch));
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
    fn verify_rejects_extra_air_challenges() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.alphas.push(42);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirChallengeCountMismatch {
                expected: AIR_COMPOSITION_ALPHA_COUNT,
                actual
            } if actual == AIR_COMPOSITION_ALPHA_COUNT + 1
        ));
    }

    #[test]
    fn verify_rejects_wrong_air_challenge_values() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let alpha = tampered.alphas.first_mut().expect("expected AIR challenge");
                *alpha = alpha.wrapping_add(1);
            },
            |err| matches!(err, Error::FriChallengeMismatch { round: 0 }),
        );
    }

    #[test]
    fn verify_rejects_air_opening_index_mismatch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected sampled AIR opening");
        first.index = first.index.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirOpeningMismatch { index: 0 }));
    }

    #[test]
    fn verify_rejects_air_opening_count_mismatch() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        assert!(
            !proof.air_openings.is_empty(),
            "expected sampled AIR openings"
        );
        proof.air_openings.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirOpeningCountMismatch { expected, actual }
                if expected == proof.queries.len() && actual + 1 == expected
        ));
    }

    #[test]
    fn verify_limits_reject_transition_count_limit() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let proof_batch = sample_batch();
        let proof = prover.prove(&proof_batch).unwrap();
        let batch = sample_batch_with_size(3);
        let limits = verify_limits_with_override(|limits| {
            limits.max_transitions = batch.transitions.len() - 1
        });
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_transitions",
                actual,
                max
            } if actual == batch.transitions.len() && max + 1 == actual
        ));
    }

    #[test]
    fn verify_limits_reject_batch_size_limit() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let proof = prover.prove(&batch).unwrap();
        let batch_bytes = batch_size_hint(&batch);
        assert!(batch_bytes > 0, "sample batch must have a byte footprint");
        let limits = verify_limits_with_override(|limits| limits.max_batch_bytes = batch_bytes - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_batch_bytes",
                actual,
                max
            } if actual == batch_bytes && max + 1 == batch_bytes
        ));
    }

    #[test]
    fn verify_limits_reject_fri_layer_count_limit() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let layer_count = proof.fri_layers.len();
        assert!(layer_count > 0, "proof must carry FRI layer roots");
        let limits = verify_limits_with_override(|limits| limits.max_fri_layers = layer_count - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_fri_layers",
                actual,
                max
            } if actual == layer_count && max + 1 == layer_count
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_air_rows() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let row_len = proof
            .air_openings
            .first()
            .expect("expected sampled AIR opening")
            .current_row
            .len();
        assert!(row_len > 0, "AIR row must carry trace values");
        let limits = verify_limits_with_override(|limits| limits.max_air_row_values = row_len - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual,
                max
            } if actual == row_len && max + 1 == row_len
        ));
    }

    #[test]
    fn verify_limits_reject_query_count_limit() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let query_count = proof.queries.len();
        assert!(query_count > 0, "proof must carry sampled queries");
        let limits = verify_limits_with_override(|limits| limits.max_queries = query_count - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual,
                max
            } if actual == query_count && max + 1 == query_count
        ));
    }

    #[test]
    fn verify_limits_reject_extra_fri_query_count() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let query_count = proof.queries.len();
        assert!(query_count > 0, "proof must carry sampled queries");
        let extra = proof
            .fri_queries
            .first()
            .expect("expected sampled FRI query")
            .clone();
        proof.fri_queries.push(extra);
        let limits = verify_limits_with_override(|limits| limits.max_queries = query_count);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual,
                max
            } if actual == query_count + 1 && max == query_count
        ));
    }

    #[test]
    fn verify_limits_reject_extra_air_opening_count() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let query_count = proof.queries.len();
        assert!(query_count > 0, "proof must carry sampled queries");
        let extra = proof
            .air_openings
            .first()
            .expect("expected sampled AIR opening")
            .clone();
        proof.air_openings.push(extra);
        let limits = verify_limits_with_override(|limits| limits.max_queries = query_count);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_queries",
                actual,
                max
            } if actual == query_count + 1 && max == query_count
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_query_chunks() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let chunk_len = proof
            .queries
            .first()
            .expect("expected sampled query")
            .chunk_values
            .len();
        assert!(chunk_len > 0, "query chunk must carry values");
        let limits =
            verify_limits_with_override(|limits| limits.max_query_chunk_values = chunk_len - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_chunk_values",
                actual,
                max
            } if actual == chunk_len && max + 1 == chunk_len
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_query_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let path = &mut proof
            .queries
            .first_mut()
            .expect("expected sampled query")
            .merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, 0);
        let err = verify_with_limits(&batch, &proof, VerifyLimits::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual,
                max
            } if actual == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1
                && max == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN
        ));
    }

    #[test]
    fn enforce_verify_limits_allows_values_at_exact_boundaries() {
        let batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        let limits = VerifyLimits {
            max_transitions: batch.transitions.len(),
            max_batch_bytes: batch_size_hint(&batch),
            max_proof_bytes: proof_size_hint(&proof),
            max_fri_layers: proof.fri_layers.len(),
            max_queries: proof.queries.len(),
            max_query_chunk_values: proof.queries[0].chunk_values.len(),
            max_query_path_len: 0,
            max_fri_round_values: proof.fri_queries[0].final_values.len(),
            max_air_row_values: 0,
        };

        enforce_verify_limits(&batch, &proof, limits).unwrap();
    }

    #[test]
    fn verify_limits_reject_oversized_proof_payload() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let proof_bytes = proof_size_hint(&proof);
        assert!(proof_bytes > 0, "proof size hint should be non-zero");
        let limits = verify_limits_with_override(|limits| limits.max_proof_bytes = proof_bytes - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                actual,
                max
            } if actual == proof_bytes && max + 1 == proof_bytes
        ));
    }

    #[test]
    fn enforce_verify_limits_rejects_air_next_and_composition_paths() {
        let batch = TransitionBatch::new("fastpq-lane-balanced", PublicInputs::default());
        let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        proof.air_openings[0].next_row_path.push(7);
        let limits = verify_limits_with_override(|limits| limits.max_query_path_len = 0);
        let err = enforce_verify_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual: 1,
                max: 0
            }
        ));

        let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        proof.air_openings[0].composition_path.push(8);
        let err = enforce_verify_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual: 1,
                max: 0
            }
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_air_merkle_paths() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let path_len = proof
            .air_openings
            .first()
            .expect("expected sampled AIR opening")
            .current_row_path
            .len();
        assert!(path_len > 0, "AIR opening must carry Merkle siblings");
        let limits = verify_limits_with_override(|limits| limits.max_query_path_len = path_len - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual,
                max
            } if actual == path_len && max + 1 == path_len
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_final_fri_values() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let values = &mut proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening")
            .final_values;
        values.resize(DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES + 1, 0);
        let err = verify_with_limits(&batch, &proof, VerifyLimits::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_fri_round_values",
                actual,
                max
            } if actual == DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES + 1
                && max == DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_fri_round_values() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let proof = prover.prove(&batch).unwrap();
        let values_len = proof
            .fri_queries
            .first()
            .and_then(|query| query.rounds.first())
            .expect("expected FRI round opening")
            .values
            .len();
        assert!(values_len > 0, "FRI round must carry opened values");
        let limits =
            verify_limits_with_override(|limits| limits.max_fri_round_values = values_len - 1);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_fri_round_values",
                actual,
                max
            } if actual == values_len && max + 1 == values_len
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_final_fri_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let path = &mut proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening")
            .final_merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, 0);
        let err = verify_with_limits(&batch, &proof, VerifyLimits::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual,
                max
            } if actual == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1
                && max == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN
        ));
    }

    #[test]
    fn verify_limits_reject_oversized_fri_round_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let path = &mut proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.rounds.first_mut())
            .expect("expected FRI round opening")
            .merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, 0);
        let err = verify_with_limits(&batch, &proof, VerifyLimits::default()).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_query_path_len",
                actual,
                max
            } if actual == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1
                && max == DEFAULT_MAX_VERIFY_QUERY_PATH_LEN
        ));
    }

    #[test]
    fn verify_rejects_wrong_final_fri_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let query = proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening");
        let sibling = query
            .final_merkle_path
            .first_mut()
            .expect("expected final FRI Merkle path");
        *sibling = sibling.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { .. }));
    }

    #[test]
    fn verify_rejects_zero_lde_domain_size() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        proof.lde_domain_size = 0;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 0, len: 0 }
        ));
    }

    #[test]
    fn verify_rejects_wrong_fri_folded_value() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let round = proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.rounds.first_mut())
            .expect("expected FRI round opening");
        round.folded_value = round.folded_value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { .. }));
    }

    #[test]
    fn verify_rejects_wrong_final_fri_value() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let value = proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.final_values.first_mut())
            .expect("expected final FRI values");
        *value = value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryMismatch { .. } | Error::QueryMerklePathMismatch { .. }
        ));
    }

    #[test]
    fn verify_rejects_malformed_fri_query_chain() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let round = tampered
                    .fri_queries
                    .first_mut()
                    .and_then(|query| query.rounds.first_mut())
                    .expect("expected sampled FRI round");
                round.values.pop();
            },
            |err| matches!(err, Error::QueryMismatch { .. }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let query = tampered
                    .fri_queries
                    .first_mut()
                    .expect("expected FRI query opening");
                query.initial_index = query.initial_index.wrapping_add(1);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let round = tampered
                    .fri_queries
                    .first_mut()
                    .and_then(|query| query.rounds.first_mut())
                    .expect("expected FRI round opening");
                round.round = round.round.wrapping_add(1);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let round = tampered
                    .fri_queries
                    .first_mut()
                    .and_then(|query| query.rounds.first_mut())
                    .expect("expected FRI round opening");
                round.index = round.index.wrapping_add(1);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
    }

    #[test]
    fn verify_rejects_malformed_fri_query_chain_lengths() {
        let (batch, proof) = sample_proof_with_size(32);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .fri_queries
                    .first_mut()
                    .and_then(|query| query.rounds.first_mut())
                    .expect("expected FRI round opening")
                    .values
                    .clear();
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .fri_queries
                    .first_mut()
                    .expect("expected FRI query opening")
                    .rounds
                    .pop();
            },
            |err| matches!(err, Error::FriChallengeLengthMismatch { .. }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let query = tampered
                    .fri_queries
                    .first_mut()
                    .expect("expected FRI query opening");
                query.final_index = query.final_index.wrapping_add(1);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .fri_queries
                    .first_mut()
                    .expect("expected FRI query opening")
                    .final_values
                    .clear();
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                tampered
                    .fri_queries
                    .first_mut()
                    .expect("expected FRI query opening")
                    .final_values
                    .push(0);
            },
            |err| matches!(err, Error::QueryMismatch { index: 0 }),
        );
    }

    #[test]
    fn verify_fri_query_chain_rejects_zero_arity() {
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values: vec![1],
            final_merkle_path: Vec::new(),
        };
        let err = verify_fri_query_chain_for_test(0, 1, &fri_query, &[], &[], &[], 0).unwrap_err();
        assert!(matches!(err, Error::FriArity(0)));
    }

    #[test]
    fn verify_fri_query_chain_accepts_terminal_leaf_without_rounds() {
        let final_values = vec![42];
        let (final_root, final_path) = single_fri_leaf_root_and_path(0, 0, &final_values);
        let fri_layers = vec![final_root];
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values,
            final_merkle_path: final_path,
        };

        verify_fri_query_chain_for_test(0, 42, &fri_query, &fri_layers, &[], &[1], 2).unwrap();
    }

    #[test]
    fn verify_fri_query_chain_accepts_single_fold_round() {
        let beta = 3;
        let round_values = vec![10, 20];
        let folded = fold_fri_values(&round_values, beta);
        let final_values = vec![folded];
        let (round_root, round_path) = single_fri_leaf_root_and_path(0, 0, &round_values);
        let (final_root, final_path) = single_fri_leaf_root_and_path(1, 0, &final_values);
        let fri_layers = vec![round_root, final_root];
        let fri_query = FriQueryOpening {
            initial_index: 1,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 1,
                values: round_values,
                folded_value: folded,
                merkle_path: round_path,
            }],
            final_index: 0,
            final_values,
            final_merkle_path: final_path,
        };

        verify_fri_query_chain_for_test(1, 20, &fri_query, &fri_layers, &[beta], &[2, 1], 2)
            .unwrap();
    }

    #[test]
    fn verify_fri_query_chain_accepts_nonzero_final_offset() {
        let beta = 5;
        let round_values = vec![12, 34];
        let sibling_values = vec![77, 88];
        let sibling_folded = fold_fri_values(&sibling_values, beta);
        let folded = fold_fri_values(&round_values, beta);
        let final_values = vec![sibling_folded, folded];
        let (round_root, round_path) =
            two_fri_leaf_root_and_path(0, 1, &round_values, &sibling_values);
        let (final_root, final_path) = single_fri_leaf_root_and_path(1, 0, &final_values);
        let fri_layers = vec![round_root, final_root];
        let fri_query = FriQueryOpening {
            initial_index: 3,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 3,
                values: round_values,
                folded_value: folded,
                merkle_path: round_path,
            }],
            final_index: 1,
            final_values,
            final_merkle_path: final_path,
        };

        verify_fri_query_chain_for_test(3, 34, &fri_query, &fri_layers, &[beta], &[4, 2], 2)
            .unwrap();
    }

    #[test]
    fn verify_fri_query_chain_rejects_single_fold_round_mismatch() {
        let beta = 3;
        let round_values = vec![10, 20];
        let folded = fold_fri_values(&round_values, beta);
        let final_values = vec![folded];
        let (round_root, round_path) = single_fri_leaf_root_and_path(0, 0, &round_values);
        let (final_root, final_path) = single_fri_leaf_root_and_path(1, 0, &final_values);
        let mut fri_layers = vec![round_root, final_root];
        let fri_query = FriQueryOpening {
            initial_index: 1,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 1,
                values: round_values,
                folded_value: folded,
                merkle_path: round_path,
            }],
            final_index: 0,
            final_values,
            final_merkle_path: final_path,
        };

        fri_layers[0][0] ^= 0x01;
        let err =
            verify_fri_query_chain_for_test(1, 20, &fri_query, &fri_layers, &[beta], &[2, 1], 2)
                .unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { index: 0 }));

        let mut bad_query = fri_query;
        bad_query.rounds[0].folded_value = bad_query.rounds[0].folded_value.wrapping_add(1);
        fri_layers[0][0] ^= 0x01;
        let err =
            verify_fri_query_chain_for_test(1, 20, &bad_query, &fri_layers, &[beta], &[2, 1], 2)
                .unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));
    }

    #[test]
    fn verify_fri_query_chain_rejects_offset_value_mismatches() {
        let beta = 5;
        let round_values = vec![12, 34];
        let sibling_values = vec![77, 88];
        let sibling_folded = fold_fri_values(&sibling_values, beta);
        let folded = fold_fri_values(&round_values, beta);
        let final_values = vec![sibling_folded, folded];
        let (round_root, round_path) =
            two_fri_leaf_root_and_path(0, 1, &round_values, &sibling_values);
        let (final_root, final_path) = single_fri_leaf_root_and_path(1, 0, &final_values);
        let fri_layers = vec![round_root, final_root];
        let fri_query = FriQueryOpening {
            initial_index: 3,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 3,
                values: round_values,
                folded_value: folded,
                merkle_path: round_path,
            }],
            final_index: 1,
            final_values,
            final_merkle_path: final_path,
        };

        let err =
            verify_fri_query_chain_for_test(3, 35, &fri_query, &fri_layers, &[beta], &[4, 2], 2)
                .unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));

        let mut bad_final_query = fri_query;
        bad_final_query.final_values[1] = bad_final_query.final_values[1].wrapping_add(1);
        let err = verify_fri_query_chain_for_test(
            3,
            34,
            &bad_final_query,
            &fri_layers,
            &[beta],
            &[4, 2],
            2,
        )
        .unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));
    }

    #[test]
    fn verify_fri_query_chain_rejects_challenge_and_layer_length_mismatches() {
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values: vec![1],
            final_merkle_path: Vec::new(),
        };
        let (final_root, _) = single_fri_leaf_root_and_path(0, 0, &[1]);
        let err = verify_fri_query_chain_for_test(0, 1, &fri_query, &[final_root], &[7], &[1], 2)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::FriChallengeLengthMismatch {
                expected: 1,
                actual: 0
            }
        ));

        let round_values = vec![1, 2];
        let folded = fold_fri_values(&round_values, 7);
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 0,
                values: round_values,
                folded_value: folded,
                merkle_path: Vec::new(),
            }],
            final_index: 0,
            final_values: vec![folded],
            final_merkle_path: Vec::new(),
        };
        let err = verify_fri_query_chain_for_test(0, 1, &fri_query, &[final_root], &[7], &[2], 2)
            .unwrap_err();
        assert!(matches!(
            err,
            Error::FriLayerLengthMismatch {
                expected: 2,
                actual: 1
            }
        ));
    }

    #[test]
    fn verify_fri_query_chain_rejects_malformed_round_and_final_roots() {
        let beta = 7;
        let round_values = vec![1, 2];
        let folded = fold_fri_values(&round_values, beta);
        let final_values = vec![folded];
        let (round_root, round_path) = single_fri_leaf_root_and_path(0, 0, &round_values);
        let (final_root, final_path) = single_fri_leaf_root_and_path(1, 0, &final_values);
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 0,
                values: round_values.clone(),
                folded_value: folded,
                merkle_path: round_path,
            }],
            final_index: 0,
            final_values: final_values.clone(),
            final_merkle_path: final_path,
        };

        let mut malformed_round_root = round_root;
        malformed_round_root[8] = 1;
        let err = verify_fri_query_chain_for_test(
            0,
            1,
            &fri_query,
            &[malformed_round_root, final_root],
            &[beta],
            &[2, 1],
            2,
        )
        .unwrap_err();
        assert!(matches!(err, Error::FriLayerMismatch { round: 0 }));

        let mut malformed_final_root = final_root;
        malformed_final_root[8] = 1;
        let err = verify_fri_query_chain_for_test(
            0,
            1,
            &fri_query,
            &[round_root, malformed_final_root],
            &[beta],
            &[2, 1],
            2,
        )
        .unwrap_err();
        assert!(matches!(err, Error::FriLayerMismatch { round: 1 }));
    }

    #[test]
    fn verify_fri_query_chain_rejects_terminal_layer_shape_errors() {
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values: vec![42],
            final_merkle_path: vec![backend::hash_fri_chunk(0, 0, &[42]).unwrap()],
        };
        let err =
            verify_fri_query_chain_for_test(0, 42, &fri_query, &[], &[], &[1], 2).unwrap_err();
        assert!(matches!(
            err,
            Error::FriLayerLengthMismatch {
                expected: 1,
                actual: 0
            }
        ));

        let mut malformed_root = field_norito::core::to_bytes(1);
        malformed_root[8] = 1;
        let err =
            verify_fri_query_chain_for_test(0, 42, &fri_query, &[malformed_root], &[], &[1], 2)
                .unwrap_err();
        assert!(matches!(err, Error::FriLayerMismatch { round: 0 }));

        let wrong_root = field_norito::core::to_bytes(1);
        let err = verify_fri_query_chain_for_test(0, 42, &fri_query, &[wrong_root], &[], &[1], 2)
            .unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { index: 0 }));
    }

    #[test]
    fn modular_fri_folding_wraps_in_goldilocks_field() {
        assert_eq!(add_mod(GOLDILOCKS_MODULUS - 1, 2), 1);
        assert_eq!(mul_mod(GOLDILOCKS_MODULUS - 1, GOLDILOCKS_MODULUS - 1), 1);

        let values = [3, 4, 5];
        let challenge = 7;
        let expected = values.iter().enumerate().fold(0u128, |acc, (idx, value)| {
            let power = (0..idx).fold(1u128, |power, _| {
                (power * u128::from(challenge)) % u128::from(GOLDILOCKS_MODULUS)
            });
            (acc + u128::from(*value) * power) % u128::from(GOLDILOCKS_MODULUS)
        });
        assert_eq!(
            fold_fri_values(&values, challenge),
            u64::try_from(expected).unwrap()
        );

        let wrapped = fold_fri_values(&[GOLDILOCKS_MODULUS - 1, 2], GOLDILOCKS_MODULUS - 1);
        assert_eq!(wrapped, GOLDILOCKS_MODULUS - 3);
    }

    #[test]
    fn verify_rejects_wrong_air_next_row_opening() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected sampled AIR opening");
        let value = first
            .next_row
            .first_mut()
            .expect("expected next AIR row values");
        *value = value.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirMerklePathMismatch { .. } | Error::AirConstraintMismatch { .. }
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
    fn verify_rejects_wrong_air_composition_merkle_path() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(32);
        let mut proof = prover.prove(&batch).unwrap();
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected sampled AIR opening");
        let sibling = first
            .composition_path
            .first_mut()
            .expect("expected composition Merkle path");
        *sibling = sibling.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirMerklePathMismatch { .. }));
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
        let limits = verify_limits_with_override(|limits| {
            limits.max_transitions = large_batch.transitions.len()
        });
        let err = verify_with_limits(&large_batch, &proof, limits).unwrap_err();
        assert!(matches!(err, Error::CommitmentMismatch));
    }

    #[test]
    fn verify_accepts_large_batch_when_limits_allow() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch_with_size(DEFAULT_MAX_VERIFY_TRANSITIONS + 1);
        let proof = prover.prove(&batch).unwrap();
        let limits =
            verify_limits_with_override(|limits| limits.max_transitions = batch.transitions.len());
        verify_with_limits(&batch, &proof, limits).unwrap();
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
    fn proof_norito_roundtrip_decodes_original() {
        let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        let encoded = norito::core::to_bytes(&proof).expect("encode proof");
        let decoded: Proof = norito::decode_from_bytes(&encoded).expect("decode proof");

        assert_eq!(decoded, proof);
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
        assert_eq!(field_norito::core::from_bytes(&encoded), Some(value));
    }

    #[test]
    fn field_norito_from_bytes_rejects_nonzero_tail() {
        let mut encoded = field_norito::core::to_bytes(7);
        encoded[8] = 1;
        assert_eq!(field_norito::core::from_bytes(&encoded), None);
    }
}
