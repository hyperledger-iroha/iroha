//! L1 checks both actual snapshot cuts and preserves original local admission refusal.

use super::*;
use crate::smartcontracts::isi::sccp::{generation_record::RetainedSccpGeneration, params};
use iroha_data_model::{block::consensus::PublicLaneStakingPolicy, parameter::Parameter};
use std::ops::Deref;

fn policy() -> PublicLaneStakingPolicy {
    let parameters = iroha_data_model::sccp::params::SccpParametersV1::taira_default();
    let asset = iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse()
        .unwrap();
    let mut policy = crate::sumeragi::test_chain::staking_fixture_policy(asset);
    policy.max_slash_bps = 1;
    policy.unbonding_delay_ms =
        parameters.committee_validity_ms + parameters.forgery_evidence_grace_ms;
    policy
}
fn world_with_policy() -> World {
    let mut world = World::default();
    world
        .set_parameter_for_testing(Parameter::Custom(policy().into_custom_parameter().unwrap()))
        .unwrap();
    crate::state::sccp_snapshot_state::tests::populate_sccp_world(world)
}

#[test]
fn restore_requires_original_policy_and_uses_no_replay_configuration() {
    let absent = World::default();
    let budget = absent
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    validate_sccp_policy_versions(&absent, &budget).unwrap();
    let world = crate::state::sccp_snapshot_state::tests::sccp_world();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let error = validate_sccp_policy_versions(&world, &budget).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("original committed public_lane_staking_v1")
    );
    let original = world_with_policy();
    let original_budget = original
        .sccp_committee_generations
        .allocation_budget()
        .clone();
    validate_sccp_policy_versions(&original, &original_budget).unwrap();
}

#[test]
fn undo_only_retained_liability_is_checked_against_its_complete_original_cut() {
    let world = world_with_policy();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let mut original = world
        .sccp_committee_generations
        .view()
        .get(&1)
        .unwrap()
        .deref()
        .clone();
    original.grace_ms += 1;
    original.liable_until_lc += 1;
    let original = RetainedSccpGeneration::admit(&original, &budget).unwrap();
    {
        let mut block = world.block();
        block
            .sccp_committee_generations
            .try_insert_admitted(1, original)
            .unwrap();
        block.commit();
    }
    {
        let mut block = world.block();
        block
            .sccp_committee_generations
            .try_remove_admitted(1)
            .unwrap();
        block.commit();
    }
    let policy = policy();
    let current = world.view();
    let parameters = params::parameters(&current).unwrap();
    params::validate_staking_policy(&current, &parameters, &policy).unwrap();
    drop(current);
    let error = validate_sccp_policy_versions(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("staking_policy.revert"),
        "{error}"
    );
    assert!(error.to_string().contains("generation 1"), "{error}");
    assert!(world.sccp_committee_generations.view().get(&1).is_none());
}

#[test]
fn original_undo_acquisition_refusal_remains_local_and_can_retry() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(0);
    let error = validate_sccp_policy_versions(&world, &budget).unwrap_err();
    assert!(matches!(error, StateRestoreError::Admission(_)), "{error}");
    budget.set_limit_bytes(limit);
    validate_sccp_policy_versions(&world, &budget).unwrap();
}
