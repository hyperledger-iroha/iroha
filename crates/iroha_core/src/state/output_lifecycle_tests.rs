//! Successor runtime output coverage and predecessor replay controls.
//! Actual signed roots and scheduled callbacks run through the private producer;
//! these component fixtures do not claim governed rollout or certified restart.

use super::*;
use crate::smartcontracts::isi::triggers::specialized::TimeTriggerRetryState;
use iroha_data_model::{
    block::execution_output::{
        TriggerLifecycleChangeV1, TriggerLifecycleEffectsV1, TriggerLifecycleOriginV1,
        TriggerLifecyclePolicyV1,
    },
    events::time::{ExecutionTime, Schedule, TimeEventFilter},
    trigger::action::TimeTriggerRetryPolicy,
};

fn activate(state: &State) {
    let catalog = crate::runtime::catalog::release_catalog().unwrap();
    let (mut setup, _recording) = output_fixture_setup(state);
    let mut tx = setup.transaction_for_callback_testing();
    for (id, definition) in &catalog.definitions {
        if tx.world.runtime_definitions.get(id).is_none() {
            tx.world.runtime_definitions.insert(*id, definition.clone());
        }
    }
    *tx.world.runtime_selection.get_mut() = catalog.lifecycle.clone();
    tx.apply();
    setup.commit_world_overlay_for_testing().unwrap();
    assert_eq!(
        *state.world.runtime_selection.view().get(),
        catalog.lifecycle
    );
}

/// Derive each order from only the immutable definitions published by its predecessor.
/// Fixture installation below is separate from governed activation coverage.
fn merged_runtime(
    network: iroha_data_model::NetworkId,
    time_first: bool,
) -> (
    iroha_data_model::runtime::definition::RootSelectionV1,
    std::collections::BTreeMap<
        iroha_data_model::runtime::definition::DefinitionId,
        iroha_data_model::runtime::definition::DefinitionDescriptorV1,
    >,
) {
    use crate::runtime::ironwood::{
        activation_change_set_for_selection, service_change_set_for_selection,
    };
    let catalog = crate::runtime::release_catalog().unwrap();
    let mut selection = catalog.baseline.clone();
    let mut published: std::collections::BTreeMap<_, _> = catalog
        .baseline_definitions
        .iter()
        .map(|id| (*id, catalog.definitions[id].clone()))
        .collect();
    for (offset, is_time) in [time_first, !time_first].into_iter().enumerate() {
        let height = 10 + offset as u64;
        let change = if is_time {
            service_change_set_for_selection(
                network,
                height,
                [0x63; 32],
                &selection,
                &catalog.lifecycle,
                &published,
            )
            .unwrap()
        } else {
            activation_change_set_for_selection(network, height, [0x64; 32], &selection, &published)
                .unwrap()
        };
        let applied = change.apply(&selection, &published).unwrap();
        published.extend(applied.published);
        selection = applied.selection;
        selection.check_closure(&published).unwrap();
        assert!(catalog.unsupported(&selection).is_none());
    }
    let ironwood = catalog.ironwood.as_ref().unwrap();
    for field in [
        "triggers.data",
        "triggers.pipeline",
        "triggers.time",
        "triggers.by_call",
        "world.smart_contract_state",
    ] {
        assert!(
            selection
                .installed_storage
                .contains(&ironwood.world_storage[field]),
            "{field}"
        );
        assert!(
            !selection
                .installed_storage
                .contains(&catalog.world_storage[field]),
            "{field} predecessor retired"
        );
    }
    assert!(crate::runtime::trigger_lifecycle_enabled(&selection).unwrap());
    assert!(crate::runtime::ironwood::enabled(&selection));
    (selection, published)
}

fn activate_merged(
    state: &State,
    time_first: bool,
) -> iroha_data_model::runtime::definition::SelectionId {
    let (selection, published) = merged_runtime(state.network_id, time_first);
    let selected = selection.id().unwrap();
    let (mut setup, _recording) = output_fixture_setup(state);
    let mut tx = setup.transaction_for_callback_testing();
    for (id, definition) in published {
        if tx.world.runtime_definitions.get(&id).is_none() {
            tx.world.runtime_definitions.insert(id, definition);
        }
    }
    *tx.world.runtime_selection.get_mut() = selection;
    tx.apply();
    setup.commit_world_overlay_for_testing().unwrap();
    assert_eq!(
        state.world.runtime_selection.view().get().id().unwrap(),
        selected
    );
    selected
}

#[test]
fn lifecycle_merged_both_orders_capture_all_four_typed_trigger_stores() {
    use iroha_data_model::events::{
        EventFilterBox,
        data::prelude::AccountEventFilter,
        pipeline::{BlockEventFilter, BlockStatus},
    };
    let mut first_selection = None;
    for time_first in [true, false] {
        let mut component = state(65_536);
        let mut policy = component
            .world
            .parameters
            .view()
            .get()
            .block()
            .execution_output();
        policy.max_pipeline_triggers = 1;
        component
            .world
            .set_parameter_for_testing(Parameter::Block(BlockParameter::ExecutionOutput(policy)))
            .unwrap();
        let state = fixture_with_state(component, None, None, |_| {});
        let selected = activate_merged(&state, time_first);
        assert_eq!(*first_selection.get_or_insert(selected), selected);
        let filters: [EventFilterBox; 4] = [
            AccountEventFilter::new()
                .for_account(ALICE_ID.clone())
                .into(),
            BlockEventFilter::new()
                .for_status(BlockStatus::Approved)
                .into(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                Schedule::starting_at(Duration::from_millis(60_000))
                    .with_period(Duration::from_millis(60_000)),
            ))
            .into(),
            ExecuteTriggerEventFilter::new().into(),
        ];
        let ids: [TriggerId; 4] = [
            "merged_data",
            "merged_pipeline",
            "merged_time",
            "merged_by_call",
        ]
        .map(|id| id.parse().unwrap());
        let instructions = ids
            .iter()
            .zip(filters)
            .map(|(id, filter)| {
                Register::trigger(Trigger::new(
                    id.clone(),
                    Action::new(
                        Vec::<InstructionBox>::new(),
                        Repeats::Exactly(2),
                        ALICE_ID.clone(),
                        filter,
                    )
                    .unwrap(),
                ))
                .into()
            })
            .collect();
        let source = carrier(
            &state,
            vec![input(
                &state,
                instructions,
                FeePaymentIntent::authority(vec![], None),
                false,
            )],
        );
        let (mut block, _recording) = recorded_network_block(&state, &source);
        assert!(block.lifecycle_output_enabled().unwrap());
        assert_eq!(
            block.lifecycle_execution_selection().unwrap(),
            Some(selected)
        );
        execute_time(&mut block, &source);
        let row = &retained(&block).rows[0];
        assert!(row.result().is_ok(), "{:?}", row.result());
        let effects = complete(row);
        assert_eq!(effects.len(), 4);
        for (index, (effect, id)) in effects.iter().zip(&ids).enumerate() {
            assert_eq!(&effect.trigger_id, id);
            assert_eq!(effect.ordinal as usize, index);
            assert_eq!(
                effect.origin,
                TriggerLifecycleOriginV1::DirectNetworkInstruction {
                    item_index: index as u32
                }
            );
            let TriggerLifecycleChangeV1::Insert { after } = &effect.change else {
                panic!("typed store insertion")
            };
            assert_eq!(after.registered_at_height, Some(2));
            assert!(after.action.persistent_hash().is_some());
        }
        assert!(block.world.triggers.data_triggers().get(&ids[0]).is_some());
        assert!(
            block
                .world
                .triggers
                .pipeline_triggers()
                .get(&ids[1])
                .is_some()
        );
        assert!(block.world.triggers.time_triggers().get(&ids[2]).is_some());
        assert!(
            block
                .world
                .triggers
                .by_call_triggers()
                .get(&ids[3])
                .is_some()
        );
        assert_eq!(block.world.triggers.ids().len(), 4);
    }
}

#[test]
fn lifecycle_merged_both_orders_freeze_capacity_and_preserve_overflow_rollback() {
    for time_first in [true, false] {
        let state = fixture(4096, None);
        let selected = activate_merged(&state, time_first);
        {
            let source = log_source(&state);
            let (mut block, _recording) = recorded_network_block(&state, &source);
            let time_limit = block.time_trigger_invocation_limit().unwrap();
            assert_eq!(
                block.lifecycle_execution_selection().unwrap(),
                Some(selected)
            );
            // A later staged service profile cannot replace constructor-owned capacity.
            *block.world.runtime_selection.get_mut() = crate::runtime::release_catalog()
                .unwrap()
                .ironwood
                .as_ref()
                .unwrap()
                .selection
                .clone();
            block.capture_execution_output_capacity();
            assert!(block.lifecycle_output_enabled().unwrap());
            assert_eq!(
                block.lifecycle_execution_selection().unwrap(),
                Some(selected)
            );
            assert_eq!(block.time_trigger_invocation_limit().unwrap(), time_limit);
        }
        assert_capture_overflow(&state);
    }
}

fn complete(
    row: &ExecutionOutputV1,
) -> &[iroha_data_model::block::execution_output::TriggerLifecycleEffectV1] {
    let trace = match row {
        ExecutionOutputV1::NetworkWithLifecycle(row) => &row.trigger_lifecycle,
        ExecutionOutputV1::PipelineWithLifecycle(row) => &row.trigger_lifecycle,
        ExecutionOutputV1::TimeWithLifecycle(row) => &row.trigger_lifecycle,
        _ => panic!("successor omitted lifecycle coverage"),
    };
    let TriggerLifecycleEffectsV1::Complete { effects } = trace;
    effects
}

fn log_source(state: &State) -> SignedBlock {
    carrier(
        state,
        vec![input(
            state,
            vec![Log::new(Level::DEBUG, "lifecycle root".into()).into()],
            FeePaymentIntent::authority(vec![], None),
            false,
        )],
    )
}

#[test]
fn lifecycle_output_selection_preserves_predecessor_and_covers_empty_successor_root() {
    for successor in [false, true] {
        let state = fixture(65_536, None);
        if successor {
            activate(&state);
        }
        let source = log_source(&state);
        let original = source.encode_wire().unwrap();
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute(&mut block, &source).unwrap();
        let rows = &retained(&block).rows;
        assert_eq!(rows.len(), 1);
        assert!(rows[0].result().is_ok());
        if successor {
            assert!(matches!(
                rows[0],
                ExecutionOutputV1::NetworkWithLifecycle(_)
            ));
            assert!(complete(&rows[0]).is_empty());
        } else {
            assert!(matches!(rows[0], ExecutionOutputV1::Network(_)));
        }
        assert_eq!(source.encode_wire().unwrap(), original);
    }
}

#[test]
fn lifecycle_output_signed_registration_carries_original_instruction_position() {
    let state = fixture(65_536, None);
    activate(&state);
    let id: TriggerId = "covered_registration".parse().unwrap();
    let action = Action::new(
        Vec::<InstructionBox>::new(),
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    let source = carrier(
        &state,
        vec![input(
            &state,
            vec![
                Log::new(Level::INFO, "original index zero".into()).into(),
                Register::trigger(Trigger::new(id.clone(), action)).into(),
            ],
            FeePaymentIntent::authority(vec![], None),
            false,
        )],
    );
    let (mut block, _recording) = recorded_network_block(&state, &source);
    execute(&mut block, &source).unwrap();
    let row = &retained(&block).rows[0];
    assert!(row.result().is_ok());
    let effects = complete(row);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].ordinal, 0);
    assert_eq!(effects[0].trigger_id, id);
    assert_eq!(
        effects[0].origin,
        TriggerLifecycleOriginV1::DirectNetworkInstruction { item_index: 1 }
    );
    let TriggerLifecycleChangeV1::Insert { after } = &effects[0].change else {
        panic!("registration insert");
    };
    assert_eq!(after.registered_at_height, Some(2));
    assert_eq!(
        after.action.bytes.len(),
        iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES
    );
    assert!(after.action.persistent_hash().is_some());
    assert_eq!(
        block
            .world
            .triggers
            .inspect_by_id(&id, |action| *action.repeats()),
        Some(Repeats::Exactly(2))
    );
    assert!(block.world.triggers.ids().get(&id).is_some());
}

fn retry_fixture(
    row_bytes: u64,
    previous: u32,
    diagnostic_bytes: usize,
) -> (State, TriggerId, SignedBlock) {
    let state = fixture(row_bytes, None);
    let id: TriggerId = "successor_retry".parse().unwrap();
    {
        let (mut setup, _recording) = output_fixture_setup(&state);
        let mut tx = setup.transaction_for_callback_testing();
        let action = Action::new(
            vec![
                InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    "failed_body".parse().unwrap(),
                    Json::new(true),
                )),
                Unregister::trigger("absent".parse().unwrap()).into(),
                Log::new(Level::DEBUG, "x".repeat(diagnostic_bytes)).into(),
            ],
            Repeats::Exactly(5),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                Schedule::starting_at(Duration::from_millis(60_000))
                    .with_period(Duration::from_millis(60_000)),
            )),
        )
        .unwrap()
        .with_retry_policy(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(3).unwrap(),
            retry_after_ms: NonZeroU64::new(5).unwrap(),
        })
        .unwrap();
        Register::trigger(Trigger::new(id.clone(), action))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        tx.world.triggers.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: previous,
                next_retry_at_ms: 2,
            }),
        );
        tx.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    activate(&state);
    let source = log_source(&state);
    (state, id, source)
}

fn execute_time(block: &mut StateBlock<'_>, source: &SignedBlock) {
    block.reserve_ordinary_execution_outputs(source).unwrap();
    block
        .produce_ordinary_execution_outputs(source, |producer| {
            producer.execute_network_sources(None)?;
            producer.execute_pipeline_outputs()?;
            producer.execute_scheduled_time_outputs()
        })
        .unwrap();
}

#[test]
fn lifecycle_output_rejected_time_keeps_only_retry_or_exhaustion_policy() {
    for previous in [1, 3] {
        let (state, id, source) = retry_fixture(65_536, previous, 0);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute_time(&mut block, &source);
        assert_eq!(retained(&block).rows.len(), 2);
        let row = &retained(&block).rows[1];
        assert!(row.result().is_err());
        assert!(!row.is_output_limit_rejection());
        let effects = complete(row);
        assert_eq!(effects.len(), 1, "failed Invoke/body must roll back");
        assert_eq!(effects[0].trigger_id, id);
        let expected_policy = if previous == 3 {
            TriggerLifecyclePolicyV1::ExhaustTimeRetry
        } else {
            TriggerLifecyclePolicyV1::AdvanceTimeRetry
        };
        assert_eq!(
            effects[0].origin,
            TriggerLifecycleOriginV1::InvocationPolicy {
                callback_index: 0,
                policy: expected_policy
            }
        );
        if previous == 3 {
            assert!(matches!(
                effects[0].change,
                TriggerLifecycleChangeV1::Remove { .. }
            ));
            assert!(block.world.triggers.time_triggers().get(&id).is_none());
        } else {
            let TriggerLifecycleChangeV1::Update { before, after } = &effects[0].change else {
                panic!("retry update");
            };
            assert_eq!(before.retry_state.unwrap().attempt, previous);
            assert_eq!(after.retry_state.unwrap().attempt, previous + 1);
            assert_eq!(
                block
                    .world
                    .triggers
                    .time_triggers()
                    .get(&id)
                    .unwrap()
                    .retry_state
                    .unwrap()
                    .retries_used,
                previous + 1
            );
        }
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("failed_body")
                .is_none()
        );
    }
}

#[test]
fn lifecycle_output_reserved_policy_survives_source_work_and_diagnostic_limits() {
    for previous in [1, 3] {
        let (state, id, source) = retry_fixture(4096, previous, 8192);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute_time(&mut block, &source);
        let row = &retained(&block).rows[1];
        assert!(!row.is_output_limit_rejection());
        assert_eq!(complete(row).len(), 1);
        if previous == 3 {
            assert!(block.world.triggers.time_triggers().get(&id).is_none());
        } else {
            let action = block.world.triggers.time_triggers().get(&id).unwrap();
            assert_eq!(action.retry_state.unwrap().retries_used, previous + 1);
            assert_eq!(action.repeats, Repeats::Exactly(5));
        }
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("failed_body")
                .is_none()
        );
    }
}

#[test]
fn lifecycle_output_network_capture_limit_rejects_one_root_and_keeps_next_root_live() {
    let state = fixture(4096, None);
    activate(&state);
    assert_capture_overflow(&state);
}

fn assert_capture_overflow(state: &State) {
    let id: TriggerId = "too_large_trace".parse().unwrap();
    let action = Action::new(
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "x".repeat(8192),
        ))],
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    // Fixed-size commitments let the single large Action fit. Exceed the row
    // using many actual retained effects instead, while staying below the
    // independent instruction/source limits.
    let mut instructions = vec![Register::trigger(Trigger::new(id.clone(), action)).into()];
    for i in 0..64 {
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        instructions.push(
            Register::trigger(Trigger::new(
                format!("overflow_{i}").parse().unwrap(),
                action,
            ))
            .into(),
        );
    }
    let source = carrier(
        &state,
        vec![
            input(
                &state,
                instructions,
                FeePaymentIntent::authority(vec![], None),
                false,
            ),
            input(
                &state,
                vec![Log::new(Level::INFO, "next root".into()).into()],
                FeePaymentIntent::authority(vec![], None),
                false,
            ),
        ],
    );
    let (mut block, _recording) = recorded_network_block(&state, &source);
    execute(&mut block, &source).unwrap();
    let rows = &retained(&block).rows;
    assert_eq!(rows.len(), 2);
    assert!(rows[0].is_output_limit_rejection());
    assert!(complete(&rows[0]).is_empty());
    assert!(rows[1].result().is_ok());
    assert!(complete(&rows[1]).is_empty());
    assert!(block.world.triggers.ids().get(&id).is_none());
}

#[test]
fn lifecycle_output_pipeline_success_and_quarantine_have_complete_coverage() {
    use iroha_data_model::events::pipeline::{BlockEventFilter, BlockStatus};
    for fails in [false, true] {
        let state = fixture(65_536, None);
        let id: TriggerId = "covered_pipeline".parse().unwrap();
        {
            let mut parameters =
                crate::state::parameter_cell_block_for_testing(&state.world).unwrap();
            let mut policy = parameters.get().block().execution_output();
            policy.max_pipeline_triggers = 1;
            crate::state::set_parameter_cell_for_testing(
                &mut parameters,
                Parameter::Block(BlockParameter::ExecutionOutput(policy)),
                state.world.sccp_committee_generations.allocation_budget(),
            )
            .unwrap();
            parameters.commit();
        }
        {
            let (mut setup, _recording) = output_fixture_setup(&state);
            let mut tx = setup.transaction_for_callback_testing();
            let body: Vec<InstructionBox> = if fails {
                vec![Unregister::trigger("absent".parse().unwrap()).into()]
            } else {
                vec![Log::new(Level::DEBUG, "pipeline".into()).into()]
            };
            let action = Action::new(
                body,
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                BlockEventFilter::new().for_status(BlockStatus::Approved),
            )
            .unwrap();
            Register::trigger(Trigger::new(id.clone(), action))
                .execute(&ALICE_ID, &mut tx)
                .unwrap();
            tx.apply();
            setup.commit_world_overlay_for_testing().unwrap();
        }
        activate(&state);
        let source = log_source(&state);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute_time(&mut block, &source);
        let rows = &retained(&block).rows;
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            rows[1],
            ExecutionOutputV1::PipelineWithLifecycle(_)
        ));
        let effects = complete(&rows[1]);
        if fails {
            assert!(rows[1].result().is_err());
            assert_eq!(effects.len(), 1);
            assert!(matches!(
                effects[0].change,
                TriggerLifecycleChangeV1::Update { .. }
            ));
            assert_eq!(
                effects[0].origin,
                TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy: TriggerLifecyclePolicyV1::QuarantinePipeline
                }
            );
            let action = block.world.triggers.pipeline_triggers().get(&id).unwrap();
            assert_eq!(action.metadata.get("__enabled"), Some(&Json::from(false)));
        } else {
            assert!(rows[1].result().is_ok());
            assert!(matches!(
                effects[0].change,
                TriggerLifecycleChangeV1::Invoke {
                    callback_index: 0,
                    ..
                }
            ));
            assert!(matches!(
                effects.last().unwrap().change,
                TriggerLifecycleChangeV1::Remove { .. }
            ));
            assert!(block.world.triggers.pipeline_triggers().get(&id).is_none());
        }
    }
}

#[test]
fn lifecycle_output_shortened_failure_diagnostics_keep_retry_and_exhaustion_effects() {
    for (previous, ceiling) in [(1, 4096), (3, 4096)] {
        let (state, id, source) = retry_fixture(ceiling, previous, 8192);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute_time(&mut block, &source);
        let row = &retained(&block).rows[1];
        assert!(row.is_internal_rejection_diagnostic_omitted());
        assert!(!row.is_output_limit_rejection());
        assert_eq!(complete(row).len(), 1);
        if previous == 3 {
            assert!(matches!(
                complete(row)[0].change,
                TriggerLifecycleChangeV1::Remove { .. }
            ));
            assert!(block.world.triggers.time_triggers().get(&id).is_none());
        } else {
            assert!(matches!(
                complete(row)[0].change,
                TriggerLifecycleChangeV1::Update { .. }
            ));
            assert_eq!(
                block
                    .world
                    .triggers
                    .time_triggers()
                    .get(&id)
                    .unwrap()
                    .retry_state
                    .unwrap()
                    .retries_used,
                2
            );
        }
    }
}

#[test]
fn lifecycle_output_transfer_allocation_failures_leave_state_apply_guard_closed() {
    use super::super::super::lifecycle as trace_output;
    use iroha_allocation::{AllocationBudget, AllocationCharge};
    // Zero headroom refuses the ledger; exactly its backing refuses the effect
    // vector next. Both failures occur after journal custody was taken.
    let ledger_bytes = std::alloc::Layout::array::<AllocationCharge>(4)
        .unwrap()
        .size();
    for headroom in [0, ledger_bytes] {
        let state = fixture(65_536, None);
        let source = log_source(&state);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        let pool = AllocationBudget::new(16 * 1024 * 1024);
        let held;
        {
            let mut tx = block.transaction_for_callback_testing();
            tx.world
                .triggers
                .enable_selected_lifecycle_journal(
                    pool.clone(),
                    trace_output::limits(65_536),
                    source.header().height().get(),
                )
                .unwrap();
            let id: TriggerId = "unpublished_transfer".parse().unwrap();
            let action = Action::new(
                Vec::<InstructionBox>::new(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new(),
            )
            .unwrap();
            Register::trigger(Trigger::new(id, action))
                .execute(&ALICE_ID, &mut tx)
                .unwrap();
            assert!(tx.world.triggers.lifecycle_failure().is_none());
            held = pool
                .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes() - headroom)
                .unwrap();
            assert!(matches!(
                trace_output::take(&mut tx, ExecutionOutputV1::Network(row(0, 0))),
                Err(ExecutionAttemptError::Deferred(_))
            ));
            assert!(!tx.world.triggers.lifecycle_allows_apply());
            assert!(tx.world.triggers.lifecycle_failure().is_some());
            tx.apply();
        }
        assert!(
            block
                .world
                .triggers
                .ids()
                .get(&"unpublished_transfer".parse().unwrap())
                .is_none()
        );
        assert_eq!(pool.reserved_bytes(), held.remaining_bytes());
        drop(held);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn lifecycle_output_missing_or_full_custody_destination_refuses_and_refunds() {
    for full in [false, true] {
        // Keep setup's retired MV generations in the baseline while measuring
        // direct refunds of the discarded output graph. Other tests may advance
        // the process-wide epoch even though this fixture owns its own pool.
        let _fixture_retirement = crossbeam_epoch::pin();
        let isolated = iroha_allocation::AllocationBudget::new(1024 * 1024 * 1024);
        let state = fixture_with_state(
            state_with_pool(65_536, Some(isolated.clone())),
            None,
            None,
            |_| {},
        );
        assert!(state.ivm_execution_budget().same_pool(&isolated));
        activate(&state);
        let id: TriggerId = "refused_custody_destination".parse().unwrap();
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        let source = carrier(
            &state,
            vec![input(
                &state,
                vec![Register::trigger(Trigger::new(id.clone(), action)).into()],
                FeePaymentIntent::authority(vec![], None),
                false,
            )],
        );
        let pool = state.ivm_execution_budget();
        let before = pool.reserved_bytes();
        {
            let (mut block, _recording) = recorded_network_block(&state, &source);
            block.reserve_ordinary_execution_outputs(&source).unwrap();
            let result = block.produce_ordinary_execution_outputs(&source, |producer| {
                // Corrupt only the private destination owner. Actual Network
                // execution still builds and transfers its original charged row.
                producer.lifecycle_custody = if full {
                    Some(iroha_allocation::ChargedBuffer::new(0, &pool).unwrap())
                } else {
                    None
                };
                producer.execute_network_sources(None)?;
                panic!("a row cannot escape without its original custody destination");
            });
            assert!(result.is_err());
            assert!(matches!(
                block.execution_output_plan,
                Some(ExecutionOutputPlanState::Poisoned)
            ));
        }
        assert_eq!(
            pool.reserved_bytes(),
            before,
            "all discarded graph custody is refunded"
        );
        assert!(state.world_view().triggers().ids().get(&id).is_none());
    }
}

#[test]
fn lifecycle_output_selected_capture_admission_remains_a_local_deferral() {
    use super::super::super::lifecycle as trace_output;
    use crate::state::StateReadOnly;
    let state = fixture(65_536, None);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    let mut tx = block.transaction_for_callback_testing();
    let pool = tx.execution_budget();
    let before = pool.reserved_bytes();
    let held = pool.try_reserve_bytes(pool.limit_bytes() - before).unwrap();
    assert!(matches!(
        trace_output::arm(&mut tx, true, 65_536),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    assert!(tx.ensure_execution_ready().is_err());
    drop(held);
    assert_eq!(pool.reserved_bytes(), before);
}

#[test]
fn lifecycle_output_special_network_failure_preserves_local_refusal_priority() {
    use super::super::super::{lifecycle as trace_output, network::completed_network_result_gate};
    use iroha_data_model::transaction::error::{TransactionLimitError, TransactionRejectionReason};
    let state = fixture(65_536, None);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    let mut tx = block.transaction_for_callback_testing();
    tx.world
        .triggers
        .enable_selected_lifecycle_journal(
            iroha_allocation::AllocationBudget::new(16 * 1024 * 1024),
            trace_output::limits(65_536),
            source.header().height().get(),
        )
        .unwrap();
    let failed = Err(TransactionRejectionReason::LimitCheck(
        TransactionLimitError {
            reason: "business failure".into(),
        },
    ));
    let special = [
        (
            true,
            false,
            "SCCP forgery evidence failed at execution (R-EX): business failure",
        ),
        (
            false,
            true,
            "Keepalive failed at execution (K2): business failure",
        ),
    ];
    for (forgery, keepalive, expected) in special {
        assert!(matches!(
            completed_network_result_gate(&tx, &failed, forgery, keepalive),
            Err(ExecutionAttemptError::Rejected(reason)) if reason == expected
        ));
    }
    assert!(!completed_network_result_gate(&tx, &failed, false, false).unwrap());
    tx.world
        .triggers
        .limit_lifecycle("deterministic trace capacity");
    assert!(completed_network_result_gate(&tx, &Ok(vec![]), true, false).unwrap());
    assert!(completed_network_result_gate(&tx, &failed, false, false).unwrap());
    for (forgery, keepalive, expected) in special {
        assert!(matches!(
            completed_network_result_gate(&tx, &failed, forgery, keepalive),
            Err(ExecutionAttemptError::Rejected(reason)) if reason == expected
        ));
    }
    tx.world
        .triggers
        .refuse_lifecycle("callback capture invariant");
    assert!(
        tx.ensure_execution_ready().is_ok(),
        "the lifecycle latch is separate"
    );
    for (forgery, keepalive, _) in special {
        for result in [&failed, &Ok(vec![])] {
            assert!(matches!(
                completed_network_result_gate(&tx, result, forgery, keepalive),
                Err(ExecutionAttemptError::Deferred(reason))
                    if reason.reason() == ivm::ExecutionDeferral::LocalInvariantViolation
            ));
        }
    }
}

#[test]
fn lifecycle_output_special_network_failure_preserves_actual_capture_allocation_refusal() {
    use super::super::super::{lifecycle as trace_output, network::completed_network_result_gate};
    use crate::smartcontracts::isi::triggers::set::lifecycle_journal::LifecycleJournalFailure;
    use iroha_data_model::transaction::error::{TransactionLimitError, TransactionRejectionReason};
    let state = fixture(65_536, None);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    let mut tx = block.transaction_for_callback_testing();
    let pool = iroha_allocation::AllocationBudget::new(16 * 1024 * 1024);
    tx.world
        .triggers
        .enable_selected_lifecycle_journal(
            pool.clone(),
            trace_output::limits(65_536),
            source.header().height().get(),
        )
        .unwrap();
    let held = pool
        .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
        .unwrap();
    Register::trigger(Trigger::new(
        "allocation_refused_callback".parse().unwrap(),
        Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap(),
    ))
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    assert!(matches!(
        tx.world.triggers.lifecycle_failure(),
        Some(LifecycleJournalFailure::Allocation(_))
    ));
    assert!(
        tx.ensure_execution_ready().is_ok(),
        "capture refusal is not a body failure"
    );
    let failed = Err(TransactionRejectionReason::LimitCheck(
        TransactionLimitError {
            reason: "later callback body failure".into(),
        },
    ));
    for (forgery, keepalive) in [(true, false), (false, true)] {
        assert!(matches!(
            completed_network_result_gate(&tx, &failed, forgery, keepalive),
            Err(ExecutionAttemptError::Deferred(reason))
                if reason.reason() == ivm::ExecutionDeferral::AllocationUnavailable
        ));
    }
    drop(tx);
    assert_eq!(pool.reserved_bytes(), held.remaining_bytes());
    drop(held);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn lifecycle_output_selected_state_apply_requires_drain_even_for_empty_trace() {
    use super::super::super::lifecycle as trace_output;
    let state = fixture(65_536, None);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    {
        let mut tx = block.transaction_for_callback_testing();
        tx.world
            .triggers
            .enable_selected_lifecycle_journal(
                iroha_allocation::AllocationBudget::new(16 * 1024 * 1024),
                trace_output::limits(65_536),
                source.header().height().get(),
            )
            .unwrap();
        SetKeyValue::account(
            ALICE_ID.clone(),
            "uncovered_write".parse().unwrap(),
            Json::new(1),
        )
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert!(tx.world.triggers.lifecycle_failure().is_none());
        assert!(!tx.world.triggers.lifecycle_allows_apply());
        tx.apply();
    }
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("uncovered_write")
            .is_none()
    );
    assert!(matches!(
        block.execution_output_plan,
        Some(ExecutionOutputPlanState::Poisoned)
    ));
}

#[test]
fn lifecycle_output_producer_rejects_missing_or_unselected_coverage() {
    use super::super::super::lifecycle as trace_output;
    for successor in [false, true] {
        let state = fixture(65_536, None);
        if successor {
            activate(&state);
        }
        let source = log_source(&state);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        let result = block.produce_ordinary_execution_outputs(&source, |producer| {
            producer.execute_network_sources(None)?;
            let original = std::mem::replace(
                &mut producer.rows[0],
                ExecutionOutputV1::network_output_limit_rejection(0),
            );
            producer.rows[0] = match original {
                ExecutionOutputV1::NetworkWithLifecycle(row) => {
                    ExecutionOutputV1::Network(row.execution)
                }
                ExecutionOutputV1::Network(row) => {
                    trace_output::empty(ExecutionOutputV1::Network(row), true)
                }
                _ => unreachable!(),
            };
            finish_empty_internal(producer)
        });
        assert!(result.is_err());
        assert!(matches!(
            block.execution_output_plan,
            Some(ExecutionOutputPlanState::Poisoned)
        ));
    }
}

#[test]
fn lifecycle_output_selection_is_frozen_for_the_complete_block() {
    let catalog = crate::runtime::catalog::release_catalog().unwrap();
    for successor in [false, true] {
        let state = fixture(65_536, None);
        if successor {
            activate(&state);
        }
        let source = log_source(&state);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        assert_eq!(block.lifecycle_output_enabled().unwrap(), successor);
        for (id, descriptor) in &catalog.definitions {
            if block.world.runtime_definitions.get(id).is_none() {
                block
                    .world
                    .runtime_definitions
                    .insert(*id, descriptor.clone());
            }
        }
        *block.world.runtime_selection.get_mut() = if successor {
            catalog.baseline.clone()
        } else {
            catalog.lifecycle.clone()
        };
        block.capture_execution_output_capacity();
        assert_eq!(block.lifecycle_output_enabled().unwrap(), successor);
        execute(&mut block, &source).unwrap();
        assert_eq!(
            retained(&block).rows[0].trigger_lifecycle().is_some(),
            successor
        );
    }
}

#[test]
fn lifecycle_output_successful_time_records_invoke_then_retry_repeat_and_retirement() {
    let state = fixture(65_536, None);
    let id: TriggerId = "successful_lifecycle_time".parse().unwrap();
    {
        let (mut setup, _recording) = output_fixture_setup(&state);
        let mut tx = setup.transaction_for_callback_testing();
        let action = Action::new(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "success".into(),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(60_000),
            ))),
        )
        .unwrap()
        .with_retry_policy(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(3).unwrap(),
            retry_after_ms: NonZeroU64::new(5).unwrap(),
        })
        .unwrap();
        Register::trigger(Trigger::new(id.clone(), action))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        tx.world.triggers.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 2,
            }),
        );
        tx.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    activate(&state);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    execute_time(&mut block, &source);
    let row = &retained(&block).rows[1];
    assert!(row.result().is_ok());
    assert!(matches!(row, ExecutionOutputV1::TimeWithLifecycle(_)));
    let effects = complete(row);
    assert_eq!(effects.len(), 4);
    for (ordinal, effect) in effects.iter().enumerate() {
        assert_eq!(effect.ordinal as usize, ordinal);
        assert_eq!(effect.trigger_id, id);
    }
    assert_eq!(
        effects[0].origin,
        TriggerLifecycleOriginV1::Derived {
            callback_index: Some(0)
        }
    );
    let TriggerLifecycleChangeV1::Invoke {
        state,
        callback_index,
    } = &effects[0].change
    else {
        panic!("pre-body invoke");
    };
    assert_eq!(*callback_index, 0);
    assert_eq!(state.retry_state.unwrap().attempt, 1);
    for (effect, policy) in effects[1..].iter().zip([
        TriggerLifecyclePolicyV1::ClearTimeRetry,
        TriggerLifecyclePolicyV1::ConsumeRepeat,
        TriggerLifecyclePolicyV1::RemoveDepleted,
    ]) {
        assert_eq!(
            effect.origin,
            TriggerLifecycleOriginV1::InvocationPolicy {
                callback_index: 0,
                policy
            }
        );
    }
    let TriggerLifecycleChangeV1::Update { before, after } = &effects[1].change else {
        panic!("clear retry");
    };
    assert_eq!(before.retry_state.unwrap().attempt, 1);
    assert!(after.retry_state.is_none());
    let TriggerLifecycleChangeV1::Update { before, after } = &effects[2].change else {
        panic!("consume repeat");
    };
    assert_ne!(before.action.bytes, after.action.bytes);
    assert_ne!(before.action_hash, after.action_hash);
    assert!(matches!(
        effects[3].change,
        TriggerLifecycleChangeV1::Remove { .. }
    ));
    assert!(block.world.triggers.time_triggers().get(&id).is_none());
}

#[test]
fn lifecycle_output_large_action_retries_and_exhausts_inside_default_row_limit() {
    for previous in [1, 3] {
        let (state, id, source) = retry_fixture(1_048_576, previous, 350_000);
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute_time(&mut block, &source);
        let row = &retained(&block).rows[1];
        assert!(!row.is_output_limit_rejection());
        assert_eq!(complete(row).len(), 1);
        if previous == 3 {
            assert!(block.world.triggers.time_triggers().get(&id).is_none());
        } else {
            assert_eq!(
                block
                    .world
                    .triggers
                    .time_triggers()
                    .get(&id)
                    .unwrap()
                    .retry_state
                    .unwrap()
                    .retries_used,
                2
            );
        }
    }
}

#[test]
fn lifecycle_output_operator_unregistration_of_invalid_source_is_publishable() {
    for invalid_source in 0..3 {
        let (state, id, _) = retry_fixture(65_536, 1, 16);
        {
            let (mut setup, _recording) = output_fixture_setup(&state);
            let mut tx = setup.transaction_for_callback_testing();
            match invalid_source {
                0 | 1 => {
                    tx.world
                        .triggers
                        .inspect_by_id_mut(&id, |action| {
                            if invalid_source == 0 {
                                action.metadata_mut().remove("__registered_block_height");
                            } else {
                                action.metadata_mut().insert(
                                    "__registered_block_height".parse().unwrap(),
                                    Json::new(u64::MAX),
                                );
                            }
                        })
                        .unwrap();
                }
                2 => {
                    assert!(tx.world.triggers.set_time_trigger_retry_state(
                        &id,
                        Some(TimeTriggerRetryState {
                            retries_used: 0,
                            next_retry_at_ms: 2,
                        }),
                    ));
                }
                _ => unreachable!(),
            }
            tx.apply();
            setup.commit_world_overlay_for_testing().unwrap();
        }
        let source = carrier(
            &state,
            vec![input(
                &state,
                vec![Unregister::trigger(id.clone()).into()],
                FeePaymentIntent::authority(vec![], None),
                false,
            )],
        );
        let (mut block, _recording) = recorded_network_block(&state, &source);
        execute(&mut block, &source).unwrap();
        let row = &retained(&block).rows[0];
        assert!(row.result().is_ok());
        let TriggerLifecycleChangeV1::Remove { before } = &complete(row)[0].change else {
            panic!("repair");
        };
        assert_eq!(before.action.unavailable_reason(), Some(1));
        assert!(block.world.triggers.time_triggers().get(&id).is_none());
    }
}

#[test]
fn lifecycle_merged_both_orders_publish_typed_repairs_for_all_four_trigger_stores() {
    use iroha_data_model::{
        block::execution_output::TriggerLifecycleStateKindV1,
        events::{
            EventFilterBox,
            data::prelude::AccountEventFilter,
            pipeline::{BlockEventFilter, BlockStatus},
        },
    };
    for time_first in [true, false] {
        for malformed in [Json::from("not a height"), Json::new(u64::MAX)] {
            let mut component = state(65_536);
            let mut policy = component
                .world
                .parameters
                .view()
                .get()
                .block()
                .execution_output();
            policy.max_pipeline_triggers = 1;
            component
                .world
                .set_parameter_for_testing(Parameter::Block(BlockParameter::ExecutionOutput(
                    policy,
                )))
                .unwrap();
            let state = fixture_with_state(component, None, None, |_| {});
            let ids: [TriggerId; 4] = [
                "repair_data",
                "repair_pipeline",
                "repair_time",
                "repair_by_call",
            ]
            .map(|id| id.parse().unwrap());
            let kinds = [
                TriggerLifecycleStateKindV1::Data,
                TriggerLifecycleStateKindV1::Pipeline,
                TriggerLifecycleStateKindV1::Time,
                TriggerLifecycleStateKindV1::ByCall,
            ];
            let filters: [EventFilterBox; 4] = [
                AccountEventFilter::new()
                    .for_account(ALICE_ID.clone())
                    .into(),
                BlockEventFilter::new()
                    .for_status(BlockStatus::Approved)
                    .into(),
                TimeEventFilter::new(ExecutionTime::Schedule(
                    Schedule::starting_at(Duration::from_millis(60_000))
                        .with_period(Duration::from_millis(60_000)),
                ))
                .into(),
                ExecuteTriggerEventFilter::new().into(),
            ];
            {
                // Restore-like corruption is fixture setup; the repair below is
                // a real signed instruction executed by the ordinary producer.
                let (mut setup, _recording) = output_fixture_setup(&state);
                let mut tx = setup.transaction_for_callback_testing();
                for (id, filter) in ids.iter().zip(filters) {
                    let action = Action::new(
                        Vec::<InstructionBox>::new(),
                        Repeats::Exactly(2),
                        ALICE_ID.clone(),
                        filter,
                    )
                    .unwrap();
                    Register::trigger(Trigger::new(id.clone(), action))
                        .execute(&ALICE_ID, &mut tx)
                        .unwrap();
                    tx.world
                        .triggers
                        .inspect_by_id_mut(id, |action| {
                            action.metadata_mut().insert(
                                "__registered_block_height".parse().unwrap(),
                                malformed.clone(),
                            );
                        })
                        .unwrap();
                }
                tx.apply();
                setup.commit_world_overlay_for_testing().unwrap();
            }
            let selected = activate_merged(&state, time_first);
            let source = carrier(
                &state,
                vec![input(
                    &state,
                    ids.iter()
                        .map(|id| Unregister::trigger(id.clone()).into())
                        .collect(),
                    FeePaymentIntent::authority(vec![], None),
                    false,
                )],
            );
            let original = source.encode_wire().unwrap();
            let (mut block, _recording) = recorded_network_block(&state, &source);
            assert_eq!(
                block.lifecycle_execution_selection().unwrap(),
                Some(selected)
            );
            execute_time(&mut block, &source);
            let rows = &retained(&block).rows;
            assert_eq!(rows.len(), 1, "unregistered internal sources never execute");
            assert!(rows[0].result().is_ok(), "{:?}", rows[0].result());
            assert!(!rows[0].is_output_limit_rejection());
            let effects = complete(&rows[0]);
            assert_eq!(effects.len(), 4);
            for (index, ((effect, id), kind)) in effects.iter().zip(&ids).zip(kinds).enumerate() {
                assert_eq!(&effect.trigger_id, id);
                assert_eq!(effect.ordinal as usize, index);
                assert_eq!(
                    effect.origin,
                    TriggerLifecycleOriginV1::DirectNetworkInstruction {
                        item_index: index as u32
                    }
                );
                let TriggerLifecycleChangeV1::Remove { before } = &effect.change else {
                    panic!("typed unavailable-source repair")
                };
                assert_eq!(before.kind, kind);
                assert_eq!(before.action.unavailable_reason(), Some(1));
                assert_eq!(before.registered_at_height, None);
                assert_eq!(before.action_hash, None);
                assert_eq!(before.retry_state, None);
                assert_eq!(before.action.persistent_hash(), None);
            }
            assert!(block.world.triggers.ids().is_empty());
            assert!(block.world.triggers.data_triggers().is_empty());
            assert!(block.world.triggers.pipeline_triggers().is_empty());
            assert!(block.world.triggers.time_triggers().is_empty());
            assert!(block.world.triggers.by_call_triggers().is_empty());
            assert_eq!(source.encode_wire().unwrap(), original);
        }
    }
}

#[test]
fn lifecycle_output_unavailable_successful_time_source_rejects_root_without_stalling_block() {
    let state = fixture(65_536, None);
    let id: TriggerId = "invalid_retry_success".parse().unwrap();
    {
        let (mut setup, _recording) = output_fixture_setup(&state);
        let mut tx = setup.transaction_for_callback_testing();
        let action = Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                ALICE_ID.clone(),
                "must_rollback".parse().unwrap(),
                Json::new(true),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                Schedule::starting_at(Duration::from_millis(60_000))
                    .with_period(Duration::from_millis(60_000)),
            )),
        )
        .unwrap()
        .with_retry_policy(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(3).unwrap(),
            retry_after_ms: NonZeroU64::new(5).unwrap(),
        })
        .unwrap();
        Register::trigger(Trigger::new(id.clone(), action))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        assert!(tx.world.triggers.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: 0,
                next_retry_at_ms: 2
            })
        ));
        tx.apply();
        setup.commit_world_overlay_for_testing().unwrap();
    }
    activate(&state);
    let source = log_source(&state);
    let (mut block, _recording) = recorded_network_block(&state, &source);
    execute_time(&mut block, &source);
    let row = &retained(&block).rows[1];
    assert!(row.is_output_limit_rejection());
    assert!(complete(row).is_empty());
    assert_eq!(
        block
            .world
            .triggers
            .time_triggers()
            .get(&id)
            .unwrap()
            .repeats,
        Repeats::Exactly(1)
    );
    assert!(
        block
            .world
            .accounts
            .get(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("must_rollback")
            .is_none()
    );
}

#[test]
fn lifecycle_merged_both_orders_apply_compiled_time_callback_to_typed_contract_state() {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};
    use iroha_data_model::{Registrable, transaction::executable::ContractInvocation};
    use iroha_model_base::state_path::StatePath;
    use ivm_abi::state_value::{
        StateValueAtomV1, StateValueRecordV1, state_value_schema_for_embedded_type_v1,
        state_value_schema_hash_v1,
    };
    let encoded_int = |value: i128| {
        let schema = state_value_schema_for_embedded_type_v1(&ivm::EmbeddedStateType::Int).unwrap();
        let schema_bytes = norito::encode_canonical(&schema).unwrap();
        let envelope =
            ivm_abi::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(value))
                .unwrap();
        norito::encode_canonical(&StateValueRecordV1 {
            schema_hash: state_value_schema_hash_v1(&schema_bytes),
            atoms: vec![StateValueAtomV1::Pointer(envelope)],
        })
        .unwrap()
    };
    let (program, manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            r#"
seiyaku MergedLifecycleState {
    permission Run;
    state StateMap<int, int> Applied;
    kotoage fn callback() authorize(Run) { Applied[7] = 9; }
}
"#,
        )
        .expect("compile the actual merged callback");
    let code_hash = ivm::contract_code_hash(&program);
    for time_first in [true, false] {
        let world = World::with([], [Account::new(ALICE_ID.clone()).build(&ALICE_ID)], []);
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(world, 1_000)).unwrap();
        let (selection, _) = merged_runtime(chain.network_id(), time_first);
        chain.select_runtime_for_testing(1_001, &selection);
        let address = chain.install_lifecycle_contract_for_testing(
            1_001,
            &ALICE_KEYPAIR,
            197,
            program.clone(),
            manifest.clone(),
        );
        chain.commit_at(1_001, vec![]);
        let id: TriggerId = "merged_contract_callback".parse().unwrap();
        let action = Action::new(
            Executable::Batch(
                vec![ExecutableBatchItem::ContractCall(ContractInvocation {
                    contract_address: address.clone(),
                    expected_code_hash: code_hash,
                    entrypoint: "callback".into(),
                    arguments: None,
                })]
                .into(),
            ),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(1_005),
            ))),
        )
        .unwrap();
        let original = chain.sign(
            &ALICE_KEYPAIR,
            [Register::trigger(Trigger::new(id.clone(), action)).into()],
            1_001,
        );
        assert_eq!(chain.commit_at(1_002, vec![original]), vec![true]);
        chain.commit_at(1_006, vec![]);
        let committed = chain.committed(4);
        let callbacks: Vec<_> = committed
            .block()
            .execution_outputs()
            .iter()
            .filter(|row| matches!(row, ExecutionOutputV1::TimeWithLifecycle(_)))
            .collect();
        assert_eq!(callbacks.len(), 1);
        let row = callbacks[0];
        assert!(row.result().is_ok(), "{:?}", row.result());
        let effects = complete(row);
        assert_eq!(effects.len(), 3);
        assert!(effects.iter().all(|effect| effect.trigger_id == id));
        assert!(matches!(
            effects[0].change,
            TriggerLifecycleChangeV1::Invoke { .. }
        ));
        assert!(matches!(
            effects[1].change,
            TriggerLifecycleChangeV1::Update { .. }
        ));
        assert!(matches!(
            effects[2].change,
            TriggerLifecycleChangeV1::Remove { .. }
        ));
        let logical =
            ivm::host::canonical_state_map_path(&"Applied".parse().unwrap(), &encoded_int(7))
                .unwrap();
        let prefix = format!(
            "sc/{}/",
            hex::encode(Hash::new(address.to_string().as_bytes()).as_ref())
        );
        let path: StatePath = format!("{prefix}{logical}").parse().unwrap();
        let view = chain.state().view();
        assert!(view.world().triggers().time_triggers().get(&id).is_none());
        let stored = view.world().smart_contract_state();
        let actual: std::collections::BTreeMap<_, _> = stored
            .iter()
            .filter(|(key, _)| key.as_ref().starts_with(&prefix))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(
            actual,
            std::collections::BTreeMap::from([(path, encoded_int(9))])
        );
        assert_eq!(
            view.world().runtime_selection().id().unwrap(),
            selection.id().unwrap()
        );
    }
}
