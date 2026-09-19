//! Actual producer-owned sources and strict applied-capture reconciliation.
//! Supplemental captures use the existing test staging API, not invented finality.

use super::tests::{apply_source, cache_canonical_test_transaction_set};
use super::*;
use crate::{
    governance::manifest::{LaneManifestRegistry, LaneManifestStatus},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::{
        Execute,
        isi::triggers::set::{
            SetReadOnly,
            invocation_identity::{pipeline_trigger_use_v1, time_trigger_use_v1},
        },
    },
    state::{State, TransactionsBlockError, World},
};
use iroha_data_model::{
    NetworkId, Registrable,
    account::Account,
    block::{
        BlockHeader, SignedBlock,
        builder::BlockBuilder,
        execution_output::{PipelineEventPositionV1, PipelineInvocationV1, TimeInvocationV1},
    },
    events::{
        pipeline::{BlockEventFilter, BlockStatus},
        time::{ExecutionTime, TimeEventFilter},
    },
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceRouteV1},
    isi::{InstructionBox, Log, Register, Unregister},
    parameter::{BlockParameter, ExecutionOutputPolicyV1, Parameter},
    transaction::{FeePaymentIntent, TransactionBuilder},
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_logger::Level;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use mv::storage::StorageReadOnly;
use std::num::{NonZeroU32, NonZeroU64};

fn fixture() -> (State, SignedBlock, TriggerId, TriggerId) {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let statuses = state
        .nexus_snapshot()
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| {
            (
                lane.id,
                LaneManifestStatus {
                    lane: lane.id,
                    alias: lane.alias.clone(),
                    dataspace: lane.dataspace_id,
                    visibility: lane.visibility,
                    storage: lane.storage,
                    governance: None,
                    manifest_path: None,
                    governance_rules: None,
                    privacy_commitments: Vec::new(),
                },
            )
        })
        .collect();
    state.install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(statuses)));
    {
        let mut parameters = state.world.parameters.block();
        let mut policy = ExecutionOutputPolicyV1::bootstrap();
        policy.max_output_bytes = 65_536;
        policy.max_pipeline_triggers = 1;
        policy.max_time_invocations = 1;
        policy.validate().unwrap();
        parameters
            .get_mut()
            .set_parameter(Parameter::Block(BlockParameter::ExecutionOutput(policy)));
        parameters.get_mut().set_parameter(Parameter::Block(
            BlockParameter::MaxTimeTriggerInvocations(NonZeroU32::MIN),
        ));
        parameters.commit();
    }
    let pipeline: TriggerId = "owned_inventory_pipeline".parse().unwrap();
    let time: TriggerId = "owned_inventory_time".parse().unwrap();
    let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    let mut tx = setup.transaction();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    let body = vec![InstructionBox::from(Log::new(
        Level::INFO,
        "actual zero-transfer callback".to_owned(),
    ))];
    let actions = [
        Trigger::new(
            pipeline.clone(),
            Action::new(
                body.clone(),
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                BlockEventFilter::new().for_status(BlockStatus::Approved),
            )
            .unwrap(),
        ),
        Trigger::new(
            time.clone(),
            Action::new(
                body,
                Repeats::Exactly(1),
                ALICE_ID.clone(),
                TimeEventFilter::new(ExecutionTime::PreCommit),
            )
            .unwrap(),
        ),
    ];
    for trigger in actions {
        Register::trigger(trigger)
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
    }
    tx.apply();
    setup.commit_world_overlay_for_testing().unwrap();
    let header = BlockHeader::new(NonZeroU64::new(2).unwrap(), None, None, 2, 0);
    let mut builder = BlockBuilder::new(header);
    // A successful and an actually rejected Network input both remain sources.
    for body in [
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "actual Network".to_owned(),
        ))],
        vec![Unregister::trigger("owned_inventory_absent".parse().unwrap()).into()],
    ] {
        let mut tx = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        );
        tx.set_creation_time(header.creation_time() - std::time::Duration::from_millis(1));
        builder.push_transaction(tx.with_instructions(body).sign(ALICE_KEYPAIR.private_key()));
    }
    (
        state,
        builder.build_with_signature(0, ALICE_KEYPAIR.private_key()),
        pipeline,
        time,
    )
}

fn execute(block: &mut StateBlock<'_>, source: &SignedBlock) {
    block.reserve_ordinary_execution_outputs(source).unwrap();
    block.execute_ordinary_output_plan(source, None).unwrap();
    cache_canonical_test_transaction_set(block, source.external_entrypoints_slice());
}

#[test]
fn actual_three_phase_zero_transcript_inventory_retains_every_call_in_output_order() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let (state, source, pipeline, time) = fixture();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(source.header());
    let height = source.header().height().get();
    let pipeline_call = PipelineInvocationV1 {
        event: PipelineEventPositionV1::BlockApproved,
        candidate_index: 0,
        trigger: pipeline_trigger_use_v1(&block.world.triggers, &pipeline, height).unwrap(),
    }
    .execution_call_hash(source.hash())
    .unwrap();
    let time_call = TimeInvocationV1 {
        schedule_index: 0,
        event: block.create_time_event(&source.header()),
        trigger: time_trigger_use_v1(&block.world.triggers, &time, height).unwrap(),
    }
    .execution_call_hash(source.hash())
    .unwrap();
    let mut expected: Vec<_> = source
        .network_entrypoints()
        .map(|input| Hash::from(input.execution_call_hash()))
        .collect();
    expected.extend([pipeline_call, time_call]);
    execute(&mut block, &source);
    assert!(
        block
            .world
            .triggers
            .pipeline_triggers()
            .get(&pipeline)
            .is_none()
    );
    assert!(block.world.triggers.time_triggers().get(&time).is_none());
    assert!(block.fastpq_transcripts.is_empty());
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
    block.inspect_owned_execution_sources_for_test(&source, |block, sources| {
        assert_eq!(sources.proposal(), source.hash());
        assert_eq!(sources.network_routes().len(), 2);
        assert_eq!(sources.entries().iter().map(|entry| entry.call()).collect::<Vec<_>>(), expected);
        block.finalize_owned_fastpq_source_inventory_with_pending(sources, None)?;
        let inventory = block.fastpq_source_inventory().unwrap().unwrap();
        assert_eq!(inventory.entries().iter().map(|entry| entry.entry_hash).collect::<Vec<_>>(), expected);
        assert!(inventory.transcript_entry_hashes().is_empty());
        assert!(inventory.entries().iter().all(|entry| entry.execution_kind == FastpqSourceExecutionKindV1::ExecutionCall));
        assert!(inventory.entries()[2..].iter().all(|entry|
            entry.route == FastpqSourceRouteV1::Unrouted && entry.dataspace_id == DataSpaceId::UNIVERSAL));
        assert!(block.finalize_owned_fastpq_source_inventory_with_pending(sources, None).is_err());
        block.verified_fastpq_source_inventory_for_capture()?;
        Ok(())
    }).unwrap();
    assert!(
        block
            .inspect_owned_execution_sources_for_test(&source, |_, _| Ok(()))
            .is_err()
    );
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn known_rejected_call_capture_and_typed_protocol_extra_remain_owned() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let (state, source, _, _) = fixture();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(source.header());
    execute(&mut block, &source);
    let protocol = Hash::new(b"owned inventory actual applied protocol test occurrence");
    block
        .inspect_owned_execution_sources_for_test(&source, |block, sources| {
            let rejected = &sources.entries()[1];
            assert_eq!(
                rejected.call(),
                Hash::from(
                    source
                        .network_entrypoint_at(1)
                        .unwrap()
                        .execution_call_hash()
                )
            );
            // The supplied delta is a capture-reconciliation fixture, not a claim
            // that the rejected instruction moved an asset or charged this fee.
            apply_source(
                block,
                rejected.call(),
                false,
                Some(sources.network_routes()[1]),
            );
            apply_source(block, protocol, true, None);
            let pending = block.submit_transfer_transcript_digest_batch();
            block.finalize_owned_fastpq_source_inventory_with_pending(sources, pending)?;
            let inventory = block.fastpq_source_inventory().unwrap().unwrap();
            assert_eq!(inventory.entries().len(), sources.entries().len() + 1);
            assert_eq!(inventory.transcript_entry_hashes().len(), 2);
            assert_eq!(inventory.entries()[1].entry_hash, rejected.call());
            let extra = inventory.entries().last().unwrap();
            assert_eq!(extra.entry_hash, protocol);
            assert_eq!(
                extra.execution_kind,
                FastpqSourceExecutionKindV1::ProtocolPurpose
            );
            assert_eq!(extra.route, FastpqSourceRouteV1::Unrouted);
            block.verified_fastpq_source_inventory_for_capture()?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn unknown_internal_capture_and_changed_known_capture_refuse_and_latch() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for mutation in 0..4 {
        let (state, source, _, _) = fixture();
        crate::sumeragi::witness::start_block();
        let mut block = state.block(source.header());
        execute(&mut block, &source);
        block
            .inspect_owned_execution_sources_for_test(&source, |block, sources| {
                let internal = sources.entries()[2].call();
                match mutation {
                    0 => apply_source(
                        block,
                        Hash::new(b"unowned internal execution call"),
                        false,
                        None,
                    ),
                    1 => apply_source(block, internal, true, None),
                    2 => apply_source(
                        block,
                        internal,
                        false,
                        Some(RoutingDecision::new(
                            iroha_model_base::topology::LaneId::SINGLE,
                            DataSpaceId::new(9),
                        )),
                    ),
                    3 => {
                        apply_source(block, internal, false, None);
                        block.fastpq_source_captures = Default::default();
                    }
                    _ => unreachable!(),
                }
                let error = block
                    .finalize_owned_fastpq_source_inventory_with_pending(sources, None)
                    .unwrap_err();
                if mutation == 0 {
                    assert_eq!(error, "FASTPQ transcript has no owned execution call");
                } else if mutation == 3 {
                    assert_eq!(
                        error,
                        "FASTPQ applied source keys differ from the transcript accumulator"
                    );
                } else {
                    assert_eq!(
                        error,
                        "FASTPQ block entry differs from its applied source context"
                    );
                }
                assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
                block.fastpq_transcripts.clear();
                block.fastpq_source_captures = Default::default();
                assert!(
                    block
                        .finalize_owned_fastpq_source_inventory_with_pending(sources, None)
                        .is_err()
                );
                assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
                assert!(
                    block
                        .verified_fastpq_source_inventory_for_capture()
                        .is_err()
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn foreign_proposal_and_frozen_context_refuse_before_digest_mutation() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for mutation in 0..4 {
        let (state, source, _, _) = fixture();
        crate::sumeragi::witness::start_block();
        let mut block = state.block(source.header());
        execute(&mut block, &source);
        block
            .inspect_owned_execution_sources_for_test(&source, |block, sources| {
                apply_source(
                    block,
                    Hash::new(b"preflight protocol occurrence"),
                    true,
                    None,
                );
                let transcripts = block.fastpq_transcripts.clone();
                let original_header = block._curr_block;
                let original_network = block.network_id;
                let original_context = block.fastpq_source_context.clone();
                match mutation {
                    0 => {
                        block._curr_block =
                            BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0)
                    }
                    1 => block.fastpq_source_context = None,
                    2 => {
                        let mut changed = (**block.fastpq_source_context.as_ref().unwrap()).clone();
                        changed.source.height += 1;
                        block.fastpq_source_context = Some(Arc::new(changed));
                    }
                    3 => {
                        // Both mutable applying context fields agree; only the
                        // producer-owned capsule retains the original network.
                        let foreign_network = NetworkId::from_genesis_hash(
                            BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0).hash(),
                        );
                        assert_ne!(foreign_network, original_network);
                        block.network_id = foreign_network;
                        let mut changed = (**block.fastpq_source_context.as_ref().unwrap()).clone();
                        changed.source.network_id = foreign_network;
                        block.fastpq_source_context = Some(Arc::new(changed));
                        assert_ne!(
                            sources.source_context(),
                            block.fastpq_source_context.as_ref().unwrap().source
                        );
                    }
                    _ => unreachable!(),
                }
                let error = block
                    .finalize_owned_fastpq_source_inventory_with_pending(sources, None)
                    .unwrap_err();
                if mutation == 3 {
                    assert_eq!(
                        error,
                        "FASTPQ owned sources differ from the applying source-height context"
                    );
                }
                assert_eq!(block.fastpq_transcripts, transcripts);
                assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
                block._curr_block = original_header;
                block.network_id = original_network;
                block.fastpq_source_context = original_context;
                assert!(
                    block
                        .finalize_owned_fastpq_source_inventory_with_pending(sources, None)
                        .is_err()
                );
                assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn owned_seal_still_rejects_late_applied_capture_after_transcript_drain() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let (state, source, _, _) = fixture();
    crate::sumeragi::witness::start_block();
    let mut block = state.block(source.header());
    execute(&mut block, &source);
    block
        .inspect_owned_execution_sources_for_test(&source, |block, sources| {
            let call = sources.entries()[2].call();
            apply_source(block, call, false, None);
            block.finalize_owned_fastpq_source_inventory_with_pending(sources, None)?;
            let inventory = block.fastpq_source_inventory().unwrap().unwrap().clone();
            let transcripts = block.drain_transfer_transcripts_with_pending(None);
            assert_eq!(transcripts.len(), 1);
            block.verified_fastpq_source_inventory_for_capture()?;
            apply_source(block, call, false, None);
            assert!(
                block
                    .verified_fastpq_source_inventory_for_capture()
                    .is_err()
            );
            assert_eq!(
                block.fastpq_source_inventory().unwrap().unwrap(),
                &inventory
            );
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}
