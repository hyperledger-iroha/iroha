fn assert_passive_committed_lane_status_reads(
    adapter: &V2LaneWorkAdapter,
    proposal: &LaneBlockProposalV1,
) {
    use super::super::status::CommittedLaneBlockExecutionStatus as ExecutionStatus;

    let lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(
        &iroha_data_model::nexus::LaneCatalog::default(),
    );
    let lane_artifact_dir = lane_config
        .entry(proposal.descriptor.lane_id)
        .expect("status fixture lane entry")
        .blocks_dir(adapter.kura.store_root())
        .join("lane_artifacts");
    let ownership_data = lane_artifact_dir.join("ownerships.norito");
    let ownership_index = lane_artifact_dir.join("ownerships.index");
    let ownership_data_temp = ownership_data.with_extension("norito.tmp");
    let ownership_index_temp = ownership_index.with_extension("index.tmp");
    std::fs::rename(&ownership_data, &ownership_data_temp)
        .expect("stage status ownership data recovery artifact");
    std::fs::rename(&ownership_index, &ownership_index_temp)
        .expect("stage status ownership index recovery artifact");
    let staged_data = std::fs::read(&ownership_data_temp).expect("read status ownership data");
    let staged_index = std::fs::read(&ownership_index_temp).expect("read status ownership index");
    let passive_revision = adapter.committed_lane_block_status_revision();
    for _ in 0..2 {
        let snapshot = adapter.committed_lane_block_status_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].proposal, *proposal);
        assert_eq!(
            snapshot[0].execution_status,
            ExecutionStatus::PayloadRecoveredAwaitingStateApplication,
        );
    }
    assert!(!ownership_data.exists());
    assert!(!ownership_index.exists());
    assert_eq!(
        std::fs::read(&ownership_data_temp).expect("reread status ownership data"),
        staged_data,
    );
    assert_eq!(
        std::fs::read(&ownership_index_temp).expect("reread status ownership index"),
        staged_index,
    );
    assert_eq!(
        adapter.committed_lane_block_status_revision(),
        passive_revision,
        "operator-status reads must not publish a Kura revision",
    );
    adapter
        .kura
        .recover_lane_block_payload(proposal)
        .expect("explicitly recover status ownership evidence");
    assert!(ownership_data.is_file());
    assert!(ownership_index.is_file());
    assert!(!ownership_data_temp.exists());
    assert!(!ownership_index_temp.exists());
}
