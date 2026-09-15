//! Unrelated state execution cannot replace a finalized owner's witness records.

use super::{
    tests::{apply_source, cache_canonical_test_transaction_set, delta, header, state},
    *,
};
use crate::sumeragi::witness;
use iroha_data_model::{asset::AssetId, block::consensus::ExecWitness};
use iroha_model_base::state_path::StatePath;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;

fn run_unrelated_block(with_transfer: bool, in_overlay: bool) {
    // This thread owns a separate StateBlock and never acquires or joins the
    // active owner's recorder. Its real transaction and drain must stay local.
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let overlay = in_overlay.then(witness::begin_exec_witness_overlay);
    let source = Hash::new(b"unrelated recorder isolation source");
    let marker: StatePath = "fastpq/unrelated-recorder-isolation".parse().unwrap();
    let transfer = delta();
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(source);
        tx.world
            .smart_contract_state
            .insert(marker.clone(), vec![1]);
        // Try both overwriting an owner write and adding unrelated read/write keys.
        for account in [&transfer.from_account, &transfer.to_account] {
            let asset = AssetId::of(transfer.asset_definition.clone(), account.clone());
            witness::record_read_asset(&asset, Some(&Quantity::from(123_u32)));
            witness::record_write_asset(&asset, &Quantity::from(456_u32));
        }
        if with_transfer {
            tx.record_test_transfer_transcripts(&ALICE_ID, source, vec![transfer]);
        }
        tx.apply();
    }
    assert_eq!(
        block.world.smart_contract_state.get(&marker),
        Some(&vec![1])
    );
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let archive = block.drain_transfer_transcripts_with_pending(None);
    assert_eq!(archive.len(), usize::from(with_transfer));
    if with_transfer {
        assert_eq!(archive[&source].len(), 1);
        assert!(archive[&source][0].poseidon_preimage_digest.is_some());
    }
    if let Some(overlay) = overlay {
        overlay.commit();
    }
}

fn capture_owner(unrelated: Option<(bool, bool)>) -> ExecWitness {
    witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let source = Hash::new(b"owned recorder isolation source");
    apply_source(&mut block, source, false, None);
    let transfer = delta();
    let asset = AssetId::of(transfer.asset_definition, transfer.from_account);
    witness::record_read_asset(&asset, Some(&transfer.from_balance_before));
    witness::record_write_asset(&asset, &transfer.from_balance_after);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let inventory = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();
    let archive = block.drain_transfer_transcripts_with_pending(None);
    assert!(block.fastpq_transcripts.is_empty());
    assert_eq!(archive.len(), 1);
    assert!(archive[&source][0].poseidon_preimage_digest.is_some());

    if let Some((with_transfer, in_overlay)) = unrelated {
        // Force the complete unrelated operation into the exact gap between the
        // owner's finalized transcript synchronization and checked capture.
        std::thread::spawn(move || run_unrelated_block(with_transfer, in_overlay))
            .join()
            .expect("unrelated state execution must finish before owner capture");
    }

    block
        .capture_exec_witness()
        .expect("unrelated state execution must preserve finalized source ownership");
    assert_eq!(
        block.fastpq_source_inventory().unwrap(),
        Some(inventory.as_ref())
    );
    let context = block.take_fastpq_witness_context().unwrap();
    assert!(Arc::ptr_eq(
        context._source_inventory.as_ref().unwrap(),
        &inventory
    ));
    let captured = block.take_exec_witness().unwrap();
    assert_eq!(captured.fastpq_transcripts.len(), 1);
    assert_eq!(captured.fastpq_transcripts[0].entry_hash, source);
    assert_eq!(captured.fastpq_transcripts[0].transcripts, archive[&source]);
    assert_eq!(captured.reads.len(), 1);
    assert!(
        captured
            .writes
            .iter()
            .any(|entry| entry.key == captured.reads[0].key)
    );
    assert!(captured.fastpq_batches.is_empty());
    captured
}

fn assert_unrelated_block_isolated(with_transfer: bool, in_overlay: bool) {
    let _guard = witness::exec_witness_guard();
    let expected = capture_owner(None);
    let actual = capture_owner(Some((with_transfer, in_overlay)));
    // Compare every byte, including owner reads/writes and synthetic block writes,
    // against the same execution without the intervening unrelated block.
    assert_eq!(actual, expected);
}

#[test]
fn finalized_source_capture_survives_unrelated_empty_block_drain() {
    assert_unrelated_block_isolated(false, false);
}

#[test]
fn finalized_source_capture_survives_unrelated_populated_block_drain() {
    assert_unrelated_block_isolated(true, false);
}

#[test]
fn finalized_source_capture_survives_unrelated_empty_overlay_commit() {
    assert_unrelated_block_isolated(false, true);
}

#[test]
fn finalized_source_capture_survives_unrelated_populated_overlay_commit() {
    assert_unrelated_block_isolated(true, true);
}
