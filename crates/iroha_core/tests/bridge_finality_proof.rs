//! Bridge finality proof construction/roundtrip.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::expect_used)]

use std::{
    collections::BTreeSet,
    num::NonZeroU64,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

use iroha_core::sumeragi::consensus::{
    PERMISSIONED_TAG, Phase, ValidatorIndex, Vote, vote_preimage,
};
use iroha_core::{
    bridge::{
        BridgeFinalityError, BridgeFinalityVerificationError, FinalityProofVerificationConfig,
        build_finality_bundle, build_finality_proof, verify_finality_proof,
    },
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
    sumeragi::status::{
        record_commit_qc, reset_commit_certs_for_tests, set_commit_cert_history_cap,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, builder::BlockBuilder},
    bridge::BridgeFinalityProof,
    consensus::{Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
    peer::PeerId,
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

fn build_signers_bitmap(signers: &BTreeSet<ValidatorIndex>, roster_len: usize) -> Vec<u8> {
    if roster_len == 0 {
        return Vec::new();
    }
    let mut bitmap = vec![0u8; roster_len.div_ceil(8)];
    for signer in signers {
        let Ok(idx) = usize::try_from(*signer) else {
            continue;
        };
        if idx >= roster_len {
            continue;
        }
        let byte = idx / 8;
        let bit = idx % 8;
        bitmap[byte] |= 1u8 << bit;
    }
    bitmap
}

#[allow(clippy::too_many_arguments)]
fn aggregate_signature_for_signers(
    chain_id: &ChainId,
    mode_tag: &str,
    phase: Phase,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    signers: &BTreeSet<ValidatorIndex>,
    keypairs: &[KeyPair],
) -> Vec<u8> {
    if signers.is_empty() {
        return Vec::new();
    }
    let vote = Vote {
        phase,
        block_hash,
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height,
        view,
        epoch,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(chain_id, mode_tag, &vote);
    let mut signatures = Vec::with_capacity(signers.len());
    for signer in signers {
        let idx = usize::try_from(*signer).expect("signer index fits");
        let kp = keypairs.get(idx).expect("signer keypair");
        let sig = Signature::new(kp.private_key(), &preimage);
        signatures.push(sig.payload().to_vec());
    }
    let sig_refs: Vec<&[u8]> = signatures.iter().map(Vec::as_slice).collect();
    iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signature")
}

fn build_commit_qc(
    chain_id: &ChainId,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    peer_ids: &[PeerId],
    keypairs: &[KeyPair],
) -> Qc {
    let signers: BTreeSet<_> = (0..peer_ids.len())
        .filter_map(|idx| ValidatorIndex::try_from(idx).ok())
        .collect();
    let signers_bitmap = build_signers_bitmap(&signers, peer_ids.len());
    let aggregate_signature = aggregate_signature_for_signers(
        chain_id,
        PERMISSIONED_TAG,
        Phase::Commit,
        block_hash,
        height,
        view,
        epoch,
        &signers,
        keypairs,
    );
    let validator_set = peer_ids.to_vec();
    let validator_set_hash = HashOf::new(&validator_set);
    Qc {
        phase: Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height,
        view,
        epoch,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap,
            bls_aggregate_signature: aggregate_signature,
        },
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

    let validator_set_hash = HashOf::new(&peer_ids);
    let cert = build_commit_qc(&chain_id, block_hash, 1, 0, 0, &peer_ids, validators);
    record_commit_qc(cert);

    let view = state.view();
    let proof = build_finality_proof(&view, 1).expect("finality proof");

    (proof, chain_id, validator_set_hash)
}

#[test]
fn builds_finality_proof_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
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
    let validator_set = vec![peer_id.clone()];
    let keypairs = vec![kp.clone()];
    let cert = build_commit_qc(
        &state.view().chain_id().clone(),
        block_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert.clone());

    let view = state.view();
    let proof = build_finality_proof(&view, 1).expect("finality proof");

    assert_eq!(proof.height, 1);
    assert_eq!(proof.chain_id, *view.chain_id());
    assert_eq!(proof.block_hash, block_hash);
    assert_eq!(proof.commit_qc, cert);
    assert_eq!(proof.block_header.hash(), block_hash);
}

#[test]
fn finality_proof_rejects_commit_qc_hash_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
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
    let keypairs = vec![kp.clone()];
    let cert = build_commit_qc(
        &state.view().chain_id().clone(),
        forged_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert);

    let view = state.view();
    match build_finality_proof(&view, 1).unwrap_err() {
        BridgeFinalityError::QcHashMismatch {
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
fn finality_proof_respects_commit_qc_retention_cap() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(2);

    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let peer_id = PeerId::new(kp.public_key().clone());
    let validator_set = vec![peer_id.clone()];
    let keypairs = vec![kp.clone()];

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

        let cert = build_commit_qc(
            &state.view().chain_id().clone(),
            block_hash,
            height,
            0,
            0,
            &validator_set,
            &keypairs,
        );
        record_commit_qc(cert);
    }

    let view = state.view();
    assert!(matches!(
        build_finality_proof(&view, 1),
        Err(BridgeFinalityError::QcNotFound(1))
    ));
    build_finality_proof(&view, 2).expect("recent proof should be retained");
    build_finality_proof(&view, 3).expect("newest proof should be retained");
}

#[test]
fn builds_finality_bundle_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
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

    let validator_set = vec![peer_id.clone()];
    let keypairs = vec![kp.clone()];
    let cert = build_commit_qc(
        &state.view().chain_id().clone(),
        block_hash,
        2,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert.clone());

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
    assert_eq!(bundle.commit_qc, cert);
    assert_eq!(bundle.justification.signatures.len(), 1);
    // MMR root reflects bag-of-peaks over blocks 1..=2; should not equal the leaf hash.
    assert!(bundle.commitment.mmr_root.is_some());
    assert_eq!(bundle.commitment.mmr_leaf_index, Some(1));
}

#[test]
fn finality_bundle_rebuilds_mmr_after_top_block_replace() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let kp = KeyPair::random();
    let peer_id = PeerId::new(kp.public_key().clone());

    let genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
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

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(genesis_block.clone())
        .expect("store genesis block");
    kura.store_block(block.clone()).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let world = World::new();
    let state = State::new_for_testing(world, kura.clone(), query_handle);

    let validator_set = vec![peer_id.clone()];
    let keypairs = vec![kp.clone()];
    let cert = build_commit_qc(
        &state.view().chain_id().clone(),
        block_hash,
        2,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert);

    let view = state.view();
    let bundle = build_finality_bundle(&view, 2).expect("finality bundle");
    let root_before = bundle.commitment.mmr_root;

    let header2_replacement = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        Some(genesis_block.hash()),
        None,
        None,
        1,
        0,
    );
    let builder = iroha_data_model::block::builder::BlockBuilder::new(header2_replacement);
    let replacement = builder.build_with_signature(0, kp.private_key());
    let replacement_hash = replacement.hash();
    kura.replace_top_block(replacement)
        .expect("replace top block");

    let cert_replacement = build_commit_qc(
        &state.view().chain_id().clone(),
        replacement_hash,
        2,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert_replacement);

    let refreshed = build_finality_bundle(&view, 2).expect("refreshed bundle");
    assert_eq!(refreshed.commitment.block_hash, replacement_hash);
    assert_ne!(root_before, refreshed.commitment.mmr_root);
}

#[test]
fn verify_finality_proof_accepts_valid_payload() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [KeyPair::random_with_algorithm(Algorithm::BlsNormal)];

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
    let validators = [KeyPair::random_with_algorithm(Algorithm::BlsNormal)];

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
    let validators = [KeyPair::random_with_algorithm(Algorithm::BlsNormal)];

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
    let validators = [KeyPair::random_with_algorithm(Algorithm::BlsNormal)];

    let (proof, chain_id, advertised_hash) = build_proof_with_validators(&validators);
    let other_peer = PeerId::new(
        KeyPair::random_with_algorithm(Algorithm::BlsNormal)
            .public_key()
            .clone(),
    );
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
