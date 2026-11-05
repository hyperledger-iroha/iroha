//! Storage-seam lifecycle tests; no carrier emission or runtime activation.

use super::*;
use iroha_data_model::events::{
    pipeline::TransactionEventFilter,
    time::{ExecutionTime, Schedule},
};
use iroha_model_base::name::Name;
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use std::{num::NonZeroU32, time::Duration};

fn limits() -> LifecycleJournalLimits {
    LifecycleJournalLimits {
        effects: 64,
        total_frame_bytes: 1_048_576,
        scratch_bytes: 262_144,
        encode: norito::core::EncodeWorkLimits {
            max_frame_bytes: 65_536,
            max_byte_visits: 10_000_000,
            max_operations: 1_000_000,
            max_depth: 128,
        },
    }
}

fn insert(
    tx: &mut SetTransaction<'_>,
    name: &str,
    kind: TriggerLifecycleStateKindV1,
    repeats: u32,
) -> TriggerId {
    let id: TriggerId = name.parse().unwrap();
    let executable = Executable::Instructions(
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "captured".into(),
        ))]
        .into(),
    );
    macro_rules! add {
        ($method:ident, $filter:expr) => {{
            let mut action = SpecializedAction::new(
                executable,
                Repeats::Exactly(repeats),
                ALICE_ID.clone(),
                $filter,
            )
            .unwrap();
            action.metadata =
                crate::smartcontracts::isi::triggers::owned_data_trigger_scope_metadata_for_testing(
                );
            action.metadata.insert(
                "__registered_block_height".parse().unwrap(),
                Json::from(1_u64),
            );
            assert!(
                tx.$method(SpecializedTrigger::new(id.clone(), action))
                    .unwrap()
            );
        }};
    }
    match kind {
        TriggerLifecycleStateKindV1::Data => add!(
            add_data_trigger,
            DataEventFilter::Account(AccountEventFilter::new().for_account(ALICE_ID.clone()))
        ),
        TriggerLifecycleStateKindV1::Pipeline => add!(
            add_pipeline_trigger,
            PipelineEventFilterBox::Transaction(TransactionEventFilter::new())
        ),
        TriggerLifecycleStateKindV1::Time => add!(
            add_time_trigger,
            TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(10)
            )))
        ),
        TriggerLifecycleStateKindV1::ByCall => {
            add!(add_by_call_trigger, ExecuteTriggerEventFilter::new())
        }
    }
    id
}

#[test]
fn lifecycle_inserts_all_store_kinds_with_persistent_digest() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let pool = AllocationBudget::new(2_000_000);
    tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
    let kinds = [
        TriggerLifecycleStateKindV1::Data,
        TriggerLifecycleStateKindV1::Pipeline,
        TriggerLifecycleStateKindV1::Time,
        TriggerLifecycleStateKindV1::ByCall,
    ];
    for (i, kind) in kinds.into_iter().enumerate() {
        let id = insert(&mut tx, &format!("original_{i}"), kind, 2);
        assert!(
            tx.lifecycle_allows_apply(),
            "capture refused: {:?}",
            tx.lifecycle_journal.as_ref().unwrap().refusal
        );
        let effect = &tx.lifecycle_effects()[i];
        assert_eq!(effect.ordinal as usize, i);
        assert_eq!(effect.trigger_id.id, id);
        let LifecycleChange::Insert(snapshot) = &effect.change else {
            panic!("insert");
        };
        assert_eq!(snapshot.kind, kind);
        assert_eq!(snapshot.registered_at_height, Some(1));
        assert_eq!(
            snapshot.codec,
            TriggerLifecycleActionCodecV1::NoritoCanonicalV1
        );
        assert_eq!(
            snapshot.action.backing_bytes(),
            iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES
        );
        assert_eq!(
            snapshot.action_hash.is_some(),
            matches!(
                kind,
                TriggerLifecycleStateKindV1::Time | TriggerLifecycleStateKindV1::Pipeline
            )
        );
        if kind == TriggerLifecycleStateKindV1::Time {
            assert_eq!(
                snapshot.action_hash,
                Some(
                    invocation_identity::lifecycle_action_hash(
                        &tx,
                        tx.time_triggers.get(&id).unwrap(),
                        1,
                        &[],
                        None
                    )
                    .unwrap()
                    .0
                )
            );
        }
    }
    assert!(tx.lifecycle_allows_apply());
    assert!(pool.reserved_bytes() > 0);
    drop(tx);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(block.ids().len(), 0);
}

#[test]
fn lifecycle_metadata_repeats_and_retry_record_equal_state_touches_but_not_errors() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(&mut tx, "updates", TriggerLifecycleStateKindV1::Time, 3);
    tx.time_triggers.get_mut(&id).unwrap().retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(3).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    });
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    let key: Name = "touch".parse().unwrap();
    for _ in 0..2 {
        tx.inspect_by_id_mut(&id, |a| {
            a.metadata_mut().insert(key.clone(), Json::from(7_u64))
        })
        .unwrap();
    }
    tx.try_inspect_by_id_mut(&id, |a| a.metadata_mut().remove(&key).ok_or(()))
        .unwrap()
        .unwrap();
    assert!(
        tx.try_inspect_by_id_mut(&id, |a| a.metadata_mut().remove(&key).ok_or(()))
            .unwrap()
            .is_err()
    );
    tx.mod_repeats(&id, |n| Ok(n)).unwrap();
    tx.mod_repeats(&id, |n| Ok(n + 1)).unwrap();
    assert!(tx.mod_repeats(&id, |_| Err(RepeatsOverflowError)).is_err());
    let retry = TimeTriggerRetryState {
        retries_used: 1,
        next_retry_at_ms: 42,
    };
    assert!(tx.set_time_trigger_retry_state(&id, Some(retry)));
    assert!(tx.set_time_trigger_retry_state(&id, Some(retry)));
    assert!(tx.set_time_trigger_retry_state(&id, None));
    assert_eq!(tx.lifecycle_effects().len(), 8);
    for i in [1, 3, 6] {
        let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[i].change else {
            panic!("update");
        };
        assert_eq!(before.action.as_bytes(), after.action.as_bytes());
        assert_eq!(before.retry_state, after.retry_state);
        assert_eq!(before.action_hash, after.action_hash);
    }
    let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[5].change else {
        panic!("retry update");
    };
    assert_ne!(before.action.as_bytes(), after.action.as_bytes());
    assert_ne!(before.action_hash, after.action_hash);
    assert_eq!(after.retry_state.unwrap().attempt, 1);
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_remove_reinsert_preserves_encounter_order_and_absent_noops() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    let id = insert(
        &mut tx,
        "incarnation",
        TriggerLifecycleStateKindV1::ByCall,
        1,
    );
    assert!(tx.remove(&id));
    assert!(!tx.remove(&id));
    assert!(!tx.set_time_trigger_retry_state(&id, None));
    assert!(tx.inspect_by_id_mut(&id, |_| ()).is_none());
    insert(
        &mut tx,
        "incarnation",
        TriggerLifecycleStateKindV1::ByCall,
        7,
    );
    assert!(matches!(
        tx.lifecycle_effects()[0].change,
        LifecycleChange::Insert(_)
    ));
    assert!(matches!(
        tx.lifecycle_effects()[1].change,
        LifecycleChange::Remove(_)
    ));
    assert!(matches!(
        tx.lifecycle_effects()[2].change,
        LifecycleChange::Insert(_)
    ));
    let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[1].change else {
        unreachable!()
    };
    let LifecycleChange::Insert(original) = &tx.lifecycle_effects()[0].change else {
        panic!("insert");
    };
    assert_eq!(before.action.as_bytes(), original.action.as_bytes());
    let LifecycleChange::Insert(reinserted) = &tx.lifecycle_effects()[2].change else {
        panic!("insert");
    };
    assert_ne!(before.action.as_bytes(), reinserted.action.as_bytes());
    assert_eq!(tx.lifecycle_effects().len(), 3);
}

#[test]
fn lifecycle_sweep_captures_other_ids_in_original_store_order() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let live = insert(&mut tx, "live", TriggerLifecycleStateKindV1::ByCall, 1);
    for (name, kind) in [
        ("z_data", TriggerLifecycleStateKindV1::Data),
        ("y_pipeline", TriggerLifecycleStateKindV1::Pipeline),
        ("x_time", TriggerLifecycleStateKindV1::Time),
    ] {
        insert(&mut tx, name, kind, 0);
    }
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    tx.set_lifecycle_origin(Origin::InvocationPolicy {
        callback_index: 9,
        policy: Policy::ConsumeRepeat,
    });
    let removed = tx.decrease_repeats([&live].into_iter());
    assert_eq!(
        removed.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["z_data", "y_pipeline", "x_time", "live"]
    );
    assert_eq!(tx.lifecycle_effects().len(), 5);
    assert!(matches!(
        tx.lifecycle_effects()[0].change,
        LifecycleChange::Update(_, _)
    ));
    for effect in &tx.lifecycle_effects()[1..] {
        assert_eq!(
            effect.origin,
            Origin::InvocationPolicy {
                callback_index: 9,
                policy: Policy::RemoveDepleted
            }
        );
        assert!(matches!(effect.change, LifecycleChange::Remove(_)));
    }
    assert_eq!(
        tx.lifecycle_origin(),
        Some(Origin::InvocationPolicy {
            callback_index: 9,
            policy: Policy::ConsumeRepeat
        })
    );
}

#[test]
fn lifecycle_rekey_captures_every_changed_authority_and_filter_only() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for (i, kind) in [
        TriggerLifecycleStateKindV1::Data,
        TriggerLifecycleStateKindV1::Pipeline,
        TriggerLifecycleStateKindV1::Time,
        TriggerLifecycleStateKindV1::ByCall,
    ]
    .into_iter()
    .enumerate()
    {
        insert(&mut tx, &format!("rekey_{i}"), kind, 2);
    }
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    tx.replace_account_id(&BOB_ID, &BOB_ID).unwrap();
    assert_eq!(tx.lifecycle_effects().len(), 4);
    for effect in tx.lifecycle_effects() {
        let LifecycleChange::Update(before, after) = &effect.change else {
            panic!("rekey update");
        };
        assert_ne!(before.action.as_bytes(), after.action.as_bytes());
        assert_eq!(
            tx.inspect_by_id(&effect.trigger_id.id, |action| action.authority().clone()),
            Some(BOB_ID.clone())
        );
    }
}

#[test]
fn lifecycle_refusal_is_sticky_prevents_apply_and_releases_all_charges() {
    for (effects, total_frame_bytes, operations) in [
        (0, 1_048_576, 1_000_000),
        (64, 0, 1_000_000),
        (64, 1_048_576, 0),
    ] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let pool = AllocationBudget::new(2_000_000);
        let mut limits = limits();
        limits.effects = effects;
        limits.total_frame_bytes = total_frame_bytes;
        limits.encode.max_operations = operations;
        tx.enable_lifecycle_journal(pool.clone(), limits).unwrap();
        let checkpoint = tx.lifecycle_checkpoint();
        insert(&mut tx, "refused", TriggerLifecycleStateKindV1::ByCall, 1);
        assert!(!tx.lifecycle_allows_apply());
        tx.lifecycle_finish_callback(checkpoint, 0, false);
        assert!(!tx.lifecycle_allows_apply());
        tx.apply();
        assert_eq!(block.ids().len(), 0);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn lifecycle_disabled_and_transaction_drop_or_apply_have_no_cross_transaction_capture() {
    let set = Set::default();
    let mut block = set.block();
    let id = {
        let mut tx = block.transaction();
        assert!(!tx.lifecycle_enabled());
        let id = insert(&mut tx, "baseline", TriggerLifecycleStateKindV1::ByCall, 2);
        assert!(tx.lifecycle_effects().is_empty());
        tx.apply();
        id
    };
    let pool = AllocationBudget::new(2_000_000);
    {
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
        assert!(tx.remove(&id));
        assert_eq!(tx.lifecycle_effects().len(), 1);
    }
    assert!(block.ids().get(&id).is_some());
    assert_eq!(pool.reserved_bytes(), 0);
    let mut tx = block.transaction();
    assert!(!tx.lifecycle_enabled());
    assert!(
        tx.enable_lifecycle_journal(AllocationBudget::new(0), limits())
            .is_err()
    );
    assert!(!tx.lifecycle_enabled());
}

#[test]
fn lifecycle_ivm_snapshot_binds_bytecode_and_missing_source_is_explicit() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id: TriggerId = "bytecode".parse().unwrap();
    let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
    let hash = HashOf::new(&code);
    let action = SpecializedAction::new(
        Executable::Ivm(code.clone()),
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    assert!(
        tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap()
    );
    let LifecycleChange::Insert(snapshot) = &tx.lifecycle_effects()[0].change else {
        panic!("insert");
    };
    assert_eq!(
        snapshot.codec,
        TriggerLifecycleActionCodecV1::NoritoCanonicalV1
    );
    assert_eq!(
        snapshot.action.as_bytes(),
        lifecycle_hash_frame(
            invocation_identity::lifecycle_action_hash(
                &tx,
                tx.by_call_triggers.get(&id).unwrap(),
                3,
                &[],
                None
            )
            .unwrap()
            .0
        )
        .unwrap()
        .as_slice()
    );
    tx.contracts.remove(hash);
    tx.inspect_by_id_mut(&id, |a| a.set_repeats(Repeats::Exactly(1)))
        .unwrap();
    let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[1].change else {
        panic!("update");
    };
    assert_eq!(
        before.codec,
        TriggerLifecycleActionCodecV1::NoritoCanonicalV1
    );
    assert_eq!(
        after.action.as_bytes(),
        lifecycle_unavailable_frame(3).unwrap().as_slice()
    );
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_long_identifier_and_frames_keep_exact_original_charges_until_rollback() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let pool = AllocationBudget::new(2_000_000);
    tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
    assert!(tx.enable_lifecycle_journal(pool.clone(), limits()).is_err());
    let base = pool.reserved_bytes();
    let checkpoint = tx.lifecycle_checkpoint();
    let id = insert(
        &mut tx,
        &"long_identifier_".repeat(8),
        TriggerLifecycleStateKindV1::ByCall,
        2,
    );
    let LifecycleChange::Insert(snapshot) = &tx.lifecycle_effects()[0].change else {
        panic!("insert");
    };
    assert_eq!(
        pool.reserved_bytes() - base,
        snapshot.action.backing_bytes() + id.name().admission_clone_layout().unwrap().size()
    );
    tx.lifecycle_finish_callback(checkpoint, 0, false);
    assert!(tx.lifecycle_effects().is_empty());
    assert_eq!(pool.reserved_bytes(), base);
    drop(tx);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn lifecycle_failed_mutator_that_changed_storage_cannot_hide_retained_effects() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "partial_error",
        TriggerLifecycleStateKindV1::ByCall,
        2,
    );
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    assert!(
        tx.try_inspect_by_id_mut(&id, |action| {
            action.set_repeats(Repeats::Exactly(9));
            Err::<(), _>(())
        })
        .unwrap()
        .is_err()
    );
    assert_eq!(tx.lifecycle_effects().len(), 1);
    let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[0].change else {
        panic!("actual update");
    };
    assert_ne!(before.action.as_bytes(), after.action.as_bytes());
    assert_eq!(
        tx.inspect_by_id(&id, |action| *action.repeats()),
        Some(Repeats::Exactly(9))
    );
}

#[test]
fn lifecycle_absent_and_duplicate_mutations_need_no_effect_admission() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let existing = insert(&mut tx, "existing", TriggerLifecycleStateKindV1::ByCall, 2);
    let mut admission = limits();
    admission.effects = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    let duplicate = SpecializedAction::new(
        Executable::Instructions(ConstVec::new_empty()),
        Repeats::Exactly(9),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    assert!(
        !tx.add_by_call_trigger(SpecializedTrigger::new(existing, duplicate))
            .unwrap()
    );
    let absent = "absent".parse().unwrap();
    assert!(!tx.remove(&absent));
    assert!(!tx.set_time_trigger_retry_state(&absent, None));
    assert!(tx.inspect_by_id_mut(&absent, |_| ()).is_none());
    assert!(tx.lifecycle_effects().is_empty());
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_malformed_registration_metadata_has_publishable_repair() {
    for malformed in [
        Json::from("not a height"),
        Json::new(-1_i64),
        Json::from(3_u64),
    ] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id = insert(&mut tx, "bad_height", TriggerLifecycleStateKindV1::Time, 2);
        tx.inspect_by_id_mut(&id, |action| {
            action.metadata_mut().insert(
                super::super::super::TRIGGER_REGISTERED_BLOCK_HEIGHT_METADATA_KEY
                    .parse()
                    .unwrap(),
                malformed.clone(),
            );
        })
        .unwrap();
        tx.enable_selected_lifecycle_journal(AllocationBudget::new(2_000_000), limits(), 2)
            .unwrap();
        assert!(tx.remove(&id));
        let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[0].change else {
            panic!("remove");
        };
        assert_eq!(
            before.codec,
            TriggerLifecycleActionCodecV1::NoritoCanonicalV1
        );
        assert_eq!(
            before.action.as_bytes(),
            lifecycle_unavailable_frame(1).unwrap().as_slice()
        );
        let _journal = tx.take_lifecycle_journal().unwrap().unwrap();
        assert!(tx.lifecycle_allows_apply());
        tx.apply();
        assert!(block.ids.get(&id).is_none());
    }
}

#[test]
fn lifecycle_invalid_retry_source_has_publishable_repair() {
    for kind in [
        TriggerLifecycleStateKindV1::Time,
        TriggerLifecycleStateKindV1::Pipeline,
        TriggerLifecycleStateKindV1::Data,
        TriggerLifecycleStateKindV1::ByCall,
    ] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id = insert(&mut tx, "invalid_retry", kind, 1);
        let retry = Some(TimeTriggerRetryState {
            retries_used: if kind == TriggerLifecycleStateKindV1::Time {
                0
            } else {
                1
            },
            next_retry_at_ms: 42,
        });
        match kind {
            TriggerLifecycleStateKindV1::Time => {
                tx.time_triggers.get_mut(&id).unwrap().retry_state = retry
            }
            TriggerLifecycleStateKindV1::Pipeline => {
                tx.pipeline_triggers.get_mut(&id).unwrap().retry_state = retry
            }
            TriggerLifecycleStateKindV1::Data => {
                tx.data_triggers.get_mut(&id).unwrap().retry_state = retry
            }
            TriggerLifecycleStateKindV1::ByCall => {
                tx.by_call_triggers.get_mut(&id).unwrap().retry_state = retry
            }
        }
        tx.enable_selected_lifecycle_journal(AllocationBudget::new(2_000_000), limits(), 2)
            .unwrap();
        assert!(tx.remove(&id));
        let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[0].change else {
            panic!("repair");
        };
        assert_eq!(
            before.codec,
            TriggerLifecycleActionCodecV1::NoritoCanonicalV1
        );
        assert_eq!(
            before.action.as_bytes(),
            lifecycle_unavailable_frame(1).unwrap().as_slice()
        );
        let _journal = tx.take_lifecycle_journal().unwrap().unwrap();
        assert!(tx.lifecycle_allows_apply());
        tx.apply();
        assert!(block.ids.get(&id).is_none());
    }
}

#[test]
fn lifecycle_corrupt_ivm_artifact_has_publishable_repair() {
    for replace_blob in [true, false] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id: TriggerId = "corrupt_artifact".parse().unwrap();
        let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let hash = HashOf::new(&code);
        let action = SpecializedAction::new(
            Executable::Ivm(code),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
                .unwrap()
        );
        let entry = tx.contracts.get_mut(&hash).unwrap();
        if replace_blob {
            entry.original_contract = IvmBytecode::from_compiled(vec![9, 8, 7]).into();
        } else {
            entry.code_hash = Hash::prehashed([0x55; 32]);
        }
        tx.enable_selected_lifecycle_journal(AllocationBudget::new(2_000_000), limits(), 2)
            .unwrap();
        assert!(tx.remove(&id));
        let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[0].change else {
            panic!("repair");
        };
        assert_eq!(
            before.codec,
            TriggerLifecycleActionCodecV1::NoritoCanonicalV1
        );
        assert_eq!(
            before.action.as_bytes(),
            lifecycle_unavailable_frame(3).unwrap().as_slice()
        );
        let _journal = tx.take_lifecycle_journal().unwrap().unwrap();
        assert!(tx.lifecycle_allows_apply());
        tx.apply();
        assert!(block.ids.get(&id).is_none());
        assert!(block.contracts.get(&hash).is_none());
    }
}

#[test]
fn lifecycle_corrupt_ivm_repair_never_hides_work_or_allocation_refusal() {
    for exhaust_work in [true, false] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id: TriggerId = "bounded_corrupt_artifact".parse().unwrap();
        let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let hash = HashOf::new(&code);
        let action = SpecializedAction::new(
            Executable::Ivm(code),
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
                .unwrap()
        );
        tx.contracts.get_mut(&hash).unwrap().code_hash = Hash::prehashed([0x55; 32]);
        let pool = AllocationBudget::new(2_000_000);
        let mut admission = limits();
        if exhaust_work {
            admission.encode.max_byte_visits = 0;
        }
        tx.enable_selected_lifecycle_journal(pool.clone(), admission, 2)
            .unwrap();
        let _held = (!exhaust_work).then(|| {
            pool.try_reserve_bytes(2_000_000 - pool.reserved_bytes())
                .unwrap()
        });
        assert!(tx.remove(&id));
        assert!(tx.lifecycle_effects().is_empty());
        if exhaust_work {
            assert!(matches!(
                tx.lifecycle_failure(),
                Some(LifecycleJournalFailure::OutputLimit(_))
            ));
        } else {
            assert!(matches!(
                tx.lifecycle_failure(),
                Some(LifecycleJournalFailure::Allocation(_))
            ));
        }
        assert!(!tx.lifecycle_allows_apply());
        assert!(tx.take_lifecycle_journal().is_err());
    }
}

#[test]
fn lifecycle_deterministic_limits_are_root_rejections_not_local_refusals() {
    for (effects, frame_bytes, operations) in [
        (0, 1_048_576, 1_000_000),
        (64, 0, 1_000_000),
        (64, 1_048_576, 0),
    ] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let mut admission = limits();
        admission.effects = effects;
        admission.total_frame_bytes = frame_bytes;
        admission.encode.max_operations = operations;
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
            .unwrap();
        insert(&mut tx, "limit", TriggerLifecycleStateKindV1::ByCall, 2);
        assert!(
            matches!(
                tx.lifecycle_failure(),
                Some(LifecycleJournalFailure::OutputLimit(_))
            ),
            "{:?}",
            tx.lifecycle_failure()
        );
        assert!(matches!(
            tx.take_lifecycle_journal(),
            Err(LifecycleJournalFailure::OutputLimit(_))
        ));
    }
}

#[test]
fn lifecycle_local_frame_allocation_refusal_remains_distinct() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let pool = AllocationBudget::new(2_000_000);
    tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
    let held = pool
        .try_reserve_bytes(2_000_000 - pool.reserved_bytes())
        .unwrap();
    insert(&mut tx, "local", TriggerLifecycleStateKindV1::ByCall, 2);
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::Allocation(_))
    ));
    drop(held);
    assert!(!tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_unrelated_rekey_needs_no_snapshot_or_registration_metadata() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let unrelated = insert(&mut tx, "unrelated", TriggerLifecycleStateKindV1::Time, 2);
    let action = tx.time_triggers.get_mut(&unrelated).unwrap();
    action.authority = BOB_ID.clone();
    action.metadata.remove("__registered_block_height");
    action
        .metadata
        .insert("large".parse().unwrap(), Json::new("x".repeat(70_000)));
    let changed = insert(&mut tx, "changed", TriggerLifecycleStateKindV1::ByCall, 2);
    let mut admission = limits();
    admission.effects = 1;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 1);
    assert_eq!(tx.lifecycle_effects()[0].trigger_id.id, changed);
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_indefinite_and_arithmetic_refusals_need_no_effect_slot() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let infinite = insert(&mut tx, "infinite", TriggerLifecycleStateKindV1::ByCall, 2);
    tx.by_call_triggers.get_mut(&infinite).unwrap().repeats = Repeats::Indefinitely;
    let finite = insert(&mut tx, "finite", TriggerLifecycleStateKindV1::ByCall, 2);
    let mut admission = limits();
    admission.effects = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    assert!(tx.decrease_repeats([&infinite].into_iter()).is_empty());
    assert!(
        tx.mod_repeats(&finite, |_| Err(RepeatsOverflowError))
            .is_err()
    );
    assert!(tx.lifecycle_allows_apply());
    assert!(tx.lifecycle_effects().is_empty());
}

#[test]
fn lifecycle_depleted_cleanup_matches_bulk_after_limit_failure_callback_and_id_mismatch() {
    for mode in 0..4 {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let mut expected = Vec::new();
        for (i, kind) in [
            TriggerLifecycleStateKindV1::Data,
            TriggerLifecycleStateKindV1::Pipeline,
            TriggerLifecycleStateKindV1::Time,
            TriggerLifecycleStateKindV1::ByCall,
        ]
        .into_iter()
        .enumerate()
        {
            expected.push(insert(&mut tx, &format!("depleted_{i}"), kind, 0));
        }
        let mut admission = limits();
        if mode == 0 {
            admission.effects = 0;
        }
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
            .unwrap();
        match mode {
            1 => tx.lifecycle_finish_callback(tx.lifecycle_checkpoint(), 0, false),
            2 => tx.refuse_lifecycle("injected local refusal"),
            3 => {
                tx.ids.remove(expected[0].clone());
            }
            _ => {}
        }
        let removed = tx.decrease_repeats(std::iter::empty());
        assert_eq!(removed, expected);
        assert!(tx.ids.is_empty());
        assert!(tx.data_triggers.is_empty());
        assert!(tx.pipeline_triggers.is_empty());
        assert!(tx.time_triggers.is_empty());
        assert!(tx.by_call_triggers.is_empty());
        assert_eq!(tx.lifecycle_allows_apply(), mode == 3);
        if mode == 3 {
            assert!(tx.lifecycle_failure().is_none());
            let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[0].change else {
                panic!("repair");
            };
            assert_eq!(
                before.action.as_bytes(),
                lifecycle_unavailable_frame(4).unwrap().as_slice()
            );
        }
    }
}

#[test]
fn lifecycle_depleted_sweep_spends_one_step_per_entry_without_rescanning() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for i in 0..12 {
        insert(
            &mut tx,
            &format!("entry_{i:02}"),
            TriggerLifecycleStateKindV1::ByCall,
            if i % 2 == 0 { 1 } else { 0 },
        );
    }
    let mut admission = limits();
    admission.encode.max_operations = 12;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.lifecycle_finish_callback(tx.lifecycle_checkpoint(), 0, false);
    assert_eq!(tx.decrease_repeats(std::iter::empty()).len(), 6);
    assert_eq!(
        tx.lifecycle_journal.as_ref().unwrap().remaining_scan_steps,
        0
    );
    assert!(tx.lifecycle_failure().is_none());
}

#[test]
fn lifecycle_failed_callback_does_not_report_an_existing_invoke_as_absent() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "existing_invoke",
        TriggerLifecycleStateKindV1::ByCall,
        2,
    );
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    tx.lifecycle_finish_callback(tx.lifecycle_checkpoint(), 0, false);
    tx.lifecycle_invoke(&id, 1);
    assert!(tx.lifecycle_failure().is_none());
    assert!(!tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_custody_transfer_keeps_original_pool_and_closes_mutation() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let pool = AllocationBudget::new(2_000_000);
    tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
    let id = insert(
        &mut tx,
        "transferred",
        TriggerLifecycleStateKindV1::ByCall,
        2,
    );
    let journal = tx.take_lifecycle_journal().unwrap().unwrap();
    assert!(journal.effects.belongs_to(&pool));
    assert!(tx.lifecycle_allows_apply());
    assert!(tx.take_lifecycle_journal().is_err());
    assert!(tx.remove(&id));
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::Refused(_))
    ));
    assert!(!tx.lifecycle_allows_apply());
    drop(tx);
    assert!(pool.reserved_bytes() > 0);
    drop(journal);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn lifecycle_selected_journal_requires_transfer_even_for_empty_coverage() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    tx.enable_selected_lifecycle_journal(AllocationBudget::new(2_000_000), limits(), 2)
        .unwrap();
    assert!(tx.lifecycle_failure().is_none());
    assert!(!tx.lifecycle_allows_apply());
    let journal = tx.take_lifecycle_journal().unwrap().unwrap();
    assert!(journal.effects.as_slice().is_empty());
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_codec_work_is_cumulative_across_snapshot_frames() {
    let set = Set::default();
    let mut block = set.block();
    let per_snapshot = {
        let mut tx = block.transaction();
        let admission = limits();
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
            .unwrap();
        insert(&mut tx, "measure", TriggerLifecycleStateKindV1::ByCall, 2);
        assert!(tx.lifecycle_allows_apply());
        admission.encode.max_byte_visits as u64
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .remaining_byte_visits()
    };
    let mut tx = block.transaction();
    let mut admission = limits();
    admission.encode.max_byte_visits = usize::try_from(per_snapshot + 1).unwrap();
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.lifecycle_journal.as_mut().unwrap().encode_work =
        LifecycleWorkBudget::new(per_snapshot + 1, admission.encode.max_operations as u64);
    insert(&mut tx, "first", TriggerLifecycleStateKindV1::ByCall, 2);
    assert!(tx.lifecycle_allows_apply());
    insert(&mut tx, "second", TriggerLifecycleStateKindV1::ByCall, 2);
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::OutputLimit(_))
    ));
    assert_eq!(tx.lifecycle_effects().len(), 1);
}

#[test]
fn lifecycle_wrong_kind_retry_lookup_needs_no_snapshot() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(&mut tx, "not_time", TriggerLifecycleStateKindV1::ByCall, 2);
    let mut admission = limits();
    admission.effects = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    assert!(!tx.set_time_trigger_retry_state(&id, None));
    assert!(tx.lifecycle_allows_apply());
    assert!(tx.lifecycle_effects().is_empty());
}

#[test]
fn lifecycle_explicit_limit_preserves_local_failure_and_consumed_journal_guard() {
    let set = Set::default();
    let mut block = set.block();
    {
        let mut tx = block.transaction();
        tx.limit_lifecycle("disabled callback limit");
        assert!(tx.lifecycle_failure().is_none());
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
            .unwrap();
        tx.limit_lifecycle("callback ordinal limit");
        assert_eq!(
            tx.lifecycle_failure(),
            Some(LifecycleJournalFailure::OutputLimit(
                "callback ordinal limit"
            ))
        );
        tx.refuse_lifecycle("original local failure");
        tx.limit_lifecycle("later callback limit");
        tx.refuse_lifecycle("later local failure");
        assert_eq!(
            tx.lifecycle_failure().unwrap().reason(),
            "original local failure"
        );
        assert!(matches!(
            tx.lifecycle_failure(),
            Some(LifecycleJournalFailure::Refused(_))
        ));
    }
    let mut tx = block.transaction();
    tx.enable_selected_lifecycle_journal(AllocationBudget::new(2_000_000), limits(), 2)
        .unwrap();
    let _journal = tx.take_lifecycle_journal().unwrap().unwrap();
    assert!(tx.lifecycle_allows_apply());
    tx.limit_lifecycle("limit after transfer");
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::Refused(_))
    ));
    assert!(!tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_missing_height_and_typed_entry_can_be_unregistered() {
    for absent_typed in [false, true] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id = insert(&mut tx, "repair", TriggerLifecycleStateKindV1::Time, 1);
        if absent_typed {
            tx.time_triggers.remove(id.clone());
        } else {
            tx.time_triggers
                .get_mut(&id)
                .unwrap()
                .metadata
                .remove(super::super::super::TRIGGER_REGISTERED_BLOCK_HEIGHT_METADATA_KEY);
        }
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
            .unwrap();
        assert!(tx.remove(&id));
        let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[0].change else {
            panic!("repair");
        };
        assert_eq!(
            before.codec,
            TriggerLifecycleActionCodecV1::NoritoCanonicalV1
        );
        assert_eq!(
            before.action.as_bytes(),
            lifecycle_unavailable_frame(if absent_typed { 2 } else { 1 })
                .unwrap()
                .as_slice()
        );
        assert!(tx.lifecycle_allows_apply());
        tx.apply();
        assert!(block.ids.get(&id).is_none());
    }
}

#[test]
fn lifecycle_absent_invoke_is_deterministic_not_local() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    tx.lifecycle_invoke(&"absent".parse().unwrap(), 0);
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::OutputLimit(_))
    ));
}

#[test]
fn lifecycle_repeated_sweeps_have_independent_step_budgets() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for n in 0..8 {
        insert(
            &mut tx,
            &format!("live_{n}"),
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
    }
    let mut admission = limits();
    admission.encode.max_operations = 8;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    for _ in 0..300 {
        assert!(tx.decrease_repeats(std::iter::empty()).is_empty());
        assert!(tx.lifecycle_failure().is_none());
    }
}

#[test]
fn lifecycle_noop_clear_retry_uses_no_effect_or_frame_capacity() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(&mut tx, "no_retry", TriggerLifecycleStateKindV1::Time, 1);
    let mut admission = limits();
    admission.effects = 0;
    admission.total_frame_bytes = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    assert!(tx.set_time_trigger_retry_state(&id, None));
    assert!(tx.lifecycle_effects().is_empty());
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_large_ivm_action_retains_constant_size_invoke_and_repeat_evidence() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id: TriggerId = "large_code".parse().unwrap();
    let action = SpecializedAction::new(
        Executable::Ivm(IvmBytecode::from_compiled(vec![1; 350_000])),
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    assert!(
        tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap()
    );
    let mut admission = limits();
    admission.encode.max_frame_bytes = 1_048_576;
    admission.encode.max_byte_visits = 256 * 1024 * 1024;
    admission.total_frame_bytes =
        3 * iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.lifecycle_invoke(&id, 0);
    tx.mod_repeats(&id, |count| Ok(count - 1)).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 2);
    assert_eq!(
        tx.lifecycle_journal.as_ref().unwrap().frame_bytes,
        3 * iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES
    );
}

#[test]
fn lifecycle_failure_policy_work_exhaustion_retains_explicit_effect() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(&mut tx, "policy_work", TriggerLifecycleStateKindV1::Time, 1);
    tx.time_triggers.get_mut(&id).unwrap().retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(2).unwrap(),
        retry_after_ms: NonZeroU64::new(5).unwrap(),
    });
    let mut admission = limits();
    admission.encode.max_operations = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.set_lifecycle_origin(Origin::InvocationPolicy {
        callback_index: 0,
        policy: Policy::AdvanceTimeRetry,
    });
    assert!(tx.set_time_trigger_retry_state(
        &id,
        Some(TimeTriggerRetryState {
            retries_used: 1,
            next_retry_at_ms: 42,
        })
    ));
    let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[0].change else {
        panic!("retry update");
    };
    assert_eq!(
        before.codec,
        TriggerLifecycleActionCodecV1::NoritoCanonicalV1
    );
    assert_eq!(
        before.action.as_bytes(),
        lifecycle_unavailable_frame(5).unwrap().as_slice()
    );
    assert_eq!(
        after.action.as_bytes(),
        lifecycle_unavailable_frame(5).unwrap().as_slice()
    );
    assert!(tx.lifecycle_allows_apply());
    tx.apply();
    assert_eq!(
        block
            .time_triggers
            .get(&id)
            .unwrap()
            .retry_state
            .unwrap()
            .retries_used,
        1
    );
}

#[test]
fn lifecycle_sweep_repairs_duplicate_ids_across_typed_stores_once_each() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let shared = insert(&mut tx, "shared", TriggerLifecycleStateKindV1::Data, 0);
    let other = insert(&mut tx, "other", TriggerLifecycleStateKindV1::Time, 0);
    let duplicate = tx.time_triggers.get(&other).unwrap().clone();
    assert!(tx.remove(&other));
    tx.time_triggers.insert(shared.clone(), duplicate);
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    assert_eq!(
        tx.decrease_repeats(std::iter::empty()),
        vec![shared.clone(), shared]
    );
    assert_eq!(tx.lifecycle_effects().len(), 2);
    let LifecycleChange::Remove(before) = &tx.lifecycle_effects()[1].change else {
        panic!("stray repair");
    };
    assert_eq!(
        before.codec,
        TriggerLifecycleActionCodecV1::NoritoCanonicalV1
    );
    assert_eq!(
        before.action.as_bytes(),
        lifecycle_unavailable_frame(4).unwrap().as_slice()
    );
    assert!(tx.data_triggers.is_empty());
    assert!(tx.time_triggers.is_empty());
    assert!(tx.lifecycle_allows_apply());
}

#[test]
fn lifecycle_unavailable_invocation_is_a_deterministic_root_limit() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "invalid_invocation",
        TriggerLifecycleStateKindV1::Time,
        2,
    );
    tx.set_time_trigger_retry_state(
        &id,
        Some(
            crate::smartcontracts::isi::triggers::specialized::TimeTriggerRetryState {
                retries_used: 0,
                next_retry_at_ms: 1,
            },
        ),
    );
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
        .unwrap();
    tx.lifecycle_invoke(&id, 0);
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::OutputLimit(_))
    ));
    assert!(tx.lifecycle_effects().is_empty());
    assert!(!tx.lifecycle_allows_apply());
}

fn shared_artifact_rekey_work(code_bytes: usize, triggers: usize) -> u64 {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let code = IvmBytecode::from_compiled(vec![1; code_bytes]);
    let hash = HashOf::new(&code);
    let first: TriggerId = "large_rekey_0".parse().unwrap();
    assert!(
        tx.add_by_call_trigger(SpecializedTrigger::new(
            first.clone(),
            SpecializedAction::new(
                Executable::Ivm(code),
                Repeats::Exactly(2),
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new()
            )
            .unwrap()
        ))
        .unwrap()
    );
    // Seed the shared stored representation directly: fixture construction must
    // not hash or allocate the same 4 MiB artifact one thousand times itself.
    let action = tx.by_call_triggers.get(&first).unwrap().clone();
    for index in 1..triggers {
        let id: TriggerId = format!("large_rekey_{index}").parse().unwrap();
        tx.by_call_triggers.insert(id.clone(), action.clone());
        tx.ids
            .insert(id.clone(), TriggeringEventType::ExecuteTrigger);
        tx.active_by_call_trigger_ids.insert(id, ());
    }
    tx.contracts.get_mut(&hash).unwrap().count =
        NonZeroU64::new(triggers.try_into().unwrap()).unwrap();
    let admission = LifecycleJournalLimits::selected(1_048_576);
    assert_eq!(admission.encode.max_byte_visits, 1024 * 1024 * 1024);
    assert_eq!(admission.encode.max_operations, 16 * 1024 * 1024);
    let pool = AllocationBudget::new(32 * 1024 * 1024);
    tx.enable_lifecycle_journal(pool.clone(), admission)
        .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), triggers);
    let journal = tx.lifecycle_journal.as_ref().unwrap();
    assert_eq!(journal.verified_ivm.as_slice().len(), 1);
    let consumed =
        admission.encode.max_byte_visits as u64 - journal.encode_work.remaining_byte_visits();
    assert!(
        tx.by_call_triggers
            .iter()
            .all(|(_, action)| action.authority == *BOB_ID)
    );
    // Mutable fields are still hashed freshly, including the rekeyed authority.
    let LifecycleChange::Update(before, after) = &tx.lifecycle_effects()[0].change else {
        panic!("rekey")
    };
    assert_ne!(before.action.as_bytes(), after.action.as_bytes());
    assert_eq!(
        after.action.as_bytes(),
        lifecycle_hash_frame(
            invocation_identity::lifecycle_action_hash(
                &tx,
                tx.by_call_triggers.get(&first).unwrap(),
                3,
                &[],
                None
            )
            .unwrap()
            .0
        )
        .unwrap()
        .as_slice()
    );
    drop(tx);
    assert_eq!(pool.reserved_bytes(), 0);
    consumed
}

#[test]
fn lifecycle_production_envelope_rekeys_one_thousand_shared_four_mib_actions() {
    let one_small = shared_artifact_rekey_work(0, 1);
    let one_large = shared_artifact_rekey_work(4 * 1024 * 1024, 1);
    let small = shared_artifact_rekey_work(0, 1_000);
    let large = shared_artifact_rekey_work(4 * 1024 * 1024, 1_000);
    assert_eq!(
        one_small, one_large,
        "cold snapshot work excludes immutable artifact size"
    );
    assert_eq!(
        small, large,
        "1000 rekeyed actions pay only mutable-field and index work"
    );
    eprintln!("lifecycle1000shared: fixed-field-work={small}, total4MiB-work={large}");
}

#[test]
fn lifecycle_production_envelope_rekeys_twenty_two_distinct_four_mib_actions() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for index in 0..22_u8 {
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(
                format!("distinct_artifact_{index}").parse().unwrap(),
                SpecializedAction::new(
                    Executable::Ivm(IvmBytecode::from_compiled(vec![index; 4 * 1024 * 1024])),
                    Repeats::Exactly(2),
                    ALICE_ID.clone(),
                    ExecuteTriggerEventFilter::new()
                )
                .unwrap()
            ))
            .unwrap()
        );
    }
    tx.enable_lifecycle_journal(
        AllocationBudget::new(32 * 1024 * 1024),
        LifecycleJournalLimits::selected(1_048_576),
    )
    .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 22);
    assert_eq!(
        tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .verified_ivm
            .as_slice()
            .len(),
        22
    );
}

fn distinct_artifact_rekey_work(bytes: usize) -> u64 {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for index in 0..100_u8 {
        // Construct each immutable owner once. This is typed stored fixture
        // preparation, not a registration or lifecycle-activation shortcut.
        let artifact =
            super::super::artifact::ImmutableArtifact::from(IvmBytecode::from_compiled(vec![
                index;
                bytes
            ]));
        let hash = artifact.blob_hash();
        let code_hash = artifact.code_hash();
        let id = insert(
            &mut tx,
            &format!("cold_distinct_{index}"),
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
        tx.by_call_triggers
            .get_mut(&id)
            .unwrap()
            .replace_executable_for_testing(ExecutableRef::Ivm(hash));
        tx.contracts.insert(
            hash,
            IvmBytecodeEntry {
                original_contract: artifact,
                code_hash,
                count: NonZeroU64::MIN,
            },
        );
    }
    let limits = LifecycleJournalLimits::selected(1_048_576);
    tx.enable_lifecycle_journal(AllocationBudget::new(32 * 1024 * 1024), limits)
        .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 100);
    assert_eq!(
        tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .verified_ivm
            .as_slice()
            .len(),
        100
    );
    assert!(
        tx.by_call_triggers
            .iter()
            .all(|(_, action)| action.authority == *BOB_ID)
    );
    limits.encode.max_byte_visits as u64
        - tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .byte_visits
}

#[test]
fn lifecycle_production_envelope_rekeys_one_hundred_distinct_four_mib_actions() {
    let small = distinct_artifact_rekey_work(1);
    let large = distinct_artifact_rekey_work(4 * 1024 * 1024);
    assert!(
        large < 64 * 1024 * 1024,
        "rekey streamed immutable artifacts: {large}"
    );
    // Hashes change the insertion positions in the small sorted tuple index;
    // at most its fixed 72-byte entry shifts can change the measured work.
    assert!(
        small.abs_diff(large) <= 100 * 100 * 72,
        "cold capture work depends on artifact bytes: small={small}, large={large}"
    );
}

#[test]
fn lifecycle_verification_cleanup_cannot_drop_failure_policy_when_work_is_exhausted() {
    let pool = AllocationBudget::new(2_000_000);
    let mut journal = LifecycleJournal::new(pool.clone(), limits(), 2).unwrap();
    let capability = invocation_identity::VerifiedIvm {
        blob: HashOf::new(&IvmBytecode::from_compiled(vec![1, 2, 3])),
        code: Hash::prehashed([7; 32]),
        length: 3,
    };
    journal.remember_ivm(capability).unwrap();
    journal.encode_work = LifecycleWorkBudget::new(0, 0);
    journal.forget_ivm(&capability.blob);
    assert!(journal.verified_ivm.as_slice().is_empty());
    assert!(
        journal.refusal.is_none(),
        "discarding an optimization must not reject reserved policy effects"
    );
    assert_eq!(journal.encode_work.remaining_byte_visits(), 0);
    drop(journal);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn lifecycle_production_envelope_bounds_repeated_uncacheable_snapshot_work() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "large_instruction",
        TriggerLifecycleStateKindV1::ByCall,
        2,
    );
    for index in 0..32 {
        tx.by_call_triggers.get_mut(&id).unwrap().metadata.insert(
            format!("large_mutable_{index}").parse().unwrap(),
            Json::new("x".repeat(32 * 1024)),
        );
    }
    let admission = LifecycleJournalLimits::selected(1_048_576);
    tx.enable_lifecycle_journal(AllocationBudget::new(32 * 1024 * 1024), admission)
        .unwrap();
    let mut snapshots = 0;
    while snapshots < 512 && tx.lifecycle_snapshot(&id).is_some() {
        snapshots += 1;
    }
    assert!(
        snapshots > 1 && snapshots < 512,
        "production root budget did not stop work: {snapshots}"
    );
    assert!(matches!(
        tx.lifecycle_failure(),
        Some(LifecycleJournalFailure::OutputLimit(_))
    ));
    assert!(
        tx.lifecycle_effects().is_empty(),
        "work bound must apply independently of retained effect count"
    );
    assert!(
        tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .verified_ivm
            .as_slice()
            .is_empty()
    );
}

#[test]
fn lifecycle_cached_snapshot_work_is_independent_of_artifact_size() {
    let mut cached_work = None;
    let mut cold_work = None;
    for size in [262_144, 4 * 1024 * 1024] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id: TriggerId = "cached_work_control".parse().unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(
                id.clone(),
                SpecializedAction::new(
                    Executable::Ivm(IvmBytecode::from_compiled(vec![7; size])),
                    Repeats::Exactly(2),
                    ALICE_ID.clone(),
                    ExecuteTriggerEventFilter::new()
                )
                .unwrap()
            ))
            .unwrap()
        );
        let pool = AllocationBudget::new(32 * 1024 * 1024);
        tx.enable_lifecycle_journal(pool.clone(), LifecycleJournalLimits::selected(1_048_576))
            .unwrap();
        let initial = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .remaining_byte_visits();
        let first = tx.lifecycle_snapshot(&id).unwrap();
        let after_first = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .remaining_byte_visits();
        let cached = tx.lifecycle_snapshot(&id).unwrap();
        let cost = after_first
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .remaining_byte_visits();
        let cold = initial - after_first;
        assert!(
            cold >= cost && cold > 0,
            "a cold source pays its small tuple-index admission"
        );
        if let Some(previous) = cold_work {
            assert_eq!(cold, previous, "cold work is independent of artifact size");
        }
        cold_work = Some(cold);
        assert_eq!(first.action.as_bytes(), cached.action.as_bytes());
        assert_eq!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .len(),
            1
        );
        if let Some(previous) = cached_work {
            assert_eq!(
                cost, previous,
                "cached work must not vary with artifact size"
            );
        }
        cached_work = Some(cost);
        eprintln!("lifecycle cached snapshot: artifact-bytes={size}, fixed-field-work={cost}");
        drop(first);
        drop(cached);
        drop(tx);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn lifecycle_ivm_verification_follows_entry_lifetime_and_transaction_rollback() {
    let set = Set::default();
    let mut block = set.block();
    let code = IvmBytecode::from_compiled(vec![7; 262_144]);
    let hash = HashOf::new(&code);
    let first: TriggerId = "cached_first".parse().unwrap();
    let second: TriggerId = "cached_second".parse().unwrap();
    let make = || {
        SpecializedAction::new(
            Executable::Ivm(code.clone()),
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap()
    };
    {
        let mut tx = block.transaction();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(first.clone(), make()))
                .unwrap()
        );
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(second.clone(), make()))
                .unwrap()
        );
        tx.apply();
    }
    let pool = AllocationBudget::new(32 * 1024 * 1024);
    let initial;
    let fresh_cost;
    {
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(pool.clone(), LifecycleJournalLimits::selected(1_048_576))
            .unwrap();
        initial = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .remaining_byte_visits();
        let original = tx.lifecycle_snapshot(&first).unwrap();
        fresh_cost = initial
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .remaining_byte_visits();
        assert!(fresh_cost > 0);
        let cached = tx.lifecycle_snapshot(&second).unwrap();
        assert_eq!(cached.action.as_bytes(), original.action.as_bytes());
        let after_cached = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .remaining_byte_visits();
        let cached_cost = initial - fresh_cost - after_cached;
        assert!(
            fresh_cost > cached_cost,
            "the original root admits its immutable tuple once"
        );
        assert_eq!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .len(),
            1
        );
        assert!(tx.remove(&first));
        assert_eq!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .len(),
            1,
            "reference count changes do not invalidate immutable content"
        );
        assert!(tx.remove(&second));
        assert!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .is_empty()
        );
        assert!(tx.contracts.get(&hash).is_none());
        let before_reinsert = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .remaining_byte_visits();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(first.clone(), make()))
                .unwrap()
        );
        assert_eq!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .len(),
            1
        );
        let reinsert_cost = before_reinsert
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .remaining_byte_visits();
        assert!(
            reinsert_cost > 0 && reinsert_cost <= fresh_cost,
            "new entry identity is checked without replaying artifact bytes"
        );
        // Previously retained before-images are independent of the cache and row.
        assert_eq!(original.action.as_bytes(), cached.action.as_bytes());
        let checkpoint = tx.lifecycle_checkpoint();
        tx.lifecycle_invoke(&first, 0);
        tx.lifecycle_finish_callback(checkpoint, 0, false);
        assert!(!tx.lifecycle_allows_apply());
    }
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(block.by_call_triggers.get(&first).is_some());
    assert!(block.by_call_triggers.get(&second).is_some());
    assert_eq!(block.contracts.get(&hash).unwrap().count.get(), 2);
    let mut tx = block.transaction();
    tx.enable_lifecycle_journal(pool.clone(), LifecycleJournalLimits::selected(1_048_576))
        .unwrap();
    assert!(
        tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .verified_ivm
            .as_slice()
            .is_empty()
    );
    tx.lifecycle_snapshot(&first).unwrap();
    assert_eq!(
        initial
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .remaining_byte_visits(),
        fresh_cost,
        "a new transaction has a fresh index and the same bounded source work after rollback"
    );
}

#[test]
fn lifecycle_ivm_verification_rechecks_current_code_identity_and_reinserted_bytes() {
    for replace_after_remove in [false, true] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let hash = HashOf::new(&code);
        let id: TriggerId = "verification_invalidation".parse().unwrap();
        let action = SpecializedAction::new(
            Executable::Ivm(code.clone()),
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        assert!(
            tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action.clone()))
                .unwrap()
        );
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
            .unwrap();
        tx.lifecycle_snapshot(&id).unwrap();
        if replace_after_remove {
            let mut forged = tx.contracts.get(&hash).unwrap().clone();
            assert!(tx.remove(&id));
            assert!(
                tx.lifecycle_journal
                    .as_ref()
                    .unwrap()
                    .verified_ivm
                    .as_slice()
                    .is_empty()
            );
            forged.original_contract = IvmBytecode::from_compiled(vec![9, 8, 7]).into();
            // Test-only corruption after the real removal seam ended the lifetime.
            tx.contracts.insert(hash, forged);
            assert!(
                tx.add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
                    .unwrap()
            );
        } else {
            tx.contracts.get_mut(&hash).unwrap().code_hash = Hash::prehashed([0x55; 32]);
        }
        let captured = tx.lifecycle_snapshot(&id).unwrap();
        assert_eq!(
            captured.action.as_bytes(),
            lifecycle_unavailable_frame(3).unwrap().as_slice()
        );
        assert!(tx.lifecycle_allows_apply());
    }
}

#[test]
fn lifecycle_stray_sweep_removes_dangling_registration_before_typed_repair() {
    use iroha_data_model::block::execution_output::{
        InvocationCompletionV1, TriggerLifecycleActionFrameV1, TriggerLifecycleChangeV1,
        TriggerLifecycleEffectV1, TriggerLifecycleEffectsV1, TriggerLifecycleOwnerV1,
        TriggerLifecycleStateV1,
    };
    let set = Set::default();
    let mut block = set.block();
    let (shared, callback);
    {
        let mut tx = block.transaction();
        callback = insert(
            &mut tx,
            "sweep_callback",
            TriggerLifecycleStateKindV1::ByCall,
            2,
        );
        shared = insert(
            &mut tx,
            "dangling_and_stray",
            TriggerLifecycleStateKindV1::Time,
            2,
        );
        let stray = insert(
            &mut tx,
            "depleted_template",
            TriggerLifecycleStateKindV1::Data,
            0,
        );
        let duplicate = tx.data_triggers.get(&stray).unwrap().clone();
        assert!(tx.remove(&stray));
        // Original inconsistent storage: the registry's typed row is absent,
        // but a depleted duplicate and both stale active IDs remain.
        tx.time_triggers.remove(shared.clone());
        tx.data_triggers.insert(shared.clone(), duplicate);
        tx.active_data_trigger_ids.insert(shared.clone(), ());
        assert!(tx.active_time_trigger_ids.get(&shared).is_some());
        tx.apply();
    }
    for commit in [false, true] {
        let pool = AllocationBudget::new(2_000_000);
        let mut tx = block.transaction();
        tx.registration_generations.insert(shared.clone(), 7);
        tx.data_trigger_eligibility_generations
            .insert(shared.clone(), 8);
        assert!(
            tx.data_trigger_index
                .postings
                .values()
                .any(|ids| ids.contains(&shared))
        );
        tx.enable_lifecycle_journal(pool.clone(), limits()).unwrap();
        tx.set_lifecycle_origin(Origin::Derived {
            callback_index: Some(0),
        });
        tx.lifecycle_invoke(&callback, 0);
        tx.set_lifecycle_origin(Origin::InvocationPolicy {
            callback_index: 0,
            policy: Policy::RemoveDepleted,
        });
        assert_eq!(
            tx.decrease_repeats(std::iter::empty()),
            vec![shared.clone()],
            "the existing State permission-cleanup caller receives this ID exactly once"
        );
        assert!(tx.ids.get(&shared).is_none());
        assert!(tx.data_triggers.get(&shared).is_none());
        assert!(tx.time_triggers.get(&shared).is_none());
        assert!(tx.active_data_trigger_ids.get(&shared).is_none());
        assert!(tx.active_time_trigger_ids.get(&shared).is_none());
        assert!(!tx.registration_generations.contains_key(&shared));
        assert!(
            !tx.data_trigger_eligibility_generations
                .contains_key(&shared)
        );
        assert!(
            !tx.data_trigger_index
                .postings
                .values()
                .any(|ids| ids.contains(&shared))
        );
        assert!(tx.by_call_triggers.get(&callback).is_some());
        assert!(tx.decrease_repeats(std::iter::empty()).is_empty());
        assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
        assert_eq!(tx.lifecycle_effects().len(), 3);
        for (effect, kind, reason) in [
            (
                &tx.lifecycle_effects()[1],
                TriggerLifecycleStateKindV1::Time,
                2,
            ),
            (
                &tx.lifecycle_effects()[2],
                TriggerLifecycleStateKindV1::Data,
                4,
            ),
        ] {
            assert_eq!(effect.trigger_id.id, shared);
            let LifecycleChange::Remove(before) = &effect.change else {
                panic!("registry removal and typed repair must be separate removals");
            };
            assert_eq!(before.kind, kind);
            assert_eq!(
                before.action.as_bytes(),
                lifecycle_unavailable_frame(reason).unwrap().as_slice()
            );
        }
        let state = |value: &LifecycleSnapshot| TriggerLifecycleStateV1 {
            kind: value.kind,
            registered_at_height: value.registered_at_height,
            action_hash: value.action_hash,
            retry_state: value.retry_state,
            action: TriggerLifecycleActionFrameV1 {
                codec: value.codec,
                bytes: value.action.as_bytes().to_vec(),
            },
        };
        let trace = TriggerLifecycleEffectsV1::Complete {
            effects: tx
                .lifecycle_effects()
                .iter()
                .map(|effect| TriggerLifecycleEffectV1 {
                    ordinal: effect.ordinal,
                    trigger_id: effect.trigger_id.id.clone(),
                    origin: effect.origin,
                    change: match &effect.change {
                        LifecycleChange::Invoke(value, callback_index) => {
                            TriggerLifecycleChangeV1::Invoke {
                                state: state(value),
                                callback_index: *callback_index,
                            }
                        }
                        LifecycleChange::Remove(before) => TriggerLifecycleChangeV1::Remove {
                            before: state(before),
                        },
                        _ => unreachable!("fixture only invokes and removes"),
                    },
                })
                .collect(),
        };
        trace
            .validate_structure(
                TriggerLifecycleOwnerV1::Network,
                2,
                &[InvocationCompletionV1 {
                    callback_index: 0,
                    trigger_id: callback.clone(),
                    outcome: TriggerCompletedOutcome::Success,
                }],
                false,
            )
            .unwrap();
        if commit {
            tx.apply();
        } else {
            drop(tx);
        }
        assert_eq!(pool.reserved_bytes(), 0, "journal custody is refunded");
        assert_eq!(block.ids.get(&shared).is_none(), commit);
        assert_eq!(block.data_triggers.get(&shared).is_none(), commit);
        assert_eq!(block.active_data_trigger_ids.get(&shared).is_none(), commit);
        assert_eq!(block.active_time_trigger_ids.get(&shared).is_none(), commit);
        assert!(block.time_triggers.get(&shared).is_none());
        assert!(block.by_call_triggers.get(&callback).is_some());
    }
}

#[test]
fn lifecycle_stray_sweep_preserves_live_registration_and_rolls_back_with_root() {
    let set = Set::default();
    let mut block = set.block();
    let shared;
    {
        let mut tx = block.transaction();
        shared = insert(
            &mut tx,
            "live_and_stray",
            TriggerLifecycleStateKindV1::Time,
            2,
        );
        let stray = insert(
            &mut tx,
            "stray_template",
            TriggerLifecycleStateKindV1::Data,
            0,
        );
        let duplicate = tx.data_triggers.get(&stray).unwrap().clone();
        assert!(tx.remove(&stray));
        tx.data_triggers.insert(shared.clone(), duplicate);
        tx.apply();
    }
    for commit in [false, true] {
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
            .unwrap();
        let generation = tx.registration_generations.get(&shared).copied();
        let snapshot = tx.lifecycle_snapshot(&shared).unwrap();
        let root = iroha_data_model::block::execution_output::TriggerUseV1 {
            trigger_id: shared.clone(),
            registered_at_height: snapshot.registered_at_height.unwrap(),
            action_hash: snapshot.action_hash.unwrap(),
        };
        tx.set_lifecycle_origin(Origin::Derived {
            callback_index: Some(0),
        });
        tx.lifecycle_invoke(&shared, 0);
        assert_eq!(tx.lifecycle_effects().len(), 1);
        tx.set_lifecycle_origin(Origin::InvocationPolicy {
            callback_index: 0,
            policy: Policy::RemoveDepleted,
        });
        assert!(
            tx.decrease_repeats(std::iter::empty()).is_empty(),
            "a live registration must keep its permissions"
        );
        assert!(tx.data_triggers.is_empty());
        assert!(tx.time_triggers.get(&shared).is_some());
        assert_eq!(tx.ids.get(&shared), Some(&TriggeringEventType::Time));
        assert_eq!(
            tx.registration_generations.get(&shared).copied(),
            generation
        );
        assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
        assert_eq!(tx.lifecycle_effects().len(), 2);
        // The live Time before-image still follows the pre-repair Invoke.
        tx.set_lifecycle_origin(Origin::InvocationPolicy {
            callback_index: 0,
            policy: Policy::ConsumeRepeat,
        });
        assert!(tx.decrease_repeats(std::iter::once(&shared)).is_empty());
        assert_eq!(
            tx.time_triggers.get(&shared).unwrap().repeats,
            Repeats::Exactly(1)
        );
        use iroha_data_model::block::execution_output::{
            InvocationCompletionV1, TriggerLifecycleActionFrameV1, TriggerLifecycleChangeV1,
            TriggerLifecycleEffectV1, TriggerLifecycleEffectsV1, TriggerLifecycleOwnerV1,
            TriggerLifecycleStateV1,
        };
        let state = |value: &LifecycleSnapshot| TriggerLifecycleStateV1 {
            kind: value.kind,
            registered_at_height: value.registered_at_height,
            action_hash: value.action_hash,
            retry_state: value.retry_state,
            action: TriggerLifecycleActionFrameV1 {
                codec: value.codec,
                bytes: value.action.as_bytes().to_vec(),
            },
        };
        let trace = TriggerLifecycleEffectsV1::Complete {
            effects: tx
                .lifecycle_effects()
                .iter()
                .map(|effect| TriggerLifecycleEffectV1 {
                    ordinal: effect.ordinal,
                    trigger_id: effect.trigger_id.id.clone(),
                    origin: effect.origin,
                    change: match &effect.change {
                        LifecycleChange::Insert(after) => TriggerLifecycleChangeV1::Insert {
                            after: state(after),
                        },
                        LifecycleChange::Update(before, after) => {
                            TriggerLifecycleChangeV1::Update {
                                before: state(before),
                                after: state(after),
                            }
                        }
                        LifecycleChange::Remove(before) => TriggerLifecycleChangeV1::Remove {
                            before: state(before),
                        },
                        LifecycleChange::Invoke(value, callback_index) => {
                            TriggerLifecycleChangeV1::Invoke {
                                state: state(value),
                                callback_index: *callback_index,
                            }
                        }
                    },
                })
                .collect(),
        };
        trace
            .validate_structure(
                TriggerLifecycleOwnerV1::Time(&root),
                2,
                &[InvocationCompletionV1 {
                    callback_index: 0,
                    trigger_id: shared.clone(),
                    outcome: TriggerCompletedOutcome::Success,
                }],
                false,
            )
            .unwrap();
        if commit {
            tx.apply();
        } else {
            drop(tx);
        }
        assert_eq!(block.data_triggers.get(&shared).is_none(), commit);
        assert_eq!(
            block.time_triggers.get(&shared).unwrap().repeats,
            Repeats::Exactly(if commit { 1 } else { 2 })
        );
        assert_eq!(block.ids.get(&shared), Some(&TriggeringEventType::Time));
    }
}

#[test]
fn lifecycle_stray_ivm_repair_keeps_the_live_artifact_with_counted_or_uncounted_duplicates() {
    for count in [1, 2] {
        let set = Set::default();
        let mut block = set.block();
        let shared: TriggerId = "live_ivm_with_stray".parse().unwrap();
        let code = IvmBytecode::from_compiled(vec![1, 2, 3]);
        let blob = HashOf::new(&code);
        {
            let mut tx = block.transaction();
            let mut action = SpecializedAction::new(
                Executable::Ivm(code),
                Repeats::Exactly(2),
                ALICE_ID.clone(),
                TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
                    Duration::from_millis(10),
                ))),
            )
            .unwrap();
            action.metadata.insert(
                "__registered_block_height".parse().unwrap(),
                Json::from(1_u64),
            );
            assert!(
                tx.add_time_trigger(SpecializedTrigger::new(shared.clone(), action))
                    .unwrap()
            );
            let stray = insert(
                &mut tx,
                "ivm_stray_template",
                TriggerLifecycleStateKindV1::Data,
                0,
            );
            let mut duplicate = tx.data_triggers.get(&stray).unwrap().clone();
            assert!(tx.remove(&stray));
            duplicate.replace_executable_for_testing(ExecutableRef::Ivm(blob));
            tx.data_triggers.insert(shared.clone(), duplicate);
            tx.contracts.get_mut(&blob).unwrap().count = NonZeroU64::new(count).unwrap();
            tx.apply();
        }
        for commit in [false, true] {
            let mut tx = block.transaction();
            tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
                .unwrap();
            let expected = invocation_identity::time_trigger_use_v1(&tx, &shared, 2).unwrap();
            tx.set_lifecycle_origin(Origin::Derived {
                callback_index: Some(0),
            });
            tx.lifecycle_invoke(&shared, 0);
            assert_eq!(
                tx.lifecycle_journal
                    .as_ref()
                    .unwrap()
                    .verified_ivm
                    .as_slice()
                    .len(),
                1
            );
            tx.set_lifecycle_origin(Origin::InvocationPolicy {
                callback_index: 0,
                policy: Policy::RemoveDepleted,
            });
            assert!(tx.decrease_repeats(std::iter::empty()).is_empty());
            assert!(tx.data_triggers.get(&shared).is_none());
            assert_eq!(tx.ids.get(&shared), Some(&TriggeringEventType::Time));
            assert_eq!(tx.contracts.get(&blob).unwrap().count, NonZeroU64::MIN);
            assert_eq!(
                tx.lifecycle_journal
                    .as_ref()
                    .unwrap()
                    .verified_ivm
                    .as_slice()
                    .len(),
                1,
                "live artifact custody did not end"
            );
            assert_eq!(
                invocation_identity::time_trigger_use_v1(&tx, &shared, 2).unwrap(),
                expected,
                "repair cannot silently change the live persistent action identity"
            );
            assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
            assert_eq!(tx.lifecycle_effects().len(), 2);
            if commit {
                tx.apply();
            } else {
                drop(tx);
            }
            assert_eq!(block.data_triggers.get(&shared).is_none(), commit);
            assert_eq!(
                block.contracts.get(&blob).unwrap().count.get(),
                if commit { 1 } else { count }
            );
        }
        // A later genuine retirement still drops the actual last reference.
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits())
            .unwrap();
        assert!(tx.remove(&shared));
        assert!(tx.contracts.get(&blob).is_none());
        assert!(
            tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .verified_ivm
                .as_slice()
                .is_empty()
        );
        assert!(tx.lifecycle_allows_apply());
        tx.apply();
    }
}

#[test]
fn lifecycle_production_envelope_accepts_4095_minimal_updates() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "minimal_updates",
        TriggerLifecycleStateKindV1::ByCall,
        1,
    );
    tx.enable_lifecycle_journal(
        AllocationBudget::new(32 * 1024 * 1024),
        LifecycleJournalLimits::selected(1_048_576),
    )
    .unwrap();
    for _ in 0..4095 {
        tx.mod_repeats(&id, |n| Ok(n + 1)).unwrap();
    }
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 4095);
}

#[test]
fn lifecycle_production_envelope_rekeys_thirteen_ten_mib_inline_actions() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    for index in 0..13 {
        let id = insert(
            &mut tx,
            &format!("large_inline_{index}"),
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
        tx.by_call_triggers
            .get_mut(&id)
            .unwrap()
            .replace_executable_for_testing(ExecutableRef::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "x".repeat(10 * 1024 * 1024),
                ))]
                .into(),
            ));
    }
    tx.enable_lifecycle_journal(
        AllocationBudget::new(32 * 1024 * 1024),
        LifecycleJournalLimits::selected(1_048_576),
    )
    .unwrap();
    tx.replace_account_id(&ALICE_ID, &BOB_ID).unwrap();
    assert!(tx.lifecycle_allows_apply(), "{:?}", tx.lifecycle_failure());
    assert_eq!(tx.lifecycle_effects().len(), 13);
}

#[test]
fn lifecycle_inline_executable_is_hashed_once_per_entry_and_reinsert_is_fresh() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(
        &mut tx,
        "inline_cached",
        TriggerLifecycleStateKindV1::ByCall,
        1,
    );
    tx.by_call_triggers
        .get_mut(&id)
        .unwrap()
        .replace_executable_for_testing(ExecutableRef::Instructions(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "x".repeat(1024 * 1024),
            ))]
            .into(),
        ));
    tx.enable_lifecycle_journal(
        AllocationBudget::new(32 * 1024 * 1024),
        LifecycleJournalLimits::selected(1_048_576),
    )
    .unwrap();
    let first = tx.lifecycle_snapshot(&id).unwrap();
    let remaining = tx
        .lifecycle_journal
        .as_ref()
        .unwrap()
        .encode_work
        .byte_visits;
    for _ in 0..127 {
        tx.mod_repeats(&id, |n| Ok(n + 1)).unwrap();
    }
    let used = remaining
        - tx.lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .byte_visits;
    assert!(
        used < 16 * 1024 * 1024,
        "immutable executable was rehashed: {used}"
    );
    assert!(tx.lifecycle_allows_apply());
    assert!(tx.remove(&id));
    insert(
        &mut tx,
        "inline_cached",
        TriggerLifecycleStateKindV1::ByCall,
        1,
    );
    assert_ne!(
        first.action.as_bytes(),
        tx.lifecycle_snapshot(&id).unwrap().action.as_bytes()
    );
}

#[test]
fn lifecycle_block_work_is_nonrefunded_across_rejected_roots_and_resets_next_block() {
    let set = Set::default();
    let mut block = set.block();
    let id;
    {
        let mut tx = block.transaction();
        id = insert(
            &mut tx,
            "block_work",
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
        for index in 0..32 {
            tx.by_call_triggers.get_mut(&id).unwrap().metadata.insert(
                format!("mutable_{index}").parse().unwrap(),
                Json::new("x".repeat(32 * 1024)),
            );
        }
        tx.apply();
    }
    let initial = block.lifecycle_work.get().byte_visits;
    let mut spent = 0;
    for _ in 0..512 {
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(
            AllocationBudget::new(32 * 1024 * 1024),
            LifecycleJournalLimits::selected(1_048_576),
        )
        .unwrap();
        if tx.lifecycle_snapshot(&id).is_none() {
            assert!(matches!(
                tx.lifecycle_failure(),
                Some(LifecycleJournalFailure::OutputLimit(_))
            ));
            break;
        }
        spent += 1;
        // Intentionally abort every root; the block's work allowance must not reset.
    }
    assert!(
        spent > 1 && spent < 512,
        "block allowance was refunded: {spent}"
    );
    assert!(block.lifecycle_work.get().byte_visits < initial);
    drop(block);
    assert_eq!(set.block().lifecycle_work.get().byte_visits, initial);
}

#[test]
fn lifecycle_empty_journals_and_1024_minimal_roots_fit_shared_block_work() {
    let set = Set::default();
    let mut block = set.block();
    let id;
    {
        let mut tx = block.transaction();
        id = insert(
            &mut tx,
            "small_roots",
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
        tx.apply();
    }
    let initial = block.lifecycle_work.get().byte_visits;
    let mut cold_cost = None;
    for _ in 0..1024 {
        let before = block.lifecycle_work.get().byte_visits;
        let mut tx = block.transaction();
        tx.enable_lifecycle_journal(
            AllocationBudget::new(32 * 1024 * 1024),
            LifecycleJournalLimits::selected(1_048_576),
        )
        .unwrap();
        assert_eq!(
            tx.lifecycle_block_work.get().byte_visits,
            before,
            "an empty row must not initialize a per-executable hash table"
        );
        tx.lifecycle_snapshot(&id).unwrap();
        let cost = before - tx.lifecycle_block_work.get().byte_visits;
        assert!(cost > 0 && cost < 128 * 1024, "minimal source work: {cost}");
        if let Some(previous) = cold_cost {
            assert_eq!(previous, cost);
        }
        cold_cost = Some(cost);
    }
    assert_eq!(
        initial - block.lifecycle_work.get().byte_visits,
        1024 * cold_cost.unwrap()
    );
}

#[test]
fn lifecycle_zero_source_budget_keeps_reserved_retry_policy() {
    let set = Set::default();
    let mut block = set.block();
    let mut tx = block.transaction();
    let id = insert(&mut tx, "lazy_policy", TriggerLifecycleStateKindV1::Time, 1);
    let mut admission = limits();
    admission.encode.max_byte_visits = 0;
    tx.enable_lifecycle_journal(AllocationBudget::new(2_000_000), admission)
        .unwrap();
    tx.set_lifecycle_origin(Origin::InvocationPolicy {
        callback_index: 0,
        policy: Policy::AdvanceTimeRetry,
    });
    let snapshot = tx.lifecycle_snapshot(&id).unwrap();
    assert_eq!(
        snapshot.action.as_bytes(),
        lifecycle_unavailable_frame(5).unwrap().as_slice()
    );
    assert!(tx.lifecycle_failure().is_none());
}

#[test]
fn lifecycle_missing_inline_binding_never_streams_the_executable_as_a_fallback() {
    let mut previous_work = None;
    for bytes in [1, 10 * 1024 * 1024] {
        let set = Set::default();
        let mut block = set.block();
        let mut tx = block.transaction();
        let id = insert(
            &mut tx,
            "unavailable_inline",
            TriggerLifecycleStateKindV1::ByCall,
            1,
        );
        let action = tx.by_call_triggers.get_mut(&id).unwrap();
        action.replace_executable_for_testing(ExecutableRef::Instructions(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "x".repeat(bytes),
            ))]
            .into(),
        ));
        assert!(action.lifecycle_executable_digest.is_some());
        // Simulate a deterministic source-encoder refusal recorded on restore.
        // There is deliberately no full-body retry in the selected capture path.
        action.lifecycle_executable_digest = None;
        let limits = LifecycleJournalLimits::selected(1_048_576);
        tx.enable_lifecycle_journal(AllocationBudget::new(32 * 1024 * 1024), limits)
            .unwrap();
        let before = tx
            .lifecycle_journal
            .as_ref()
            .unwrap()
            .encode_work
            .byte_visits;
        let snapshot = tx.lifecycle_snapshot(&id).unwrap();
        assert_eq!(
            snapshot.action.as_bytes(),
            lifecycle_unavailable_frame(3).unwrap().as_slice()
        );
        assert!(snapshot.action_hash.is_none() && snapshot.registered_at_height.is_none());
        let work = before
            - tx.lifecycle_journal
                .as_ref()
                .unwrap()
                .encode_work
                .byte_visits;
        if let Some(previous) = previous_work {
            assert_eq!(work, previous);
        }
        previous_work = Some(work);
        assert!(tx.lifecycle_allows_apply());
    }
}
