#[test]
fn install_lane_manifests_updates_privacy_registry() {
    let chain: ChainId = "lane-privacy-registry".parse().unwrap();
    let world = World::default();
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_chain(world, kura, query_handle, chain);
    let commitment = LanePrivacyCommitment::merkle(
        LaneCommitmentId::new(9),
        MerkleCommitment::from_root_bytes([0x11; 32], 8),
    );
    let status = LaneManifestStatus {
        lane: TestLaneId::SINGLE,
        alias: "private".to_string(),
        dataspace: TestDataSpaceId::UNIVERSAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::CommitmentOnly,
        governance: None,
        manifest_path: Some(PathBuf::from("/tmp/privacy.json")),
        governance_rules: None,
        privacy_commitments: vec![commitment],
    };
    let mut statuses = BTreeMap::new();
    statuses.insert(TestLaneId::SINGLE, status);
    let registry = Arc::new(LaneManifestRegistry::from_statuses(statuses));
    state.install_lane_manifests(&registry);
    let snapshot = state.lane_privacy_registry.read().clone();
    assert!(!snapshot.is_empty(), "privacy registry should not be empty");
    assert!(
        snapshot.lane(TestLaneId::SINGLE).is_some(),
        "privacy registry should contain lane entry"
    );
}
