//! Helpers for bridge finality proofs built from commit certificates.
use crate::state::{State as CoreState, StateReadOnly};
use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus_v2::SumeragiV2Status,
        consensus_v2::finality::{V2FinalityArtifact, V2QuorumCertificateVerificationError},
    },
    bridge::{
        BRIDGE_FINALITY_ATTESTATION_VERSION_V1, BRIDGE_FINALITY_PROOF_VERSION_V2, BridgeCommitment,
        BridgeFinalityAttestationBodyV1, BridgeFinalityAttestationV1,
        BridgeFinalityAttestationValidationError, BridgeFinalityBundle, BridgeFinalityProof,
    },
};
use iroha_model_base::peer::PeerId;
use thiserror::Error;
/// A Sumeragi-v2 finality artifact whose structure, roster PoPs, and CommitQC
/// cryptography have already been verified.
///
/// The wrapper is intentionally not decodable and exposes no mutable access.
/// Untrusted implementations of [`BridgeStateReadOnly`] must call [`Self::verify_for_header`]
/// to mint it. Kura-backed implementations use the private constructor only
/// after Kura's cache-backed verification boundary succeeds, and attach the
/// header authenticated by Kura's private durable finality record.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct VerifiedV2FinalityArtifact {
    artifact: V2FinalityArtifact,
    retained_header: BlockHeader,
}
impl VerifiedV2FinalityArtifact {
    /// Fully verify an untrusted artifact against its exact retained header.
    ///
    /// # Errors
    ///
    /// Returns the canonical v2 verification error when structural, PoP, or
    /// CommitQC cryptographic validation fails.
    pub fn verify_for_header(
        retained_header: BlockHeader,
        artifact: V2FinalityArtifact,
    ) -> Result<Self, V2QuorumCertificateVerificationError> {
        artifact.verify()?;
        artifact
            .validate_for_header(&retained_header)
            .map_err(V2QuorumCertificateVerificationError::InvalidArtifact)?;
        Ok(Self {
            artifact,
            retained_header,
        })
    }
    /// Borrow the verified artifact without allowing mutation.
    #[must_use]
    pub const fn artifact(&self) -> &V2FinalityArtifact {
        &self.artifact
    }
    /// Consume the wrapper and return the verified artifact.
    #[must_use]
    pub fn into_artifact(self) -> V2FinalityArtifact {
        self.artifact
    }
    /// Borrow Kura's authenticated retained header when one accompanied the artifact.
    #[must_use]
    pub const fn retained_header(&self) -> &BlockHeader {
        &self.retained_header
    }
    fn from_kura_verified(block_header: BlockHeader, artifact: V2FinalityArtifact) -> Self {
        Self {
            artifact,
            retained_header: block_header,
        }
    }
}
/// Narrow read-only surface used by bridge finality proof builders.
///
/// This keeps bridge-proof construction independent from full `StateView` snapshots.
pub trait BridgeStateReadOnly {
    /// Exact genesis-derived network identity bound to the state snapshot.
    fn bridge_network_id(&self) -> &NetworkId;
    /// Load an exact durable Sumeragi-v2 finality artifact whose structure, roster PoPs, and
    /// CommitQC cryptography have already been verified by the storage boundary.
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String>;
}
impl<T: StateReadOnly> BridgeStateReadOnly for T {
    fn bridge_network_id(&self) -> &NetworkId {
        self.network_id()
    }
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }
}
impl BridgeStateReadOnly for CoreState {
    fn bridge_network_id(&self) -> &NetworkId {
        self.network_id_ref()
    }
    fn bridge_verified_v2_finality_artifact(
        &self,
        height: u64,
    ) -> Result<Option<VerifiedV2FinalityArtifact>, String> {
        self.kura()
            .v2_finality_artifact_with_header(height)
            .map(|record| {
                record.map(|(header, artifact)| {
                    VerifiedV2FinalityArtifact::from_kura_verified(header, artifact)
                })
            })
            .map_err(|error| error.to_string())
    }
}
/// Errors returned when constructing a bridge finality proof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BridgeFinalityError {
    /// The requested block height is zero.
    #[error("invalid block height {0}")]
    InvalidHeight(u64),
    /// No durable Sumeragi-v2 finality artifact exists for the requested height.
    #[error("Sumeragi-v2 finality artifact for height {0} not found")]
    FinalityArtifactNotFound(u64),
    /// Kura could not decode or validate the durable artifact.
    #[error("failed to load Sumeragi-v2 finality artifact for height {height}: {reason}")]
    FinalityArtifactRead {
        /// Height being proven.
        height: u64,
        /// Bounded Kura validation diagnostic.
        reason: String,
    },
    /// The durable artifact does not match the selected block header or chain.
    #[error("Sumeragi-v2 finality artifact for height {height} does not match the selected block")]
    FinalityArtifactMismatch {
        /// Height being proven.
        height: u64,
    },
}
/// Build a self-contained finality proof for the block at `height`.
///
/// The proof bundles the block header and Kura's exact immutable v2 finality
/// artifact. The artifact owns BLS PoPs aligned with its frozen powered roster,
/// so historical verification never consults mutable validator state.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the height is zero, the durable retained-header artifact is
/// missing/malformed, or the exact v2 artifact fails cryptographic verification.
pub fn build_finality_proof(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    if height == 0 {
        return Err(BridgeFinalityError::InvalidHeight(height));
    }
    let verified_finality = state
        .bridge_verified_v2_finality_artifact(height)
        .map_err(|reason| BridgeFinalityError::FinalityArtifactRead { height, reason })?
        .ok_or(BridgeFinalityError::FinalityArtifactNotFound(height))?;
    build_finality_proof_from_verified(state.bridge_network_id(), height, &verified_finality)
}
fn build_finality_proof_from_verified(
    network_id: &NetworkId,
    height: u64,
    verified_finality: &VerifiedV2FinalityArtifact,
) -> Result<BridgeFinalityProof, BridgeFinalityError> {
    let block_header = verified_finality.retained_header().clone();
    let finality_artifact = verified_finality.artifact().clone();
    if finality_artifact.height != height
        || block_header.height().get() != height
        || finality_artifact.height_context.network_id != *network_id
        || finality_artifact
            .validate_for_header(&block_header)
            .is_err()
    {
        return Err(BridgeFinalityError::FinalityArtifactMismatch { height });
    }
    Ok(BridgeFinalityProof {
        version: BRIDGE_FINALITY_PROOF_VERSION_V2,
        block_header,
        finality_artifact,
    })
}
/// Build and sign one challenge-bound attestation for the exact committed state tip.
///
/// The first block hash and requested tip are taken from one immutable state view. The
/// embedded proof is loaded from Kura's verified finality boundary, and the reducer status
/// must name that exact proof before the node key is allowed to sign anything.
///
/// # Errors
///
/// Returns [`BridgeFinalityAttestationBuildError`] when the state is empty, the requested
/// height is not its exact tip, finality is unavailable, the status/proof body is inconsistent,
/// or the configured signer cannot produce a verifiable signature.
pub fn build_finality_attestation(
    state: &impl StateReadOnly,
    status: SumeragiV2Status,
    height: u64,
    challenge: [u8; 32],
    signer: &KeyPair,
) -> Result<BridgeFinalityAttestationV1, BridgeFinalityAttestationBuildError> {
    if signer.algorithm() != Algorithm::BlsNormal {
        return Err(BridgeFinalityAttestationBuildError::InvalidSignerAlgorithm);
    }
    let committed_height = u64::try_from(state.block_hashes().len())
        .map_err(|_| BridgeFinalityAttestationBuildError::HeightOverflow)?;
    if committed_height == 0 {
        return Err(BridgeFinalityAttestationBuildError::EmptyState);
    }
    require_exact_durable_tip_height(height, committed_height)?;
    let genesis_block_hash = state
        .block_hashes()
        .first()
        .copied()
        .ok_or(BridgeFinalityAttestationBuildError::EmptyState)?;
    let committed_tip_hash = state
        .block_hashes()
        .last()
        .copied()
        .ok_or(BridgeFinalityAttestationBuildError::EmptyState)?;
    let genesis_finality_proof = build_finality_proof(state, 1)
        .map_err(BridgeFinalityAttestationBuildError::GenesisFinalityProof)?;
    require_finality_proof_at_committed_genesis(
        genesis_block_hash,
        genesis_finality_proof.finality_artifact.block_hash,
    )?;
    let finality_proof = build_finality_proof(state, height)
        .map_err(BridgeFinalityAttestationBuildError::FinalityProof)?;
    require_finality_proof_at_committed_tip(
        committed_tip_hash,
        finality_proof.finality_artifact.block_hash,
    )?;
    let node_id = PeerId::new(signer.public_key().clone());
    let node_fingerprint = Hash::new(norito::codec::Encode::encode(&node_id));
    let body = BridgeFinalityAttestationBodyV1 {
        version: BRIDGE_FINALITY_ATTESTATION_VERSION_V1,
        challenge,
        network_id: *state.network_id(),
        node_id,
        node_fingerprint,
        genesis_block_hash,
        genesis_finality_proof,
        status,
        finality_proof,
    };
    body.validate_consistency()
        .map_err(BridgeFinalityAttestationBuildError::InvalidBody)?;
    let signature = SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
        .map_err(|error| BridgeFinalityAttestationBuildError::Signing(error.to_string()))?;
    let attestation = BridgeFinalityAttestationV1 { body, signature };
    attestation
        .verify()
        .map_err(BridgeFinalityAttestationBuildError::InvalidBody)?;
    Ok(attestation)
}
fn require_finality_proof_at_committed_tip(
    committed_tip_hash: iroha_crypto::HashOf<BlockHeader>,
    proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if proof_block_hash != committed_tip_hash {
        return Err(BridgeFinalityAttestationBuildError::FinalityTipMismatch {
            committed_tip_hash,
            proof_block_hash,
        });
    }
    Ok(())
}
fn require_exact_durable_tip_height(
    requested: u64,
    committed: u64,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if requested != committed {
        return Err(BridgeFinalityAttestationBuildError::HeightIsNotDurableTip {
            requested,
            committed,
        });
    }
    Ok(())
}
fn require_finality_proof_at_committed_genesis(
    committed_genesis_hash: iroha_crypto::HashOf<BlockHeader>,
    proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
) -> Result<(), BridgeFinalityAttestationBuildError> {
    if proof_block_hash != committed_genesis_hash {
        return Err(
            BridgeFinalityAttestationBuildError::GenesisFinalityMismatch {
                committed_genesis_hash,
                proof_block_hash,
            },
        );
    }
    Ok(())
}
/// Failure while producing a node-signed durable-tip finality attestation.
#[derive(Debug, Error)]
pub enum BridgeFinalityAttestationBuildError {
    /// No committed genesis exists in the state snapshot.
    #[error("cannot attest finality for an empty state")]
    EmptyState,
    /// The committed block count cannot be represented on the wire.
    #[error("committed height does not fit into u64")]
    HeightOverflow,
    /// Only the exact durable tip may be attested.
    #[error("requested height {requested} is not durable tip {committed}")]
    HeightIsNotDurableTip {
        /// Requested block height.
        requested: u64,
        /// Exact committed state-view tip.
        committed: u64,
    },
    /// The verified finality record is not for the immutable state-view tip hash.
    #[error(
        "finality proof block hash {proof_block_hash:?} does not match committed tip {committed_tip_hash:?}"
    )]
    FinalityTipMismatch {
        /// Last block hash in the immutable state view.
        committed_tip_hash: iroha_crypto::HashOf<BlockHeader>,
        /// Block hash authenticated by the loaded finality proof.
        proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
    },
    /// The verified height-one finality record is not for the immutable state-view genesis hash.
    #[error(
        "genesis finality proof block hash {proof_block_hash:?} does not match committed genesis {committed_genesis_hash:?}"
    )]
    GenesisFinalityMismatch {
        /// First block hash in the immutable state view.
        committed_genesis_hash: iroha_crypto::HashOf<BlockHeader>,
        /// Block hash authenticated by the loaded height-one proof.
        proof_block_hash: iroha_crypto::HashOf<BlockHeader>,
    },
    /// The production node signer is not the current BLS consensus identity.
    #[error("finality attestation signer must use BlsNormal")]
    InvalidSignerAlgorithm,
    /// Kura could not produce the exact verified proof for the requested tip.
    #[error("failed to build durable-tip finality proof: {0:?}")]
    FinalityProof(BridgeFinalityError),
    /// Kura could not produce the exact verified proof for committed height one.
    #[error("failed to build committed-genesis finality proof: {0:?}")]
    GenesisFinalityProof(BridgeFinalityError),
    /// Status, node identity, genesis, or proof duplicate bindings disagree.
    #[error("finality attestation body is inconsistent: {0}")]
    InvalidBody(BridgeFinalityAttestationValidationError),
    /// The configured private key failed to sign the domain-separated body hash.
    #[error("failed to sign finality attestation: {0}")]
    Signing(String),
}
/// Build a compact commitment plus exact typed finality proof for `height`.
///
/// # Errors
///
/// Returns [`BridgeFinalityError`] when the underlying finality proof cannot be
/// built for the requested height.
pub fn build_finality_bundle(
    state: &impl BridgeStateReadOnly,
    height: u64,
) -> Result<BridgeFinalityBundle, BridgeFinalityError> {
    let proof = build_finality_proof(state, height)?;
    let commitment = BridgeCommitment {
        network_id: proof.finality_artifact.height_context.network_id,
        height_context_id: proof.finality_artifact.context_id(),
        block_height: proof.finality_artifact.height,
        block_hash: proof.finality_artifact.block_hash,
    };
    Ok(BridgeFinalityBundle {
        commitment,
        finality_proof: proof,
    })
}
/// Verification errors raised when checking a BridgeFinalityProof.
#[allow(variant_size_differences)]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BridgeFinalityVerificationError {
    /// The caller expected a different finalized height.
    #[error("finality proof height mismatch: expected {expected}, actual {actual}")]
    HeightMismatch {
        /// Height requested by the caller.
        expected: u64,
        /// Height carried by the exact v2 artifact.
        actual: u64,
    },
    /// Exact proof verification failed.
    #[error(transparent)]
    Verification(#[from] iroha_data_model::bridge::BridgeFinalityVerifyError),
}
/// Verification knobs for verify_finality_proof.
#[derive(Debug, Clone, Copy)]
pub struct FinalityProofVerificationConfig<'a> {
    /// Exact genesis-derived network identity expected by the verifier.
    pub expected_network_id: &'a NetworkId,
    /// Optional expected height to bind the proof to a specific block.
    pub expected_height: Option<u64>,
    /// Trusted context id for the exact height being verified.
    pub trusted_context_id: iroha_data_model::block::consensus_v2::HeightContextId,
}
/// Verify a BridgeFinalityProof against network, height, context, powered quorum,
/// PoP, and aggregate-signature expectations.
///
/// # Errors
///
/// Returns BridgeFinalityVerificationError when the expected height differs or
/// the exact typed Sumeragi-v2 proof fails verification.
pub fn verify_finality_proof(
    proof: &BridgeFinalityProof,
    config: &FinalityProofVerificationConfig<'_>,
) -> Result<(), BridgeFinalityVerificationError> {
    if let Some(expected_height) = config.expected_height {
        let actual = proof.finality_artifact.height;
        if actual != expected_height {
            return Err(BridgeFinalityVerificationError::HeightMismatch {
                expected: expected_height,
                actual,
            });
        }
    }
    let mut verifier = iroha_data_model::bridge::BridgeFinalityVerifier::with_context(
        *config.expected_network_id,
        config.trusted_context_id,
    );
    verifier.verify(proof)?;
    Ok(())
}
#[cfg(test)]
mod tests;
