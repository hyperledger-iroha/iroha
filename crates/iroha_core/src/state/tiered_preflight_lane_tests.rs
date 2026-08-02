#[test]
fn preflight_lane_geometry_rejects_retired_lane_root_file() {
    let temp = tempdir().expect("tmpdir");
    let mut backend =
        TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 1, 0);

    let lane1 = LaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..LaneConfig::default()
    };
    let two_lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![LaneConfig::default(), lane1])
        .expect("two-lane catalog");
    let two_lane_cfg = RuntimeLaneConfig::from_catalog(&two_lane_catalog);
    backend
        .reconcile_lane_geometry(&RuntimeLaneConfig::default(), &two_lane_cfg, &[])
        .expect("provision lane snapshots");

    let retired_lane_root = temp.path().join("retired").join("lanes");
    if let Some(parent) = retired_lane_root.parent() {
        fs::create_dir_all(parent).expect("retired parent");
    }
    fs::write(&retired_lane_root, b"blocker").expect("retired root blocker");

    let err = backend
        .preflight_lane_geometry(&two_lane_cfg, &RuntimeLaneConfig::default(), &[], &[])
        .expect_err("retired root file must fail preflight");

    assert!(
        format!("{err:?}").contains("expected directory path"),
        "unexpected error: {err:?}"
    );
    let lane1_entry = two_lane_cfg.entry(LaneId::from(1)).expect("lane 1 entry");
    assert!(
        lane_snapshot_dir(&temp.path().join("lanes"), lane1_entry).exists(),
        "preflight must not retire source snapshot dir"
    );
    assert!(
        retired_lane_root.is_file(),
        "preflight must leave conflicting retired root in place"
    );
}
