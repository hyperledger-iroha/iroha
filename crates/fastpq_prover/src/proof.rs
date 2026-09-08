use crate::{
    Error, Result, TransitionBatch,
    backend::{
        self, BackendArtifact, BackendConfig, ExecutionMode, LOOKUP_PRODUCT_DOMAIN,
        MerkleTreeRoleV1, PoseidonExecutionMode, StarkBackend, TRANSCRIPT_TAG_AIR_ROOTS,
        TRANSCRIPT_TAG_ALPHA_PREFIX, TRANSCRIPT_TAG_COLUMN_MIX_PREFIX, TRANSCRIPT_TAG_GAMMA,
        TRANSCRIPT_TAG_INIT, TRANSCRIPT_TAG_ROOTS,
    },
    field::GoldilocksFp4V1,
    ordering,
    semantics::{ProofSemantics, validate_batch_semantics},
    trace,
};
#[cfg(test)]
use crate::{backend::AIR_COMPOSITION_ALPHA_COUNT, trace_commitment};
use core::convert::TryFrom;
use fastpq_isi::{
    CANONICAL_PARAMETER_SETS, GoldilocksDigest384V1 as NativeGoldilocksDigest384V1,
    StarkParameterSet, find_by_name,
};
use iroha_crypto::Hash;
use iroha_data_model::privacy::GoldilocksDigest384V1;
use norito::{NoritoDeserialize, NoritoSerialize};

#[path = "proof/lde_leaf_cache.rs"]
mod lde_leaf_cache;
/// Protocol version advertised by the V1 prover implementation.
const PROTOCOL_VERSION: u16 = 1;
#[cfg(test)]
/// Canonical first-release root-frame identity for [`PublicIO`].
const PUBLIC_IO_SCHEMA_NAME: &str = "fastpq_prover::proof::FastpqStateTransitionPublicIoV1";
#[cfg(test)]
/// Canonical first-release root-frame identity for [`Proof`].
const PROOF_SCHEMA_NAME: &str = "fastpq_prover::proof::FastpqStateTransitionProofV1";
/// Default maximum transitions accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_TRANSITIONS: usize = 256;
/// Default maximum batch payload bytes accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_BATCH_BYTES: usize = 256 * 1024;
/// Default maximum approximate proof payload bytes accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_PROOF_BYTES: usize = 512 * 1024;
/// Default maximum FRI layers accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_FRI_LAYERS: usize = 19;
/// Default maximum query openings accepted by the V1 verifier.
const DEFAULT_MAX_VERIFY_QUERIES: usize = 136;
/// Default maximum LDE values carried by a single query chunk.
const DEFAULT_MAX_VERIFY_QUERY_CHUNK_VALUES: usize = 128;
/// Default maximum Merkle siblings carried by a single query opening.
const DEFAULT_MAX_VERIFY_QUERY_PATH_LEN: usize = 64;
/// Default maximum FRI values carried by a single round opening.
const DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES: usize = 16;
/// Default maximum AIR row values carried by a sampled opening.
const DEFAULT_MAX_VERIFY_AIR_ROW_VALUES: usize = trace::DEFAULT_MAX_TRACE_COLUMNS;
/// Public inputs committed by the prover and checked by the verifier.
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    Default,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "fastpq_prover::proof::PublicIO",
    frame = "fastpq_prover::proof::FastpqStateTransitionPublicIoV1"
)]
pub struct PublicIO {
    /// Data-space identifier (little-endian UUID).
    pub dsid: [u8; 16],
    /// Slot timestamp (nanoseconds since epoch).
    pub slot: u64,
    /// Sparse Merkle tree root before executing the batch.
    pub old_root: [u8; 32],
    /// Sparse Merkle tree root after executing the batch.
    pub new_root: [u8; 32],
    /// Contextual permission-table commitment supplied for this slot.
    ///
    /// Transcript binding does not prove permission membership or authorization.
    pub perm_root: [u8; 32],
    /// Transaction set hash recorded by the scheduler.
    pub tx_set_hash: [u8; 32],
    /// Deterministic ordering hash over canonicalised transitions.
    pub ordering_hash: [u8; 32],
}
/// Mixed-trace evaluation opening at a verifier query in the LDE domain.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct QueryOpening {
    /// Domain index opened by the prover.
    pub index: u32,
    /// Evaluation value at the queried index.
    pub value: GoldilocksFp4V1,
    /// Full LDE leaf chunk containing `value`.
    pub chunk_values: Vec<GoldilocksFp4V1>,
    /// Merkle authentication path from the LDE leaf chunk to `lde_root`.
    pub merkle_path: Vec<GoldilocksDigest384V1>,
}
/// Opened FRI fold group for one query at one round.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct FriRoundOpening {
    /// FRI round number.
    pub round: u32,
    /// Index inside the current round domain.
    pub index: u32,
    /// Multiplicative-coset values ordered as `f(x · ζ^k)` for increasing `k`.
    ///
    /// The final reduction may use an effective arity smaller than the configured arity.
    pub values: Vec<GoldilocksFp4V1>,
    /// Folded value carried into the next round.
    pub folded_value: GoldilocksFp4V1,
    /// Merkle authentication path for `values` under `fri_layers[round]`.
    pub merkle_path: Vec<GoldilocksDigest384V1>,
}
/// Per-query FRI opening chain across all committed rounds.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct FriQueryOpening {
    /// Initial evaluation-domain index sampled by the transcript.
    pub initial_index: u32,
    /// Round openings from the initial layer through the last fold.
    pub rounds: Vec<FriRoundOpening>,
    /// Index inside the final FRI layer.
    pub final_index: u32,
    /// Complete terminal-domain evaluations authenticated under the terminal root.
    ///
    /// V1 retains four evaluations in natural domain order, committed as one
    /// leaf, and requires their interpolating polynomial to have degree < 1.
    pub final_values: Vec<GoldilocksFp4V1>,
    /// Merkle authentication path for `final_values` under the terminal root.
    pub final_merkle_path: Vec<GoldilocksDigest384V1>,
}
/// Sampled AIR row and composition opening.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct AirConstraintOpening {
    /// Evaluation-domain index sampled by the verifier transcript.
    pub index: u32,
    /// AIR trace row values at `index`.
    pub current_row: Vec<u64>,
    /// AIR trace row values at `(index + blowup_factor) mod domain_size`, the
    /// next base-trace point in the LDE domain.
    pub next_row: Vec<u64>,
    /// Merkle authentication path for `current_row` under `air_trace_root`.
    pub current_row_path: Vec<GoldilocksDigest384V1>,
    /// Merkle authentication path for `next_row` under `air_trace_root`.
    pub next_row_path: Vec<GoldilocksDigest384V1>,
    /// Sampled quotient value combined with the mixed trace for joint FRI.
    pub composition_value: GoldilocksFp4V1,
    /// Merkle authentication path for `composition_value` under `air_composition_root`.
    pub composition_path: Vec<GoldilocksDigest384V1>,
}
/// Proof artifact produced by the FASTPQ prover.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::proof::Proof",
    frame = "fastpq_prover::proof::FastpqStateTransitionProofV1"
)]
pub struct Proof {
    /// Protocol version used to derive Fiat–Shamir challenges.
    pub protocol_version: u16,
    /// Parameter set name used by the prover.
    pub parameter: String,
    /// Deterministic commitment over the canonicalised batch.
    pub trace_commitment: GoldilocksDigest384V1,
    /// V1 public inputs.
    pub public_io: PublicIO,
    /// Poseidon commitment over the trace columns.
    pub trace_root: GoldilocksDigest384V1,
    /// Poseidon commitment over row-major AIR trace openings.
    pub air_trace_root: GoldilocksDigest384V1,
    /// Poseidon commitment over the AIR composition evaluation vector.
    pub air_composition_root: GoldilocksDigest384V1,
    /// Poseidon commitment over Fp4 mixed-trace LDE leaves.
    pub lde_root: GoldilocksDigest384V1,
    /// Number of evaluation rows committed by `lde_root`.
    pub lde_domain_size: u32,
    /// Permission accumulator over the canonical witness LDE.
    pub lookup_grand_product: u64,
    /// Transcript-derived permission accumulator challenge.
    pub lookup_challenge: u64,
    /// Fp4 composition challenges sampled after the trace roots and permission challenge.
    pub alphas: Vec<GoldilocksFp4V1>,
    /// FRI folding challenges (`β_ℓ`).
    pub betas: Vec<GoldilocksFp4V1>,
    /// Roots for each joint FRI layer (last element is the terminal root).
    ///
    /// The initial layer evaluates `Q + rho*T + sigma*X^N*T`, with both
    /// challenges sampled after the mixed-trace and quotient commitments.
    pub fri_layers: Vec<GoldilocksDigest384V1>,
    /// Openings into the evaluation domain sampled by the verifier.
    pub queries: Vec<QueryOpening>,
    /// AIR constraint openings for the same sampled query indices.
    pub air_openings: Vec<AirConstraintOpening>,
    /// Per-round FRI openings for the same sampled query indices.
    pub fri_queries: Vec<FriQueryOpening>,
}
impl Proof {
    /// Access the canonical six-lane preprocessing-trace commitment.
    pub fn commitment(&self) -> GoldilocksDigest384V1 {
        self.trace_commitment
    }
}
/// Limits applied before FASTPQ V1 proof verification consumes proof-carried openings.
///
/// These are independent ceilings, not an admitted workload guarantee. A batch
/// below the 256-transition default can still exceed the 512-KiB approximate
/// proof-size ceiling because AIR width, query count, and authentication paths
/// also contribute to the proof size.
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
fn prover_self_check_limits(batch: &TransitionBatch, proof: &Proof) -> VerifyLimits {
    VerifyLimits {
        max_transitions: DEFAULT_MAX_VERIFY_TRANSITIONS.max(batch.transitions.len()),
        max_batch_bytes: DEFAULT_MAX_VERIFY_BATCH_BYTES.max(batch_size_hint(batch)),
        max_proof_bytes: DEFAULT_MAX_VERIFY_PROOF_BYTES.max(proof_size_hint(proof)),
        ..VerifyLimits::default()
    }
}
/// Enforce the batch-only portion of the default verifier limits before expensive proof work.
pub fn enforce_default_verify_batch_limits(batch: &TransitionBatch) -> Result<()> {
    let limits = VerifyLimits::default();
    enforce_transition_limit(batch, limits)?;
    enforce_batch_size_limit(batch, limits)
}
/// Enforce every default verifier resource limit on an already generated proof.
pub fn enforce_default_verify_limits(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
    enforce_verify_limits(batch, proof, VerifyLimits::default())
}
/// FASTPQ prover wiring canonical STARK parameters to the backend.
#[derive(Debug, Clone)]
pub struct Prover {
    backend: StarkBackend,
}
impl Prover {
    fn new(params: StarkParameterSet) -> Self {
        Self::from_backend_config(BackendConfig::new(params))
    }
    fn from_backend_config(config: BackendConfig) -> Self {
        Self {
            backend: StarkBackend::new(config),
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
    /// This helper mirrors [`Prover::canonical`] but forces the backend to use the specified
    /// [`ExecutionMode`]. CPU and automatic selection use the implemented CPU proof path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownParameter`] when the supplied name is not part of
    /// the canonical FASTPQ catalogue, or [`Error::NativeV1GpuUnavailable`] for an explicit GPU
    /// request. Final-V1 proof GPU dispatch is not implemented.
    pub fn canonical_with_execution_mode(
        parameter_name: &str,
        mode: ExecutionMode,
    ) -> Result<Self> {
        let poseidon_mode = match mode {
            ExecutionMode::Cpu => PoseidonExecutionMode::Cpu,
            ExecutionMode::Gpu => PoseidonExecutionMode::Gpu,
            ExecutionMode::Auto => PoseidonExecutionMode::Auto,
        };
        Self::canonical_with_modes(parameter_name, mode, poseidon_mode)
    }
    /// Construct a prover using explicit execution and Poseidon pipeline modes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownParameter`] when the named parameter set is not
    /// part of the canonical FASTPQ catalogue, or [`Error::NativeV1GpuUnavailable`] when either
    /// execution or Poseidon mode explicitly requires GPU proof dispatch.
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
        config.validate_native_v1_modes()?;
        Ok(Self::from_backend_config(config))
    }
    /// Return the sole canonical parameter set exposed by this crate.
    pub fn canonical_parameter_sets() -> &'static [StarkParameterSet] {
        &CANONICAL_PARAMETER_SETS
    }
    /// Produce a proof for the provided batch.
    ///
    /// Every returned proof satisfies the same default resource limits as
    /// [`verify`]. The transition-count and proof-byte ceilings apply together;
    /// staying below the row ceiling does not guarantee that a proof fits.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidProofSemantics`] unless the batch is an unchanged empty statement or
    /// contains only operations with a production root-bound V1 relation, or [`Error::VerifierLimitExceeded`]
    /// if the batch or generated proof exceeds the paired verifier's default limits.
    /// Other errors propagate from
    /// [`crate::trace_commitment`]
    /// and the configured backend implementation. The generated proof is verified through the
    /// canonical verifier path before being returned.
    pub fn prove(&self, batch: &TransitionBatch) -> Result<Proof> {
        enforce_default_verify_batch_limits(batch)?;
        let proof = self.prove_with_semantics(batch, ProofSemantics::StateTransition)?;
        enforce_default_verify_limits(batch, &proof)?;
        Ok(proof)
    }

    pub(crate) fn prove_with_semantics(
        &self,
        batch: &TransitionBatch,
        semantics: ProofSemantics,
    ) -> Result<Proof> {
        self.backend.validate_native_v1_modes()?;
        validate_batch_semantics(batch, semantics)?;
        self.prove_raw(batch)
    }

    /// Produce a cryptographic fixture proof without assigning state-transition semantics.
    ///
    /// This escape hatch is available only to unit tests and `dev-tools` builds. It proves byte
    /// and transcript determinism, not that the supplied operations constitute valid state
    /// updates. Production callers must use [`Self::prove`] or an AXT-bound proving wrapper.
    /// Batch-count and byte limits expand for these developer fixtures; their
    /// proofs can exceed the default production verifier's resource limits.
    ///
    /// # Errors
    ///
    /// Propagates trace construction, backend, encoding, and cryptographic self-check errors.
    #[cfg(any(test, feature = "dev-tools"))]
    pub fn prove_raw_statement(&self, batch: &TransitionBatch) -> Result<Proof> {
        self.prove_raw(batch)
    }

    fn prove_raw(&self, batch: &TransitionBatch) -> Result<Proof> {
        self.backend.validate_native_v1_modes()?;
        if self.backend.parameter_name() != batch.parameter {
            return Err(Error::ParameterMismatch {
                expected: self.backend.parameter_name().to_owned(),
                actual: batch.parameter.clone(),
            });
        }
        let canonical = batch.canonicalized();
        trace::ensure_trace_schema_limit(&canonical, DEFAULT_MAX_VERIFY_AIR_ROW_VALUES)?;
        let ordering = ordering::ordering_hash(&canonical)?;
        let public_io = build_public_io(&canonical, ordering);
        let artifact = self
            .backend
            .prove(&canonical, &public_io, PROTOCOL_VERSION)?;
        let proof = materialise_proof(public_io, artifact)?;
        verify_with_limits_raw(
            &canonical,
            &proof,
            prover_self_check_limits(&canonical, &proof),
        )?;
        Ok(proof)
    }
}
/// Verify a V1 proof with default proof-size and transcript limits.
///
/// # Errors
///
/// Returns [`Error::InvalidProofSemantics`] unless the batch is an unchanged empty statement or
/// contains only operations with a production root-bound V1 relation,
/// [`Error::UnknownParameter`] when the proof references an unknown parameter set, or another
/// [`Error`] identifying the invalid proof component.
pub fn verify(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
    verify_with_semantics(batch, proof, ProofSemantics::StateTransition)
}

/// Verify a proof under an explicitly selected, caller-authenticated semantic profile.
pub fn verify_with_semantics(
    batch: &TransitionBatch,
    proof: &Proof,
    semantics: ProofSemantics,
) -> Result<()> {
    verify_with_limits_and_semantics(batch, proof, VerifyLimits::default(), semantics)
}
/// Verify a V1 proof from proof contents, public inputs, commitments, Merkle paths, AIR openings,
/// FRI query chains, challenges, and protocol/parameter checks.
///
/// # Errors
///
/// Returns [`Error::VerifierLimitExceeded`] before semantic or cryptographic work when inputs
/// exceed the supplied limits, [`Error::InvalidProofSemantics`] for batches containing operations
/// without a production root-bound V1 relation, [`Error::UnknownParameter`] for unknown parameter
/// sets, or another [`Error`] identifying the invalid proof component.
#[allow(clippy::too_many_lines)]
pub fn verify_with_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    verify_with_limits_and_semantics(batch, proof, limits, ProofSemantics::StateTransition)
}

/// Verify a proof under explicit resource limits and a caller-authenticated semantic profile.
pub fn verify_with_limits_and_semantics(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
    semantics: ProofSemantics,
) -> Result<()> {
    // Bound attacker-controlled batch/proof scans before inspecting profile semantics or
    // rebuilding transcript commitments.
    enforce_verify_limits(batch, proof, limits)?;
    verify_prechecked_with_semantics(batch, proof, limits.max_air_row_values, semantics)
}

/// Verify an AXT proof after the caller has applied [`VerifyLimits::default`].
///
/// This is crate-private so only admission paths that performed the exact default
/// preflight can skip its repeated payload scans.
pub(crate) fn verify_with_default_limits_prechecked_and_semantics(
    batch: &TransitionBatch,
    proof: &Proof,
    semantics: ProofSemantics,
) -> Result<()> {
    verify_prechecked_with_semantics(batch, proof, DEFAULT_MAX_VERIFY_AIR_ROW_VALUES, semantics)
}

fn verify_prechecked_with_semantics(
    batch: &TransitionBatch,
    proof: &Proof,
    max_air_row_values: usize,
    semantics: ProofSemantics,
) -> Result<()> {
    validate_canonical_goldilocks_elements(proof)?;
    validate_batch_semantics(batch, semantics)?;
    trace::ensure_trace_schema_limit(batch, max_air_row_values)?;
    verify_after_limits(batch, proof)
}

/// Verify a cryptographic fixture proof without assigning state-transition semantics.
///
/// This escape hatch is available only to unit tests and `dev-tools` builds. It checks the proof
/// protocol and its binding to the supplied batch, but deliberately makes no claim that the batch
/// operations are valid state updates.
/// Applies the default verifier limits. Larger fixtures must select an explicit
/// finite budget with [`verify_raw_statement_with_limits`].
///
/// # Errors
///
/// Returns the same cryptographic, encoding, or verifier-limit errors as [`verify_with_limits`].
#[cfg(any(test, feature = "dev-tools"))]
pub fn verify_raw_statement(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
    verify_raw_statement_with_limits(batch, proof, VerifyLimits::default())
}

/// Verify a raw cryptographic fixture under explicit, finite resource limits.
///
/// Available only to tests and `dev-tools`, this applies every normal resource
/// and cryptographic check without assigning state-transition semantics. The
/// caller must select the fixture budget; the verifier never expands it from
/// proof-controlled lengths. Production verification remains subject to its
/// authenticated semantic profile and independently selected admission limits.
///
/// # Errors
///
/// Returns the same cryptographic, encoding, or verifier-limit errors as
/// [`verify_with_limits`].
#[cfg(any(test, feature = "dev-tools"))]
pub fn verify_raw_statement_with_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    verify_with_limits_raw(batch, proof, limits)
}

fn verify_with_limits_raw(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    enforce_verify_limits(batch, proof, limits)?;
    validate_canonical_goldilocks_elements(proof)?;
    trace::ensure_trace_schema_limit(batch, limits.max_air_row_values)?;
    verify_after_limits(batch, proof)
}

fn validate_canonical_goldilocks_elements(proof: &Proof) -> Result<()> {
    validate_canonical_goldilocks_transcript_scalars(proof)?;
    validate_canonical_goldilocks_queries(proof)?;
    validate_canonical_goldilocks_air_openings(proof)?;
    validate_canonical_goldilocks_fri_openings(proof)
}

fn validate_canonical_goldilocks_transcript_scalars(proof: &Proof) -> Result<()> {
    ensure_canonical_goldilocks(proof.lookup_grand_product, "lookup_grand_product", &[])?;
    ensure_canonical_goldilocks(proof.lookup_challenge, "lookup_challenge", &[])?;
    for (index, &alpha) in proof.alphas.iter().enumerate() {
        ensure_canonical_fp4(alpha, "alphas", &[index])?;
    }
    for (index, &beta) in proof.betas.iter().enumerate() {
        ensure_canonical_fp4(beta, "betas", &[index])?;
    }
    Ok(())
}

fn validate_canonical_goldilocks_queries(proof: &Proof) -> Result<()> {
    for (query_index, query) in proof.queries.iter().enumerate() {
        ensure_canonical_fp4(query.value, "queries.value", &[query_index])?;
        for (value_index, &value) in query.chunk_values.iter().enumerate() {
            ensure_canonical_fp4(value, "queries.chunk_values", &[query_index, value_index])?;
        }
    }
    Ok(())
}

fn validate_canonical_goldilocks_air_openings(proof: &Proof) -> Result<()> {
    for (query_index, opening) in proof.air_openings.iter().enumerate() {
        for (value_index, &value) in opening.current_row.iter().enumerate() {
            ensure_canonical_goldilocks(
                value,
                "air_openings.current_row",
                &[query_index, value_index],
            )?;
        }
        for (value_index, &value) in opening.next_row.iter().enumerate() {
            ensure_canonical_goldilocks(
                value,
                "air_openings.next_row",
                &[query_index, value_index],
            )?;
        }
        ensure_canonical_fp4(
            opening.composition_value,
            "air_openings.composition_value",
            &[query_index],
        )?;
    }
    Ok(())
}

fn validate_canonical_goldilocks_fri_openings(proof: &Proof) -> Result<()> {
    for (query_index, query) in proof.fri_queries.iter().enumerate() {
        for (round_index, round) in query.rounds.iter().enumerate() {
            for (value_index, &value) in round.values.iter().enumerate() {
                ensure_canonical_fp4(
                    value,
                    "fri_queries.rounds.values",
                    &[query_index, round_index, value_index],
                )?;
            }
            ensure_canonical_fp4(
                round.folded_value,
                "fri_queries.rounds.folded_value",
                &[query_index, round_index],
            )?;
        }
        for (value_index, &value) in query.final_values.iter().enumerate() {
            ensure_canonical_fp4(
                value,
                "fri_queries.final_values",
                &[query_index, value_index],
            )?;
        }
    }
    Ok(())
}

fn ensure_canonical_goldilocks(value: u64, context: &'static str, indices: &[usize]) -> Result<()> {
    if value >= GOLDILOCKS_MODULUS {
        return Err(Error::NonCanonicalGoldilocksElement {
            context,
            indices: indices.to_vec(),
        });
    }
    Ok(())
}

fn ensure_canonical_fp4(
    value: GoldilocksFp4V1,
    context: &'static str,
    indices: &[usize],
) -> Result<()> {
    for (coefficient_index, coefficient) in value.coefficients().into_iter().enumerate() {
        let mut nested = indices.to_vec();
        nested.push(coefficient_index);
        ensure_canonical_goldilocks(coefficient, context, &nested)?;
    }
    Ok(())
}

fn verify_wire_merkle_path(
    cache: &mut backend::MerkleNodeCache,
    role: MerkleTreeRoleV1,
    root: NativeGoldilocksDigest384V1,
    leaf: NativeGoldilocksDigest384V1,
    leaf_index: usize,
    path: &[GoldilocksDigest384V1],
) -> Result<bool> {
    let path = path
        .iter()
        .copied()
        .map(GoldilocksDigest384V1::as_fastpq)
        .collect::<Vec<_>>();
    cache.verify_path(role, root, leaf, leaf_index, &path)
}

fn ensure_lde_air_row_binding(
    query_pos: usize,
    query_value: GoldilocksFp4V1,
    row: &[u64],
    column_mix: &[GoldilocksFp4V1],
) -> Result<()> {
    if row.len() != column_mix.len() {
        return Err(Error::AirOpeningMismatch { index: query_pos });
    }
    let expected = row
        .iter()
        .zip(column_mix)
        .fold(GoldilocksFp4V1::ZERO, |sum, (&value, &mix)| {
            sum.add(mix.mul_base(value))
        });
    if expected != query_value {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn verify_after_limits(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
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
    let canonical = batch.canonicalized();
    let expected_ordering = ordering::ordering_hash(&canonical)?;
    let expected_public_io = build_public_io(&canonical, expected_ordering);
    ensure_public_io_matches(&expected_public_io, &proof.public_io)?;
    let lde_domain_size = usize::try_from(proof.lde_domain_size)
        .map_err(|_| Error::QueryIndexOverflow { index: usize::MAX })?;
    if lde_domain_size == 0 {
        return Err(Error::QueryIndexOutOfRange { index: 0, len: 0 });
    }
    let expected_derived = backend::derive_batch_commitments(
        &params,
        &canonical,
        &expected_public_io,
        proof.protocol_version,
    )?;
    if proof.trace_commitment != expected_derived.trace_commitment {
        return Err(Error::CommitmentMismatch);
    }
    if proof.trace_root.as_fastpq() != expected_derived.trace_root {
        return Err(Error::TraceRootMismatch);
    }
    if proof.lde_root.as_fastpq() != expected_derived.lde_root
        || proof.lde_domain_size != expected_derived.lde_domain_size
    {
        return Err(Error::LdeRootMismatch);
    }
    if proof.lookup_grand_product != expected_derived.lookup_grand_product {
        return Err(Error::LookupGrandProductMismatch);
    }
    if proof.lookup_challenge != expected_derived.lookup_challenge {
        return Err(Error::LookupChallengeMismatch);
    }
    if proof.air_trace_root.as_fastpq() != expected_derived.air_trace_root {
        return Err(Error::AirTraceRootMismatch);
    }
    if proof.air_composition_root.as_fastpq() != expected_derived.air_composition_root {
        return Err(Error::AirCompositionRootMismatch);
    }
    let trace_root = proof.trace_root.as_fastpq();
    let lde_root = proof.lde_root.as_fastpq();
    let air_trace_root = proof.air_trace_root.as_fastpq();
    let air_composition_root = proof.air_composition_root.as_fastpq();
    let mut transcript = backend::Transcript::initialise(
        &proof.public_io,
        &proof.parameter,
        proof.protocol_version,
        TRANSCRIPT_TAG_INIT,
    )?;
    let column_names = trace::column_names_for_batch(batch)?;
    transcript.append_trace_oracles(
        trace_root,
        air_trace_root,
        proof.lde_domain_size,
        column_names.len(),
    )?;
    let column_mix = (0..column_names.len())
        .map(|index| {
            transcript.challenge_extension(&format!("{TRANSCRIPT_TAG_COLUMN_MIX_PREFIX}:{index}"))
        })
        .collect::<Vec<_>>();
    transcript.append_message(
        TRANSCRIPT_TAG_ROOTS,
        &[lde_root.to_le_bytes(), trace_root.to_le_bytes()].concat(),
    );
    let expected_lookup_challenge = transcript.challenge_field(TRANSCRIPT_TAG_GAMMA);
    if proof.lookup_challenge != expected_lookup_challenge {
        return Err(Error::LookupChallengeMismatch);
    }
    let expected_alpha_count = backend::air_composition_alpha_count(&column_names);
    if proof.alphas.len() != expected_alpha_count {
        return Err(Error::AirChallengeCountMismatch {
            expected: expected_alpha_count,
            actual: proof.alphas.len(),
        });
    }
    for (idx, &alpha) in proof.alphas.iter().enumerate() {
        let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
        let expected = transcript.challenge_extension(&tag);
        if expected != alpha {
            return Err(Error::AirChallengeMismatch { index: idx });
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
    let joint_fri =
        backend::JointFriBatch::from_transcript(&params, lde_domain_size, &mut transcript)?;
    let quotient_domain = backend::AirQuotientDomain::new(&params, lde_domain_size)?;
    let fri_layer_lengths =
        expected_fri_layer_lengths(lde_domain_size, params.fri.arity, params.fri.max_reductions)?;
    let terminal_degree_bound = fri_terminal_degree_bound(
        lde_domain_size,
        params.fri.blowup_factor,
        params.fri.arity,
        &fri_layer_lengths,
    )?;
    if proof.fri_layers.len() != fri_layer_lengths.len() {
        return Err(Error::FriLayerLengthMismatch {
            expected: fri_layer_lengths.len(),
            actual: proof.fri_layers.len(),
        });
    }
    let round_count = proof.fri_layers.len().saturating_sub(1);
    let mut expected_betas = Vec::with_capacity(round_count);
    for (round, root) in proof.fri_layers.iter().take(round_count).enumerate() {
        let root = root.as_fastpq();
        transcript.append_fri_layer(round, root);
        expected_betas.push(transcript.challenge_beta(round));
    }
    let final_root = proof
        .fri_layers
        .last()
        .expect("non-empty FRI layer commitments")
        .as_fastpq();
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
    )?;
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
    let lde_chunk_size = backend::lde_chunk_size(params.fri.arity)?;
    let lde_leaf_count = leaf_count_for_values(lde_domain_size, lde_chunk_size)?;
    let lde_path_len = merkle_path_len_for_leaf_count(lde_leaf_count)?;
    let air_path_len = merkle_path_len_for_leaf_count(lde_domain_size)?;
    let fri_domain = backend::FriDomain::from_lde_parameters(
        params.lde_root,
        params.lde_log_size,
        lde_domain_size,
        params.omega_coset,
    )?;
    // Share only complete typed node computations across this bounded verification.
    let mut merkle_cache = backend::MerkleNodeCache::default();
    let mut lde_leaf_cache = lde_leaf_cache::LdeLeafCache::default();
    let mut air_leaf_cache = lde_leaf_cache::AirTraceLeafCache::default();
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
        if query.merkle_path.len() != lde_path_len {
            return Err(Error::QueryMerklePathMismatch { index: pos });
        }
        let leaf = lde_leaf_cache.hash(leaf_index, &query.chunk_values)?;
        if !verify_wire_merkle_path(
            &mut merkle_cache,
            MerkleTreeRoleV1::Lde,
            lde_root,
            leaf,
            leaf_index,
            &query.merkle_path,
        )? {
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
        let current_leaf = air_leaf_cache.hash(expected_idx, &air_opening.current_row)?;
        if !verify_wire_merkle_path(
            &mut merkle_cache,
            MerkleTreeRoleV1::AirTrace,
            air_trace_root,
            current_leaf,
            expected_idx,
            &air_opening.current_row_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        // Both values are authenticated independently above. Bind the mixed
        // LDE oracle to this AIR row using the post-trace-commitment challenges.
        // Batch-derived commitment replay remains required for every relation
        // that the current AIR and opening schema do not express.
        ensure_lde_air_row_binding(pos, query.value, &air_opening.current_row, &column_mix)?;
        let next_idx = expected_idx
            .checked_add(next_step)
            .ok_or(Error::QueryIndexOverflow {
                index: expected_idx,
            })?
            % lde_domain_size;
        let next_leaf = air_leaf_cache.hash(next_idx, &air_opening.next_row)?;
        if !verify_wire_merkle_path(
            &mut merkle_cache,
            MerkleTreeRoleV1::AirTrace,
            air_trace_root,
            next_leaf,
            next_idx,
            &air_opening.next_row_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        let expected_composition = backend::air_quotient_value_for_rows(
            &column_names,
            &air_opening.current_row,
            &air_opening.next_row,
            &proof.alphas,
            quotient_domain.weights_at(expected_idx)?,
        )?;
        if expected_composition != air_opening.composition_value {
            return Err(Error::AirConstraintMismatch { index: pos });
        }
        let composition_leaf =
            backend::hash_air_composition_leaf(expected_idx, air_opening.composition_value)?;
        if !verify_wire_merkle_path(
            &mut merkle_cache,
            MerkleTreeRoleV1::AirComposition,
            air_composition_root,
            composition_leaf,
            expected_idx,
            &air_opening.composition_path,
        )? {
            return Err(Error::AirMerklePathMismatch { index: pos });
        }
        verify_fri_query_chain(
            &mut merkle_cache,
            &proof.fri_queries[pos],
            FriQueryVerification {
                query_pos: pos,
                initial_index: expected_idx,
                initial_value: joint_fri.value_at(
                    expected_idx,
                    air_opening.composition_value,
                    query.value,
                )?,
                fri_layers: &proof.fri_layers,
                betas: &proof.betas,
                fri_layer_lengths: &fri_layer_lengths,
                terminal_degree_bound,
                arity: params.fri.arity,
                domain: fri_domain,
            },
        )?;
    }
    Ok(())
}
#[allow(clippy::too_many_lines)]
fn enforce_verify_limits(
    batch: &TransitionBatch,
    proof: &Proof,
    limits: VerifyLimits,
) -> Result<()> {
    enforce_transition_limit(batch, limits)?;
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
    for fri_query in &proof.fri_queries {
        if fri_query.rounds.len() > limits.max_fri_layers {
            return Err(Error::VerifierLimitExceeded {
                limit: "max_fri_layers",
                actual: fri_query.rounds.len(),
                max: limits.max_fri_layers,
            });
        }
    }
    // Only scan variable-size payloads after their enclosing collection counts are bounded.
    enforce_batch_size_limit(batch, limits)?;
    let proof_bytes = proof_size_hint(proof);
    if proof_bytes > limits.max_proof_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_proof_bytes",
            actual: proof_bytes,
            max: limits.max_proof_bytes,
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
fn enforce_transition_limit(batch: &TransitionBatch, limits: VerifyLimits) -> Result<()> {
    if batch.transitions.len() > limits.max_transitions {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_transitions",
            actual: batch.transitions.len(),
            max: limits.max_transitions,
        });
    }
    Ok(())
}
fn enforce_batch_size_limit(batch: &TransitionBatch, limits: VerifyLimits) -> Result<()> {
    let batch_bytes = batch_size_hint(batch);
    if batch_bytes > limits.max_batch_bytes {
        return Err(Error::VerifierLimitExceeded {
            limit: "max_batch_bytes",
            actual: batch_bytes,
            max: limits.max_batch_bytes,
        });
    }
    Ok(())
}
#[derive(Clone, Copy)]
struct FriQueryVerification<'a> {
    query_pos: usize,
    initial_index: usize,
    initial_value: GoldilocksFp4V1,
    fri_layers: &'a [GoldilocksDigest384V1],
    betas: &'a [GoldilocksFp4V1],
    fri_layer_lengths: &'a [usize],
    terminal_degree_bound: usize,
    arity: u32,
    domain: backend::FriDomain,
}

#[derive(Clone, Copy)]
struct FriFinalVerification<'a> {
    query_pos: usize,
    index: usize,
    value: GoldilocksFp4V1,
    root: &'a GoldilocksDigest384V1,
    length: usize,
    round: usize,
}

/// Fixed FRI geometry shared with the bounded compact candidate verifier.
/// Standalone prototype query verification remains test-only.
pub(crate) mod compact_fri_support {
    use super::*;

    /// Caller-fixed FRI geometry and transcript challenges for one prototype opening.
    #[cfg(test)]
    #[derive(Clone, Copy)]
    pub(crate) struct Context<'a> {
        pub(crate) query_pos: usize,
        pub(crate) initial_index: usize,
        pub(crate) initial_value: GoldilocksFp4V1,
        pub(crate) fri_layers: &'a [GoldilocksDigest384V1],
        pub(crate) betas: &'a [GoldilocksFp4V1],
        pub(crate) fri_layer_lengths: &'a [usize],
        pub(crate) terminal_degree_bound: usize,
        pub(crate) arity: u32,
        pub(crate) domain: backend::FriDomain,
    }

    /// Apply the existing exact-index/path/fold/terminal checks without replaying a trace.
    #[cfg(test)]
    pub(crate) fn verify_query(
        merkle_cache: &mut backend::MerkleNodeCache,
        opening: &FriQueryOpening,
        context: Context<'_>,
    ) -> Result<()> {
        super::verify_fri_query_chain(
            merkle_cache,
            opening,
            FriQueryVerification {
                query_pos: context.query_pos,
                initial_index: context.initial_index,
                initial_value: context.initial_value,
                fri_layers: context.fri_layers,
                betas: context.betas,
                fri_layer_lengths: context.fri_layer_lengths,
                terminal_degree_bound: context.terminal_degree_bound,
                arity: context.arity,
                domain: context.domain,
            },
        )
    }

    /// Derive the existing bounded binary layer schedule from fixed geometry.
    pub(crate) fn layer_lengths(
        domain_size: usize,
        arity: u32,
        max_reductions: u32,
    ) -> Result<Vec<usize>> {
        super::expected_fri_layer_lengths(domain_size, arity, max_reductions)
    }

    /// Derive the existing exact terminal degree bound for the joint quotient proof.
    pub(crate) fn terminal_degree_bound(
        domain_size: usize,
        blowup_factor: u32,
        arity: u32,
        layer_lengths: &[usize],
    ) -> Result<usize> {
        super::fri_terminal_degree_bound(domain_size, blowup_factor, arity, layer_lengths)
    }
}

fn verify_fri_query_chain(
    merkle_cache: &mut backend::MerkleNodeCache,
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
        terminal_degree_bound,
        arity,
        mut domain,
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
    ensure_canonical_fp4(initial_value, "fri_initial_value", &[query_pos])?;
    let mut value = initial_value;
    for (round, opening) in fri_query.rounds.iter().enumerate() {
        let round_len = fri_layer_lengths[round];
        if index >= round_len {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let round_arity = backend::fri_round_arity(round_len, arity)?;
        let output_len = round_len / round_arity;
        let leaf_index = index % output_len;
        if usize::try_from(opening.round).ok() != Some(round)
            || usize::try_from(opening.index).ok() != Some(index)
            || opening.values.len() != round_arity
        {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let position = index / output_len;
        if opening.values.get(position).copied() != Some(value) {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        let round_leaf_count = output_len;
        let round_path_len = merkle_path_len_for_leaf_count(round_leaf_count)?;
        if opening.merkle_path.len() != round_path_len {
            return Err(Error::QueryMerklePathMismatch { index: query_pos });
        }
        let root = fri_layers[round].as_fastpq();
        let leaf = backend::hash_fri_chunk(round, leaf_index, &opening.values)?;
        if !verify_wire_merkle_path(
            merkle_cache,
            MerkleTreeRoleV1::Fri(
                u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
            ),
            root,
            leaf,
            leaf_index,
            &opening.merkle_path,
        )? {
            return Err(Error::QueryMerklePathMismatch { index: query_pos });
        }
        let folded = fold_fri_values(
            &opening.values,
            betas[round],
            domain.point(leaf_index),
            domain.coset_generator(output_len),
        )?;
        if folded != opening.folded_value {
            return Err(Error::QueryMismatch { index: query_pos });
        }
        index = leaf_index;
        value = folded;
        domain = domain.folded(round_arity);
    }
    let final_len = *fri_layer_lengths
        .last()
        .expect("FRI layer lengths checked non-empty");
    let final_round = betas.len();
    verify_fri_final_opening(
        merkle_cache,
        fri_query,
        FriFinalVerification {
            query_pos,
            index,
            value,
            root: &fri_layers[final_round],
            length: final_len,
            round: final_round,
        },
    )?;
    if !domain.evaluations_have_degree_below(&fri_query.final_values, terminal_degree_bound)? {
        return Err(Error::FriTerminalDegreeMismatch {
            degree_bound: terminal_degree_bound,
        });
    }
    Ok(())
}

fn verify_fri_final_opening(
    merkle_cache: &mut backend::MerkleNodeCache,
    fri_query: &FriQueryOpening,
    context: FriFinalVerification<'_>,
) -> Result<()> {
    let FriFinalVerification {
        query_pos,
        index,
        value,
        root,
        length,
        round,
    } = context;
    if usize::try_from(fri_query.final_index).ok() != Some(index) || index >= length {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let terminal_size = fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize;
    if !length.is_power_of_two() || length > terminal_size {
        return Err(Error::FriDomainSize {
            length,
            arity: terminal_size,
        });
    }
    // Authenticate every terminal evaluation in its natural domain order. A
    // single binary coset cannot support the complete terminal degree check.
    let final_leaf_index = 0;
    if fri_query.final_values.len() != length
        || fri_query.final_values.get(index).copied() != Some(value)
    {
        return Err(Error::QueryMismatch { index: query_pos });
    }
    let final_path_len = merkle_path_len_for_leaf_count(1)?;
    if fri_query.final_merkle_path.len() != final_path_len {
        return Err(Error::QueryMerklePathMismatch { index: query_pos });
    }
    let final_root = root.as_fastpq();
    let final_leaf = backend::hash_fri_chunk(round, final_leaf_index, &fri_query.final_values)?;
    if !verify_wire_merkle_path(
        merkle_cache,
        MerkleTreeRoleV1::Fri(
            u32::try_from(round).map_err(|_| Error::QueryIndexOverflow { index: round })?,
        ),
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
    if arity != 2 {
        return Err(Error::FriArity(arity as u32));
    }
    if !domain_size.is_power_of_two() {
        return Err(Error::FriDomainSize {
            length: domain_size,
            arity,
        });
    }
    let terminal_size = fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize;
    let max_rounds = usize::try_from(max_reductions).expect("FRI reduction bound fits usize");
    let mut current = domain_size;
    let mut rounds = 0usize;
    let mut lengths = Vec::new();
    while current > terminal_size && rounds < max_rounds {
        lengths.push(current);
        let round_arity = backend::fri_round_arity(current, arity)?;
        current /= round_arity;
        rounds += 1;
    }
    if current > terminal_size {
        return Err(Error::FriReductionLimit {
            max_reductions,
            remaining: current,
            arity,
        });
    }
    lengths.push(current);
    Ok(lengths)
}

fn fri_terminal_degree_bound(
    domain_size: usize,
    blowup_factor: u32,
    arity: u32,
    layer_lengths: &[usize],
) -> Result<usize> {
    let blowup = usize::try_from(blowup_factor).expect("FRI blowup factor fits usize");
    if blowup == 0 || !domain_size.is_multiple_of(blowup) || layer_lengths.is_empty() {
        return Err(Error::FriDomainSize {
            length: domain_size,
            arity: blowup,
        });
    }
    let configured_arity = usize::try_from(arity).map_err(|_| Error::FriArity(arity))?;
    let trace_len = domain_size / blowup;
    if layer_lengths[0] != domain_size {
        return Err(Error::FriLayerLengthMismatch {
            expected: domain_size,
            actual: layer_lengths[0],
        });
    }
    // Retain conservative quotient degree headroom below 2*N; the currently
    // implemented quadratic row/transition relations have quotients below N.
    // Binary folding must preserve this exclusive bound exactly. Rounding it
    // upward at degree one would silently admit a larger original polynomial.
    // Full batch-derived commitment reconstruction remains mandatory until all
    // state-transition relations and public boundaries are constrained.
    let mut degree_bound = trace_len
        .checked_mul(backend::AIR_QUOTIENT_DEGREE_EXPANSION_V1)
        .ok_or(Error::TraceLengthOverflow { rows: trace_len })?;
    for lengths in layer_lengths.windows(2) {
        let current = lengths[0];
        let next = lengths[1];
        let round_arity = backend::fri_round_arity(current, configured_arity)?;
        if current / round_arity != next {
            return Err(Error::FriLayerLengthMismatch {
                expected: current / round_arity,
                actual: next,
            });
        }
        if !degree_bound.is_multiple_of(round_arity) {
            return Err(Error::FriTerminalDegreeBound {
                degree_bound,
                domain_len: current,
            });
        }
        degree_bound /= round_arity;
    }
    let terminal_len = *layer_lengths
        .last()
        .expect("non-empty FRI layer schedule checked above");
    if degree_bound == 0 || degree_bound >= terminal_len {
        return Err(Error::FriTerminalDegreeBound {
            degree_bound,
            domain_len: terminal_len,
        });
    }
    Ok(degree_bound)
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
fn fold_fri_values(
    values: &[GoldilocksFp4V1],
    challenge: GoldilocksFp4V1,
    x: u64,
    coset_generator: u64,
) -> Result<GoldilocksFp4V1> {
    backend::fold_fri_coset(values, challenge, x, coset_generator)
}
fn batch_size_hint(batch: &TransitionBatch) -> usize {
    let mut total = batch.parameter.len().saturating_add(16 + 8 + 32 * 4); // fixed PublicInputs payload
    for transition in &batch.transitions {
        total = total
            .saturating_add(transition.key.len())
            .saturating_add(transition.pre_value.len())
            .saturating_add(transition.post_value.len())
            .saturating_add(4); // operation discriminant
        if matches!(
            &transition.operation,
            crate::OperationKind::RoleGrant { .. } | crate::OperationKind::RoleRevoke { .. }
        ) {
            total = total.saturating_add(32 + 32 + 8);
        }
    }
    for (key, value) in &batch.metadata {
        total = total.saturating_add(key.len()).saturating_add(value.len());
    }
    total
}
fn proof_size_hint(proof: &Proof) -> usize {
    let mut total = 0usize;
    total = total.saturating_add(2); // protocol_version
    total = total.saturating_add(proof.parameter.len());
    total = total.saturating_add(GoldilocksDigest384V1::BYTES); // trace_commitment
    total = total.saturating_add(public_io_size_hint(&proof.public_io));
    total = total.saturating_add(GoldilocksDigest384V1::BYTES * 4); // proof roots
    total = total.saturating_add(4); // lde_domain_size
    total = total.saturating_add(16); // lookup product and challenge
    total = total.saturating_add(proof.alphas.len().saturating_mul(32));
    total = total.saturating_add(proof.betas.len().saturating_mul(32));
    total = total.saturating_add(
        proof
            .fri_layers
            .len()
            .saturating_mul(GoldilocksDigest384V1::BYTES),
    );
    for query in &proof.queries {
        total = total.saturating_add(4); // index
        total = total.saturating_add(32); // value
        total = total.saturating_add(query.chunk_values.len().saturating_mul(32));
        total = total.saturating_add(
            query
                .merkle_path
                .len()
                .saturating_mul(GoldilocksDigest384V1::BYTES),
        );
    }
    for opening in &proof.air_openings {
        total = total.saturating_add(4); // index
        total = total.saturating_add(opening.current_row.len().saturating_mul(8));
        total = total.saturating_add(opening.next_row.len().saturating_mul(8));
        total = total.saturating_add(
            opening
                .current_row_path
                .len()
                .saturating_mul(GoldilocksDigest384V1::BYTES),
        );
        total = total.saturating_add(
            opening
                .next_row_path
                .len()
                .saturating_mul(GoldilocksDigest384V1::BYTES),
        );
        total = total.saturating_add(32); // composition_value
        total = total.saturating_add(
            opening
                .composition_path
                .len()
                .saturating_mul(GoldilocksDigest384V1::BYTES),
        );
    }
    for query in &proof.fri_queries {
        total = total.saturating_add(4); // initial_index
        for round in &query.rounds {
            total = total.saturating_add(4); // round
            total = total.saturating_add(4); // index
            total = total.saturating_add(round.values.len().saturating_mul(32));
            total = total.saturating_add(32); // folded_value
            total = total.saturating_add(
                round
                    .merkle_path
                    .len()
                    .saturating_mul(GoldilocksDigest384V1::BYTES),
            );
        }
        total = total.saturating_add(4); // final_index
        total = total.saturating_add(query.final_values.len().saturating_mul(32));
        total = total.saturating_add(
            query
                .final_merkle_path
                .len()
                .saturating_mul(GoldilocksDigest384V1::BYTES),
        );
    }
    total
}
fn public_io_size_hint(public_io: &PublicIO) -> usize {
    let _ = public_io;
    16 + 8 + 32 * 5
}
fn materialise_proof(public_io: PublicIO, artifact: BackendArtifact) -> Result<Proof> {
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
        .map(GoldilocksDigest384V1::from)
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
                merkle_path: merkle_path
                    .into_iter()
                    .map(GoldilocksDigest384V1::from)
                    .collect(),
            },
        )
        .collect();
    Ok(Proof {
        protocol_version: PROTOCOL_VERSION,
        parameter: artifact.parameter,
        trace_commitment: artifact.trace_commitment,
        public_io,
        trace_root: artifact.trace_root.into(),
        air_trace_root: artifact.air_trace_root.into(),
        air_composition_root: artifact.air_composition_root.into(),
        lde_root: artifact.lde_root.into(),
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
fn build_public_io(batch: &TransitionBatch, ordering_hash: Hash) -> PublicIO {
    let inputs = &batch.public_inputs;
    PublicIO {
        dsid: inputs.dsid,
        slot: inputs.slot,
        old_root: inputs.old_root,
        new_root: inputs.new_root,
        perm_root: inputs.perm_root,
        tx_set_hash: inputs.tx_set_hash,
        ordering_hash: ordering_hash.into(),
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
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OperationKind, PublicInputs, StateTransition};

    fn fp4(value: u64) -> GoldilocksFp4V1 {
        GoldilocksFp4V1::from_base(value).expect("canonical Goldilocks test value")
    }

    fn fp4_values(values: &[u64]) -> Vec<GoldilocksFp4V1> {
        values.iter().copied().map(fp4).collect()
    }

    fn digest384(value: u64) -> NativeGoldilocksDigest384V1 {
        NativeGoldilocksDigest384V1::new([value; 6]).expect("canonical Digest384 test value")
    }

    fn wire_digest384(value: u64) -> GoldilocksDigest384V1 {
        digest384(value).into()
    }

    /// Cryptographic-layer tests intentionally bypass production statement
    /// semantics; tests of the public gate call `super::verify` explicitly.
    fn verify(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
        super::verify_raw_statement(batch, proof)
    }

    fn make_digest384_noncanonical(bytes: &mut [u8; GoldilocksDigest384V1::BYTES], lane: usize) {
        let offset = lane * 8;
        bytes[offset..offset + 8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
    }

    fn noncanonical_digest384_bytes() -> [u8; GoldilocksDigest384V1::BYTES] {
        let mut bytes = wire_digest384(0).to_le_bytes();
        make_digest384_noncanonical(&mut bytes, 0);
        bytes
    }

    fn add_mod(a: u64, b: u64) -> u64 {
        let sum = u128::from(a) + u128::from(b);
        u64::try_from(sum % u128::from(GOLDILOCKS_MODULUS)).expect("modulus reduction fits in u64")
    }

    fn mul_mod(a: u64, b: u64) -> u64 {
        let product = u128::from(a) * u128::from(b);
        u64::try_from(product % u128::from(GOLDILOCKS_MODULUS))
            .expect("modulus reduction fits in u64")
    }

    fn verify_with_limits(
        batch: &TransitionBatch,
        proof: &Proof,
        limits: VerifyLimits,
    ) -> Result<()> {
        verify_with_limits_raw(batch, proof, limits)
    }
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
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.push(StateTransition::new(
            b"asset/xor/alice".to_vec(),
            1_u64.to_le_bytes().to_vec(),
            2_u64.to_le_bytes().to_vec(),
            OperationKind::MetaSet,
        ));
        batch.push(StateTransition::new(
            b"asset/xor/bob".to_vec(),
            10_u64.to_le_bytes().to_vec(),
            11_u64.to_le_bytes().to_vec(),
            OperationKind::MetaSet,
        ));
        batch.sort();
        annotate_batch(&mut batch);
        batch
    }
    fn sample_batch_with_size(rows: usize) -> TransitionBatch {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        for idx in 0..rows {
            let key = format!("asset/xor/account-{idx:04}").into_bytes();
            let idx_u64 = u64::try_from(idx).expect("sample row index fits u64");
            let pre = idx_u64.to_le_bytes().to_vec();
            let op = OperationKind::MetaSet;
            let post_value = idx_u64.wrapping_add(1);
            let post = post_value.to_le_bytes().to_vec();
            batch.push(StateTransition::new(key, pre, post, op));
        }
        batch.sort();
        annotate_batch(&mut batch);
        batch
    }
    fn sample_proof_with_size(rows: usize) -> (TransitionBatch, Proof) {
        // Keep ordinary cryptographic-negative fixtures within every default
        // limit, so preflight does not mask the intended rejection. The common
        // eight-row baseline is checked with the unchanged default byte ceiling.
        // Each negative test mutates its own clone. Build and verify the same
        // deterministic baseline only once, rather than repeating the expensive
        // full prover/replay pipeline for every unrelated preflight mutation.
        static EIGHT_ROWS: std::sync::OnceLock<(TransitionBatch, Proof)> =
            std::sync::OnceLock::new();
        static SIXTEEN_ROWS: std::sync::OnceLock<(TransitionBatch, Proof)> =
            std::sync::OnceLock::new();
        let fixture = match rows {
            8 => &EIGHT_ROWS,
            16 => &SIXTEEN_ROWS,
            _ => return sample_proof_with_size_and_limits(rows, VerifyLimits::default()),
        };
        fixture
            .get_or_init(|| sample_proof_with_size_and_limits(rows, VerifyLimits::default()))
            .clone()
    }
    fn sample_proof_with_size_and_limits(
        rows: usize,
        limits: VerifyLimits,
    ) -> (TransitionBatch, Proof) {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch_with_size(rows);
        let ordering = ordering::ordering_hash(&batch).unwrap();
        let public_io = build_public_io(&batch, ordering);
        let artifact = prover
            .backend
            .prove(&batch, &public_io, PROTOCOL_VERSION)
            .unwrap();
        let proof = materialise_proof(public_io, artifact).unwrap();
        verify_with_limits(&batch, &proof, limits).unwrap();
        (batch, proof)
    }
    fn single_fri_leaf_root_and_path(
        round: usize,
        leaf_index: usize,
        values: &[GoldilocksFp4V1],
    ) -> (GoldilocksDigest384V1, Vec<GoldilocksDigest384V1>) {
        let leaf = backend::hash_fri_chunk(round, leaf_index, values).unwrap();
        let role = MerkleTreeRoleV1::Fri(u32::try_from(round).expect("round fits u32"));
        (
            backend::merkle_root_for_role(&[leaf], role).unwrap().into(),
            vec![leaf.into()],
        )
    }
    fn two_fri_leaf_root_and_path(
        round: usize,
        leaf_index: usize,
        values: &[GoldilocksFp4V1],
        sibling_values: &[GoldilocksFp4V1],
    ) -> (GoldilocksDigest384V1, Vec<GoldilocksDigest384V1>) {
        let leaf = backend::hash_fri_chunk(round, leaf_index, values).unwrap();
        let sibling_index = if leaf_index == 0 { 1 } else { leaf_index - 1 };
        let sibling = backend::hash_fri_chunk(round, sibling_index, sibling_values).unwrap();
        let leaves = if leaf_index == 0 {
            vec![leaf, sibling]
        } else {
            vec![sibling, leaf]
        };
        let role = MerkleTreeRoleV1::Fri(u32::try_from(round).expect("round fits u32"));
        (
            backend::merkle_root_for_role(&leaves, role).unwrap().into(),
            vec![sibling.into()],
        )
    }
    fn fold_fri_group_for_test(
        values: &[GoldilocksFp4V1],
        beta: GoldilocksFp4V1,
        layer_len: usize,
        leaf_index: usize,
    ) -> GoldilocksFp4V1 {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let domain = backend::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            layer_len,
            params.omega_coset,
        )
        .expect("test FRI domain");
        let output_len = layer_len / values.len();
        fold_fri_values(
            values,
            beta,
            domain.point(leaf_index),
            domain.coset_generator(output_len),
        )
        .expect("test FRI fold")
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
    fn assert_noncanonical_goldilocks_rejected(
        proof: &Proof,
        expected_context: &'static str,
        expected_indices: &[usize],
        mutate: impl FnOnce(&mut Proof),
    ) {
        let mut tampered = proof.clone();
        mutate(&mut tampered);
        let err = validate_canonical_goldilocks_elements(&tampered).unwrap_err();
        match err {
            Error::NonCanonicalGoldilocksElement { context, indices } => {
                assert_eq!(context, expected_context);
                assert_eq!(indices, expected_indices);
            }
            other => panic!("unexpected verifier error: {other:?}"),
        }
    }
    fn target_public_io_for(batch: &TransitionBatch) -> PublicIO {
        let ordering = ordering::ordering_hash(batch).unwrap();
        build_public_io(batch, ordering)
    }
    fn proof_over_source_trace_with_target_statement(
        prover: &Prover,
        source: &TransitionBatch,
        target: &TransitionBatch,
    ) -> Proof {
        let params = find_by_name(prover.backend.parameter_name())
            .copied()
            .expect("prover uses a canonical parameter set");
        let target_commitment = trace_commitment(&params, target).unwrap();
        let target_public_io = target_public_io_for(target);
        let mut artifact = prover
            .backend
            .prove(source, &target_public_io, PROTOCOL_VERSION)
            .unwrap();
        artifact.trace_commitment = target_commitment;
        materialise_proof(target_public_io, artifact).unwrap()
    }
    fn proof_over_source_lde_with_target_trace_root(
        prover: &Prover,
        source: &TransitionBatch,
        target: &TransitionBatch,
    ) -> Proof {
        let params = find_by_name(prover.backend.parameter_name())
            .copied()
            .expect("prover uses a canonical parameter set");
        let target_commitment = trace_commitment(&params, target).unwrap();
        let target_public_io = target_public_io_for(target);
        let target_roots =
            backend::derive_batch_commitments(&params, target, &target_public_io, PROTOCOL_VERSION)
                .unwrap();
        let mut artifact = prover
            .backend
            .prove_with_transcript_trace_root(
                source,
                &target_public_io,
                PROTOCOL_VERSION,
                target_roots.trace_root,
            )
            .unwrap();
        artifact.trace_commitment = target_commitment;
        materialise_proof(target_public_io, artifact).unwrap()
    }
    fn graft_artifact_commitments(target: &mut Proof, donor: &Proof) {
        target.trace_root = donor.trace_root;
        target.lde_root = donor.lde_root;
        target.air_trace_root = donor.air_trace_root;
        target.air_composition_root = donor.air_composition_root;
        target.lde_domain_size = donor.lde_domain_size;
        target.lookup_grand_product = donor.lookup_grand_product;
        target.lookup_challenge = donor.lookup_challenge;
        target.alphas = donor.alphas.clone();
        target.betas = donor.betas.clone();
        target.fri_layers = donor.fri_layers.clone();
    }
    fn verify_fri_query_chain_for_test(
        initial_index: usize,
        initial_value: u64,
        fri_query: &FriQueryOpening,
        fri_layers: &[GoldilocksDigest384V1],
        betas: &[GoldilocksFp4V1],
        fri_layer_lengths: &[usize],
        arity: u32,
    ) -> Result<()> {
        verify_fri_query_chain_with_terminal_bound_for_test(
            initial_index,
            initial_value,
            fri_query,
            fri_layers,
            betas,
            fri_layer_lengths,
            fri_layer_lengths.last().copied().unwrap_or(1),
            arity,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn verify_fri_query_chain_with_terminal_bound_for_test(
        initial_index: usize,
        initial_value: u64,
        fri_query: &FriQueryOpening,
        fri_layers: &[GoldilocksDigest384V1],
        betas: &[GoldilocksFp4V1],
        fri_layer_lengths: &[usize],
        terminal_degree_bound: usize,
        arity: u32,
    ) -> Result<()> {
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let domain_size = fri_layer_lengths.first().copied().unwrap_or(1);
        let domain = backend::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            domain_size,
            params.omega_coset,
        )?;
        verify_fri_query_chain(
            &mut backend::MerkleNodeCache::default(),
            fri_query,
            FriQueryVerification {
                query_pos: 0,
                initial_index,
                initial_value: fp4(initial_value),
                fri_layers,
                betas,
                fri_layer_lengths,
                terminal_degree_bound,
                arity,
                domain,
            },
        )
    }
    fn sample_backend_artifact() -> BackendArtifact {
        BackendArtifact {
            parameter: "fastpq-state-transition-stark-v1".to_owned(),
            trace_commitment: digest384(10).into(),
            trace_root: digest384(11),
            air_trace_root: digest384(12),
            air_composition_root: digest384(13),
            lde_root: digest384(14),
            lde_domain_size: 1,
            lookup_grand_product: 15,
            lookup_challenge: 16,
            alphas: fp4_values(&[17, 18]),
            fri_layers: vec![digest384(19)],
            fri_betas: Vec::new(),
            query_openings: vec![(0, fp4(18))],
            query_chunks: vec![vec![fp4(18)]],
            query_paths: vec![Vec::new()],
            air_openings: vec![AirConstraintOpening {
                index: 0,
                current_row: Vec::new(),
                next_row: Vec::new(),
                current_row_path: Vec::new(),
                next_row_path: Vec::new(),
                composition_value: fp4(19),
                composition_path: Vec::new(),
            }],
            fri_query_openings: vec![FriQueryOpening {
                initial_index: 0,
                rounds: Vec::new(),
                final_index: 0,
                final_values: vec![fp4(21)],
                final_merkle_path: Vec::new(),
            }],
        }
    }
    fn materialise_sample_artifact(artifact: BackendArtifact) -> Result<Proof> {
        materialise_proof(PublicIO::default(), artifact)
    }
    #[test]
    fn public_io_preserves_explicit_zero_permission_and_tx_set_hash() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.public_inputs.dsid = [0x11; 16];
        batch.public_inputs.slot = 7;
        batch.public_inputs.old_root = [0xAA; 32];
        batch.public_inputs.new_root = [0xBB; 32];
        batch.push(StateTransition::new(
            b"metadata/permission-root-binding".to_vec(),
            b"old".to_vec(),
            b"new".to_vec(),
            OperationKind::MetaSet,
        ));
        batch.sort();
        let ordering = ordering::ordering_hash(&batch).expect("ordering");
        let public_io = build_public_io(&batch, ordering);
        assert_eq!(public_io.perm_root, batch.public_inputs.perm_root);
        assert_eq!(public_io.tx_set_hash, batch.public_inputs.tx_set_hash);
    }
    #[test]
    fn public_io_preserves_explicit_roots_and_hashes_claims() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.public_inputs.dsid = [0x11; 16];
        batch.public_inputs.slot = 99;
        batch.public_inputs.old_root = [0x22; 32];
        batch.public_inputs.new_root = [0x33; 32];
        batch.public_inputs.perm_root = [0x44; 32];
        batch.public_inputs.tx_set_hash = [0x55; 32];
        let ordering = Hash::new(b"explicit-ordering-hash");
        let public_io = build_public_io(&batch, ordering);
        assert_eq!(public_io.dsid, batch.public_inputs.dsid);
        assert_eq!(public_io.slot, batch.public_inputs.slot);
        assert_eq!(public_io.old_root, batch.public_inputs.old_root);
        assert_eq!(public_io.new_root, batch.public_inputs.new_root);
        assert_eq!(public_io.perm_root, batch.public_inputs.perm_root);
        assert_eq!(public_io.tx_set_hash, batch.public_inputs.tx_set_hash);
        let expected_ordering: [u8; Hash::LENGTH] = ordering.into();
        assert_eq!(public_io.ordering_hash, expected_ordering);
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
        ensure_public_io_matches(&expected, &expected).unwrap();
    }
    #[test]
    fn batch_size_hint_counts_transition_and_metadata_payloads() {
        let mut batch = TransitionBatch::new("param", PublicInputs::default());
        batch.push(StateTransition::new(
            b"key".to_vec(),
            vec![1, 2],
            vec![3, 4, 5],
            OperationKind::MetaSet,
        ));
        batch.metadata.insert("meta".to_owned(), vec![9, 8, 7]);
        let expected =
            "param".len() + (16 + 8 + 32 * 4) + "key".len() + 2 + 3 + 4 + "meta".len() + 3;
        assert_eq!(batch_size_hint(&batch), expected);
    }
    #[test]
    fn batch_size_hint_counts_fixed_permission_operation_payloads() {
        let mut batch = TransitionBatch::new("param", PublicInputs::default());
        batch.push(StateTransition::new(
            b"permission/key".to_vec(),
            Vec::new(),
            vec![1],
            OperationKind::RoleGrant {
                role_id: [0x11; 32],
                permission_id: [0x22; 32],
                epoch: 7,
            },
        ));
        let base = "param".len() + (16 + 8 + 32 * 4) + "permission/key".len() + 1 + 4;
        assert_eq!(batch_size_hint(&batch), base + 32 + 32 + 8);
    }
    #[test]
    fn raw_crypto_verifier_accepts_canonical_v1_roundtrip() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let proof = prover.prove_raw_statement(&batch).unwrap();
        verify_raw_statement(&batch, &proof).unwrap();
    }
    #[test]
    fn raw_fixture_verifier_preserves_explicit_admission_limits() {
        let batch = sample_batch();
        let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        let limits = VerifyLimits {
            max_proof_bytes: 0,
            ..VerifyLimits::default()
        };
        assert!(matches!(
            verify_raw_statement_with_limits(&batch, &proof, limits),
            Err(Error::VerifierLimitExceeded {
                limit: "max_proof_bytes",
                max: 0,
                ..
            })
        ));
        assert_eq!(VerifyLimits::default().max_proof_bytes, 512 * 1024);
    }
    #[test]
    fn raw_proof_is_independent_of_ambient_norito_layout() {
        let prover = Prover::canonical_with_execution_mode(
            "fastpq-state-transition-stark-v1",
            ExecutionMode::Cpu,
        )
        .unwrap();
        let batch = sample_batch();
        let canonical = prover.prove_raw_statement(&batch).expect("canonical proof");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            verify_raw_statement(&batch, &canonical)
                .expect("a proof must verify in another decode context");
            prover
                .prove_raw_statement(&batch)
                .expect("proof generated in another decode context")
        };
        assert_eq!(canonical, alternate);
        verify_raw_statement(&batch, &alternate)
            .expect("canonical verification of alternate proof");
    }
    #[test]
    fn strict_state_profile_accepts_unchanged_empty_batch() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        let proof = prover.prove(&batch).expect("strict empty no-op proof");
        super::verify(&batch, &proof).expect("strict empty no-op verification");
    }
    #[test]
    fn public_prover_rejects_default_batch_limits_before_semantics_or_proof_work() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let oversized_rows = sample_batch_with_size(DEFAULT_MAX_VERIFY_TRANSITIONS + 1);
        assert!(matches!(
            prover.prove(&oversized_rows),
            Err(Error::VerifierLimitExceeded {
                limit: "max_transitions",
                ..
            })
        ));
        let mut oversized_bytes =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        oversized_bytes.metadata.insert(
            "oversized-test-metadata".into(),
            vec![0; DEFAULT_MAX_VERIFY_BATCH_BYTES],
        );
        assert!(matches!(
            prover.prove(&oversized_bytes),
            Err(Error::VerifierLimitExceeded {
                limit: "max_batch_bytes",
                ..
            })
        ));
    }
    #[test]
    fn default_proof_ceiling_rejects_sixteen_row_transfer_opening_shape() {
        // Construct only the opening shape, without costly witness generation,
        // hashing or FRI. Even omitting every key/value column, sixteen transfer
        // rows require more than the default byte budget in the current wire.
        let batch = sample_batch_with_size(16);
        let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        let zero = wire_digest384(0);
        let columns = 12 + 128 + 196;
        let query_count = 128;
        proof.lde_domain_size = 128;
        proof.alphas = vec![fp4(0); AIR_COMPOSITION_ALPHA_COUNT];
        proof.betas = vec![fp4(0); 5];
        proof.fri_layers = vec![zero; 6];
        proof.queries = vec![
            QueryOpening {
                index: 0,
                value: fp4(0),
                chunk_values: vec![fp4(0); 64],
                merkle_path: vec![zero; 1],
            };
            query_count
        ];
        proof.air_openings = vec![
            AirConstraintOpening {
                index: 0,
                current_row: vec![0; columns],
                next_row: vec![0; columns],
                current_row_path: vec![zero; 7],
                next_row_path: vec![zero; 7],
                composition_value: fp4(0),
                composition_path: vec![zero; 7],
            };
            query_count
        ];
        proof.fri_queries = vec![
            FriQueryOpening {
                initial_index: 0,
                rounds: (0..5)
                    .map(|round| FriRoundOpening {
                        round: round as u32,
                        index: 0,
                        values: vec![fp4(0); 2],
                        folded_value: fp4(0),
                        merkle_path: vec![zero; 6 - round],
                    })
                    .collect(),
                final_index: 0,
                final_values: vec![fp4(0); 4],
                final_merkle_path: vec![zero; 1],
            };
            query_count
        ];
        let bytes = proof_size_hint(&proof);
        assert!(bytes > VerifyLimits::default().max_proof_bytes);
        assert!(matches!(
            enforce_default_verify_limits(&batch, &proof),
            Err(Error::VerifierLimitExceeded { limit: "max_proof_bytes", actual, .. })
                if actual == bytes
        ));
        // Explicit developer-only diagnostic limits still admit the shape;
        // this is not a valid cryptographic proof and is never verified here.
        enforce_verify_limits(&batch, &proof, prover_self_check_limits(&batch, &proof))
            .expect("diagnostic geometry may exceed production byte limits");
    }
    #[test]
    fn full_width_slot_uses_a_canonical_trace_residue() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = TransitionBatch::new(
            "fastpq-state-transition-stark-v1",
            PublicInputs {
                slot: u64::MAX,
                ..PublicInputs::default()
            },
        );
        let proof = prover.prove(&batch).expect("full-width slot proof");
        assert_eq!(proof.public_io.slot, u64::MAX);
        super::verify(&batch, &proof).expect("full-width slot verification");
    }
    #[test]
    fn strict_state_profile_rejects_unrooted_metadata_statement() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let raw_proof = prover
            .prove_raw_statement(&batch)
            .expect("cryptographic fixture proof");
        assert!(matches!(
            prover.prove(&batch),
            Err(Error::InvalidProofSemantics {
                profile: "state_transition",
                ..
            })
        ));
        assert!(matches!(
            super::verify(&batch, &raw_proof),
            Err(Error::InvalidProofSemantics {
                profile: "state_transition",
                ..
            })
        ));
    }
    #[test]
    fn verify_rejects_batch_parameter_mismatch_before_replay() {
        let (mut batch, proof) = sample_proof_with_size(8);
        batch.parameter = "test-mismatched-parameter".to_owned();
        let err = verify_raw_statement(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::ParameterMismatch {
                expected,
                actual
            } if expected == "fastpq-state-transition-stark-v1"
                && actual == "test-mismatched-parameter"
        ));
    }
    #[test]
    fn verify_rejects_unknown_parameter_relabelled_proof() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.parameter = "test-unknown-parameter".to_owned();
        let err = verify_raw_statement(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::UnknownParameter(parameter) if parameter == "test-unknown-parameter"
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
        let prover = Prover::canonical_with_execution_mode(
            "fastpq-state-transition-stark-v1",
            ExecutionMode::Cpu,
        )
        .expect("prover");
        assert_eq!(prover.backend.execution_mode(), ExecutionMode::Cpu);
    }
    #[test]
    fn canonical_parameter_sets_expose_the_canonical_catalogue() {
        assert_eq!(
            Prover::canonical_parameter_sets(),
            CANONICAL_PARAMETER_SETS.as_slice()
        );
    }
    #[test]
    fn native_v1_canonical_gpu_modes_fail_before_prover_construction() {
        for execution in [ExecutionMode::Cpu, ExecutionMode::Auto, ExecutionMode::Gpu] {
            for poseidon in [
                PoseidonExecutionMode::Cpu,
                PoseidonExecutionMode::Auto,
                PoseidonExecutionMode::Gpu,
            ] {
                let result = Prover::canonical_with_modes(
                    "fastpq-state-transition-stark-v1",
                    execution,
                    poseidon,
                );
                if execution == ExecutionMode::Gpu || poseidon == PoseidonExecutionMode::Gpu {
                    assert!(matches!(result, Err(Error::NativeV1GpuUnavailable)));
                } else {
                    result.expect("CPU and automatic native-V1 modes are implemented");
                }
            }
        }
        assert!(matches!(
            Prover::canonical_with_execution_mode(
                "fastpq-state-transition-stark-v1",
                ExecutionMode::Gpu,
            ),
            Err(Error::NativeV1GpuUnavailable)
        ));
    }
    #[test]
    fn native_v1_gpu_rejection_precedes_statement_and_witness_validation() {
        let prover = Prover::from_backend_config(
            BackendConfig::new(CANONICAL_PARAMETER_SETS[0]).with_execution_mode(ExecutionMode::Gpu),
        );
        let batch = TransitionBatch::new("invalid-parameter", PublicInputs::default());
        for result in [prover.prove(&batch), prover.prove_raw_statement(&batch)] {
            assert!(matches!(result, Err(Error::NativeV1GpuUnavailable)));
        }
    }
    #[test]
    fn native_v1_cpu_and_auto_proofs_are_identical() {
        let parameter = "fastpq-state-transition-stark-v1";
        let batch = TransitionBatch::new(parameter, PublicInputs::default());
        let cpu = Prover::canonical_with_execution_mode(parameter, ExecutionMode::Cpu)
            .expect("CPU prover")
            .prove(&batch)
            .expect("CPU proof");
        let automatic = Prover::canonical_with_execution_mode(parameter, ExecutionMode::Auto)
            .expect("automatic prover")
            .prove(&batch)
            .expect("automatic proof");
        assert_eq!(
            norito::core::to_bytes(&cpu).unwrap(),
            norito::core::to_bytes(&automatic).unwrap()
        );
    }
    #[test]
    fn materialise_proof_maps_backend_artifact_fields() {
        let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        assert_eq!(proof.protocol_version, PROTOCOL_VERSION);
        assert_eq!(proof.parameter, "fastpq-state-transition-stark-v1");
        assert_eq!(proof.trace_root, wire_digest384(11));
        assert_eq!(proof.air_trace_root, wire_digest384(12));
        assert_eq!(proof.air_composition_root, wire_digest384(13));
        assert_eq!(proof.lde_root, wire_digest384(14));
        assert_eq!(proof.lde_domain_size, 1);
        assert_eq!(proof.lookup_grand_product, 15);
        assert_eq!(proof.lookup_challenge, 16);
        assert_eq!(proof.alphas, fp4_values(&[17, 18]));
        assert_eq!(proof.betas, Vec::<GoldilocksFp4V1>::new());
        assert_eq!(proof.fri_layers, vec![wire_digest384(19)]);
        assert_eq!(proof.queries.len(), 1);
        assert_eq!(proof.queries[0].index, 0);
        assert_eq!(proof.queries[0].value, fp4(18));
        assert_eq!(proof.queries[0].chunk_values, vec![fp4(18)]);
        assert_eq!(proof.air_openings.len(), 1);
        assert_eq!(proof.fri_queries.len(), 1);
    }
    #[test]
    fn materialise_proof_preserves_commitment_and_public_io() {
        let commitment: GoldilocksDigest384V1 = digest384(19).into();
        let public_io = PublicIO {
            dsid: [0x01; 16],
            slot: 17,
            old_root: [0x02; 32],
            new_root: [0x03; 32],
            perm_root: [0x04; 32],
            tx_set_hash: [0x05; 32],
            ordering_hash: [0x06; 32],
        };
        let mut artifact = sample_backend_artifact();
        artifact.trace_commitment = commitment;
        let proof = materialise_proof(public_io.clone(), artifact).expect("materialise proof");
        assert_eq!(proof.commitment(), commitment);
        assert_eq!(proof.trace_commitment, commitment);
        assert_eq!(proof.public_io, public_io);
    }
    #[test]
    fn materialise_proof_preserves_multiple_query_openings_and_paths() {
        let mut artifact = sample_backend_artifact();
        artifact.query_openings.push((3, fp4(33)));
        artifact.query_chunks.push(fp4_values(&[30, 31, 32, 33]));
        artifact
            .query_paths
            .push(vec![digest384(101), digest384(102)]);
        artifact.air_openings.push(AirConstraintOpening {
            index: 3,
            current_row: vec![1, 2, 3],
            next_row: vec![4, 5, 6],
            current_row_path: vec![wire_digest384(201)],
            next_row_path: vec![wire_digest384(202)],
            composition_value: fp4(34),
            composition_path: vec![wire_digest384(203)],
        });
        artifact.fri_query_openings.push(FriQueryOpening {
            initial_index: 3,
            rounds: Vec::new(),
            final_index: 3,
            final_values: fp4_values(&[33, 34, 35, 36]),
            final_merkle_path: vec![wire_digest384(204)],
        });
        let proof = materialise_sample_artifact(artifact).unwrap();
        assert_eq!(proof.queries.len(), 2);
        assert_eq!(proof.queries[1].index, 3);
        assert_eq!(proof.queries[1].value, fp4(33));
        assert_eq!(proof.queries[1].chunk_values, fp4_values(&[30, 31, 32, 33]));
        assert_eq!(
            proof.queries[1].merkle_path,
            vec![wire_digest384(101), wire_digest384(102)]
        );
        assert_eq!(proof.air_openings[1].index, 3);
        assert_eq!(
            proof.air_openings[1].composition_path,
            vec![wire_digest384(203)]
        );
        assert_eq!(
            proof.fri_queries[1].final_merkle_path,
            vec![wire_digest384(204)]
        );
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
        let (batch, proof) = sample_proof_with_size(8);
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
    fn proof_digest_carriers_reject_noncanonical_words_at_construction() {
        assert!(
            GoldilocksDigest384V1::from_le_bytes(noncanonical_digest384_bytes()).is_none(),
            "proof roots, FRI layers, and Merkle paths share the canonical digest type"
        );
    }
    #[test]
    fn verify_rejects_modified_roots() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.trace_root = wire_digest384(0xAA);
        verify(&batch, &proof).unwrap_err();
    }
    #[test]
    fn verify_rejects_valid_field_trace_root_from_other_batch() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch_with_size(8);
        let foreign = sample_batch_with_size(9);
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        let foreign_proof = prover.prove_raw_statement(&foreign).unwrap();
        proof.trace_root = foreign_proof.trace_root;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_same_shape_foreign_trace_root() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch_with_size(8);
        let mut foreign = batch.clone();
        foreign.transitions[2].post_value[0] ^= 0x5A;
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        let foreign_proof = prover.prove_raw_statement(&foreign).unwrap();
        proof.trace_root = foreign_proof.trace_root;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_limits_reject_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_proof_bytes = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_proof_bytes",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_batch_size_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_batch_bytes = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_batch_bytes",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_query_count_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_queries = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_queries",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_transition_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_transitions = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_transitions",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_transition_limit_rejects_before_semantic_profile_validation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let proof = prover.prove_raw_statement(&batch).unwrap();
        assert!(matches!(
            validate_batch_semantics(&batch, ProofSemantics::StateTransition),
            Err(Error::InvalidProofSemantics { .. })
        ));
        assert!(matches!(
            validate_batch_semantics(&batch, ProofSemantics::AxtTransferClaim),
            Err(Error::InvalidProofSemantics { .. })
        ));

        let limits = verify_limits_with_override(|limits| limits.max_transitions = 0);
        let err = super::verify_with_limits_and_semantics(
            &batch,
            &proof,
            limits,
            ProofSemantics::AxtTransferClaim,
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_transitions",
                    actual,
                    max: 0,
                } if actual == batch.transitions.len()
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_fri_layer_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_fri_layers = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_fri_layers",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_query_path_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_query_path_len = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_query_path_len",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_air_row_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_air_row_values = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_air_row_values",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_fri_value_limit_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.trace_root = wire_digest384(1);
        let limits = verify_limits_with_override(|limits| limits.max_fri_round_values = 0);
        let err = verify_with_limits(&batch, &proof, limits).unwrap_err();
        assert!(
            matches!(
                err,
                Error::VerifierLimitExceeded {
                    limit: "max_fri_round_values",
                    ..
                }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_zero_lde_domain_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.lde_domain_size = 0;
        proof.trace_root = wire_digest384(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 0, len: 0 }
        ));
    }
    #[test]
    fn verify_protocol_version_rejects_before_trace_root_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.protocol_version = PROTOCOL_VERSION.wrapping_add(1);
        proof.trace_root = wire_digest384(0xAA);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedProtocolVersion {
                version
            } if version == PROTOCOL_VERSION + 1
        ));
    }
    #[test]
    fn verify_parameter_mismatch_rejects_before_trace_root_binding() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let mut batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        batch.parameter = "test-mismatched-parameter".to_owned();
        proof.trace_root = wire_digest384(0xAA);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::ParameterMismatch {
                expected,
                actual,
            } if expected == "fastpq-state-transition-stark-v1"
                && actual == "test-mismatched-parameter"
        ));
    }
    #[test]
    fn verify_public_io_mismatch_rejects_before_trace_root_binding() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.slot = proof.public_io.slot.wrapping_add(1);
        proof.trace_root = wire_digest384(0xAA);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "slot" }));
    }
    #[test]
    fn verify_rejects_commitment_mismatch_before_trace_root_binding() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.trace_commitment = digest384(53).into();
        proof.trace_root = wire_digest384(0xAA);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::CommitmentMismatch));
    }
    #[test]
    fn verify_rejects_valid_field_lde_root_tamper_after_trace_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.lde_root = wire_digest384(42);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::LdeRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_valid_field_air_trace_root_tamper_after_trace_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.air_trace_root = wire_digest384(43);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::AirTraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_valid_field_air_composition_root_tamper_after_trace_binding() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.air_composition_root = wire_digest384(44);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::AirCompositionRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_tampered_commitment() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.trace_commitment = digest384(53).into();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::CommitmentMismatch));
    }
    #[test]
    fn verify_rejects_relabelled_proof_from_different_batch() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let target = sample_batch_with_size(9);
        let mut proof = prover.prove_raw_statement(&source).unwrap();
        let target_proof = prover.prove_raw_statement(&target).unwrap();
        proof.trace_commitment = target_proof.trace_commitment;
        proof.public_io = target_proof.public_io;
        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_self_consistent_proof_with_foreign_public_io() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let target = sample_batch_with_size(9);
        let proof = proof_over_source_trace_with_target_statement(&prover, &source, &target);
        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_same_shape_foreign_trace_with_target_statement() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let mut target = source.clone();
        target.transitions[3].post_value[0] ^= 0x7F;
        let proof = proof_over_source_trace_with_target_statement(&prover, &source, &target);
        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_self_consistent_foreign_lde_bound_to_target_trace_root() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let mut target = source.clone();
        target.transitions[3].post_value[0] ^= 0x7F;
        let proof = proof_over_source_lde_with_target_trace_root(&prover, &source, &target);
        let legitimate = prover.prove_raw_statement(&target).unwrap();
        assert_eq!(proof.trace_root, legitimate.trace_root);
        assert_ne!(proof.lde_root, legitimate.lde_root);

        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(err, Error::LdeRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_same_transitions_with_foreign_public_inputs() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let mut target = source.clone();
        target.public_inputs.slot = target.public_inputs.slot.wrapping_add(1);
        target.public_inputs.dsid[0] ^= 0x5A;
        let proof = proof_over_source_trace_with_target_statement(&prover, &source, &target);
        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(err, Error::TraceRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_foreign_trace_with_grafted_target_commitments() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let mut target = source.clone();
        target.transitions[3].post_value[0] ^= 0x7F;
        let mut proof = proof_over_source_trace_with_target_statement(&prover, &source, &target);
        let target_proof = prover.prove_raw_statement(&target).unwrap();
        graft_artifact_commitments(&mut proof, &target_proof);
        let err = verify(&target, &proof).unwrap_err();
        assert!(
            matches!(
                err,
                Error::QueryMismatch { .. }
                    | Error::QueryMerklePathMismatch { .. }
                    | Error::AirMerklePathMismatch { .. }
                    | Error::AirConstraintMismatch { .. }
            ),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_foreign_artifact_after_target_roots_are_grafted() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let source = sample_batch_with_size(8);
        let mut target = source.clone();
        target.transitions[3].post_value[0] ^= 0x42;
        let mut proof = proof_over_source_trace_with_target_statement(&prover, &source, &target);
        let target_proof = prover.prove_raw_statement(&target).unwrap();
        proof.trace_root = target_proof.trace_root;
        proof.lde_root = target_proof.lde_root;
        proof.air_trace_root = target_proof.air_trace_root;
        proof.air_composition_root = target_proof.air_composition_root;
        proof.lde_domain_size = target_proof.lde_domain_size;
        let err = verify(&target, &proof).unwrap_err();
        // The permission challenge is sampled from the replaced commitment
        // roots before AIR coefficients or Merkle openings are inspected.
        assert!(matches!(err, Error::LookupChallengeMismatch));
    }
    #[test]
    fn verify_rejects_ordering_hash_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.ordering_hash[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::OrderingHashMismatch));
    }
    #[test]
    fn verify_rejects_dsid_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.dsid[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "dsid" }));
    }
    #[test]
    fn verify_rejects_slot_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.slot = proof.public_io.slot.wrapping_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "slot" }));
    }
    #[test]
    fn verify_rejects_old_root_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.old_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "old_root" }));
    }
    #[test]
    fn verify_rejects_new_root_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.new_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::PublicIoMismatch { field: "new_root" }));
    }
    #[test]
    fn verify_rejects_perm_root_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.public_io.perm_root[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::PublicIoMismatch { field: "perm_root" }
        ));
    }
    #[test]
    fn verify_rejects_tx_set_hash_mutation() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
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
    fn verify_preserves_explicit_zero_roots_and_hashes() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let mut batch = sample_batch();
        batch.public_inputs.perm_root = [0; 32];
        batch.public_inputs.tx_set_hash = [0; 32];
        let proof = prover.prove_raw_statement(&batch).unwrap();
        assert_eq!(proof.public_io.perm_root, [0; 32]);
        assert_eq!(proof.public_io.tx_set_hash, [0; 32]);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| tampered.public_io.tx_set_hash = [1; 32],
            |err| {
                matches!(
                    err,
                    Error::PublicIoMismatch {
                        field: "tx_set_hash"
                    }
                )
            },
        );
    }
    #[test]
    fn verify_rejects_wrong_betas() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        if let Some(beta) = proof.betas.first_mut() {
            let mut coefficients = beta.coefficients();
            coefficients[0] = add_mod(coefficients[0], 1);
            *beta = GoldilocksFp4V1::new(coefficients).expect("canonical mutated beta");
        }
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeMismatch { round: 0 }));
    }
    #[test]
    fn verify_rejects_wrong_lookup_challenge() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch_with_size(8);
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.lookup_challenge = if proof.lookup_challenge == 0 {
            1
        } else {
            proof.lookup_challenge - 1
        };
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::LookupChallengeMismatch));
    }
    #[test]
    fn verify_rejects_wrong_lookup_grand_product() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch_with_size(8);
        let mut proof = prover.prove_raw_statement(&batch).unwrap();
        proof.lookup_grand_product = if proof.lookup_grand_product == 0 {
            1
        } else {
            proof.lookup_grand_product - 1
        };
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::LookupGrandProductMismatch));
    }
    #[test]
    fn verify_rejects_modified_lde_root() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.lde_root = wire_digest384(0xAA);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::LdeRootMismatch));
    }
    #[test]
    fn verify_rejects_fri_layer_length_mismatch() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(
            proof.fri_layers.len() > 1,
            "expected at least one FRI layer plus terminal root"
        );
        proof.fri_layers.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriLayerLengthMismatch { .. }));
    }
    #[test]
    fn verify_rejects_extra_fri_layer_for_domain_schedule() {
        let (batch, mut proof) = sample_proof_with_size(8);
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
    fn verify_rejects_empty_fri_layer_roots() {
        let (batch, proof) = sample_proof_with_size(8);
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
    }
    #[test]
    fn verify_rejects_fri_layer_mutation() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(
            !proof.fri_layers.is_empty(),
            "expected non-empty FRI layer list"
        );
        proof.fri_layers[0] = wire_digest384(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeMismatch { round: 0 }));
    }
    #[test]
    fn verify_rejects_fri_challenge_length_mismatch() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(!proof.betas.is_empty(), "expected at least one FRI beta");
        proof.betas.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::FriChallengeLengthMismatch { .. }));
    }
    #[test]
    fn verify_rejects_query_count_mismatch() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(!proof.queries.is_empty(), "expected queries in proof");
        proof.queries.pop();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryCountMismatch { .. }));
    }
    #[test]
    fn verify_rejects_fri_and_air_vector_count_mismatches() {
        let (batch, proof) = sample_proof_with_size(8);
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
    fn verify_rejects_query_opening_permutation() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(proof.queries.len() > 1, "expected multiple query openings");
        proof.queries.swap(0, 1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));
    }
    #[test]
    fn verify_rejects_air_opening_permutation() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(
            proof.air_openings.len() > 1,
            "expected multiple AIR openings"
        );
        proof.air_openings.swap(0, 1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirOpeningMismatch { index: 0 }));
    }
    #[test]
    fn verify_rejects_fri_query_permutation() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(
            proof.fri_queries.len() > 1,
            "expected multiple FRI query openings"
        );
        proof.fri_queries.swap(0, 1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));
    }
    #[test]
    fn verify_rejects_duplicate_query_opening_with_preserved_count() {
        let (batch, mut proof) = sample_proof_with_size(8);
        assert!(proof.queries.len() > 1, "expected multiple query openings");
        proof.queries[1] = proof.queries[0].clone();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 1 }));
    }
    #[test]
    fn verify_rejects_wrong_query_value() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        first.value = first.value.add(fp4(1));
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { .. }));
    }
    #[test]
    fn verify_rejects_wrong_query_chunk_value() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        let chunk_value = first
            .chunk_values
            .first_mut()
            .expect("expected query chunk values");
        *chunk_value = chunk_value.add(fp4(1));
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryMismatch { .. } | Error::QueryMerklePathMismatch { .. }
        ));
    }
    #[test]
    fn verify_rejects_wrong_query_merkle_path() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let first = proof
            .queries
            .first_mut()
            .expect("expected at least one query opening");
        let sibling = first
            .merkle_path
            .first_mut()
            .expect("expected query Merkle path");
        *sibling = wire_digest384(0xBAD);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { .. }));
    }
    #[test]
    fn verify_rejects_malformed_query_and_air_openings() {
        let (batch, proof) = sample_proof_with_size(8);
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
                    .push(fp4(0));
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
                    .push(wire_digest384(0));
            },
            |err| matches!(err, Error::AirMerklePathMismatch { index: 0 }),
        );
    }
    #[test]
    fn verify_rejects_wrong_air_composition_root() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.air_composition_root = wire_digest384(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirCompositionRootMismatch));
    }
    #[test]
    fn verify_rejects_wrong_air_trace_root() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.air_trace_root = wire_digest384(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirTraceRootMismatch));
    }
    #[test]
    fn verify_rejects_missing_air_challenges() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.alphas.clear();
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirChallengeCountMismatch {
                expected: AIR_COMPOSITION_ALPHA_COUNT,
                actual: 0
            }
        ));
    }
    #[test]
    fn verify_rejects_extra_air_challenges() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.alphas.push(fp4(42));
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
        let (batch, proof) = sample_proof_with_size(8);
        assert_verify_rejects(
            &batch,
            &proof,
            |tampered| {
                let alpha = tampered.alphas.first_mut().expect("expected AIR challenge");
                *alpha = alpha.add(fp4(1));
            },
            |err| matches!(err, Error::AirChallengeMismatch { index: 0 }),
        );
    }
    #[test]
    fn verify_rejects_air_opening_index_mismatch() {
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let proof_batch = sample_batch();
        let proof = prover.prove_raw_statement(&proof_batch).unwrap();
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
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let batch = sample_batch();
        let proof = prover.prove_raw_statement(&batch).unwrap();
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
        let (batch, proof) = sample_proof_with_size(8);
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
        let (batch, proof) = sample_proof_with_size(8);
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
        let (batch, proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let (batch, proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let path = &mut proof
            .queries
            .first_mut()
            .expect("expected sampled query")
            .merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, wire_digest384(0));
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
        let batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
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
    fn enforce_verify_limits_bounds_nested_fri_rounds_before_payload_scan() {
        let batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        proof.fri_queries[0].rounds = vec![
            FriRoundOpening {
                round: 0,
                index: 0,
                values: Vec::new(),
                folded_value: GoldilocksFp4V1::ZERO,
                merkle_path: Vec::new(),
            };
            2
        ];
        let limits = verify_limits_with_override(|limits| limits.max_fri_layers = 1);
        let err = enforce_verify_limits(&batch, &proof, limits).unwrap_err();
        assert!(matches!(
            err,
            Error::VerifierLimitExceeded {
                limit: "max_fri_layers",
                actual: 2,
                max: 1,
            }
        ));
    }
    #[test]
    fn verify_limits_reject_oversized_proof_payload() {
        let (batch, proof) = sample_proof_with_size(8);
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
        let batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
        proof.air_openings[0].next_row_path.push(wire_digest384(7));
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
        proof.air_openings[0]
            .composition_path
            .push(wire_digest384(8));
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
        let (batch, proof) = sample_proof_with_size(8);
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
    fn trace_schema_width_is_bounded_before_proving_or_verifier_replay() {
        let mut batch =
            TransitionBatch::new("fastpq-state-transition-stark-v1", PublicInputs::default());
        batch.push(StateTransition::new(
            b"opaque-effect".to_vec(),
            vec![0xA5; (DEFAULT_MAX_VERIFY_AIR_ROW_VALUES + 1) * crate::LIMB_BYTES],
            Vec::new(),
            OperationKind::MetaSet,
        ));
        let actual = trace::column_count_for_batch(&batch).expect("metadata carrier schema");
        assert!(actual > DEFAULT_MAX_VERIFY_AIR_ROW_VALUES);

        let prover =
            Prover::canonical("fastpq-state-transition-stark-v1").expect("canonical prover");
        assert!(matches!(
            prover.prove_raw_statement(&batch),
            Err(Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual: observed,
                max: DEFAULT_MAX_VERIFY_AIR_ROW_VALUES,
            }) if observed == actual
        ));

        let proof = materialise_sample_artifact(sample_backend_artifact()).expect("sample proof");
        assert!(matches!(
            verify_with_limits(&batch, &proof, VerifyLimits::default()),
            Err(Error::VerifierLimitExceeded {
                limit: "max_air_row_values",
                actual: observed,
                max: DEFAULT_MAX_VERIFY_AIR_ROW_VALUES,
            }) if observed == actual
        ));
    }
    #[test]
    fn verify_limits_reject_oversized_final_fri_values() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let values = &mut proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening")
            .final_values;
        values.resize(
            DEFAULT_MAX_VERIFY_FRI_ROUND_VALUES + 1,
            GoldilocksFp4V1::ZERO,
        );
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let values = &mut proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.rounds.first_mut())
            .expect("expected FRI round opening")
            .values;
        // Keep the four-value terminal within the limit, then exceed the limit
        // only in a folding round so the intended preflight branch is exercised.
        let limits = VerifyLimits::default();
        values.resize(limits.max_fri_round_values + 1, GoldilocksFp4V1::ZERO);
        let values_len = values.len();
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let path = &mut proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening")
            .final_merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, wire_digest384(0));
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let path = &mut proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.rounds.first_mut())
            .expect("expected FRI round opening")
            .merkle_path;
        path.resize(DEFAULT_MAX_VERIFY_QUERY_PATH_LEN + 1, wire_digest384(0));
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let query = proof
            .fri_queries
            .first_mut()
            .expect("expected FRI query opening");
        let sibling = query
            .final_merkle_path
            .first_mut()
            .expect("expected final FRI Merkle path");
        *sibling = wire_digest384(0xBAD);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { .. }));
    }
    #[test]
    fn verify_rejects_zero_lde_domain_size() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.lde_domain_size = 0;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryIndexOutOfRange { index: 0, len: 0 }
        ));
    }
    #[test]
    fn verify_rejects_nonzero_lde_domain_size_tamper() {
        let (batch, mut proof) = sample_proof_with_size(8);
        proof.lde_domain_size = proof.lde_domain_size.saturating_add(1);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(
            matches!(err, Error::LdeRootMismatch),
            "unexpected verifier error: {err:?}"
        );
    }
    #[test]
    fn verify_rejects_wrong_fri_folded_value() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let round = proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.rounds.first_mut())
            .expect("expected FRI round opening");
        round.folded_value = round.folded_value.add(fp4(1));
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { .. }));
    }
    #[test]
    fn verify_rejects_wrong_final_fri_value() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let value = proof
            .fri_queries
            .first_mut()
            .and_then(|query| query.final_values.first_mut())
            .expect("expected final FRI values");
        *value = value.add(fp4(1));
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::QueryMismatch { .. } | Error::QueryMerklePathMismatch { .. }
        ));
    }
    #[test]
    fn verify_rejects_malformed_fri_query_chain() {
        let (batch, proof) = sample_proof_with_size(8);
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
        let (batch, proof) = sample_proof_with_size(8);
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
                    .push(fp4(0));
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
            final_values: vec![fp4(1)],
            final_merkle_path: Vec::new(),
        };
        let err = verify_fri_query_chain_for_test(0, 1, &fri_query, &[], &[], &[], 0).unwrap_err();
        assert!(matches!(err, Error::FriArity(0)));
    }
    #[test]
    fn lde_air_row_binding_checks_the_sampled_column_linear_combination() {
        let row = [GOLDILOCKS_MODULUS - 1, GOLDILOCKS_MODULUS - 2];
        let mix = [fp4(GOLDILOCKS_MODULUS - 1), fp4(2)];
        let expected = fp4(GOLDILOCKS_MODULUS - 3);
        ensure_lde_air_row_binding(7, expected, &row, &mix)
            .expect("canonical modular linear combination");
        assert!(matches!(
            ensure_lde_air_row_binding(7, expected.sub(fp4(1)), &row, &mix),
            Err(Error::QueryMismatch { index: 7 })
        ));
        let different_row = [row[0] - 1, row[1]];
        assert!(matches!(
            ensure_lde_air_row_binding(7, expected, &different_row, &mix),
            Err(Error::QueryMismatch { index: 7 })
        ));
    }
    #[test]
    fn lde_air_row_binding_checks_nonbase_extension_coordinates() {
        let row = [3, 5];
        let mix = [
            GoldilocksFp4V1::new([7, 11, 13, 17]).unwrap(),
            GoldilocksFp4V1::new([19, 23, 29, 31]).unwrap(),
        ];
        let expected = mix[0].mul_base(row[0]).add(mix[1].mul_base(row[1]));
        ensure_lde_air_row_binding(2, expected, &row, &mix).unwrap();
        for lane in 0..4 {
            let mut coefficients = expected.coefficients();
            coefficients[lane] += 1;
            assert!(matches!(
                ensure_lde_air_row_binding(
                    2,
                    GoldilocksFp4V1::new(coefficients).unwrap(),
                    &row,
                    &mix
                ),
                Err(Error::QueryMismatch { index: 2 })
            ));
        }
    }

    #[test]
    fn lde_air_row_binding_rejects_truncated_or_extended_oracles() {
        assert!(matches!(
            ensure_lde_air_row_binding(3, fp4(12), &[4], &fp4_values(&[3, 7])),
            Err(Error::AirOpeningMismatch { index: 3 })
        ));
        assert!(matches!(
            ensure_lde_air_row_binding(3, fp4(12), &[4, 0], &[fp4(3)]),
            Err(Error::AirOpeningMismatch { index: 3 })
        ));
    }
    #[test]
    fn final_v1_binary_fri_schedule_preserves_a_complete_terminal_domain() {
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let lengths = expected_fri_layer_lengths(
            1usize << params.lde_log_size,
            params.fri.arity,
            params.fri.max_reductions,
        )
        .expect("final V1 FRI schedule");
        let expected = (2..=19).rev().map(|log| 1_usize << log).collect::<Vec<_>>();
        assert_eq!(lengths, expected);
        assert_eq!(
            lengths.last().copied(),
            Some(fastpq_isi::FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize)
        );
        assert_eq!(
            fri_terminal_degree_bound(
                1usize << params.lde_log_size,
                params.fri.blowup_factor,
                params.fri.arity,
                &lengths,
            )
            .expect("final V1 terminal degree bound"),
            1
        );
    }
    #[test]
    fn terminal_schedule_preserves_and_enforces_the_initial_degree_bound() {
        // x^3 exceeds the exclusive bound < 2 for N_trace=1. One binary fold
        // leaves a nonconstant terminal polynomial, which must be rejected.
        let params = fastpq_isi::FASTPQ_FINAL_V1;
        let lengths = expected_fri_layer_lengths(8, params.fri.arity, params.fri.max_reductions)
            .expect("degree-preserving schedule");
        assert_eq!(lengths, [8, 4]);
        let domain = backend::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            8,
            params.omega_coset,
        )
        .expect("canonical eight-point domain");
        let values = (0..8)
            .map(|index| {
                let x = domain.point(index);
                fp4(mul_mod(mul_mod(x, x), x))
            })
            .collect::<Vec<_>>();
        assert!(!domain.evaluations_have_degree_below(&values, 2).unwrap());
        let beta = fp4(3);
        let folded = (0..4)
            .map(|index| {
                fold_fri_values(
                    &[values[index], values[index + 4]],
                    beta,
                    domain.point(index),
                    domain.coset_generator(4),
                )
                .expect("binary polynomial fold")
            })
            .collect::<Vec<_>>();
        let terminal_domain = domain.folded(2);
        let terminal_bound =
            fri_terminal_degree_bound(8, params.fri.blowup_factor, params.fri.arity, &lengths)
                .expect("exact terminal degree bound");
        assert_eq!(terminal_bound, 1);
        assert!(
            !terminal_domain
                .evaluations_have_degree_below(&folded, terminal_bound)
                .unwrap()
        );

        // The former extra fold rounded degree one back up to one and accepted
        // x^3 as the constant 15. Reject that schedule before any query check.
        assert!(matches!(
            fri_terminal_degree_bound(8, params.fri.blowup_factor, params.fri.arity, &[8, 4, 2]),
            Err(Error::FriTerminalDegreeBound {
                degree_bound: 1,
                domain_len: 4
            })
        ));
    }
    #[test]
    fn verify_fri_query_chain_accepts_terminal_leaf_without_rounds() {
        let final_values = vec![fp4(42)];
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
    fn verify_fri_final_opening_authenticates_all_four_terminal_values() {
        let values = fp4_values(&[42, 42, 42, 42]);
        let (root, path) = single_fri_leaf_root_and_path(0, 0, &values);
        let query = FriQueryOpening {
            initial_index: 3,
            rounds: Vec::new(),
            final_index: 3,
            final_values: values,
            final_merkle_path: path,
        };
        verify_fri_query_chain_with_terminal_bound_for_test(
            3,
            42,
            &query,
            &[root],
            &[],
            &[4],
            1,
            2,
        )
        .expect("authenticated four-point constant terminal");
        let mut substituted = query.clone();
        substituted.final_values[0] = fp4(43);
        assert!(matches!(
            verify_fri_query_chain_with_terminal_bound_for_test(
                3,
                42,
                &substituted,
                &[root],
                &[],
                &[4],
                1,
                2,
            ),
            Err(Error::QueryMerklePathMismatch { index: 0 })
        ));
        let mut truncated = query;
        truncated.final_values.truncate(2);
        assert!(matches!(
            verify_fri_query_chain_with_terminal_bound_for_test(
                3,
                42,
                &truncated,
                &[root],
                &[],
                &[4],
                1,
                2,
            ),
            Err(Error::QueryMismatch { index: 0 })
        ));
    }
    #[test]
    fn verify_fri_query_chain_rejects_authenticated_terminal_high_degree() {
        let final_values = fp4_values(&[42, 42, 42, 43]);
        let (final_root, final_path) = single_fri_leaf_root_and_path(0, 0, &final_values);
        let fri_layers = vec![final_root];
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values,
            final_merkle_path: final_path,
        };
        let error = verify_fri_query_chain_with_terminal_bound_for_test(
            0,
            42,
            &fri_query,
            &fri_layers,
            &[],
            &[4],
            1,
            2,
        )
        .expect_err("non-constant terminal evaluations must violate degree < 1");
        assert!(matches!(
            error,
            Error::FriTerminalDegreeMismatch { degree_bound: 1 }
        ));
    }
    #[test]
    fn verify_fri_query_chain_accepts_single_fold_round() {
        let beta = fp4(3);
        let round_values = fp4_values(&[10, 20]);
        let folded = fold_fri_group_for_test(&round_values, beta, 2, 0);
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
        let beta = fp4(5);
        let round_values = fp4_values(&[12, 34]);
        let sibling_values = fp4_values(&[77, 88]);
        let sibling_folded = fold_fri_group_for_test(&sibling_values, beta, 4, 0);
        let folded = fold_fri_group_for_test(&round_values, beta, 4, 1);
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
        let beta = fp4(3);
        let round_values = fp4_values(&[10, 20]);
        let folded = fold_fri_group_for_test(&round_values, beta, 2, 0);
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
        fri_layers[0] = wire_digest384(1);
        let err =
            verify_fri_query_chain_for_test(1, 20, &fri_query, &fri_layers, &[beta], &[2, 1], 2)
                .unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { index: 0 }));
        let mut bad_query = fri_query;
        bad_query.rounds[0].folded_value = bad_query.rounds[0].folded_value.add(fp4(1));
        fri_layers[0] = round_root;
        let err =
            verify_fri_query_chain_for_test(1, 20, &bad_query, &fri_layers, &[beta], &[2, 1], 2)
                .unwrap_err();
        assert!(matches!(err, Error::QueryMismatch { index: 0 }));
    }
    #[test]
    fn verify_fri_query_chain_rejects_offset_value_mismatches() {
        let beta = fp4(5);
        let round_values = fp4_values(&[12, 34]);
        let sibling_values = fp4_values(&[77, 88]);
        let sibling_folded = fold_fri_group_for_test(&sibling_values, beta, 4, 0);
        let folded = fold_fri_group_for_test(&round_values, beta, 4, 1);
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
        bad_final_query.final_values[1] = bad_final_query.final_values[1].add(fp4(1));
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
            final_values: vec![fp4(1)],
            final_merkle_path: Vec::new(),
        };
        let (final_root, _) = single_fri_leaf_root_and_path(0, 0, &[fp4(1)]);
        let err =
            verify_fri_query_chain_for_test(0, 1, &fri_query, &[final_root], &[fp4(7)], &[1], 2)
                .unwrap_err();
        assert!(matches!(
            err,
            Error::FriChallengeLengthMismatch {
                expected: 1,
                actual: 0
            }
        ));
        let round_values = fp4_values(&[1, 2]);
        let folded = fold_fri_group_for_test(&round_values, fp4(7), 2, 0);
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
        let err =
            verify_fri_query_chain_for_test(0, 1, &fri_query, &[final_root], &[fp4(7)], &[2], 2)
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
    fn verify_fri_query_chain_rejects_terminal_layer_shape_errors() {
        let fri_query = FriQueryOpening {
            initial_index: 0,
            rounds: Vec::new(),
            final_index: 0,
            final_values: vec![fp4(42)],
            final_merkle_path: vec![backend::hash_fri_chunk(0, 0, &[fp4(42)]).unwrap().into()],
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
        let wrong_root = wire_digest384(1);
        let err = verify_fri_query_chain_for_test(0, 42, &fri_query, &[wrong_root], &[], &[1], 2)
            .unwrap_err();
        assert!(matches!(err, Error::QueryMerklePathMismatch { index: 0 }));
    }
    #[test]
    fn modular_fri_folding_wraps_in_goldilocks_field() {
        assert_eq!(add_mod(GOLDILOCKS_MODULUS - 1, 2), 1);
        assert_eq!(mul_mod(GOLDILOCKS_MODULUS - 1, GOLDILOCKS_MODULUS - 1), 1);
        let params = fastpq_isi::CANONICAL_PARAMETER_SETS[0];
        let domain = backend::FriDomain::from_lde_parameters(
            params.lde_root,
            params.lde_log_size,
            2,
            params.omega_coset,
        )
        .expect("two-point FRI domain");
        let x = domain.point(0);
        let zeta = domain.coset_generator(1);
        let values = [fp4(GOLDILOCKS_MODULUS - 1), fp4(2)];
        assert_eq!(
            fold_fri_values(&values, fp4(x), x, zeta).unwrap(),
            values[0]
        );
        assert_eq!(
            fold_fri_values(&values, fp4(mul_mod(x, zeta)), x, zeta).unwrap(),
            values[1]
        );
    }
    #[test]
    fn verify_rejects_wrong_air_next_row_opening() {
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
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
        let (batch, mut proof) = sample_proof_with_size(8);
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected sampled AIR opening");
        let sibling = first
            .composition_path
            .first_mut()
            .expect("expected composition Merkle path");
        *sibling = wire_digest384(0xBAD);
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::AirMerklePathMismatch { .. }));
    }
    #[test]
    fn verify_rejects_wrong_air_composition_opening() {
        let (batch, mut proof) = sample_proof_with_size(8);
        let first = proof
            .air_openings
            .first_mut()
            .expect("expected at least one AIR opening");
        first.composition_value = first.composition_value.add(fp4(1));
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(
            err,
            Error::AirConstraintMismatch { .. } | Error::AirMerklePathMismatch { .. }
        ));
    }
    #[test]
    fn verify_rejects_mismatched_large_batch_by_ordering_hash_before_commitment() {
        let prover = Prover::canonical("fastpq-state-transition-stark-v1").unwrap();
        let small_batch = sample_batch();
        let proof = prover.prove_raw_statement(&small_batch).unwrap();
        let large_batch = sample_batch_with_size(DEFAULT_MAX_VERIFY_TRANSITIONS + 1);
        let limits = verify_limits_with_override(|limits| {
            limits.max_transitions = large_batch.transitions.len()
        });
        let err = verify_with_limits(&large_batch, &proof, limits).unwrap_err();
        assert!(matches!(err, Error::OrderingHashMismatch));
    }
    #[test]
    fn verify_accepts_large_batch_when_limits_allow() {
        let row_count = DEFAULT_MAX_VERIFY_TRANSITIONS + 1;
        let limits = verify_limits_with_override(|limits| {
            limits.max_transitions = row_count;
            limits.max_proof_bytes = 2 * 1024 * 1024;
        });
        let (batch, proof) = sample_proof_with_size_and_limits(row_count, limits);
        verify_with_limits(&batch, &proof, limits).unwrap();
    }
    mod wire_contract;
}
