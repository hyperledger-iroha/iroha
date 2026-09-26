//! Actual producer-owned sources and strict applied-capture reconciliation.
//! Captures come from real callbacks, fees and the expired-governance-lock sweep.

use super::tests::cache_canonical_test_transaction_set;
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
    state::{
        ExecutionOutputSealMetadata, GovernanceLockCustody, GovernanceLockRecord,
        GovernanceLocksForReferendum, State, TransactionsBlockError, World,
    },
};
use iroha_config::parameters::actual::{GasLiquidity, GasRate, GasVolatility};
use iroha_data_model::{
    NetworkId,
    account::Account,
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
    block::{
        BlockHeader, SignedBlock,
        builder::BlockBuilder,
        execution_output::{
            ExecutionOutputV1, PipelineEventPositionV1, PipelineInvocationV1, TimeInvocationV1,
        },
    },
    domain::Domain,
    events::{
        pipeline::{BlockEventFilter, BlockStatus},
        time::{ExecutionTime, TimeEventFilter},
    },
    fastpq::{FastpqSourceExecutionKindV1, FastpqSourceRouteV1},
    isi::{InstructionBox, Log, Mint, Register, Transfer, Unregister},
    parameter::{BlockParameter, ExecutionOutputPolicyV1, Parameter},
    transaction::{
        Executable, FeeChargeKind, FeeChargeLimit, FeePaymentIntent, TransactionBuilder,
    },
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_logger::Level;
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use mv::storage::StorageReadOnly;
use std::num::{NonZeroU32, NonZeroU64};

fn fixture() -> (State, SignedBlock, TriggerId, TriggerId) {
    fixture_with_effects(false, false)
}

fn fixture_with_effects(
    pipeline_transfer: bool,
    fee_and_protocol: bool,
) -> (State, SignedBlock, TriggerId, TriggerId) {
    let mut state = State::new_for_testing(
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
    let domain = DomainId::try_new("owned-inventory", "universal").unwrap();
    let asset = AssetDefinitionId::derive_from_components(domain.clone(), "coin".parse().unwrap());
    if fee_and_protocol {
        let fees = &mut state.nexus.get_mut().fees;
        fees.base_fee = Quantity::zero();
        fees.per_byte_fee = Quantity::zero();
        fees.per_instruction_fee = Quantity::zero();
        fees.per_gas_unit_fee = Quantity::zero();
        fees.fee_asset_id = asset.to_string();
        fees.fee_sink_account_id = BOB_ID.to_string();
        state.pipeline.gas.tech_account_id = BOB_ID.to_string();
        state.pipeline.gas.accepted_assets = vec![asset.canonical_address()];
        state.pipeline.gas.units_per_gas = vec![GasRate {
            asset: asset.canonical_address(),
            units_per_gas: 1,
            twap_local_per_xor: Numeric::one(),
            liquidity: GasLiquidity::Tier1,
            volatility: GasVolatility::Stable,
        }];
    }
    let pipeline: TriggerId = "owned_inventory_pipeline".parse().unwrap();
    let time: TriggerId = "owned_inventory_time".parse().unwrap();
    let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    let mut tx = setup.transaction();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    if pipeline_transfer || fee_and_protocol {
        Register::account(Account::new(BOB_ID.clone()))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        Register::domain(Domain::new(domain))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        Register::asset_definition(AssetDefinition::numeric(
            asset.clone(),
            "Owned inventory",
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        Mint::asset_quantity(
            Quantity::from(if fee_and_protocol {
                1_000_000_010_u64
            } else {
                10
            }),
            AssetId::of(asset.clone(), ALICE_ID.clone()),
        )
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    }
    if fee_and_protocol {
        Mint::asset_quantity(Quantity::one(), AssetId::of(asset.clone(), BOB_ID.clone()))
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        // Seed a funded persisted obligation, then let the real height-2 sweep
        // validate and consume its custody capability before ordinary execution.
        let custody = GovernanceLockCustody {
            escrowed: true,
            asset_definition_id: asset.clone(),
            bond_escrow_account: BOB_ID.clone(),
            slash_receiver_account: BOB_ID.clone(),
        };
        tx.validate_fastpq_governance_lock("owned-inventory-expiry", &ALICE_ID, &custody)
            .unwrap();
        tx.world.put_governance_locks(
            "owned-inventory-expiry".into(),
            GovernanceLocksForReferendum {
                locks: BTreeMap::from([(
                    ALICE_ID.clone(),
                    GovernanceLockRecord {
                        owner: ALICE_ID.clone(),
                        amount: Quantity::one(),
                        slashed: Quantity::zero(),
                        expiry_height: 1,
                        direction: 0,
                        duration_blocks: 1,
                        custody,
                    },
                )]),
            },
        );
    }
    let body = vec![InstructionBox::from(Log::new(
        Level::INFO,
        "actual zero-transfer callback".to_owned(),
    ))];
    let actions = [
        Trigger::new(
            pipeline.clone(),
            Action::new(
                if pipeline_transfer {
                    vec![
                        Transfer::asset_quantity(
                            AssetId::of(asset.clone(), ALICE_ID.clone()),
                            1_u32,
                            BOB_ID.clone(),
                        )
                        .into(),
                    ]
                } else {
                    body.clone()
                },
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
        let gas = crate::gas::meter_instructions(&body);
        let mut tx = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(
                if fee_and_protocol {
                    vec![FeeChargeLimit::new(
                        FeeChargeKind::PipelineGas,
                        asset.clone(),
                        Quantity::from(gas),
                    )]
                } else {
                    Vec::new()
                },
                fee_and_protocol.then(|| NonZeroU64::new(gas).unwrap()),
            ),
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

fn seal_metadata(block: &mut StateBlock<'_>) -> Result<ExecutionOutputSealMetadata, String> {
    Ok(ExecutionOutputSealMetadata {
        committed_fragment_count: u64::try_from(block.committed_fragment_count()).unwrap(),
        lane_finality_statements: Vec::new(),
    })
}

#[test]
fn actual_three_phase_zero_transcript_inventory_retains_every_call_in_output_order() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let (state, mut source, pipeline, time) = fixture();
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
    block
        .seal_execution_outputs(&mut source, |block, _, routes| {
            assert_eq!(routes.len(), 2);
            seal_metadata(block)
        })
        .unwrap();
    let inventory = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();
    assert_eq!(
        inventory
            .entries()
            .iter()
            .map(|entry| entry.entry_hash)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(inventory.transcript_entry_hashes().is_empty());
    assert!(
        inventory
            .entries()
            .iter()
            .all(|entry| entry.execution_kind == FastpqSourceExecutionKindV1::ExecutionCall)
    );
    assert!(
        inventory.entries()[2..]
            .iter()
            .all(|entry| entry.route == FastpqSourceRouteV1::Unrouted
                && entry.dataspace_id == DataSpaceId::UNIVERSAL)
    );
    block.verify_execution_output_seal(&source).unwrap();
    // A second consumer cannot obtain or finalize the already sealed source owner.
    assert!(
        block
            .inspect_owned_execution_sources_for_test(&source, |_, _| Ok(()))
            .is_err()
    );
    assert!(
        block
            .verified_fastpq_source_inventory_for_capture()
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
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let (state, mut source, _, _) = fixture_with_effects(false, true);
    let gas_fee = |index| {
        let TransactionEntrypoint::External(transaction) =
            source.network_entrypoint_at(index).unwrap()
        else {
            panic!("signed Network source")
        };
        let Executable::Instructions(body) = transaction.instructions() else {
            panic!("instruction fee fixture")
        };
        Quantity::from(crate::gas::meter_instructions(body.as_ref()))
    };
    let success_fee = gas_fee(0);
    let rejected_fee = gas_fee(1);
    let total_fee = success_fee.checked_add(&rejected_fee).unwrap();
    assert!(!success_fee.is_zero() && !rejected_fee.is_zero());
    crate::sumeragi::witness::start_block();
    let mut block = state.block(source.header());
    assert!(
        block
            .world
            .governance_locks
            .get("owned-inventory-expiry")
            .is_none()
    );
    let captures = block.captured_fastpq_transcript_sources().unwrap();
    assert_eq!(captures.len(), 1);
    let protocol = *captures.keys().next().unwrap();
    assert!(captures[&protocol].is_protocol_purpose());
    let release = &block.fastpq_transcripts[&protocol][0].deltas[0];
    assert_eq!(release.from_account, *BOB_ID);
    assert_eq!(release.to_account, *ALICE_ID);
    assert_eq!(release.amount, Quantity::one());
    assert_eq!(release.from_balance_before, Quantity::one());
    assert_eq!(release.from_balance_after, Quantity::zero());
    assert_eq!(release.to_balance_before, Quantity::from(1_000_000_010_u64));
    assert_eq!(release.to_balance_after, Quantity::from(1_000_000_011_u64));
    let asset = release.asset_definition.clone();
    execute(&mut block, &source);
    assert_eq!(
        block
            .world
            .assets
            .get(&AssetId::of(asset.clone(), ALICE_ID.clone()))
            .unwrap()
            .0,
        Quantity::from(1_000_000_011_u64)
            .checked_sub(&total_fee)
            .unwrap()
    );
    assert_eq!(
        block
            .world
            .assets
            .get(&AssetId::of(asset, BOB_ID.clone()))
            .unwrap()
            .0,
        total_fee
    );
    let rejected = Hash::from(
        source
            .network_entrypoint_at(1)
            .unwrap()
            .execution_call_hash(),
    );
    assert!(block.fastpq_transcripts.contains_key(&rejected));
    assert!(!block.captured_fastpq_transcript_sources().unwrap()[&rejected].is_protocol_purpose());
    let (ordinary, mandatory) = block.fastpq_source_usage_for_testing();
    assert_eq!(
        (
            ordinary.executed_entries,
            ordinary.transcripts,
            ordinary.deltas
        ),
        (4, 2, 2)
    );
    assert_eq!(
        (
            mandatory.executed_entries,
            mandatory.transcripts,
            mandatory.deltas
        ),
        (1, 1, 1)
    );
    block
        .seal_execution_outputs(&mut source, |block, _, _| seal_metadata(block))
        .unwrap();
    let inventory = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();
    assert_eq!(inventory.entries().len(), 5);
    assert_eq!(inventory.transcript_entry_hashes().len(), 3);
    assert_eq!(inventory.entries()[1].entry_hash, rejected);
    let extra = inventory.entries().last().unwrap();
    assert_eq!(extra.entry_hash, protocol);
    assert_eq!(
        extra.execution_kind,
        FastpqSourceExecutionKindV1::ProtocolPurpose
    );
    assert_eq!(extra.route, FastpqSourceRouteV1::Unrouted);
    let ExecutionOutputV1::Network(success) = &source.execution_outputs()[0] else {
        panic!("Network output")
    };
    assert!(success.result.is_ok());
    let ExecutionOutputV1::Network(rejected_output) = &source.execution_outputs()[1] else {
        panic!("Network output")
    };
    assert!(rejected_output.result.is_err());
    let success = Hash::from(
        source
            .network_entrypoint_at(0)
            .unwrap()
            .execution_call_hash(),
    );
    for (call, fee) in [(success, success_fee), (rejected, rejected_fee)] {
        assert_eq!(source.fastpq_transcripts()[&call].len(), 1);
        let delta = &source.fastpq_transcripts()[&call][0].deltas[0];
        assert_eq!(delta.from_account, *ALICE_ID);
        assert_eq!(delta.to_account, *BOB_ID);
        assert_eq!(delta.amount, fee);
    }
    block.verify_execution_output_seal(&source).unwrap();
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn unknown_internal_capture_and_changed_known_capture_refuse_and_latch() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for mutation in 0..4 {
        let (state, mut source, _, _) = fixture_with_effects(true, false);
        crate::sumeragi::witness::start_block();
        let mut block = state.block(source.header());
        execute(&mut block, &source);
        assert_eq!(block.fastpq_transcripts.len(), 1);
        let internal = *block.fastpq_transcripts.keys().next().unwrap();
        let actual_capture = block.captured_fastpq_transcript_sources().unwrap()[&internal];
        assert!(!actual_capture.is_protocol_purpose());
        assert_eq!(actual_capture.route(), FastpqSourceRouteV1::Unrouted);
        // Corrupt facts captured by the actual Pipeline transfer. These are
        // hostile state mutations, never an admitted replacement source owner.
        block.fastpq_source_captures = Default::default();
        if mutation != 3 {
            let hash = if mutation == 0 {
                Hash::new(b"unowned internal execution call")
            } else {
                internal
            };
            if hash != internal {
                let mut bundle = block.fastpq_transcripts.remove(&internal).unwrap();
                for transcript in &mut bundle {
                    transcript.batch_hash = hash;
                    transcript.poseidon_preimage_digest = None;
                }
                block.fastpq_transcripts.insert(hash, bundle);
            }
            let captured = block
                .fastpq_source_context
                .as_ref()
                .unwrap()
                .capture_transcript(
                    (mutation != 1).then_some(hash),
                    hash,
                    (mutation == 2).then_some(iroha_model_base::topology::LaneId::SINGLE),
                    (mutation == 2).then_some(DataSpaceId::new(9)),
                    usize::try_from(actual_capture.first_fragment_index()).unwrap(),
                );
            block.fastpq_source_captures.record(captured);
        }
        assert!(
            block
                .seal_execution_outputs(&mut source, |block, _, _| seal_metadata(block))
                .is_err()
        );
        let error = block.fastpq_source_inventory().unwrap_err().to_owned();
        let expected = match mutation {
            0 => "FASTPQ transcript has no owned execution call",
            3 => "FASTPQ applied source keys differ from the transcript accumulator",
            _ => "FASTPQ block entry differs from its applied source context",
        };
        assert_eq!(error, expected);
        block.fastpq_transcripts.clear();
        block.fastpq_source_captures = Default::default();
        assert!(
            block
                .seal_execution_outputs(&mut source, |block, _, _| seal_metadata(block))
                .is_err()
        );
        assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
        assert!(
            block
                .verified_fastpq_source_inventory_for_capture()
                .is_err()
        );
        assert!(matches!(
            block.commit().unwrap_err(),
            TransactionsBlockError::ExecutionOutputCapacity
        ));
    }
}

#[test]
fn foreign_proposal_and_frozen_context_refuse_before_digest_mutation() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for mutation in 0..4 {
        let (state, mut source, _, _) = fixture_with_effects(true, false);
        crate::sumeragi::witness::start_block();
        let mut block = state.block(source.header());
        execute(&mut block, &source);
        assert_eq!(block.fastpq_transcripts.len(), 1);
        let transcripts = block.fastpq_transcripts.clone();
        let original_header = block._curr_block;
        let original_network = block.network_id;
        let original_context = block.fastpq_source_context.clone();
        assert!(
            block
                .seal_execution_outputs(&mut source, |block, _, _| {
                    // Mutation inside the finalizer reaches source reconciliation after
                    // the original proposal owner has passed the outer seal preflight.
                    match mutation {
                        0 => {
                            block._curr_block =
                                BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0)
                        }
                        1 => block.fastpq_source_context = None,
                        2 => {
                            let mut changed =
                                (**block.fastpq_source_context.as_ref().unwrap()).clone();
                            changed.source.height += 1;
                            block.fastpq_source_context = Some(Arc::new(changed));
                        }
                        3 => {
                            let foreign_network = NetworkId::from_genesis_hash(
                                BlockHeader::new(NonZeroU64::new(3).unwrap(), None, None, 3, 0)
                                    .hash(),
                            );
                            assert_ne!(foreign_network, original_network);
                            block.network_id = foreign_network;
                            let mut changed =
                                (**block.fastpq_source_context.as_ref().unwrap()).clone();
                            changed.source.network_id = foreign_network;
                            block.fastpq_source_context = Some(Arc::new(changed));
                            assert_ne!(
                                original_context.as_ref().unwrap().source,
                                block.fastpq_source_context.as_ref().unwrap().source
                            );
                        }
                        _ => unreachable!(),
                    }
                    seal_metadata(block)
                })
                .is_err()
        );
        let error = block.fastpq_source_inventory().unwrap_err().to_owned();
        let expected = match mutation {
            0 => "FASTPQ owned sources belong to another proposal",
            1 => "FASTPQ inventory has no frozen source-height context",
            _ => "FASTPQ owned sources differ from the applying source-height context",
        };
        assert_eq!(error, expected);
        assert_eq!(block.fastpq_transcripts, transcripts);
        block._curr_block = original_header;
        block.network_id = original_network;
        block.fastpq_source_context = original_context;
        assert!(
            block
                .seal_execution_outputs(&mut source, |block, _, _| seal_metadata(block))
                .is_err()
        );
        assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
        assert!(
            block
                .verified_fastpq_source_inventory_for_capture()
                .is_err()
        );
        assert!(matches!(
            block.commit().unwrap_err(),
            TransactionsBlockError::ExecutionOutputCapacity
        ));
    }
}

#[test]
fn owned_seal_still_rejects_late_applied_capture_after_transcript_drain() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let (state, mut source, _, _) = fixture_with_effects(true, false);
    crate::sumeragi::witness::start_block();
    let mut block = state.block(source.header());
    execute(&mut block, &source);
    assert_eq!(block.fastpq_transcripts.len(), 1);
    let call = *block.fastpq_transcripts.keys().next().unwrap();
    let captured = block.captured_fastpq_transcript_sources().unwrap()[&call];
    block
        .seal_execution_outputs(&mut source, |block, _, _| seal_metadata(block))
        .unwrap();
    let inventory = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();
    assert!(block.fastpq_transcripts.is_empty());
    assert_eq!(source.fastpq_transcripts().len(), 1);
    assert_eq!(source.fastpq_transcripts()[&call].len(), 1);
    block.verify_execution_output_seal(&source).unwrap();
    // Simulate a hostile post-drain append directly at the captured-fact boundary;
    // no test helper can reopen an admitted source transaction after sealing.
    block.fastpq_source_captures.record(Ok(captured));
    assert_eq!(
        block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap_err(),
        "FASTPQ transcript capture was applied after source sealing"
    );
    assert_eq!(
        block.fastpq_source_inventory().unwrap().unwrap(),
        inventory.as_ref()
    );
    assert!(block.verify_execution_output_seal(&source).is_err());
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}
