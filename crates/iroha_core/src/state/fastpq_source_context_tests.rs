//! FASTPQ source provenance follows actual transaction apply and rollback boundaries.

use super::*;
use crate::{
    fastpq::{FastpqCapturedSourceRoute, FastpqSourceCaptureError},
    query::store::LiveQueryStore,
};
use iroha_data_model::{
    block::BlockHeader,
    fastpq::{FastpqSourceLaneV1, TransferSmtWitness},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use nonzero_ext::nonzero;

fn state() -> State {
    State::new(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn header() -> BlockHeader {
    BlockHeader::new(nonzero!(1_u64), None, None, None, 7, 0)
}

fn delta() -> TransferDeltaTranscript {
    TransferDeltaTranscript {
        from_account: (*ALICE_ID).clone(),
        to_account: (*BOB_ID).clone(),
        asset_definition: AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        ),
        amount: Quantity::from(1_u32),
        from_balance_before: Quantity::from(10_u32),
        from_balance_after: Quantity::from(9_u32),
        to_balance_before: Quantity::zero(),
        to_balance_after: Quantity::from(1_u32),
        from_smt_witness: TransferSmtWitness::default(),
        to_smt_witness: TransferSmtWitness::default(),
    }
}

#[test]
fn source_records_publish_only_on_apply_and_survive_transcript_drain() {
    let state = state();
    let mut block = state.block(header());
    let hash = Hash::new(b"execution call");
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
    }
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    assert!(block.fastpq_transcripts.is_empty());
    let first_fragment = block.committed_fragments as u64;
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.current_lane_id = Some(LaneId::SINGLE);
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.apply();
    }
    let captured = block.captured_fastpq_transcript_sources().unwrap()[&hash];
    assert_eq!(captured.entry_hash(), hash);
    assert_eq!(captured.source().network_id, state.network_id);
    assert_eq!(captured.source().height, 1);
    assert_eq!(captured.first_fragment_index(), first_fragment);
    assert!(!captured.is_protocol_purpose());
    assert_eq!(block.drain_transfer_transcripts().len(), 1);
    assert_eq!(
        block.captured_fastpq_transcript_sources().unwrap()[&hash],
        captured
    );
}

#[test]
fn source_incarnation_is_frozen_before_block_and_transaction_lane_mutation() {
    let state = state();
    let mut block = state.block(header());
    let original = StateReadOnly::lane_incarnation_at_height(&block, LaneId::SINGLE, 1).unwrap();
    let hash = Hash::new(b"execution call");
    let mut changed: [u8; 32] = original.into();
    changed[20] ^= 1;
    block
        .lane_incarnations
        .insert(LaneId::SINGLE, Hash::prehashed(changed));
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.current_lane_id = Some(LaneId::SINGLE);
        tx.current_dataspace_id = Some(DataSpaceId::new(7));
        tx.lane_incarnations
            .insert(LaneId::SINGLE, Hash::new(b"later transaction mutation"));
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        // A later caller-context change cannot rewrite the previously captured occurrence.
        tx.current_dataspace_id = Some(DataSpaceId::new(8));
        tx.apply();
    }
    assert_eq!(
        block.captured_fastpq_transcript_sources().unwrap()[&hash].route(),
        FastpqCapturedSourceRoute::Lane(FastpqSourceLaneV1 {
            lane_id: LaneId::SINGLE,
            lane_incarnation: original,
        })
    );
    assert_eq!(
        block.captured_fastpq_transcript_sources().unwrap()[&hash].dataspace_id(),
        DataSpaceId::new(7)
    );
}

#[test]
fn native_protocol_purposes_keep_distinct_keys_and_explicit_absent_lane() {
    let state = state();
    let mut block = state.block(header());
    let hashes = [
        Hash::new(b"typed purpose one"),
        Hash::new(b"typed purpose two"),
    ];
    let fragment = block.committed_fragments as u64;
    {
        let mut tx = block.transaction();
        tx.current_dataspace_id = Some(DataSpaceId::new(7));
        for hash in hashes {
            tx.record_test_transfer_transcripts(&ALICE_ID, hash, vec![delta()]);
        }
        tx.apply();
    }
    let sources = block.captured_fastpq_transcript_sources().unwrap();
    assert_eq!(sources.len(), 2);
    for hash in hashes {
        let captured = sources[&hash];
        assert!(captured.is_protocol_purpose());
        assert_eq!(captured.entry_hash(), hash);
        assert_eq!(captured.first_fragment_index(), fragment);
        assert_eq!(captured.route(), FastpqCapturedSourceRoute::Unrouted);
        assert_eq!(captured.dataspace_id(), DataSpaceId::new(7));
        assert_eq!(block.fastpq_transcripts[&hash][0].batch_hash, hash);
    }
}

#[test]
fn discarded_conflicts_do_not_poison_block_but_applied_conflicts_hide_partial_map() {
    let state = state();
    let mut block = state.block(header());
    let hash = Hash::new(b"execution call");
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.current_dataspace_id = Some(DataSpaceId::new(7));
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
    }
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.apply();
    }
    assert_eq!(block.captured_fastpq_transcript_sources().unwrap().len(), 1);
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(hash);
        tx.current_dataspace_id = Some(DataSpaceId::new(7));
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.apply();
    }
    assert_eq!(
        block.captured_fastpq_transcript_sources(),
        Err(&FastpqSourceCaptureError::ConflictingSource { entry_hash: hash })
    );
}

#[test]
fn unresolved_applied_lane_remains_error_after_later_valid_capture() {
    let state = state();
    let mut block = state.merge_preexecution_block(header());
    let missing = LaneId::new(999);
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::new(b"bad call"));
        tx.current_lane_id = Some(missing);
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.apply();
    }
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::new(b"good call"));
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.apply();
    }
    assert_eq!(
        block.captured_fastpq_transcript_sources(),
        Err(&FastpqSourceCaptureError::MissingLaneIncarnation {
            lane_id: missing,
            entry_hash: Hash::new(b"bad call")
        })
    );
    assert_eq!(
        block.fastpq_transcripts.len(),
        2,
        "capture status does not change existing ledger execution"
    );
}

#[test]
fn batched_calls_keep_their_captured_context_when_apply_clears_active_call() {
    let state = state();
    let mut block = state.block(header());
    let hashes = [
        Hash::new(b"batch first call"),
        Hash::new(b"batch second call"),
    ];
    {
        let mut tx = block.transaction();
        for (index, hash) in hashes.iter().enumerate() {
            tx.tx_call_hash = Some(*hash);
            tx.current_dataspace_id = Some(DataSpaceId::new(index as u64));
            tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        }
        tx.tx_call_hash = None;
        tx.apply();
    }
    let sources = block.captured_fastpq_transcript_sources().unwrap();
    assert_eq!(sources.len(), 2);
    for (index, hash) in hashes.iter().enumerate() {
        assert_eq!(sources[hash].dataspace_id(), DataSpaceId::new(index as u64));
        assert!(!sources[hash].is_protocol_purpose());
        assert_eq!(block.fastpq_transcripts[hash][0].batch_hash, *hash);
    }
}

#[test]
fn changing_the_apply_bucket_after_recording_invalidates_source_capture() {
    let state = state();
    let mut block = state.block(header());
    {
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(Hash::new(b"recorded call"));
        tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
        tx.tx_call_hash = Some(Hash::new(b"different apply bucket"));
        tx.apply();
    }
    assert_eq!(
        block.captured_fastpq_transcript_sources(),
        Err(&FastpqSourceCaptureError::ExecutionIdentityMismatch)
    );
}

#[test]
fn normal_and_replacement_scopes_freeze_incarnation_before_pristine_stage() {
    fn mutate_stage(block: &mut StateBlock<'_>) -> Result<(), core::convert::Infallible> {
        block
            .lane_incarnations
            .insert(LaneId::SINGLE, Hash::new(b"pristine stage mutation"));
        Ok(())
    }
    let state = state();
    let original = {
        let probe = state.merge_preexecution_block(header());
        StateReadOnly::lane_incarnation_at_height(&probe, LaneId::SINGLE, 1).unwrap()
    };
    for replacement in [false, true] {
        let mut block = if replacement {
            state
                .block_and_revert_with_pristine_stage(header(), mutate_stage)
                .unwrap()
        } else {
            state
                .block_with_pristine_stage(header(), mutate_stage)
                .unwrap()
        };
        let hash = Hash::new(b"staged call");
        {
            let mut tx = block.transaction();
            tx.tx_call_hash = Some(hash);
            tx.current_lane_id = Some(LaneId::SINGLE);
            tx.record_transfer_transcript(&ALICE_ID, delta()).unwrap();
            tx.apply();
        }
        assert_eq!(
            block.captured_fastpq_transcript_sources().unwrap()[&hash].route(),
            FastpqCapturedSourceRoute::Lane(FastpqSourceLaneV1 {
                lane_id: LaneId::SINGLE,
                lane_incarnation: original
            })
        );
    }
}
