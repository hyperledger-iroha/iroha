//! Bridge finality proof construction/roundtrip.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::expect_used)]

use std::{
    collections::BTreeSet,
    num::{NonZeroU64, NonZeroUsize},
    sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError},
};

use iroha_core::sumeragi::{
    consensus::{
        PERMISSIONED_TAG, Phase, ValidatorIndex, Vote, default_chain_order_hash, vote_preimage,
    },
    network_topology::commit_quorum_from_len,
};
use iroha_core::{
    bridge::{
        BridgeFinalityError, BridgeFinalityVerificationError, BridgeStateReadOnly,
        FinalityProofVerificationConfig, build_finality_bundle, build_finality_proof,
        verify_finality_proof,
    },
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, StateReadOnly, World},
    sumeragi::status::{
        record_commit_qc, reset_commit_certs_for_tests, set_commit_cert_history_cap,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    ChainId,
    block::{BlockHeader, SignedBlock, builder::BlockBuilder},
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

struct SparseBridgeState {
    chain_id: ChainId,
    blocks: Vec<(usize, Arc<SignedBlock>)>,
    pops: Vec<(PublicKey, Vec<u8>)>,
}

impl SparseBridgeState {
    fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            blocks: Vec::new(),
            pops: Vec::new(),
        }
    }

    fn with_block(mut self, height: usize, block: SignedBlock) -> Self {
        self.blocks.push((height, Arc::new(block)));
        self
    }

    fn with_validator_pop(mut self, keypair: &KeyPair) -> Self {
        let pop = iroha_crypto::bls_normal_pop_prove(keypair.private_key()).expect("validator pop");
        self.pops.push((keypair.public_key().clone(), pop));
        self
    }
}

impl BridgeStateReadOnly for SparseBridgeState {
    fn bridge_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    fn bridge_block_by_height(&self, height: NonZeroUsize) -> Option<Arc<SignedBlock>> {
        self.blocks
            .iter()
            .find(|(stored_height, _)| *stored_height == height.get())
            .map(|(_, block)| Arc::clone(block))
    }

    fn bridge_commit_qc_for_block(
        &self,
        _height: u64,
        _block_hash: HashOf<BlockHeader>,
    ) -> Option<Qc> {
        None
    }

    fn bridge_validator_pop(&self, public_key: &PublicKey) -> Option<Vec<u8>> {
        self.pops
            .iter()
            .find(|(stored_public_key, _)| stored_public_key == public_key)
            .map(|(_, pop)| pop.clone())
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

fn checked_signature(private_key: &PrivateKey, payload: &[u8]) -> Signature {
    Signature::try_new(private_key, payload).expect("test fixture signing should succeed")
}

fn checked_bls_keypair() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
        .expect("bridge finality proof fixture key generation should succeed")
}

fn deterministic_bls_validators(count: usize) -> Vec<KeyPair> {
    (0..count)
        .map(|index| {
            let seed_byte = u8::try_from(index + 1).expect("quorum fixture index fits in u8");
            KeyPair::try_from_seed(vec![seed_byte; 32], Algorithm::BlsNormal)
                .expect("deterministic bridge quorum validator key generation should succeed")
        })
        .collect()
}

fn validator_pops(validators: &[KeyPair]) -> Vec<Vec<u8>> {
    validators
        .iter()
        .map(|validator| {
            iroha_crypto::bls_normal_pop_prove(validator.private_key())
                .expect("bridge quorum validator PoP generation should succeed")
        })
        .collect()
}

#[test]
fn checked_bls_keypair_preserves_validator_algorithm() {
    assert_eq!(checked_bls_keypair().algorithm(), Algorithm::BlsNormal);
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
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(chain_id, mode_tag, &vote);
    let mut signatures = Vec::with_capacity(signers.len());
    for signer in signers {
        let idx = usize::try_from(*signer).expect("signer index fits");
        let kp = keypairs.get(idx).expect("signer keypair");
        let sig = checked_signature(kp.private_key(), &preimage);
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
    build_commit_qc_for_signers(
        chain_id, block_hash, height, view, epoch, peer_ids, keypairs, &signers,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_commit_qc_for_signers(
    chain_id: &ChainId,
    block_hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
    epoch: u64,
    peer_ids: &[PeerId],
    keypairs: &[KeyPair],
    signers: &BTreeSet<ValidatorIndex>,
) -> Qc {
    let signers_bitmap = build_signers_bitmap(signers, peer_ids.len());
    let aggregate_signature = aggregate_signature_for_signers(
        chain_id,
        PERMISSIONED_TAG,
        Phase::Commit,
        block_hash,
        height,
        view,
        epoch,
        signers,
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
        chain_order_hash: default_chain_order_hash(),
        rechain_seq: 0,
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

fn direct_finality_proof_for_signers(
    validators: &[KeyPair],
    validator_set_pops: &[Vec<u8>],
    signers: &BTreeSet<ValidatorIndex>,
) -> (BridgeFinalityProof, ChainId, HashOf<Vec<PeerId>>) {
    assert_eq!(validators.len(), validator_set_pops.len());
    let chain_id: ChainId = "iroha:bridge-quorum-policy"
        .parse()
        .expect("bridge quorum fixture chain id should parse");
    let block_header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block_hash = block_header.hash();
    let validator_set: Vec<_> = validators
        .iter()
        .map(|validator| PeerId::new(validator.public_key().clone()))
        .collect();
    let validator_set_hash = HashOf::new(&validator_set);
    let commit_qc = build_commit_qc_for_signers(
        &chain_id,
        block_hash,
        1,
        0,
        0,
        &validator_set,
        validators,
        signers,
    );
    (
        BridgeFinalityProof {
            height: 1,
            chain_id: chain_id.clone(),
            block_header,
            block_hash,
            commit_qc,
            validator_set_pops: validator_set_pops.to_vec(),
        },
        chain_id,
        validator_set_hash,
    )
}

fn signer_prefix(count: usize) -> BTreeSet<ValidatorIndex> {
    (0..count)
        .map(|index| ValidatorIndex::try_from(index).expect("quorum fixture signer index fits"))
        .collect()
}

const fn retired_bridge_quorum_from_len(len: usize) -> usize {
    if len > 3 {
        (len.saturating_sub(1) / 3) * 2 + 1
    } else {
        len
    }
}

fn assert_proof_aggregate_signature_is_valid(
    proof: &BridgeFinalityProof,
    signers: &BTreeSet<ValidatorIndex>,
) {
    let certificate = &proof.commit_qc;
    assert_eq!(
        certificate.aggregate.signers_bitmap,
        build_signers_bitmap(signers, certificate.validator_set.len()),
        "fixture bitmap must name exactly the validators that signed"
    );
    let vote = Vote {
        phase: certificate.phase,
        block_hash: certificate.subject_block_hash,
        parent_state_root: certificate.parent_state_root,
        post_state_root: certificate.post_state_root,
        height: certificate.height,
        view: certificate.view,
        epoch: certificate.epoch,
        chain_order_hash: certificate.chain_order_hash,
        rechain_seq: certificate.rechain_seq,
        highest_qc: None,
        signer: 0,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(&proof.chain_id, &certificate.mode_tag, &vote);
    let public_keys: Vec<_> = signers
        .iter()
        .map(|signer| {
            certificate.validator_set
                [usize::try_from(*signer).expect("quorum fixture signer index fits")]
            .public_key()
        })
        .collect();
    let pops: Vec<_> = signers
        .iter()
        .map(|signer| {
            proof.validator_set_pops
                [usize::try_from(*signer).expect("quorum fixture signer index fits")]
            .as_slice()
        })
        .collect();

    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.aggregate.bls_aggregate_signature,
        &public_keys,
        &pops,
    )
    .expect("adversarial quorum fixture must carry a valid BLS aggregate signature");
}

fn seed_validator_pops(world: &mut World, validators: &[KeyPair]) {
    for validator in validators {
        let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
            .expect("generate pop for validator");
        world.register_validator_pop_for_testing(validator.public_key().clone(), pop);
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
    let mut world = World::new();
    seed_validator_pops(&mut world, validators);
    let state = State::new_for_testing(world, kura, query_handle);
    let chain_id = state.view().chain_id().clone();

    let validator_set_hash = HashOf::new(&peer_ids);
    let cert = build_commit_qc(&chain_id, block_hash, 1, 0, 0, &peer_ids, validators);
    record_commit_qc(cert);

    let view = state.view();
    let proof = build_finality_proof(&view, 1).expect("finality proof");

    (proof, chain_id, validator_set_hash)
}

fn verification_config<'a>(
    chain_id: &'a ChainId,
    expected_height: Option<u64>,
    trusted_validator_set_hash: Option<HashOf<Vec<PeerId>>>,
) -> FinalityProofVerificationConfig<'a> {
    FinalityProofVerificationConfig {
        expected_chain_id: chain_id,
        expected_height,
        trusted_validator_set_hash,
    }
}

#[test]
fn builds_finality_proof_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
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
    let mut world = iroha_core::state::World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
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

    let kp = checked_bls_keypair();
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
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
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
fn finality_proof_prefers_matching_qc_when_conflicting_candidate_exists() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
    let state = State::new_for_testing(world, kura, query_handle);

    let forged_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC7; Hash::LENGTH]));
    let validator_set = vec![peer_id];
    let keypairs = vec![kp];
    let chain_id = state.view().chain_id().clone();
    let conflicting = build_commit_qc(&chain_id, forged_hash, 1, 10, 0, &validator_set, &keypairs);
    let matching = build_commit_qc(&chain_id, block_hash, 1, 0, 0, &validator_set, &keypairs);
    record_commit_qc(conflicting);
    record_commit_qc(matching.clone());

    let proof = build_finality_proof(&state.view(), 1).expect("matching proof");
    assert_eq!(proof.block_hash, block_hash);
    assert_eq!(proof.commit_qc, matching);
}

#[test]
fn finality_proof_uses_latest_qc_view_for_same_block_hash() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
    let state = State::new_for_testing(world, kura, query_handle);

    let validator_set = vec![peer_id];
    let keypairs = vec![kp];
    let chain_id = state.view().chain_id().clone();
    record_commit_qc(build_commit_qc(
        &chain_id,
        block_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    ));
    let latest = build_commit_qc(&chain_id, block_hash, 1, 7, 0, &validator_set, &keypairs);
    record_commit_qc(latest.clone());

    let proof = build_finality_proof(&state.view(), 1).expect("finality proof");
    assert_eq!(proof.commit_qc, latest);
}

#[test]
fn finality_proof_rejects_zero_height() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);

    assert!(matches!(
        build_finality_proof(&state.view(), 0),
        Err(BridgeFinalityError::InvalidHeight(0))
    ));
}

#[test]
fn finality_proof_rejects_missing_block() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);

    assert!(matches!(
        build_finality_proof(&state.view(), 1),
        Err(BridgeFinalityError::BlockNotFound(1))
    ));
}

#[test]
fn finality_proof_rejects_missing_validator_pop() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());

    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);

    let validator_set = vec![peer_id];
    let keypairs = vec![kp];
    let cert = build_commit_qc(
        &state.view().chain_id().clone(),
        block_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    );
    record_commit_qc(cert);

    assert!(matches!(
        build_finality_proof(&state.view(), 1),
        Err(BridgeFinalityError::MissingValidatorPop { index: 0 })
    ));
}

#[test]
fn historical_finality_proof_survives_process_commit_qc_retention() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(2);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());
    let validator_set = vec![peer_id.clone()];
    let keypairs = vec![kp.clone()];

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
    let mut state = State::new_for_testing(world, kura.clone(), query);

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
        record_commit_qc(cert.clone());
        state.insert_commit_qc_for_testing(block_hash, cert);
    }

    assert!(
        iroha_core::sumeragi::status::commit_qc_history()
            .iter()
            .all(|qc| qc.height != 1),
        "fixture must evict the oldest QC from process history"
    );
    let oldest_hash = kura
        .get_block(NonZeroUsize::new(1).expect("nonzero height"))
        .expect("oldest block")
        .hash();
    assert!(
        state
            .commit_roster_snapshot_for_block(1, oldest_hash)
            .is_none(),
        "fixture must have no retained journal or Kura roster sidecar for the oldest block"
    );
    let view = state.view();
    let oldest = build_finality_proof(&view, 1)
        .expect("oldest proof should fall back to the durable commit-QC archive");
    verify_finality_proof(
        &oldest,
        verification_config(
            &oldest.chain_id,
            Some(1),
            Some(oldest.commit_qc.validator_set_hash),
        ),
    )
    .expect("historical fallback proof must still verify cryptographically");
    build_finality_proof(&view, 2).expect("recent proof should be retained");
    build_finality_proof(&view, 3).expect("newest proof should be retained");
}

#[test]
fn builds_finality_bundle_for_stored_block() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let kp = checked_bls_keypair();
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

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    kura.store_block(genesis_block)
        .expect("store genesis block");
    kura.store_block(block).expect("store block");
    let query_handle = iroha_core::query::store::LiveQueryStore::start_test();
    let mut world = iroha_core::state::World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
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
    assert_eq!(bundle.justification.signatures.len(), 0);
    // MMR root reflects bag-of-peaks over blocks 1..=2; should not equal the leaf hash.
    assert!(bundle.commitment.mmr_root.is_some());
    assert_eq!(bundle.commitment.mmr_leaf_index, Some(1));
}

#[test]
fn finality_bundle_rejects_zero_height() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);

    assert!(matches!(
        build_finality_bundle(&state.view(), 0),
        Err(BridgeFinalityError::InvalidHeight(0))
    ));
}

#[test]
fn finality_bundle_reports_missing_mmr_leaf_after_proof_succeeds() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());
    let chain_id: ChainId = "iroha:bridge-sparse-state".parse().expect("chain id");

    let header = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let validator_set = vec![peer_id];
    let keypairs = vec![kp.clone()];
    let cert = build_commit_qc(&chain_id, block_hash, 2, 0, 0, &validator_set, &keypairs);
    record_commit_qc(cert);

    let state = SparseBridgeState::new(chain_id)
        .with_block(2, block)
        .with_validator_pop(&kp);

    assert!(matches!(
        build_finality_bundle(&state, 2),
        Err(BridgeFinalityError::BlockNotFound(1))
    ));
}

#[test]
fn finality_bundle_rebuilds_mmr_for_lower_height_request() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());

    let genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let genesis_block = BlockBuilder::new(genesis).build_with_signature(0, kp.private_key());
    let genesis_hash = genesis_block.hash();

    let header2 = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        Some(genesis_hash),
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header2).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(genesis_block)
        .expect("store genesis block");
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
    let state = State::new_for_testing(world, kura, query_handle);

    let validator_set = vec![peer_id];
    let keypairs = vec![kp];
    let chain_id = state.view().chain_id().clone();
    record_commit_qc(build_commit_qc(
        &chain_id,
        genesis_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    ));
    record_commit_qc(build_commit_qc(
        &chain_id,
        block_hash,
        2,
        0,
        0,
        &validator_set,
        &keypairs,
    ));

    let view = state.view();
    build_finality_bundle(&view, 2).expect("height 2 bundle");
    let lower = build_finality_bundle(&view, 1).expect("height 1 bundle");

    assert_eq!(lower.commitment.block_height, 1);
    assert_eq!(lower.commitment.block_hash, genesis_hash);
    assert_eq!(lower.commitment.mmr_leaf_index, Some(0));
    assert_eq!(lower.commitment.mmr_peaks.as_ref().map(Vec::len), Some(1));
}

#[test]
fn finality_bundle_rebuilds_mmr_after_top_block_replace() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let kp = checked_bls_keypair();
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
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
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
fn finality_bundle_rebuilds_mmr_for_different_chain_id() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());
    let validator_set = vec![peer_id];
    let keypairs = vec![kp.clone()];

    let chain_a: ChainId = "iroha:bridge-chain-a".parse().expect("chain id");
    let header_a = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let block_a = BlockBuilder::new(header_a).build_with_signature(0, kp.private_key());
    let hash_a = block_a.hash();
    record_commit_qc(build_commit_qc(
        &chain_a,
        hash_a,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    ));
    let state_a = SparseBridgeState::new(chain_a)
        .with_block(1, block_a)
        .with_validator_pop(&kp);
    let bundle_a = build_finality_bundle(&state_a, 1).expect("chain A bundle");

    let chain_b: ChainId = "iroha:bridge-chain-b".parse().expect("chain id");
    let header_b = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        1,
        0,
    );
    let block_b = BlockBuilder::new(header_b).build_with_signature(0, kp.private_key());
    let hash_b = block_b.hash();
    record_commit_qc(build_commit_qc(
        &chain_b,
        hash_b,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    ));
    let state_b = SparseBridgeState::new(chain_b)
        .with_block(1, block_b)
        .with_validator_pop(&kp);
    let bundle_b = build_finality_bundle(&state_b, 1).expect("chain B bundle");

    assert_ne!(hash_a, hash_b);
    assert_eq!(bundle_a.commitment.block_hash, hash_a);
    assert_eq!(bundle_b.commitment.block_hash, hash_b);
    assert_ne!(bundle_a.commitment.mmr_root, bundle_b.commitment.mmr_root);
}

#[test]
fn finality_bundle_extends_mmr_from_cached_lower_height() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);

    let kp = checked_bls_keypair();
    let peer_id = PeerId::new(kp.public_key().clone());

    let genesis = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let genesis_block = BlockBuilder::new(genesis).build_with_signature(0, kp.private_key());
    let genesis_hash = genesis_block.hash();

    let header2 = BlockHeader::new(
        NonZeroU64::new(2).expect("non-zero"),
        Some(genesis_hash),
        None,
        None,
        0,
        0,
    );
    let block = BlockBuilder::new(header2).build_with_signature(0, kp.private_key());
    let block_hash = block.hash();

    let kura = Kura::blank_kura_for_testing();
    kura.store_block(genesis_block)
        .expect("store genesis block");
    kura.store_block(block).expect("store block");
    let query_handle = LiveQueryStore::start_test();
    let mut world = World::new();
    seed_validator_pops(&mut world, std::slice::from_ref(&kp));
    let state = State::new_for_testing(world, kura, query_handle);

    let validator_set = vec![peer_id];
    let keypairs = vec![kp];
    let chain_id = state.view().chain_id().clone();
    record_commit_qc(build_commit_qc(
        &chain_id,
        genesis_hash,
        1,
        0,
        0,
        &validator_set,
        &keypairs,
    ));
    record_commit_qc(build_commit_qc(
        &chain_id,
        block_hash,
        2,
        0,
        0,
        &validator_set,
        &keypairs,
    ));

    let view = state.view();
    let first = build_finality_bundle(&view, 1).expect("height 1 bundle");
    let second = build_finality_bundle(&view, 2).expect("height 2 bundle");

    assert_eq!(first.commitment.mmr_leaf_index, Some(0));
    assert_eq!(second.commitment.mmr_leaf_index, Some(1));
    assert_ne!(first.commitment.mmr_root, second.commitment.mmr_root);
    assert_eq!(second.commitment.block_hash, block_hash);
}

#[test]
fn verify_finality_proof_accepts_valid_payload() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let config = FinalityProofVerificationConfig {
        expected_chain_id: &chain_id,
        expected_height: Some(proof.height),
        trusted_validator_set_hash: Some(validator_set_hash),
    };

    verify_finality_proof(&proof, &config).expect("proof should verify");
}

#[test]
fn verify_finality_proof_accepts_without_optional_height_or_trusted_roster() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (proof, chain_id, _) = build_proof_with_validators(&validators);
    let config = verification_config(&chain_id, None, None);

    verify_finality_proof(&proof, &config).expect("proof should verify");
}

#[test]
fn verify_finality_proof_accepts_four_validator_quorum_subset() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [
        checked_bls_keypair(),
        checked_bls_keypair(),
        checked_bls_keypair(),
        checked_bls_keypair(),
    ];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let signers: BTreeSet<_> = (0..3)
        .map(|idx| ValidatorIndex::try_from(idx).expect("validator index fits"))
        .collect();
    proof.commit_qc.aggregate.signers_bitmap =
        build_signers_bitmap(&signers, proof.commit_qc.validator_set.len());
    proof.commit_qc.aggregate.bls_aggregate_signature = aggregate_signature_for_signers(
        &chain_id,
        PERMISSIONED_TAG,
        Phase::Commit,
        proof.block_hash,
        proof.height,
        proof.commit_qc.view,
        proof.commit_qc.epoch,
        &signers,
        &validators,
    );
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    verify_finality_proof(&proof, &config).expect("quorum subset should verify");
}

#[test]
fn verify_finality_quorum_matches_live_sumeragi_for_rosters_zero_through_64() {
    const MAX_ROSTER_LEN: usize = 64;

    let validators = deterministic_bls_validators(MAX_ROSTER_LEN);
    let pops = validator_pops(&validators);
    let no_signers = BTreeSet::new();
    let (mut empty_proof, empty_chain_id, empty_validator_set_hash) =
        direct_finality_proof_for_signers(&[], &[], &no_signers);
    empty_proof.validator_set_pops.push(vec![0xA5]);
    empty_proof.commit_qc.aggregate.signers_bitmap = vec![0xFF];
    empty_proof.commit_qc.aggregate.bls_aggregate_signature = vec![0xA5];
    let empty_config = verification_config(
        &empty_chain_id,
        Some(empty_proof.height),
        Some(empty_validator_set_hash),
    );
    assert_eq!(commit_quorum_from_len(0), 0);
    assert_eq!(
        verify_finality_proof(&empty_proof, &empty_config),
        Err(BridgeFinalityVerificationError::EmptyValidatorSet),
        "an empty roster must be rejected before malformed PoPs, bitmap, or signature bytes"
    );

    let one_signer = signer_prefix(1);
    for roster_len in 1..=MAX_ROSTER_LEN {
        let (proof, chain_id, validator_set_hash) = direct_finality_proof_for_signers(
            &validators[..roster_len],
            &pops[..roster_len],
            &one_signer,
        );
        assert_proof_aggregate_signature_is_valid(&proof, &one_signer);
        let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));
        let expected_required = commit_quorum_from_len(roster_len);

        if expected_required == 1 {
            verify_finality_proof(&proof, &config)
                .expect("one valid signature must satisfy a one-validator roster");
            continue;
        }

        assert_eq!(
            verify_finality_proof(&proof, &config),
            Err(BridgeFinalityVerificationError::InsufficientSignatures {
                collected: 1,
                required: expected_required,
            }),
            "bridge verifier quorum diverged from live Sumeragi for roster_len={roster_len}"
        );
    }
}

#[test]
fn verify_finality_rejects_every_valid_aggregate_accepted_by_retired_weaker_quorum() {
    const MAX_ROSTER_LEN: usize = 64;

    let validators = deterministic_bls_validators(MAX_ROSTER_LEN);
    let pops = validator_pops(&validators);
    let mut divergent_roster_count = 0;

    for roster_len in 1..=MAX_ROSTER_LEN {
        let retired_required = retired_bridge_quorum_from_len(roster_len);
        let required = commit_quorum_from_len(roster_len);
        if retired_required == required {
            continue;
        }
        divergent_roster_count += 1;
        assert!(
            retired_required < required,
            "the retired bridge policy must never be stronger than live Sumeragi"
        );

        let signers = signer_prefix(retired_required);
        let (proof, chain_id, validator_set_hash) = direct_finality_proof_for_signers(
            &validators[..roster_len],
            &pops[..roster_len],
            &signers,
        );
        assert_proof_aggregate_signature_is_valid(&proof, &signers);
        let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

        assert_eq!(
            verify_finality_proof(&proof, &config),
            Err(BridgeFinalityVerificationError::InsufficientSignatures {
                collected: retired_required,
                required,
            }),
            "valid aggregate at retired quorum must fail for roster_len={roster_len}"
        );
    }

    assert_eq!(
        divergent_roster_count, 40,
        "the regression sweep must exercise every divergent roster size through 64"
    );
}

#[test]
fn verify_finality_enforces_exact_quorum_at_representative_boundaries() {
    const MAX_ROSTER_LEN: usize = 64;
    const BOUNDARIES: [usize; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 31, 63, 64];

    let validators = deterministic_bls_validators(MAX_ROSTER_LEN);
    let pops = validator_pops(&validators);

    for roster_len in BOUNDARIES {
        let required = commit_quorum_from_len(roster_len);
        let exact_signers = signer_prefix(required);
        let (exact_proof, chain_id, validator_set_hash) = direct_finality_proof_for_signers(
            &validators[..roster_len],
            &pops[..roster_len],
            &exact_signers,
        );
        let config = verification_config(
            &chain_id,
            Some(exact_proof.height),
            Some(validator_set_hash),
        );
        verify_finality_proof(&exact_proof, &config).unwrap_or_else(|error| {
            panic!("exact quorum failed for roster_len={roster_len}: {error}")
        });

        let below_signers = signer_prefix(required - 1);
        let (mut below_proof, below_chain_id, below_validator_set_hash) =
            direct_finality_proof_for_signers(
                &validators[..roster_len],
                &pops[..roster_len],
                &below_signers,
            );
        if below_signers.is_empty() {
            below_proof.commit_qc.aggregate.bls_aggregate_signature = exact_proof
                .commit_qc
                .aggregate
                .bls_aggregate_signature
                .clone();
        } else {
            assert_proof_aggregate_signature_is_valid(&below_proof, &below_signers);
        }
        let below_config = verification_config(
            &below_chain_id,
            Some(below_proof.height),
            Some(below_validator_set_hash),
        );

        assert_eq!(
            verify_finality_proof(&below_proof, &below_config),
            Err(BridgeFinalityVerificationError::InsufficientSignatures {
                collected: required - 1,
                required,
            }),
            "threshold-minus-one proof must fail for roster_len={roster_len}"
        );
    }
}

#[test]
fn verify_finality_proof_rejects_chain_id_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

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
    let validators = [checked_bls_keypair()];

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
    let validators = [checked_bls_keypair()];

    let (proof, chain_id, advertised_hash) = build_proof_with_validators(&validators);
    let other_peer = PeerId::new(checked_bls_keypair().public_key().clone());
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

#[test]
fn verify_finality_proof_rejects_block_header_height_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.height += 1;
    let config = verification_config(&chain_id, None, Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::BlockHeaderHeightMismatch {
            proof_height,
            header_height,
        } => {
            assert_eq!(proof_height, 2);
            assert_eq!(header_height, 1);
        }
        other => panic!("expected block-header height mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_qc_height_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let proof_height = proof.height;
    proof.commit_qc.height = proof_height + 1;
    let config = verification_config(&chain_id, Some(proof_height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::QcHeightMismatch {
            proof_height: actual_proof_height,
            cert_height,
        } => {
            assert_eq!(actual_proof_height, proof_height);
            assert_eq!(cert_height, proof_height + 1);
        }
        other => panic!("expected QC height mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_unexpected_certificate_phase() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.phase = Phase::Prepare;
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::UnexpectedCertificatePhase { actual } => {
            assert_eq!(actual, Phase::Prepare);
        }
        other => panic!("expected unexpected certificate phase, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_block_hash_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let forged_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH]));
    proof.block_hash = forged_hash;
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::BlockHashMismatch {
            header_hash,
            proof_hash,
            certificate_hash,
        } => {
            assert_eq!(header_hash, certificate_hash);
            assert_eq!(proof_hash, forged_hash);
        }
        other => panic!("expected block hash mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_unsupported_validator_set_hash_version() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.validator_set_hash_version = VALIDATOR_SET_HASH_VERSION_V1 + 1;
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::UnsupportedValidatorSetHashVersion { version } => {
            assert_eq!(version, VALIDATOR_SET_HASH_VERSION_V1 + 1);
        }
        other => panic!("expected unsupported validator set hash version, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_validator_set_hash_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let other_peer = PeerId::new(checked_bls_keypair().public_key().clone());
    let advertised = HashOf::new(&vec![other_peer]);
    proof.commit_qc.validator_set_hash = advertised;
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::ValidatorSetHashMismatch {
            computed,
            advertised: actual_advertised,
        } => {
            assert_ne!(computed, advertised);
            assert_eq!(actual_advertised, advertised);
        }
        other => panic!("expected validator set hash mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_empty_validator_set() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, _) = build_proof_with_validators(&validators);
    proof.commit_qc.validator_set.clear();
    proof.commit_qc.validator_set_hash = HashOf::new(&Vec::<PeerId>::new());
    let config = verification_config(&chain_id, Some(proof.height), None);

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::EmptyValidatorSet)
    ));
}

#[test]
fn verify_finality_proof_rejects_validator_set_pop_length_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.validator_set_pops.clear();
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::ValidatorSetPopLengthMismatch { expected, actual } => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected validator-set PoP length mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_invalid_validator_pop() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    let other = checked_bls_keypair();
    proof.validator_set_pops[0] =
        iroha_crypto::bls_normal_pop_prove(other.private_key()).expect("other pop");
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::AggregateSignatureInvalid)
    ));
}

#[test]
fn verify_finality_proof_rejects_signer_bitmap_length_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.signers_bitmap.clear();
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::SignerBitmapLengthMismatch { expected, actual } => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 0);
        }
        other => panic!("expected signer bitmap length mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_oversized_signer_bitmap() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.signers_bitmap = vec![0b0000_0001, 0];
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::SignerBitmapLengthMismatch { expected, actual } => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("expected oversized signer bitmap mismatch, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_signer_out_of_bounds() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.signers_bitmap = vec![0b0000_0010];
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::SignerOutOfBounds { signer, roster_len } => {
            assert_eq!(signer, 1);
            assert_eq!(roster_len, 1);
        }
        other => panic!("expected signer out of bounds, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_zero_signers_even_with_signature_bytes() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.signers_bitmap = vec![0];
    assert!(!proof.commit_qc.aggregate.bls_aggregate_signature.is_empty());
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::InsufficientSignatures {
            collected,
            required,
        } => {
            assert_eq!(collected, 0);
            assert_eq!(required, 1);
        }
        other => panic!("expected insufficient signatures for zero signers, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_missing_aggregate_signature() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.bls_aggregate_signature.clear();
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::AggregateSignatureMissing)
    ));
}

#[test]
fn verify_finality_proof_rejects_insufficient_signatures() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair(), checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.aggregate.signers_bitmap = vec![0b0000_0001];
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    match verify_finality_proof(&proof, &config).unwrap_err() {
        BridgeFinalityVerificationError::InsufficientSignatures {
            collected,
            required,
        } => {
            assert_eq!(collected, 1);
            assert_eq!(required, 2);
        }
        other => panic!("expected insufficient signatures, got {other:?}"),
    }
}

#[test]
fn verify_finality_proof_rejects_mode_tag_mismatch() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.mode_tag = "permissioned:v2".to_string();
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::AggregateSignatureInvalid)
    ));
}

#[test]
fn verify_finality_proof_rejects_state_root_tampering() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    proof.commit_qc.post_state_root = Hash::prehashed([0x42; Hash::LENGTH]);
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::AggregateSignatureInvalid)
    ));
}

#[test]
fn verify_finality_proof_rejects_invalid_aggregate_signature() {
    let _exclusive = lock_finality_tests();
    let _guard = CommitCertHistoryGuard::with_cap(DEFAULT_COMMIT_CERT_HISTORY_CAP);
    let validators = [checked_bls_keypair()];

    let (mut proof, chain_id, validator_set_hash) = build_proof_with_validators(&validators);
    assert!(!proof.commit_qc.aggregate.bls_aggregate_signature.is_empty());
    proof.commit_qc.aggregate.bls_aggregate_signature.fill(0xA5);
    let config = verification_config(&chain_id, Some(proof.height), Some(validator_set_hash));

    assert!(matches!(
        verify_finality_proof(&proof, &config),
        Err(BridgeFinalityVerificationError::AggregateSignatureInvalid)
    ));
}
