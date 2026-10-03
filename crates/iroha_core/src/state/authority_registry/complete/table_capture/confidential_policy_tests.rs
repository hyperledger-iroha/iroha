//! Actual definition catalog rejects untrusted transition projections at both cuts.
//!
//! TODO: link this regression module with the confidential-policy definition
//! checker after the currently shared build has completed.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{
        World,
        authority_registry::grouped_ownership::{GroupImage, GroupMismatch, GroupedOwnershipError},
    },
};
use iroha_crypto::Hash;
use iroha_data_model::{
    account::Account,
    asset::{
        AssetBalancePolicy, AssetConfidentialPolicy, AssetDefinition, AssetDefinitionId,
        ConfidentialPolicyMode, ConfidentialPolicyTransition,
    },
    prelude::Registrable,
};
use iroha_test_samples::ALICE_ID;

fn definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("policy", "universal").unwrap(),
        "coin".parse().unwrap(),
    )
}

fn state() -> State {
    let mut definition =
        AssetDefinition::numeric(definition_id(), "coin", AssetBalancePolicy::Global, None)
            .build(&ALICE_ID);
    let mut policy = AssetConfidentialPolicy::convertible();
    policy.pending_transition = Some(ConfidentialPolicyTransition {
        new_mode: ConfidentialPolicyMode::ShieldedOnly,
        effective_height: 41,
        previous_mode: ConfidentialPolicyMode::Convertible,
        transition_id: Hash::new(b"checked-policy-catalog"),
        conversion_window: Some(1),
    });
    definition.set_confidential_policy(policy);
    State::new_for_testing(
        World::with(
            [],
            [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
            [definition],
        ),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

fn capture(state: &State) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let TableMaterializer::Single { capture, .. } = TABLE_MATERIALIZERS
        .iter()
        .find(|owner| owner.table_ids().any(|id| id == "world.asset_definitions"))
        .unwrap()
    else {
        panic!("one checked original definition owner");
    };
    capture(state, native_test_support::limits())
}

#[test]
fn actual_catalog_rejects_corrupt_policy_projections_after_state_construction() {
    for previous in [false, true] {
        for defect in 0..5 {
            let mut state = state();
            // Constructors repair derived indexes; inject only afterwards to
            // prove this check is consumed by the actual live table catalog.
            match defect {
                0 => {
                    state.world.confidential_policy_transition_index = mv::storage::Storage::new();
                }
                1 => {
                    state.world.confidential_policy_transition_counts = mv::storage::Storage::new();
                }
                2 => {
                    state
                        .world
                        .confidential_policy_transition_counts
                        .insert(41, 2);
                }
                3 => {
                    state
                        .world
                        .confidential_policy_transition_index
                        .insert((42, definition_id()), ());
                }
                4 => {
                    state
                        .world
                        .confidential_policy_transition_counts
                        .insert(41, 0);
                }
                _ => unreachable!(),
            }
            if previous {
                if matches!(defect, 0 | 3) {
                    let mut block = state.world.confidential_policy_transition_index.block();
                    block.insert((41, definition_id()), ());
                    block.remove((42, definition_id()));
                    block.commit();
                } else {
                    let mut block = state.world.confidential_policy_transition_counts.block();
                    block.insert(41, 1);
                    block.commit();
                }
            }
            state.ivm_execution_budget().set_limit_bytes(0);
            assert_eq!(
                capture(&state).err(),
                Some(LeafError::GroupedOwnership(
                    GroupedOwnershipError::Corrupt {
                        index: if matches!(defect, 0 | 3) {
                            "world.confidential_policy_transition_index"
                        } else {
                            "world.confidential_policy_transition_counts"
                        },
                        image: if previous {
                            GroupImage::Predecessor
                        } else {
                            GroupImage::Current
                        },
                        mismatch: match defect {
                            0 | 1 => GroupMismatch::MissingMember,
                            4 => GroupMismatch::EmptyGroup,
                            _ => GroupMismatch::ForeignMember,
                        },
                    }
                ))
            );
            // An active State publication takes precedence over inconsistent
            // reader content; the local retry exposes no partial leaf set.
            let mut publication = state.state_view_publication();
            let guard = publication.begin();
            assert!(capture(&state).unwrap().is_none());
            drop(guard);
        }
    }
}

#[test]
fn actual_catalog_preserves_original_allocation_refusal_and_retry() {
    let state = state();
    let original = state.ivm_execution_budget();
    // State keeps its original native-tip and publication-release owners alive.
    // Only the capture's own allocations may disappear when its snapshot drops.
    let resident = original.reserved_bytes();
    original.set_limit_bytes(0);
    assert!(matches!(
        capture(&state),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    assert_eq!(original.reserved_bytes(), resident);
    original.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture(&state).unwrap().unwrap();
    assert_eq!(snapshot.table_id(), "world.asset_definitions");
    assert_eq!(snapshot.row_count(), 1);
    assert!(original.reserved_bytes() > resident);
    drop(snapshot);
    assert_eq!(original.reserved_bytes(), resident);
}
