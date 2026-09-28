//! Bridge finality proof and attestation regression tests.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use std::num::NonZeroU64;

fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("bridge fixture key generation should succeed")
}
fn checked_bls_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("bridge BLS fixture key generation should succeed")
}
fn blank_state() -> CoreState {
    CoreState::new_for_testing(
        crate::state::World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    )
}
#[test]
fn finality_attestation_requires_exact_state_view_tip_hash() {
    let committed_tip = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero height"),
        None,
        None,
        0,
        0,
    )
    .hash();
    let another_tip = BlockHeader::new(
        NonZeroU64::new(3).expect("non-zero height"),
        Some(committed_tip),
        None,
        0,
        0,
    )
    .hash();
    require_finality_proof_at_committed_tip(committed_tip, committed_tip)
        .expect("exact tip must bind");
    assert!(matches!(
        require_finality_proof_at_committed_tip(committed_tip, another_tip),
        Err(BridgeFinalityAttestationBuildError::FinalityTipMismatch {
            committed_tip_hash,
            proof_block_hash,
        }) if committed_tip_hash == committed_tip && proof_block_hash == another_tip
    ));
}
#[test]
fn finality_attestation_fails_closed_when_requested_tip_races_state_view() {
    require_exact_durable_tip_height(7, 7).expect("same immutable tip height");
    assert!(matches!(
        require_exact_durable_tip_height(7, 8),
        Err(BridgeFinalityAttestationBuildError::HeightIsNotDurableTip {
            requested: 7,
            committed: 8,
        })
    ));
}
#[test]
fn finality_attestation_requires_exact_state_view_genesis_hash() {
    let committed_genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        0,
        0,
    )
    .hash();
    let substituted_genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        1,
        0,
    )
    .hash();
    require_finality_proof_at_committed_genesis(committed_genesis, committed_genesis)
        .expect("exact genesis must bind");
    assert!(matches!(
        require_finality_proof_at_committed_genesis(
            committed_genesis,
            substituted_genesis,
        ),
        Err(BridgeFinalityAttestationBuildError::GenesisFinalityMismatch {
            committed_genesis_hash,
            proof_block_hash,
        }) if committed_genesis_hash == committed_genesis
            && proof_block_hash == substituted_genesis
    ));
}
#[test]
fn checked_keypair_helpers_preserve_requested_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    assert_eq!(checked_bls_keypair().algorithm(), Algorithm::BlsNormal);
}
#[test]
fn finality_builder_rejects_zero_and_unretained_heights() {
    let state = blank_state();
    assert_eq!(
        build_finality_proof(&state, 0),
        Err(BridgeFinalityError::InvalidHeight(0))
    );
    assert_eq!(
        build_finality_proof(&state, 2),
        Err(BridgeFinalityError::FinalityArtifactNotFound(2))
    );
    assert_eq!(
        build_finality_bundle(&state, 2),
        Err(BridgeFinalityError::FinalityArtifactNotFound(2))
    );
}
