// Real independent ISI execution keeps eligibility and occurrence grouping in leg order.
use super::*;

fn entry(
    id: &str,
    from: &AccountId,
    to: &AccountId,
    definition: &AssetDefinitionId,
    amount: Quantity,
) -> TransferAssetBatchEntry {
    TransferAssetBatchEntry::with_leg_id(id, from.clone(), to.clone(), definition.clone(), amount)
}

fn statuses(tx: &StateTransaction<'_, '_>) -> Vec<bool> {
    tx.world
        .internal_event_buf
        .iter()
        .filter_map(|event| match event.as_ref() {
            DataEvent::Domain(DomainEvent::Asset(ScopedAsset {
                event: AssetEvent::BatchTransferOutcome(outcome),
                ..
            })) => Some(matches!(
                &outcome.status,
                iroha_data_model::events::data::prelude::AssetBatchTransferLegStatus::Applied
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn independent_later_funded_legs_use_prior_applied_balances_and_one_occurrence() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, alice_asset) = build_asset_transfer_control_test_state(10);
    let bob_asset = AssetId::new(definition.clone(), BOB_ID.clone());
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 7, 0);
    let mut block = state.block(header);
    let hash;
    {
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0x81);
        hash = tx.tx_call_hash.unwrap();
        tx.world.add_account_permission(
            &ALICE_ID,
            Permission::from(
                iroha_executor_data_model::permission::asset::CanTransferAsset {
                    asset: bob_asset.clone(),
                },
            ),
        );
        TransferAssetBatch::independent(vec![
            entry("unfunded", &BOB_ID, &ALICE_ID, &definition, Quantity::one()),
            entry(
                "fund",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(5_u32),
            ),
            entry(
                "spend",
                &BOB_ID,
                &ALICE_ID,
                &definition,
                Quantity::from(3_u32),
            ),
            entry(
                "repeat",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(8_u32),
            ),
        ])
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert_eq!(statuses(&tx), vec![false, true, true, true]);
        assert_eq!(asset_balance_or_zero(&tx, &alice_asset), Quantity::zero());
        assert_eq!(
            asset_balance_or_zero(&tx, &bob_asset),
            Quantity::from(10_u32)
        );
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 1);
        tx.apply();
    }
    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts.len(), 1);
    assert_eq!(transcripts[&hash].len(), 1);
    let transcript = &transcripts[&hash][0];
    assert_eq!(transcript.poseidon_preimage_digest, None);
    assert_eq!(transcript.deltas.len(), 3);
    let expected = [(10_u32, 5_u32, 0_u32, 5_u32), (5, 2, 5, 8), (8, 0, 2, 10)];
    for (delta, (from_before, from_after, to_before, to_after)) in
        transcript.deltas.iter().zip(expected)
    {
        assert_eq!(delta.from_balance_before, Quantity::from(from_before));
        assert_eq!(delta.from_balance_after, Quantity::from(from_after));
        assert_eq!(delta.to_balance_before, Quantity::from(to_before));
        assert_eq!(delta.to_balance_after, Quantity::from(to_after));
    }
    assert_eq!(block.captured_fastpq_transcript_sources().unwrap().len(), 1);
}

#[test]
fn independent_rejected_preparations_keep_singleton_digest_and_outcome_order() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let destination = AssetId::new(definition.clone(), BOB_ID.clone());
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 7, 0);
    let mut block = state.block(header);
    let hash;
    {
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0x82);
        hash = tx.tx_call_hash.unwrap();
        TransferAssetBatch::independent(vec![
            entry(
                "before",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(11_u32),
            ),
            entry(
                "accepted",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(3_u32),
            ),
            entry(
                "after",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(20_u32),
            ),
        ])
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert_eq!(statuses(&tx), vec![false, true, false]);
        assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(7_u32));
        assert_eq!(
            asset_balance_or_zero(&tx, &destination),
            Quantity::from(3_u32)
        );
        tx.apply();
    }
    let transcripts = block.drain_transfer_transcripts();
    assert_eq!(transcripts[&hash].len(), 1);
    let transcript = &transcripts[&hash][0];
    assert_eq!(transcript.deltas.len(), 1);
    assert_eq!(transcript.deltas[0].amount, Quantity::from(3_u32));
    assert_eq!(
        transcript.poseidon_preimage_digest,
        Some(crate::fastpq::poseidon_preimage_digest(
            &transcript.deltas[0],
            &hash
        ))
    );
    assert_eq!(
        transcript.authority_digest,
        crate::fastpq::authority_digest(&ALICE_ID)
    );
}

#[test]
fn independent_zero_accepted_legs_publish_no_occurrence() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 7, 0);
    let mut block = state.block(header);
    {
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0x83);
        tx.current_lane_id = Some(iroha_data_model::nexus::LaneId::new(999));
        TransferAssetBatch::independent(vec![
            entry(
                "one",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(11_u32),
            ),
            entry(
                "two",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(12_u32),
            ),
        ])
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert_eq!(statuses(&tx), vec![false, false]);
        assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(10_u32));
        assert_eq!(tx.pending_transfer_transcript_count_for_testing(), 0);
        tx.apply();
    }
    assert!(block.drain_transfer_transcripts().is_empty());
    assert!(
        block
            .captured_fastpq_transcript_sources()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn independent_control_usage_counts_only_interleaved_successful_legs() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 86_400_000, 0);
    let mut block = state.block(header);
    let hash;
    {
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0x84);
        hash = tx.tx_call_hash.unwrap();
        SetAssetTransferControl::new(
            ALICE_ID.clone(),
            definition.clone(),
            vec![AssetTransferLimit {
                window: AssetTransferControlWindow::Day,
                cap_amount: Some(Quantity::from(5_u32)),
            }],
        )
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        TransferAssetBatch::independent(vec![
            entry(
                "first",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(2_u32),
            ),
            entry(
                "over",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(4_u32),
            ),
            entry(
                "last",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                Quantity::from(3_u32),
            ),
        ])
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert_eq!(statuses(&tx), vec![true, false, true]);
        assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::from(5_u32));
        let controls = load_asset_transfer_control_store(&tx, &ALICE_ID);
        let record = controls.find(&definition).unwrap();
        assert_eq!(record.usages.len(), 1);
        assert_eq!(record.usages[0].spent_amount, Quantity::from(5_u32));
        assert_eq!(record.usages[0].bucket_start_ms, 86_400_000);
        tx.apply();
    }
    let transcripts = block.drain_transfer_transcripts();
    let transcript = &transcripts[&hash][0];
    assert_eq!(
        transcript
            .deltas
            .iter()
            .map(|delta| delta.amount.clone())
            .collect::<Vec<_>>(),
        vec![Quantity::from(2_u32), Quantity::from(3_u32)]
    );
    assert_eq!(transcript.poseidon_preimage_digest, None);
}

#[test]
fn independent_full_quantity_and_self_transfer_keep_exact_repeated_key_values() {
    let _suppression = crate::sumeragi::witness::suppress_recording_for_current_thread();
    let (state, definition, source) = build_asset_transfer_control_test_state(10);
    let initial: Quantity = "18446744073709551618.125".parse().unwrap();
    let transferred: Quantity = "18446744073709551617.125".parse().unwrap();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 7, 0);
    let mut block = state.block(header);
    let hash;
    {
        let mut tx = block.transaction();
        seed_test_call_hash(&mut tx, 0x85);
        hash = tx.tx_call_hash.unwrap();
        **tx.world.assets.get_mut(&source).unwrap() = initial.clone();
        TransferAssetBatch::independent(vec![
            entry(
                "self",
                &ALICE_ID,
                &ALICE_ID,
                &definition,
                "0.125".parse().unwrap(),
            ),
            entry(
                "large",
                &ALICE_ID,
                &BOB_ID,
                &definition,
                transferred.clone(),
            ),
        ])
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        assert_eq!(statuses(&tx), vec![true, true]);
        assert_eq!(asset_balance_or_zero(&tx, &source), Quantity::one());
        tx.apply();
    }
    let transcripts = block.drain_transfer_transcripts();
    let transcript = &transcripts[&hash][0];
    assert_eq!(transcript.deltas.len(), 2);
    assert_eq!(transcript.poseidon_preimage_digest, None);
    let first = &transcript.deltas[0];
    assert_eq!(first.from_balance_before, initial);
    assert_eq!(
        first.from_balance_after,
        "18446744073709551618".parse::<Quantity>().unwrap()
    );
    assert_eq!(first.to_balance_before, first.from_balance_after);
    assert_eq!(first.to_balance_after, first.from_balance_before);
    let second = &transcript.deltas[1];
    assert_eq!(second.from_balance_before, first.to_balance_after);
    assert_eq!(second.from_balance_after, Quantity::one());
    assert_eq!(second.amount, transferred);
    assert_eq!(second.to_balance_after, second.amount);
}
