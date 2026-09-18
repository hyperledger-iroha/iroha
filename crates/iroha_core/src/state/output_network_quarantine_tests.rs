//! Actual signed-source quarantine admission and immutable selection controls.
//! These exercise the private Network owner, not carrier finality or the retired DAG.

use super::*;
use iroha_data_model::{
    ValidationFail,
    isi::governance::CastPlainBallot,
    transaction::{
        error::TransactionRejectionReason,
        signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
        },
    },
};
use iroha_model_base::metadata::Metadata;
use iroha_primitives::numeric::Quantity;

fn write_quarantine(key: &str, value: u64) -> InstructionBox {
    SetKeyValue::account(ALICE_ID.clone(), key.parse().unwrap(), Json::new(value)).into()
}
fn signed_quarantine_input(
    state: &State,
    instructions: Vec<InstructionBox>,
    classification: Option<Json>,
    batch: bool,
    attempt: u64,
    future: bool,
) -> TransactionEntrypoint {
    let mut metadata = Metadata::default();
    if let Some(value) = classification {
        metadata.insert("quarantine".parse().unwrap(), value);
    }
    metadata.insert("ranking_attempt".parse().unwrap(), Json::new(attempt));
    let mut builder = TransactionBuilder::new(
        state.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(vec![], None),
    );
    builder.set_creation_time(if future {
        Duration::from_secs(1_000_000)
    } else {
        Duration::from_millis(1)
    });
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
        builder
            .with_metadata(metadata)
            .with_executable(executable)
            .sign(ALICE_KEYPAIR.private_key()),
    )
}
fn assert_plain_success(row: &NetworkExecutionOutputV1, index: usize) {
    assert_eq!(
        row,
        &NetworkExecutionOutputV1 {
            input_index: u32::try_from(index).unwrap(),
            result: TransactionResult::new(Ok(vec![])),
            completions: vec![],
        }
    );
}
fn assert_quarantine_overflow(row: &NetworkExecutionOutputV1, index: usize) {
    assert_eq!(
        row,
        &NetworkExecutionOutputV1 {
            input_index: u32::try_from(index).unwrap(),
            result: TransactionResult::new(Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("quarantine overflow".into())
            ))),
            completions: vec![],
        }
    );
}
fn earlier_ranked(
    state: &State,
    instructions: Vec<InstructionBox>,
    later: &TransactionEntrypoint,
    future: bool,
) -> TransactionEntrypoint {
    (0..1024)
        .find_map(|attempt| {
            let candidate = signed_quarantine_input(
                state,
                instructions.clone(),
                Some(Json::new(true)),
                false,
                attempt,
                future,
            );
            (candidate.hash() < later.hash()).then_some(candidate)
        })
        .expect("deterministic signed fixture finds an earlier actual source hash")
}

#[test]
fn actual_quarantine_zero_exact_and_overflow_quota_own_complete_rows_and_effects() {
    let _guard = witness::exec_witness_guard();
    for quota in [0, 1, 3] {
        let mut state = fixture(65_536, None);
        state.pipeline.quarantine_max_txs_per_block = quota;
        let inputs: Vec<_> = (0..3)
            .map(|index| {
                signed_quarantine_input(
                    &state,
                    vec![
                        write_quarantine(&format!("quota_{index}"), index),
                        write_quarantine("last_quota_effect", index),
                    ],
                    Some(Json::new(true)),
                    false,
                    index,
                    false,
                )
            })
            .collect();
        let mut rank: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| (input.hash(), index))
            .collect();
        rank.sort_unstable();
        let selected: Vec<_> = rank
            .into_iter()
            .take(quota)
            .map(|(_, index)| index)
            .collect();
        let source = carrier(inputs);
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        assert_eq!(retained(&block).rows.len(), 3);
        for index in 0..3 {
            let row = network_row(&block, index);
            if selected.contains(&index) {
                assert_plain_success(row, index);
            } else {
                assert_quarantine_overflow(row, index);
            }
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get(format!("quota_{index}").as_str()),
                selected
                    .contains(&index)
                    .then(|| Json::new(index as u64))
                    .as_ref()
            );
        }
        assert_eq!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("last_quota_effect"),
            selected
                .iter()
                .max()
                .map(|index| Json::new(*index as u64))
                .as_ref()
        );
        assert_eq!(block.committed_fragment_count(), fragments + quota);
        assert_eq!(block.gas_used_in_block > 0, quota != 0);
    }
}

#[test]
fn hash_ranked_selection_is_mode_independent_and_does_not_reorder_actual_effects() {
    let _guard = witness::exec_witness_guard();
    let seed = fixture(65_536, None);
    let inputs: Vec<_> = (0..3)
        .map(|index| {
            signed_quarantine_input(
                &seed,
                vec![
                    write_quarantine("permutation_last", index),
                    write_quarantine(&format!("permutation_{index}"), index),
                ],
                Some(Json::new(true)),
                false,
                index,
                false,
            )
        })
        .collect();
    let mut hashes: Vec<_> = inputs.iter().map(TransactionEntrypoint::hash).collect();
    hashes.sort_unstable();
    let selected = &hashes[..2];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut mode_baseline = None;
        for parallel in [false, true] {
            let mut state = fixture(65_536, None);
            assert_eq!(state.network_id, seed.network_id);
            state.pipeline.parallel_apply = parallel;
            state.pipeline.quarantine_max_txs_per_block = 2;
            let source = carrier(order.iter().map(|index| inputs[*index].clone()).collect());
            witness::start_block();
            let mut block = state.block(source.header());
            execute(&mut block, &source).unwrap();
            let mut actual = Vec::new();
            for (position, original) in order.iter().copied().enumerate() {
                let selected_source = selected.contains(&inputs[original].hash());
                if selected_source {
                    assert_plain_success(network_row(&block, position), position);
                    actual.push(inputs[original].hash());
                } else {
                    assert_quarantine_overflow(network_row(&block, position), position);
                }
                assert_eq!(
                    block
                        .world
                        .account(&ALICE_ID)
                        .unwrap()
                        .metadata()
                        .get(format!("permutation_{original}").as_str())
                        .is_some(),
                    selected_source
                );
            }
            actual.sort_unstable();
            assert_eq!(actual.as_slice(), selected);
            let last = order
                .iter()
                .rev()
                .find(|index| selected.contains(&inputs[**index].hash()))
                .unwrap();
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get("permutation_last"),
                Some(&Json::new(*last as u64))
            );
            assert!(block.gas_used_in_block > 0);
            let observed = (retained(&block).rows.clone(), block.gas_used_in_block);
            if let Some(expected) = &mode_baseline {
                assert_eq!(&observed, expected);
            } else {
                mode_baseline = Some(observed);
            }
        }
    }
}

#[test]
fn only_exact_signed_boolean_true_uses_the_disabled_quarantine_quota() {
    let _guard = witness::exec_witness_guard();
    let mut state = fixture(65_536, None);
    state.pipeline.quarantine_max_txs_per_block = 0;
    let classifications = [
        Some(Json::new(true)),
        Some(Json::new(false)),
        Some(Json::new("true")),
        Some(Json::new(1)),
        None,
    ];
    let source = carrier(
        classifications
            .into_iter()
            .enumerate()
            .map(|(index, value)| {
                signed_quarantine_input(
                    &state,
                    vec![write_quarantine(
                        &format!("classification_{index}"),
                        index as u64,
                    )],
                    value,
                    false,
                    index as u64,
                    false,
                )
            })
            .collect(),
    );
    witness::start_block();
    let mut block = state.block(source.header());
    execute(&mut block, &source).unwrap();
    assert_quarantine_overflow(network_row(&block, 0), 0);
    for index in 1..5 {
        assert_plain_success(network_row(&block, index), index);
    }
    for index in 0..5 {
        assert_eq!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(format!("classification_{index}").as_str())
                .is_some(),
            index != 0
        );
    }
    assert!(block.gas_used_in_block > 0);
}

#[test]
fn stateless_invalid_lower_hash_does_not_consume_a_quarantine_slot() {
    let _guard = witness::exec_witness_guard();
    let mut state = fixture(65_536, None);
    state.pipeline.quarantine_max_txs_per_block = 1;
    let healthy = signed_quarantine_input(
        &state,
        vec![write_quarantine("valid_quota", 1)],
        Some(Json::new(true)),
        false,
        1,
        false,
    );
    let invalid = earlier_ranked(
        &state,
        vec![write_quarantine("invalid_quota", 1)],
        &healthy,
        true,
    );
    assert!(invalid.hash() < healthy.hash());
    let source = carrier(vec![invalid, healthy]);
    witness::start_block();
    let mut block = state.block(source.header());
    execute(&mut block, &source).unwrap();
    let row = network_row(&block, 0);
    assert!(
        matches!(row.result.as_ref(), Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason))) if reason == "transaction creation time 1000000000 is not earlier than block creation time 2")
    );
    assert!(row.result.batch_transfer_outcomes().is_empty());
    assert!(row.completions.is_empty());
    assert_plain_success(network_row(&block, 1), 1);
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("invalid_quota")
            .is_none()
    );
    assert_eq!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("valid_quota"),
        Some(&Json::new(1_u64))
    );
    assert!(block.gas_used_in_block > 0);
}

#[test]
fn actual_business_failure_does_not_refill_the_frozen_quarantine_selection() {
    let _guard = witness::exec_witness_guard();
    let mut state = fixture(65_536, None);
    state.pipeline.quarantine_max_txs_per_block = 1;
    let later = signed_quarantine_input(
        &state,
        vec![write_quarantine("quota_refill", 1)],
        Some(Json::new(true)),
        false,
        1,
        false,
    );
    let rejected = earlier_ranked(
        &state,
        vec![
            write_quarantine("failed_quota_write", 1),
            Unregister::trigger("missing_quarantine_trigger".parse().unwrap()).into(),
        ],
        &later,
        false,
    );
    let source = carrier(vec![rejected, later]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    let row = network_row(&block, 0);
    assert!(row.result.is_err());
    assert!(
        !matches!(row.result.as_ref(), Err(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(reason))) if reason == "quarantine overflow")
    );
    assert!(!retained(&block).rows[0].is_output_limit_rejection());
    assert!(row.result.batch_transfer_outcomes().is_empty());
    assert!(row.completions.is_empty());
    assert_quarantine_overflow(network_row(&block, 1), 1);
    for key in ["failed_quota_write", "quota_refill"] {
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(key)
                .is_none()
        );
    }
    assert_eq!(block.committed_fragment_count(), fragments);
    assert!(block.gas_used_in_block > 0);
}

#[test]
fn healthy_callback_output_overflow_does_not_refill_quarantine_or_apply_effects() {
    let _guard = witness::exec_witness_guard();
    let mut state = fixture(16_384, Some(32_768));
    state.pipeline.quarantine_max_txs_per_block = 1;
    let later = signed_quarantine_input(
        &state,
        vec![write_quarantine("overflow_refill", 1)],
        Some(Json::new(true)),
        false,
        1,
        false,
    );
    let oversized = earlier_ranked(
        &state,
        vec![ExecuteTrigger::new("network_callback".parse().unwrap()).into()],
        &later,
        false,
    );
    let source = carrier(vec![oversized, later]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    assert!(retained(&block).rows[0].is_output_limit_rejection());
    assert!(
        network_row(&block, 0)
            .result
            .batch_transfer_outcomes()
            .is_empty()
    );
    assert!(network_row(&block, 0).completions.is_empty());
    assert_quarantine_overflow(network_row(&block, 1), 1);
    for key in ["callback_write", "overflow_refill"] {
        assert!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get(key)
                .is_none()
        );
    }
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
    assert!(
        block
            .world
            .external_event_buf
            .iter()
            .all(|event| !matches!(event, EventBox::TriggerCompleted(_)))
    );
    assert_eq!(block.committed_fragment_count(), fragments);
    assert!(block.gas_used_in_block > 0);
}

#[test]
fn signed_batch_and_ballot_cannot_bypass_a_zero_quarantine_quota() {
    let _guard = witness::exec_witness_guard();
    let mut state = fixture(65_536, None);
    state.pipeline.quarantine_max_txs_per_block = 0;
    let ballot: InstructionBox = CastPlainBallot {
        referendum_id: "quarantine_ballot".to_owned(),
        direction: 0,
        owner: ALICE_ID.clone(),
        amount: Quantity::from(1_u32),
        duration_blocks: 100,
    }
    .into();
    // The ballot is a real signed executable shape. Quota refusal must precede
    // permission/referendum/business validation; this does not claim a valid vote.
    let source = carrier(vec![
        signed_quarantine_input(
            &state,
            vec![write_quarantine("batch_bypass", 1)],
            Some(Json::new(true)),
            true,
            1,
            false,
        ),
        signed_quarantine_input(&state, vec![ballot], Some(Json::new(true)), false, 2, false),
    ]);
    witness::start_block();
    let mut block = state.block(source.header());
    let fragments = block.committed_fragment_count();
    execute(&mut block, &source).unwrap();
    for index in 0..2 {
        assert_quarantine_overflow(network_row(&block, index), index);
    }
    assert!(
        block
            .world
            .account(&ALICE_ID)
            .unwrap()
            .metadata()
            .get("batch_bypass")
            .is_none()
    );
    assert!(
        block
            .world
            .governance_locks()
            .get(&"quarantine_ballot".to_owned())
            .is_none()
    );
    assert!(
        block
            .world
            .governance_slashes()
            .get(&"quarantine_ballot".to_owned())
            .is_none()
    );
    assert_eq!(block.gas_used_in_block, 0);
    assert_eq!(block.committed_fragment_count(), fragments);
}

#[test]
fn sealed_reveal_quota_uses_actual_pending_commitments_and_outer_source_hashes() {
    let _guard = witness::exec_witness_guard();
    for quota in [0, 1] {
        let mut state = fixture(65_536, None);
        state.pipeline.quarantine_max_txs_per_block = quota;
        let mut commits = Vec::new();
        let mut reveals = Vec::new();
        for index in 0..2 {
            let TransactionEntrypoint::External(signed) = signed_quarantine_input(
                &state,
                vec![write_quarantine(&format!("sealed_quota_{index}"), index)],
                Some(Json::new(true)),
                false,
                index,
                false,
            ) else {
                unreachable!()
            };
            let salt = [u8::try_from(index + 1).unwrap(); 32];
            let commitment =
                compute_sealed_transaction_commitment(&state.network_id, &signed, salt, 9);
            let commit = SignedSealedTransactionCommitment::sign(
                SealedTransactionCommitmentPayload::new(
                    state.network_id,
                    ALICE_ID.clone(),
                    commitment,
                    3,
                    9,
                    None,
                ),
                ALICE_KEYPAIR.private_key(),
            );
            commit.verify_signature().unwrap();
            commits.push(TransactionEntrypoint::SealedCommitment(commit));
            reveals.push(TransactionEntrypoint::SealedReveal(
                SealedTransactionReveal::new(commitment, signed, salt),
            ));
        }
        let source = carrier(commits);
        witness::start_block();
        let mut committing = state.block(source.header());
        let prior_pending_count = committing.world.smart_contract_state().iter().count();
        execute(&mut committing, &source).unwrap();
        for index in 0..2 {
            assert_plain_success(network_row(&committing, index), index);
        }
        let pending: Vec<_> = committing
            .world
            .smart_contract_state()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(pending.len(), prior_pending_count + 2);
        // Retain actual commitment instruction writes through the existing world-only
        // fixture helper. No fake pending record, finalized block, or QC is installed.
        committing.commit_world_overlay_for_testing().unwrap();
        let selected = reveals
            .iter()
            .enumerate()
            .min_by_key(|(_, input)| input.hash())
            .unwrap()
            .0;
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(3).unwrap(),
            None,
            None,
            3,
            0,
        ));
        for reveal in &reveals {
            let TransactionEntrypoint::SealedReveal(reveal) = reveal else {
                unreachable!()
            };
            builder.push_sealed_transaction_reveal(reveal.clone());
        }
        let source = builder.build_with_signature(0, ALICE_KEYPAIR.private_key());
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        for index in 0..2 {
            if quota == 1 && index == selected {
                assert_plain_success(network_row(&block, index), index);
            } else {
                assert_quarantine_overflow(network_row(&block, index), index);
            }
            assert_eq!(
                block
                    .world
                    .account(&ALICE_ID)
                    .unwrap()
                    .metadata()
                    .get(format!("sealed_quota_{index}").as_str())
                    .is_some(),
                quota == 1 && index == selected
            );
        }
        let retained_pending: Vec<_> = block
            .world
            .smart_contract_state()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(retained_pending.len(), pending.len() - quota);
        if quota == 0 {
            assert_eq!(retained_pending, pending);
        }
        assert_eq!(block.committed_fragment_count(), fragments + quota);
        assert_eq!(block.gas_used_in_block > 0, quota != 0);
    }
}

#[test]
fn real_nexus_fee_is_not_charged_for_quota_refusal_before_business_execution() {
    use iroha_data_model::{
        asset::{AssetDefinitionId, AssetId},
        transaction::{FeeChargeKind, FeeChargeLimit},
    };
    let _guard = witness::exec_witness_guard();
    let _fee_guard = crate::sumeragi::status::nexus_fee_test_lock()
        .lock()
        .unwrap();
    for quota in [0, 1] {
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("network-fee", "universal").unwrap(),
            "quarantine_fee".parse().unwrap(),
        );
        let mut state = fixture_with_fee_asset(65_536, None, Some(asset.clone()));
        state.pipeline.quarantine_max_txs_per_block = quota;
        let mut metadata = Metadata::default();
        metadata.insert("quarantine".parse().unwrap(), Json::new(true));
        let mut builder = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(
                vec![FeeChargeLimit::new(
                    FeeChargeKind::Nexus,
                    asset.clone(),
                    Quantity::from(1_u32),
                )],
                None,
            ),
        );
        builder.set_creation_time(Duration::from_millis(1));
        let source = carrier(vec![TransactionEntrypoint::External(
            builder
                .with_metadata(metadata)
                .with_instructions([write_quarantine("paid_quarantine_effect", 1)])
                .sign(ALICE_KEYPAIR.private_key()),
        )]);
        witness::start_block();
        let mut block = state.block(source.header());
        let fragments = block.committed_fragment_count();
        execute(&mut block, &source).unwrap();
        if quota == 0 {
            assert_quarantine_overflow(network_row(&block, 0), 0);
        } else {
            assert_plain_success(network_row(&block, 0), 0);
        }
        let expected = Quantity::from(if quota == 0 { 10_u32 } else { 9 });
        assert_eq!(
            block
                .world
                .assets()
                .get(&AssetId::of(asset.clone(), ALICE_ID.clone()))
                .unwrap()
                .0,
            expected
        );
        assert_eq!(
            block
                .world
                .asset_definition(&asset)
                .unwrap()
                .total_quantity(),
            &expected
        );
        assert!(
            block
                .world
                .assets()
                .get(&AssetId::of(asset, iroha_test_samples::BOB_ID.clone()))
                .is_none_or(|balance| balance.0 == Quantity::zero())
        );
        assert_eq!(
            block
                .world
                .account(&ALICE_ID)
                .unwrap()
                .metadata()
                .get("paid_quarantine_effect")
                .is_some(),
            quota == 1
        );
        assert_eq!(block.gas_used_in_block > 0, quota == 1);
        assert_eq!(block.committed_fragment_count(), fragments + quota);
    }
}
