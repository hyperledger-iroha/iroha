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
            .matches("HistoricalNativeAmxSourceAuthority::CertifiedCoordinator(")
            .count(),
        1,
        "bundle-only diagnostics must establish the still-active coordinator authority",
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
