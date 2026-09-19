// Completed native sources remain authenticated history after later economic
// applications; neither their Decisions nor their finality can be relabelled.

fn publish_next_native_group_for_test(
    fixture: &NativeEconomicFixture,
    group: &super::VerifiedLaneDecisionGroupV1,
) -> crate::block::CommittedBlock {
    let state = &fixture.native.state;
    let parent = state
        .kura
        .get_block(NonZeroUsize::new(state.committed_height()).unwrap())
        .expect("actual committed native parent");
    let finality = state
        .kura
        .v2_finality_artifact(parent.header().height().get())
        .unwrap()
        .expect("actual parent finality");
    let context = crate::sumeragi::v2_context::build_successor_height_context(
        &finality,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state)
            .expect("derive Native applying policy from its exact committed predecessor"),
        None,
    )
    .expect("successor inherits exact four-validator authority");
    let mut carrier = empty_global_block_after(Some(&parent)).canonical_resultless_proposal();
    carrier.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &state.nexus_snapshot(),
        carrier.header().height().get(),
    )));
    let batch = state
        .prepare_lane_decision_batch(std::slice::from_ref(group))
        .expect("one exact independent route is ready on the current State");
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    let key = merge_carrier_finality_fixture_keypair();
    carrier
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(key.private_key(), carrier.hash()),
            ),
        ]))
        .unwrap();
    let (overlay, committed) = prepared_native_publication_for_test(state, &carrier, context);
    overlay
        .commit()
        .expect("publish actual native economics exactly once");
    promote_native_execution_finality_for_test(state, &committed);
    committed
}

fn cold_restore_completed_native_history_for_test(
    state: &State,
) -> (tempfile::TempDir, Box<State>) {
    fn copy_durable_tree(source: &std::path::Path, destination: &std::path::Path) {
        std::fs::create_dir_all(destination).expect("create isolated cold-store directory");
        for entry in std::fs::read_dir(source).expect("read quiescent durable history") {
            let entry = entry.expect("read durable history entry");
            let source = entry.path();
            let destination = destination.join(entry.file_name());
            let kind = entry.file_type().expect("inspect durable history entry");
            if kind.is_dir() {
                copy_durable_tree(&source, &destination);
            } else {
                assert!(
                    kind.is_file(),
                    "durable fixture history contains only regular files"
                );
                let bytes = std::fs::read(&source).expect("read exact durable source bytes");
                std::fs::write(&destination, &bytes).expect("copy exact durable source bytes");
                assert_eq!(std::fs::read(&destination).unwrap(), bytes);
                assert_eq!(
                    std::fs::read(&source).unwrap(),
                    bytes,
                    "cold copy must not change the live fixture's durable bytes"
                );
            }
        }
    }
    let nexus = state.nexus_snapshot();
    let cold_root = tempfile::tempdir().expect("retain isolated cold history owner");
    // The original Kura retains its exclusive root lock. Copy the quiescent
    // store, including every body, finality and geometry file, then acquire a
    // normal independent Kura lock on the copied root.
    copy_durable_tree(&state.kura.store_root(), cold_root.path());
    let config = strict_kura_config_for_testing(cold_root.path().to_path_buf());
    let configured_lanes =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.configured_lane_catalog);
    let (cold_kura, _) = Kura::new_with_configured_lane_catalog(
        &config,
        &configured_lanes,
        &nexus.configured_lane_catalog,
    )
    .expect("reopen genuine native history without a warm Kura cache");
    let mut restored = deserialize::KuraSeed {
        kura: cold_kura,
        lane_manifests: state.lane_manifests.read().clone(),
        query_handle: LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        telemetry: crate::telemetry::StateTelemetry::default(),
    }
    .into_state_from_json(norito::json::to_value(state).unwrap())
    .expect("restore completed native membership and exact immutable manifest baseline");
    restored
        .kura
        .bind_lane_storage_network(restored.network_id)
        .expect("bind cold lane storage to the authenticated snapshot network");
    restored
        .prepare_restored_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)
        .expect("authenticate copied primary geometry against the configured baseline");
    restored
        .restore_kura_lane_segments_from_nexus()
        .expect("restore copied lane instances from exact snapshot lineage");
    restored.configure_test_runtime_defaults();
    restored
        .set_nexus_from_config(nexus)
        .expect("restore the original process fee policy");
    restored.install_lane_compliance_engine(state.lane_compliance_engine());
    assert_eq!(
        restored.execution_policy_digest_v1().unwrap(),
        state.execution_policy_digest_v1().unwrap()
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap()
    );
    (cold_root, restored)
}

fn assert_completed_native_history_for_test(
    state: &State,
    blocks: &[crate::block::CommittedBlock],
    original_groups: &[super::VerifiedLaneDecisionGroupV1],
) {
    use crate::kura::NativeLaneBatchCarrierReadV1;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let durable_height = state.kura.exact_durable_blocks_count().unwrap();
    for (block, group) in blocks.iter().zip(original_groups) {
        let signed = block.as_ref();
        let height = NonZeroUsize::new(signed.header().height().get() as usize).unwrap();
        let expected_wire = signed.encode_wire().unwrap();
        assert_eq!(
            state.kura.get_block(height).unwrap().encode_wire().unwrap(),
            expected_wire,
            "later applications and cold reopen retain the original complete result bytes"
        );
        let original_finality = block.verified_v2_finality_artifact().unwrap();
        let path = state
            .kura
            .v2_finality_artifact_path_for_testing(height.get() as u64);
        let finality_bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            state
                .kura
                .v2_finality_artifact(height.get() as u64)
                .unwrap()
                .as_ref(),
            Some(original_finality),
            "the original exact QC remains the only source authority"
        );
        let NativeLaneBatchCarrierReadV1::Ready(included) = state
            .read_finalized_native_lane_batch(height, signed.hash())
            .unwrap()
        else {
            panic!("complete retained native source remains readable");
        };
        assert_eq!(included.batch().groups, vec![group.to_wire()]);
        let entrypoint = group.body().payload().input.entrypoint.hash();
        assert_eq!(
            state.view().transactions.get(&entrypoint),
            Some(height),
            "later economic work cannot relocate first execution membership"
        );
        assert!(
            state
                .replay_finalized_native_lane_batch(&included, &[])
                .is_err(),
            "completed native history is never an applying pre-State"
        );
        let successor = empty_global_block_after(
            state
                .kura
                .get_block(NonZeroUsize::new(state.committed_height()).unwrap())
                .as_deref(),
        );
        let error = state
            .preexecute_lane_decision_groups(successor.header(), std::slice::from_ref(group))
            .err()
            .expect("the old execution cannot run again");
        let expected_reason = if state
            .verified_lane_consensus_contexts()
            .unwrap()
            .unwrap()
            .contexts()
            .is_empty()
        {
            "native execution batch exceeds its distinct open route count"
        } else {
            "native execution reuses a committed carrier or sealed signed-execution identity"
        };
        assert!(
            matches!(error, MergeLedgerCommitError::ExecutionBatchInvalid(ref reason)
            if reason == expected_reason),
            "the original completed source is rejected before start hooks/economics: {error}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            finality_bytes,
            "rejection cannot rewrite retained finality custody"
        );
        assert_eq!(
            state.kura.get_block(height).unwrap().encode_wire().unwrap(),
            expected_wire
        );
    }
    assert_eq!(
        state.kura.exact_durable_blocks_count().unwrap(),
        durable_height
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before,
        "historical reads and replay denials leave every committed effect unchanged"
    );
}

state_test!(consensus_stack native_completed_history_rejects_reapplication_after_second_economic_commit
    native_completed_history_rejects_reapplication_after_second_economic_commit_impl();
);
fn native_completed_history_rejects_reapplication_after_second_economic_commit_impl() {
    let (fixture, _, _) = native_publication_fixture_for_test(&[
        NativeEconomicCase::Transfer(25),
        NativeEconomicCase::Transfer(15),
    ]);
    let state = &fixture.native.state;
    let original_groups = native_economic_groups(&fixture);
    assert_eq!(original_groups.len(), 2);
    let admission_height = state.committed_height();
    let mut committed = Vec::new();
    let mut total = Quantity::zero();
    for original in &original_groups {
        let groups = native_economic_groups(&fixture);
        let current = groups
            .iter()
            .find(|group| {
                group.body().payload().input.entrypoint.hash()
                    == original.body().payload().input.entrypoint.hash()
            })
            .unwrap();
        // Successful Network outputs and increasing balances prove that each
        // separate publication owns real economics, including the second cycle.
        let block = publish_next_native_group_for_test(&fixture, current);
        assert_eq!(block.as_ref().network_entrypoint_count(), 1);
        assert!(
            block
                .as_ref()
                .network_output_at(0)
                .unwrap()
                .1
                .result
                .is_ok()
        );
        assert!(block.as_ref().output_results().all(|result| result.is_ok()));
        committed.push(block);
        assert_eq!(state.committed_height(), admission_height + committed.len());
        let view = state.world_view();
        let current_total = view.assets().get(&fixture.destination).unwrap().0.clone();
        assert!(
            current_total > total,
            "each separate native publication applies its own transfer"
        );
        total = current_total;
        drop(view);
        assert_completed_native_history_for_test(
            state,
            &committed,
            &original_groups[..committed.len()],
        );
    }
    assert_eq!(total, Quantity::from(40_u32));
    assert_eq!(
        state.world.assets.view().get(&fixture.source).unwrap().0,
        Quantity::from(60_u32)
    );
    assert_eq!(
        committed[1].as_ref().header().prev_block_hash(),
        Some(committed[0].as_ref().hash())
    );
    assert!(
        state
            .verified_lane_consensus_contexts()
            .unwrap()
            .unwrap()
            .contexts()
            .is_empty(),
        "both actual input obligations close after their own executions"
    );
    assert_completed_native_history_for_test(state, &committed, &original_groups);
    let admission_wire = fixture.native.block.encode_wire().unwrap();
    let admission_height =
        NonZeroUsize::new(fixture.native.block.header().height().get() as usize).unwrap();
    let admission_finality = state
        .kura
        .v2_finality_artifact(admission_height.get() as u64)
        .unwrap();
    let (_cold_root, restored) = cold_restore_completed_native_history_for_test(state);
    assert_eq!(
        restored
            .kura
            .get_block(admission_height)
            .unwrap()
            .encode_wire()
            .unwrap(),
        admission_wire,
        "cold history retains the original first-admission body, not a reconstructed substitute"
    );
    assert_eq!(
        restored
            .kura
            .v2_finality_artifact(admission_height.get() as u64)
            .unwrap(),
        admission_finality,
        "the original admission's cryptographic custody survives cold reopening"
    );
    assert_completed_native_history_for_test(&restored, &committed, &original_groups);
    assert!(
        restored
            .read_finalized_native_lane_batch(
                NonZeroUsize::new(committed[1].as_ref().header().height().get() as usize).unwrap(),
                committed[0].as_ref().hash(),
            )
            .is_err(),
        "old finality cannot be relabelled as the newer carrier's inclusion"
    );
    assert_eq!(
        restored.world.assets.view().get(&fixture.source).unwrap().0,
        Quantity::from(60_u32)
    );
    assert_eq!(
        restored
            .world
            .assets
            .view()
            .get(&fixture.destination)
            .unwrap()
            .0,
        Quantity::from(40_u32)
    );

    // Relabel a copied completed source into a fresh proposal/current base and a
    // fresh route identity. Its old Decisions never authorize that new namespace.
    let before = crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap();
    let last = committed.last().unwrap().as_ref();
    for change_instance in [false, true] {
        let mut copied = committed[0]
            .as_ref()
            .execution_context()
            .unwrap()
            .native_lane_decisions
            .as_deref()
            .unwrap()
            .clone();
        copied.base_state_height = restored.committed_height() as u64;
        copied.base_state_hash = restored.lane_execution_state_hash().unwrap();
        if change_instance {
            copied.groups[0].payload.descriptor.slots[0].instance_id =
                Hash::new(b"fresh copied native instance");
            copied.groups[0].payload.descriptor.slots[0].lane_incarnation =
                Hash::new(b"fresh copied native incarnation");
        }
        let mut candidate = empty_global_block_after(Some(last)).canonical_resultless_proposal();
        candidate.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &restored.nexus_snapshot(),
            candidate.header().height().get(),
        )));
        candidate.set_execution_context(Some(
            BlockExecutionContextBundle::default().with_native_lane_decisions(copied),
        ));
        assert!(
            restored
                .prepare_proposed_native_lane_batch_source(&candidate, &[])
                .is_err(),
            "a completed source copied into a fresh header/base/namespace has no current authority"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            before
        );
    }
    assert_completed_native_history_for_test(&restored, &committed, &original_groups);
}
