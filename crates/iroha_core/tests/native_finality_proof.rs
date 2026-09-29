//! Native embedded-certificate serving and independently pinned verification.
use iroha_core::{
    state::World,
    sumeragi::{
        finality::{build_bundle, build_checkpoint, build_proof},
        test_chain::{CertifiedTestChain, Signers, TestChainConfig},
    },
};
use iroha_crypto::Algorithm;
use iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier;

fn chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain
}

#[test]
fn builder_returns_exact_native_frame_and_authenticated_committee() {
    let chain = chain();
    let view = chain.state().view();
    let proof = build_proof(&view, 2).unwrap();
    assert_eq!(proof.block_header, chain.committed(2).block().header());
    assert_eq!(
        proof.block_wire,
        chain.committed(2).block().encode_wire().unwrap()
    );
    assert_eq!(proof.committee.len(), 4);
    for member in &proof.committee {
        assert_eq!(
            member.public_key.try_algorithm().unwrap(),
            Algorithm::BlsNormal
        );
        assert!(!member.proof_of_possession.is_empty());
    }
    let bundle = build_bundle(&view, 2).unwrap();
    assert_eq!(bundle.network_id, chain.network_id());
    assert_eq!(bundle.finality_proof, proof);
    assert!(!chain.kura().store_root().join("v2_finality").exists());
}

#[test]
fn absent_heights_and_subquorum_certificates_fail_closed() {
    let chain = chain();
    let view = chain.state().view();
    for height in [0, 3] {
        assert!(build_proof(&view, height).is_err());
        assert!(build_bundle(&view, height).is_err());
    }
    let mut invalid = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    invalid.commit_with(None, Vec::new(), Signers::BelowQuorum);
    assert!(build_proof(&invalid.state().view(), 2).is_err());
}

#[test]
fn verifier_rejects_header_frame_and_possession_substitution() {
    let chain = chain();
    let view = chain.state().view();
    let root = build_checkpoint(&view, 1).unwrap();
    let proof = build_proof(&view, 2).unwrap();
    for attack in 0..4 {
        let mut changed = proof.clone();
        match attack {
            0 => changed.block_header = chain.genesis().header(),
            1 => changed.block_wire.clear(),
            2 => changed.committee[0].proof_of_possession[0] ^= 1,
            3 => changed.committee.swap(0, 1),
            _ => unreachable!(),
        }
        let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &root,
            &chain.network_id(),
            "sumeragi-certified-test-chain",
        )
        .unwrap();
        assert!(verifier.verify(&changed).is_err(), "attack {attack}");
        assert!(
            verifier.verify(&proof).is_ok(),
            "rejection must not advance the prefix"
        );
    }
}

#[test]
fn verifier_requires_the_independently_selected_instance_and_next_height() {
    let chain = chain();
    let view = chain.state().view();
    let genesis = build_proof(&view, 1).unwrap();
    let proof = build_proof(&view, 2).unwrap();
    let mut unanchored =
        SumeragiFinalityVerifier::new(chain.genesis(), "foreign-chain", genesis.committee.clone())
            .unwrap();
    unanchored.verify(&genesis).unwrap();
    assert!(unanchored.verify(&proof).is_err());
    let checkpoint = build_checkpoint(&view, 2).unwrap();
    let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        &chain.network_id(),
        "sumeragi-certified-test-chain",
    )
    .unwrap();
    assert!(verifier.verify(&proof).is_err());
    assert!(
        verifier
            .verify_same_decision(checkpoint.tip(), &proof)
            .is_ok()
    );
}
