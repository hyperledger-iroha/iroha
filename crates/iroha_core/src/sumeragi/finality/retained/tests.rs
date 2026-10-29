//! Retained current attestations: bounded request reads, unchanged signed bodies and
//! the stated historical-corruption interval.

use super::*;
use crate::{
    state::World,
    sumeragi::{
        certified_chain::relation_counts,
        finality::{AttestationBuildError, build_attestation},
        node::NodeIdentity,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    sumeragi::SumeragiStatus,
    sumeragi_finality::{CertifiedResultScope, SumeragiFinalityAttestation},
};
use norito::codec::Encode as _;

fn installed_signer() -> KeyPair {
    KeyPair::from_seed(vec![0xC1; 32], Algorithm::BlsNormal)
}

fn status_at(chain: &CertifiedTestChain, height: u64) -> SumeragiStatus {
    SumeragiStatus {
        protocol_version: iroha_data_model::sumeragi::PROTOCOL_VERSION,
        config_fingerprint: Hash::new(b"exact attestation proof reader configuration"),
        beacon_horizon: None,
        instance: chain.instance().0,
        height: height + 1,
        view: 0,
        stage: 0,
        leader: None,
        proxy_tail: None,
        high_qc_view: None,
        level: 0,
        start_level: 0,
        t_retx_ms: 100,
        committed_height: height,
        applied_height: height,
        awaiting: false,
        signer: Some(installed_signer().public_key().clone()),
        unanchored: false,
        abstaining: false,
        halted: None,
        footprint: iroha_data_model::sumeragi::SumeragiFootprint::default(),
    }
}

fn attest(
    view: &impl StateReadOnly,
    status: SumeragiStatus,
    height: u64,
    challenge: [u8; 32],
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
        height,
        challenge,
        &signer,
    )
}

fn attest_tip(
    chain: &CertifiedTestChain,
    challenge: [u8; 32],
) -> Result<SumeragiFinalityAttestation, AttestationBuildError> {
    let view = chain.state().view();
    attest(
        &view,
        status_at(chain, chain.height()),
        chain.height(),
        challenge,
    )
}

fn chain_at(height: u64) -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
    while chain.height() < height {
        chain.commit(Vec::new());
    }
    chain
}

fn minted(chain: &CertifiedTestChain) -> RetainedAttestationProducer {
    let mut producer = RetainedAttestationProducer::attach(Arc::clone(chain.state())).unwrap();
    producer.step();
    assert!(
        chain.kura().retained_attestation().current().is_some(),
        "one complete walk mints the authority"
    );
    producer
}

fn same_body_except_fresh_clock(
    actual: &SumeragiFinalityAttestation,
    original: &SumeragiFinalityAttestation,
) {
    actual.verify(CertifiedResultScope::TairaGlobal).unwrap();
    original.verify(CertifiedResultScope::TairaGlobal).unwrap();
    let mut expected = original.body.clone();
    expected.observed_at_unix_ms = actual.body.observed_at_unix_ms;
    assert_eq!(actual.body.encode(), expected.encode());
}

fn expire_coverage(holder: &RetainedAttestation) {
    let mut slot = holder.published.write();
    let mut expired = Published::clone(slot.as_ref().unwrap());
    expired.coverage_expires_at = Instant::now();
    *slot = Some(Arc::new(expired));
}

#[test]
fn retained_attestation_reads_only_the_target_pair_and_signs_the_same_body() {
    let chain = chain_at(12);
    let original = attest_tip(&chain, [71; 32]).unwrap();
    let (legacy, walked) = relation_counts::measure(|| attest_tip(&chain, [71; 32]));
    legacy.unwrap();
    assert!(
        (1..=12).all(|height| walked.frames.contains(&height)),
        "without a producer every frame is still read: {:?}",
        walked.frames
    );
    let _producer = minted(&chain);
    let (actual, bounded) = relation_counts::measure(|| attest_tip(&chain, [71; 32]));
    let actual = actual.unwrap();
    eprintln!(
        "retained attestation frames={:?} qcs={:?} legacy_frames={}",
        bounded.frames,
        bounded.qcs,
        walked.frames.len()
    );
    assert!(
        bounded
            .frames
            .iter()
            .all(|height| [1, 11, 12].contains(height)),
        "only genesis and the target pair are read: {:?}",
        bounded.frames
    );
    assert_eq!(bounded.qcs, vec![12], "only the target quorum is verified");
    same_body_except_fresh_clock(&actual, &original);
}

#[test]
fn historical_corruption_is_detected_by_the_scrub_not_the_next_challenge() {
    let chain = chain_at(12);
    let mut producer = minted(&chain);
    let at = NonZeroUsize::new(6).unwrap();
    chain.kura().corrupt_native_frame_for_test(at);
    // The stated contract: a warm authority does not reread frame 6 for this challenge.
    attest_tip(&chain, [72; 32]).unwrap();
    // The next scrub pass rereads it; the authority then refuses until restart.
    producer.next_scrub = Instant::now();
    for _ in 0..4 {
        producer.step();
    }
    let holder = chain.kura().retained_attestation();
    assert!(holder.poisoned.load(Ordering::Acquire));
    assert!(attest_tip(&chain, [73; 32]).is_err());
    chain.kura().corrupt_native_frame_for_test(at);
    for _ in 0..4 {
        producer.step();
    }
    assert!(
        attest_tip(&chain, [74; 32]).is_err(),
        "repaired storage and later work never clear poison"
    );
    drop(producer);
    assert!(
        attest_tip(&chain, [75; 32]).is_err(),
        "a detached poisoned authority still refuses"
    );
}

#[test]
fn a_clean_scrub_pass_renews_expired_coverage() {
    let chain = chain_at(12);
    let mut producer = minted(&chain);
    let holder = chain.kura().retained_attestation();
    expire_coverage(holder);
    assert!(
        attest_tip(&chain, [76; 32]).is_err(),
        "expired coverage refuses"
    );
    producer.next_scrub = Instant::now();
    while producer.step() {}
    assert!(!holder.poisoned.load(Ordering::Acquire));
    attest_tip(&chain, [77; 32]).unwrap();
}

#[test]
fn a_lagging_authority_refuses_until_the_producer_advances() {
    let mut chain = chain_at(12);
    let mut producer = minted(&chain);
    for _ in 0..=MAX_LEAD {
        chain.commit(Vec::new());
    }
    assert!(
        attest_tip(&chain, [78; 32]).is_err(),
        "a view more than MAX_LEAD ahead of the published tip refuses"
    );
    producer.step();
    let holder = chain.kura().retained_attestation();
    assert_eq!(holder.current().unwrap().tip.height(), chain.height());
    attest_tip(&chain, [79; 32]).unwrap();
    // A short lead is served directly; the producer closes it on its next step.
    chain.commit(Vec::new());
    attest_tip(&chain, [80; 32]).unwrap();
}

#[test]
fn a_view_trailing_a_fresh_advance_still_resolves() {
    let mut chain = chain_at(12);
    let mut producer = minted(&chain);
    let state = Arc::clone(chain.state());
    let trailing = state.view();
    let status = status_at(&chain, 12);
    chain.commit(Vec::new());
    producer.step();
    assert_eq!(
        chain
            .kura()
            .retained_attestation()
            .current()
            .unwrap()
            .tip
            .height(),
        13
    );
    attest(&trailing, status, 12, [81; 32]).unwrap();
}

#[test]
fn a_genesis_frame_contradicting_the_retained_decision_poisons() {
    let chain = chain_at(12);
    let _producer = minted(&chain);
    let holder = chain.kura().retained_attestation();
    {
        let mut slot = holder.published.write();
        let mut tampered = Published::clone(slot.as_ref().unwrap());
        tampered.genesis.result = iroha_sumeragi::types::Hash32([0xAA; 32]);
        *slot = Some(Arc::new(tampered));
    }
    assert!(attest_tip(&chain, [82; 32]).is_err());
    assert!(holder.poisoned.load(Ordering::Acquire));
}

#[test]
fn one_producer_per_store_and_dropping_it_restores_the_complete_walk() {
    let chain = chain_at(12);
    let producer = minted(&chain);
    assert!(RetainedAttestationProducer::attach(Arc::clone(chain.state())).is_err());
    drop(producer);
    let (attested, read) = relation_counts::measure(|| attest_tip(&chain, [83; 32]));
    attested.unwrap();
    assert!(
        (1..=12).all(|height| read.frames.contains(&height)),
        "a detached store reads every frame again: {:?}",
        read.frames
    );
    let _again = minted(&chain);
}

#[test]
fn concurrent_current_attestations_share_one_retained_authority() {
    let chain = chain_at(12);
    let _producer = minted(&chain);
    let state = Arc::clone(chain.state());
    let status = status_at(&chain, 12);
    std::thread::scope(|scope| {
        // Every caller supplies a distinct nonzero challenge admitted by the signed-body contract.
        for index in 1..=32_u8 {
            let state = &state;
            let status = status.clone();
            scope.spawn(move || {
                let view = state.view();
                let attestation = attest(&view, status, 12, [index; 32]).unwrap();
                assert_eq!(attestation.body.challenge, [index; 32]);
                attestation
                    .verify(CertifiedResultScope::TairaGlobal)
                    .unwrap();
            });
        }
    });
}

#[test]
fn release_recheck_follows_the_retained_authority() {
    let chain = chain_at(12);
    assert!(
        retained_attestation_releasable(chain.state(), 12),
        "a detached store releases"
    );
    let _producer = minted(&chain);
    assert!(retained_attestation_releasable(chain.state(), 12));
    expire_coverage(chain.kura().retained_attestation());
    assert!(!retained_attestation_releasable(chain.state(), 12));
}
