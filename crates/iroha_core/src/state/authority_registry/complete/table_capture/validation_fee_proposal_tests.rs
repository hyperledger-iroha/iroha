//! Actual proposal catalog consumes both retained fee-index images.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        GovernanceProposalRecord, GovernanceProposalStatus, World,
        authority_registry::grouped_ownership::{GroupImage, GroupMismatch, GroupedOwnershipError},
    },
};
use iroha_data_model::governance::types::ProposalKind;
use mv::storage::Storage;

const ID: [u8; 32] = [41; 32];

fn state() -> State {
    let kind = crate::governance::parliament::tests::validation_fee_policy_proposal();
    let ProposalKind::ValidationFeePolicy(policy) = &kind else {
        unreachable!()
    };
    let mut world = World::default();
    world.governance_proposals.insert(
        ID,
        GovernanceProposalRecord {
            proposer: policy.proposal_operator.clone(),
            kind,
            created_height: 41,
            status: GovernanceProposalStatus::Enacted,
        },
    );
    State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn capture(state: &State) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let TableMaterializer::Single { capture, .. } = TABLE_MATERIALIZERS
        .iter()
        .find(|owner| {
            owner
                .table_ids()
                .any(|id| id == "world.governance_proposals")
        })
        .unwrap()
    else {
        panic!("one original checked proposal capture");
    };
    capture(state, native_test_support::limits())
}

#[test]
fn actual_catalog_rejects_fee_index_corruption_inserted_after_state_construction() {
    for previous in [false, true] {
        for defect in 0..4 {
            let mut state = state();
            // State construction rebuilds projections; mutation must follow it
            // to establish that the live catalog actually consumes this check.
            if defect < 2 {
                state.world.validation_fee_proposal_index = Storage::new();
            }
            let extra = match defect {
                0 => None,
                1 | 2 => Some((42, ID)),
                3 => Some((41, [99; 32])),
                _ => unreachable!(),
            };
            if let Some(key) = extra {
                state.world.validation_fee_proposal_index.insert(key, ());
            }
            if previous {
                let mut block = state.world.validation_fee_proposal_index.block();
                block.insert((41, ID), ());
                if let Some(key) = extra {
                    block.remove(key);
                }
                block.commit();
            }
            state.ivm_execution_budget().set_limit_bytes(0);
            assert_eq!(
                capture(&state).err(),
                Some(LeafError::GroupedOwnership(
                    GroupedOwnershipError::Corrupt {
                        index: "world.validation_fee_proposal_index",
                        image: if previous {
                            GroupImage::Predecessor
                        } else {
                            GroupImage::Current
                        },
                        mismatch: if defect < 2 {
                            GroupMismatch::MissingMember
                        } else {
                            GroupMismatch::ForeignMember
                        },
                    }
                ))
            );
            let mut publication = state.state_view_publication();
            let guard = publication.begin();
            assert!(capture(&state).unwrap().is_none());
            drop(guard);
        }
    }
}

#[test]
fn actual_fee_proposal_capture_preserves_original_pool_refusal_and_final_owner() {
    let state = state();
    let original = state.ivm_execution_budget();
    let resident = original.reserved_bytes();
    original.set_limit_bytes(0);
    assert!(matches!(
        capture(&state),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    assert_eq!(original.reserved_bytes(), resident);
    original.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture(&state).unwrap().unwrap();
    assert_eq!(snapshot.table_id(), "world.governance_proposals");
    assert_eq!(snapshot.row_count(), 1);
    assert!(original.reserved_bytes() > resident);
    drop(snapshot);
    assert_eq!(original.reserved_bytes(), resident);
}

#[test]
fn retained_tombstones_exhaust_only_the_local_catalog_work_allowance() {
    let state = state();
    // Each absent removal is one physical undo row. The current canonical
    // count remains one. The complete scans cost 658, exceeding the descriptor
    // allowance of two rows times 328; a larger local allowance admits the cut.
    {
        let mut block = state.world.governance_proposals.block();
        for tag in 60..161 {
            block.remove([tag; 32]);
        }
        block.commit();
    }
    assert_eq!(
        capture(&state).err(),
        Some(LeafError::GroupedOwnership(
            GroupedOwnershipError::WorkLimit
        ))
    );
    let TableMaterializer::Single { capture, .. } = TABLE_MATERIALIZERS
        .iter()
        .find(|owner| {
            owner
                .table_ids()
                .any(|id| id == "world.governance_proposals")
        })
        .unwrap()
    else {
        unreachable!()
    };
    let mut limits = native_test_support::limits();
    limits.max_rows = 4;
    let snapshot = capture(&state, limits).unwrap().unwrap();
    assert_eq!(snapshot.row_count(), 1);
}
