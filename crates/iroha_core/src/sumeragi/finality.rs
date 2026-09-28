//! Portable finality and challenged node statements from the current certified chain.

use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    sumeragi::SumeragiStatus,
    sumeragi_finality::{
        FinalityError, FinalityValidator, SumeragiFinalityAttestation,
        SumeragiFinalityAttestationBody, SumeragiFinalityBundle, SumeragiFinalityProof,
    },
};
use iroha_sumeragi::crypto::NoAttestation;
use norito::codec::Encode as _;

use super::{
    certified_chain::{CertifiedChain, ChainReadError, QcVerification},
    node::NodeIdentity,
};
use crate::state::StateReadOnly;

/// Why a current portable proof could not be served.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProofError {
    /// No current certified frame can be read for this height.
    #[error(transparent)]
    Chain(#[from] ChainReadError),
    /// The historical committee cannot independently verify this certificate.
    #[error("height {0} lacks an independently verified commit certificate")]
    UnverifiedCommittee(u64),
    /// Canonical framing failed.
    #[error("cannot encode certified block: {0}")]
    Encoding(String),
    /// The portable verifier rejected the produced proof.
    #[error(transparent)]
    Portable(#[from] FinalityError),
}

/// Build the current embedded-certificate proof from one immutable state view.
///
/// # Errors
/// Missing/corrupt frames, unavailable authenticated committees or invalid certificates.
pub fn build_proof(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<SumeragiFinalityProof, ProofError> {
    let chain = CertifiedChain::new(view)?.with_attestation_verifier(&NoAttestation);
    let certified = chain.certified(height)?;
    if !matches!(
        (height, certified.verification()),
        (1, QcVerification::Genesis) | (2.., QcVerification::Verified)
    ) {
        return Err(ProofError::UnverifiedCommittee(height));
    }
    let proof = SumeragiFinalityProof {
        block_header: certified.block().header(),
        block_wire: certified
            .block()
            .encode_wire()
            .map_err(|error| ProofError::Encoding(error.to_string()))?,
        committee: chain
            .proof_committee(height)?
            .into_iter()
            .map(|(public_key, proof_of_possession)| FinalityValidator {
                public_key,
                proof_of_possession,
            })
            .collect(),
    };
    // Serving uses the same portable checks as the independent client. This also rejects
    // any application-attested QC until its complete attestation verifier is installed.
    proof.decode_checked()?;
    Ok(proof)
}

/// Build a network-bound current proof bundle from one immutable state view.
///
/// # Errors
/// See [`build_proof`].
pub fn build_bundle(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<SumeragiFinalityBundle, ProofError> {
    Ok(SumeragiFinalityBundle {
        network_id: *view.network_id(),
        finality_proof: build_proof(view, height)?,
    })
}

/// Why a challenged current-driver capture cannot be signed.
#[derive(Debug, thiserror::Error)]
pub enum AttestationBuildError {
    /// Genesis is not committed.
    #[error("cannot attest an empty state")]
    EmptyState,
    /// The state height cannot fit on the wire.
    #[error("committed height exceeds u64")]
    HeightOverflow,
    /// Requested and immutable state-tip heights differ.
    #[error("requested height {requested} is not durable tip {committed}")]
    HeightIsNotDurableTip {
        /// Requested height.
        requested: u64,
        /// Immutable applied height.
        committed: u64,
    },
    /// A separately sampled driver status has not reached exactly this applied tip.
    #[error("driver status and durable tip heights differ")]
    StatusHeightMismatch,
    /// The current driver stopped or halted.
    #[error("consensus requires restart")]
    RestartRequired,
    /// Status contradicts its current consensus instance or its local identity.
    #[error("current driver status is inconsistent")]
    InvalidStatus,
    /// The supplied signer is not the installed BLS node key.
    #[error("attestation signer is not the installed BLS node key")]
    InvalidSigner,
    /// The tip proof is unavailable or invalid.
    #[error("tip proof unavailable: {0}")]
    FinalityProof(ProofError),
    /// The genesis proof is unavailable or invalid.
    #[error("genesis proof unavailable: {0}")]
    GenesisFinalityProof(ProofError),
    /// The final exact body failed consistency validation.
    #[error(transparent)]
    InvalidBody(FinalityError),
    /// Signing failed.
    #[error("node signing failed: {0}")]
    Signing(String),
}

/// Whether the actual driver's public status is internally coherent.
#[must_use]
pub fn status_is_consistent(status: &SumeragiStatus) -> bool {
    status.stage <= 2
        && status.applied_height <= status.committed_height
        && status.height >= status.committed_height
        && status.height <= status.committed_height.saturating_add(1)
}

/// Sign an exact durable-tip capture using the installed current node identity.
///
/// # Errors
/// Missing proofs, mismatched heights/identity/instance, halted state or invalid signing.
pub fn build_attestation(
    view: &impl StateReadOnly,
    status: SumeragiStatus,
    identity: &NodeIdentity,
    build_fingerprint: Hash,
    height: u64,
    challenge: [u8; 32],
    signer: &KeyPair,
) -> Result<SumeragiFinalityAttestation, AttestationBuildError> {
    use AttestationBuildError as Error;
    if signer.algorithm() != Algorithm::BlsNormal
        || identity.node_id.public_key() != signer.public_key()
    {
        return Err(Error::InvalidSigner);
    }
    if status.is_halted() {
        return Err(Error::RestartRequired);
    }
    if !status_is_consistent(&status)
        || status
            .signer
            .as_ref()
            .is_some_and(|key| key != identity.node_id.public_key())
    {
        return Err(Error::InvalidStatus);
    }
    let committed = u64::try_from(view.block_hashes().len()).map_err(|_| Error::HeightOverflow)?;
    let genesis_block_hash = view
        .block_hashes()
        .first()
        .copied()
        .ok_or(Error::EmptyState)?;
    if height != committed {
        return Err(Error::HeightIsNotDurableTip {
            requested: height,
            committed,
        });
    }
    let genesis_finality_proof = build_proof(view, 1).map_err(Error::GenesisFinalityProof)?;
    let finality_proof = if height == 1 {
        genesis_finality_proof.clone()
    } else {
        build_proof(view, height).map_err(Error::FinalityProof)?
    };
    let chain = CertifiedChain::new(view).map_err(|error| Error::FinalityProof(error.into()))?;
    if status.instance != chain.instance().0 {
        return Err(Error::InvalidStatus);
    }
    // Only a height mismatch after successful proof and identity validation is retryable.
    if status.applied_height != committed || status.committed_height != committed {
        return Err(Error::StatusHeightMismatch);
    }
    let body = SumeragiFinalityAttestationBody {
        challenge,
        network_id: *view.network_id(),
        node_id: identity.node_id.clone(),
        node_fingerprint: Hash::new(identity.node_id.encode()),
        build_fingerprint,
        config_fingerprint: identity.config_fingerprint,
        genesis_block_hash,
        genesis_finality_proof,
        status,
        finality_proof,
    };
    body.validate_consistency().map_err(Error::InvalidBody)?;
    let signature = SignatureOf::try_from_hash(signer.private_key(), body.signing_hash())
        .map_err(|error| Error::Signing(error.to_string()))?;
    let attestation = SumeragiFinalityAttestation { body, signature };
    attestation.verify().map_err(Error::InvalidBody)?;
    Ok(attestation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    };

    #[test]
    fn portable_proof_uses_current_embedded_certificates_and_rejects_subquorum() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000)).unwrap();
        chain.commit_at(20_000, Vec::new());
        let proof = build_proof(&chain.state().view(), 2).unwrap();
        assert_eq!(proof.height(), 2);
        assert_eq!(proof.committee.len(), 4);
        assert!(proof.decode_checked().is_ok());
        assert!(build_proof(&chain.state().view(), 1).is_ok());
        assert!(build_proof(&chain.state().view(), 0).is_err());
        assert!(build_proof(&chain.state().view(), 3).is_err());

        let mut invalid =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000)).unwrap();
        invalid.commit_with(Some(20_000), Vec::new(), Signers::BelowQuorum);
        assert!(matches!(
            build_proof(&invalid.state().view(), 2),
            Err(ProofError::Chain(ChainReadError::Certificate { .. }))
        ));
    }
}
