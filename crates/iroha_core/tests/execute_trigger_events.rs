//! Validate that by-call trigger execution emits both the trigger event and resulting data events.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::borrow::Cow;

use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    query::store::LiveQueryStore,
    smartcontracts::triggers::{
        set::{ExecutableRef, SetReadOnly},
        specialized::LoadedActionTrait,
    },
    state::{State, WorldReadOnly},
};
use iroha_data_model::prelude::*;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use mv::storage::StorageReadOnly;

fn build_state_and_ids() -> (State, ChainId, TriggerId, AssetId) {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
        domain_id.clone(),
        "rose".parse().expect("asset name"),
    );
    let asset_definition =
        AssetDefinition::new(asset_definition_id, NumericSpec::default()).build(&ALICE_ID);
    let stored_asset_definition_id = asset_definition.id().clone();
    let fee_domain_id =
        DomainId::parse_fully_qualified("universal.universal").expect("fee domain id");
    let fee_domain = Domain::new(fee_domain_id.clone()).build(&ALICE_ID);
    let fee_asset_definition_id =
        iroha_data_model::asset::AssetDefinitionId::new(fee_domain_id, "xor".parse().unwrap());
    let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
        .with_name("xor".to_owned())
        .build(&ALICE_ID);
    let fee_asset = Asset::new(
        AssetId::new(fee_asset_definition_id, ALICE_ID.clone()),
        Quantity::from(100_000_u64),
    );
    let world = iroha_core::state::World::with_assets(
        [domain, fee_domain],
        [account],
        [asset_definition, fee_asset_definition],
        [fee_asset],
        [],
    );

    let kura = iroha_core::kura::Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
    let state = State::new_with_chain_for_testing(world, kura.clone(), query, chain_id.clone());

    let trigger_id: TriggerId = "sse_smoke_trigger".parse().expect("trigger id");
    let asset_id = AssetId::new(stored_asset_definition_id, ALICE_ID.clone());
    state
        .view()
        .world()
        .asset_definition(asset_id.definition())
        .expect("seeded asset definition must be resolvable");

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
            vec![InstructionBox::from(Mint::asset_quantity(
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
    let register_tx = TransactionBuilder::new(
        chain_id.clone(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
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
    asset_id: &AssetId,
    parent: &iroha_core::block::CommittedBlock,
) -> (Vec<EventBox>, usize, Option<String>) {
    let exec_tx = TransactionBuilder::new(
        chain_id.clone(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
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
    execute_state_block
        .world()
        .asset_definition(asset_id.definition())
        .expect("execute block must see seeded asset definition");
    let valid_execute =
        ValidBlock::validate_unchecked(execute_block.into(), &mut execute_state_block)
            .unpack(|_| {});
    let committed_execute = valid_execute.commit_unchecked().unpack(|_| {});
    let events = execute_state_block.apply_without_execution(&committed_execute, Vec::new());
    let fragment_count = execute_state_block.committed_fragment_count();
    execute_state_block.commit().expect("execute block commits");
    let execute_error = committed_execute
        .as_ref()
        .error(0)
        .map(|error| format!("{error:?}"));
    (events, fragment_count, execute_error)
}

fn assert_trigger_registered(state: &State, trigger_id: &TriggerId, asset_id: &AssetId) {
    let view = state.view();
    let action = view
        .world()
        .triggers()
        .by_call_triggers()
        .get(trigger_id)
        .expect("trigger should be registered");
    let ExecutableRef::Instructions(instructions) = action.executable() else {
        panic!("trigger should store instruction executable");
    };
    let [instruction] = instructions.as_ref() else {
        panic!("trigger should store exactly one instruction");
    };
    let mint = match instruction.as_any().downcast_ref::<MintBox>() {
        Some(MintBox::Asset(mint)) => mint,
        _ => panic!("trigger instruction should mint a numeric asset"),
    };
    assert_eq!(
        mint.destination(),
        asset_id,
        "registered trigger must mint the seeded asset"
    );
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
    assert_trigger_registered(&state, &trigger_id, &asset_id);
    state
        .view()
        .world()
        .asset_definition(asset_id.definition())
        .expect("asset definition must survive trigger registration");

    let (events, fragment_count, execute_error) = execute_trigger(
        &state,
        &chain_id,
        &trigger_id,
        &asset_id,
        &committed_register,
    );
    assert!(
        execute_error.is_none(),
        "ExecuteTrigger transaction rejected: {execute_error:?}"
    );
    assert!(fragment_count > 0, "execute transaction should be applied");
    assert_trigger_events(&events, &trigger_id, &asset_id, &alice_id);
}
