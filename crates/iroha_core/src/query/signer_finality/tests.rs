//! Native State/Kura association tests with real, software-signed three-of-four BLS fixtures.
//!
//! The fixture signs exact complete block bytes with revision-4 RS16 contexts. These tests
//! exercise the production durable reader; they do not qualify a live validator deployment,
//! certify the fixture application-state roots, or simulate physical signer custody.

use super::{SignerFinalityErrorV1, verify_signer_finality_v1};
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, builder::BlockBuilder},
};
use iroha_sccp::{
    SCCP_TAIRA_CHAIN_ID_V1, SccpFinalizedBlockTestFixtureV1,
    sccp_finalize_taira_block_test_fixture_v1, sccp_taira_finality_network_id_v1,
};
use std::sync::Arc;

fn native_state(kura: Arc<Kura>) -> State {
    State::new_with_chain_and_network_id_for_testing(
        World::default(),
        kura,
        LiveQueryStore::start_test(),
        SCCP_TAIRA_CHAIN_ID_V1.parse().expect("fixture chain"),
        sccp_taira_finality_network_id_v1(),
    )
}

fn finalized_block() -> SccpFinalizedBlockTestFixtureV1 {
    let key = KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519)
        .expect("local block fixture key");
    let header = BlockHeader::new(
        1_u64.try_into().expect("positive height"),
        None,
        None,
        1_700_000_000_002,
        0,
    );
    let mut block = BlockBuilder::new(header)
        .try_build_with_signature(0, key.private_key())
        .expect("sign empty block fixture");
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &iroha_data_model::parameter::ExecutionOutputPolicyV1::bootstrap().limits(),
        )
        .expect("attach complete empty execution outputs");
    let finalized = sccp_finalize_taira_block_test_fixture_v1(&block, None);
    let artifact = &finalized.proof().finality_artifact;
    assert_eq!(artifact.height_context.roster.len(), 4);
    assert_eq!(artifact.commit_qc.signers.len(), 3);
    assert_eq!(
        artifact.height_context.da_layout.encoding,
        iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16
    );
    artifact.verify().expect("real BLS Commit certificate");
    finalized
}

fn stage_native_block(state: &mut State, finalized: &SccpFinalizedBlockTestFixtureV1) {
    state
        .kura()
        .store_block(Arc::new(finalized.block().clone()))
        .expect("persist exact complete block");
    state.push_block_hash_for_testing(finalized.block().hash());
}

fn persist_finality(state: &State, finalized: &SccpFinalizedBlockTestFixtureV1) {
    let artifact = &finalized.proof().finality_artifact;
    let receipt = state
        .kura()
        .store_v2_finality_artifact(artifact)
        .expect("persist cryptographically valid exact finality");
    assert_eq!(receipt.height(), artifact.height);
    assert_eq!(receipt.block_hash(), artifact.block_hash);
    assert_eq!(
        state
            .kura()
            .v2_finality_artifact(artifact.height)
            .expect("read authenticated durable finality")
            .as_ref(),
        Some(artifact)
    );
}

#[test]
fn invalid_or_absent_coordinates_fail_closed() {
    let state = native_state(Kura::blank_kura_for_testing());
    for (height, hash) in [
        (0, [1; 32]),
        (1, [0; 32]),
        (1, [1; 32]),
        (u64::MAX, [1; 32]),
    ] {
        assert_eq!(
            verify_signer_finality_v1(&state.view(), height, hash),
            Err(SignerFinalityErrorV1)
        );
    }
    assert_eq!(
        SignerFinalityErrorV1.to_string(),
        "signer finality is unavailable for the exact native block"
    );
}

#[test]
fn native_hash_cache_without_a_durable_block_is_not_finality() {
    let mut state = native_state(Kura::blank_kura_for_testing());
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"cache-only negative"));
    state.push_block_hash_for_testing(hash);
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, *hash.as_ref()),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn exact_native_block_requires_the_durable_authenticated_certificate() {
    let finalized = finalized_block();
    let mut state = native_state(Kura::blank_kura_for_testing());
    stage_native_block(&mut state, &finalized);
    let hash = *finalized.block().hash().as_ref();
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, hash),
        Err(SignerFinalityErrorV1)
    );
    persist_finality(&state, &finalized);
    let verified = verify_signer_finality_v1(&state.view(), 1, hash)
        .expect("exact State/Kura block and real durable certificate");
    assert_eq!(verified.height(), 1);
    assert_eq!(verified.block_hash(), hash);
    let mut wrong_hash = hash;
    wrong_hash[0] ^= 1;
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, wrong_hash),
        Err(SignerFinalityErrorV1)
    );
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 2, hash),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn durable_certificate_cannot_substitute_for_the_same_state_history() {
    let finalized = finalized_block();
    let kura = Kura::blank_kura_for_testing();
    let mut state = native_state(Arc::clone(&kura));
    stage_native_block(&mut state, &finalized);
    persist_finality(&state, &finalized);
    let hash = *finalized.block().hash().as_ref();
    let mut other_state = native_state(kura);
    assert_eq!(
        verify_signer_finality_v1(&other_state.view(), 1, hash),
        Err(SignerFinalityErrorV1)
    );
    let other_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other native view"));
    other_state.push_block_hash_for_testing(other_hash);
    assert_eq!(
        verify_signer_finality_v1(&other_state.view(), 1, hash),
        Err(SignerFinalityErrorV1)
    );
    assert_eq!(
        verify_signer_finality_v1(&other_state.view(), 1, *other_hash.as_ref()),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn identical_block_and_certificate_cannot_authorize_a_foreign_state_network() {
    let finalized = finalized_block();
    let kura = Kura::blank_kura_for_testing();
    let mut state = native_state(Arc::clone(&kura));
    stage_native_block(&mut state, &finalized);
    persist_finality(&state, &finalized);
    let hash = *finalized.block().hash().as_ref();
    verify_signer_finality_v1(&state.view(), 1, hash).expect("exact original network");
    let foreign_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"different State network with identical retained block and certificate",
    )));
    assert_ne!(
        foreign_network,
        finalized
            .proof()
            .finality_artifact
            .height_context
            .network_id
    );
    // A State cannot relabel storage already bound to another exact network.
    assert!(kura.bind_lane_storage_network(foreign_network).is_err());
    verify_signer_finality_v1(&state.view(), 1, hash)
        .expect("failed rebind preserves the original authenticated network");
    // Independently bound storage can retain the same cryptographically valid source
    // bytes. The signer reader must still join their network to its own State.
    let mut foreign_state = State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        SCCP_TAIRA_CHAIN_ID_V1
            .parse()
            .expect("same operator-selected chain label"),
        foreign_network,
    );
    stage_native_block(&mut foreign_state, &finalized);
    persist_finality(&foreign_state, &finalized);
    assert!(
        foreign_state
            .view()
            .block_hashes()
            .iter()
            .eq(state.view().block_hashes().iter()),
        "both States retain the same ordered block history"
    );
    assert_eq!(
        verify_signer_finality_v1(&foreign_state.view(), 1, hash),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn invalid_commit_signature_cannot_create_durable_authority() {
    let finalized = finalized_block();
    let mut state = native_state(Kura::blank_kura_for_testing());
    stage_native_block(&mut state, &finalized);
    let mut corrupted = finalized.proof().finality_artifact.clone();
    corrupted.commit_qc.aggregate_signature[0] ^= 1;
    assert!(corrupted.verify().is_err());
    assert!(state.kura().store_v2_finality_artifact(&corrupted).is_err());
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, *finalized.block().hash().as_ref()),
        Err(SignerFinalityErrorV1)
    );
}

#[test]
fn corrupt_retained_finality_fails_after_a_successful_read() {
    let finalized = finalized_block();
    let mut state = native_state(Kura::blank_kura_for_testing());
    stage_native_block(&mut state, &finalized);
    persist_finality(&state, &finalized);
    let hash = *finalized.block().hash().as_ref();
    verify_signer_finality_v1(&state.view(), 1, hash).expect("initial valid native read");
    std::fs::write(
        state.kura().v2_finality_artifact_path_for_testing(1),
        b"malformed finality negative",
    )
    .expect("corrupt isolated fixture sidecar");
    assert_eq!(
        verify_signer_finality_v1(&state.view(), 1, hash),
        Err(SignerFinalityErrorV1)
    );
}
