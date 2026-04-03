//! Integration tests covering settlement admission and overlay execution paths.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::unwrap_used)]

use std::str::FromStr as _;

use iroha_core::{
    kura::Kura,
    pipeline::overlay::TxOverlay,
    query::store::LiveQueryStore,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    account::Account,
    asset::{
        Asset, AssetDefinition,
        prelude::{AssetDefinitionId, AssetId},
    },
    block::BlockHeader,
    domain::{Domain, DomainId},
    isi::{
        InstructionBox,
        error::{InstructionEvaluationError, InstructionExecutionError, TypeError},
        settlement::{
            DvpIsi, PvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
            SettlementPlan,
        },
    },
    metadata::Metadata,
    prelude::{AccountId, Numeric, NumericSpec, ValidationFail},
};
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn settlement_state() -> (State, AssetDefinitionId, AssetDefinitionId) {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

    let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

    let delivery_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "bond".parse().unwrap(),
    );
    let payment_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "usd".parse().unwrap(),
    );

    let delivery_def = AssetDefinition::numeric(delivery_def_id.clone()).build(&ALICE_ID);
    let payment_def = AssetDefinition::numeric(payment_def_id.clone()).build(&ALICE_ID);

    let alice_delivery = Asset::new(
        AssetId::new(delivery_def_id.clone(), ALICE_ID.clone()),
        Numeric::from(10u32),
    );
    let bob_payment = Asset::new(
        AssetId::new(payment_def_id.clone(), BOB_ID.clone()),
        Numeric::from(1_000u32),
    );

    let world = World::with_assets(
        [domain],
        [alice, bob],
        [delivery_def, payment_def],
        [alice_delivery, bob_payment],
        [],
    );

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);

    (state, delivery_def_id, payment_def_id)
}

fn settlement_state_with_payment_spec(
    payment_spec: NumericSpec,
) -> (State, AssetDefinitionId, AssetDefinitionId) {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

    let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);

    let delivery_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "bond".parse().unwrap(),
    );
    let payment_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "usd".parse().unwrap(),
    );

    let delivery_def =
        AssetDefinition::new(delivery_def_id.clone(), NumericSpec::integer()).build(&ALICE_ID);
    let payment_def = AssetDefinition::new(payment_def_id.clone(), payment_spec).build(&ALICE_ID);

    let alice_delivery = Asset::new(
        AssetId::new(delivery_def_id.clone(), ALICE_ID.clone()),
        Numeric::from(5u32),
    );
    let bob_payment = Asset::new(
        AssetId::new(payment_def_id.clone(), BOB_ID.clone()),
        Numeric::from(2u32),
    );

    let world = World::with_assets(
        [domain],
        [alice, bob],
        [delivery_def, payment_def],
        [alice_delivery, bob_payment],
        [],
    );

    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);

    (state, delivery_def_id, payment_def_id)
}

fn apply_overlay(
    stx: &mut iroha_core::state::StateTransaction<'_, '_>,
    authority: &AccountId,
    instructions: Vec<InstructionBox>,
) -> Result<(), ValidationFail> {
    TxOverlay::from_instructions(instructions).apply(stx, authority)
}

fn asset_balance(
    stx: &iroha_core::state::StateTransaction<'_, '_>,
    definition: &AssetDefinitionId,
    account: &AccountId,
) -> Numeric {
    WorldReadOnly::assets(&stx.world)
        .get(&AssetId::new(definition.clone(), account.clone()))
        .map_or_else(Numeric::zero, |owned| owned.0.clone())
}

#[test]
fn dvp_overlay_rejects_underfunded_leg() {
    let (state, delivery_def_id, payment_def_id) = settlement_state();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let instruction = DvpIsi {
        settlement_id: "overlay_underfunded".parse().unwrap(),
        delivery_leg: SettlementLeg::new(
            delivery_def_id.clone(),
            Numeric::from(5u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        payment_leg: SettlementLeg::new(
            payment_def_id.clone(),
            Numeric::from(2_000u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        plan: SettlementPlan::new(
            SettlementExecutionOrder::PaymentThenDelivery,
            SettlementAtomicity::AllOrNothing,
        ),
        metadata: Metadata::default(),
    };

    let err = apply_overlay(&mut stx, &ALICE_ID, vec![InstructionBox::from(instruction)])
        .expect_err("insufficient payment leg should fail admission");
    match err {
        ValidationFail::InstructionFailed(exec_err) => {
            let msg = exec_err.to_string();
            assert!(msg.contains("available"), "unexpected error message: {msg}");
        }
        other => panic!("unexpected validation error: {other:?}"),
    }
    // Ensure balances remain unchanged
    let alice_bond = AssetId::new(delivery_def_id, ALICE_ID.clone());
    let bob_cash = AssetId::new(payment_def_id, BOB_ID.clone());
    let alice_balance = WorldReadOnly::assets(&stx.world)
        .get(&alice_bond)
        .map_or_else(Numeric::zero, |owned| owned.0.clone());
    let bob_balance = WorldReadOnly::assets(&stx.world)
        .get(&bob_cash)
        .map_or_else(Numeric::zero, |owned| owned.0.clone());
    assert_eq!(alice_balance, Numeric::from(10u32));
    assert_eq!(bob_balance, Numeric::from(1_000u32));
}

#[test]
fn pvp_overlay_executes_when_funded() {
    let (state, primary_def_id, counter_def_id) = settlement_state();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let instruction = PvpIsi {
        settlement_id: "overlay_funded_fx".parse().unwrap(),
        primary_leg: SettlementLeg::new(
            primary_def_id.clone(),
            Numeric::from(5u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        counter_leg: SettlementLeg::new(
            counter_def_id.clone(),
            Numeric::from(500u32),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        plan: SettlementPlan::default(),
        metadata: Metadata::default(),
    };

    apply_overlay(&mut stx, &ALICE_ID, vec![InstructionBox::from(instruction)])
        .expect("funded PvP overlay should succeed");

    assert_eq!(
        asset_balance(&stx, &primary_def_id, &ALICE_ID),
        Numeric::from(5u32)
    );
    assert_eq!(
        asset_balance(&stx, &primary_def_id, &BOB_ID),
        Numeric::from(5u32)
    );
    assert_eq!(
        asset_balance(&stx, &counter_def_id, &ALICE_ID),
        Numeric::from(500u32)
    );
    assert_eq!(
        asset_balance(&stx, &counter_def_id, &BOB_ID),
        Numeric::from(500u32)
    );
}

#[test]
fn dvp_overlay_commit_first_keeps_delivery_on_payment_failure() {
    let (state, delivery_def_id, payment_def_id) =
        settlement_state_with_payment_spec(NumericSpec::fractional(2));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let instruction = DvpIsi {
        settlement_id: "overlay_commit_first".parse().unwrap(),
        delivery_leg: SettlementLeg::new(
            delivery_def_id.clone(),
            Numeric::from(5u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        payment_leg: SettlementLeg::new(
            payment_def_id.clone(),
            Numeric::from_str("1.001").expect("numeric"),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        plan: SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::CommitFirstLeg,
        ),
        metadata: Metadata::default(),
    };

    let err = apply_overlay(&mut stx, &ALICE_ID, vec![InstructionBox::from(instruction)])
        .expect_err("scale violation should fail payment leg");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::Evaluate(
            InstructionEvaluationError::Type(TypeError::AssetNumericSpec(_)),
        )) => {}
        ValidationFail::InstructionFailed(exec_err) => panic!("unexpected error: {exec_err:?}"),
        other => panic!("unexpected validation error: {other:?}"),
    }

    let alice_delivery = asset_balance(&stx, &delivery_def_id, &ALICE_ID);
    let bob_delivery = asset_balance(&stx, &delivery_def_id, &BOB_ID);
    let alice_cash = asset_balance(&stx, &payment_def_id, &ALICE_ID);
    let bob_cash = asset_balance(&stx, &payment_def_id, &BOB_ID);

    assert!(
        alice_delivery.is_zero(),
        "seller delivery leg should remain debited"
    );
    assert_eq!(bob_delivery, Numeric::from(5u32));
    assert_eq!(alice_cash, Numeric::zero());
    assert_eq!(bob_cash, Numeric::from(2u32));
}

#[test]
fn dvp_overlay_commit_second_rolls_back_on_payment_failure() {
    let (state, delivery_def_id, payment_def_id) =
        settlement_state_with_payment_spec(NumericSpec::fractional(2));
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();

    let alice_delivery_before = asset_balance(&stx, &delivery_def_id, &ALICE_ID);
    let bob_delivery_before = asset_balance(&stx, &delivery_def_id, &BOB_ID);
    let bob_cash_before = asset_balance(&stx, &payment_def_id, &BOB_ID);

    let instruction = DvpIsi {
        settlement_id: "overlay_commit_second".parse().unwrap(),
        delivery_leg: SettlementLeg::new(
            delivery_def_id.clone(),
            Numeric::from(5u32),
            ALICE_ID.clone(),
            BOB_ID.clone(),
        ),
        payment_leg: SettlementLeg::new(
            payment_def_id.clone(),
            Numeric::from_str("1.001").expect("numeric"),
            BOB_ID.clone(),
            ALICE_ID.clone(),
        ),
        plan: SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::CommitSecondLeg,
        ),
        metadata: Metadata::default(),
    };

    let err = apply_overlay(&mut stx, &ALICE_ID, vec![InstructionBox::from(instruction)])
        .expect_err("scale violation should fail payment leg");
    match err {
        ValidationFail::InstructionFailed(InstructionExecutionError::Evaluate(
            InstructionEvaluationError::Type(TypeError::AssetNumericSpec(_)),
        )) => {}
        ValidationFail::InstructionFailed(exec_err) => panic!("unexpected error: {exec_err:?}"),
        other => panic!("unexpected validation error: {other:?}"),
    }

    assert_eq!(
        asset_balance(&stx, &delivery_def_id, &ALICE_ID),
        alice_delivery_before,
        "delivery leg should roll back",
    );
    assert_eq!(
        asset_balance(&stx, &delivery_def_id, &BOB_ID),
        bob_delivery_before,
        "buyer delivery balance should remain unchanged",
    );
    assert_eq!(
        asset_balance(&stx, &payment_def_id, &BOB_ID),
        bob_cash_before,
        "payer cash balance should stay intact",
    );
}
