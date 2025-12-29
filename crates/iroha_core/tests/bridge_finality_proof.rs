//! Bridge finality proof construction/roundtrip.
#![allow(clippy::expect_used)]

use iroha_core::{
    bridge::{
        BridgeFinalityError, BridgeFinalityVerificationError, FinalityProofVerificationConfig,
        build_finality_proof, verify_finality_proof,
    },
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
    sumeragi::status::{
        record_commit_certificate, reset_commit_certs_for_tests, set_commit_cert_history_cap,
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair, SignatureOf};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, BlockSignature, builder::BlockBuilder},
    bridge::BridgeFinalityProof,
    consensus::{CommitCertificate, VALIDATOR_SET_HASH_VERSION_V1},
    peer::PeerId,
};
use std::{
    num::NonZeroU64,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

const DEFAULT_COMMIT_CERT_HISTORY_CAP: usize = 512;
static FINALITY_TEST_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn lock_finality_tests() -> MutexGuard<'static, ()> {
    FINALITY_TEST_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

struct CommitCertHistoryGuard;

impl CommitCertHistoryGuard {
    fn with_cap(cap: usize) -> Self {
        reset_commit_certs_for_tests();
        set_commit_cert_history_cap(cap);
        Self
    }
}

impl Drop for CommitCertHistoryGuard {
    fn drop(&mut self) {
        reset_commit_certs_for_tests();
        set_commit_cert_history_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    }
}

fn build_proof_with_validators(
    validators: &[KeyPair],
) -> (BridgeFinalityProof, ChainId, HashOf<Vec<PeerId>>) {
    let peer_ids: Vec<_> = validators
        .iter()
        .map(|kp| PeerId::new(kp.public_key().clone()))
        .collect();

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let builder = BlockBuilder::new(header);
    let block = builder.build_with_signature(0, validators[0].private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let world = World::new();
    let state = State::new_for_testing(world, kura, query_handle);
    let chain_id = state.view().chain_id().clone();

    let signature = SignatureOf::from_hash(validators[0].private_key(), block_hash);
    let validator_set_hash = HashOf::new(&peer_ids);
    let cert = CommitCertificate {
        height: 1,
        block_hash,
        view: 0,
        epoch: 0,
        validator_set_hash,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: peer_ids.clone(),
        signatures: vec![BlockSignature::new(0, signature)],
    };
    record_commit_certificate(cert);

    let view = state.view();
    let proof = build_finality_proof(&view, 1).expect("finality proof");

    (proof, chain_id, validator_set_hash)
}

#[test]
fn builds_finality_proof_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = iroha_crypto::KeyPair::random();
    let peer_id = PeerId::new(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    let block = builder.build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = iroha_core::query::store::LiveQueryStore::start_test();
    let world = iroha_core::state::World::new();
    let state = State::new_for_testing(world, kura, query_handle);

    // Record commit certificate matching the stored block.
    let signature = SignatureOf::from_hash(kp.private_key(), block_hash);
    let cert = CommitCertificate {
        height: 1,
        block_hash,
        view: 0,
        epoch: 0,
        validator_set_hash: iroha_crypto::HashOf::new(&vec![peer_id.clone()]),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: vec![peer_id],
        signatures: vec![iroha_data_model::block::BlockSignature::new(0, signature)],
    };
    record_commit_certificate(cert.clone());

    let view = state.view();
    let proof = build_finality_proof(&view, 1).expect("finality proof");

    assert_eq!(proof.height, 1);
    assert_eq!(proof.chain_id, *view.chain_id());
    assert_eq!(proof.block_hash, block_hash);
    assert_eq!(proof.commit_certificate, cert);
    assert_eq!(proof.block_header.hash(), block_hash);
}

#[test]
fn finality_proof_rejects_commit_certificate_hash_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = KeyPair::random();
    let peer_id = PeerId::new(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let builder = BlockBuilder::new(header);
    let block = builder.build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let world = World::new();
    let state = State::new_for_testing(world, kura, query_handle);

    // Commit certificate points at a forged hash that disagrees with storage.
    let forged_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xD1; Hash::LENGTH]));
    let validator_set = vec![peer_id.clone()];
    let cert = CommitCertificate {
        height: 1,
        block_hash: forged_hash,
        view: 0,
        epoch: 0,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        signatures: vec![BlockSignature::new(
            0,
            SignatureOf::from_hash(kp.private_key(), forged_hash),
        )],
    };
    record_commit_certificate(cert);

    let view = state.view();
    match build_finality_proof(&view, 1).unwrap_err() {
        BridgeFinalityError::CommitCertificateHashMismatch {
            height,
            cert_hash,
            block_hash: stored,
        } => {
            assert_eq!(height, 1);
            assert_eq!(cert_hash, forged_hash);
            assert_eq!(stored, block_hash);
        }
        other => panic!("expected hash mismatch error, got {other:?}"),
    }
}

#[test]
fn finality_proof_respects_commit_certificate_retention_cap() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(2);

    let kp = KeyPair::random();
    let peer_id = PeerId::new(kp.public_key().clone());
    let validator_set = vec![peer_id.clone()];
    let validator_set_hash = HashOf::new(&validator_set);

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura.clone(), query);

    let mut parent = None;
    for height in 1..=3 {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero"),
            parent,
            None,
            None,
            0,
            0,
        );
        let builder = BlockBuilder::new(header);
        let block = builder.build_with_signature(0, kp.private_key());
        let block_hash = block.hash();
        parent = Some(block_hash);
        kura.store_block(block).expect("store block");

        let signature = SignatureOf::from_hash(kp.private_key(), block_hash);
        let cert = CommitCertificate {
            height,
            block_hash,
            view: 0,
            epoch: 0,
            validator_set_hash,
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: validator_set.clone(),
            signatures: vec![BlockSignature::new(0, signature)],
        };
        record_commit_certificate(cert);
    }

    let view = state.view();
    assert!(matches!(
        build_finality_proof(&view, 1),
        Err(BridgeFinalityError::CommitCertificateNotFound(1))
    ));
    build_finality_proof(&view, 2).expect("recent proof should be retained");
    build_finality_proof(&view, 3).expect("newest proof should be retained");
}

#[test]
fn builds_finality_bundle_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let kp = iroha_crypto::KeyPair::random();
    let peer_id = PeerId::new(kp.public_key().clone());

    let genesis = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let builder = iroha_data_model::block::builder::BlockBuilder::new(genesis);
    let genesis_block = builder.build_with_signature(0, kp.private_key());

    let header2 = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        Some(genesis_block.hash()),
        None,
        None,
        0,
        0,
    );
    let builder = iroha_data_model::block::builder::BlockBuilder::new(header2);
    let block = builder.build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    kura.store_block(genesis_block)
        .expect("store genesis block");
    kura.store_block(block).expect("store block");
    let query_handle = iroha_core::query::store::LiveQueryStore::start_test();
    let world = iroha_core::state::World::new();
    let state = iroha_core::state::State::new_for_testing(world, kura, query_handle);

    let signature = SignatureOf::from_hash(kp.private_key(), block_hash);
    let validator_set = vec![peer_id.clone()];
    let cert = CommitCertificate {
        height: 2,
        block_hash,
        view: 0,
        epoch: 0,
        validator_set_hash: iroha_crypto::HashOf::new(&validator_set),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: validator_set.clone(),
        signatures: vec![iroha_data_model::block::BlockSignature::new(0, signature)],
    };
    record_commit_certificate(cert.clone());

    let view = state.view();
    let bundle = iroha_core::bridge::build_finality_bundle(&view, 2).expect("finality bundle");

    assert_eq!(bundle.commitment.block_height, 2);
    assert_eq!(bundle.commitment.block_hash, block_hash);
    assert_eq!(bundle.commitment.authority_set.validator_set, validator_set);
    assert_eq!(
        bundle.commitment.authority_set.validator_set_hash,
        cert.validator_set_hash
    );
    assert_eq!(bundle.commitment.authority_set.id, 2);
    assert_eq!(bundle.block_header.hash(), block_hash);
    assert_eq!(bundle.commit_certificate, cert);
    assert_eq!(bundle.justification.signatures.len(), 1);
    // MMR root reflects bag-of-peaks over blocks 1..=2; should not equal the leaf hash.
    assert!(bundle.commitment.mmr_root.is_some());
    assert_eq!(bundle.commitment.mmr_leaf_index, Some(1));
}

#[test]
fn verify_finality_proof_accepts_valid_payload() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [KeyPair::random()];

    let (proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let config = FinalityProofVerificationConfig {
        expected_chain_id: &chain_id,
        expected_height: Some(proof.height),
        trusted_validator_set_hash: Some(validator_set_hash),
    };

    verify_finality_proof(&proof, &config).expect("proof should verify");
}

#[test]
fn verify_finality_proof_rejects_chain_id_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [KeyPair::random()];

    let (proof, _chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let wrong_chain: ChainId = "iroha:different-chain".parse().expect("chain id parses");
    let config = FinalityProofVerificationConfig {
        expected_chain_id: &wrong_chain,
        expected_height: Some(proof.height),
        trusted_validator_set_hash: Some(validator_set_hash),
    };

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::ChainIdMismatch { actual, .. } => {
            assert_eq!(actual, proof.chain_id);
        }
        other => panic!("expected chain-id mismatch error, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_height_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [KeyPair::random()];

    let (proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let config = FinalityProofVerificationConfig {
        expected_chain_id: &chain_id,
        expected_height: Some(proof.height + 1),
        trusted_validator_set_hash: Some(validator_set_hash),
    };

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::HeightMismatch { expected, actual } => {
            assert_eq!(expected, proof.height + 1);
            assert_eq!(actual, proof.height);
        }
        other => panic!("expected height mismatch error, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_trusted_roster_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [KeyPair::random()];

    let (proof, chain_id, advertised_hash) = build_proof_with_validators(&validators);
    let other_peer = PeerId::new(KeyPair::random().public_key().clone());
    let trusted_hash = HashOf::new(&vec![other_peer]);
    let config = FinalityProofVerificationConfig {
        expected_chain_id: &chain_id,
        expected_height: Some(proof.height),
        trusted_validator_set_hash: Some(trusted_hash),
    };

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::TrustedValidatorSetHashMismatch {
            trusted,
            advertised,
        } => {
            assert_eq!(trusted, trusted_hash);
            assert_eq!(advertised, advertised_hash);
        }
        other => panic!("expected trusted roster mismatch error, got {other:?}"),
    }
}
