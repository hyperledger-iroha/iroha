//! Native custody captures, terminal lifetime and refusal before monetary effects.

use super::*;
use crate::{
    runtime::{
        activate_additions_for_testing, catalog::additions::NATIVE_ESCROW_CAPTURE,
        obligations::Step,
    },
    state::{State, World},
};
use iroha_data_model::{
    block::BlockHeader,
    escrow::AssetEscrowStatus,
    isi::escrow::{AcceptAssetEscrow, ReleaseAssetEscrow},
    runtime::definition::{DefinitionDescriptorV1, ImplementationCodeV1},
};
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::ALICE_ID;

fn state(selected: bool) -> State {
    let state = State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    if selected {
        activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    }
    state
}
fn header() -> BlockHeader {
    BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1000, 0)
}
fn row(seed: u8) -> AssetEscrowRecord {
    AssetEscrowRecord {
        id: EscrowId::new(Hash::prehashed([seed; 32])),
        seller: ALICE_ID.clone(),
        buyer: None,
        asset_definition: iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
            .parse()
            .unwrap(),
        amount: Quantity::from(5_u32),
        custody: ALICE_ID.clone(),
        status: AssetEscrowStatus::Open,
        kind: AssetEscrowKind::Marketplace,
        remaining_amount: Quantity::from(5_u32),
        release_authority: None,
        expires_at_ms: None,
        evidence_hashes: vec![],
        conditions: vec![],
        created_at_ms: 1000,
        accepted_at_ms: None,
        payment_sent_at_ms: None,
        disputed_at_ms: None,
        closed_at_ms: None,
        resolution: None,
    }
}
fn install(tx: &mut StateTransaction<'_, '_>, row: &AssetEscrowRecord, kind: Kind) {
    let prepared = prepare_opening(tx, row, kind).unwrap();
    tx.world
        .insert_captured_asset_escrow_entry(row, prepared)
        .unwrap();
}
fn rebind(tx: &mut StateTransaction<'_, '_>, wire: &str) {
    let selection = tx.world.runtime_selection.canonical().clone();
    let old = selection
        .bindings
        .iter()
        .find(|binding| binding.wire_id == wire)
        .unwrap();
    let DefinitionDescriptorV1::Implementation(mut implementation) = tx
        .world
        .runtime_definitions
        .get(&old.implementation)
        .unwrap()
        .clone()
    else {
        panic!("implementation")
    };
    implementation.code = ImplementationCodeV1::Native {
        module: "fixture.native-escrow.replacement".into(),
        reference_closure: [0xA5; 32],
    };
    let descriptor = DefinitionDescriptorV1::Implementation(implementation);
    let id = descriptor.id().unwrap();
    tx.world.runtime_definitions.insert(id, descriptor);
    let selection = tx.world.runtime_selection.get_mut();
    selection.active.push(id);
    selection.active.sort();
    selection
        .bindings
        .iter_mut()
        .find(|binding| binding.wire_id == wire)
        .unwrap()
        .implementation = id;
}

#[test]
fn inactive_service_is_inert_and_activation_drains_earlier_custody() {
    let state = state(false);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let row = row(1);
    install(&mut tx, &row, Kind::Marketplace);
    assert!(tx.world.runtime_escrow_captures.iter().next().is_none());
    assert!(
        prepare_execution(&tx, &AcceptAssetEscrow { escrow_id: row.id }.into())
            .unwrap()
            .is_none()
    );
    let before = tx.world.runtime_selection.canonical();
    let catalog = super::super::release_catalog().unwrap();
    let after = catalog
        .activation(
            NATIVE_ESCROW_CAPTURE.module,
            before,
            &catalog.definitions,
            tx.network_id,
            10,
            [0; 32],
        )
        .unwrap()
        .apply(before, &catalog.definitions)
        .unwrap()
        .selection;
    assert!(check_transition(&*tx.world, before, &after).is_err());
}

#[test]
fn row_and_capture_are_atomic_until_authoritative_terminal_status() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let mut row = row(2);
    assert!(tx.world.insert_asset_escrow_entry(&row).is_err());
    assert!(tx.world.asset_escrows.get(&row.id).is_none());
    install(&mut tx, &row, Kind::Marketplace);
    let captured = tx
        .world
        .runtime_escrow_captures
        .get(&row.id)
        .unwrap()
        .clone();
    row.status = AssetEscrowStatus::Accepted;
    row.accepted_at_ms = Some(1001);
    tx.world.insert_asset_escrow_entry(&row).unwrap();
    assert_eq!(
        tx.world.runtime_escrow_captures.get(&row.id),
        Some(&captured)
    );
    let mut substituted = row.clone();
    substituted.amount = Quantity::from(6_u32);
    assert!(tx.world.insert_asset_escrow_entry(&substituted).is_err());
    assert_eq!(tx.world.asset_escrows.get(&row.id), Some(&row));
    row.status = AssetEscrowStatus::Cancelled;
    row.remaining_amount = Quantity::zero();
    row.closed_at_ms = Some(1002);
    tx.world.insert_asset_escrow_entry(&row).unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&row.id).is_none());
    row.status = AssetEscrowStatus::Open;
    assert!(tx.world.insert_asset_escrow_entry(&row).is_err());
}

#[test]
fn direct_scope_preserves_acceptance_step_and_never_synthesizes_missing_capture() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let row = row(3);
    install(&mut tx, &row, Kind::Marketplace);
    let accept = AcceptAssetEscrow { escrow_id: row.id }.into();
    let release = ReleaseAssetEscrow { escrow_id: row.id }.into();
    assert_eq!(
        prepare_execution(&tx, &accept).unwrap().unwrap().step,
        Step::Continue
    );
    assert_eq!(
        prepare_execution(&tx, &release).unwrap().unwrap().step,
        Step::Complete
    );
    tx.world.runtime_escrow_captures.remove(row.id);
    assert!(prepare_execution(&tx, &accept).is_err());
    assert!(guard_existing(&tx, &row.id).is_err());
}

#[test]
fn opening_and_existing_custody_refuse_a_different_enclosing_selection() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let row = row(4);
    install(&mut tx, &row, Kind::Marketplace);
    let retained = tx
        .world
        .runtime_escrow_captures
        .get(&row.id)
        .unwrap()
        .capture
        .clone();
    let old = retained
        .execution_selection(
            tx.world.runtime_selection.canonical(),
            &PublishedDefinitions(&tx.world.runtime_definitions),
        )
        .unwrap();
    let wire = wire_id_of::<ReleaseAssetEscrow>().unwrap();
    rebind(&mut tx, wire);
    assert!(
        guard_existing(&tx, &row.id).is_err(),
        "canonical replacement cannot mutate old custody without its scope"
    );
    tx.world.runtime_selection.set_executing(Some(old));
    assert!(guard_existing(&tx, &row.id).is_ok());
    assert!(
        prepare_opening(&tx, &self::row(5), Kind::Marketplace).is_err(),
        "new custody cannot capture an older user-selected context"
    );
    tx.world.runtime_selection.set_executing(None);
    let prepared = prepare_opening(&tx, &self::row(5), Kind::Marketplace).unwrap();
    tx.world
        .insert_captured_asset_escrow_entry(&self::row(5), prepared)
        .unwrap();
}

#[test]
fn child_partition_inherits_parent_capture_without_releasing_live_remainder() {
    use iroha_data_model::sorafs::orderbook::{
        orderbook_order_escrow_id, orderbook_settlement_escrow_id,
    };
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let mut parent = row(6);
    parent.id = orderbook_order_escrow_id([6; 32]);
    parent.kind = AssetEscrowKind::Lock;
    parent.status = AssetEscrowStatus::Locked;
    install(&mut tx, &parent, Kind::OrderbookLock);
    let capture = tx
        .world
        .runtime_escrow_captures
        .get(&parent.id)
        .unwrap()
        .capture
        .clone();
    let mut child = parent.clone();
    child.id = orderbook_settlement_escrow_id([7; 32]);
    child.amount = Quantity::from(2_u32);
    child.remaining_amount = child.amount.clone();
    let prepared = prepare_child(&tx, &parent.id, &child).unwrap();
    parent.remaining_amount = Quantity::from(3_u32);
    tx.world.insert_asset_escrow_entry(&parent).unwrap();
    tx.world
        .insert_captured_asset_escrow_entry(&child, prepared)
        .unwrap();
    assert_eq!(
        tx.world
            .runtime_escrow_captures
            .get(&child.id)
            .unwrap()
            .capture,
        capture
    );
    assert!(tx.world.runtime_escrow_captures.get(&parent.id).is_some());
    assert!(
        prepare_execution(
            &tx,
            &iroha_data_model::isi::escrow::CancelAssetLock {
                escrow_id: child.id,
                expected_remaining_amount: child.remaining_amount.clone()
            }
            .into()
        )
        .is_err()
    );
    let canonical = tx.world.runtime_selection.canonical().clone();
    rebind(&mut tx, wire_id_of::<ReleaseAssetEscrow>().unwrap());
    let mismatched_context = tx.world.runtime_selection.canonical().clone();
    *tx.world.runtime_selection.get_mut() = canonical.clone();
    assert!(check_transition(&*tx.world, &canonical, &mismatched_context).is_err());
    tx.world
        .runtime_selection
        .set_executing(Some(mismatched_context));
    let mut next_child = child.clone();
    next_child.id = orderbook_settlement_escrow_id([8; 32]);
    assert!(prepare_child(&tx, &parent.id, &next_child).is_err());
    tx.world.runtime_selection.set_executing(None);
    assert!(prepare_child(&tx, &parent.id, &next_child).is_ok());
    assert!(tx.world.asset_escrows.get(&next_child.id).is_none());
    let matcher = wire_id_of::<iroha_data_model::isi::sorafs::MatchSorafsOrderbook>().unwrap();
    let definition = tx
        .world
        .runtime_selection
        .canonical()
        .bindings
        .iter()
        .find(|entry| entry.wire_id == matcher)
        .unwrap()
        .interface;
    tx.world.runtime_selection.get_mut().restrictions.push(
        iroha_data_model::runtime::definition::RestrictionEntryV1 {
            definition,
            restriction: iroha_data_model::runtime::definition::RestrictionV1::CompletionOnly(1),
        },
    );
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    assert!(
        prepare_child(&tx, &parent.id, &next_child).is_err(),
        "a new provider exposure still needs matcher admission"
    );
    assert!(tx.world.asset_escrows.get(&next_child.id).is_none());
}

#[test]
fn selected_restore_refuses_missing_or_orphaned_captures() {
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let row = row(8);
    install(&mut tx, &row, Kind::Marketplace);
    validate_persistence(&*tx.world).unwrap();
    let captured = tx.world.runtime_escrow_captures.remove(row.id).unwrap();
    assert!(validate_persistence(&*tx.world).is_err());
    tx.world.runtime_escrow_captures.insert(row.id, captured);
    tx.world.asset_escrows.remove(row.id);
    assert!(validate_persistence(&*tx.world).is_err());
}

#[test]
fn orderbook_terminal_admission_is_pinned_until_drain_but_creator_interfaces_can_close() {
    use iroha_data_model::{
        isi::sorafs::*,
        runtime::definition::{RestrictionEntryV1, RestrictionV1},
        sorafs::orderbook::orderbook_order_escrow_id,
    };
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let mut row = row(11);
    row.id = orderbook_order_escrow_id([11; 32]);
    row.kind = AssetEscrowKind::Lock;
    row.status = AssetEscrowStatus::Locked;
    install(&mut tx, &row, Kind::OrderbookLock);
    let before = tx.world.runtime_selection.canonical().clone();
    let lookup = PublishedDefinitions(&tx.world.runtime_definitions);
    let live = super::super::obligations::LiveObligations::of(&tx.world);
    let restricted = |wire: &str, implementation: bool, restriction| {
        let mut after = before.clone();
        let bound = after
            .bindings
            .iter()
            .find(|binding| binding.wire_id == wire)
            .unwrap();
        after.restrictions.push(RestrictionEntryV1 {
            definition: if implementation {
                bound.implementation
            } else {
                bound.interface
            },
            restriction,
        });
        after.restrictions.sort_by_key(|entry| entry.definition);
        after
    };
    for wire in [
        wire_id_of::<CancelSorafsOrderbookOrder>().unwrap(),
        wire_id_of::<MaintainSorafsOrderbook>().unwrap(),
        wire_id_of::<RecordSorafsOrderbookSettlementReceipt>().unwrap(),
    ] {
        for restriction in [
            RestrictionV1::AdmissionClosed(6),
            RestrictionV1::CompletionOnly(6),
        ] {
            for implementation in [false, true] {
                let after = restricted(wire, implementation, restriction);
                assert!(check_transition(&*tx.world, &before, &after).is_err());
                assert!(
                    super::super::obligations::stranded(&before, &after, &lookup, &live).is_some()
                );
            }
        }
        assert!(
            orderbook_admission_gap(&restricted(wire, false, RestrictionV1::Deprecated(6)))
                .is_none()
        );
        assert!(
            orderbook_admission_gap(&restricted(wire, false, RestrictionV1::Disabled(6))).is_none(),
            "the narrow admission gate does not replace the separate disabled-definition policy"
        );
    }
    for wire in [
        wire_id_of::<SubmitSorafsOrderbookOrder>().unwrap(),
        wire_id_of::<MatchSorafsOrderbook>().unwrap(),
    ] {
        for restriction in [
            RestrictionV1::AdmissionClosed(6),
            RestrictionV1::CompletionOnly(6),
        ] {
            let after = restricted(wire, false, restriction);
            check_transition(&*tx.world, &before, &after).unwrap();
            assert!(super::super::obligations::stranded(&before, &after, &lookup, &live).is_none());
        }
    }
    let terminal_closed = restricted(
        wire_id_of::<MaintainSorafsOrderbook>().unwrap(),
        false,
        RestrictionV1::CompletionOnly(6),
    );
    row.status = AssetEscrowStatus::Cancelled;
    row.remaining_amount = Quantity::zero();
    row.closed_at_ms = Some(1001);
    tx.world.insert_asset_escrow_entry(&row).unwrap();
    check_transition(&*tx.world, &before, &terminal_closed).unwrap();
    assert!(
        super::super::obligations::stranded(
            &before,
            &terminal_closed,
            &PublishedDefinitions(&tx.world.runtime_definitions),
            &super::super::obligations::LiveObligations::of(&tx.world)
        )
        .is_none()
    );
}

#[test]
fn conditional_identity_keeps_recipient_and_predicates_while_attestations_advance() {
    use iroha_data_model::escrow::{
        ConditionalEscrowAttestation, ConditionalEscrowCondition, ConditionalEscrowConditionState,
        ConditionalEscrowOracleCondition, ConditionalEscrowPredicate, ConditionalEscrowValue,
    };
    let state = state(true);
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let mut row = row(9);
    row.kind = AssetEscrowKind::Conditional;
    row.status = AssetEscrowStatus::Locked;
    row.buyer = Some(iroha_test_samples::BOB_ID.clone());
    row.conditions.push(ConditionalEscrowConditionState {
        condition: ConditionalEscrowCondition::Oracle(ConditionalEscrowOracleCondition {
            id: "delivery".parse().unwrap(),
            attestor: ALICE_ID.clone(),
            predicate: ConditionalEscrowPredicate::Equals(ConditionalEscrowValue::Bool(true)),
            sequence: std::num::NonZeroU32::new(1).unwrap(),
        }),
        attestation: None,
        satisfied_at_ms: None,
    });
    install(&mut tx, &row, Kind::Conditional);
    let mut substitute = row.clone();
    substitute.buyer = Some(ALICE_ID.clone());
    assert!(tx.world.insert_asset_escrow_entry(&substitute).is_err());
    let mut substitute = row.clone();
    let ConditionalEscrowCondition::Oracle(condition) = &mut substitute.conditions[0].condition
    else {
        unreachable!()
    };
    condition.predicate = ConditionalEscrowPredicate::Equals(ConditionalEscrowValue::Bool(false));
    assert!(tx.world.insert_asset_escrow_entry(&substitute).is_err());
    row.conditions[0].attestation = Some(ConditionalEscrowAttestation {
        attestor: ALICE_ID.clone(),
        value: ConditionalEscrowValue::Bool(true),
        evidence_hash: None,
        committed_at_ms: 1001,
    });
    row.conditions[0].satisfied_at_ms = Some(1001);
    tx.world.insert_asset_escrow_entry(&row).unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&row.id).is_some());
}

#[test]
fn new_native_custody_cannot_capture_a_closed_named_host_profile() {
    let state = state(true);
    crate::runtime::activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
    )
    .unwrap();
    crate::runtime::activate_additions_for_testing(
        &state.world,
        crate::runtime::catalog::additions::CONTRACT_LIFECYCLE.module,
    )
    .unwrap();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle);
    let profile = *tx
        .world
        .runtime_selection
        .get()
        .active
        .iter()
        .find(|id| {
            matches!(tx.world.runtime_definitions.get(id),
            Some(DefinitionDescriptorV1::HostProfile(profile)) if profile.abi_hash == hash)
        })
        .unwrap();
    let opening = row(71);
    for restriction in [
        iroha_data_model::runtime::definition::RestrictionV1::AdmissionClosed(5),
        iroha_data_model::runtime::definition::RestrictionV1::CompletionOnly(5),
        iroha_data_model::runtime::definition::RestrictionV1::Disabled(5),
    ] {
        tx.world.runtime_selection.get_mut().restrictions =
            vec![iroha_data_model::runtime::definition::RestrictionEntryV1 {
                definition: profile,
                restriction,
            }];
        assert!(prepare_opening(&tx, &opening, Kind::Marketplace).is_err());
        assert!(tx.world.asset_escrows.get(&opening.id).is_none());
        assert!(tx.world.runtime_escrow_captures.get(&opening.id).is_none());
    }
    tx.world.runtime_selection.get_mut().restrictions.clear();
    assert!(prepare_opening(&tx, &opening, Kind::Marketplace).is_ok());
}
