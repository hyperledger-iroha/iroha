use soranet_relay::config::{ConfigError, VpnConfig, VpnCoverTrafficConfig};

#[test]
fn vpn_defaults_apply_and_validate() {
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
    let mut cfg = VpnConfig {
        enabled: true,
        cell_size_bytes: 1_024,
        flow_label_bits: 20,
        pacing_millis: 15,
        padding_budget_ms: 8,
        exit_class: "standard".to_string(),
        lease_secs: 600,
        dns_push_interval_secs: 120,
        route_push: vec!["10.0.0.0/24".into()],
        dns_overrides: vec!["8.8.8.8".into()],
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
