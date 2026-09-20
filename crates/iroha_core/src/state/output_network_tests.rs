//! Actual signed Network execution under the private output owner.
//! These unit fixtures establish execution behavior, not carrier finality or DA.

use super::*;
use crate::{
    governance::manifest::{LaneManifestRegistry, LaneManifestStatus},
    state::WorldReadOnly,
};
use iroha_data_model::{
    account::Account,
    events::{EventBox, execute_trigger::ExecuteTriggerEventFilter},
    isi::{ExecuteTrigger, Register, SetKeyValue, Unregister},
    transaction::{Executable, ExecutableBatchItem},
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_model_base::domain::DomainId;
use std::{sync::Arc, time::Duration};

fn install_routes(state: &State) {
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
}

fn fixture(row_bytes: u64, callback_bytes: Option<usize>) -> State {
    fixture_with_fee_asset(row_bytes, callback_bytes, None)
}

fn fixture_with_fee_asset(
    row_bytes: u64,
    callback_bytes: Option<usize>,
    fee_asset: Option<iroha_data_model::asset::AssetDefinitionId>,
) -> State {
    let mut state = state(row_bytes);
    install_routes(&state);
    if let Some(asset) = &fee_asset {
        use iroha_primitives::numeric::Quantity;
        let fees = &mut state.nexus.get_mut().fees;
        fees.base_fee = Quantity::from(1_u32);
        fees.per_byte_fee = Quantity::zero();
        fees.per_instruction_fee = Quantity::zero();
        fees.per_gas_unit_fee = Quantity::zero();
        fees.fee_asset_id = asset.to_string();
        fees.fee_sink_account_id = iroha_test_samples::BOB_ID.to_string();
    }
    let mut setup = state.block(BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0));
    let mut transaction = setup.transaction();
    Register::account(Account::new(ALICE_ID.clone()))
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
    if let Some(asset) = fee_asset {
        use iroha_data_model::{
            asset::{AssetBalancePolicy, AssetDefinition, AssetId},
            domain::Domain,
            isi::Mint,
        };
        use iroha_primitives::numeric::Quantity;
        Register::account(Account::new(iroha_test_samples::BOB_ID.clone()))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        Register::domain(Domain::new(
            DomainId::try_new("network-fee", "universal").unwrap(),
        ))
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
        Register::asset_definition(AssetDefinition::numeric(
            asset.clone(),
            "Network fee".to_owned(),
            AssetBalancePolicy::Global,
            None,
        ))
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
        Mint::asset_quantity(Quantity::from(10_u32), AssetId::of(asset, ALICE_ID.clone()))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
    }
    if let Some(bytes) = callback_bytes {
        let id: TriggerId = "network_callback".parse().unwrap();
        let action = Action::new(
            vec![
                InstructionBox::from(SetKeyValue::account(
                    ALICE_ID.clone(),
                    "callback_write".parse().unwrap(),
                    Json::new(7),
                )),
                Log::new(Level::DEBUG, "x".repeat(bytes)).into(),
            ],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new()
                .for_trigger(id.clone())
                .under_authority(ALICE_ID.clone()),
        )
        .unwrap();
        Register::trigger(Trigger::new(id, action))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
    }
    transaction.apply();
    setup.commit_world_overlay_for_testing().unwrap();
    state
}

fn input(
    state: &State,
    instructions: Vec<InstructionBox>,
    fee: FeePaymentIntent,
    batch: bool,
) -> TransactionEntrypoint {
    let mut tx = TransactionBuilder::new(state.network_id, ALICE_ID.clone(), fee);
    tx.set_creation_time(Duration::from_millis(1));
    let executable = if batch {
        Executable::Batch(
            instructions
                .into_iter()
                .map(ExecutableBatchItem::Instruction)
                .collect::<Vec<_>>()
                .into(),
        )
    } else {
        Executable::Instructions(instructions.into())
    };
    TransactionEntrypoint::External(
        tx.with_executable(executable)
            .sign(ALICE_KEYPAIR.private_key()),
    )
}

fn carrier(inputs: Vec<TransactionEntrypoint>) -> SignedBlock {
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        0,
    ));
    for input in inputs {
        match input {
            TransactionEntrypoint::External(tx) => {
                builder.push_transaction(tx);
            }
            TransactionEntrypoint::SealedCommitment(tx) => {
                builder.push_sealed_transaction_commitment(tx);
            }
            TransactionEntrypoint::SealedReveal(tx) => {
                builder.push_sealed_transaction_reveal(tx);
            }
        }
    }
    builder.build_with_signature(0, ALICE_KEYPAIR.private_key())
}

fn execute(block: &mut StateBlock<'_>, source: &SignedBlock) -> Result<(), String> {
    block.reserve_ordinary_execution_outputs(source)?;
    block.produce_ordinary_execution_outputs(source, |producer| {
        producer.execute_network_sources(None)?;
        for index in 0..source.network_entrypoint_count() {
            assert!(producer.network_route(index).is_some());
        }
        assert!(
            producer
                .network_route(source.network_entrypoint_count())
                .is_none()
        );
        finish_empty_internal(producer)
    })
}

fn network_row<'a>(block: &'a StateBlock<'_>, index: usize) -> &'a NetworkExecutionOutputV1 {
    let ExecutionOutputV1::Network(row) = &retained(block).rows[index] else {
        panic!("Network row")
    };
    row
}

#[test]
fn actual_signed_sources_apply_once_in_original_output_positions() {
    let _guard = witness::exec_witness_guard();
    let state = fixture(65_536, None);
    let source = carrier(
        (0..2)
            .map(|index| {
                input(
                    &state,
                    vec![
                        SetKeyValue::account(
                            ALICE_ID.clone(),
                            format!("network_{index}").parse().unwrap(),
                            Json::new(index),
                        )
                        .into(),
                    ],
                    FeePaymentIntent::authority(vec![], None),
                    false,
                )
            })
            .collect(),
    );
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert_eq!(block.committed_fragment_count(), fragments + 2);
    for index in 0..2 {
        let row = network_row(&block, index);
        assert_eq!(row.input_index as usize, index);
        assert!(row.result.is_ok(), "{:?}", row.result);
        assert!(row.completions.is_empty());
        let key: Name = format!("network_{index}").parse().unwrap();
        assert_eq!(
            block.world.account(&ALICE_ID).unwrap().metadata().get(&key),
            Some(&Json::new(index as i32))
        );
    }
    assert!(block.gas_used_in_block > 0);
    assert!(execute(&mut block, &source).is_err());
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn actual_callback_fits_exactly_or_rolls_back_before_applying() {
    let _guard = witness::exec_witness_guard();
    let mut exact = None;
    for case in 0..3 {
        let bytes = match case {
            0 => 65_536,
            1 => exact.unwrap(),
            _ => exact.unwrap() - 1,
        };
        let state = fixture(bytes, Some(32_768));
        let source = carrier(vec![input(
            &state,
            vec![ExecuteTrigger::new("network_callback".parse().unwrap()).into()],
            FeePaymentIntent::authority(vec![], None),
            false,
        )]);
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        let row = network_row(&block, 0);
        let key: Name = "callback_write".parse().unwrap();
        if case < 2 {
            assert!(row.result.is_ok(), "{:?}", row.result);
            assert_eq!(row.result.as_ref().unwrap().len(), 1);
            assert_eq!(row.completions.len(), 1);
            assert_eq!(
                block.world.account(&ALICE_ID).unwrap().metadata().get(&key),
                Some(&Json::new(7))
            );
            assert_eq!(block.committed_fragment_count(), fragments + 1);
            let measured = norito::canonical_frame_len(&retained(&block).rows[0]).unwrap() as u64;
            if let Some(expected) = exact {
                assert_eq!(measured, expected);
            } else {
                exact = Some(measured);
            }
        } else {
            assert!(retained(&block).rows[0].is_output_limit_rejection());
            assert!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get(&key)
                    .is_none()
            );
            assert_eq!(block.committed_fragment_count(), fragments);
            assert!(
                block
                    .world
                    .external_event_buf
                    .iter()
                    .all(|event| !matches!(event, EventBox::TriggerCompleted(_)))
            );
            assert_eq!(
                block
                    .world
                    .triggers
                    .by_call_triggers()
                    .get(&"network_callback".parse().unwrap())
                    .unwrap()
                    .repeats,
                Repeats::Exactly(1)
            );
            assert!(witness::snapshot_exec_witness().writes.is_empty());
        }
        assert!(block.gas_used_in_block > 0);
    }
}

#[test]
fn real_business_rejection_wins_after_oversized_callback_and_discards_capture() {
    let _guard = witness::exec_witness_guard();
    let missing = DomainId::try_new("missing-network-domain", "universal").unwrap();
    for bytes in [16_384, 65_536] {
        let state = fixture(bytes, Some(32_768));
        let source = carrier(vec![input(
            &state,
            vec![
                ExecuteTrigger::new("network_callback".parse().unwrap()).into(),
                Unregister::domain(missing.clone()).into(),
            ],
            FeePaymentIntent::authority(vec![], None),
            false,
        )]);
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        let row = network_row(&block, 0);
        assert!(
            matches!(row.result.as_ref(), Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::InstructionFailed(iroha_data_model::isi::error::InstructionExecutionError::Find(
                iroha_data_model::query::error::FindError::Domain(id)))
        )) if id == &missing),
            "{:?}",
            row.result
        );
        assert!(!retained(&block).rows[0].is_output_limit_rejection());
        assert!(row.completions.is_empty());
        assert!(row.result.batch_transfer_outcomes().is_empty());
        let key: Name = "callback_write".parse().unwrap();
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(&key)
                .is_none()
        );
        assert_eq!(block.committed_fragment_count(), fragments);
        assert!(
            block
                .world
                .external_event_buf
                .iter()
                .all(|event| !matches!(event, EventBox::TriggerCompleted(_)))
        );
        assert_eq!(
            block
                .world
                .triggers
                .by_call_triggers()
                .get(&"network_callback".parse().unwrap())
                .unwrap()
                .repeats,
            Repeats::Exactly(1)
        );
        assert!(witness::snapshot_exec_witness().writes.is_empty());
    }
}

#[test]
fn block_gas_admission_rejects_before_business_or_transaction_gas() {
    let _guard = witness::exec_witness_guard();
    let state = fixture(65_536, Some(1024));
    let source = carrier(vec![input(
        &state,
        vec![ExecuteTrigger::new("network_callback".parse().unwrap()).into()],
        FeePaymentIntent::authority(vec![], None),
        false,
    )]);
    witness::start_block();
    let mut block = state.block(source.header());
    // ExecuteTrigger is rejected by the real pre-body gas admission guard.
    // This does not exercise the owner's final post-success gas fallback.
    block.gas_limit_per_block = 1;
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(
        matches!(network_row(&block, 0).result.as_ref(), Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
        iroha_data_model::ValidationFail::NotPermitted(reason))) if reason.starts_with("block gas limit exceeded:"))
    );
    assert_eq!(block.gas_used_in_block, 0);
    assert_eq!(block.committed_fragment_count(), fragments);
    let key: Name = "callback_write".parse().unwrap();
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get(&key)
            .is_none()
    );
    assert!(network_row(&block, 0).completions.is_empty());
}

#[test]
fn stateless_rejection_does_not_execute_its_business_instructions() {
    let _guard = witness::exec_witness_guard();
    let state = fixture(65_536, None);
    let mut tx = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    );
    tx.set_creation_time(Duration::from_secs(1_000_000));
    let source = carrier(vec![TransactionEntrypoint::External(
        tx.with_instructions([SetKeyValue::account(
            ALICE_ID.clone(),
            "future_write".parse().unwrap(),
            Json::new(1),
        )])
        .sign(ALICE_KEYPAIR.private_key()),
    )]);
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(
        matches!(network_row(&block, 0).result.as_ref(), Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
        iroha_data_model::ValidationFail::NotPermitted(reason))) if reason == "transaction creation time 1000000000 is not earlier than block creation time 2")
    );
    let key: Name = "future_write".parse().unwrap();
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get(&key)
            .is_none()
    );
    assert_eq!(block.committed_fragment_count(), fragments);
    assert_eq!(block.gas_used_in_block, 0);
}

#[test]
fn rejected_live_batch_rolls_back_business_and_applies_only_its_actual_fee_fragment() {
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        transaction::{FeeChargeKind, FeeChargeLimit},
    };
    use iroha_primitives::numeric::Quantity;
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    crate::sumeragi::status::reset_nexus_economics_for_tests();
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("network-fee", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let state = fixture_with_fee_asset(65_536, None, Some(asset.clone()));
    let missing = DomainId::try_new("missing-network-fee-domain", "universal").unwrap();
    let fee = FeePaymentIntent::authority(
        vec![FeeChargeLimit::new(
            FeeChargeKind::Nexus,
            asset.clone(),
            Quantity::from(1_u32),
        )],
        None,
    );
    let source = carrier(vec![input(
        &state,
        vec![
            SetKeyValue::account(
                ALICE_ID.clone(),
                "fee_business_write".parse().unwrap(),
                Json::new(1),
            )
            .into(),
            Unregister::domain(missing.clone()).into(),
        ],
        fee,
        true,
    )]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    let row = network_row(&block, 0);
    assert!(
        matches!(row.result.as_ref(), Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
        iroha_data_model::ValidationFail::InstructionFailed(iroha_data_model::isi::error::InstructionExecutionError::Find(
            iroha_data_model::query::error::FindError::Domain(id)))
    )) if id == &missing),
        "{:?}",
        row.result
    );
    assert!(row.completions.is_empty());
    assert!(row.result.batch_transfer_outcomes().is_empty());
    let key: Name = "fee_business_write".parse().unwrap();
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get(&key)
            .is_none()
    );
    assert_eq!(
        block
            .world
            .assets()
            .get(&AssetId::of(asset, ALICE_ID.clone()))
            .unwrap()
            .0,
        Quantity::from(9_u32)
    );
    assert_eq!(
        block.committed_fragment_count(),
        fragments + 1,
        "only the independently applied fee fragment survives"
    );
    assert!(block.gas_used_in_block > 0);
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[test]
fn ordinary_owner_refuses_merge_control_before_any_execution_continuation() {
    use iroha_data_model::{
        block::{BlockExecutionContextBundle, CertifiedMergeLedgerReference},
        merge::MergeQuorumCertificate,
    };
    let state = fixture(65_536, None);
    let validators = Vec::<iroha_model_base::peer::PeerId>::new();
    // Deliberately untrusted control: source exclusion must not consume it as
    // an empty ordinary carrier or attempt to turn its shape into authority.
    let reference = CertifiedMergeLedgerReference {
        version: 1,
        entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"merge-control")),
        encoded_len: 1,
        epoch_id: 1,
        execution_batch_hash: None,
        entrypoint_count: None,
        entrypoint_merkle_root: None,
        result_merkle_root: None,
        base_state_height: None,
        base_state_hash: None,
        merge_qc: MergeQuorumCertificate::new(
            0,
            1,
            2,
            HashOf::from_untyped_unchecked(Hash::new(b"parent")),
            state.network_id,
            1,
            HashOf::new(&validators),
            validators,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Hash::new(b"untrusted"),
        ),
    };
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(2).unwrap(),
        None,
        None,
        2,
        0,
    ));
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(reference),
    ));
    let source = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
    let mut block = state.block(source.header());
    block.reserve_ordinary_execution_outputs(&source).unwrap();
    let fragments = block.committed_fragment_count();
    assert!(
        block
            .produce_ordinary_execution_outputs(&source, |_| panic!(
                "foreign source must not enter continuation"
            ))
            .is_err()
    );
    assert_eq!(block.committed_fragment_count(), fragments);
    assert!(matches!(
        block.commit().unwrap_err(),
        TransactionsBlockError::ExecutionOutputCapacity
    ));
}

#[path = "output_network_penalty_tests.rs"]
mod penalties;

#[path = "output_pipeline_tests.rs"]
mod pipeline;

#[path = "output_seal_tests.rs"]
mod seal;

#[path = "output_network_fee_tests.rs"]
mod fees;

#[path = "output_network_effect_tests.rs"]
mod effects;

#[path = "output_network_quarantine_tests.rs"]
mod quarantine;

#[test]
fn frozen_fraud_admission_refuses_before_business_work_and_grace_preserves_execution() {
    for grace in [Duration::ZERO, Duration::from_secs(1)] {
        let _guard = witness::exec_witness_guard();
        let mut state = fixture(65_536, None);
        state.fraud_monitoring.enabled = true;
        state.fraud_monitoring.required_minimum_band =
            Some(iroha_config::parameters::actual::FraudRiskBand::Low);
        state.fraud_monitoring.missing_assessment_grace = grace;
        let source = carrier(vec![input(
            &state,
            vec![
                SetKeyValue::account(
                    ALICE_ID.clone(),
                    "fraud_effect".parse().unwrap(),
                    Json::new(1_u32),
                )
                .into(),
            ],
            FeePaymentIntent::authority(vec![], None),
            false,
        )]);
        let mut block = state.block(source.header());
        let before = block.committed_fragment_count();
        witness::start_block();
        execute(&mut block, &source).unwrap();
        let result = network_row(&block, 0);
        if grace.is_zero() {
            assert!(
                matches!(result.result.as_ref(), Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(reason))) if reason == "fraud monitoring requires an attached assessment")
            );
            assert_eq!(block.committed_fragment_count(), before);
            assert_eq!(block.gas_used_in_block, 0);
            assert!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("fraud_effect")
                    .is_none()
            );
        } else {
            assert!(result.result.is_ok());
            assert_eq!(block.committed_fragment_count(), before + 1);
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("fraud_effect"),
                Some(&Json::new(1_u32))
            );
        }
    }
}

#[test]
fn ordinary_signed_creation_time_must_precede_its_actual_carrier() {
    for created_at in [1_u64, 2, 3] {
        let _guard = witness::exec_witness_guard();
        let state = fixture(65_536, None);
        let mut builder = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(vec![], None),
        );
        builder.set_creation_time(Duration::from_millis(created_at));
        let source = carrier(vec![TransactionEntrypoint::External(
            builder
                .with_instructions([SetKeyValue::account(
                    ALICE_ID.clone(),
                    "source_time_effect".parse().unwrap(),
                    Json::new(1_u32),
                )])
                .sign(ALICE_KEYPAIR.private_key()),
        )]);
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        witness::start_block();
        execute(&mut block, &source).unwrap();
        let row = network_row(&block, 0);
        assert_eq!(row.input_index, 0);
        assert!(row.completions.is_empty());
        assert!(row.result.batch_transfer_outcomes().is_empty());
        if created_at == 1 {
            assert!(row.result.is_ok());
            assert_eq!(block.committed_fragment_count(), fragments + 1);
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("source_time_effect"),
                Some(&Json::new(1_u32))
            );
        } else {
            assert!(matches!(row.result.as_ref(),
                Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::NotPermitted(reason)))
                if reason == &format!("transaction creation time {created_at} is not earlier than block creation time 2")));
            assert_eq!(block.committed_fragment_count(), fragments);
            assert_eq!(block.gas_used_in_block, 0);
            assert!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("source_time_effect")
                    .is_none()
            );
        }
    }
}
