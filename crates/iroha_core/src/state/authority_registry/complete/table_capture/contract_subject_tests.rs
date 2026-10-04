//! Real catalog routing checks original subject sources before funded row encoding.
use super::*;
use crate::state::{
    authority_registry::grouped_ownership::{GroupImage, GroupedOwnershipError},
    contract_subject_validation::test_support::*,
};
use iroha_test_samples::BOB_ID;

fn capture(
    state: &State,
    limits: LeafLimits,
) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
    let TableMaterializer::Single { capture, .. } = TABLE_MATERIALIZERS
        .iter()
        .find(|owner| {
            owner
                .table_ids()
                .any(|id| id == "world.contract_subject_bindings")
        })
        .unwrap()
    else {
        panic!("single checked subject reader");
    };
    capture(state, limits)
}
fn limits() -> LeafLimits {
    let mut limits = native_test_support::limits();
    limits.max_rows = 16;
    limits
}

#[test]
fn actual_subject_catalog_rejects_post_constructor_corruption_in_either_image_before_allocation() {
    for prior in [false, true] {
        for defect in 0..4 {
            let mut state = state();
            match defect {
                0 => {
                    let mut bad = binding();
                    bad.lifecycle.revision = 0;
                    state.world.contract_subject_bindings.insert(address(), bad);
                    if prior {
                        let mut block = state.world.contract_subject_bindings.block();
                        block.insert(address(), binding());
                        block.commit();
                    }
                }
                1 => {
                    state
                        .world
                        .contract_subject_addresses
                        .insert(BOB_ID.clone(), address());
                    if prior {
                        let mut block = state.world.contract_subject_addresses.block();
                        block.remove(BOB_ID.clone());
                        block.commit();
                    }
                }
                2 => {
                    let subject = binding().subject;
                    let value = state.world.accounts.view().get(&subject).unwrap().clone();
                    let filtered = state
                        .world
                        .accounts
                        .view()
                        .iter()
                        .filter(|(key, _)| *key != &subject)
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect();
                    state.world.accounts = filtered;
                    if prior {
                        let mut block = state.world.accounts.block();
                        block.insert(subject, value);
                        block.commit();
                    }
                }
                3 => {
                    state
                        .world
                        .contract_instances
                        .insert(address(), iroha_crypto::Hash::new(b"drift"));
                    if prior {
                        let mut block = state.world.contract_instances.block();
                        block.remove(address());
                        block.commit();
                    }
                }
                _ => unreachable!(),
            }
            let original = state.ivm_execution_budget();
            let resident = original.reserved_bytes();
            original.set_limit_bytes(0);
            let error = capture(&state, limits()).err().unwrap();
            let image = match error {
                LeafError::GroupedOwnership(
                    GroupedOwnershipError::Source { image, .. }
                    | GroupedOwnershipError::Corrupt { image, .. },
                ) => image,
                other => panic!("source validation must precede allocation: {other:?}"),
            };
            assert_eq!(
                image,
                if prior {
                    GroupImage::Predecessor
                } else {
                    GroupImage::Current
                }
            );
            assert_eq!(original.reserved_bytes(), resident);
            let mut publication = state.state_view_publication();
            let guard = publication.begin();
            assert!(capture(&state, limits()).unwrap().is_none());
            drop(guard);
        }
    }
}

#[test]
fn actual_subject_catalog_retains_original_pool_refusal_retry_and_final_owner() {
    let state = state();
    let original = state.ivm_execution_budget();
    let resident = original.reserved_bytes();
    original.set_limit_bytes(0);
    assert!(matches!(
        capture(&state, limits()),
        Err(LeafError::Admission(_) | LeafError::OrderedRange(_))
    ));
    assert_eq!(original.reserved_bytes(), resident);
    original.set_limit_bytes(16 * 1024 * 1024);
    let snapshot = capture(&state, limits()).unwrap().unwrap();
    assert_eq!(snapshot.table_id(), "world.contract_subject_bindings");
    assert_eq!(snapshot.row_count(), 1);
    assert!(original.reserved_bytes() > resident);
    drop(snapshot);
    assert_eq!(original.reserved_bytes(), resident);
}

#[test]
fn actual_subject_catalog_large_valid_reason_defers_work_then_retries_same_source() {
    use iroha_data_model::smart_contract::ContractEmergencyHoldV1;
    let mut state = state();
    let mut held = binding();
    held.lifecycle.emergency_hold = Some(ContractEmergencyHoldV1 {
        incident_digest: [1; 32],
        proposal_content_id: [2; 32],
        governance_attempt_id: [3; 32],
        reason: " ".repeat(100_000) + "incident",
        imposed_at_height: 1,
        expires_at_height: 2,
    });
    state
        .world
        .contract_subject_bindings
        .insert(address(), held);
    let mut small = limits();
    small.max_rows = 2;
    assert_eq!(
        capture(&state, small).err(),
        Some(LeafError::GroupedOwnership(
            GroupedOwnershipError::WorkLimit
        ))
    );
    let mut larger = limits();
    larger.max_rows = 32;
    larger.max_payload_bytes = 1024 * 1024;
    larger.max_streamed_value_bytes = 1024 * 1024;
    larger.max_ordered_table_bytes = 2 * 1024 * 1024;
    let original = state.ivm_execution_budget();
    let resident = original.reserved_bytes();
    let snapshot = capture(&state, larger).unwrap().unwrap();
    assert_eq!(snapshot.row_count(), 1);
    assert!(original.reserved_bytes() > resident);
    drop(snapshot);
    assert_eq!(original.reserved_bytes(), resident);
}
