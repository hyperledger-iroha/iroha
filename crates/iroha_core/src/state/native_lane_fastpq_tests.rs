// Actual native execution retains local FASTPQ captures; no claim reconstructs them.
// Inventory qualification is disposable and does not publish or acknowledge Apply.

fn assert_native_fastpq_retained_for_test(
    overlay: &StateBlock<'_>,
    outputs: &[super::lane_decision_execution::PreexecutedLaneDecisionGroupV1],
) {
    use iroha_data_model::fastpq::FastpqSourceExecutionKindV1;
    let mut expected = BTreeMap::new();
    for output in outputs {
        let input = &output.source.payload.input;
        let call = Hash::from(input.entrypoint.execution_call_hash());
        let route = input.routing_plan().unwrap().coordinator_route();
        for bundle in &output.fastpq_transcripts {
            assert_eq!(bundle.entry_hash, Hash::from(input.entrypoint.hash()));
            assert!(expected.insert(call, bundle.transcripts.clone()).is_none());
            let capture = &overlay.captured_fastpq_transcript_sources().unwrap()[&call];
            assert_eq!(capture.entry_hash(), call);
            assert_eq!(capture.source().height, overlay._curr_block.height().get());
            assert_eq!(capture.source().network_id, overlay.network_id);
            assert_eq!(capture.dataspace_id(), route.dataspace_id);
            assert_eq!(
                capture.execution_kind(),
                FastpqSourceExecutionKindV1::ExecutionCall
            );
        }
    }
    assert_eq!(
        overlay.fastpq_transcripts, expected,
        "native prefix snapshots do not drain the actual map"
    );
    assert!(
        overlay
            .captured_fastpq_transcript_sources()
            .unwrap()
            .keys()
            .eq(expected.keys()),
        "the same overlay retains every real native capture"
    );
}

fn native_fastpq_actual_application_header(fixture: &NativeEconomicFixture) -> BlockHeader {
    native_consumer_stage_carrier(fixture).header()
}

fn native_fastpq_prepare<'state>(
    state: &'state State,
    header: &BlockHeader,
    groups: &[super::VerifiedLaneDecisionGroupV1],
) -> super::lane_decision_batch::PreparedLaneDecisionBatchV1<'state> {
    let batch = state.prepare_lane_decision_batch(groups).unwrap();
    state
        .replay_lane_decision_batch(header, &batch, groups.to_vec())
        .unwrap()
}

fn native_fastpq_cache_actual_input_set(
    overlay: &mut StateBlock<'_>,
    groups: &[super::VerifiedLaneDecisionGroupV1],
) {
    let digest = iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(
        groups
            .iter()
            .map(|group| &group.body().payload().input.entrypoint),
    )
    .unwrap();
    overlay.set_fastpq_tx_set_hash(digest.into());
}

#[inline(never)]
fn native_fastpq_apply_real_additional_transfer(
    overlay: &mut StateBlock<'_>,
    fixture: &NativeEconomicFixture,
    call: Option<(Hash, crate::queue::RoutingDecision)>,
) {
    use crate::smartcontracts::Execute;
    let mut transaction = overlay.transaction();
    match call {
        Some((hash, route)) => {
            transaction.tx_call_hash = Some(hash);
            transaction.current_lane_id = Some(route.lane_id);
            transaction.current_dataspace_id = Some(route.dataspace_id);
            Transfer::asset_quantity(
                fixture.source.clone(),
                1u32,
                fixture.destination.account().clone(),
            )
            .execute(fixture.source.account(), &mut transaction)
            .unwrap();
        }
        None => {
            assert!(transaction.tx_call_hash.is_none());
            // Exercise the production typed numeric-movement owner: it validates
            // this exact source account and performs real debit/credit/capture.
            // This is not a test claim of a completed staking protocol operation.
            crate::smartcontracts::isi::asset::isi::execute_staking_bond_transfer(
                &mut transaction,
                fixture.source.account(),
                LaneId::SINGLE,
                fixture.destination.account(),
                fixture.source.account(),
                false,
                fixture.source.clone(),
                fixture.destination.clone(),
                Quantity::from(1u32),
            )
            .unwrap();
        }
    }
    transaction.apply();
}

state_test! { sync native_fastpq_transfer_and_reveal_keep_actual_sources_through_common_inventory
    for case in [NativeEconomicCase::Transfer(25), NativeEconomicCase::Reveal(0)] {
        let fixture = native_economic_fixture(&[case], true);
        let state = &fixture.native.state;
        let groups = native_economic_groups(&fixture);
        let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
        let header = native_fastpq_actual_application_header(&fixture);
        let mut prepared = native_fastpq_prepare(state, &header, &groups);
        assert!(prepared.executions()[0].result.is_ok(), "{:?}", prepared.executions()[0].result);
        assert_native_fastpq_retained_for_test(prepared.overlay(), prepared.executions());
        let input = &groups[0].body().payload().input.entrypoint;
        let call = Hash::from(input.execution_call_hash());
        if matches!(case, NativeEconomicCase::Reveal(_)) {
            assert_ne!(call, Hash::from(input.hash()));
            assert_eq!(prepared.executions()[0].fastpq_transcripts[0].entry_hash, Hash::from(input.hash()));
        }
        let native_snapshot = prepared.executions()[0].fastpq_transcripts[0].transcripts.clone();
        let overlay = prepared.overlay_mut_for_test();
        native_fastpq_cache_actual_input_set(overlay, &groups);
        overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap();
        let inventory = overlay.fastpq_source_inventory().unwrap().unwrap();
        assert_eq!(inventory.entries().len(), 1);
        assert_eq!(inventory.entries()[0].entry_hash, call);
        assert_eq!(inventory.transcript_entry_hashes(), &BTreeSet::from([call]));
        assert!(overlay.verified_fastpq_source_inventory_for_capture().is_ok());
        let actual = overlay.drain_transfer_transcripts_with_pending(None);
        assert_eq!(actual[&call], native_snapshot);
        assert_eq!(overlay.captured_fastpq_transcript_sources().unwrap().len(), 1,
            "common output drain preserves the actual sealed capture owner");
        assert!(overlay.verified_fastpq_source_inventory_for_capture().is_ok());
        drop(prepared);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    }
}

state_test! { sync native_fastpq_common_inventory_retains_due_start_and_actual_protocol_work
    let fixture = native_economic_fixture_with_world_initializer(
        &[NativeEconomicCase::Transfer(25)], true, None, None, |world| {
            world.governance_referenda.insert("native-source-due-start".into(), GovernanceReferendumRecord {
                h_start: 7, h_end: 9, status: GovernanceReferendumStatus::Proposed,
                mode: GovernanceReferendumMode::default(),
                plain_context: iroha_data_model::governance::conviction::PlainVotingContextV1::NotApplicable,
                            plain_result: iroha_data_model::governance::conviction::PlainVotingResultV1::NotApplicable,
            });
        });
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let header = native_fastpq_actual_application_header(&fixture);
    assert_eq!(header.height().get(), 7);
    let mut prepared = native_fastpq_prepare(state, &header, &groups);
    assert!(prepared.executions()[0].result.is_ok());
    assert_eq!(prepared.overlay().world.governance_referenda.get("native-source-due-start").unwrap().status,
        GovernanceReferendumStatus::Open, "actual shared start transition precedes native input");
    assert_native_fastpq_retained_for_test(prepared.overlay(), prepared.executions());
    let native_call = Hash::from(groups[0].body().payload().input.entrypoint.execution_call_hash());
    let overlay = prepared.overlay_mut_for_test();
    native_fastpq_apply_real_additional_transfer(overlay, &fixture, None);
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(74u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(26u32));
    let captures = overlay.captured_fastpq_transcript_sources().unwrap();
    assert_eq!(captures.len(), 2);
    let protocol = *captures.iter().find(|(_, capture)| capture.is_protocol_purpose()).unwrap().0;
    assert_ne!(native_call, protocol);
    native_fastpq_cache_actual_input_set(overlay, &groups);
    overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap();
    let inventory = overlay.fastpq_source_inventory().unwrap().unwrap();
    assert_eq!(inventory.entries().iter().map(|entry| entry.entry_hash).collect::<Vec<_>>(), [native_call, protocol]);
    assert_eq!(inventory.entries()[1].execution_kind,
        iroha_data_model::fastpq::FastpqSourceExecutionKindV1::ProtocolPurpose);
    assert!(overlay.verified_fastpq_source_inventory_for_capture().is_ok());
    let actual = overlay.drain_transfer_transcripts_with_pending(None);
    assert_eq!(actual.len(), 2);
    assert!(actual.contains_key(&native_call) && actual.contains_key(&protocol));
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before,
        "native, start transition and additional protocol work all roll back together");
    assert_eq!(state.world.governance_referenda.view().get("native-source-due-start").unwrap().status,
        GovernanceReferendumStatus::Proposed);
}

state_test! { sync native_fastpq_omission_substitution_and_competing_inputs_latch_inventory_failure
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let header = native_fastpq_actual_application_header(&fixture);
    let input = groups[0].body().payload().input.entrypoint.clone();
    let route = groups[0].body().payload().input.routing_plan().unwrap().coordinator_route();
    let call = Hash::from(input.execution_call_hash());
    for mutation in 0..6 {
        let mut prepared = native_fastpq_prepare(state, &header, &groups);
        let overlay = prepared.overlay_mut_for_test();
        native_fastpq_cache_actual_input_set(overlay, &groups);
        let original = overlay.fastpq_transcripts.clone();
        match mutation {
            0 => { overlay.fastpq_transcripts.remove(&call); }
            1 => { overlay.fastpq_transcripts.get_mut(&call).unwrap()[0].authority_digest = Hash::new(b"substituted public authority"); }
            2 => {
                overlay.fastpq_transcripts.remove(&call);
                overlay.fastpq_source_captures.take_unsealed_sources(&BTreeSet::from([call])).unwrap();
            }
            3 => {}
            4 => {
                overlay.fastpq_source_captures.take_unsealed_sources(&BTreeSet::from([call])).unwrap();
            }
            5 => {
                let old_capture = overlay.captured_fastpq_transcript_sources().unwrap()[&call];
                overlay.fastpq_source_captures.take_unsealed_sources(&BTreeSet::from([call])).unwrap();
                // A real later applied transfer supplies a different capture. Restore
                // the original rows only: equal public bytes cannot replace custody.
                native_fastpq_apply_real_additional_transfer(overlay, &fixture, Some((call, route)));
                let replacement = overlay.captured_fastpq_transcript_sources().unwrap()[&call];
                assert_ne!(replacement.first_fragment_index(), old_capture.first_fragment_index());
                overlay.fastpq_transcripts = original.clone();
            }
            _ => unreachable!(),
        }
        let error = if mutation == 3 {
            overlay.finalize_fastpq_source_inventory(std::slice::from_ref(&input), &[route], &[]).unwrap_err()
        } else {
            overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap_err()
        };
        assert!(error.contains(match mutation {
            3 => "competing external",
            4 | 5 => "native FASTPQ captures",
            _ => "native FASTPQ rows",
        }), "{error}");
        assert_eq!(overlay.fastpq_source_inventory().unwrap_err(), error);
        overlay.fastpq_transcripts = original;
        assert!(overlay.finalize_fastpq_source_inventory(&[], &[], &[]).is_err(),
            "restoring supplied bytes cannot clear failed inventory ownership");
        assert_eq!(overlay.fastpq_source_inventory().unwrap_err(), error);
        drop(prepared);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    }
}

state_test! { sync native_fastpq_post_prefix_actual_extra_row_cannot_replace_private_output_seal
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let header = native_fastpq_actual_application_header(&fixture);
    let mut prepared = native_fastpq_prepare(state, &header, &groups);
    let input = &groups[0].body().payload().input;
    let call = Hash::from(input.entrypoint.execution_call_hash());
    let route = input.routing_plan().unwrap().coordinator_route();
    let overlay = prepared.overlay_mut_for_test();
    let count = overlay.fastpq_transcripts[&call].len();
    native_fastpq_apply_real_additional_transfer(overlay, &fixture, Some((call, route)));
    assert_eq!(overlay.fastpq_transcripts[&call].len(), count + 1);
    assert_eq!(overlay.captured_fastpq_transcript_sources().unwrap().len(), 1,
        "same-key capture metadata cannot conceal an extra actual occurrence");
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(74u32));
    native_fastpq_cache_actual_input_set(overlay, &groups);
    let error = overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap_err();
    assert!(error.contains("native FASTPQ rows"), "{error}");
    assert_eq!(overlay.fastpq_source_inventory().unwrap_err(), error);
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_fastpq_duplicate_snapshot_selection_is_atomic_and_rejection_keeps_inventory_entry
    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let groups = native_economic_groups(&fixture);
    let header = native_fastpq_actual_application_header(&fixture);
    let mut prepared = native_fastpq_prepare(&fixture.native.state, &header, &groups);
    let input = groups[0].body().payload().input.entrypoint.clone();
    let overlay = prepared.overlay_mut_for_test();
    let original = overlay.fastpq_transcripts.clone();
    let captures = overlay.captured_fastpq_transcript_sources().unwrap().clone();
    assert!(overlay.retain_native_lane_fastpq_outputs(&[input.clone(), input]).is_err());
    assert_eq!(overlay.fastpq_transcripts, original);
    assert_eq!(overlay.captured_fastpq_transcript_sources().unwrap(), &captures);
    native_fastpq_cache_actual_input_set(overlay, &groups);
    overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap();
    assert!(overlay.retain_native_lane_fastpq_outputs(&[]).is_err());
    assert_eq!(overlay.fastpq_transcripts, original);
    assert_eq!(overlay.captured_fastpq_transcript_sources().unwrap(), &captures);
    drop(prepared);

    let fixture = native_economic_fixture(&[NativeEconomicCase::Transfer(101)], true);
    let groups = native_economic_groups(&fixture);
    let header = native_fastpq_actual_application_header(&fixture);
    let mut prepared = native_fastpq_prepare(&fixture.native.state, &header, &groups);
    assert!(prepared.executions()[0].result.is_err());
    assert!(prepared.executions()[0].fastpq_transcripts.is_empty());
    let overlay = prepared.overlay_mut_for_test();
    assert!(overlay.fastpq_transcripts.is_empty());
    assert!(overlay.captured_fastpq_transcript_sources().unwrap().is_empty());
    native_fastpq_cache_actual_input_set(overlay, &groups);
    overlay.finalize_fastpq_source_inventory(&[], &[], &[]).unwrap();
    let inventory = overlay.fastpq_source_inventory().unwrap().unwrap();
    assert_eq!(inventory.entries().len(), 1, "deterministically rejected native input remains an executed source");
    assert!(inventory.transcript_entry_hashes().is_empty());
}
