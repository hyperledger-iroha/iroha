use iroha_crypto::SignatureOf;
use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader, BlockPayload, BlockResult, BlockSignature},
    transaction::{
        ExecutionStep,
        signed::{
            TransactionBuilder, TransactionEntrypoint, TransactionResult, TransactionResultInner,
        },
    },
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};
use iroha_primitives::const_vec::ConstVec;
use nonzero_ext::nonzero;
use norito::codec::{DecodeAll as _, Encode as _};

use super::*;
use crate::kura::Kura;

#[derive(norito::codec::Decode, norito::codec::Encode)]
struct MutableSignedBlockWire {
    signatures: BTreeSet<BlockSignature>,
    payload: BlockPayload,
    result: Option<BlockResult>,
}

fn block_proof_fixture() -> (
    SignedBlock,
    HashOf<TransactionEntrypoint>,
    HashOf<TransactionEntrypoint>,
) {
    let keypair = crate::state::checked_keypair();
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(keypair.private_key());
    let scheduled = TimeTriggerEntrypoint {
        id: "block_proof_schedule".parse().expect("trigger id"),
        instructions: ExecutionStep(ConstVec::new_empty()),
        authority,
    };
    let external_hash = tx.hash_as_entrypoint();
    let scheduled_hash = scheduled.hash_as_entrypoint();

    let external_tree: CanonMerkleTree<TransactionEntrypoint> =
        [external_hash].into_iter().collect();
    let header = BlockHeader::new(nonzero!(1_u64), None, external_tree.root(), None, 0, 0);
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(keypair.private_key(), header.hash())
            .expect("test block signing should succeed"),
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    block
        .set_transaction_results(
            vec![scheduled],
            &[external_hash, scheduled_hash],
            vec![
                TransactionResultInner::Ok(DataTriggerSequence::default()),
                TransactionResultInner::Ok(DataTriggerSequence::default()),
            ],
        )
        .expect("test block entrypoints and results should align");
    (block, external_hash, scheduled_hash)
}

fn mutate_stored_block(
    block: &SignedBlock,
    mutate: impl FnOnce(&mut BlockPayload, &mut BlockResult),
) -> SignedBlock {
    let encoded = block.encode();
    let mut wire = MutableSignedBlockWire::decode_all(&mut encoded.as_slice())
        .expect("fixture block wire should decode into mutable parts");
    let result = wire.result.as_mut().expect("fixture block has results");
    mutate(&mut wire.payload, result);
    SignedBlock::decode_all(&mut wire.encode().as_slice())
        .expect("tampered fixture should remain a structurally valid block wire")
}

fn proof_error(
    block: SignedBlock,
    requested_height: NonZeroU64,
    entry_hash: HashOf<TransactionEntrypoint>,
) -> BlockProofError {
    let kura = Kura::blank_kura_for_testing();
    kura.append_pending_block_for_bench(Arc::new(block));
    block_proofs_for_entry_from_kura(kura.as_ref(), requested_height, entry_hash)
        .expect_err("adversarial stored block must not produce a proof")
}

fn assert_entry_geometry_error(
    error: BlockProofError,
    entry_hash: HashOf<TransactionEntrypoint>,
    block_height: NonZeroU64,
) {
    match error {
        BlockProofError::MerkleProofUnavailable {
            entry_hash: actual_entry_hash,
            block_height: actual_block_height,
        } => {
            assert_eq!(actual_entry_hash, entry_hash);
            assert_eq!(actual_block_height, block_height);
        }
        other => panic!("expected canonical entry geometry error, got {other:?}"),
    }
}

fn assert_result_geometry_error(
    error: BlockProofError,
    entry_hash: HashOf<TransactionEntrypoint>,
    block_height: NonZeroU64,
) {
    match error {
        BlockProofError::ExecutionResultMissing {
            entry_hash: actual_entry_hash,
            block_height: actual_block_height,
        } => {
            assert_eq!(actual_entry_hash, entry_hash);
            assert_eq!(actual_block_height, block_height);
        }
        other => panic!("expected canonical result geometry error, got {other:?}"),
    }
}

#[test]
fn block_proofs_for_external_entry_use_full_executed_tree() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let world = World::default();
    let state = State::new_for_testing(world, Arc::clone(&kura), query);
    let (block, entry_hash, _) = block_proof_fixture();

    let block_arc = Arc::new(block);
    kura.store_block(Arc::clone(&block_arc))
        .expect("store block");

    {
        let mut hashes = state.block_hashes.block();
        hashes.push(block_arc.hash());
        hashes.commit();
    }

    let proofs = state
        .block_proofs_for_entry(block_arc.header().height(), entry_hash)
        .expect("proofs available");

    let external_root = block_arc
        .header()
        .merkle_root()
        .expect("external entry merkle root");
    let full_entry_root = block_arc
        .full_entry_merkle_root()
        .expect("full entry merkle root");
    assert_ne!(external_root, full_entry_root);
    assert_eq!(proofs.block_hash, block_arc.hash());
    assert_eq!(
        proofs.executed_block_wire_hash,
        block_arc
            .executed_block_wire_hash()
            .expect("executed block wire hash")
    );
    assert_eq!(proofs.entry_commitment.root(), &full_entry_root);
    assert_eq!(proofs.entry_commitment.leaf_count().get(), 2);
    assert!(proofs.entry_proof.verify(&proofs.entry_commitment));

    let result_root = block_arc
        .header()
        .result_merkle_root()
        .expect("result merkle root");
    let result_commitment = proofs.result_commitment;
    assert_eq!(result_commitment.root(), &result_root);
    assert_eq!(result_commitment.leaf_count().get(), 2);
    let result_proof = proofs.result_proof;
    assert!(result_proof.verify(&result_commitment));
}

#[test]
fn block_proofs_reject_stored_full_entry_root_drift() {
    let block_height = nonzero!(1_u64);
    let (block, external_hash, scheduled_hash) = block_proof_fixture();
    let canonical_commitment = block
        .full_entry_merkle_commitment()
        .expect("fixture full entry commitment");
    let substituted_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"substituted stored entry leaf",
    ));
    let substituted_tree: CanonMerkleTree<TransactionEntrypoint> =
        [substituted_hash, scheduled_hash].into_iter().collect();
    let substituted_commitment = substituted_tree
        .commitment()
        .expect("substituted entry commitment");
    assert_eq!(
        substituted_commitment.leaf_count(),
        canonical_commitment.leaf_count()
    );
    assert_ne!(substituted_commitment.root(), canonical_commitment.root());

    let tampered = mutate_stored_block(&block, |_, result| {
        result.merkle = substituted_tree;
    });
    assert_entry_geometry_error(
        proof_error(tampered, block_height, external_hash),
        external_hash,
        block_height,
    );
}

#[test]
fn block_proofs_reject_stored_full_entry_count_drift() {
    let block_height = nonzero!(1_u64);
    let (block, external_hash, scheduled_hash) = block_proof_fixture();
    let extra_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"extra stored entry leaf",
    ));
    let substituted_tree: CanonMerkleTree<TransactionEntrypoint> =
        [external_hash, scheduled_hash, extra_hash]
            .into_iter()
            .collect();
    assert_eq!(substituted_tree.leaf_count(), 3);

    let tampered = mutate_stored_block(&block, |_, result| {
        result.merkle = substituted_tree;
    });
    assert_entry_geometry_error(
        proof_error(tampered, block_height, external_hash),
        external_hash,
        block_height,
    );
}

#[test]
fn block_proofs_reject_stored_result_commitment_drift() {
    let block_height = nonzero!(1_u64);
    let (block, external_hash, _) = block_proof_fixture();
    let canonical_hashes = block.result_hashes().collect::<Vec<_>>();
    let substituted_hash = HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(
        b"substituted stored result leaf",
    ));
    let substituted_tree: CanonMerkleTree<TransactionResult> =
        [substituted_hash, canonical_hashes[1]]
            .into_iter()
            .collect();
    assert_eq!(substituted_tree.leaf_count(), canonical_hashes.len());
    assert_ne!(substituted_tree.root(), block.header().result_merkle_root());

    let tampered = mutate_stored_block(&block, |_, result| {
        result.result_merkle = substituted_tree;
    });
    assert_result_geometry_error(
        proof_error(tampered, block_height, external_hash),
        external_hash,
        block_height,
    );
}

#[test]
fn block_proofs_reject_header_result_root_drift() {
    let block_height = nonzero!(1_u64);
    let (block, external_hash, _) = block_proof_fixture();
    let substituted_hash = HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(
        b"substituted header result leaf",
    ));
    let substituted_tree: CanonMerkleTree<TransactionResult> =
        [substituted_hash].into_iter().collect();
    let substituted_root = substituted_tree.root();
    assert_ne!(substituted_root, block.header().result_merkle_root());

    let tampered = mutate_stored_block(&block, |payload, _| {
        payload.header.result_merkle_root = substituted_root;
    });
    assert_result_geometry_error(
        proof_error(tampered, block_height, external_hash),
        external_hash,
        block_height,
    );
}

#[test]
fn block_proofs_reject_self_consistent_result_count_misalignment() {
    let block_height = nonzero!(1_u64);
    let (block, external_hash, _) = block_proof_fixture();
    let tampered = mutate_stored_block(&block, |payload, result| {
        result.transaction_results.truncate(1);
        result.result_merkle = result
            .transaction_results
            .iter()
            .map(TransactionResult::hash)
            .collect();
        payload.header.result_merkle_root = result.result_merkle.root();
    });
    assert_eq!(
        tampered
            .result_merkle_commitment()
            .expect("tampered result commitment")
            .leaf_count()
            .get(),
        1
    );
    assert_eq!(
        tampered
            .full_entry_merkle_commitment()
            .expect("tampered entry commitment")
            .leaf_count()
            .get(),
        2
    );
    assert_result_geometry_error(
        proof_error(tampered, block_height, external_hash),
        external_hash,
        block_height,
    );
}

#[test]
fn block_proofs_reject_requested_slot_header_height_mismatch() {
    let requested_height = nonzero!(1_u64);
    let actual_height = nonzero!(2_u64);
    let (block, external_hash, _) = block_proof_fixture();
    let tampered = mutate_stored_block(&block, |payload, _| {
        payload.header.height = actual_height;
    });

    match proof_error(tampered, requested_height, external_hash) {
        BlockProofError::BlockHeightMismatch { requested, actual } => {
            assert_eq!(requested, requested_height);
            assert_eq!(actual, actual_height);
        }
        other => panic!("expected canonical Kura slot/header error, got {other:?}"),
    }
}
