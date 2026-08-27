use soranet_relay::config::{
    ConfigError, VPN_MAX_COVER_BURST_CELLS_V1, VpnConfig, VpnCoverTrafficConfig,
};
fn secure_receipt_spool() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("create receipt spool");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("protect receipt spool");
    }
    directory
}
#[test]
fn vpn_defaults_apply_and_validate() {
    let receipt_spool = secure_receipt_spool();
    let mut cfg = VpnConfig {
        enabled: true,
        cell_size_bytes: 0,
        flow_label_bits: 24,
        pacing_millis: 0,
        padding_budget_ms: 0,
        exit_class: "standard".to_string(),
        lease_secs: 0,
        dns_push_interval_secs: 0,
        route_push: vec!["10.0.0.0/24 ".to_string()],
        dns_overrides: vec![" 1.1.1.1 ".to_string()],
        helper_ticket_issuer_public_key_path: Some(
            "/run/secrets/vpn-helper-ticket-issuer-public-key.hex".into(),
        ),
        helper_ticket_replay_store_capacity: 0,
        helper_ticket_replay_store_path: Default::default(),
        backend_endpoint: Some("unix:/tmp/soranet-vpn-config-defaults.sock".into()),
        backend_expected_uid: Some(0),
        backend_expected_gid: Some(0),
        backend_bootstrap_secret_path: Some("/run/secrets/vpn-backend-bootstrap.hex".into()),
        usage_voucher_credit_window_bytes: 0,
        usage_voucher_max_age_ms: 0,
        usage_voucher_setup_timeout_ms: 0,
        receipt_spool_dir: Some(receipt_spool.path().to_path_buf()),
        cover: VpnCoverTrafficConfig {
            enabled: false,
            cover_to_data_per_mille: 0,
            heartbeat_ms: 0,
            max_cover_burst: 0,
            max_jitter_millis: 0,
        },
        billing: Default::default(),
    };
    cfg.validate().expect("vpn defaults should validate");
    assert_eq!(cfg.cell_size_bytes, 1_024);
    assert!(cfg.pacing_millis > 0);
    assert!(cfg.padding_budget_ms > 0);
    assert_eq!(cfg.route_push[0], "10.0.0.0/24");
    assert_eq!(cfg.dns_overrides[0], "1.1.1.1");
    assert_eq!(cfg.cover.cover_to_data_per_mille, 250);
    assert_eq!(cfg.cover.heartbeat_ms, 500);
    assert_eq!(cfg.cover.max_cover_burst, 3);
    assert_eq!(cfg.cover.max_jitter_millis, 10);
    assert_eq!(cfg.helper_ticket_replay_store_capacity, 8_192);
    assert!(!cfg.helper_ticket_replay_store_path.as_os_str().is_empty());
    assert_eq!(cfg.usage_voucher_credit_window_bytes, 1_048_576);
    assert_eq!(cfg.usage_voucher_max_age_ms, 5_000);
    assert_eq!(cfg.usage_voucher_setup_timeout_ms, 30_000);
}
#[test]
fn vpn_flow_label_bits_must_be_bounded() {
    let mut cfg = VpnConfig {
        enabled: true,
        flow_label_bits: 0,
        lease_secs: 60,
        dns_push_interval_secs: 60,
        ..VpnConfig::default()
    };
    let err = cfg.validate().expect_err("zero bits should fail");
    assert!(matches!(err, ConfigError::Vpn(message) if message.contains("flow_label_bits")));
    cfg.flow_label_bits = 25;
    let err = cfg.validate().expect_err("overflow bits should fail");
    assert!(matches!(err, ConfigError::Vpn(message) if message.contains("flow_label_bits")));
}
#[test]
fn vpn_cover_jitter_guardrails() {
    let mut cfg = VpnConfig {
        enabled: true,
        cell_size_bytes: 1_024,
        flow_label_bits: 24,
        pacing_millis: 10,
        padding_budget_ms: 10,
        exit_class: "standard".to_string(),
        lease_secs: 60,
        dns_push_interval_secs: 60,
        route_push: vec![],
        dns_overrides: vec![],
        helper_ticket_issuer_public_key_path: Some(
            "/run/secrets/vpn-helper-ticket-issuer-public-key.hex".into(),
        ),
        helper_ticket_replay_store_capacity: 8_192,
        helper_ticket_replay_store_path: "./storage/soranet/vpn_helper_ticket_replays.norito"
            .into(),
        backend_endpoint: Some("unix:/tmp/soranet-vpn-config-cover.sock".into()),
        backend_expected_uid: Some(0),
        backend_expected_gid: Some(0),
        backend_bootstrap_secret_path: Some("/run/secrets/vpn-backend-bootstrap.hex".into()),
        usage_voucher_credit_window_bytes: 1_048_576,
        usage_voucher_max_age_ms: 5_000,
        usage_voucher_setup_timeout_ms: 30_000,
        receipt_spool_dir: None,
        cover: VpnCoverTrafficConfig {
            enabled: true,
            cover_to_data_per_mille: 500,
            heartbeat_ms: 50,
            max_cover_burst: 1,
            max_jitter_millis: 100,
        },
        billing: Default::default(),
    };
    let err = cfg.validate().expect_err("jitter should be bounded");
    match err {
        ConfigError::Vpn(message) => assert!(
            message.contains("max_jitter_millis"),
            "unexpected message: {message}"
        ),
        other => panic!("unexpected error {other:?}"),
    }
}

#[test]
fn vpn_cover_burst_is_bounded() {
    let mut cfg = VpnConfig::default();
    cfg.cover.enabled = true;
    cfg.cover.max_cover_burst = VPN_MAX_COVER_BURST_CELLS_V1 + 1;
    let error = cfg
        .validate()
        .expect_err("oversized cover burst must fail before scheduling");
    assert!(
        matches!(error, ConfigError::Vpn(message) if message.contains("max_cover_burst") && message.contains("64"))
    );
}
#[test]
fn vpn_runtime_available_allows_enable() {
    let cfg = VpnConfig {
        enabled: true,
        ..VpnConfig::default()
    };
    cfg.require_runtime_available()
        .expect("vpn runtime availability should pass");
}
#[test]
fn vpn_config_json_roundtrip_preserves_fields() {
    let receipt_spool = secure_receipt_spool();
    let mut cfg = VpnConfig {
        enabled: true,
        cell_size_bytes: 1_024,
        flow_label_bits: 24,
        pacing_millis: 15,
        padding_budget_ms: 8,
        exit_class: "standard".to_string(),
        lease_secs: 600,
        dns_push_interval_secs: 120,
        route_push: vec!["10.0.0.0/24".into()],
        dns_overrides: vec!["8.8.8.8".into()],
        helper_ticket_issuer_public_key_path: Some(
            "/run/secrets/vpn-helper-ticket-issuer-public-key.hex".into(),
        ),
        helper_ticket_replay_store_capacity: 4_096,
        helper_ticket_replay_store_path: "/var/lib/soranet/helper-replays.norito".into(),
        backend_endpoint: Some("unix:/tmp/soranet-vpn-config-roundtrip.sock".into()),
        backend_expected_uid: Some(0),
        backend_expected_gid: Some(0),
        backend_bootstrap_secret_path: Some("/run/secrets/vpn-backend-bootstrap.hex".into()),
        usage_voucher_credit_window_bytes: 256 * 1_024,
        usage_voucher_max_age_ms: 7_000,
        usage_voucher_setup_timeout_ms: 45_000,
        receipt_spool_dir: Some(receipt_spool.path().to_path_buf()),
        cover: VpnCoverTrafficConfig::default(),
        billing: Default::default(),
    };
    cfg.validate().expect("config should validate");
    let json = norito::json::to_vec(&cfg).expect("serialize vpn config");
    let decoded: VpnConfig = norito::json::from_slice(&json).expect("decode vpn config");
    assert_eq!(cfg.enabled, decoded.enabled);
    assert_eq!(cfg.cell_size_bytes, decoded.cell_size_bytes);
    assert_eq!(cfg.flow_label_bits, decoded.flow_label_bits);
    assert_eq!(cfg.pacing_millis, decoded.pacing_millis);
    assert_eq!(cfg.padding_budget_ms, decoded.padding_budget_ms);
    assert_eq!(cfg.route_push, decoded.route_push);
    assert_eq!(cfg.dns_overrides, decoded.dns_overrides);
    assert_eq!(
        cfg.helper_ticket_replay_store_capacity,
        decoded.helper_ticket_replay_store_capacity
    );
    assert_eq!(
        cfg.helper_ticket_replay_store_path,
        decoded.helper_ticket_replay_store_path
    );
    assert_eq!(cfg.backend_expected_uid, decoded.backend_expected_uid);
    assert_eq!(cfg.backend_expected_gid, decoded.backend_expected_gid);
    assert_eq!(
        cfg.usage_voucher_credit_window_bytes,
        decoded.usage_voucher_credit_window_bytes
    );
    assert_eq!(
        cfg.usage_voucher_max_age_ms,
        decoded.usage_voucher_max_age_ms
    );
    assert_eq!(
        cfg.usage_voucher_setup_timeout_ms,
        decoded.usage_voucher_setup_timeout_ms
    );
    assert_eq!(cfg.receipt_spool_dir, decoded.receipt_spool_dir);
}
#[test]
fn vpn_usage_voucher_freshness_is_bounded() {
    for invalid in [1_999, 30_001] {
        let mut cfg = VpnConfig {
            enabled: true,
            usage_voucher_max_age_ms: invalid,
            ..VpnConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("unsafe voucher freshness window must fail");
        assert!(
            matches!(err, ConfigError::Vpn(message) if message.contains("usage_voucher_max_age_ms"))
        );
    }
}
#[test]
fn vpn_usage_voucher_credit_window_is_bounded() {
    for invalid in [256 * 1_024 - 1, 16 * 1_048_576 + 1] {
        let mut cfg = VpnConfig {
            enabled: true,
            usage_voucher_credit_window_bytes: invalid,
            ..VpnConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("out-of-range prepaid credit must fail");
        assert!(
            matches!(err, ConfigError::Vpn(message) if message.contains("usage_voucher_credit_window_bytes"))
        );
    }
}
#[test]
fn vpn_usage_voucher_setup_timeout_is_bounded() {
    for invalid in [4_999, 120_001] {
        let mut cfg = VpnConfig {
            enabled: true,
            usage_voucher_max_age_ms: 5_000,
            usage_voucher_setup_timeout_ms: invalid,
            ..VpnConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("unsafe voucher setup timeout must fail");
        assert!(
            matches!(err, ConfigError::Vpn(message) if message.contains("usage_voucher_setup_timeout_ms"))
        );
    }
}
#[test]
fn vpn_rejects_mismatched_cell_size() {
    let mut cfg = VpnConfig {
        enabled: true,
        cell_size_bytes: 512,
        ..VpnConfig::default()
    };
    let err = cfg.validate().expect_err("cell size mismatch should fail");
    match err {
        ConfigError::Vpn(message) => {
            assert!(
                message.contains("pinned cell length"),
                "unexpected message: {message}"
            )
        }
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_meter_hash_must_be_valid_hex() {
    let mut cfg = VpnConfig {
        enabled: true,
        billing: soranet_relay::config::VpnBillingConfig {
            meter_hash_hex: "1234".to_string(),
            ..Default::default()
        },
        ..VpnConfig::default()
    };
    let err = cfg.validate().expect_err("invalid meter hash should fail");
    match err {
        ConfigError::Vpn(message) => assert!(
            message.contains("meter_hash_hex"),
            "unexpected message: {message}"
        ),
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn vpn_pacing_must_fit_u16() {
    let mut cfg = VpnConfig {
        enabled: true,
        pacing_millis: u64::from(u16::MAX) + 1,
        ..VpnConfig::default()
    };
    let err = cfg.validate().expect_err("pacing overflow should fail");
    match err {
        ConfigError::Vpn(message) => {
            assert!(message.contains("u16"), "unexpected message: {message}")
        }
        other => panic!("unexpected error {other:?}"),
    }
}
