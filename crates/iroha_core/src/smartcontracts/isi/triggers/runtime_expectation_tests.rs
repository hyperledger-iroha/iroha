// Runtime consent regressions, included in the trigger execution test module.

fn replace_trigger_kernel(world: &mut crate::state::WorldTransaction<'_, '_>) {
    use iroha_data_model::runtime::definition::DefinitionDescriptorV1;
    let current = world.runtime_selection.get().kernel;
    let mut descriptor = world.runtime_definitions.get(&current).unwrap().clone();
    let DefinitionDescriptorV1::Kernel(profile) = &mut descriptor else {
        panic!("the kernel has its declared kind")
    };
    profile.reference_closure[0] ^= 1;
    let replacement = descriptor.id().unwrap();
    world.runtime_definitions.insert(replacement, descriptor);
    world.runtime_selection.get_mut().kernel = replacement;
    world
        .runtime_selection
        .get()
        .check_closure(&crate::runtime::change_set::PublishedDefinitions(
            &world.runtime_definitions,
        ))
        .expect("the changed selection is closed");
}

#[test]
fn trigger_runtime_capture_is_committed_and_cannot_be_rewritten() {
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let block = new_dummy_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut tx = state_block.transaction();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let id: TriggerId = "runtime_capture".parse().unwrap();
    let trigger = by_call_trigger(id.clone(), ALICE_ID.clone());
    let expected = runtime_expectation::capture_trigger_runtime(
        trigger.action().executable(),
        &tx.world,
        tx._curr_block.height().get(),
    )
    .unwrap();
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let stored = tx
        .world
        .triggers
        .inspect_by_id(&id, |action| {
            action
                .metadata()
                .get(runtime_expectation::metadata_key())
                .cloned()
        })
        .flatten()
        .unwrap();
    assert_eq!(
        stored.try_into_any_norito::<iroha_crypto::Hash>().unwrap(),
        expected
    );
    let encoded = norito::json::to_json(&stored).unwrap();
    let decoded: Json = norito::json::from_str(&encoded).unwrap();
    assert_eq!(decoded, stored, "snapshot JSON retains the exact capture");
    let key = runtime_expectation::metadata_key().clone();
    assert!(
        SetKeyValue::trigger(id.clone(), key.clone(), Json::new(false))
            .execute(&ALICE_ID, &mut tx)
            .is_err()
    );
    assert!(
        RemoveKeyValue::trigger(id.clone(), key)
            .execute(&ALICE_ID, &mut tx)
            .is_err()
    );
    assert_eq!(
        tx.world
            .triggers
            .inspect_by_id(&id, |action| {
                action
                    .metadata()
                    .get(runtime_expectation::metadata_key())
                    .cloned()
            })
            .flatten(),
        Some(stored)
    );

    let mut forged = by_call_trigger("forged_runtime_capture".parse().unwrap(), ALICE_ID.clone());
    let mut metadata = Metadata::default();
    metadata.insert(
        runtime_expectation::metadata_key().clone(),
        Json::new(expected),
    );
    forged = Trigger::new(
        forged.id().clone(),
        forged.action().clone().with_metadata(metadata),
    );
    assert!(
        Register::trigger(forged)
            .execute(&ALICE_ID, &mut tx)
            .is_err()
    );
}

#[test]
fn trigger_runtime_change_rejects_a_call_without_consuming_its_repeat() {
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let block = new_dummy_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut tx = state_block.transaction_for_callback_testing();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let id: TriggerId = "runtime_changed_call".parse().unwrap();
    let trigger = Trigger::new(
        id.clone(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(id.clone()),
        )
        .unwrap(),
    );
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    replace_trigger_kernel(&mut tx.world);
    let error = ExecuteTrigger::new(id.clone())
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err();
    assert!(error.to_string().contains("expectation"), "{error:?}");
    assert_eq!(
        tx.world
            .triggers
            .inspect_by_id(&id, |action| *action.repeats()),
        Some(Repeats::Exactly(1))
    );
}

#[test]
fn trigger_runtime_change_suspends_data_callbacks_and_consumes_only_the_event() {
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let block = new_dummy_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut tx = state_block.transaction_for_callback_testing();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let id: TriggerId = "runtime_changed_data".parse().unwrap();
    Register::trigger(data_trigger(
        id.name().as_ref(),
        ALICE_ID.clone(),
        AccountEventFilter::new().for_account(ALICE_ID.clone()),
    ))
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    tx.world.internal_event_buf.clear();
    let original = tx.world.runtime_selection.get().clone();
    replace_trigger_kernel(&mut tx.world);
    SetKeyValue::account(
        ALICE_ID.clone(),
        "runtime_event".parse().unwrap(),
        Json::new(1_u64),
    )
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    assert!(!tx.world.internal_event_buf.is_empty());
    assert!(tx.execute_data_triggers_dfs(&ALICE_ID).unwrap().is_empty());
    assert!(tx.world.internal_event_buf.is_empty());
    assert_eq!(
        tx.world
            .triggers
            .inspect_by_id(&id, |action| *action.repeats()),
        Some(Repeats::Indefinitely)
    );
    *tx.world.runtime_selection.get_mut() = original;
    assert!(
        tx.execute_data_triggers_dfs(&ALICE_ID).unwrap().is_empty(),
        "restoring semantics does not replay the skipped event"
    );
}

#[test]
fn trigger_runtime_capture_refuses_missing_malformed_and_restricted_contexts() {
    use iroha_data_model::{
        isi::Log,
        level::Level,
        runtime::definition::{RestrictionEntryV1, RestrictionV1},
    };
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let block = new_dummy_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut tx = state_block.transaction();
    let instructions: Vec<InstructionBox> = vec![Log::new(Level::INFO, "captured".into()).into()];
    let executable = Executable::from(instructions.clone());
    let loaded = set::ExecutableRef::Instructions(instructions.into());
    let height = tx._curr_block.height().get();
    let capture =
        runtime_expectation::capture_trigger_runtime(&executable, &tx.world, height).unwrap();
    let mut metadata = Metadata::default();
    assert!(check_trigger_runtime(&metadata, &loaded, &tx.world, height).is_err());
    metadata.insert(
        runtime_expectation::metadata_key().clone(),
        Json::new(false),
    );
    assert!(check_trigger_runtime(&metadata, &loaded, &tx.world, height).is_err());
    metadata.insert(
        runtime_expectation::metadata_key().clone(),
        Json::new(capture),
    );
    check_trigger_runtime(&metadata, &loaded, &tx.world, height).unwrap();
    let definition = tx
        .world
        .runtime_selection
        .get()
        .bindings
        .iter()
        .find(|binding| binding.wire_id == iroha_data_model::isi::wire_id_of::<Log>().unwrap())
        .unwrap()
        .interface;
    for restriction in [
        RestrictionV1::AdmissionClosed(height),
        RestrictionV1::CompletionOnly(height),
        RestrictionV1::Disabled(height),
    ] {
        tx.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
            definition,
            restriction,
        }];
        assert!(matches!(
            check_trigger_runtime(&metadata, &loaded, &tx.world, height),
            Err(InstructionExecutionError::Runtime(
                iroha_data_model::runtime::lifecycle::RuntimeRejectionV1::DefinitionRestricted { .. }
            ))
        ));
        assert!(
            runtime_expectation::capture_trigger_runtime(&executable, &tx.world, height).is_err(),
            "registration cannot create a suspended callback under an existing restriction"
        );
    }
}

#[test]
fn trigger_runtime_local_decoder_refusal_is_deferred_not_suspended() {
    let state = State::new(
        component_trigger_world(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let block = new_dummy_block();
    let mut state_block = state.block(block.as_ref().header());
    let tx = state_block.transaction();
    let executable = Executable::from(Vec::<InstructionBox>::new());
    let loaded = set::ExecutableRef::Instructions(Vec::<InstructionBox>::new().into());
    let height = tx._curr_block.height().get();
    let capture =
        runtime_expectation::capture_trigger_runtime(&executable, &tx.world, height).unwrap();
    let mut metadata = Metadata::default();
    metadata.insert(
        runtime_expectation::metadata_key().clone(),
        Json::new(capture),
    );
    check_trigger_runtime(&metadata, &loaded, &tx.world, height).unwrap();
    let error = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || check_trigger_runtime(&metadata, &loaded, &tx.world, height),
    )
    .unwrap_err();
    assert!(
        !matches!(error, InstructionExecutionError::Runtime(_)),
        "{error:?}"
    );
    assert!(tx.is_execution_deferred());
    assert!(
        tx.borrow_execution_deferral()
            .unwrap()
            .decode_refusal()
            .is_some()
    );
}
