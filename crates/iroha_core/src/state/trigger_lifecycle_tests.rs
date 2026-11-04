//! Dormant lifecycle capture through real State transactions and callback dispatch.

use super::*;
use crate::smartcontracts::{
    Execute,
    isi::triggers::{
        set::{
            SetReadOnly,
            lifecycle_journal::{LifecycleChange, LifecycleJournalLimits},
        },
        specialized::{SpecializedAction, SpecializedTrigger},
    },
};
use iroha_allocation::AllocationBudget;
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use mv::storage::StorageReadOnly;

fn limits(effects: usize) -> LifecycleJournalLimits {
    LifecycleJournalLimits {
        effects,
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

fn fixture() -> State {
    let world = World::with(
        [],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [],
    );
    crate::state::tests::authenticate_trigger_fixture(State::new(
        world,
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    ))
}

fn header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 42, 0)
}

fn insert(
    tx: &mut StateTransaction<'_, '_>,
    name: &str,
    authority: AccountId,
    instructions: Vec<InstructionBox>,
) -> TriggerId {
    let id: TriggerId = name.parse().unwrap();
    let action = SpecializedAction::new(
        Executable::Instructions(instructions.into()),
        Repeats::Exactly(2),
        authority,
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    assert!(
        tx.world
            .triggers
            .add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap()
    );
    id
}

#[test]
fn lifecycle_state_origin_scopes_restore_after_nested_error_and_unwind() {
    let state = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    let root = Origin::DirectNetworkInstruction { item_index: 1 };
    tx.with_trigger_lifecycle_origin(root, |tx| {
        assert_eq!(tx.world.triggers.lifecycle_origin(), Some(root));
        let result: Result<(), ()> = tx.with_trigger_lifecycle_origin(
            Origin::Derived {
                callback_index: Some(3),
            },
            |tx| {
                assert_eq!(
                    tx.derived_trigger_lifecycle_origin(),
                    Origin::Derived {
                        callback_index: Some(3)
                    }
                );
                Err(())
            },
        );
        assert!(result.is_err());
        assert_eq!(tx.world.triggers.lifecycle_origin(), Some(root));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tx.with_trigger_lifecycle_policy(Some(4), Policy::AdvanceTimeRetry, |_| {
                    panic!("fixture unwind")
                });
            }))
            .is_err()
        );
        assert_eq!(tx.world.triggers.lifecycle_origin(), Some(root));
    });
    assert_eq!(
        tx.world.triggers.lifecycle_origin(),
        Some(Origin::Derived {
            callback_index: None
        })
    );
    assert!(!tx.world.triggers.lifecycle_allows_apply());
}

#[test]
fn lifecycle_state_callback_invoke_precedes_body_and_uses_ticket_not_display_index() {
    let state = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction_for_callback_testing();
    let id = insert(&mut tx, "invoke", ALICE_ID.clone(), vec![]);
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    let executable = tx
        .world
        .triggers
        .by_call_triggers()
        .get(&id)
        .unwrap()
        .executable()
        .clone();
    let event = ExecuteTriggerEvent {
        trigger_id: id.clone(),
        authority: ALICE_ID.clone(),
        args: Json::default(),
    };
    tx.with_trigger_lifecycle_origin(Origin::DirectNetworkInstruction { item_index: 8 }, |tx| {
        tx.execute_trigger(&id, &ALICE_ID, &executable, event.clone().into(), 97, None)
            .unwrap();
    });
    let effect = &tx.world.triggers.lifecycle_effects()[0];
    assert_eq!(
        effect.origin,
        Origin::Derived {
            callback_index: Some(0)
        }
    );
    assert!(matches!(effect.change, LifecycleChange::Invoke(_, 0)));
    assert_eq!(tx.world.triggers.lifecycle_last_callback(), Some(0));
    tx.decrease_trigger_repeats_and_cleanup(&id, Some(0));
    assert_eq!(
        tx.world.triggers.lifecycle_effects()[1].origin,
        Origin::InvocationPolicy {
            callback_index: 0,
            policy: Policy::ConsumeRepeat
        }
    );
}

#[test]
fn lifecycle_state_nested_callbacks_preserve_outer_policy_ordinal() {
    let state = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction_for_callback_testing();
    let inner = insert(&mut tx, "inner", ALICE_ID.clone(), vec![]);
    let outer = insert(
        &mut tx,
        "outer",
        ALICE_ID.clone(),
        vec![ExecuteTrigger::new(inner.clone()).into()],
    );
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    tx.execute_called_trigger(
        &outer,
        &ExecuteTriggerEvent {
            trigger_id: outer.clone(),
            authority: ALICE_ID.clone(),
            args: Json::default(),
        },
    )
    .unwrap();
    let effects = tx.world.triggers.lifecycle_effects();
    assert_eq!(effects.len(), 4);
    assert_eq!(effects[0].trigger_id.id, outer);
    assert!(matches!(effects[0].change, LifecycleChange::Invoke(_, 0)));
    assert_eq!(effects[1].trigger_id.id, inner);
    assert!(matches!(effects[1].change, LifecycleChange::Invoke(_, 1)));
    assert_eq!(
        effects[2].origin,
        Origin::InvocationPolicy {
            callback_index: 1,
            policy: Policy::ConsumeRepeat
        }
    );
    assert_eq!(
        effects[3].origin,
        Origin::InvocationPolicy {
            callback_index: 0,
            policy: Policy::ConsumeRepeat
        }
    );
}

#[test]
fn lifecycle_state_failed_callback_drops_invoke_and_body_and_transaction_rolls_back() {
    let state = fixture();
    let mut block = state.block(header());
    let id = {
        let mut tx = block.transaction();
        let id: TriggerId = "failed".parse().unwrap();
        let action = SpecializedAction::new(
            Executable::Batch(ConstVec::new_empty()),
            Repeats::Exactly(2),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        tx.world
            .triggers
            .add_by_call_trigger(SpecializedTrigger::new(id.clone(), action))
            .unwrap();
        tx.apply();
        id
    };
    let pool = AllocationBudget::new(2_000_000);
    {
        let mut tx = block.transaction_for_callback_testing();
        tx.world
            .triggers
            .enable_lifecycle_journal(pool.clone(), limits(32))
            .unwrap();
        // An empty callback batch fails after its pre-body Invoke.
        let executable = tx
            .world
            .triggers
            .by_call_triggers()
            .get(&id)
            .unwrap()
            .executable()
            .clone();
        let event = ExecuteTriggerEvent {
            trigger_id: id.clone(),
            authority: ALICE_ID.clone(),
            args: Json::default(),
        };
        assert!(
            tx.execute_trigger(&id, &ALICE_ID, &executable, event.into(), 44, None)
                .is_err()
        );
        assert!(tx.world.triggers.lifecycle_effects().is_empty());
        tx.world.triggers.remove(&id);
        // Existing callback failure guard also refuses application of later effects.
        tx.apply();
    }
    assert!(block.world.triggers.by_call_triggers().get(&id).is_some());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn lifecycle_state_refused_capture_rolls_back_other_world_fields_too() {
    let state = fixture();
    let mut block = state.block(header());
    {
        let mut tx = block.transaction();
        tx.world
            .triggers
            .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(0))
            .unwrap();
        SetKeyValue::account(
            ALICE_ID.clone(),
            "rollback".parse().unwrap(),
            Json::from(true),
        )
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        insert(&mut tx, "refused", ALICE_ID.clone(), vec![]);
        tx.apply();
    }
    assert!(
        block
            .world
            .triggers
            .ids()
            .get(&"refused".parse().unwrap())
            .is_none()
    );
    let account = block.world.accounts.get(&ALICE_ID).unwrap();
    assert!(account.metadata().get("rollback").is_none());
}

#[test]
fn lifecycle_state_implicit_account_retirement_reaches_removal_seam() {
    let state = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    let id = insert(&mut tx, "retire_owner", BOB_ID.clone(), vec![]);
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    tx.with_trigger_lifecycle_origin(Origin::DirectNetworkInstruction { item_index: 2 }, |tx| {
        Unregister::account(BOB_ID.clone())
            .execute(&ALICE_ID, tx)
            .unwrap();
    });
    assert_eq!(tx.world.triggers.lifecycle_effects().len(), 1);
    let effect = &tx.world.triggers.lifecycle_effects()[0];
    assert_eq!(effect.trigger_id.id, id);
    assert!(matches!(effect.change, LifecycleChange::Remove(_)));
    assert_eq!(
        effect.origin,
        Origin::DirectNetworkInstruction { item_index: 2 }
    );
}

#[test]
fn lifecycle_state_direct_origin_requires_original_signed_item_and_root() {
    let state = fixture();
    let mut block = state.block(header());
    let mut tx = block.transaction();
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    let instructions = vec![
        InstructionBox::from(Log::new(Level::INFO, "first".into())),
        InstructionBox::from(Log::new(Level::INFO, "second".into())),
    ];
    let signed = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions(instructions.clone())
    .sign(ALICE_KEYPAIR.private_key());
    let Executable::Instructions(original) = signed.instructions() else {
        panic!("signed Instructions");
    };
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(signed.hash());
    tx.current_entrypoint_index = Some(0);
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&signed, &original[1], 1, true),
        Origin::DirectNetworkInstruction { item_index: 1 }
    );
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&signed, &original[0], 1, true),
        Origin::Derived {
            callback_index: None
        }
    );
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&signed, &original[1], 1, false),
        Origin::Derived {
            callback_index: None
        }
    );
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&signed, &instructions[1], 1, true),
        Origin::Derived {
            callback_index: None
        },
        "an equal synthesized instruction is not the original signed slot",
    );
    tx.tx_call_hash = Some(Hash::new(b"foreign"));
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&signed, &original[1], 1, true),
        Origin::Derived {
            callback_index: None
        }
    );

    let address = iroha_data_model::smart_contract::ContractAddress::derive(
        &state.network_id,
        &ALICE_ID,
        1,
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    let batch = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_executable(Executable::Batch(
        vec![
            iroha_data_model::transaction::ExecutableBatchItem::ContractCall(
                iroha_data_model::transaction::executable::ContractInvocation {
                    contract_address: address,
                    expected_code_hash: Hash::new(b"call"),
                    entrypoint: "run".into(),
                    arguments: None,
                },
            ),
            iroha_data_model::transaction::ExecutableBatchItem::Instruction(
                instructions[1].clone(),
            ),
        ]
        .into(),
    ))
    .sign(ALICE_KEYPAIR.private_key());
    let Executable::Batch(batch_items) = batch.instructions() else {
        panic!("signed Batch");
    };
    let iroha_data_model::transaction::ExecutableBatchItem::Instruction(batch_instruction) =
        &batch_items[1]
    else {
        panic!("original Batch instruction");
    };
    tx.current_network_entrypoint_hash = Some(batch.hash_as_entrypoint());
    tx.tx_call_hash = Some(Hash::from(batch.hash_as_entrypoint()));
    tx.current_tx_hash = Some(batch.hash());
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&batch, batch_instruction, 1, true),
        Origin::DirectNetworkInstruction { item_index: 1 }
    );
    assert_eq!(
        tx.direct_trigger_lifecycle_origin(&batch, batch_instruction, 0, true),
        Origin::Derived {
            callback_index: None
        }
    );
}

#[test]
fn lifecycle_executor_applies_create_enabled_and_remove_only_when_executed() {
    let state = fixture();
    let mut block = state.block(header());
    // Bind the real signed-root budget required by the existing artifact apply
    // boundary. Host syscalls are driven directly below to isolate this seam.
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let signed = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], NonZeroU64::new(1_000_000)),
    )
    .with_executable(Executable::Ivm(IvmBytecode::from_compiled(program)))
    .sign(ALICE_KEYPAIR.private_key());
    let mut tx = block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(signed.hash());
    tx.begin_execution_effect_budget(&signed).unwrap();
    tx.world
        .triggers
        .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
        .unwrap();
    let id: TriggerId = "host_effects".parse().unwrap();
    let action = Action::new(
        Vec::<InstructionBox>::new(),
        Repeats::Exactly(2),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .unwrap();
    use ivm::{host::IVMHost, pointer_abi::PointerType};
    let mut vm = ivm::IVM::new(10_000);
    let mut host = crate::smartcontracts::ivm::host::CoreHost::new(ALICE_ID.clone());
    let trigger = Trigger::new(id.clone(), action);
    let payload = norito::to_bytes(&Json::new(trigger)).unwrap();
    let pointer = vm
        .alloc_host_tlv(&crate::state::tests::make_tlv(PointerType::Json, &payload))
        .unwrap();
    vm.set_register(10, pointer);
    host.syscall(ivm::syscalls::SYSCALL_CREATE_TRIGGER, &mut vm)
        .unwrap();
    let name = norito::to_bytes(id.name()).unwrap();
    let pointer = vm
        .alloc_host_tlv(&crate::state::tests::make_tlv(PointerType::Name, &name))
        .unwrap();
    vm.set_register(10, pointer);
    vm.set_register(11, 0);
    host.syscall(ivm::syscalls::SYSCALL_SET_TRIGGER_ENABLED, &mut vm)
        .unwrap();
    vm.set_register(10, pointer);
    host.syscall(ivm::syscalls::SYSCALL_REMOVE_TRIGGER, &mut vm)
        .unwrap();
    // Actual syscalls queue effects; only the executor's applied mutations count.
    assert!(tx.world.triggers.lifecycle_effects().is_empty());
    assert!(tx.world.triggers.ids().get(&id).is_none());
    host.apply_queued(&mut tx, &ALICE_ID).unwrap();
    let effects = tx.world.triggers.lifecycle_effects();
    assert_eq!(effects.len(), 3);
    assert!(matches!(effects[0].change, LifecycleChange::Insert(_)));
    assert!(matches!(effects[1].change, LifecycleChange::Update(_, _)));
    assert!(matches!(effects[2].change, LifecycleChange::Remove(_)));
    assert!(effects.iter().all(|effect| effect.origin
        == Origin::Derived {
            callback_index: None
        }));
}

#[test]
fn lifecycle_callback_failure_removes_already_executed_business_effects_from_trace() {
    let state = fixture();
    let mut block = state.block(header());
    let (target, failing) = {
        let mut tx = block.transaction();
        let target = insert(&mut tx, "callback_target", ALICE_ID.clone(), vec![]);
        let failing = insert(
            &mut tx,
            "callback_failing",
            ALICE_ID.clone(),
            vec![
                SetKeyValue::trigger(target.clone(), "written".parse().unwrap(), Json::from(true))
                    .into(),
                Unregister::trigger("missing_trigger".parse().unwrap()).into(),
            ],
        );
        tx.apply();
        (target, failing)
    };
    {
        let mut tx = block.transaction_for_callback_testing();
        tx.world
            .triggers
            .enable_lifecycle_journal(AllocationBudget::new(2_000_000), limits(32))
            .unwrap();
        assert!(
            tx.execute_called_trigger(
                &failing,
                &ExecuteTriggerEvent {
                    trigger_id: failing.clone(),
                    authority: ALICE_ID.clone(),
                    args: Json::default()
                }
            )
            .is_err()
        );
        assert!(tx.world.triggers.lifecycle_effects().is_empty());
        // The State transaction is the existing business rollback owner.
        tx.apply();
    }
    assert!(
        block
            .world
            .triggers
            .by_call_triggers()
            .get(&target)
            .unwrap()
            .metadata()
            .get("written")
            .is_none()
    );
}

#[test]
fn lifecycle_compact_wire_identity_and_unchanged_ivm_abi() {
    // Compact frames live in existing opaque bytes under a selected service;
    // the original covered wire graph and baseline identities stay unchanged.
    let wire = crate::release_identity::wire_schema_hash();
    let abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    println!("wire_schema_hash={}", iroha_crypto::Hash::prehashed(wire));
    println!("ivm_abi_hash={}", iroha_crypto::Hash::prehashed(abi));
    assert_eq!(
        wire,
        [
            0x8c, 0x6e, 0x53, 0x80, 0x04, 0xb1, 0x5b, 0x52, 0xa6, 0xb6, 0xca, 0x43, 0x0b, 0x5c,
            0x9d, 0x6c, 0x32, 0xa8, 0x79, 0x8e, 0x03, 0x9a, 0x70, 0x3b, 0x79, 0x8d, 0x71, 0x20,
            0x4c, 0xf0, 0x8d, 0xf3
        ]
    );
    assert_eq!(
        abi,
        [
            0x75, 0x31, 0xb4, 0x5d, 0xd9, 0xd7, 0x9a, 0xab, 0xc3, 0xa0, 0xf6, 0x4a, 0xb3, 0x0a,
            0x05, 0x3a, 0xf8, 0x47, 0xdf, 0xe2, 0x48, 0x07, 0x8c, 0xff, 0x83, 0x9a, 0xb2, 0xe1,
            0x4e, 0x32, 0x61, 0xa7
        ]
    );
}

#[test]
fn lifecycle_raw_world_apply_cannot_publish_a_refused_capture() {
    let state = fixture();
    let mut block = state.block(header());
    let pool = AllocationBudget::new(2_000_000);
    let mut world = block
        .world
        .transaction_without_telemetry(LaneConfig::default(), 0);
    world
        .triggers
        .enable_lifecycle_journal(pool.clone(), limits(32))
        .unwrap();
    world
        .account_mut(&ALICE_ID)
        .unwrap()
        .insert("unpublished".parse().unwrap(), Json::from(true));
    world.triggers.refuse_lifecycle("test capture refusal");
    world.apply();
    assert!(
        block
            .world
            .accounts
            .get(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("unpublished")
            .is_none()
    );
    assert_eq!(pool.reserved_bytes(), 0);
}
