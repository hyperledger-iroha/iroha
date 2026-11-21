//! Selected staging, exact source scope, immutable identity and original local refusal.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{
    Registrable, account::Account, block::consensus::SumeragiRootScope,
    smart_contract::ContractArtifactId,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{ALICE_ID, BOB_ID};

fn state() -> State {
    let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.world.block();
    block
        .set_parameter_for_testing(crate::sumeragi::lanes::routing::test_support::metadata(
            SumeragiRootScope::Global,
        ))
        .unwrap();
    block.commit();
    state
}

fn select(block: &mut crate::state::StateBlock<'_>) {
    let catalog = crate::runtime::release_catalog().unwrap();
    let applied = {
        let previous = block.world.runtime_selection.get();
        let published =
            crate::runtime::change_set::PublishedDefinitions(&block.world.runtime_definitions);
        let change = catalog
            .activation(
                crate::runtime::catalog::additions::GENERIC_CARRIER_ARTIFACTS.module,
                previous,
                &published,
                *block.network_id(),
                1,
                [0; 32],
            )
            .unwrap();
        change.apply(previous, &published).unwrap()
    };
    for (id, definition) in applied.published {
        block.world.runtime_definitions.insert(id, definition);
    }
    *block.world.runtime_selection.get_mut() = applied.selection;
}

fn request() -> StageRuntimeCarrierArtifactV1 {
    let code = super::super::execution::tests::program(&[
        ivm_abi::encoding::wide::encode_ri(ivm_abi::instruction::wide::arithmetic::ADDI, 10, 0, 0),
        ivm_abi::encoding::wide::encode_halt(),
    ]);
    StageRuntimeCarrierArtifactV1 {
        artifact_id: ContractArtifactId::new(
            DataSpaceId::UNIVERSAL,
            ivm::contract_code_hash(&code),
        ),
        code,
    }
}

fn scope(state: &mut StateTransaction<'_, '_>) {
    state.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
    state.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
}

#[test]
fn selected_staging_registers_exact_code_idempotently_before_carrier_kernel_selection() {
    let state = state();
    let request = request();
    let mut block = state.block(super::super::execution::tests::header());
    select(&mut block);
    let mut transaction = block.transaction();
    scope(&mut transaction);
    assert_eq!(
        transaction.world.runtime_selection.get().kernel,
        crate::runtime::release_catalog().unwrap().baseline.kernel
    );
    request
        .clone()
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
    request
        .clone()
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
    assert_eq!(
        transaction
            .world
            .runtime_generic_artifacts
            .get(&request.artifact_id)
            .unwrap()
            .as_slice(),
        request.code
    );
    assert!(
        transaction
            .world
            .contract_code
            .get(&request.artifact_id)
            .is_none()
    );
}

#[test]
fn absent_selection_unknown_account_foreign_scope_hash_and_profile_are_refused() {
    let absent = state();
    let mut block = absent.block(super::super::execution::tests::header());
    let mut transaction = block.transaction();
    scope(&mut transaction);
    assert!(request().execute(&ALICE_ID, &mut transaction).is_err());
    drop(transaction);
    drop(block);
    let selected = state();
    let mut block = selected.block(super::super::execution::tests::header());
    select(&mut block);
    let mut transaction = block.transaction();
    scope(&mut transaction);
    assert!(request().execute(&BOB_ID, &mut transaction).is_err());
    let mut foreign = request();
    foreign.artifact_id.dataspace_id = DataSpaceId::new(17);
    assert!(foreign.execute(&ALICE_ID, &mut transaction).is_err());
    let mut wrong_hash = request();
    wrong_hash.code.push(0);
    assert!(wrong_hash.execute(&ALICE_ID, &mut transaction).is_err());
    let mut native = request();
    native.code =
        ivm::ProgramMetadata::default().encode_for_profile(ivm::IvmProfileId::C10_ABI_V1, 0);
    native
        .code
        .extend_from_slice(&ivm_abi::encoding::wide::encode_halt().to_le_bytes());
    native.artifact_id.code_hash = ivm::contract_code_hash(&native.code);
    assert!(native.execute(&ALICE_ID, &mut transaction).is_err());
    assert_eq!(
        transaction.world.runtime_generic_artifacts.iter().count(),
        0
    );
}

#[test]
fn refused_verifier_allocation_keeps_its_original_owner_and_no_staged_bytes() {
    let state = state();
    let mut block = state.block(super::super::execution::tests::header());
    select(&mut block);
    let mut transaction = block.transaction();
    scope(&mut transaction);
    let budget = transaction.execution_budget();
    let previous = budget.limit_bytes();
    budget.set_limit_bytes(budget.reserved_bytes());
    assert!(request().execute(&ALICE_ID, &mut transaction).is_err());
    budget.set_limit_bytes(previous);
    assert!(transaction.ensure_execution_ready().is_err());
    assert_eq!(
        transaction.world.runtime_generic_artifacts.iter().count(),
        0
    );
}
