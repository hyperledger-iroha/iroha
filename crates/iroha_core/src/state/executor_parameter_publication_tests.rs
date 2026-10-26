//! Executor default reconciliation prepares exact immutable graphs before writes.
use super::*;
use crate::execution_attempt::ExecutionAttemptError;
use iroha_data_model::parameter::CustomParameter;
use iroha_test_samples::ALICE_ID;

fn declared(id: &str, value: u32) -> ExecutorDataModel {
    let id: iroha_data_model::parameter::CustomParameterId = id.parse().unwrap();
    let mut model = ExecutorDataModel::default();
    model
        .parameters
        .insert(CustomParameter::new(id, Json::new(value)));
    model
}
#[test]
fn refusal_leaves_model_and_parameters_unchanged_and_retry_reconciles() {
    let retired = Permission::new("RetiredPermission".to_owned(), Json::new(()));
    let role_id: RoleId = "executor_reconciliation_refusal".parse().unwrap();
    let role = Role {
        id: role_id.clone(),
        permissions: BTreeSet::from([retired.clone()]),
        permission_epochs: BTreeMap::from([(retired.clone(), 5)]),
    };
    let mut world = World::with_assets_and_roles(
        [],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
        [],
        [],
        [role],
    );
    world
        .account_permissions
        .insert(ALICE_ID.clone(), BTreeSet::from([retired.clone()]));
    let mut block = world.block();
    let mut tx = block.transaction_without_telemetry(LaneConfig::default(), 0);
    let pool = tx.sccp_committee_generations.allocation_budget().clone();
    let original = tx.parameters.get().clone();
    let model = tx.executor_data_model.get().clone();
    let held = pool.reserved_bytes();
    let original_limit = pool.limit_bytes();
    pool.set_limit_bytes(held);
    let error = tx
        .apply_executor_data_model(declared("executor_owned", 17))
        .unwrap_err();
    let ExecutionAttemptError::Deferred(original_refusal) = error else {
        panic!("finite original pool refusal must remain unfinished execution");
    };
    assert!(original_refusal.allocation_refusal().is_some());
    assert!(original.same_version(tx.parameters.get()));
    assert_eq!(tx.executor_data_model.get(), &model);
    assert_eq!(
        tx.account_permissions.get(&ALICE_ID),
        Some(&BTreeSet::from([retired.clone()]))
    );
    let role = tx.roles.get(&role_id).unwrap();
    assert_eq!(role.permissions, BTreeSet::from([retired.clone()]));
    assert_eq!(role.permission_epochs.get(&retired), Some(&5));
    assert_eq!(pool.reserved_bytes(), held);
    pool.set_limit_bytes(original_limit);
    tx.apply_executor_data_model(declared("executor_owned", 17))
        .unwrap();
    assert_eq!(
        tx.parameters
            .get()
            .custom()
            .get(&"executor_owned".parse().unwrap())
            .unwrap()
            .payload(),
        &Json::new(17_u32)
    );
    assert!(original.same_version(tx.parameters.get_before_block()));
    assert!(tx.account_permissions.get(&ALICE_ID).is_none());
    let role = tx.roles.get(&role_id).unwrap();
    assert!(role.permissions.is_empty());
    assert!(role.permission_epochs.is_empty());
}
#[test]
fn explicit_override_survives_default_change_and_removal_is_prepared() {
    let world = World::default();
    let mut block = world.block();
    let mut tx = block.transaction_without_telemetry(LaneConfig::default(), 0);
    let id: iroha_data_model::parameter::CustomParameterId = "executor_owned".parse().unwrap();
    tx.apply_executor_data_model(declared("executor_owned", 17))
        .unwrap();
    tx.set_parameter_for_testing(Parameter::Custom(CustomParameter::new(
        id.clone(),
        Json::new(91_u32),
    )))
    .unwrap();
    tx.apply_executor_data_model(declared("executor_owned", 19))
        .unwrap();
    assert_eq!(
        tx.parameters.get().custom().get(&id).unwrap().payload(),
        &Json::new(91_u32)
    );
    tx.apply_executor_data_model(ExecutorDataModel::default())
        .unwrap();
    assert!(tx.parameters.get().custom().get(&id).is_none());
}
#[test]
fn executor_key_mismatch_cannot_smuggle_a_consensus_owned_parameter() {
    let world = World::default();
    let mut block = world.block();
    let mut tx = block.transaction_without_telemetry(LaneConfig::default(), 0);
    let original = tx.parameters.get().clone();
    let mut model = ExecutorDataModel::default();
    let custom = crate::sumeragi::test_chain::staking_fixture_policy(
        iroha_data_model::parameter::system::SumeragiNposParameters::default()
            .xor_asset_definition_id,
    )
    .into_custom_parameter()
    .unwrap();
    // The canonical collection derives keys from immutable entries. A separate
    // offered key is rejected by its update boundary before a malformed executor
    // declaration can exist or reach State publication.
    assert!(matches!(
        model.parameters.try_with_update(
            &"executor_disguise".parse().unwrap(),
            Some(&custom),
            tx.sccp_committee_generations.allocation_budget(),
        ),
        Err(iroha_data_model::parameter::CustomParametersAdmissionError::KeyValueMismatch)
    ));
    assert!(model.parameters.is_empty());
    // The canonical collection stores each identity once, so a mismatched key
    // must be rejected by the actual executor-model reader before publication.
    let mut spoofed = norito::json::value::to_value(&model).unwrap();
    let mut parameters = norito::json::Map::new();
    parameters.insert(
        "executor_disguise".to_owned(),
        norito::json::value::to_value(&custom).unwrap(),
    );
    spoofed.as_object_mut().unwrap().insert(
        "parameters".to_owned(),
        norito::json::Value::Object(parameters),
    );
    let error = norito::json::value::from_value::<ExecutorDataModel>(spoofed)
        .expect_err("a wire key cannot disguise the original consensus-owned identity");
    assert!(
        error.to_string().contains("matching each value identity"),
        "{error}"
    );
    assert!(original.same_version(tx.parameters.get()));
    assert!(tx.parameters.get().staking_policy().is_none());

    // Even an honestly named executor declaration cannot author consensus policy.
    let id = custom.id().clone();
    model.parameters.insert(custom);
    tx.apply_executor_data_model(model)
        .expect("consensus-owned executor declarations are excluded");
    assert!(tx.parameters.get().staking_policy().is_none());
    assert!(!tx.executor_data_model.get().parameters.contains_key(&id));
}
