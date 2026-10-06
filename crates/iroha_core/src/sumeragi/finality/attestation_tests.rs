//! Genuine source and allocation controls for one-cut challenged attestation proof production.

use super::*;
use crate::{
    state::World,
    sumeragi::{
        certified_chain::{QcVerification, relation_counts},
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};

fn installed_signer() -> KeyPair {
    KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal)
}

fn status(chain: &CertifiedTestChain) -> SumeragiStatus {
    let signer = installed_signer();
    assert!(
        chain
            .validators()
            .iter()
            .any(|(peer, _)| peer.public_key() == signer.public_key())
    );
    SumeragiStatus {
        protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
        config_fingerprint: Hash::new(b"exact attestation proof reader configuration"),
        beacon_horizon: None,
        instance: chain.instance().0,
        height: chain.height() + 1,
        view: 0,
        stage: 0,
        leader: None,
        proxy_tail: None,
        high_qc_view: None,
        level: 0,
        start_level: 0,
        t_retx_ms: 100,
        committed_height: chain.height(),
        applied_height: chain.height(),
        awaiting: false,
        signer: Some(signer.public_key().clone()),
        unanchored: false,
        abstaining: false,
        halted: None,
        footprint: iroha_data_model::sumeragi::SumeragiFootprint::default(),
    }
}

fn capture(
    view: &impl StateReadOnly,
    chain: &CertifiedTestChain,
    challenge: [u8; 32],
    status: SumeragiStatus,
) -> Result<SumeragiFinalityAttestation, AttestationBuildError> {
    let signer = installed_signer();
    let identity = NodeIdentity {
        node_id: iroha_model_base::peer::PeerId::new(signer.public_key().clone()),
        config_fingerprint: Hash::new(b"exact attestation proof reader configuration"),
    };
    build_attestation(
        view,
        status,
        &identity,
        Hash::new(b"exact attestation proof reader build"),
        chain.height(),
        challenge,
        &signer,
    )
}

#[inline(never)]
fn with_chain_at(height: u64, check_original: fn(&CertifiedTestChain)) {
    let mut chain = if height == 10 {
        CertifiedTestChain::npos_boundary_fixture()
    } else {
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap()
    };
    if height > 1 {
        chain.commit(Vec::new());
    }
    assert_eq!(chain.height(), height);
    check_original(&chain);
}

#[inline(never)]
fn check_original_attestation(chain: &CertifiedTestChain) {
    let height = chain.height();
    let view = chain.state().view();
    // Independently exercised public producers are the byte-for-byte comparison oracle.
    let original_genesis = build_proof(&view, 1).unwrap();
    let original_tip = build_proof(&view, height).unwrap();
    let expected_frames = if height == 1 {
        vec![1, 1]
    } else {
        let mut frames = vec![1, 1, height];
        frames.extend(2..height);
        frames
    };
    let (produced, checked) =
        relation_counts::measure(|| capture(&view, chain, [41; 32], status(chain)));
    let produced = produced.unwrap();
    assert_eq!(
        checked.frames, expected_frames,
        "one original genesis prefix serves both proofs"
    );
    assert_eq!(
        checked.qcs,
        (2..=height).collect::<Vec<_>>(),
        "every original native QC remains checked exactly once"
    );
    assert_eq!(produced.body.genesis_finality_proof, original_genesis);
    assert_eq!(produced.body.finality_proof, original_tip);
    assert_eq!(produced.body.challenge, [41; 32]);
    assert_eq!(produced.body.network_id, chain.network_id());
    assert_eq!(produced.body.genesis_block_hash, chain.genesis().hash());
    assert_eq!(produced.body.status.instance, chain.instance().0);
    produced.verify().unwrap();
    if height == 10 {
        let native = CertifiedChain::new(&view)
            .unwrap()
            .certified(height)
            .unwrap();
        assert_eq!(native.verification(), QcVerification::Verified);
        assert!(native.commitment().schedule.boundary.is_some());
        assert!(!native.header().unwrap().attest);
        let qc = native.commit_qc().unwrap();
        assert_eq!(qc.signers.count_ones(), 3);
        assert!(!qc.attest && qc.attestations.is_empty() && qc.attestation_witness.is_none());
    }
    let fresh = capture(&view, chain, [42; 32], status(chain)).unwrap();
    assert_eq!(
        fresh.body.genesis_finality_proof,
        produced.body.genesis_finality_proof
    );
    assert_eq!(fresh.body.finality_proof, produced.body.finality_proof);
    assert_eq!(fresh.body.challenge, [42; 32]);
    assert_ne!(
        fresh.signature, produced.signature,
        "each actual challenge is separately signed"
    );
    fresh.verify().unwrap();
}

#[test]
fn challenged_genesis_attestation_keeps_exact_original_proof() {
    with_chain_at(1, check_original_attestation);
}

#[test]
fn challenged_tip_attestation_checks_one_original_native_prefix() {
    with_chain_at(2, check_original_attestation);
}

#[test]
fn challenged_boundary_attestation_checks_every_exact_native_quorum() {
    with_chain_at(10, check_original_attestation);
}

#[test]
fn attestation_resource_refusal_retries_the_same_original_source_without_retained_cache() {
    with_chain_at(2, check_resource_retry);
}

#[inline(never)]
fn check_resource_retry(chain: &CertifiedTestChain) {
    let view = chain.state().view();
    let source_genesis = chain.genesis().encode_wire().unwrap();
    let source_tip = chain.committed(2).block().encode_wire().unwrap();
    let hashes = view.block_hashes().iter().copied().collect::<Vec<_>>();
    let budget = view.execution_budget();
    let reserved = budget.reserved_bytes();
    let original = capture(&view, chain, [43; 32], status(chain)).unwrap();
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || capture(&view, chain, [44; 32], status(chain)),
    )
    .unwrap_err();
    assert!(
        matches!(
            refused,
            AttestationBuildError::GenesisFinalityProof(ProofError::Deferred(_))
                | AttestationBuildError::FinalityProof(ProofError::Deferred(_))
        ),
        "the real original native decoder retains local resource refusal: {refused:?}"
    );
    assert_eq!(budget.reserved_bytes(), reserved);
    assert_eq!(chain.height(), 2);
    assert_eq!(
        view.block_hashes().iter().copied().collect::<Vec<_>>(),
        hashes
    );
    assert_eq!(chain.genesis().encode_wire().unwrap(), source_genesis);
    assert_eq!(
        chain.committed(2).block().encode_wire().unwrap(),
        source_tip
    );
    let (retried, checked) =
        relation_counts::measure(|| capture(&view, chain, [44; 32], status(chain)));
    let retried = retried.unwrap();
    assert_eq!(checked.frames, vec![1, 1, 2]);
    assert_eq!(
        checked.qcs,
        vec![2],
        "a new attempt verifies its own actual certificate"
    );
    assert_eq!(
        retried.body.genesis_finality_proof,
        original.body.genesis_finality_proof
    );
    assert_eq!(retried.body.finality_proof, original.body.finality_proof);
    assert_eq!(retried.body.challenge, [44; 32]);
    retried.verify().unwrap();
    assert_eq!(budget.reserved_bytes(), reserved);
}

#[test]
fn attestation_checks_original_tip_certificate_before_status_instance_and_height_errors() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    chain.commit(Vec::new());
    let mut invalid_instance = status(&chain);
    invalid_instance.instance[0] ^= 1;
    let mut invalid_height = status(&chain);
    invalid_height.committed_height -= 1;
    invalid_height.applied_height -= 1;
    invalid_height.height -= 1;
    {
        let view = chain.state().view();
        let (result, checked) =
            relation_counts::measure(|| capture(&view, &chain, [45; 32], invalid_instance.clone()));
        assert!(matches!(result, Err(AttestationBuildError::InvalidStatus)));
        assert_eq!(checked.frames, vec![1, 1, 2]);
        assert_eq!(checked.qcs, vec![2]);
        assert!(matches!(
            capture(&view, &chain, [45; 32], invalid_height.clone()),
            Err(AttestationBuildError::StatusHeightMismatch)
        ));
    }
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    let view = chain.state().view();
    for status in [invalid_instance, invalid_height] {
        assert!(
            matches!(
                capture(&view, &chain, [45; 32], status),
                Err(AttestationBuildError::FinalityProof(ProofError::Chain(
                    ChainReadError::Certificate { height: 2, .. }
                )))
            ),
            "an invalid native original still refuses before later status checks"
        );
    }
}
