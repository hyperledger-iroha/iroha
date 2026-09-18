//! Persistent trigger-store identity tests; no callback/execution authority is fabricated.

use super::super::Set;
use super::*;
use crate::smartcontracts::isi::triggers::specialized::{
    SpecializedAction, SpecializedTrigger, TimeTriggerRetryState,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    events::{
        pipeline::{BlockEventFilter, BlockStatus, PipelineEventFilterBox},
        time::Schedule,
    },
    transaction::{
        IvmBytecode,
        executable::{ContractArgumentRecord, ContractInvocation, ExecutableBatchItem},
    },
};
use iroha_primitives::{const_vec::ConstVec, json::Json};
use std::num::{NonZeroU32, NonZeroU64};

fn authority(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn instructions(message: &str) -> ConstVec<InstructionBox> {
    ConstVec::from(vec![Log::new(Level::INFO, message.to_owned()).into()])
}

fn time_action(executable: Executable) -> SpecializedAction<TimeEventFilter> {
    let mut action = SpecializedAction::new(
        executable,
        Repeats::Exactly(4),
        authority(0x31),
        TimeEventFilter(ExecutionTime::Schedule(Schedule {
            start_ms: 1,
            period_ms: Some(100),
        })),
    )
    .unwrap();
    action.metadata.insert(
        trigger_registered_block_height_metadata_key().clone(),
        Json::from(3_u64),
    );
    action.retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::new(3).unwrap(),
        retry_after_ms: NonZeroU64::new(100).unwrap(),
    });
    action
}

fn fixture(executable: Executable) -> (Set, TriggerId, TriggerId) {
    let set = Set::default();
    let time: TriggerId = "bound-time".parse().unwrap();
    let pipeline: TriggerId = "bound-pipeline".parse().unwrap();
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(SpecializedTrigger::new(
            time.clone(),
            time_action(executable),
        ))
        .unwrap();
        let mut action = SpecializedAction::new(
            Executable::Instructions(instructions("pipeline")),
            Repeats::Exactly(2),
            authority(0x32),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        )
        .unwrap();
        action.metadata.insert(
            trigger_registered_block_height_metadata_key().clone(),
            Json::from(3_u64),
        );
        tx.add_pipeline_trigger(SpecializedTrigger::new(pipeline.clone(), action))
            .unwrap();
        tx.apply();
        block.commit();
    }
    (set, time, pipeline)
}

#[test]
fn trigger_use_streams_exact_persistent_model_frames_without_core_schema() {
    let (set, time, pipeline) = fixture(Executable::Instructions(instructions("root")));
    let view = set.view();
    let actual = time_trigger_use_v1(&view, &time, 8).unwrap();
    let action = view.time_triggers().get(&time).unwrap();
    let mut expected = ACTION_DOMAIN.to_vec();
    expected.push(1);
    expected.extend(norito::encode_canonical(&action.authority).unwrap());
    expected.extend(norito::encode_canonical(&action.filter).unwrap());
    expected.extend(norito::encode_canonical(&action.repeats).unwrap());
    expected.extend(norito::encode_canonical(&action.metadata).unwrap());
    expected.extend(norito::encode_canonical(&action.retry_policy).unwrap());
    expected.extend(norito::encode_canonical(&Option::<(u32, u64)>::None).unwrap());
    expected.push(0);
    expected.extend(norito::encode_canonical(&instructions("root")).unwrap());
    assert_eq!(actual.action_hash, Hash::new(&expected));
    assert_eq!(actual.registered_at_height, 3);
    assert_eq!(actual.trigger_id, time);
    assert_eq!(time_trigger_use_v1(&view, &time, 100).unwrap(), actual);
    let alternative =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _guard = norito::core::DecodeFlagsGuard::enter(alternative);
    assert_eq!(time_trigger_use_v1(&view, &time, 8).unwrap(), actual);
    assert_ne!(
        pipeline_trigger_use_v1(&view, &pipeline, 8)
            .unwrap()
            .action_hash,
        actual.action_hash
    );
}

#[test]
fn trigger_use_commits_each_persistent_action_and_retry_field() {
    let (set, time, _) = fixture(Executable::Instructions(instructions("root")));
    let mut block = set.block();
    let mut tx = block.transaction();
    tx.time_triggers.get_mut(&time).unwrap().retry_state = Some(TimeTriggerRetryState {
        retries_used: 1,
        next_retry_at_ms: 500,
    });
    let original = tx.time_triggers.get(&time).unwrap().clone();
    let base = time_trigger_use_v1(&tx, &time, 8).unwrap();
    for mutation in 0..10 {
        let mut action = original.clone();
        match mutation {
            0 => action.authority = authority(0x77),
            1 => action.repeats = Repeats::Exactly(5),
            2 => {
                action.filter = TimeEventFilter(ExecutionTime::Schedule(Schedule {
                    start_ms: 2,
                    period_ms: Some(100),
                }))
            }
            3 => {
                action
                    .metadata
                    .insert("application-value".parse().unwrap(), Json::from(1_u64));
            }
            4 => action.retry_policy.as_mut().unwrap().max_retries = NonZeroU32::new(4).unwrap(),
            5 => {
                action.retry_policy.as_mut().unwrap().retry_after_ms = NonZeroU64::new(101).unwrap()
            }
            6 => action.retry_state.as_mut().unwrap().retries_used = 2,
            7 => action.retry_state.as_mut().unwrap().next_retry_at_ms = 501,
            8 => action.executable = ExecutableRef::Instructions(instructions("other root")),
            9 => {
                action.metadata.insert(
                    trigger_registered_block_height_metadata_key().clone(),
                    Json::from(4_u64),
                );
            }
            _ => unreachable!(),
        }
        tx.time_triggers.insert(time.clone(), action);
        assert_ne!(
            time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
            base.action_hash,
            "persistent field {mutation}"
        );
    }
    let mut action = original.clone();
    action.retry_state = None;
    tx.time_triggers.insert(time.clone(), action);
    assert_ne!(
        time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
        base.action_hash
    );
    let mut action = original;
    action.retry_state = None;
    action.retry_policy = None;
    tx.time_triggers.insert(time.clone(), action);
    assert_ne!(
        time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
        base.action_hash
    );
}

#[test]
fn trigger_use_rejects_missing_malformed_current_and_future_registration() {
    let (set, time, _) = fixture(Executable::Instructions(instructions("root")));
    let mut block = set.block();
    let mut tx = block.transaction();
    assert!(
        time_trigger_use_v1(&tx, &time, 0)
            .unwrap_err()
            .contains("strictly before")
    );
    tx.time_triggers
        .get_mut(&time)
        .unwrap()
        .metadata
        .remove(trigger_registered_block_height_metadata_key());
    assert!(
        time_trigger_use_v1(&tx, &time, 8)
            .unwrap_err()
            .contains("missing")
    );
    for invalid in [Json::from("3"), Json::new(-1_i64), Json::from(true)] {
        tx.time_triggers.get_mut(&time).unwrap().metadata.insert(
            trigger_registered_block_height_metadata_key().clone(),
            invalid,
        );
        assert!(
            time_trigger_use_v1(&tx, &time, 8)
                .unwrap_err()
                .contains("malformed")
        );
    }
    for invalid in [8_u64, 9, u64::MAX] {
        tx.time_triggers.get_mut(&time).unwrap().metadata.insert(
            trigger_registered_block_height_metadata_key().clone(),
            Json::from(invalid),
        );
        assert!(
            time_trigger_use_v1(&tx, &time, 8)
                .unwrap_err()
                .contains("strictly before")
        );
    }
    tx.time_triggers.get_mut(&time).unwrap().metadata.insert(
        trigger_registered_block_height_metadata_key().clone(),
        Json::from(0_u64),
    );
    assert_eq!(
        time_trigger_use_v1(&tx, &time, 1)
            .unwrap()
            .registered_at_height,
        0
    );
}

#[test]
fn trigger_use_requires_exact_type_registry_and_pipeline_retry_shape() {
    let (set, time, pipeline) = fixture(Executable::Instructions(instructions("root")));
    let mut block = set.block();
    let mut tx = block.transaction();
    assert!(pipeline_trigger_use_v1(&tx, &time, 8).is_err());
    assert!(time_trigger_use_v1(&tx, &pipeline, 8).is_err());
    assert!(pipeline_trigger_use_v1(&tx, &"absent".parse().unwrap(), 8).is_err());
    let pipeline_original = pipeline_trigger_use_v1(&tx, &pipeline, 8).unwrap();
    tx.pipeline_triggers.get_mut(&pipeline).unwrap().filter =
        PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Committed));
    assert_ne!(
        pipeline_trigger_use_v1(&tx, &pipeline, 8)
            .unwrap()
            .action_hash,
        pipeline_original.action_hash
    );
    tx.pipeline_triggers.get_mut(&pipeline).unwrap().retry_state = Some(TimeTriggerRetryState {
        retries_used: 1,
        next_retry_at_ms: 100,
    });
    assert!(
        pipeline_trigger_use_v1(&tx, &pipeline, 8)
            .unwrap_err()
            .contains("Time retry")
    );
    tx.pipeline_triggers.get_mut(&pipeline).unwrap().retry_state = None;
    tx.pipeline_triggers
        .get_mut(&pipeline)
        .unwrap()
        .retry_policy = Some(TimeTriggerRetryPolicy {
        max_retries: NonZeroU32::MIN,
        retry_after_ms: NonZeroU64::MIN,
    });
    assert!(
        pipeline_trigger_use_v1(&tx, &pipeline, 8)
            .unwrap_err()
            .contains("Time retry")
    );
    tx.ids.insert(time.clone(), TriggeringEventType::Pipeline);
    assert!(time_trigger_use_v1(&tx, &time, 8).is_err());
    tx.ids.insert(time.clone(), TriggeringEventType::Time);
    tx.time_triggers.remove(time.clone());
    assert!(
        time_trigger_use_v1(&tx, &time, 8)
            .unwrap_err()
            .contains("action is absent")
    );
}

#[test]
fn trigger_use_authenticates_borrowed_artifact_lookup_and_complete_code_identity() {
    // Bytes are an identity fixture, not a claim of ABI-admitted executable code.
    let artifact = IvmBytecode::from_compiled(vec![0xAA; 64 * 1024]);
    let lookup = HashOf::new(&artifact);
    let (set, time, _) = fixture(Executable::Ivm(artifact));
    let mut block = set.block();
    let mut tx = block.transaction();
    let pointer = tx.get_original_contract(&lookup).unwrap().as_ref().as_ptr();
    let original = time_trigger_use_v1(&tx, &time, 8).unwrap();
    assert_eq!(
        tx.get_original_contract(&lookup).unwrap().as_ref().as_ptr(),
        pointer
    );
    {
        let alternative =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _guard = norito::core::DecodeFlagsGuard::enter(alternative);
        assert_eq!(time_trigger_use_v1(&tx, &time, 8).unwrap(), original);
    }
    let retained = tx.contracts.get(&lookup).unwrap().clone();
    tx.contracts.remove(lookup);
    assert!(
        time_trigger_use_v1(&tx, &time, 8)
            .unwrap_err()
            .contains("artifact is absent")
    );
    tx.contracts.insert(lookup, retained.clone());
    tx.contracts.get_mut(&lookup).unwrap().code_hash = Hash::new(b"unchecked caller digest");
    assert!(
        time_trigger_use_v1(&tx, &time, 8)
            .unwrap_err()
            .contains("deployable identity")
    );
    tx.contracts.insert(lookup, retained.clone());
    let different = IvmBytecode::from_compiled(vec![0xBB; 64 * 1024]);
    let different_lookup = HashOf::new(&different);
    let entry = tx.contracts.get_mut(&lookup).unwrap();
    entry.original_contract = different;
    entry.code_hash = ivm::contract_code_hash(entry.original_contract.as_ref());
    assert!(
        time_trigger_use_v1(&tx, &time, 8)
            .unwrap_err()
            .contains("lookup identity")
    );
    // Correctly relocating both retained identities is a different legitimate
    // content binding, not a false negative or permission to execute the bytes.
    let changed_entry = tx.contracts.remove(lookup).unwrap();
    tx.contracts.insert(different_lookup, changed_entry);
    tx.time_triggers.get_mut(&time).unwrap().executable = ExecutableRef::Ivm(different_lookup);
    assert_ne!(
        time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
        original.action_hash
    );
    tx.time_triggers.get_mut(&time).unwrap().executable = ExecutableRef::Ivm(lookup);
    tx.contracts.insert(lookup, retained);
    assert_eq!(time_trigger_use_v1(&tx, &time, 8).unwrap(), original);
}

#[test]
fn trigger_use_ignores_ephemeral_generation_and_contract_reference_count() {
    let artifact = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
    let lookup = HashOf::new(&artifact);
    let original_action = time_action(Executable::Ivm(artifact));
    let (set, time, _) = fixture(original_action.executable.clone());
    let mut block = set.block();
    let mut tx = block.transaction();
    let original = time_trigger_use_v1(&tx, &time, 8).unwrap();
    assert_eq!(tx.registration_generation(&time), 0);
    // Low-level Set fixture preserves content/registration metadata solely to
    // prove generation independence; public Register stamps current height.
    assert!(tx.remove(&time));
    tx.add_time_trigger(SpecializedTrigger::new(time.clone(), original_action))
        .unwrap();
    assert!(tx.registration_generation(&time) > 0);
    assert_eq!(time_trigger_use_v1(&tx, &time, 8).unwrap(), original);
    tx.contracts.get_mut(&lookup).unwrap().count = NonZeroU64::new(77).unwrap();
    assert_eq!(time_trigger_use_v1(&tx, &time, 8).unwrap(), original);
    tx.apply();
    block.commit();
    assert_eq!(
        time_trigger_use_v1(&set.view(), &time, 8).unwrap(),
        original
    );
}

#[test]
fn trigger_use_distinguishes_executable_variant_from_same_instruction_content() {
    let body = instructions("root");
    let (set, time, _) = fixture(Executable::Instructions(body.clone()));
    let mut block = set.block();
    let mut tx = block.transaction();
    let original = time_trigger_use_v1(&tx, &time, 8).unwrap();
    tx.time_triggers.get_mut(&time).unwrap().executable = ExecutableRef::Batch(ConstVec::from(
        body.iter()
            .cloned()
            .map(ExecutableBatchItem::Instruction)
            .collect::<Vec<_>>(),
    ));
    assert_ne!(
        time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
        original.action_hash
    );
}

#[test]
fn trigger_use_commits_complete_declared_contract_invocation() {
    // This authenticates the declaration in the trigger store. Deployment,
    // argument ABI and use-time instance binding remain executor checks.
    let address = |nonce| {
        ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .unwrap(),
            &authority(0x31),
            nonce,
            iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        )
        .unwrap()
    };
    let call = ContractInvocation {
        contract_address: address(1),
        expected_code_hash: Hash::new(b"declared exact deployment"),
        entrypoint: "main".to_owned(),
        arguments: None,
    };
    let (set, time, _) = fixture(Executable::ContractCall(call.clone()));
    let mut block = set.block();
    let mut tx = block.transaction();
    let original = time_trigger_use_v1(&tx, &time, 8).unwrap();
    for mutation in 0..4 {
        let mut changed = call.clone();
        match mutation {
            0 => changed.contract_address = address(2),
            1 => changed.expected_code_hash = Hash::new(b"different deployment"),
            2 => changed.entrypoint = "other".to_owned(),
            3 => changed.arguments = Some(ContractArgumentRecord::try_new(vec![1, 2, 3]).unwrap()),
            _ => unreachable!(),
        }
        tx.time_triggers.get_mut(&time).unwrap().executable = ExecutableRef::ContractCall(changed);
        assert_ne!(
            time_trigger_use_v1(&tx, &time, 8).unwrap().action_hash,
            original.action_hash,
            "declared invocation field {mutation}"
        );
    }
}

#[test]
fn trigger_use_authority_rekey_changes_action_and_calls_without_growing_descriptor() {
    use iroha_data_model::{
        account::{MultisigMember, MultisigPolicy},
        block::{
            BlockHeader,
            execution_output::{PipelineEventPositionV1, PipelineInvocationV1, TimeInvocationV1},
        },
        events::time::{TimeEvent, TimeInterval},
    };

    let (set, time, pipeline) = fixture(Executable::Instructions(instructions("root")));
    let mut block = set.block();
    let mut tx = block.transaction();
    let before_time = time_trigger_use_v1(&tx, &time, 8).unwrap();
    let before_pipeline = pipeline_trigger_use_v1(&tx, &pipeline, 8).unwrap();
    let proposal = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"authority rekey invocation identity test",
    ));
    let time_call = |trigger: TriggerUseV1| {
        TimeInvocationV1 {
            schedule_index: 2,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 100,
                    length_ms: 20,
                },
            },
            trigger,
        }
        .execution_call_hash(proposal)
        .unwrap()
    };
    let pipeline_call = |trigger: TriggerUseV1| {
        PipelineInvocationV1 {
            event: PipelineEventPositionV1::BlockApproved,
            candidate_index: 4,
            trigger,
        }
        .execution_call_hash(proposal)
        .unwrap()
    };
    let members = [0x61, 0x62]
        .into_iter()
        .map(|seed| {
            MultisigMember::new(
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
                1,
            )
            .unwrap()
        })
        .collect();
    let replacement = AccountId::new_multisig(MultisigPolicy::new(2, members).unwrap());
    assert!(
        norito::encode_canonical(&replacement).unwrap().len()
            > norito::encode_canonical(&authority(0x31)).unwrap().len()
    );
    // Exercise the actual Set rekey mutation, not a fabricated descriptor hash.
    // Account rekey authorization remains the enclosing State instruction's job.
    tx.replace_account_id(&authority(0x31), &replacement)
        .unwrap();
    tx.replace_account_id(&authority(0x32), &replacement)
        .unwrap();
    assert_eq!(tx.time_triggers.get(&time).unwrap().authority, replacement);
    assert_eq!(
        tx.pipeline_triggers.get(&pipeline).unwrap().authority,
        replacement
    );
    let after_time = time_trigger_use_v1(&tx, &time, 8).unwrap();
    let after_pipeline = pipeline_trigger_use_v1(&tx, &pipeline, 8).unwrap();
    for (before, after) in [
        (&before_time, &after_time),
        (&before_pipeline, &after_pipeline),
    ] {
        assert_eq!(before.trigger_id, after.trigger_id);
        assert_eq!(before.registered_at_height, after.registered_at_height);
        assert_ne!(before.action_hash, after.action_hash);
        assert_eq!(
            norito::encode_canonical(before).unwrap().len(),
            norito::encode_canonical(after).unwrap().len()
        );
    }
    assert_ne!(time_call(before_time.clone()), time_call(after_time));
    assert_ne!(
        pipeline_call(before_pipeline.clone()),
        pipeline_call(after_pipeline)
    );
    drop(tx);
    drop(block);
    let view = set.view();
    assert_eq!(time_trigger_use_v1(&view, &time, 8).unwrap(), before_time);
    assert_eq!(
        pipeline_trigger_use_v1(&view, &pipeline, 8).unwrap(),
        before_pipeline
    );
}
