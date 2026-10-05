//! Portable finality and challenged node statements from the current certified chain.

use iroha_crypto::{Algorithm, Hash, KeyPair, SignatureOf};
use iroha_data_model::{
    sumeragi::SumeragiStatus,
    sumeragi_finality::{
        FinalityError, FinalityValidator, SumeragiFinalityAttestation,
        SumeragiFinalityAttestationBody, SumeragiFinalityBundle, SumeragiFinalityCheckpoint,
        SumeragiFinalityProof, SumeragiFinalityVerifier,
    },
};
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
    /// Original local history acquisition has not completed.
    #[error(transparent)]
    Deferred(crate::execution_attempt::ExecutionDeferred),
}

impl From<crate::execution_attempt::ExecutionAttemptError<ChainReadError>> for ProofError {
    fn from(error: crate::execution_attempt::ExecutionAttemptError<ChainReadError>) -> Self {
        match error {
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => Self::Chain(error),
            crate::execution_attempt::ExecutionAttemptError::Deferred(local) => {
                Self::Deferred(local)
            }
        }
    }
}

/// Build the current embedded-certificate proof from one immutable state view.
///
/// # Errors
/// Missing/corrupt frames, unavailable authenticated committees or invalid certificates.
pub fn build_proof(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<SumeragiFinalityProof, ProofError> {
    let chain = CertifiedChain::new(view)?;
    proof_from_chain(&chain, height)
}

fn proof_from_chain<V: StateReadOnly>(
    chain: &CertifiedChain<'_, V>,
    height: u64,
) -> Result<SumeragiFinalityProof, ProofError> {
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
        // The complete certified read already authenticates this exact
        // historical epoch. Project its original keys/PoPs without a second
        // prefix read; portable verification below remains independent.
        committee: certified
            .commitment()
            .schedule
            .current
            .committee
            .iter()
            .map(|member| FinalityValidator {
                public_key: member.validator.public_key().clone(),
                proof_of_possession: member.proof_of_possession.clone(),
            })
            .collect(),
    };
    // The native reader verifies complete application attestations before serving.
    // Portable checks additionally enforce the independent client framing contract.
    proof.decode_checked()?;
    Ok(proof)
}

/// Export a portable checkpoint from the node's authenticated native history.
///
/// The returned checkpoint is suitable for local response self-checks. Remote
/// clients must select their own trust root independently of the served response.
/// Each iteration retains at most three portable decisions; original Core history
/// verification and its resource limits remain owned by `CertifiedChain`.
///
/// # Errors
/// Missing history, invalid finality or any failure to retain the exact checkpoint.
pub fn build_checkpoint(
    view: &impl StateReadOnly,
    height: u64,
) -> Result<SumeragiFinalityCheckpoint, ProofError> {
    let chain = CertifiedChain::new(view)?;
    // Refuse an unavailable target before walking any prefix.
    let tip = proof_from_chain(&chain, height)?;
    let genesis = proof_from_chain(&chain, 1)?;
    let mut verifier = SumeragiFinalityVerifier::new(
        chain.genesis(),
        &view.chain_id().to_string(),
        genesis.committee.clone(),
    )?;
    for at in 1..=height {
        let proof = if at == height {
            tip.clone()
        } else {
            proof_from_chain(&chain, at)?
        };
        verifier.verify(&proof)?;
        let checkpoint = verifier.export_checkpoint(&proof)?;
        if at == height {
            return Ok(checkpoint);
        }
        verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            view.network_id(),
            &view.chain_id().to_string(),
        )?;
    }
    Err(ProofError::UnverifiedCommittee(height))
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
    /// The current installed node wall clock is before the Unix epoch, zero or overflowing.
    #[error("current node Unix clock is unavailable")]
    ClockUnavailable,
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
    let observed_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|reading| u64::try_from(reading.as_millis()).ok())
        .filter(|reading| *reading != 0)
        .ok_or(Error::ClockUnavailable)?;
    let body = SumeragiFinalityAttestationBody {
        challenge,
        observed_at_unix_ms,
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

    // The normal-stack native fixture completes before the later proof-reader
    // assertion frame exists. Its genuinely signed prefix and original State
    // remain owned by this helper throughout both independent reads.
    #[inline(never)]
    fn with_original_finality_boundary(assert_original: fn(&CertifiedTestChain)) {
        let mut chain = Box::new(CertifiedTestChain::npos_boundary_fixture());
        chain.commit(Vec::new());
        assert_eq!(chain.height(), 10);
        assert_original(&chain);
    }

    // Independent test oracle: exactly one complete native certified read. It
    // does not call the producer or the separate proof_committee read, and it
    // retains the unchanged canonical output, including every original PoP.
    #[inline(never)]
    fn original_finality_from_one_certified_read<V: StateReadOnly>(
        view: &V,
        height: u64,
        require_native_boundary: bool,
    ) -> Result<SumeragiFinalityProof, ProofError> {
        let reader = CertifiedChain::new(view)?;
        let original = reader.certified(height)?;
        assert_eq!(original.height(), height);
        assert_eq!(
            original.verification(),
            if height == 1 {
                QcVerification::Genesis
            } else {
                QcVerification::Verified
            }
        );
        if require_native_boundary {
            assert!(original.header().expect("native boundary header").attest);
            assert!(
                original
                    .commit_qc()
                    .expect("genuine boundary CommitQC")
                    .attest
            );
            assert!(original.commitment().schedule.boundary.is_some());
        }
        Ok(SumeragiFinalityProof {
            block_header: original.block().header(),
            block_wire: original
                .block()
                .encode_wire()
                .expect("original canonical signed frame"),
            committee: original
                .commitment()
                .schedule
                .current
                .committee
                .iter()
                .map(|member| FinalityValidator {
                    public_key: member.validator.public_key().clone(),
                    proof_of_possession: member.proof_of_possession.clone(),
                })
                .collect(),
        })
    }

    #[inline(never)]
    fn assert_single_certified_finality_source(
        chain: &CertifiedTestChain,
        height: u64,
        expected_frames: &[u64],
        expected_qcs: &[u64],
        require_native_boundary: bool,
    ) {
        use crate::sumeragi::certified_chain::relation_counts;

        let view = chain.state().view();
        let (original, once) = relation_counts::measure(|| {
            original_finality_from_one_certified_read(&view, height, require_native_boundary)
        });
        let original = original.expect("the exact signed original prefix authenticates once");
        assert_eq!(once.frames, expected_frames);
        assert_eq!(once.qcs, expected_qcs);
        assert!(original.decode_checked().is_ok());

        let (produced, served) = relation_counts::measure(|| build_proof(&view, height));
        let produced = produced.expect("ordinary current-source proof producer");
        assert_eq!(
            produced, original,
            "same header, canonical signed bytes and exact epoch PoPs"
        );
        assert_eq!(produced.height(), height);
        assert!(produced.decode_checked().is_ok());
        assert_eq!(
            served.frames, once.frames,
            "portable producer must use the same already authenticated source instead of reading its committee again"
        );
        assert_eq!(
            served.qcs, once.qcs,
            "the committee projection must not repeat the complete native quorum/Pasta verification"
        );
    }

    #[test]
    fn portable_genesis_proof_uses_one_authenticated_frame_relation() {
        let chain = CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000))
            .expect("actual signed genesis and retained State");
        assert_single_certified_finality_source(&chain, 1, &[1, 1], &[], false);
    }

    #[test]
    fn portable_proof_uses_one_authenticated_prefix_for_exact_committee() {
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000))
            .expect("actual signed genesis and retained State");
        chain.commit(Vec::new());
        chain.commit(Vec::new());
        assert_single_certified_finality_source(&chain, 3, &[1, 3, 2], &[2, 3], false);
    }

    #[test]
    fn portable_native_boundary_proof_uses_one_authenticated_prefix_and_original_pasta() {
        with_original_finality_boundary(
            assert_portable_native_boundary_proof_uses_one_authenticated_prefix_and_original_pasta,
        );
    }

    #[inline(never)]
    fn assert_portable_native_boundary_proof_uses_one_authenticated_prefix_and_original_pasta(
        chain: &CertifiedTestChain,
    ) {
        assert_single_certified_finality_source(
            chain,
            10,
            &[1, 10, 2, 3, 4, 5, 6, 7, 8, 9],
            &[2, 3, 4, 5, 6, 7, 8, 9, 10],
            true,
        );
    }

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
        invalid.commit_at(20_000, Vec::new());
        invalid.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
        assert!(matches!(
            build_proof(&invalid.state().view(), 2),
            Err(ProofError::Chain(ChainReadError::Certificate { .. }))
        ));
    }
    #[test]
    fn original_checkpoint_binary_refusal_is_local_and_retries_exact_original_source() {
        use crate::execution_attempt::ExecutionDeferred;
        use iroha_data_model::block::decode_framed_signed_block;
        use ivm::error::ExecutionDeferral;
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000))
            .expect("original signed State genesis");
        chain.commit(Vec::new());
        let view = chain.state().view();
        let checkpoint = build_checkpoint(&view, 2).unwrap();
        let original = checkpoint.encode_canonical().unwrap();
        let chain_id = view.chain_id().to_string();
        let wire = chain
            .genesis()
            .canonical_resultless_proposal()
            .unwrap()
            .encode_wire()
            .unwrap();
        let limits = |allocation| {
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64)
        };
        let producer =
            norito::with_decode_limits_scope(limits(0), || decode_framed_signed_block(&wire))
                .unwrap_err();
        assert_eq!(
            producer.kind(),
            norito::core::DecodeAttemptErrorKind::EnclosingLimit
        );
        assert!(
            matches!(producer.into_error().decode_resource_error(), Some(
            norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit: 0 }
        ) if attempted > 0)
        );
        let read = || {
            SumeragiFinalityVerifier::from_trusted_checkpoint(
                &checkpoint,
                &chain.network_id(),
                &chain_id,
            )
        };
        let error =
            norito::with_decode_limits_scope(limits(0), || read().map_err(ProofError::from))
                .unwrap_err();
        let expected = ExecutionDeferred::from(ExecutionDeferral::ActiveMemoryCapacity);
        assert!(
            matches!(&error, ProofError::Deferred(local) if local == &expected),
            "{error:?}"
        );
        assert_eq!(checkpoint.encode_canonical().unwrap(), original);
        let retried = read().unwrap();
        assert_eq!(
            retried.export_checkpoint(checkpoint.tip()).unwrap(),
            checkpoint
        );
        let completed = norito::with_decode_limits_scope(limits(8 * 1024 * 1024), || {
            let inner = norito::with_decode_limits_scope(limits(0), read).unwrap_err();
            ProofError::from(inner)
        });
        assert!(
            matches!(&completed, ProofError::Deferred(local) if local == &expected),
            "the captured original admission refusal survives scope retirement: {completed:?}"
        );
        // A new canonical attempt cannot borrow the retired attempt's opaque origin.
        let reintroduced = norito::with_decode_limits_scope(limits(8 * 1024 * 1024), || {
            let inner = norito::with_decode_limits_scope(limits(0), read).unwrap_err();
            let iroha_data_model::sumeragi_finality::FinalityReadError::DecodeResource(original) =
                inner
            else {
                panic!("the actual checkpoint read must retain its original decoder error");
            };
            assert_eq!(
                original.kind(),
                norito::core::DecodeAttemptErrorKind::EnclosingLimit
            );
            let stale = original.into_error();
            let recaptured =
                norito::core::classify_decode_attempt(|| Err::<(), _>(stale)).unwrap_err();
            assert_eq!(
                recaptured.kind(),
                norito::core::DecodeAttemptErrorKind::Invalid
            );
            ProofError::from(
                iroha_data_model::sumeragi_finality::FinalityReadError::DecodeResource(recaptured),
            )
        });
        assert!(
            matches!(reintroduced, ProofError::Portable(_)),
            "a fresh observer rejects the retired origin: {reintroduced:?}"
        );
        // Copied diagnostic numbers carry no original emitting layer or attempt family.
        let reconstructed = norito::with_decode_limits_scope(limits(0), || {
            let error = norito::core::classify_decode_attempt(|| {
                Err::<(), _>(norito::Error::TotalAllocationExceeded {
                    attempted: 1,
                    limit: 0,
                })
            })
            .unwrap_err();
            assert_eq!(error.kind(), norito::core::DecodeAttemptErrorKind::Invalid);
            ProofError::from(
                iroha_data_model::sumeragi_finality::FinalityReadError::DecodeResource(error),
            )
        });
        assert!(
            matches!(reconstructed, ProofError::Portable(_)),
            "copied numbers cannot mint local admission provenance: {reconstructed:?}"
        );
        assert_eq!(checkpoint.encode_canonical().unwrap(), original);
    }

    #[test]
    fn native_checkpoint_continues_the_exact_original_prefix() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000)).unwrap();
        chain.commit(Vec::new());
        chain.commit(Vec::new());
        let view = chain.state().view();
        let checkpoint = build_checkpoint(&view, 2).unwrap();
        assert_eq!(checkpoint.height(), 2);
        assert_eq!(checkpoint.network_id(), chain.network_id());
        let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &checkpoint,
            &chain.network_id(),
            &view.chain_id().to_string(),
        )
        .unwrap();
        let next = build_proof(&view, 3).unwrap();
        assert_eq!(verifier.verify(&next).unwrap().header(), next.block_header);
        assert!(build_checkpoint(&view, 0).is_err());
        assert!(build_checkpoint(&view, 4).is_err());
    }

    #[test]
    fn portable_builder_verifies_original_pasta_boundary_witness() {
        let mut chain = CertifiedTestChain::npos_boundary_fixture();
        chain.commit(Vec::new());
        let proof = build_proof(&chain.state().view(), 10).unwrap();
        assert_eq!(proof.height(), 10);
        assert!(proof.decode_checked().is_ok());
    }

    #[test]
    fn native_attestation_signs_actual_current_unix_reading_separately_from_block_time() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 10_000)).unwrap();
        chain.commit_at(20_000, Vec::new());
        let signer = KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal);
        let identity = NodeIdentity {
            node_id: iroha_model_base::peer::PeerId::new(signer.public_key().clone()),
            config_fingerprint: Hash::new(b"actual test native configuration"),
        };
        let status = SumeragiStatus {
            protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
            config_fingerprint: identity.config_fingerprint,
            beacon_horizon: None,
            instance: chain.instance().0,
            height: 3,
            view: 0,
            stage: 0,
            leader: None,
            proxy_tail: None,
            high_qc_view: None,
            level: 0,
            start_level: 0,
            t_retx_ms: 100,
            committed_height: 2,
            applied_height: 2,
            awaiting: false,
            signer: Some(signer.public_key().clone()),
            unanchored: false,
            abstaining: false,
            halted: None,
            footprint: iroha_data_model::sumeragi::SumeragiFootprint::default(),
        };
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let original = build_attestation(
            &chain.state().view(),
            status,
            &identity,
            Hash::new(b"actual test native executable"),
            2,
            [41; 32],
            &signer,
        )
        .unwrap();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let observed = u128::from(original.body.observed_at_unix_ms);
        assert!(observed >= before && observed <= after);
        assert_ne!(
            original.body.observed_at_unix_ms,
            original.body.finality_proof.block_header.creation_time_ms
        );
        original.verify().unwrap();
        let mut changed = original;
        changed.body.observed_at_unix_ms += 1;
        assert!(
            changed.verify().is_err(),
            "the exact current reading belongs to the node's signature"
        );
    }
}

impl From<iroha_data_model::sumeragi_finality::FinalityReadError> for ProofError {
    fn from(error: iroha_data_model::sumeragi_finality::FinalityReadError) -> Self {
        use iroha_data_model::sumeragi_finality::FinalityReadError;
        match error {
            FinalityReadError::Invalid(error) => Self::Portable(error),
            FinalityReadError::DecodeResource(original) => {
                let completed = |error: norito::core::DecodeAttemptError| {
                    Self::Portable(FinalityError(error.to_string()))
                };
                if cfg!(all(test, sumeragi_core_mutation = "HC52")) {
                    return completed(original);
                }
                match crate::execution_attempt::canonical_decode_attempt_error(original, completed)
                {
                    crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                    crate::execution_attempt::ExecutionAttemptError::Deferred(local) => {
                        Self::Deferred(local)
                    }
                }
            }
            FinalityReadError::Genesis(error) => {
                match crate::execution_attempt::genesis_read_attempt_error(error, |error| {
                    Self::Encoding(error.to_string())
                }) {
                    crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
                    crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                        Self::Deferred(reason)
                    }
                }
            }
        }
    }
}
