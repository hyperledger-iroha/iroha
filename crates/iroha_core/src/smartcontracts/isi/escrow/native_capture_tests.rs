// Real funded native escrow execution across a compiled permission-binding replacement.

#[test]
fn native_capture_funded_escrow_completes_through_executor_after_compiled_rebinding() {
    use crate::runtime::{
        activate_additions_for_testing,
        catalog::additions::{NATIVE_ESCROW_CAPTURE, RESTRICTED_DATASPACE_READ},
    };
    use iroha_data_model::runtime::definition::{DefinitionDescriptorV1, ImplementationCodeV1};
    let seller = fixture_account("captured-seller");
    let buyer = fixture_account("captured-buyer");
    let court = fixture_account("captured-court");
    let asset = fixture_asset_definition_id();
    let state = state_with_parties(&seller, &buyer, &court, &asset, Quantity::from(100_u32));
    activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    let mut block = state.block(block_header(&state, 1000));
    let mut tx = block.transaction();
    let id = fixture_escrow_id("captured-executor");
    let executor = tx.world.executor.clone();
    executor
        .execute_instruction(
            &mut tx,
            &seller,
            &OpenAssetEscrow::new(id, asset.clone(), 40_u32).into(),
        )
        .unwrap();
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(60_u32));
    let capture = tx.world.runtime_escrow_captures.get(&id).unwrap().clone();
    let profile = tx.world.runtime_selection.canonical().services_profile;
    let catalog = crate::runtime::release_catalog().unwrap();
    let (replacement, descriptor) = catalog.definitions.iter().find(|(_, descriptor)| matches!(descriptor,
        DefinitionDescriptorV1::Implementation(implementation)
            if implementation.services_profile == profile
                && matches!(&implementation.code, ImplementationCodeV1::Native { module, .. } if module == RESTRICTED_DATASPACE_READ.module)))
        .expect("compiled permission successor for the native escrow family");
    tx.world
        .runtime_definitions
        .insert(*replacement, descriptor.clone());
    let selection = tx.world.runtime_selection.get_mut();
    selection.active.push(*replacement);
    selection.active.sort();
    selection.active.dedup();
    for binding in &mut selection.bindings {
        if RESTRICTED_DATASPACE_READ
            .rebinds
            .contains(&binding.wire_id.as_str())
        {
            binding.implementation = *replacement;
        }
    }
    let successor = tx.world.runtime_selection.canonical().clone();
    successor
        .check_closure(&crate::runtime::change_set::PublishedDefinitions(
            &tx.world.runtime_definitions,
        ))
        .unwrap();
    assert_eq!(catalog.unsupported(&successor), None);
    let cancel = CancelAssetEscrow { escrow_id: id };
    assert!(
        cancel.clone().execute(&seller, &mut tx).is_err(),
        "unscoped replacement cannot spend retained custody"
    );
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(60_u32));
    assert_eq!(tx.world.runtime_escrow_captures.get(&id), Some(&capture));
    executor
        .execute_instruction(&mut tx, &seller, &cancel.into())
        .unwrap();
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(100_u32));
    assert_eq!(
        tx.world.asset_escrows.get(&id).unwrap().status,
        AssetEscrowStatus::Cancelled
    );
    assert!(tx.world.runtime_escrow_captures.get(&id).is_none());
    assert_eq!(tx.world.runtime_selection.canonical(), &successor);
    assert_eq!(tx.world.runtime_selection.get(), &successor);
}

#[test]
fn native_capture_refused_opening_moves_no_funds_and_leaves_no_custody() {
    use crate::runtime::{
        activate_additions_for_testing, catalog::additions::NATIVE_ESCROW_CAPTURE,
    };
    use iroha_data_model::runtime::definition::{RestrictionEntryV1, RestrictionV1};
    let seller = fixture_account("capture-refusal-seller");
    let buyer = fixture_account("capture-refusal-buyer");
    let court = fixture_account("capture-refusal-court");
    let asset = fixture_asset_definition_id();
    let state = state_with_parties(&seller, &buyer, &court, &asset, Quantity::from(100_u32));
    activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    let mut block = state.block(block_header(&state, 1000));
    let mut tx = block.transaction();
    let id = fixture_escrow_id("capture-refusal");
    let release_wire = iroha_data_model::isi::wire_id_of::<ReleaseAssetEscrow>().unwrap();
    let definition = tx
        .world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|entry| entry.wire_id == release_wire)
        .unwrap()
        .interface;
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(1),
        });
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    assert!(
        OpenAssetEscrow::new(id, asset.clone(), 40_u32)
            .execute(&seller, &mut tx)
            .is_err()
    );
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(100_u32));
    assert!(tx.world.asset_escrows.get(&id).is_none());
    assert!(tx.world.runtime_escrow_captures.get(&id).is_none());
    assert!(
        tx.world
            .accounts
            .get(&escrow_custody_account_id(tx.network_id(), &id, &asset))
            .is_none()
    );
}

#[test]
fn native_capture_acceptance_cannot_borrow_a_registered_completion_exemption() {
    use crate::runtime::{
        activate_additions_for_testing,
        catalog::additions::{NATIVE_ESCROW_CAPTURE, RUNTIME_LIFECYCLE},
    };
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1,
        runtime::{
            definition::{RestrictionEntryV1, RestrictionV1},
            lifecycle::ObligationKindV1,
            obligation::ObligationId,
        },
    };
    let seller = fixture_account("capture-accept-seller");
    let buyer = fixture_account("capture-accept-buyer");
    let court = fixture_account("capture-accept-court");
    let asset = fixture_asset_definition_id();
    let state = state_with_parties(&seller, &buyer, &court, &asset, Quantity::from(100_u32));
    activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module).unwrap();
    activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    let mut block = state.block(block_header(&state, 1000));
    let mut tx = block.transaction();
    let id = fixture_escrow_id("capture-accept");
    let executor = tx.world.executor.clone();
    executor
        .execute_instruction(
            &mut tx,
            &seller,
            &OpenAssetEscrow::new(id, asset.clone(), 40_u32).into(),
        )
        .unwrap();
    let accept: iroha_data_model::isi::InstructionBox = AcceptAssetEscrow { escrow_id: id }.into();
    crate::runtime::obligations::registry::register(
        &mut tx,
        &buyer,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![accept.clone()],
            expires_ms: 61_000,
        },
    )
    .unwrap();
    let obligation_id = ObligationId::derive(&buyer).unwrap();
    let pending = tx
        .world
        .runtime_obligations
        .get(&obligation_id)
        .unwrap()
        .clone();
    let row = tx.world.asset_escrows.get(&id).unwrap().clone();
    let capture = tx.world.runtime_escrow_captures.get(&id).unwrap().clone();
    let wire = iroha_data_model::isi::instruction_wire_id(&accept).unwrap();
    let definition = tx
        .world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|entry| entry.wire_id == wire)
        .unwrap()
        .interface;
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(1),
        });
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    assert!(
        executor
            .execute_instruction(&mut tx, &buyer, &accept)
            .is_err()
    );
    assert_eq!(tx.world.asset_escrows.get(&id), Some(&row));
    assert_eq!(tx.world.runtime_escrow_captures.get(&id), Some(&capture));
    assert_eq!(
        tx.world.runtime_obligations.get(&obligation_id),
        Some(&pending)
    );
    assert!(!pending.terminal_effects[0].performed);
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(60_u32));
    assert_eq!(
        tx.world.runtime_selection.get(),
        tx.world.runtime_selection.canonical()
    );
}

#[test]
fn native_capture_opening_cannot_borrow_a_registered_completion_exemption() {
    use crate::runtime::{
        activate_additions_for_testing,
        catalog::additions::{NATIVE_ESCROW_CAPTURE, RUNTIME_LIFECYCLE},
    };
    use iroha_data_model::{
        isi::runtime_obligation::RegisterRuntimeObligationV1,
        runtime::{
            definition::{RestrictionEntryV1, RestrictionV1},
            lifecycle::ObligationKindV1,
            obligation::ObligationId,
        },
    };
    let seller = fixture_account("capture-open-seller");
    let buyer = fixture_account("capture-open-buyer");
    let court = fixture_account("capture-open-court");
    let asset = fixture_asset_definition_id();
    let state = state_with_parties(&seller, &buyer, &court, &asset, Quantity::from(100_u32));
    activate_additions_for_testing(&state.world, RUNTIME_LIFECYCLE.module).unwrap();
    activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    let mut block = state.block(block_header(&state, 1000));
    let mut tx = block.transaction();
    let id = fixture_escrow_id("capture-registered-open");
    let opening: iroha_data_model::isi::InstructionBox =
        OpenAssetEscrow::new(id, asset.clone(), 40_u32).into();
    crate::runtime::obligations::registry::register(
        &mut tx,
        &seller,
        &RegisterRuntimeObligationV1 {
            contract: None,
            kind: ObligationKindV1::Escrow,
            terminal_effects: vec![opening.clone()],
            expires_ms: 61_000,
        },
    )
    .unwrap();
    let obligation_id = ObligationId::derive(&seller).unwrap();
    let pending = tx
        .world
        .runtime_obligations
        .get(&obligation_id)
        .unwrap()
        .clone();
    let wire = iroha_data_model::isi::instruction_wire_id(&opening).unwrap();
    let definition = tx
        .world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|entry| entry.wire_id == wire)
        .unwrap()
        .interface;
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .push(RestrictionEntryV1 {
            definition,
            restriction: RestrictionV1::CompletionOnly(1),
        });
    tx.world
        .runtime_selection
        .get_mut()
        .restrictions
        .sort_by_key(|entry| entry.definition);
    let executor = tx.world.executor.clone();
    assert!(
        executor
            .execute_instruction(&mut tx, &seller, &opening)
            .is_err()
    );
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(100_u32));
    assert!(tx.world.asset_escrows.get(&id).is_none());
    assert!(tx.world.runtime_escrow_captures.get(&id).is_none());
    assert!(
        tx.world
            .accounts
            .get(&escrow_custody_account_id(tx.network_id(), &id, &asset))
            .is_none()
    );
    assert_eq!(
        tx.world.runtime_obligations.get(&obligation_id),
        Some(&pending)
    );
    assert!(!pending.terminal_effects[0].performed);
    assert_eq!(
        tx.world.runtime_selection.get(),
        tx.world.runtime_selection.canonical()
    );
}

#[test]
fn native_capture_lock_conditional_and_orderbook_funding_follow_actual_custody_lifetime() {
    use crate::runtime::{
        activate_additions_for_testing, catalog::additions::NATIVE_ESCROW_CAPTURE,
    };
    use iroha_data_model::{
        runtime::native_escrow::NativeEscrowKindV1 as Kind,
        sorafs::orderbook::{orderbook_order_escrow_id, orderbook_settlement_escrow_id},
    };
    let seller = fixture_account("capture-kinds-seller");
    let buyer = fixture_account("capture-kinds-buyer");
    let court = fixture_account("capture-kinds-court");
    let asset = fixture_asset_definition_id();
    let state = state_with_parties(&seller, &buyer, &court, &asset, Quantity::from(100_u32));
    activate_additions_for_testing(&state.world, NATIVE_ESCROW_CAPTURE.module).unwrap();
    let mut block = state.block(block_header(&state, 1000));
    let mut tx = block.transaction_for_fastpq_testing(Hash::prehashed([0xD5; 32]));
    let lock = fixture_escrow_id("capture-generic-lock");
    OpenAssetLock::new(lock, asset.clone(), buyer.clone(), 20_u32)
        .execute(&seller, &mut tx)
        .unwrap();
    assert_eq!(
        tx.world.runtime_escrow_captures.get(&lock).unwrap().kind,
        Kind::Lock
    );
    DrawdownAssetLock::new(lock, 5_u32, 20_u32)
        .execute(&buyer, &mut tx)
        .unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&lock).is_some());
    CancelAssetLock::new(lock, 15_u32)
        .execute(&seller, &mut tx)
        .unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&lock).is_none());
    let conditional = fixture_escrow_id("capture-conditional");
    OpenConditionalEscrow::new(
        conditional,
        asset.clone(),
        buyer.clone(),
        20_u32,
        conditional_terms(conditional, &buyer, &court, 10_000).conditions,
        10_000,
    )
    .execute(&seller, &mut tx)
    .unwrap();
    assert_eq!(
        tx.world
            .runtime_escrow_captures
            .get(&conditional)
            .unwrap()
            .kind,
        Kind::Conditional
    );
    AttestEscrowCondition::new(
        conditional,
        "delivered".parse().unwrap(),
        ConditionalEscrowValue::Bool(true),
        None,
    )
    .execute(&court, &mut tx)
    .unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&conditional).is_none());
    let parent = orderbook_order_escrow_id([1; 32]);
    let child = orderbook_settlement_escrow_id([2; 32]);
    open_orderbook_order_asset_lock(
        &mut tx,
        parent,
        &seller,
        asset.clone(),
        Quantity::from(20_u32),
        10_000,
    )
    .unwrap();
    let original = tx
        .world
        .runtime_escrow_captures
        .get(&parent)
        .unwrap()
        .capture
        .clone();
    partition_orderbook_asset_lock(
        &mut tx,
        &parent,
        child,
        buyer.clone(),
        court.clone(),
        Quantity::from(5_u32),
        10_000,
    )
    .unwrap();
    assert_eq!(
        tx.world
            .runtime_escrow_captures
            .get(&child)
            .unwrap()
            .capture,
        original
    );
    assert!(tx.world.runtime_escrow_captures.get(&parent).is_some());
    cancel_orderbook_order_asset_lock(
        &mut tx,
        &parent,
        &seller,
        &asset,
        &Quantity::from(20_u32),
        10_000,
    )
    .unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&parent).is_none());
    settle_orderbook_asset_lock(
        &mut tx,
        &child,
        &court,
        &seller,
        Quantity::from(4_u32),
        Quantity::from(1_u32),
    )
    .unwrap();
    assert!(tx.world.runtime_escrow_captures.get(&child).is_none());
    assert!(tx.world.runtime_escrow_captures.iter().next().is_none());
    assert_eq!(balance(&tx, &seller, &asset), Quantity::from(71_u32));
    assert_eq!(balance(&tx, &buyer, &asset), Quantity::from(29_u32));
}
