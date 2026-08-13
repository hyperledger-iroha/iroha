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
#[test]
fn preflight_lane_geometry_rejects_relabel_target_snapshot_dir() {
    let temp = tempdir().expect("tmpdir");
    let mut backend =
        TieredStateBackend::new(true, 0, 0, 0, Some(temp.path().to_path_buf()), None, 1, 0);
    let initial_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![LaneConfig {
            alias: "Alpha Lane".to_string(),
            ..LaneConfig::default()
        }],
    )
    .expect("initial catalog");
    let initial_cfg = RuntimeLaneConfig::from_catalog(&initial_catalog);
    backend
        .reconcile_lane_geometry(&RuntimeLaneConfig::default(), &initial_cfg, &[])
        .expect("provision initial snapshot");
    let old_entry = initial_cfg
        .entry(LaneId::SINGLE)
        .expect("initial lane entry");
    let updated_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![LaneConfig {
            alias: "Payments Lane".to_string(),
            ..LaneConfig::default()
        }],
    )
    .expect("updated catalog");
    let updated_cfg = RuntimeLaneConfig::from_catalog(&updated_catalog);
    let new_entry = updated_cfg
        .entry(LaneId::SINGLE)
        .expect("updated lane entry");
    let lanes_root = temp.path().join("lanes");
    let target_dir = lane_snapshot_dir(&lanes_root, new_entry);
    fs::create_dir_all(&target_dir).expect("seed conflicting relabel target");
    let err = backend
        .preflight_lane_geometry(&initial_cfg, &updated_cfg, &[], &[(old_entry, new_entry)])
        .expect_err("occupied relabel target must fail preflight");
    assert!(
        format!("{err:?}").contains("lane snapshot relabel target already exists"),
        "unexpected error: {err:?}"
    );
    assert!(
        lane_snapshot_dir(&lanes_root, old_entry).exists(),
        "preflight must not move source snapshot dir"
    );
    assert!(
        target_dir.exists(),
        "preflight must leave conflicting target dir in place"
    );
}
