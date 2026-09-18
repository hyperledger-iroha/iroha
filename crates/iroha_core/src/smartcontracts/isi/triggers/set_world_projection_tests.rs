//! Actual trigger-store net-delta and borrowed semantic codec controls.
//! These tests do not execute callbacks or authenticate a full State root.

use super::*;
use crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY;
use crate::state::world_projection::{WorldDeltaBuilder, WorldNetDelta, hash_value};
use iroha_data_model::events::{execute_trigger::ExecuteTriggerEventFilter, time::Schedule};
use iroha_primitives::json::Json;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use std::{num::NonZeroU32, time::Duration};

fn project(block: &SetBlock<'_>) -> WorldNetDelta {
    let mut builder = WorldDeltaBuilder::new();
    block
        .append_world_projection(&mut builder)
        .expect("trigger projection");
    builder.finish().expect("complete trigger projection")
}

fn log_instruction() -> InstructionBox {
    Log::new(Level::INFO, "world delta".to_owned()).into()
}

fn halt_blob() -> IvmBytecode {
    let mut program = ivm::ProgramMetadata::default().encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    IvmBytecode::from_compiled(program)
}

fn invocation() -> ContractInvocation {
    let network = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
        .parse()
        .expect("network fixture");
    ContractInvocation {
        contract_address: iroha_data_model::smart_contract::ContractAddress::derive(
            &network,
            &ALICE_ID,
            7,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .expect("contract identity"),
        expected_code_hash: Hash::new(b"world-delta-contract"),
        entrypoint: "main".to_owned(),
        arguments: None,
    }
}

fn register_call(tx: &mut SetTransaction<'_, '_>, id: &str, executable: Executable) {
    let action = SpecializedAction::new(
        executable,
        Repeats::Exactly(3),
        ALICE_ID.clone(),
        ExecuteTriggerEventFilter::new(),
    )
    .expect("valid by-call authority");
    assert!(
        tx.add_by_call_trigger(SpecializedTrigger::new(
            id.parse().expect("trigger id"),
            action,
        ))
        .expect("register by-call action")
    );
}

#[test]
fn borrowed_action_matches_owned_dto_for_all_executable_variants_and_flags() {
    let set = Set::default();
    let mut block = set.block();
    {
        let mut tx = block.transaction();
        for (id, executable) in [
            ("ivm", Executable::Ivm(halt_blob())),
            ("call", Executable::ContractCall(invocation())),
            (
                "instructions",
                Executable::Instructions(vec![log_instruction()].into()),
            ),
            (
                "batch",
                Executable::Batch(
                    vec![
                        ExecutableBatchItem::Instruction(log_instruction()),
                        ExecutableBatchItem::ContractCall(invocation()),
                    ]
                    .into(),
                ),
            ),
        ] {
            register_call(&mut tx, id, executable);
        }
        tx.apply();
    }
    for (_, action) in block.by_call_triggers.iter() {
        let expected = LoadedActionDto::from(action).encode();
        let expected_hash = hash_value(&LoadedActionDto::from(action)).unwrap();
        for flags in [
            0,
            norito::core::default_encode_flags(),
            norito::core::header_flags::PACKED_STRUCT | norito::core::header_flags::COMPACT_LEN,
        ] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(BorrowedWorldAction::new(action).encode(), expected);
            assert_eq!(hash_world_action(action).unwrap(), expected_hash);
        }
    }
    assert_eq!(
        project(&block).changed_values(),
        13,
        "four actions/ids/active ids and one blob"
    );
}

#[test]
fn borrowed_contract_binds_blob_code_hash_and_reference_count() {
    let blob = halt_blob();
    let entry = IvmBytecodeEntry {
        code_hash: ivm::contract_code_hash(blob.as_ref()),
        original_contract: blob,
        count: NonZeroU64::MIN,
    };
    assert_eq!(
        BorrowedWorldContract::from(&entry).encode(),
        IvmBytecodeEntryDto::from(&entry).encode()
    );
    assert_eq!(
        hash_world_contract(&entry).unwrap(),
        hash_value(&IvmBytecodeEntryDto::from(&entry)).unwrap()
    );
    let original = hash_world_contract(&entry).unwrap();
    for changed in [
        IvmBytecodeEntry {
            count: NonZeroU64::new(2).unwrap(),
            ..entry.clone()
        },
        IvmBytecodeEntry {
            code_hash: Hash::new(b"substituted-code"),
            ..entry.clone()
        },
        IvmBytecodeEntry {
            original_contract: IvmBytecode::from_compiled(vec![1, 2, 3]),
            ..entry.clone()
        },
    ] {
        assert_ne!(
            hash_world_contract(&changed).unwrap(),
            original,
            "even an invalid stored identity must not disappear from the diagnostic projection"
        );
    }
}

#[test]
fn actual_trigger_rollback_and_canceled_registration_do_not_change_net_delta() {
    let set = Set::default();
    let mut block = set.block();
    let empty = project(&block);
    {
        let mut tx = block.transaction();
        register_call(&mut tx, "aborted", Executable::Ivm(halt_blob()));
        // Dropping the actual transaction removes action, ids, active ids and blob.
    }
    assert_eq!(project(&block), empty);
    {
        let mut tx = block.transaction();
        register_call(&mut tx, "canceled", Executable::Ivm(halt_blob()));
        assert!(tx.remove(&"canceled".parse().unwrap()));
        tx.apply();
    }
    assert!(block.by_call_triggers.touched_entries().next().is_some());
    assert!(block.contracts.touched_entries().next().is_some());
    assert_eq!(
        project(&block),
        empty,
        "applied absent-to-absent history is not a net change"
    );
    let mut legacy = Vec::new();
    block.append_merge_execution_write_set(&mut legacy);
    assert!(
        !legacy.is_empty(),
        "existing merge touch-history format remains unchanged"
    );
}

#[test]
fn actual_registration_order_and_shared_blob_refcount_history_are_canonical() {
    let set = Set::default();
    let mut first = set.block();
    {
        let mut tx = first.transaction();
        register_call(&mut tx, "a", Executable::Ivm(halt_blob()));
        register_call(&mut tx, "b", Executable::Ivm(halt_blob()));
        tx.apply();
    }
    let expected = project(&first);
    assert_eq!(
        expected.changed_values(),
        7,
        "two actions/ids/active ids and shared blob"
    );
    assert_eq!(first.contracts.iter().next().unwrap().1.count.get(), 2);
    drop(first);
    let mut second = set.block();
    {
        let mut tx = second.transaction();
        register_call(&mut tx, "temporary", Executable::Ivm(halt_blob()));
        assert!(tx.remove(&"temporary".parse().unwrap()));
        register_call(&mut tx, "b", Executable::Ivm(halt_blob()));
        register_call(&mut tx, "a", Executable::Ivm(halt_blob()));
        tx.apply();
    }
    assert_eq!(
        project(&second),
        expected,
        "temporary dedup entries do not alter final semantic delta"
    );
    assert_eq!(second.contracts.iter().next().unwrap().1.count.get(), 2);
}

#[test]
fn actual_repeat_enable_and_retry_mutations_are_bound_and_noop_access_is_not() {
    let set = Set::default();
    let id: TriggerId = "scheduled".parse().unwrap();
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        let mut action = SpecializedAction::new(
            Executable::Instructions(vec![log_instruction()].into()),
            Repeats::Exactly(3),
            ALICE_ID.clone(),
            TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
                Duration::from_millis(5),
            ))),
        )
        .expect("time action");
        action.retry_policy = Some(TimeTriggerRetryPolicy {
            max_retries: NonZeroU32::new(3).unwrap(),
            retry_after_ms: NonZeroU64::new(500).unwrap(),
        });
        assert!(
            tx.add_time_trigger(SpecializedTrigger::new(id.clone(), action))
                .unwrap()
        );
        tx.apply();
        block.commit();
    }
    let mut block = set.block();
    let unchanged = project(&block);
    {
        let mut tx = block.transaction();
        tx.mod_repeats(&id, Ok).expect("unchanged repeats");
        tx.apply();
    }
    assert!(block.time_triggers.touched_entries().next().is_some());
    assert_eq!(project(&block), unchanged);
    {
        let mut tx = block.transaction();
        tx.mod_repeats(&id, |count| Ok(count - 1)).unwrap();
        tx.apply();
    }
    let repeated = project(&block);
    assert_eq!(repeated.changed_values(), 1);
    assert_ne!(repeated, unchanged);
    {
        let mut tx = block.transaction();
        assert!(tx.set_time_trigger_retry_state(
            &id,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 42
            })
        ));
        tx.apply();
    }
    let retry = project(&block);
    assert_ne!(retry, repeated);
    {
        let mut tx = block.transaction();
        tx.inspect_by_id_mut(&id, |action| {
            action.metadata_mut().insert(
                TRIGGER_ENABLED_METADATA_KEY.parse().unwrap(),
                Json::from(false),
            );
        })
        .expect("registered action");
        tx.apply();
    }
    assert_ne!(project(&block), retry);
    assert_eq!(
        project(&block).changed_values(),
        2,
        "action and exact active-id removal"
    );
    let action = block.time_triggers.get(&id).unwrap();
    assert_eq!(
        BorrowedWorldAction::new(action).encode(),
        LoadedActionDto::from(action).encode()
    );
}

#[test]
fn borrowed_action_binds_authority_filter_metadata_and_retry_fields() {
    let set = Set::default();
    let mut block = set.block();
    {
        let mut tx = block.transaction();
        register_call(
            &mut tx,
            "identity",
            Executable::Instructions(vec![log_instruction()].into()),
        );
        tx.apply();
    }
    let action = block
        .by_call_triggers
        .get(&"identity".parse().unwrap())
        .unwrap();
    let original = hash_world_action(action).unwrap();
    for changed in [
        LoadedAction {
            authority: BOB_ID.clone(),
            ..action.clone()
        },
        LoadedAction {
            filter: action.filter.clone().under_authority(BOB_ID.clone()),
            ..action.clone()
        },
        LoadedAction {
            retry_state: Some(TimeTriggerRetryState {
                retries_used: 2,
                next_retry_at_ms: 99,
            }),
            ..action.clone()
        },
        LoadedAction {
            retry_policy: Some(TimeTriggerRetryPolicy {
                max_retries: NonZeroU32::MIN,
                retry_after_ms: NonZeroU64::MIN,
            }),
            ..action.clone()
        },
        LoadedAction {
            metadata: {
                let mut m = Metadata::default();
                m.insert("bound".parse().unwrap(), Json::from(1_u64));
                m
            },
            ..action.clone()
        },
    ] {
        assert_ne!(hash_world_action(&changed).unwrap(), original);
    }
}

#[test]
fn net_delta_hook_mentions_every_trigger_block_store() {
    let source = include_str!("set.rs");
    let declaration = source
        .split("pub struct SetBlock<'set> {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let method = source
        .split("pub(crate) fn append_world_projection(")
        .nth(1)
        .unwrap()
        .split("\n    }\n}")
        .next()
        .unwrap();
    let fields: Vec<_> = declaration
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.starts_with("///"))
                .then(|| line.split_once(':'))
                .flatten()
                .map(|(name, _)| name)
        })
        .collect();
    assert_eq!(
        fields.len(),
        10,
        "review new trigger-store owners explicitly"
    );
    for field in fields {
        assert!(
            method.contains(&format!("&self.{field},")),
            "missing trigger store {field}"
        );
    }
}
