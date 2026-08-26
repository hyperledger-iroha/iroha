#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn time_trigger_precommit_executes_and_emits_time_event() -> Result<()> {
    use iroha_data_model::events::time::{ExecutionTime, TimeEventFilter};
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    // Prepare world and a simple pre-commit time trigger that sets account metadata
    let block = new_dummy_block_with_payload(|h| {
        h.set_height(NonZeroU64::new(1).unwrap());
    });
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    Register::domain(Domain::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
    ))
    .execute(&ALICE_ID, &mut stx)
    .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let tkey: Name = "tick".parse().unwrap();
    let trigger_id: TriggerId = "tick_once".parse().unwrap();
    let t = Trigger::new(
        trigger_id.clone(),
        Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                ALICE_ID.clone(),
                tkey.clone(),
                Json::from(norito::json!(1)),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    Register::trigger(t).execute(&ALICE_ID, &mut stx).unwrap();
    stx.apply();
    let _ = state_block.apply_without_execution(&block, Vec::new());
    state_block.commit().unwrap();
    // Apply a new block: time trigger should fire during apply
    let block2 = new_dummy_block_with_payload(|h| {
        h.set_height(NonZeroU64::new(2).unwrap());
        h.creation_time_ms = 1;
    });
    let mut state_block2 = state.block(block2.as_ref().header());
    let mut events = state_block2.apply(&block2, Vec::new());
    // Exactly one Time event and a single TriggerCompleted notification
    let time_count = events
        .iter()
        .filter(|e| matches!(e, EventBox::Time(_)))
        .count();
    assert_eq!(time_count, 1, "expected exactly one Time event");
    let completed: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let EventBox::TriggerCompleted(ev) = e {
                Some(ev)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        completed.len(),
        1,
        "expected exactly one TriggerCompleted event for the time trigger"
    );
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    let completion = completed[0];
    assert_eq!(completion.trigger_id(), &trigger_id);
    assert!(matches!(
        completion.outcome(),
        TriggerCompletedOutcome::Success
    ));
    // Data event: account metadata inserted for ALICE (tick=1)
    let meta_insert_count = events
        .iter()
        .filter(|e| {
            if let EventBox::Data(ev) = e
                && let iroha_data_model::events::data::prelude::DataEvent::Account(
                    iroha_data_model::events::data::prelude::AccountEvent::MetadataInserted(mc),
                ) = ev.as_ref()
            {
                return *mc.target() == *ALICE_ID
                    && mc.key() == &tkey
                    && mc.value() == &Json::from(norito::json!(1));
            }
            false
        })
        .count();
    assert_eq!(
        meta_insert_count, 1,
        "expected exactly one MetadataInserted event for tick"
    );
    // Negative cases: no asset add/remove events should be present
    assert!(events.iter().all(|e| {
        if let EventBox::Data(ev) = e
            && let iroha_data_model::events::data::prelude::DataEvent::Domain(
                iroha_data_model::events::data::prelude::DomainEvent::Asset(
                    iroha_data_model::events::data::prelude::ScopedAsset {
                        event: iroha_data_model::events::data::prelude::AssetEvent::Added(_),
                        ..
                    },
                ),
            ) = ev.as_ref()
        {
            return false;
        }
        true
    }));
    assert!(events.iter().all(|e| {
        if let EventBox::Data(ev) = e
            && let iroha_data_model::events::data::prelude::DataEvent::Domain(
                iroha_data_model::events::data::prelude::DomainEvent::Asset(
                    iroha_data_model::events::data::prelude::ScopedAsset {
                        event: iroha_data_model::events::data::prelude::AssetEvent::Removed(_),
                        ..
                    },
                ),
            ) = ev.as_ref()
        {
            return false;
        }
        true
    }));
    // Negative: no metadata removal for the same key
    assert!(events.iter().all(|e| {
        if let EventBox::Data(ev) = e
            && let iroha_data_model::events::data::prelude::DataEvent::Account(
                iroha_data_model::events::data::prelude::AccountEvent::MetadataRemoved(mc),
            ) = ev.as_ref()
        {
            return !(*mc.target() == *ALICE_ID && mc.key() == &tkey);
        }
        true
    }));
    // Drop the mutable block scope before we borrow a view; otherwise
    // the view lock blocks waiting for this block to finish applying.
    state_block2.commit().unwrap();
    // And the effect should be visible
    let tick_val = state
        .view()
        .world
        .map_account(&ALICE_ID, |a| a.value().metadata().get(&tkey).cloned())
        .unwrap();
    assert_eq!(tick_val, Some(Json::from(norito::json!(1))));
    // Drain any remaining events to keep tests isolated
    events.clear();
    Ok(())
}
fn persist_committed_test_block(kura: &Kura, block: &CommittedBlock) {
    let block_arc = Arc::new(block.clone().into());
    kura.store_block(block_arc)
        .expect("store committed block in kura");
}
#[test]
fn scheduled_time_trigger_retry_succeeds_once_and_consumes_repeats_on_success() {
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    use iroha_data_model::trigger::action::TimeTriggerRetryPolicy;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let trigger_id: TriggerId = "time_retry_once".parse().unwrap();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "retrycoin".parse().unwrap(),
    );
    let alice_asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let retry_policy = TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(1).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    };
    let world = World::with(
        [Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = State::new(world, kura.clone(), query_handle);
    let block1 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(1).unwrap());
        header.creation_time_ms = 0;
    });
    let mut state_block1 = state.block(block1.as_ref().header());
    {
        let mut stx = state_block1.transaction();
        let action = Action::new(
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                alice_asset_id.clone(),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(1),
            ))),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_retry_policy(retry_policy)
        .expect("scheduled trigger fixture accepts its retry policy");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    let _ = state_block1.apply_without_execution(&block1, Vec::new());
    state_block1.commit().unwrap();
    persist_committed_test_block(&kura, &block1);
    let block2 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(2).unwrap());
        header.creation_time_ms = 6;
    });
    {
        let view = state.view();
        assert!(
            view.time_triggers_due_for_block(&block2.as_ref().header()),
            "scheduled retry trigger should be due for block2"
        );
    }
    let mut state_block2 = state.block(block2.as_ref().header());
    let events2 = state_block2.apply(&block2, Vec::new());
    let completions2: Vec<_> = events2
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions2.len(),
        1,
        "expected one failed completion, events: {events2:#?}"
    );
    assert!(matches!(
        completions2[0].outcome(),
        TriggerCompletedOutcome::Failure(_)
    ));
    state_block2.commit().unwrap();
    persist_committed_test_block(&kura, &block2);
    {
        let view = state.view();
        let action = view
            .world
            .triggers()
            .time_triggers()
            .get(&trigger_id)
            .expect("trigger should remain registered after first failure");
        assert_eq!(action.repeats, Repeats::Exactly(1));
        assert_eq!(
            action.retry_state,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 11,
            })
        );
    }
    let block3 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(3).unwrap());
        header.creation_time_ms = 8;
    });
    let mut state_block3 = state.block(block3.as_ref().header());
    {
        let mut stx = state_block3.transaction();
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "retrycoin",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").unwrap()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.apply();
    }
    let _ = state_block3.apply_without_execution(&block3, Vec::new());
    state_block3.commit().unwrap();
    persist_committed_test_block(&kura, &block3);
    let block4 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(4).unwrap());
        header.creation_time_ms = 12;
    });
    let mut state_block4 = state.block(block4.as_ref().header());
    let events4 = state_block4.apply(&block4, Vec::new());
    let completions4: Vec<_> = events4
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(completions4.len(), 1, "expected one retry completion");
    assert!(matches!(
        completions4[0].outcome(),
        TriggerCompletedOutcome::Success
    ));
    state_block4.commit().unwrap();
    persist_committed_test_block(&kura, &block4);
    let view = state.view();
    let alice_asset = view
        .world
        .asset(&alice_asset_id)
        .expect("retry should mint the asset after the definition is registered");
    assert_eq!(&**alice_asset.value(), &Quantity::from(1_u32));
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_none(),
        "one-shot trigger should be removed after successful retry"
    );
}
#[test]
fn scheduled_time_trigger_retry_budget_exhaustion_unregisters_trigger() {
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    use iroha_data_model::trigger::action::TimeTriggerRetryPolicy;
    fn result_bearing_retry_block(
        state: &State,
        update_header: impl FnOnce(&mut BlockHeader),
    ) -> CommittedBlock {
        let signer = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
        let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
            signer.public_key().clone(),
        )]);
        let valid = ValidBlock::new_dummy_and_modify_header(signer.private_key(), update_header);
        let mut signed: SignedBlock = valid.into();
        let snapshot = state.block(signed.header()).axt_policy_snapshot();
        signed
            .set_transaction_results_with_transcripts(
                Vec::new(),
                &[],
                Vec::new(),
                BTreeMap::new(),
                Vec::new(),
                snapshot,
            )
            .expect("attach the required retry-fixture AXT policy snapshot");
        let signature =
            iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), signed.header().hash())
                .expect("sign the result-bearing retry fixture block");
        signed
            .replace_signatures(
                [iroha_data_model::block::BlockSignature::new(0, signature)]
                    .into_iter()
                    .collect(),
            )
            .expect("replace the retry fixture signature after attaching results");
        ValidBlock::new_unverified_for_tests(signed)
            .commit(&topology)
            .unpack(|_| {})
            .expect("commit the result-bearing retry fixture block")
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let trigger_id: TriggerId = "time_retry_exhausted".parse().unwrap();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "exhaustcoin".parse().unwrap(),
    );
    let alice_asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let retry_policy = TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(1).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    };
    let world = World::with(
        [Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = State::new(world, kura.clone(), query_handle);
    let block1 = result_bearing_retry_block(&state, |header| {
        header.set_height(NonZeroU64::new(1).unwrap());
        header.creation_time_ms = 0;
    });
    let mut state_block1 = state.block(block1.as_ref().header());
    {
        let mut stx = state_block1.transaction();
        let action = Action::new(
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                alice_asset_id.clone(),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(1),
            ))),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_retry_policy(retry_policy)
        .expect("scheduled trigger fixture accepts its retry policy");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    let _ = state_block1.apply_without_execution(&block1, Vec::new());
    state_block1.commit().unwrap();
    persist_committed_test_block(&kura, &block1);
    let block2 = result_bearing_retry_block(&state, |header| {
        header.set_height(NonZeroU64::new(2).unwrap());
        header.creation_time_ms = 6;
    });
    {
        let view = state.view();
        assert!(
            view.time_triggers_due_for_block(&block2.as_ref().header()),
            "scheduled retry trigger should be due for block2"
        );
    }
    let mut state_block2 = state.block(block2.as_ref().header());
    let events2 = state_block2.apply(&block2, Vec::new());
    let completions2: Vec<_> = events2
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions2.len(),
        1,
        "expected first failure event, events: {events2:#?}"
    );
    assert!(matches!(
        completions2[0].outcome(),
        TriggerCompletedOutcome::Failure(_)
    ));
    state_block2.commit().unwrap();
    persist_committed_test_block(&kura, &block2);
    {
        let view = state.view();
        let action = view
            .world
            .triggers()
            .time_triggers()
            .get(&trigger_id)
            .expect("trigger should still exist while retry budget remains");
        assert_eq!(action.repeats, Repeats::Exactly(1));
        assert_eq!(
            action.retry_state,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 11,
            })
        );
    }
    let block3 = result_bearing_retry_block(&state, |header| {
        header.set_height(NonZeroU64::new(3).unwrap());
        header.creation_time_ms = 12;
    });
    let mut state_block3 = state.block(block3.as_ref().header());
    let events3 = state_block3.apply(&block3, Vec::new());
    let completions3: Vec<_> = events3
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(completions3.len(), 1, "expected exhausted retry failure");
    assert!(matches!(
        completions3[0].outcome(),
        TriggerCompletedOutcome::Failure(_)
    ));
    state_block3.commit().unwrap();
    persist_committed_test_block(&kura, &block3);
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_none(),
        "trigger should be unregistered after retry budget exhaustion"
    );
    assert!(
        view.world.asset(&alice_asset_id).is_err(),
        "failed trigger must not mint any asset"
    );
}
#[test]
fn periodic_time_trigger_drops_missed_ticks_while_retry_pending() {
    use iroha_data_model::events::trigger_completed::TriggerCompletedOutcome;
    use iroha_data_model::trigger::action::TimeTriggerRetryPolicy;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let trigger_id: TriggerId = "time_retry_periodic".parse().unwrap();
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "periodiccoin".parse().unwrap(),
    );
    let alice_asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
    let retry_policy = TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(1).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    };
    let world = World::with(
        [Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&ALICE_ID)],
        [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
        [],
    );
    let state = State::new(world, kura.clone(), query_handle);
    {
        let mut parameters = state.world.parameters.block();
        parameters.sumeragi.block_cadence_ms = nonzero!(1_u64);
        parameters.commit();
    }
    let block1 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(1).unwrap());
        header.creation_time_ms = 0;
    });
    let mut state_block1 = state.block(block1.as_ref().header());
    {
        let mut stx = state_block1.transaction();
        let action = Action::new(
            vec![InstructionBox::from(Mint::asset_quantity(
                1_u32,
                alice_asset_id.clone(),
            ))],
            Repeats::Exactly(3),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::Schedule(
                Schedule::starting_at(Duration::from_millis(1))
                    .with_period(Duration::from_millis(4)),
            )),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_retry_policy(retry_policy)
        .expect("scheduled trigger fixture accepts its retry policy");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    let _ = state_block1.apply_without_execution(&block1, Vec::new());
    state_block1.commit().unwrap();
    persist_committed_test_block(&kura, &block1);
    let block2 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(2).unwrap());
        header.creation_time_ms = 2;
    });
    {
        let view = state.view();
        assert!(
            view.time_triggers_due_for_block(&block2.as_ref().header()),
            "periodic retry trigger should be due for block2"
        );
    }
    let mut state_block2 = state.block(block2.as_ref().header());
    let events2 = state_block2.apply(&block2, Vec::new());
    let completions2: Vec<_> = events2
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions2.len(),
        1,
        "expected initial failure, events: {events2:#?}"
    );
    assert!(matches!(
        completions2[0].outcome(),
        TriggerCompletedOutcome::Failure(_)
    ));
    state_block2.commit().unwrap();
    persist_committed_test_block(&kura, &block2);
    {
        let view = state.view();
        let action = view
            .world
            .triggers()
            .time_triggers()
            .get(&trigger_id)
            .expect("periodic trigger should stay registered after first failure");
        assert_eq!(action.repeats, Repeats::Exactly(3));
        assert_eq!(
            action.retry_state,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 7,
            })
        );
    }
    let block3 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(3).unwrap());
        header.creation_time_ms = 6;
    });
    let mut state_block3 = state.block(block3.as_ref().header());
    {
        let mut stx = state_block3.transaction();
        Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "periodiccoin",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").unwrap()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        stx.apply();
    }
    let events3 = state_block3.apply(&block3, Vec::new());
    let completions3: Vec<_> = events3
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert!(
        completions3.is_empty(),
        "scheduled ticks must be suppressed while retry is pending"
    );
    state_block3.commit().unwrap();
    persist_committed_test_block(&kura, &block3);
    {
        let view = state.view();
        let action = view
            .world
            .triggers()
            .time_triggers()
            .get(&trigger_id)
            .expect("trigger should remain pending after suppressed block");
        assert_eq!(action.repeats, Repeats::Exactly(3));
        assert_eq!(
            action.retry_state,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 7,
            })
        );
    }
    let block4 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(4).unwrap());
        header.creation_time_ms = 8;
    });
    let mut state_block4 = state.block(block4.as_ref().header());
    let events4 = state_block4.apply(&block4, Vec::new());
    let completions4: Vec<_> = events4
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(completions4.len(), 1, "expected retry success");
    assert!(matches!(
        completions4[0].outcome(),
        TriggerCompletedOutcome::Success
    ));
    state_block4.commit().unwrap();
    persist_committed_test_block(&kura, &block4);
    {
        let view = state.view();
        let alice_asset = view
            .world
            .asset(&alice_asset_id)
            .expect("retry success should mint exactly once");
        assert_eq!(&**alice_asset.value(), &Quantity::from(1_u32));
        let action = view
            .world
            .triggers()
            .time_triggers()
            .get(&trigger_id)
            .expect("periodic trigger should remain after successful retry");
        assert_eq!(action.repeats, Repeats::Exactly(2));
        assert_eq!(action.retry_state, None);
    }
    let block5 = new_dummy_block_with_payload(|header| {
        header.set_height(NonZeroU64::new(5).unwrap());
        header.creation_time_ms = 10;
    });
    let mut state_block5 = state.block(block5.as_ref().header());
    let events5 = state_block5.apply(&block5, Vec::new());
    let completions5: Vec<_> = events5
        .iter()
        .filter_map(|event| match event {
            EventBox::TriggerCompleted(event) if event.trigger_id() == &trigger_id => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions5.len(),
        1,
        "expected next scheduled tick only once"
    );
    assert!(matches!(
        completions5[0].outcome(),
        TriggerCompletedOutcome::Success
    ));
    state_block5.commit().unwrap();
    persist_committed_test_block(&kura, &block5);
    let view = state.view();
    let alice_asset = view
        .world
        .asset(&alice_asset_id)
        .expect("periodic trigger should mint the next scheduled tick");
    assert_eq!(&**alice_asset.value(), &Quantity::from(2_u32));
    let action = view
        .world
        .triggers()
        .time_triggers()
        .get(&trigger_id)
        .expect("periodic trigger should still have one repeat left");
    assert_eq!(action.repeats, Repeats::Exactly(1));
    assert_eq!(action.retry_state, None);
}
#[test]
fn ivm_trigger_respects_pipeline_cycle_cap() {
    use iroha_data_model::{
        events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter},
        transaction::{Executable, IvmBytecode},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query_handle);
    let mut pipeline = state.pipeline.clone();
    pipeline.ivm_max_cycles_upper_bound = NonZeroU64::new(1).expect("one is non-zero");
    state.set_pipeline(pipeline);
    let block = new_dummy_block_with_payload(|_| {});
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
    Register::domain(Domain::new(domain_id.clone()))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let trigger_id: TriggerId = "ivm_gas_guard".parse().unwrap();
    let mut raw = Vec::new();
    // Two ADD instructions cost two cycles in total, exceeding the configured cap of one.
    raw.extend_from_slice(
        &ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::ADD, 3, 1, 2)
            .to_le_bytes(),
    );
    raw.extend_from_slice(
        &ivm::encoding::wide::encode_rr(ivm::instruction::wide::arithmetic::ADD, 3, 1, 2)
            .to_le_bytes(),
    );
    raw.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let mut bytecode = assemble_ivm_header(&raw);
    bytecode[8..16].copy_from_slice(&1_u64.to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(bytecode);
    let filter = ExecuteTriggerEventFilter::new()
        .for_trigger(trigger_id.clone())
        .under_authority(ALICE_ID.clone());
    let action = Action::new(
        Executable::Ivm(bytecode),
        Repeats::Exactly(1),
        ALICE_ID.clone(),
        filter,
    )
    .expect("trigger action fixture satisfies validation invariants");
    Register::trigger(Trigger::new(trigger_id.clone(), action))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    stx.apply();
    state_block.commit().unwrap();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let evt = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &evt)
        .expect_err("trigger should exhaust the configured gas budget");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
            assert!(msg.contains("max cycles"), "unexpected error: {msg}");
        }
        other => panic!("unexpected rejection: {other:?}"),
    }
}
#[test]
fn ivm_time_trigger_reuses_cache_across_blocks() {
    use iroha_data_model::{
        events::time::{ExecutionTime, TimeEventFilter},
        transaction::{Executable, IvmBytecode},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let block1 = new_dummy_block_with_payload(|h| {
        h.set_height(NonZeroU64::new(1).unwrap());
        h.creation_time_ms = 1;
    });
    let mut state_block = state.block(block1.as_ref().header());
    let mut stx = state_block.transaction();
    Register::domain(Domain::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
    ))
    .execute(&ALICE_ID, &mut stx)
    .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let trigger_id: TriggerId = "ivm_time_cache".parse().unwrap();
    let mut raw = Vec::new();
    raw.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(assemble_ivm_header(&raw));
    let trigger = Trigger::new(
        trigger_id,
        Action::new(
            Executable::Ivm(bytecode),
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .expect("trigger action fixture satisfies validation invariants"),
    );
    Register::trigger(trigger)
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    stx.apply();
    let _ = state_block.apply_without_execution(&block1, Vec::new());
    state_block.commit().unwrap();
    let block2 = new_dummy_block_with_payload(|h| {
        h.set_height(NonZeroU64::new(2).unwrap());
        h.creation_time_ms = 2;
    });
    let mut state_block2 = state.block(block2.as_ref().header());
    state_block2.execute_time_triggers(&block2.as_ref().header());
    let _ = state_block2.apply_without_execution(&block2, Vec::new());
    state_block2.commit().unwrap();
    let after_first = state.trigger_ivm_cache.lock().stats();
    let block3 = new_dummy_block_with_payload(|h| {
        h.set_height(NonZeroU64::new(3).unwrap());
        h.creation_time_ms = 3;
    });
    let mut state_block3 = state.block(block3.as_ref().header());
    state_block3.execute_time_triggers(&block3.as_ref().header());
    let _ = state_block3.apply_without_execution(&block3, Vec::new());
    state_block3.commit().unwrap();
    let after_second = state.trigger_ivm_cache.lock().stats();
    assert!(
        after_second.metadata_hits > after_first.metadata_hits,
        "warm generic trigger must resolve its retained summary without parsing"
    );
    assert!(
        after_second.runtime_hits > after_first.runtime_hits,
        "warm generic trigger must check out its retained dirty-reset runtime"
    );
    assert_eq!(
        after_second.artifact_hashes, after_first.artifact_hashes,
        "warm generic trigger must not hash or copy its stored program"
    );
    assert_eq!(
        after_second.preparations, after_first.preparations,
        "warm generic trigger must not parse, validate, or predecode its program again"
    );
    assert_eq!(
        after_second.prepared_loads, after_first.prepared_loads,
        "warm generic trigger must not load its program into another VM"
    );
    assert_eq!(
        after_second.template_builds, after_first.template_builds,
        "warm generic trigger must not reconstruct its runtime template"
    );
}
#[test]
fn contract_query_cache_isolated_and_reuses_owned_runtime() {
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    const GAS_LIMIT: u64 = 10_000;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let mut program = ivm::ProgramMetadata::default().encode();
    let interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "QueryCacheFixture".to_owned(),
        compiler_fingerprint: "iroha-core-state-tests".to_owned(),
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "inspect".to_owned(),
            kind: EntryPointKind::View,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        error_codes: Vec::new(),
        states: Vec::new(),
    };
    program.extend_from_slice(&interface.encode_section());
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let code_hash = ivm::contract_code_hash(&program);
    let first = state
        .prepare_contract_query_program(code_hash, &program)
        .expect("cold preparation");
    let allocation = {
        let mut runtime = first
            .checkout_runtime(GAS_LIMIT, ivm::Memory::HEAP_MAX_SIZE)
            .expect("cold runtime");
        let allocation = runtime
            .memory
            .load_region(0, 1)
            .expect("code memory")
            .as_ptr();
        runtime.set_register(7, 99);
        runtime
            .memory
            .preload_input(0, &[0xA5])
            .expect("dirty input page");
        allocation
    };
    // A trusted content-addressed hit neither reads nor reparses the byte
    // slice. The empty slice makes accidental fallback validation fail.
    let second = state
        .prepare_contract_query_program(code_hash, &[])
        .expect("content-addressed hit");
    let runtime = second
        .checkout_runtime(GAS_LIMIT, ivm::Memory::HEAP_MAX_SIZE)
        .expect("warm runtime");
    assert_eq!(runtime.register(7), 0);
    assert_eq!(runtime.remaining_gas(), GAS_LIMIT);
    assert_eq!(
        runtime
            .memory
            .load_region(0, 1)
            .expect("code memory")
            .as_ptr(),
        allocation
    );
    assert_eq!(
        runtime
            .memory
            .load_region(0x0020_0000, 1)
            .expect("input memory"),
        &[0]
    );
    drop(runtime);
    let summary_stats = state.contract_query_ivm_cache_stats();
    let prepared_stats = state.contract_query_prepared_cache_stats();
    assert_eq!(summary_stats.metadata_misses, 1);
    assert_eq!(summary_stats.metadata_hits, 1);
    assert_eq!(prepared_stats.runtime_misses, 1);
    assert_eq!(prepared_stats.runtime_hits, 1);
    assert_eq!(prepared_stats.runtime_prepared_loads, 1);
    assert_eq!(prepared_stats.runtime_template_builds, 1);
    assert_eq!(prepared_stats.runtime_dirty_resets, 2);
    assert_eq!(
        state.trigger_ivm_cache.lock().stats().metadata_misses,
        0,
        "public query preparation must not churn the consensus trigger cache"
    );
}
#[test]
fn execute_called_trigger_fails_closed_on_missing_bytecode_with_warm_prepared_artifact() {
    use iroha_data_model::{
        events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter},
        transaction::{Executable, IvmBytecode},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "missing_bytecode_by_call".parse().unwrap();
    let program = ivm::KotodamaCompiler::new()
        .compile_source(
            r#"
seiyaku MissingBytecodeTrigger {
  kotoage fn main(int marker) authorize("missing_bytecode_probe") {
let _marker = marker;
  }
}
"#,
        )
        .expect("compile missing-bytecode trigger probe");
    let code_hash = ivm::contract_code_hash(&program);
    state
        .pipeline_ivm_prepared_cache
        .read()
        .get_or_prepare(code_hash, &program)
        .expect("prewarm authenticated trigger artifact");
    let bytecode = IvmBytecode::from_compiled(program);
    let blob_hash = HashOf::new(&bytecode);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let action = Action::new(
            Executable::Ivm(bytecode),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    assert!(
        state.world.triggers.remove_contract_for_test(blob_hash),
        "contract entry should be removed for test setup"
    );
    assert!(
        state
            .pipeline_ivm_prepared_cache
            .read()
            .get(code_hash)
            .is_some(),
        "adversarial fixture must retain a warm prepared artifact"
    );
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &event)
        .expect_err("missing bytecode should reject trigger execution");
    let err_debug = format!("{err:?}");
    assert!(
        err_debug.contains("missing trigger bytecode"),
        "unexpected error: {err_debug}"
    );
    drop(stx);
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_some(),
        "failed execution should not unregister the trigger in the rolled-back transaction"
    );
}
#[test]
fn execute_called_trigger_rejects_depleted_entry_and_prunes_trigger() {
    use iroha_data_model::{
        events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "depleted_by_call".parse().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    {
        let mut trigger_block = state.world.triggers.block();
        let mut trigger_tx = trigger_block.transaction();
        let updated = trigger_tx.inspect_by_id_mut(&trigger_id, |action| {
            action.set_repeats(Repeats::Exactly(0));
        });
        assert!(
            updated.is_some(),
            "trigger should be present for corruption"
        );
        trigger_tx.apply();
        trigger_block.commit();
    }
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &event)
        .expect_err("depleted trigger should be rejected");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Find(FindError::Trigger(id)),
        )) => assert_eq!(id, trigger_id),
        other => panic!("unexpected rejection: {other:?}"),
    }
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_none(),
        "depleted trigger should be removed"
    );
}
#[test]
fn execute_called_trigger_rejects_disabled_trigger() {
    use iroha_data_model::events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter};
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "disabled_by_call".parse().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let mut metadata = Metadata::default();
        metadata.insert(
            crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY
                .parse::<Name>()
                .expect("valid metadata key"),
            Json::from(false),
        );
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_metadata(metadata);
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &event)
        .expect_err("disabled trigger should be rejected");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Find(FindError::Trigger(id)),
        )) => assert_eq!(id, trigger_id),
        other => panic!("unexpected rejection: {other:?}"),
    }
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_some(),
        "disabled trigger should remain registered"
    );
}
#[test]
fn execute_called_trigger_rejects_numeric_zero_enabled_trigger() {
    use iroha_data_model::events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter};
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "numeric_zero_by_call".parse().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let mut metadata = Metadata::default();
        metadata.insert(
            crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY
                .parse::<Name>()
                .expect("valid metadata key"),
            Json::from(0_u64),
        );
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_metadata(metadata);
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &event)
        .expect_err("numeric-zero enabled trigger should be rejected");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Find(FindError::Trigger(id)),
        )) => assert_eq!(id, trigger_id),
        other => panic!("unexpected rejection: {other:?}"),
    }
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    let trigger = view
        .world
        .triggers()
        .by_call_triggers()
        .get(&trigger_id)
        .expect("disabled trigger should remain registered");
    assert_eq!(trigger.repeats(), &Repeats::Exactly(1));
    assert!(
        view.world
            .triggers()
            .active_by_call_trigger_ids()
            .get(&trigger_id)
            .is_none(),
        "numeric-zero enabled trigger must not appear active"
    );
}
#[test]
fn execute_called_trigger_rejects_malformed_enabled_trigger() {
    use iroha_data_model::events::execute_trigger::{ExecuteTriggerEvent, ExecuteTriggerEventFilter};
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "malformed_enabled_by_call".parse().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let mut metadata = Metadata::default();
        metadata.insert(
            crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY
                .parse::<Name>()
                .expect("valid metadata key"),
            Json::from(norito::json!({"malformed": true})),
        );
        let action = Action::new(
            Vec::<InstructionBox>::new(),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(trigger_id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_metadata(metadata);
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = ExecuteTriggerEvent {
        trigger_id: trigger_id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    let err = stx
        .execute_called_trigger(&trigger_id, &event)
        .expect_err("malformed enabled metadata should fail closed");
    match err {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::Find(FindError::Trigger(id)),
        )) => assert_eq!(id, trigger_id),
        other => panic!("unexpected rejection: {other:?}"),
    }
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    let trigger = view
        .world
        .triggers()
        .by_call_triggers()
        .get(&trigger_id)
        .expect("disabled trigger should remain registered");
    assert_eq!(trigger.repeats(), &Repeats::Exactly(1));
    assert!(
        view.world
            .triggers()
            .active_by_call_trigger_ids()
            .get(&trigger_id)
            .is_none(),
        "malformed enabled trigger must not appear active"
    );
}
#[test]
fn execute_data_triggers_dfs_skips_disabled_trigger() {
    use iroha_data_model::prelude::DataEvent;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "disabled_data_trigger".parse().unwrap();
    let flag_key: Name = "flag".parse().expect("valid name");
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let mut metadata = Metadata::default();
        metadata.insert(
            crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY
                .parse::<Name>()
                .expect("valid metadata key"),
            Json::from(false),
        );
        let action = Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                ALICE_ID.clone(),
                flag_key.clone(),
                Json::from(true),
            ))],
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            data_pre::DataEventFilter::Any,
        )
        .expect("trigger action fixture satisfies validation invariants")
        .with_metadata(metadata);
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = data_pre::DomainEvent::Created(
        Domain::new(DomainId::try_new("alpha", "universal").unwrap()).build(&ALICE_ID),
    );
    stx.world
        .internal_event_buf
        .push(Arc::new(DataEvent::Domain(event)));
    let steps = stx
        .execute_data_triggers_dfs(&ALICE_ID)
        .expect("disabled trigger should be skipped");
    assert!(steps.is_empty(), "disabled trigger should not execute");
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_some(),
        "disabled trigger should remain registered"
    );
    let flag_val = view
        .world
        .map_account(&ALICE_ID, |account| {
            account.value().metadata().get(&flag_key).cloned()
        })
        .unwrap();
    assert!(flag_val.is_none(), "disabled trigger must not mutate state");
}
#[test]
fn execute_data_triggers_dfs_skips_numeric_zero_and_malformed_enabled_triggers() {
    use iroha_data_model::prelude::DataEvent;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let numeric_trigger_id: TriggerId = "numeric_zero_data_trigger".parse().unwrap();
    let malformed_trigger_id: TriggerId = "malformed_enabled_data_trigger".parse().unwrap();
    let numeric_flag: Name = "numeric_data_flag".parse().expect("valid name");
    let malformed_flag: Name = "malformed_data_flag".parse().expect("valid name");
    let enabled_key = crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY
        .parse::<Name>()
        .expect("valid metadata key");
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        for (trigger_id, flag_key, enabled_value) in [
            (
                numeric_trigger_id.clone(),
                numeric_flag.clone(),
                Json::from(0_u64),
            ),
            (
                malformed_trigger_id.clone(),
                malformed_flag.clone(),
                Json::from(norito::json!([true])),
            ),
        ] {
            let mut metadata = Metadata::default();
            metadata.insert(enabled_key.clone(), enabled_value);
            let action = Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    flag_key,
                    Json::from(true),
                ))],
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                data_pre::DataEventFilter::Any,
            )
            .expect("trigger action fixture satisfies validation invariants")
            .with_metadata(metadata);
            Register::trigger(Trigger::new(trigger_id, action))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
        }
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = data_pre::DomainEvent::Created(
        Domain::new(DomainId::try_new("alpha", "universal").unwrap()).build(&ALICE_ID),
    );
    stx.world
        .internal_event_buf
        .push(Arc::new(DataEvent::Domain(event)));
    let steps = stx
        .execute_data_triggers_dfs(&ALICE_ID)
        .expect("disabled data triggers should be skipped");
    assert!(
        steps.is_empty(),
        "numeric-zero and malformed data triggers must not execute"
    );
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    let account = view.world.account(&ALICE_ID).expect("alice account");
    assert!(
        account.metadata().get(&numeric_flag).is_none(),
        "numeric-zero data trigger must not mutate state"
    );
    assert!(
        account.metadata().get(&malformed_flag).is_none(),
        "malformed-enabled data trigger must not mutate state"
    );
    for trigger_id in [&numeric_trigger_id, &malformed_trigger_id] {
        let trigger = view
            .world
            .triggers()
            .data_triggers()
            .get(trigger_id)
            .expect("disabled trigger should remain registered");
        assert_eq!(trigger.repeats(), &Repeats::Exactly(1));
        assert!(
            view.world
                .triggers()
                .active_data_trigger_ids()
                .get(trigger_id)
                .is_none(),
            "disabled data trigger must not appear active"
        );
    }
}
#[test]
fn execute_data_triggers_dfs_prunes_depleted_trigger_without_mutating_state() {
    use iroha_data_model::prelude::DataEvent;
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "depleted_data_trigger".parse().unwrap();
    let flag_key: Name = "depleted_data_flag".parse().expect("valid name");
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let action = Action::new(
            vec![InstructionBox::from(SetKeyValue::account(
                ALICE_ID.clone(),
                flag_key.clone(),
                Json::from(true),
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            data_pre::DataEventFilter::Any,
        )
        .expect("trigger action fixture satisfies validation invariants");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    {
        let mut trigger_block = state.world.triggers.block();
        let mut trigger_tx = trigger_block.transaction();
        let updated = trigger_tx.inspect_by_id_mut(&trigger_id, |action| {
            action.set_repeats(Repeats::Exactly(0));
        });
        assert!(
            updated.is_some(),
            "trigger should be present for depletion setup"
        );
        trigger_tx.apply();
        trigger_block.commit();
    }
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let event = data_pre::DomainEvent::Created(
        Domain::new(DomainId::try_new("alpha", "universal").unwrap()).build(&ALICE_ID),
    );
    stx.world
        .internal_event_buf
        .push(Arc::new(DataEvent::Domain(event)));
    let steps = stx
        .execute_data_triggers_dfs(&ALICE_ID)
        .expect("depleted trigger should be pruned without error");
    assert!(steps.is_empty(), "depleted trigger must not execute");
    stx.apply();
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_none(),
        "depleted data trigger should be pruned"
    );
    let flag_val = view
        .world
        .map_account(&ALICE_ID, |account| {
            account.value().metadata().get(&flag_key).cloned()
        })
        .unwrap();
    assert!(
        flag_val.is_none(),
        "depleted data trigger must not mutate state"
    );
}
#[test]
fn execute_data_triggers_dfs_clears_events_without_triggers() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    Register::domain(Domain::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
    ))
    .execute(&ALICE_ID, &mut stx)
    .unwrap();
    Register::account(new_sample_account(&ALICE_ID))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    let flag_key: Name = "flag".parse().expect("valid name");
    SetKeyValue::account(ALICE_ID.clone(), flag_key, Json::from(norito::json!(true)))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    assert!(
        !stx.world.internal_event_buf.is_empty(),
        "expected data events from metadata update"
    );
    assert!(
        stx.world.triggers.data_triggers().is_empty(),
        "test should run without data triggers"
    );
    let steps = stx
        .execute_data_triggers_dfs(&ALICE_ID)
        .expect("no triggers should yield ok result");
    assert!(steps.is_empty(), "no data triggers should execute");
    assert!(
        stx.world.internal_event_buf.is_empty(),
        "buffered events should be cleared when no triggers are registered"
    );
    stx.apply();
    state_block.commit().unwrap();
}
#[test]
fn execute_data_triggers_dfs_uses_registered_trigger_authority() {
    use iroha_primitives::json::Json;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(World::default(), kura, query_handle);
    let asset_def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
    let asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
    let flag_key: Name = "trigger_authority".parse().unwrap();
    let trigger_id: TriggerId = "data_trigger_registered_authority".parse().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        Register::account(new_sample_account(&BOB_ID))
            .execute(&BOB_ID, &mut stx)
            .unwrap();
        Register::asset_definition(AssetDefinition::numeric(
            asset_def_id.clone(),
            "rose",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            Some(DomainId::try_new("wonderland", "universal").unwrap()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        let data_trigger = Trigger::new(
            trigger_id.clone(),
            Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    BOB_ID.clone(),
                    flag_key.clone(),
                    Json::from(norito::json!("ok")),
                ))],
                Repeats::Exactly(1),
                BOB_ID.clone(),
                data_pre::DataEventFilter::Asset(
                    data_pre::AssetEventFilter::new().for_asset(asset_id.clone()),
                ),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        );
        Register::trigger(data_trigger)
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Mint::asset_quantity(1_u32, asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let steps = stx
            .execute_data_triggers_dfs(&ALICE_ID)
            .expect("data trigger should run under its registered authority");
        assert_eq!(steps.len(), 1, "expected one data trigger execution");
        stx.apply();
    }
    state_block.commit().unwrap();
    let flag_value = state
        .view()
        .world
        .map_account(&BOB_ID, |account| {
            account.value().metadata().get(&flag_key).cloned()
        })
        .unwrap();
    assert_eq!(flag_value, Some(Json::from(norito::json!("ok"))));
}
#[test]
fn execute_data_triggers_dfs_skips_missing_trigger_after_bytecode_drop() {
    use iroha_data_model::{
        prelude::DataEvent,
        transaction::{Executable, IvmBytecode},
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(World::default(), kura, query_handle);
    let trigger_id: TriggerId = "missing_bytecode_data".parse().unwrap();
    let mut raw = Vec::new();
    raw.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(assemble_ivm_header(&raw));
    let blob_hash = HashOf::new(&bytecode);
    let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    {
        let mut stx = state_block.transaction();
        Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        Register::account(new_sample_account(&ALICE_ID))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        let action = Action::new(
            Executable::Ivm(bytecode),
            Repeats::Indefinitely,
            ALICE_ID.clone(),
            data_pre::DataEventFilter::Any,
        )
        .expect("trigger action fixture satisfies validation invariants");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.apply();
    }
    state_block.commit().unwrap();
    assert!(
        state.world.triggers.remove_contract_for_test(blob_hash),
        "contract entry should be removed for test setup"
    );
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let mut stx = state_block.transaction();
    let alpha_domain: DomainId = DomainId::try_new("alpha", "universal").unwrap();
    let beta_domain: DomainId = DomainId::try_new("beta", "universal").unwrap();
    let event_a = data_pre::DomainEvent::Created(Domain::new(alpha_domain).build(&ALICE_ID));
    let event_b = data_pre::DomainEvent::Created(Domain::new(beta_domain).build(&ALICE_ID));
    stx.world
        .internal_event_buf
        .push(Arc::new(DataEvent::Domain(event_a)));
    stx.world
        .internal_event_buf
        .push(Arc::new(DataEvent::Domain(event_b)));
    let err = stx
        .execute_data_triggers_dfs(&ALICE_ID)
        .expect_err("missing bytecode should reject data-trigger execution");
    let err_debug = format!("{err:?}");
    assert!(
        err_debug.contains("missing trigger bytecode"),
        "unexpected error: {err_debug}"
    );
    drop(stx);
    state_block.commit().unwrap();
    let view = state.view();
    assert!(
        view.world.triggers().ids().get(&trigger_id).is_some(),
        "rolled-back missing-bytecode execution should leave repair/deserialization to prune"
    );
}
