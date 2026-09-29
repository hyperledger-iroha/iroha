//! Original lane merge references are work; metadata alone cannot create a proposal.
use super::*;
use iroha_data_model::block::{BlockHeader, builder::BlockBuilder as WireBlockBuilder};
use iroha_model_base::topology::LaneId;
use std::{collections::BTreeSet, num::NonZeroU64};

#[test]
fn native_lane_merge_is_work_without_a_direct_network_execution_leaf() {
    let merge = SumeragiLaneMerge {
        lane: LaneId::new(1),
        incarnation: [1; 32],
        from: 1,
        to: 1,
        tip_hash: [2; 32],
        tip_result: [3; 32],
    };
    let mut builder = WireBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        0,
    ));
    let mut context = BlockExecutionContextBundle::default();
    context.lane_merge = Some(SumeragiLaneMergeSection {
        merges: vec![merge],
        time_floor_ms: 1,
        merged_count: 0,
    });
    builder.set_execution_context(Some(context));
    let block = builder.build(BTreeSet::new());
    assert_eq!(block.network_entrypoint_count(), 0);
    assert!(block.has_consensus_work());
    let encoded = encode(&block).unwrap();
    let decoded = decode(&encoded).unwrap();
    assert!(decoded.has_consensus_work());
    assert_eq!(decoded.lane_merge().unwrap().merges, vec![merge]);
    assert_eq!(decoded.encode_wire().unwrap(), encoded);
    // Work classification does not grant execution authority. Native merge expansion
    // authenticates exact tip/hash/result/incarnation and time against the source store;
    // malformed_merges_are_invalid_and_missing_blocks_pending exercises those checks.
}

#[test]
fn outputs_and_empty_context_cannot_create_proposal_work() {
    let mut builder = WireBlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        19,
    ));
    builder.set_execution_context(Some(BlockExecutionContextBundle::default()));
    let block = builder.build(BTreeSet::new());
    assert!(!block.has_consensus_work());
    assert_eq!(encode(&block), Err(PayloadError::EmptyBlock));
    assert!(matches!(
        decode(&block.encode_wire().unwrap()),
        Err(PayloadError::EmptyBlock)
    ));
}
