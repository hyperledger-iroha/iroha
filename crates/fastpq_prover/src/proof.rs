use core::convert::TryFrom;
use fastpq_isi::{CANONICAL_PARAMETER_SETS, StarkParameterSet, find_by_name};
use iroha_crypto::Hash;
use norito::{NoritoDeserialize, NoritoSerialize};

use crate::{
    Error, Result, TransitionBatch,
    backend::{
        self, Backend, BackendArtifact, BackendConfig, ExecutionMode, LOOKUP_PRODUCT_DOMAIN,
        PoseidonExecutionMode, StarkBackend, TRANSCRIPT_TAG_ALPHA_PREFIX, TRANSCRIPT_TAG_GAMMA,
        TRANSCRIPT_TAG_INIT, TRANSCRIPT_TAG_ROOTS,
    },
    ordering,
    trace::{
        self, PoseidonPipelinePolicy, build_trace, column_index, derive_polynomial_data,
        hash_columns_from_coefficients, merkle_root, merkle_root_with_first_level,
    },
    trace_commitment,
};

/// Protocol version advertised by the Stage 2 prover implementation.
const PROTOCOL_VERSION: u16 = 1;
/// Domain tag for permission root fallback commitments.
const PERM_ROOT_DOMAIN: &[u8] = b"fastpq:v1:perm_root";
/// Domain tag for transaction set hash fallback commitments.
const TX_SET_DOMAIN: &[u8] = b"fastpq:v1:tx_set";

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[repr(C)]
pub struct QueryOpening {
    /// Domain index opened by the prover.
    pub index: u32,
    /// Evaluation value at the queried index.
    pub value: u64,
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
    /// Poseidon commitment over the lookup witness LDE leaves.
    pub lookup_root: [u8; 32],
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
}

impl Proof {
    /// Access the commitment hash.
    pub fn commitment(&self) -> Hash {
        self.trace_commitment
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
        Ok(materialise_proof(
            commitment,
            public_io,
            artifact,
            params_version,
        ))
    }
}

/// Verify a proof by recomputing the commitment deterministically and replaying
/// the Stage 2 Fiat–Shamir transcript.
///
/// # Errors
///
/// Returns [`Error::UnknownParameter`] when the proof references an unknown
/// parameter set or an appropriate [`Error`] variant when validation fails.
#[allow(clippy::too_many_lines)]
pub fn verify(batch: &TransitionBatch, proof: &Proof) -> Result<()> {
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

    let expected_commitment = trace_commitment(&params, batch)?;
    if expected_commitment != proof.trace_commitment {
        return Err(Error::CommitmentMismatch);
    }

    let expected_ordering = ordering::ordering_hash(batch)?;
    if proof.public_io.ordering_hash != hash_norito::core::to_bytes(&expected_ordering) {
        return Err(Error::OrderingHashMismatch);
    }

    let expected_permission_hashes = collect_permission_hashes(batch)?;
    if proof.public_io.permission_hashes != expected_permission_hashes {
        return Err(Error::PermissionHashMismatch);
    }

    let trace = build_trace(batch)?;
    let planner = crate::Planner::new(&params);
    let mut polynomial_data = derive_polynomial_data(&trace, &planner, ExecutionMode::Cpu);

    let column_digests = hash_columns_from_coefficients(
        &trace,
        &polynomial_data.coefficients,
        &planner,
        ExecutionMode::Cpu,
        PoseidonPipelinePolicy::for_mode(ExecutionMode::Cpu),
    );
    let trace_root =
        merkle_root_with_first_level(column_digests.leaves(), column_digests.fused_parents());
    if proof.trace_root != field_norito::core::to_bytes(trace_root) {
        return Err(Error::TraceRootMismatch);
    }

    let lde_columns = polynomial_data.lde_columns();
    let lde_values = backend::hash_trace_rows(lde_columns);
    let lde_hashes = backend::hash_lde_leaves(&lde_values, params.fri.arity)?;
    let lde_root = merkle_root(&lde_hashes);
    if proof.lookup_root != field_norito::core::to_bytes(lde_root) {
        return Err(Error::LookupRootMismatch);
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

    for (idx, &alpha) in proof.alphas.iter().enumerate() {
        let tag = format!("{TRANSCRIPT_TAG_ALPHA_PREFIX}:{idx}");
        let expected = transcript.challenge_field(&tag);
        if expected != alpha {
            return Err(Error::FriChallengeMismatch { round: idx });
        }
    }

    let selector_index =
        column_index(&trace, "s_perm").ok_or_else(|| Error::MissingColumn("s_perm".to_string()))?;
    let witness_index = column_index(&trace, "perm_hash")
        .ok_or_else(|| Error::MissingColumn("perm_hash".to_string()))?;
    let lookup_product = backend::compute_lookup_grand_product(
        lde_columns[selector_index].as_slice(),
        lde_columns[witness_index].as_slice(),
        gamma,
    );
    if lookup_product != proof.lookup_grand_product {
        return Err(Error::LookupGrandProductMismatch);
    }
    transcript.append_message(
        LOOKUP_PRODUCT_DOMAIN,
        &proof.lookup_grand_product.to_le_bytes(),
    );

    let (fri_layers, betas) = backend::fold_with_fri(
        &lde_values,
        params.fri.arity,
        params.fri.max_reductions,
        &mut transcript,
    )?;
    if fri_layers.len() != proof.fri_layers.len() {
        return Err(Error::FriLayerLengthMismatch {
            expected: fri_layers.len(),
            actual: proof.fri_layers.len(),
        });
    }
    for (round, (&expected, actual_bytes)) in
        fri_layers.iter().zip(proof.fri_layers.iter()).enumerate()
    {
        if field_norito::core::to_bytes(expected) != *actual_bytes {
            return Err(Error::FriLayerMismatch { round });
        }
    }

    if betas.len() != proof.betas.len() {
        return Err(Error::FriChallengeLengthMismatch {
            expected: betas.len(),
            actual: proof.betas.len(),
        });
    }
    for (round, (&expected, &actual)) in betas.iter().zip(proof.betas.iter()).enumerate() {
        if expected != actual {
            return Err(Error::FriChallengeMismatch { round });
        }
    }

    let expected_queries = backend::sample_queries(
        lde_values.len(),
        usize::try_from(params.fri.queries).expect("query count fits usize"),
        &mut transcript,
    );
    if expected_queries.len() != proof.queries.len() {
        return Err(Error::QueryCountMismatch {
            expected: expected_queries.len(),
            actual: proof.queries.len(),
        });
    }
    for (pos, (&expected_idx, query)) in expected_queries.iter().zip(&proof.queries).enumerate() {
        let expected_index =
            u32::try_from(expected_idx).map_err(|_| Error::QueryIndexOverflow {
                index: expected_idx,
            })?;
        if expected_index != query.index {
            return Err(Error::QueryMismatch { index: pos });
        }
        let value = lde_values
            .get(expected_idx)
            .copied()
            .unwrap_or_else(|| *lde_values.last().unwrap_or(&0));
        if value != query.value {
            return Err(Error::QueryMismatch { index: pos });
        }
    }

    Ok(())
}

fn materialise_proof(
    commitment: Hash,
    public_io: PublicIO,
    artifact: BackendArtifact,
    params_version: u16,
) -> Proof {
    let fri_layers = artifact
        .fri_layers
        .into_iter()
        .map(field_norito::core::to_bytes)
        .collect();
    let queries = artifact
        .query_openings
        .into_iter()
        .map(|(index, value)| QueryOpening { index, value })
        .collect();
    Proof {
        protocol_version: PROTOCOL_VERSION,
        params_version,
        parameter: artifact.parameter,
        trace_commitment: commitment,
        public_io,
        trace_root: field_norito::core::to_bytes(artifact.trace_root),
        lookup_root: field_norito::core::to_bytes(artifact.lookup_root),
        lookup_grand_product: artifact.lookup_grand_product,
        lookup_challenge: artifact.lookup_challenge,
        alphas: artifact.alphas,
        betas: artifact.fri_betas,
        fri_layers,
        queries,
    }
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
    fn verify_rejects_ordering_hash_mutation() {
        let prover = Prover::canonical("fastpq-lane-balanced").unwrap();
        let batch = sample_batch();
        let mut proof = prover.prove(&batch).unwrap();
        proof.public_io.ordering_hash[0] ^= 0x01;
        let err = verify(&batch, &proof).unwrap_err();
        assert!(matches!(err, Error::OrderingHashMismatch));
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
        assert!(matches!(err, Error::FriLayerLengthMismatch { .. }));
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
        assert!(matches!(err, Error::FriLayerMismatch { round: 0 }));
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
            lookup_root: [8; 32],
            lookup_grand_product: 9,
            lookup_challenge: 10,
            alphas: vec![11, 12],
            betas: vec![13, 14],
            fri_layers: vec![[15; 32], [16; 32]],
            queries: vec![QueryOpening {
                index: 0,
                value: 123,
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
