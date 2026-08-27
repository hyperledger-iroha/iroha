#[test]
fn sorafs_site_binding_defaults_are_disabled_and_bounded() {
    let config = SorafsGatewaySiteBindings::default();
    assert_eq!(config.path, None);
    assert_eq!(config.max_bytes.get(), 1024 * 1024);
    assert_eq!(config.max_sites.get(), 1024);
}
#[test]
fn streaming_codec_default_entropy_mode_matches_build_flag() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let codec = StreamingCodec::from_defaults();
    assert_eq!(codec.entropy_mode, EntropyMode::RansBundled);
}
#[test]
fn streaming_default_entropy_string_tracks_build_flag() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let default = defaults::streaming::codec::entropy_mode();
    assert_eq!(
        default,
        defaults::streaming::codec::BUNDLED_ENTROPY_MODE,
        "string helper should mirror bundled availability"
    );
}
#[test]
fn soranet_pow_defaults_are_const_initializable() {
    const CONST_POW: SoranetPow = SoranetPow::default_const();
    const CONST_PUZZLE: SoranetPuzzle = SoranetPuzzle::default_const();
    let runtime = SoranetPow::default();
    assert_eq!(
        runtime.difficulty,
        iroha_crypto::soranet::puzzle::DEFAULT_DIFFICULTY
    );
    assert_ne!(runtime.difficulty, 0);
    assert_eq!(CONST_POW.difficulty, runtime.difficulty);
    assert_eq!(CONST_POW.max_future_skew, runtime.max_future_skew);
    assert_eq!(CONST_POW.min_ticket_ttl, runtime.min_ticket_ttl);
    assert_eq!(CONST_POW.ticket_ttl, runtime.ticket_ttl);
    assert_eq!(
        CONST_POW.revocation_store_capacity,
        runtime.revocation_store_capacity
    );
    assert_eq!(CONST_POW.revocation_max_ttl, runtime.revocation_max_ttl);
    assert_eq!(
        CONST_POW.revocation_store_path,
        runtime.revocation_store_path
    );
    assert_eq!(CONST_PUZZLE.memory_kib, runtime.puzzle.memory_kib);
    assert_eq!(CONST_PUZZLE.time_cost, runtime.puzzle.time_cost);
    assert_eq!(CONST_PUZZLE.lanes, runtime.puzzle.lanes);
}
#[test]
fn no_trusted_peers() {
    let value = TrustedPeers {
        myself: dummy_peer(80),
        others: unique_vec![],
        pops: std::collections::BTreeMap::default(),
    };
    assert!(!value.contains_other_trusted_peers());
}
#[test]
fn one_trusted_peer() {
    let value = TrustedPeers {
        myself: dummy_peer(80),
        others: unique_vec![dummy_peer(81)],
        pops: std::collections::BTreeMap::default(),
    };
    assert!(value.contains_other_trusted_peers());
}
#[test]
fn many_trusted_peers() {
    let value = TrustedPeers {
        myself: dummy_peer(80),
        others: unique_vec![dummy_peer(1), dummy_peer(2), dummy_peer(3), dummy_peer(4),],
        pops: std::collections::BTreeMap::default(),
    };
    assert!(value.contains_other_trusted_peers());
}
#[test]
fn telemetry_profile_capabilities_match_expectations() {
    let disabled = TelemetryProfile::Disabled.capabilities();
    assert!(!disabled.metrics_enabled());
    assert!(!disabled.expensive_metrics_enabled());
    assert!(!disabled.developer_outputs_enabled());
    let operator = TelemetryProfile::Operator.capabilities();
    assert!(operator.metrics_enabled());
    assert!(!operator.expensive_metrics_enabled());
    assert!(!operator.developer_outputs_enabled());
    let full = TelemetryProfile::Full.capabilities();
    assert!(full.metrics_enabled());
    assert!(full.expensive_metrics_enabled());
    assert!(full.developer_outputs_enabled());
    let combined = TelemetryCapabilities::from(TelemetryProfile::Developer)
        .union(TelemetryCapabilities::from(TelemetryProfile::Extended));
    assert!(combined.metrics_enabled());
    assert!(combined.expensive_metrics_enabled());
    assert!(combined.developer_outputs_enabled());
}
#[test]
fn telemetry_profile_from_user_enum_round_trips() {
    use super::user;
    assert_eq!(
        TelemetryProfile::from(user::TelemetryProfile::Operator),
        TelemetryProfile::Operator
    );
    assert_eq!(
        TelemetryProfile::from(user::TelemetryProfile::Extended),
        TelemetryProfile::Extended
    );
    assert_eq!(
        TelemetryProfile::from(user::TelemetryProfile::Developer),
        TelemetryProfile::Developer
    );
    assert_eq!(
        TelemetryProfile::from(user::TelemetryProfile::Full),
        TelemetryProfile::Full
    );
}
#[test]
fn fraud_monitoring_new_dedup_and_defaults() {
    use url::Url;
    let url = Url::parse("https://risk.example/api").expect("url");
    let cfg = FraudMonitoring::new(
        true,
        vec![url.clone(), url.clone()],
        Duration::from_millis(0),
        Duration::from_millis(0),
        5,
        Some(FraudRiskBand::High),
        Vec::new(),
    );
    assert_eq!(cfg.service_endpoints.len(), 1);
    assert_eq!(cfg.service_endpoints[0], url);
    assert_eq!(
        cfg.connect_timeout,
        defaults::fraud_monitoring::CONNECT_TIMEOUT
    );
    assert_eq!(
        cfg.request_timeout,
        defaults::fraud_monitoring::REQUEST_TIMEOUT
    );
    assert_eq!(cfg.missing_assessment_grace, Duration::from_secs(5));
    assert_eq!(cfg.required_minimum_band, Some(FraudRiskBand::High));
}
#[test]
fn fraud_monitoring_default_matches_defaults() {
    let cfg = FraudMonitoring::default();
    assert!(!cfg.enabled);
    assert!(cfg.service_endpoints.is_empty());
    assert_eq!(
        cfg.connect_timeout,
        defaults::fraud_monitoring::CONNECT_TIMEOUT
    );
    assert_eq!(
        cfg.request_timeout,
        defaults::fraud_monitoring::REQUEST_TIMEOUT
    );
    assert_eq!(
        cfg.missing_assessment_grace,
        Duration::from_secs(defaults::fraud_monitoring::MISSING_ASSESSMENT_GRACE_SECS,)
    );
    assert!(cfg.required_minimum_band.is_none());
    assert!(cfg.attesters.is_empty());
}
#[test]
fn lane_config_derives_storage_geometry() {
    let catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("nonzero lane count"),
        vec![
            LaneConfigMetadata::default(),
            LaneConfigMetadata {
                id: LaneId::new(1),
                alias: "Public Lane ①".to_string(),
                lane_type: Some("default_public".to_string()),
                governance: Some("parliament".to_string()),
                ..LaneConfigMetadata::default()
            },
        ],
    )
    .expect("catalog");
    let config = LaneConfig::from_catalog(&catalog);
    let entries = config.entries();
    assert_eq!(entries.len(), 2);
    let default_entry = config.entry(LaneId::SINGLE).expect("default lane exists");
    assert_eq!(default_entry.alias, "default");
    assert_eq!(default_entry.slug, "default");
    assert_eq!(default_entry.kura_segment, "lane_000_default");
    assert_eq!(default_entry.merge_segment, "lane_000_default_merge");
    assert_eq!(
        default_entry.merge_log_path("/tmp/iroha"),
        PathBuf::from("/tmp/iroha/merge_ledger/lane_000_default_merge.log")
    );
    assert_eq!(
        default_entry.key_prefix,
        LaneId::SINGLE.as_u32().to_be_bytes()
    );
    assert_eq!(default_entry.dataspace_id, DataSpaceId::UNIVERSAL);
    assert_eq!(default_entry.visibility, LaneVisibility::Public);
    assert_eq!(
        default_entry.storage_profile,
        LaneStorageProfile::FullReplica
    );
    let public_entry = config.entry(LaneId::new(1)).expect("lane 1 exists");
    assert_eq!(public_entry.alias, "Public Lane ①");
    assert_eq!(public_entry.slug, "public_lane");
    assert_eq!(public_entry.kura_segment, "lane_001_public_lane");
    assert_eq!(public_entry.merge_segment, "lane_001_public_lane_merge");
    assert_eq!(
        public_entry.merge_log_path("/tmp/iroha"),
        PathBuf::from("/tmp/iroha/merge_ledger/lane_001_public_lane_merge.log")
    );
    assert_eq!(
        public_entry.key_prefix,
        LaneId::new(1).as_u32().to_be_bytes()
    );
    assert_eq!(public_entry.dataspace_id, DataSpaceId::UNIVERSAL);
    assert_eq!(public_entry.visibility, LaneVisibility::Public);
    assert_eq!(
        public_entry.storage_profile,
        LaneStorageProfile::FullReplica
    );
}
