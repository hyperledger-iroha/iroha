// Actual authenticated merge prefix, complete source seal and recorder controls.
// Included in State's consensus-stack fixture module so the exact certified source
// and four-validator QueuePlan/DA helpers remain the same as production-path tests.

state_test!(consensus_stack merge_prefix_recorded_output_seal_retains_exact_sources_and_rejects_tampering
    merge_prefix_recorded_output_seal_retains_exact_sources_and_rejects_tampering_on_consensus_stack();
);
fn merge_prefix_recorded_output_seal_retains_exact_sources_and_rejects_tampering_on_consensus_stack()
 {
    for sealed in [false, true] {
        let fixture = unpersisted_autonomous_merge_commit_fixture(
            false,
            false,
            Some(QueuePlanTransferFixture::Single),
            sealed,
            None,
            false,
        );
        let mut carrier = fixture.carrier;
        let inputs = fixture
            .entry
            .execution_batch
            .as_ref()
            .unwrap()
            .lanes
            .iter()
            .flat_map(|lane| lane.entrypoints.iter())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(inputs.len(), 1);
        if sealed {
            assert_ne!(
                Hash::from(inputs[0].hash()),
                Hash::from(inputs[0].execution_call_hash())
            );
        }
        let expected_tx_set: [u8; 32] =
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(inputs.iter())
                .unwrap()
                .into();
        // Resolve the real certified sidecar through the production applying
        // constructor. Its recorder begins after writers and before pristine work.
        fixture
            .state
            .kura
            .persist_pending_certified_merge_entry(&fixture.entry)
            .unwrap();
        let (mut block, guard) =
            ValidBlock::recorded_merge_state_block_for_testing(&carrier, &fixture.state)
                .expect("recorded production merge constructor");
        let prefix_usage = block.fastpq_source_usage_for_testing();
        assert_eq!(prefix_usage.0.executed_entries, 1);
        assert!(
            block.verify_execution_output_publication().is_err(),
            "the unsealed prefix may not publish a raw State block"
        );
        block
            .execute_and_seal_ordinary_outputs(&mut carrier, None, |block, _, routes| {
                assert!(
                    routes.is_empty(),
                    "merge prefix is not rerouted as ordinary input"
                );
                Ok::<_, String>(ExecutionOutputSealMetadata {
                    committed_fragment_count: block.committed_fragment_count().try_into().unwrap(),
                    lane_finality_statements: Vec::new(),
                })
            })
            .expect("prefix and complete internal tail use their one original output budget");
        assert!(block.fastpq_transcripts.is_empty());
        let inventory = block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap();
        assert_eq!(inventory.tx_set_hash(), expected_tx_set);
        assert_eq!(
            inventory.entries()[0].entry_hash,
            Hash::from(inputs[0].execution_call_hash())
        );
        assert_eq!(
            block.fastpq_source_usage_for_testing().0.executed_entries,
            1,
            "the tail cannot grant the prefix a second logical E"
        );
        block
            .finalize_lane_consensus_contexts(&carrier, None)
            .unwrap();
        block
            .capture_exec_witness()
            .expect("actual prefix recorder joins the sealed inventory");
        block.verify_merge_prefix_surface().unwrap();
        block.verify_execution_output_seal(&carrier).unwrap();
        let witness = block.exec_witness.as_ref().unwrap();
        if !sealed {
            assert!(
                !witness.fastpq_transcripts.is_empty(),
                "executed transfer must survive final capture"
            );
            assert!(
                !witness.writes.is_empty(),
                "ordinary prefix writes must survive the same capture"
            );
        }
        // Equal serialized inventory contents cannot replace original custody.
        let original = block.fastpq_source_inventory.take().unwrap();
        block.fastpq_source_inventory = Some(Ok(Arc::new((*inventory).clone())));
        assert!(block.verify_merge_prefix_surface().is_err());
        block.fastpq_source_inventory = Some(original);
        block.verify_merge_prefix_surface().unwrap();
        if !sealed {
            block
                .exec_witness
                .as_mut()
                .unwrap()
                .fastpq_transcripts
                .clear();
            assert!(
                block.verify_merge_prefix_surface().is_err(),
                "removing the retained prefix cannot shrink source ownership"
            );
        }
        assert!(
            block.verify_execution_output_publication().is_err(),
            "source/witness sealing never fabricates finality authority"
        );
        drop(guard);
        drop(block);
    }
}

state_test!(consensus_stack merge_prefix_transcript_substitution_refuses_complete_output_seal
    merge_prefix_transcript_substitution_refuses_complete_output_seal_on_consensus_stack();
);
fn merge_prefix_transcript_substitution_refuses_complete_output_seal_on_consensus_stack() {
    let fixture = unpersisted_autonomous_merge_commit_fixture(
        false,
        false,
        Some(QueuePlanTransferFixture::Single),
        false,
        None,
        false,
    );
    let carrier = fixture.carrier;
    let (mut block, guard) = fixture
        .state
        .block_with_owned_start_stages(
            carrier.header(),
            |block| {
                let guard = crate::sumeragi::witness::begin_exec_witness_capture()
                    .map_err(MergeLedgerCommitError::ExecutionBatchInvalid)?;
                block.stage_certified_merge_entry(&fixture.entry, ConsensusMode::Permissioned)?;
                Ok::<_, MergeLedgerCommitError>(guard)
            },
            |_, guard| Ok(guard),
        )
        .unwrap();
    assert!(!block.fastpq_transcripts.is_empty());
    block.fastpq_transcripts.clear();
    block.reserve_ordinary_execution_outputs(&carrier).unwrap();
    assert!(
        block.execute_ordinary_output_plan(&carrier, None).is_err(),
        "the actual producer must refuse a missing authenticated prefix before sealing"
    );
    assert!(matches!(
        block.execution_output_plan,
        Some(output_capacity::ExecutionOutputPlanState::Poisoned)
    ));
    drop(guard);
    drop(block);
}

/// Exact certified/RS16/QueuePlan merge fixture for consuming custody tests.
pub(in crate::state) fn unpersisted_merge_custody_fixture(
    sealed: bool,
) -> (State, MergeLedgerEntry, SignedBlock, Vec<PeerId>) {
    let fixture = unpersisted_autonomous_merge_commit_fixture(
        false,
        false,
        Some(QueuePlanTransferFixture::Single),
        sealed,
        None,
        false,
    );
    let validators = fixture
        .validator_keypairs
        .iter()
        .map(|key| PeerId::new(key.public_key().clone()))
        .collect();
    (fixture.state, fixture.entry, fixture.carrier, validators)
}

state_test!(consensus_stack merge_prefix_recorder_reset_refuses_final_seal
    merge_prefix_recorder_reset_refuses_final_seal_on_consensus_stack();
);
fn merge_prefix_recorder_reset_refuses_final_seal_on_consensus_stack() {
    for reset_after_seal in [false, true] {
        let fixture = unpersisted_autonomous_merge_commit_fixture(
            false,
            false,
            Some(QueuePlanTransferFixture::Single),
            false,
            None,
            false,
        );
        let mut carrier = fixture.carrier;
        fixture
            .state
            .kura
            .persist_pending_certified_merge_entry(&fixture.entry)
            .unwrap();
        let (mut block, guard) =
            ValidBlock::recorded_merge_state_block_for_testing(&carrier, &fixture.state).unwrap();
        block.require_merge_prefix_recording().unwrap();
        if reset_after_seal {
            block
                .execute_and_seal_ordinary_outputs(&mut carrier, None, |block, _, _| {
                    Ok::<_, String>(ExecutionOutputSealMetadata {
                        committed_fragment_count: block
                            .committed_fragment_count()
                            .try_into()
                            .unwrap(),
                        lane_finality_statements: Vec::new(),
                    })
                })
                .unwrap();
            block
                .finalize_lane_consensus_contexts(&carrier, None)
                .unwrap();
        }
        crate::sumeragi::witness::start_block();
        assert!(block.require_merge_prefix_recording().is_err());
        if reset_after_seal {
            assert!(block.capture_exec_witness().is_err());
            assert!(block.exec_witness.is_none());
        } else {
            assert!(block.reserve_ordinary_execution_outputs(&carrier).is_err());
        }
        assert!(block.verify_execution_output_publication().is_err());
        drop(guard);
        drop(block);
    }
}

state_test!(consensus_stack late_component_capture_refuses_prestaged_merge_controls
    late_component_capture_refuses_prestaged_merge_controls_on_consensus_stack();
);
fn late_component_capture_refuses_prestaged_merge_controls_on_consensus_stack() {
    for transfer in [None, Some(QueuePlanTransferFixture::Single)] {
        let fixture =
            unpersisted_autonomous_merge_commit_fixture(false, false, transfer, false, None, false);
        let mut block = fixture
            .state
            .block_with_certified_merge_entry(
                fixture.carrier.header(),
                &fixture.entry,
                ConsensusMode::Permissioned,
            )
            .unwrap();
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ValidBlock::validate_unchecked(fixture.carrier, &mut block)
        }));
        assert!(
            refused.is_err(),
            "late fixture capture must refuse pre-staged merge controls"
        );
        assert!(crate::sumeragi::witness::current_exec_witness_capture_identity().is_none());
    }
}

/// Exact control-only merge source from a committed drain and three real votes.
pub(in crate::state) fn unpersisted_control_only_merge_custody_fixture()
-> (State, MergeLedgerEntry, SignedBlock, Vec<PeerId>) {
    let (mut state, _, commit_keypairs, parent) = configured_single_lane_queue_plan_state();
    let mut nexus = state.nexus_snapshot();
    nexus.autoscale =
        autoscale_transition_test_nexus(nexus.lane_catalog.lanes().to_vec(), 1, 2, 100).autoscale;
    state
        .set_nexus(nexus)
        .expect("configured managed-lane lifecycle");
    let mut drain_keypairs = seed_governed_autoscale_committee_for_test(&state, 4);
    drain_keypairs.sort_by_key(|key| PeerId::new(key.public_key().clone()));
    let lane_id = LaneId::new(1);
    let close = empty_global_block_after(Some(&parent));
    state
        .apply_lane_lifecycle_with_options(
            &iroha_data_model::nexus::LaneLifecyclePlan {
                additions: vec![autoscale_elastic_lane_config(
                    lane_id,
                    DataSpaceId::UNIVERSAL,
                    parent.header().height().get(),
                )],
                retire: Vec::new(),
            },
            false,
            true,
        )
        .expect("real managed-lane allocation pins its exact governed committee");
    state
        .kura
        .store_block(Arc::new(close.clone()))
        .expect("signed RS16 close carrier");
    {
        let mut block = state.block(close.header());
        block
            .stage_autoscale_sample_record_for_count(&close, 0)
            .expect("close carrier retains its exact runtime sample");
        block
            .stage_autoscale_lane_drain_intent(lane_id, 2, 2, 0, 0)
            .expect("real irreversible drain-intent transition");
        block
            .stage_canonical_carrier_membership(
                Vec::new(),
                close.header().height().try_into().unwrap(),
            )
            .unwrap();
        block.block_hashes.push(close.hash());
        block
            .commit()
            .expect("publish the actual drain intent before voting");
    }
    assert_eq!(
        state.committed_height() as u64,
        close.header().height().get()
    );
    let (body, committee) = state
        .pending_autoscale_lane_drain_body()
        .expect("only the committed drain may supply its vote body");
    assert_eq!(
        body.intent.close_global_height,
        close.header().height().get()
    );
    assert_eq!(body.intent.lane_id, lane_id);
    assert_eq!(body.intent.min_quorum, 3);
    assert_eq!(
        committee,
        drain_keypairs
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>()
    );
    let mut collector = crate::sumeragi::v2_runner::native_drain::NativeDrainOwner::new();
    for (index, key) in drain_keypairs.iter().take(3).enumerate() {
        let peer = PeerId::new(key.public_key().clone());
        let vote = crate::lane_consensus::LaneDrainVoteV1::new_signed(
            body.clone(),
            peer.clone(),
            key.private_key(),
        )
        .unwrap();
        assert!(
            collector
                .accept_remote_vote(&state, peer, vote, std::time::Instant::now())
                .unwrap()
        );
        assert_eq!(collector.certificate().is_some(), index == 2);
    }
    let certificate = collector
        .certificate()
        .expect("three exact signed votes")
        .clone();
    crate::lane_consensus::validate_lane_drain_certificate(&certificate).unwrap();
    let candidate = state
        .merge_drain_candidate_for_next_carrier(
            &close.header(),
            0,
            certificate.clone(),
            ConsensusMode::Permissioned,
        )
        .expect("actual certificate and committed lifecycle produce the control-only source");
    assert!(candidate.execution_batch.is_none());
    assert!(candidate.lane_snapshots.is_empty());
    assert_eq!(candidate.lane_drain_certificates, vec![certificate]);
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    state
        .validate_certified_merge_entry_for_global_order(&entry, ConsensusMode::Permissioned)
        .expect("real merge QC authenticates the exact drain-only candidate");
    let carrier = certified_merge_carrier_after(&close, &entry);
    (state, entry, carrier, committee)
}
