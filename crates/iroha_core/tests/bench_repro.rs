//! Reproduces the actual signed-genesis ISI gas calibration fixture.

#[path = "../benches/support/isi_gas_fixture.rs"]
mod isi_gas_fixture;

use iroha_core::{
    executor::{Executor, InstructionExecutionProfile},
    gas,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{block::consensus::SumeragiRootScope, prelude::*};
use iroha_model_base::domain::DomainId;
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;

fn build_bench_state() -> (State, AccountId) {
    let (authority, _) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [],
    );
    (
        isi_gas_fixture::state_after_signed_genesis(
            world,
            vec![
                Grant::account_permission(
                    iroha_executor_data_model::permission::role::CanManageRoles,
                    authority.clone(),
                )
                .into(),
            ],
        ),
        authority,
    )
}

#[test]
fn fixture_retains_signed_genesis_and_original_root_authority() {
    let (state, authority) = build_bench_state();
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.exact_durable_block_count().unwrap(), 1);
    let genesis = state
        .block_by_height(1_usize.try_into().unwrap())
        .expect("original genesis read completes")
        .expect("original genesis is retained");
    assert_eq!(
        state.network_id_ref(),
        &NetworkId::from_genesis_hash(genesis.hash())
    );
    let signed = iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(&genesis)
        .expect("signed genesis metadata");
    assert_eq!(
        signed.sumeragi_context.root_scope,
        SumeragiRootScope::Global
    );
    assert_eq!(
        iroha_core::sumeragi::lanes::routing::committed_root_scope(state.view().world()),
        Some(signed.sumeragi_context.root_scope),
    );
    assert!(state.view().world().account(&authority).is_ok());
    let _ = isi_gas_fixture::block_header(&state);
}

#[test]
fn execute_register_account_like_bench_rolls_back_after_each_iteration() {
    let (state, authority) = build_bench_state();
    let (registered, _) = gen_account_in("wonderland");
    let domain_id = DomainId::try_new("wonderland", "universal").unwrap();
    for _ in 0..2 {
        let mut block = state.block(isi_gas_fixture::block_header(&state));
        let mut tx = block.transaction();
        let instruction: InstructionBox =
            Register::account(Account::new(registered.clone())).into();
        let quote = gas::meter_instruction(&instruction, &gas::DEFAULT_ZK_GAS)
            .expect("gas quote completes");
        assert!(quote > 0);
        Executor::default()
            .execute_instruction_with_profile(
                &mut tx,
                &authority,
                instruction,
                InstructionExecutionProfile::Bench,
            )
            .expect("actual benchmark-profile instruction execution");
        assert!(tx.world.accounts().get(&registered).is_some());
        assert!(tx.world.accounts().get(&authority).is_some());
        assert!(tx.world.domains().get(&domain_id).is_some());
        // Match the bench: no apply or block commit, so the next fresh
        // transaction sees the original committed State and can register again.
        drop(tx);
        drop(block);
        assert!(state.view().world().accounts().get(&registered).is_none());
        assert_eq!(state.committed_height(), 1);
    }
}

#[test]
fn execute_domain_owned_asset_definition_like_bench_rolls_back_after_each_iteration() {
    let (state, authority) = build_bench_state();
    let domain = DomainId::try_new("wonderland", "universal").unwrap();
    let definition = isi_gas_fixture::domain_owned_asset_definition(domain.clone(), "xor");
    let id = definition.id().clone();
    for _ in 0..2 {
        let mut block = state.block(isi_gas_fixture::block_header(&state));
        let mut tx = block.transaction();
        Executor::default()
            .execute_instruction_with_profile(
                &mut tx,
                &authority,
                Register::asset_definition(definition.clone()).into(),
                InstructionExecutionProfile::Bench,
            )
            .expect("domain owner registers the benchmark asset after genesis");
        let registered = tx.world.asset_definition(&id).unwrap();
        assert_eq!(registered.owning_domain(), &Some(domain.clone()));
        assert_eq!(registered.owned_by(), &authority);
        drop(tx);
        drop(block);
        assert!(state.view().world().asset_definition(&id).is_err());
        assert_eq!(state.committed_height(), 1);
    }
}

#[test]
fn execute_role_grant_and_revoke_like_bench_preserves_holder_and_rolls_back() {
    for recipient_holds_role in [false, true] {
        let (state, authority) = build_bench_state();
        let (recipient, _) = gen_account_in("wonderland");
        let role: RoleId = "bench_role".parse().unwrap();
        let execute = |tx: &mut iroha_core::state::StateTransaction<'_, '_>, instruction| {
            Executor::default()
                .execute_instruction_with_profile(
                    tx,
                    &authority,
                    instruction,
                    InstructionExecutionProfile::Bench,
                )
                .expect("authorized benchmark role operation");
        };
        {
            let mut block = state.block(isi_gas_fixture::block_header(&state));
            let mut tx = block.transaction();
            execute(
                &mut tx,
                Register::account(Account::new(recipient.clone())).into(),
            );
            execute(
                &mut tx,
                Register::role(Role::new(role.clone(), authority.clone())).into(),
            );
            if recipient_holds_role {
                execute(
                    &mut tx,
                    Grant::account_role(role.clone(), recipient.clone()).into(),
                );
            }
            tx.apply();
            block
                .commit_world_overlay_for_testing()
                .expect("commit role setup");
        }
        for _ in 0..2 {
            let mut block = state.block(isi_gas_fixture::block_header(&state));
            let mut tx = block.transaction();
            let instruction = if recipient_holds_role {
                Revoke::account_role(role.clone(), recipient.clone()).into()
            } else {
                Grant::account_role(role.clone(), recipient.clone()).into()
            };
            execute(&mut tx, instruction);
            assert!(
                tx.world
                    .account_roles_iter(&authority)
                    .any(|held| held == &role)
            );
            assert_eq!(
                tx.world
                    .account_roles_iter(&recipient)
                    .any(|held| held == &role),
                !recipient_holds_role,
            );
            drop(tx);
            drop(block);
            let view = state.view();
            assert!(
                view.world()
                    .account_roles_iter(&authority)
                    .any(|held| held == &role)
            );
            assert_eq!(
                view.world()
                    .account_roles_iter(&recipient)
                    .any(|held| held == &role),
                recipient_holds_role,
            );
            assert_eq!(state.committed_height(), 1);
        }
    }
}

#[test]
fn execute_trigger_like_bench_owns_callback_and_rolls_back_consumption() {
    use iroha_core::smartcontracts::isi::triggers::set::SetReadOnly;

    let (state, authority) = build_bench_state();
    let id: TriggerId = "bench_callback".parse().unwrap();
    {
        let mut block = state.block(isi_gas_fixture::block_header(&state));
        let mut tx = block.transaction();
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            authority.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(id.clone())
                .under_authority(authority.clone()),
        )
        .unwrap();
        Executor::default()
            .execute_instruction_with_profile(
                &mut tx,
                &authority,
                Register::trigger(Trigger::new(id.clone(), action)).into(),
                InstructionExecutionProfile::Bench,
            )
            .expect("register callback through the permission-enforcing executor");
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
    }
    for _ in 0..2 {
        let mut block = state.block(isi_gas_fixture::block_header(&state));
        let mut tx = isi_gas_fixture::component_transaction(&mut block);
        Executor::default()
            .execute_instruction_with_profile(
                &mut tx,
                &authority,
                ExecuteTrigger::new(id.clone()).into(),
                InstructionExecutionProfile::Bench,
            )
            .expect("empty callback retains its bounded pre-body invocation owner");
        assert!(tx.world.triggers().inspect_by_id(&id, |_| ()).is_none());
        drop(tx);
        drop(block);
        assert_eq!(
            state
                .view()
                .world()
                .triggers()
                .inspect_by_id(&id, |action| *action.repeats()),
            Some(Repeats::Exactly(1)),
            "dropping the measured overlay restores callback consumption",
        );
        assert_eq!(state.committed_height(), 1);
    }
}
