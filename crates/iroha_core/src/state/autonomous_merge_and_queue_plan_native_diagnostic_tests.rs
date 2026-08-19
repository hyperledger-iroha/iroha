// Pending Native AMX diagnostic authentication and source-boundary regressions.
fn assert_passive_state_diagnostics(
    state: &State,
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    session: &crate::lane_consensus::CommittedLaneBlockSession,
) {
    let lane_artifact_dir = lane_config
        .entry(LaneId::SINGLE)
        .expect("default diagnostic lane entry")
        .blocks_dir(kura.store_root())
        .join("lane_artifacts");
    let ownership_data = lane_artifact_dir.join("ownerships.norito");
    let ownership_index = lane_artifact_dir.join("ownerships.index");
    let ownership_data_temp = ownership_data.with_extension("norito.tmp");
    let ownership_index_temp = ownership_index.with_extension("index.tmp");
    std::fs::rename(&ownership_data, &ownership_data_temp)
        .expect("stage ownership data recovery artifact");
    std::fs::rename(&ownership_index, &ownership_index_temp)
        .expect("stage ownership index recovery artifact");
    let staged_data = std::fs::read(&ownership_data_temp).expect("read staged ownership data");
    let staged_index = std::fs::read(&ownership_index_temp).expect("read staged ownership index");
    let passive_revision = kura.committed_lane_status_revision();
    for _ in 0..2 {
        let _ = state.durable_lane_diagnostics();
        state
            .native_amx_participant_applications_diagnostics()
            .expect("derive passive Native diagnostics");
        state
            .autonomous_lane_execution_diagnostics()
            .expect("derive passive autonomous diagnostics");
    }
    assert!(!ownership_data.exists());
    assert!(!ownership_index.exists());
    assert_eq!(
        std::fs::read(&ownership_data_temp).expect("reread staged ownership data"),
        staged_data,
    );
    assert_eq!(
        std::fs::read(&ownership_index_temp).expect("reread staged ownership index"),
        staged_index,
    );
    assert_eq!(
        kura.committed_lane_status_revision(),
        passive_revision,
        "repeated State diagnostics must not promote Kura recovery artifacts",
    );
    kura.recover_lane_block_payload(&session.proposal)
        .expect("explicitly recover diagnostic ownership evidence");
    assert!(ownership_data.is_file());
    assert!(ownership_index.is_file());
    assert!(!ownership_data_temp.exists());
    assert!(!ownership_index_temp.exists());
}
#[test]
fn pending_native_diagnostic_entry_rejects_forged_merge_qc_and_lane_bindings() {
    let (state, validator_keypairs, commit_keypairs, parent) = configured_two_lane_merge_state();
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (_, certificate) = queue_plan_admission_certificate_for_state_test(
        &state,
        routing_plan,
        &validator_keypairs,
        1,
        0x62,
    );
    let candidate = state
        .merge_candidate_with_queue_plan_admissions(&parent.header(), 0, None, vec![certificate])
        .expect("canonical diagnostic merge candidate")
        .expect("QueuePlan control produces a merge candidate");
    let merge_qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, merge_qc);
    assert!(
        state
            .authenticated_native_amx_participant_application_rows_from_merge_entry(&entry)
            .expect("the exact merge-QC-authenticated diagnostic entry is valid")
            .is_empty()
    );
    let mut forged_binding = entry.clone();
    forged_binding.active_lanes[1].incarnation =
        Hash::new(b"forged-pending-native-diagnostic-incarnation");
    let incarnation_entries = forged_binding
        .active_lanes
        .iter()
        .map(
            |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                lane_id: binding.lane_id,
                incarnation: binding.incarnation,
            },
        )
        .collect::<Vec<_>>();
    forged_binding.incarnation_root =
        iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(&incarnation_entries);
    forged_binding.activation_root =
        crate::merge::merge_activation_root(&forged_binding.active_lanes);
    assert!(
            state
                .authenticated_native_amx_participant_application_rows_from_merge_entry(
                    &forged_binding,
                )
                .is_err(),
            "active-lane bytes changed after certification must fail the merge-QC trust root",
        );
    let mut forged_qc = entry;
    forged_qc.merge_qc.aggregate_signature[0] ^= 0x80;
    assert!(
        state
            .authenticated_native_amx_participant_application_rows_from_merge_entry(&forged_qc,)
            .is_err(),
        "forged merge-QC bytes must fail before diagnostic rows are derived",
    );
}
#[test]
fn historical_native_amx_recovery_and_diagnostics_share_the_frozen_source_boundary() {
    let source = include_str!("../state.rs");
    assert_eq!(
        source
            .matches("crate::block::validate_historical_native_amx_source_bundle(")
            .count(),
        2,
        "restart recovery and certified diagnostics must share one source validator",
    );
    assert_eq!(
        source
            .matches("HistoricalNativeAmxSourceAuthority::MergeQcActiveLanes(")
            .count(),
        2,
        "recovery and pending-merge diagnostics must use merge-QC-authenticated lane bindings",
    );
    assert_eq!(
        source
            .matches("HistoricalNativeAmxSourceAuthority::CertifiedCoordinator {")
            .count(),
        1,
        "bundle-only diagnostics must use proposal-finality-authenticated coordinator authority",
    );
    let recovery_branch = source
        .rfind("if !validate_live_authority {")
        .map(|start| &source[start..])
        .and_then(|suffix| suffix.split("continue;").next())
        .expect("historical merge-validation branch remains source-bound");
    assert!(recovery_branch.contains("validate_historical_native_amx_source_bundle"));
    assert!(recovery_branch.contains("MergeQcActiveLanes"));
    assert!(
        !recovery_branch.contains("validate_native_amx_receipt_against_plan("),
        "historical recovery must not re-enter current lifecycle validation",
    );
}
