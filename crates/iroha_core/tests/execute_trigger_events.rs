//! Validate that by-call trigger execution emits both the trigger event and resulting data events.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    query::store::LiveQueryStore,
    smartcontracts::triggers::set::SetReadOnly,
    state::{State, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use mv::storage::StorageReadOnly;

fn build_state_and_ids() -> (State, ChainId, TriggerId, AssetId) {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account = Account::new(ALICE_ID.clone().to_account_id(domain_id)).build(&ALICE_ID);
    let asset_definition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&ALICE_ID);
    let world = iroha_core::state::World::with([domain], [account], [asset_definition]);

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let state = {
        #[cfg(feature = "telemetry")]
        {
            State::new_with_chain(
                world,
                kura.clone(),
                query,
                chain_id.clone(),
                iroha_core::telemetry::StateTelemetry::default(),
            )
        }
        #[cfg(not(feature = "telemetry"))]
        {
            State::new_with_chain(world, kura.clone(), query, chain_id.clone())
        }
    };

    let trigger_id: TriggerId = "sse_smoke_trigger".parse().expect("trigger id");
    let asset_id = AssetId::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        ),
        ALICE_ID.clone(),
    );

    (state, chain_id, trigger_id, asset_id)
}

fn register_trigger(
    state: &State,
    chain_id: &ChainId,
    trigger_id: &TriggerId,
    asset_id: &AssetId,
) -> (iroha_core::block::CommittedBlock, usize) {
    let register_trigger = Register::trigger(Trigger::new(
        trigger_id.clone(),
        Action::new(
            vec![InstructionBox::from(Mint::asset_numeric(
                1_u32,
                asset_id.clone(),
            ))],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        ),
    ));
    let register_tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
        .with_instructions([register_trigger])
        .sign(ALICE_KEYPAIR.private_key());

    let register_block =
        BlockBuilder::new(vec![iroha_core::tx::AcceptedTransaction::new_unchecked(
            Cow::Owned(register_tx),
        )])
        .chain(0, None)
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut register_state_block = state.block(register_block.header());
    let valid_register =
        ValidBlock::validate_unchecked(register_block.into(), &mut register_state_block)
            .unpack(|_| {});
    let committed_register = valid_register.commit_unchecked().unpack(|_| {});
    let _ = register_state_block.apply_without_execution(&committed_register, Vec::new());
    let fragment_count = register_state_block.committed_fragment_count();
    register_state_block
        .commit()
        .expect("register block commits");
    (committed_register, fragment_count)
}

fn execute_trigger(
    state: &State,
    chain_id: &ChainId,
    trigger_id: &TriggerId,
    parent: &iroha_core::block::CommittedBlock,
) -> (Vec<EventBox>, usize, Option<String>) {
    let exec_tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
        .with_instructions([InstructionBox::from(ExecuteTrigger::new(
            trigger_id.clone(),
        ))])
        .sign(ALICE_KEYPAIR.private_key());

    let execute_block =
        BlockBuilder::new(vec![iroha_core::tx::AcceptedTransaction::new_unchecked(
            Cow::Owned(exec_tx),
        )])
        .chain(0, Some(parent.as_ref()))
        .sign(ALICE_KEYPAIR.private_key())
        .unpack(|_| {});
    let mut execute_state_block = state.block(execute_block.header());
    let valid_execute =
        ValidBlock::validate_unchecked(execute_block.into(), &mut execute_state_block)
            .unpack(|_| {});
    let committed_execute = valid_execute.commit_unchecked().unpack(|_| {});
    let events = execute_state_block.apply_without_execution(&committed_execute, Vec::new());
    let fragment_count = execute_state_block.committed_fragment_count();
    execute_state_block.commit().expect("execute block commits");
    let execute_error = committed_execute.as_ref().error(0).map(ToString::to_string);
    (events, fragment_count, execute_error)
}

fn assert_trigger_registered(state: &State, trigger_id: &TriggerId) {
    let registered = state
        .view()
        .world()
        .triggers()
        .by_call_triggers()
        .get(trigger_id)
        .is_some();
    assert!(registered, "trigger should be registered");
}

fn assert_trigger_events(
    events: &[EventBox],
    trigger_id: &TriggerId,
    asset_id: &AssetId,
    alice_id: &AccountId,
) {
    let mut saw_execute = false;
    let mut saw_asset_added = false;

    for ev in events {
        match ev {
            EventBox::ExecuteTrigger(ev) => {
                if ev.trigger_id() == trigger_id && ev.authority() == alice_id {
                    saw_execute = true;
                }
            }
            EventBox::Data(shared) => {
                if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Asset(
                    AssetEvent::Added(changed),
                ))) = shared.as_ref()
                    && changed.asset() == asset_id
                {
                    saw_asset_added = true;
                }
            }
            _ => {}
        }
    }

    assert!(
        saw_execute,
        "ExecuteTrigger event should be broadcast for by-call triggers"
    );
    assert!(
        saw_asset_added,
        "Minted asset should emit an AssetEvent::Added data event"
    );
}

#[test]
fn execute_trigger_emits_execute_and_data_events() {
    let (state, chain_id, trigger_id, asset_id) = build_state_and_ids();
    let alice_id = ALICE_ID.clone();

    let (committed_register, register_fragments) =
        register_trigger(&state, &chain_id, &trigger_id, &asset_id);
    assert!(
        register_fragments > 0,
        "register transaction should be applied"
    );
    assert_trigger_registered(&state, &trigger_id);

    let (events, fragment_count, execute_error) =
        execute_trigger(&state, &chain_id, &trigger_id, &committed_register);
    assert!(
        execute_error.is_none(),
        "ExecuteTrigger transaction rejected: {execute_error:?}"
    );
    assert!(fragment_count > 0, "execute transaction should be applied");
    assert_trigger_events(&events, &trigger_id, &asset_id, &alice_id);
}
