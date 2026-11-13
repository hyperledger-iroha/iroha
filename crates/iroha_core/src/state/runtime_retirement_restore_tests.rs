//! Retirement history shape checks cover the original current and rollback cuts.

use super::*;
use iroha_data_model::runtime::{definition::DefinitionId, retirement::RuntimeRetirementStateV1};

fn record() -> RuntimeRetirementStateV1 {
    RuntimeRetirementStateV1 {
        history_started: 2,
        last_required: Some(3),
        governed_excluded_at: None,
        receipt: None,
    }
}

#[test]
fn retirement_restore_rejects_invalid_current_and_undo_history() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let definition = DefinitionId([0x79; 32]);
    {
        let mut block = world.block();
        block.runtime_retirements.insert(definition, record());
        block.commit();
    }
    validate_runtime_retirement_shapes(&world, &budget).unwrap();
    {
        let mut block = world.block();
        let mut invalid = record();
        invalid.history_started = 1;
        block.runtime_retirements.insert(definition, invalid);
        block.commit();
    }
    let error = validate_runtime_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid permanent runtime history")
    );
    {
        let mut block = world.block();
        block.runtime_retirements.insert(definition, record());
        block.commit();
    }
    let error = validate_runtime_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("runtime_retirements.revert"),
        "{error}"
    );
    assert_eq!(
        world.runtime_retirements.view().get(&definition),
        Some(&record())
    );
}

#[test]
fn retirement_restore_preserves_original_undo_resource_refusal_and_retry() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(0);
    let error = validate_runtime_retirement_shapes(&world, &budget).unwrap_err();
    assert!(matches!(error, StateRestoreError::Admission(_)), "{error}");
    budget.set_limit_bytes(limit);
    validate_runtime_retirement_shapes(&world, &budget).unwrap();
}

#[test]
fn retirement_restore_requires_a_selected_history_owner_before_root_authentication() {
    let state = State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    validate_runtime_retirement_roots(&state).unwrap();
    {
        let mut block = state.world.block();
        block
            .runtime_retirements
            .insert(DefinitionId([0x79; 32]), record());
        block.commit();
    }
    let error = validate_runtime_retirement_roots(&state).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unselected runtime retirement history"),
        "{error}"
    );
    {
        let mut block = state.world.block();
        block.runtime_retirements.remove(DefinitionId([0x79; 32]));
        block.commit();
    }
    let error = validate_runtime_retirement_roots(&state).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unselected runtime retirement history"),
        "{error}"
    );
}
