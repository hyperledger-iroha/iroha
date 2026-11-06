//! Real Time output ownership consumes autonomous attempts even on rollback.

use super::*;
use crate::smartcontracts::isi::triggers::progress;
use iroha_data_model::events::time::Schedule;
use iroha_model_base::metadata::Metadata;
use std::time::Duration;

fn autonomous(id: &str, body: Vec<InstructionBox>) -> Trigger {
    let mut metadata = Metadata::default();
    metadata.insert(progress::DECLARATION_KEY.parse().unwrap(), Json::new(1_u32));
    Trigger::new(
        id.parse().unwrap(),
        Action::new(
            body,
            Repeats::Exactly(10),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                // Authenticated genesis derives its own timestamp. A 1 ms
                // period is due in every two-ms fixture interval; the explicit
                // one-invocation phase cap below admits exactly one attempt.
                Schedule::starting_at(Duration::ZERO).with_period(Duration::from_millis(1)),
            )),
        )
        .unwrap()
        .with_metadata(metadata),
    )
}

#[test]
fn native_progress_debits_success_rejection_and_output_limit_once() {
    for lifecycle in [false, true] {
        for mode in 0..3 {
            let mut body = vec![
                SetKeyValue::account(
                    ALICE_ID.clone(),
                    "progress_effect".parse().unwrap(),
                    Json::new(true),
                )
                .into(),
            ];
            if mode == 1 {
                body.push(Unregister::trigger("absent_progress_target".parse().unwrap()).into());
            } else if mode == 2 {
                body.push(Log::new(Level::INFO, "x".repeat(16_384)).into());
            }
            let (state, source) = fixture(
                if mode == 2 { 4_096 } else { 65_536 },
                1,
                vec![autonomous("progress_test", body)],
                plain_network(),
            );
            if lifecycle {
                let catalog = crate::runtime::release_catalog().unwrap();
                let (mut setup, _recording) = output_fixture_setup(&state);
                let mut tx = setup.transaction_for_callback_testing();
                for (id, definition) in &catalog.definitions {
                    if tx.world.runtime_definitions.get(id).is_none() {
                        tx.world.runtime_definitions.insert(*id, definition.clone());
                    }
                }
                *tx.world.runtime_selection.get_mut() = catalog.lifecycle.clone();
                tx.apply();
                setup.commit_world_overlay_for_testing().unwrap();
            }
            let (mut block, _recording) = state
                .block_with_recorded_pristine_carrier_stage(
                    &source,
                    |block| {
                        crate::smartcontracts::ivm::active_host_abi_hashes(&block.world)
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    },
                    |error| error,
                )
                .expect("original carrier owns the selected lifecycle output policy");
            let parent_time = output_fixture_parent_time(&state);
            let successor_time =
                u64::try_from(source.header().creation_time().as_millis()).unwrap();
            assert_eq!(successor_time.checked_sub(parent_time), Some(2));
            block.reserve_ordinary_execution_outputs(&source).unwrap();
            block
                .produce_ordinary_execution_outputs(&source, |producer| {
                    producer.execute_network_sources(None)?;
                    producer.execute_pipeline_outputs()?;
                    producer.execute_scheduled_time_outputs()
                })
                .unwrap();
            assert_eq!(
                retained(&block).rows.len(),
                2,
                "one Network and one Time output"
            );
            let output = &retained(&block).rows[1];
            if mode == 0 {
                assert!(output.result().is_ok());
            } else if mode == 1 {
                assert!(output.result().is_err());
                assert!(!output.is_output_limit_rejection());
            } else {
                assert!(output.is_output_limit_rejection());
            }
            if lifecycle {
                use iroha_data_model::block::execution_output::{
                    TriggerLifecycleOriginV1, TriggerLifecyclePolicyV1,
                };
                let effects = output.require_trigger_lifecycle().unwrap().effects();
                assert_eq!(
                    effects
                        .iter()
                        .filter(|effect| matches!(
                            effect.origin,
                            TriggerLifecycleOriginV1::InvocationPolicy {
                                policy: TriggerLifecyclePolicyV1::ConsumeTimeProgress,
                                ..
                            }
                        ))
                        .count(),
                    1,
                    "every actual attempt retains its native debit, mode={mode}"
                );
                if mode != 0 {
                    assert_eq!(effects.len(), 1, "business rollback keeps only the debit");
                }
            }
            let action = block
                .world
                .triggers
                .time_triggers()
                .get(&"progress_test".parse().unwrap())
                .unwrap();
            assert_eq!(progress::remaining(&action.metadata), 0, "mode={mode}");
            assert_eq!(
                action.repeats,
                Repeats::Exactly(if mode == 0 { 9 } else { 10 })
            );
            assert_eq!(
                block.world.triggers.next_scheduled_progress_at_ms(3, 3),
                None
            );
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("progress_effect")
                    .is_some(),
                mode == 0
            );
        }
    }
}

#[test]
fn autonomous_callback_cannot_replenish_itself_or_register_another_allowance() {
    let child = autonomous(
        "progress_child",
        vec![Log::new(Level::INFO, "child".to_owned()).into()],
    );
    let (state, source) = fixture(
        65_536,
        1,
        vec![autonomous(
            "progress_parent",
            vec![Register::trigger(child).into()],
        )],
        plain_network(),
    );
    let (mut block, _recording) = recorded_component_block(&state, source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    block
        .produce_ordinary_execution_outputs(&source, |producer| {
            execute_network(producer)?;
            producer.execute_scheduled_time_outputs()
        })
        .unwrap();
    assert_eq!(
        retained(&block).rows.len(),
        2,
        "one Network and one Time output"
    );
    assert!(time_row(&block).result.as_ref().is_err());
    assert!(
        block
            .world
            .triggers
            .time_triggers()
            .get(&"progress_child".parse().unwrap())
            .is_none()
    );
    let parent = block
        .world
        .triggers
        .time_triggers()
        .get(&"progress_parent".parse().unwrap())
        .unwrap();
    assert_eq!(progress::remaining(&parent.metadata), 0);
}

#[test]
fn native_progress_counter_and_declaration_are_not_user_writable() {
    for key in [progress::REMAINING_KEY, progress::DECLARATION_KEY] {
        let target: TriggerId = "progress_target".parse().unwrap();
        let (state, source) = fixture(
            65_536,
            1,
            vec![autonomous(
                "progress_target",
                vec![
                    SetKeyValue::trigger(target, key.parse().unwrap(), Json::new(1_024_u32)).into(),
                ],
            )],
            plain_network(),
        );
        let (mut block, _recording) = recorded_component_block(&state, source.header());
        block.reserve_ordinary_execution_outputs(&source).unwrap();
        block
            .produce_ordinary_execution_outputs(&source, |producer| {
                execute_network(producer)?;
                producer.execute_scheduled_time_outputs()
            })
            .unwrap();
        assert_eq!(
            retained(&block).rows.len(),
            2,
            "one Network and one Time output"
        );
        assert!(time_row(&block).result.as_ref().is_err());
        let action = block
            .world
            .triggers
            .time_triggers()
            .get(&"progress_target".parse().unwrap())
            .unwrap();
        assert_eq!(progress::remaining(&action.metadata), 0);
    }
}
