use super::*;
use iroha_data_model::nexus::{
    LaneCatalog, LaneConfig as LaneConfigMetadata, LaneId, LaneVisibility,
};
use iroha_primitives::{addr::socket_addr, unique_vec};
use std::{collections::BTreeMap, num::NonZeroU32};
fn checked_random_keypair() -> KeyPair {
    KeyPair::try_random().expect("generate checked iroha_config dummy peer keypair")
}
fn dummy_peer(port: u16) -> Peer {
    Peer::new(
        socket_addr!(127.0.0.1:port),
        checked_random_keypair().into_parts().0,
    )
}
#[test]
fn torii_operator_auth_defaults_match_expected() {
    let auth = ToriiOperatorAuth::default();
    assert!(matches!(
        auth.token_fallback,
        OperatorTokenFallback::Bootstrap
    ));
    assert!(matches!(
        auth.token_source,
        OperatorTokenSource::OperatorTokens
    ));
    assert_eq!(
        auth.mtls_trusted_proxy_cidrs,
        defaults::torii::operator_auth::mtls_trusted_proxy_cidrs()
    );
}
#[test]
fn concurrency_validate_rejects_zero_stacks() {
    let mut cfg = Concurrency::from_defaults();
    cfg.scheduler_stack_bytes = 0;
    let err = cfg.validate().expect_err("zero stack should be invalid");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_defaults_to_safe_tokio_stack() {
    let cfg = Concurrency::from_defaults();
    assert_eq!(
        cfg.tokio_stack_bytes,
        defaults::concurrency::TOKIO_STACK_BYTES
    );
    cfg.validate().expect("default Tokio stack must be valid");
}
#[test]
fn concurrency_validate_rejects_tokio_stack_below_minimum() {
    let mut cfg = Concurrency::from_defaults();
    cfg.tokio_stack_bytes = defaults::concurrency::TOKIO_STACK_BYTES_MIN - 1;
    let err = cfg
        .validate()
        .expect_err("Tokio stack below minimum must fail");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_validate_rejects_tokio_stack_above_maximum() {
    let mut cfg = Concurrency::from_defaults();
    cfg.tokio_stack_bytes = defaults::concurrency::TOKIO_STACK_BYTES_MAX + 1;
    let err = cfg
        .validate()
        .expect_err("Tokio stack above maximum must fail");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_validate_rejects_too_small_sumeragi_stack() {
    let mut cfg = Concurrency::from_defaults();
    cfg.sumeragi_stack_bytes = defaults::concurrency::SUMERAGI_STACK_BYTES_MIN - 1;
    let err = cfg
        .validate()
        .expect_err("Sumeragi stack below minimum must fail");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_validate_rejects_known_unsafe_sumeragi_stack() {
    let mut cfg = Concurrency::from_defaults();
    cfg.sumeragi_stack_bytes = 32 * 1024 * 1024;
    let err = cfg
        .validate()
        .expect_err("known unsafe Sumeragi stack size must fail");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_validate_rejects_excessive_sumeragi_stack() {
    let mut cfg = Concurrency::from_defaults();
    cfg.sumeragi_stack_bytes = defaults::concurrency::SUMERAGI_STACK_BYTES_MAX + 1;
    let err = cfg
        .validate()
        .expect_err("Sumeragi stack above maximum must fail");
    assert!(matches!(
        err.current_context(),
        ParseError::InvalidConcurrencyConfig
    ));
}
#[test]
fn concurrency_validate_accepts_defaults() {
    assert!(Concurrency::from_defaults().validate().is_ok());
}
#[test]
fn lane_config_uses_metadata_shard_id() {
    let mut metadata = BTreeMap::new();
    metadata.insert("da_shard_id".to_string(), "9".to_string());
    let catalog = LaneCatalog::new(
        NonZeroU32::new(6).expect("lane count"),
        vec![LaneConfigMetadata {
            id: LaneId::new(5),
            alias: "lane5".into(),
            metadata,
            ..LaneConfigMetadata::default()
        }],
    )
    .expect("lane catalog");
    let config = LaneConfig::from_catalog(&catalog);
    let entry = config.entry(LaneId::new(5)).expect("lane entry");
    assert_eq!(entry.shard_id, 9);
    assert_eq!(config.shard_id(LaneId::new(5)), 9);
}
#[test]
fn shard_mapping_exposes_lane_binding() {
    let mut metadata = BTreeMap::new();
    metadata.insert("da_shard_id".to_string(), "7".to_string());
    let catalog = LaneCatalog::new(
        NonZeroU32::new(2).expect("lane count"),
        vec![
            LaneConfigMetadata {
                id: LaneId::new(0),
                alias: "lane1".into(),
                metadata,
                ..LaneConfigMetadata::default()
            },
            LaneConfigMetadata {
                id: LaneId::new(1),
                alias: "lane2".into(),
                ..LaneConfigMetadata::default()
            },
        ],
    )
    .expect("lane catalog");
    let config = LaneConfig::from_catalog(&catalog);
    assert_eq!(config.shard_id(LaneId::new(0)), 7);
    assert_eq!(config.shard_id(LaneId::new(1)), 1);
}
#[test]
fn shard_defaults_to_lane_id_when_metadata_missing() {
    let catalog = LaneCatalog::new(
        NonZeroU32::new(4).expect("lane count"),
        vec![LaneConfigMetadata {
            id: LaneId::new(3),
            alias: "lane3".into(),
            ..LaneConfigMetadata::default()
        }],
    )
    .expect("lane catalog");
    let config = LaneConfig::from_catalog(&catalog);
    let entry = config.entry(LaneId::new(3)).expect("lane entry");
    assert_eq!(entry.shard_id, 3);
}
#[test]
fn sorafs_anonymity_stage_accepts_only_exact_v1_labels() {
    for (label, expected) in [
        ("anon-guard-pq", SorafsAnonymityStage::GuardPq),
        ("anon-majority-pq", SorafsAnonymityStage::MajorityPq),
        ("anon-strict-pq", SorafsAnonymityStage::StrictPq),
    ] {
        assert_eq!(SorafsAnonymityStage::parse(label), Some(expected));
    }
    for rejected in [
        "",
        " anon-guard-pq",
        "anon-guard-pq ",
        "ANON-GUARD-PQ",
        "anon_guard_pq",
        "anon_majority_pq",
        "anon_strict_pq",
        "stage_a",
        "stage-a",
        "stagea",
        "stage_b",
        "stage-b",
        "stageb",
        "stage_c",
        "stage-c",
        "stagec",
        "anon-unknown",
    ] {
        assert_eq!(
            SorafsAnonymityStage::parse(rejected),
            None,
            "retired or noncanonical label `{rejected}` must fail"
        );
    }
}
#[test]
fn sorafs_anonymity_stage_labels_are_canonical() {
    assert_eq!(SorafsAnonymityStage::GuardPq.label(), "anon-guard-pq");
    assert_eq!(SorafsAnonymityStage::MajorityPq.label(), "anon-majority-pq");
    assert_eq!(SorafsAnonymityStage::StrictPq.label(), "anon-strict-pq");
}
#[test]
fn sorafs_rollout_phase_accepts_only_exact_v1_labels() {
    for (label, expected) in [
        ("canary", SorafsRolloutPhase::Canary),
        ("ramp", SorafsRolloutPhase::Ramp),
        ("default", SorafsRolloutPhase::Default),
    ] {
        assert_eq!(SorafsRolloutPhase::parse(label), Some(expected));
    }
    for rejected in [
        "",
        " canary",
        "canary ",
        "CANARY",
        "stage_a",
        "stage-a",
        "stagea",
        "stage_b",
        "stage-b",
        "stageb",
        "stage_c",
        "stage-c",
        "stagec",
        "majority",
        "stable",
        "ga",
        "unknown-rollout",
    ] {
        assert_eq!(
            SorafsRolloutPhase::parse(rejected),
            None,
            "retired or noncanonical label `{rejected}` must fail"
        );
    }
}
#[test]
fn sorafs_gateway_effective_anonymity_policy_respects_phase_fallback() {
    let mut gateway = SorafsGateway::default();
    assert_eq!(
        gateway.effective_anonymity_policy(),
        SorafsAnonymityStage::GuardPq
    );
    gateway.anonymity_policy = None;
    gateway.rollout_phase = SorafsRolloutPhase::Default;
    assert_eq!(
        gateway.effective_anonymity_policy(),
        SorafsAnonymityStage::StrictPq
    );
    gateway.anonymity_policy = Some(SorafsAnonymityStage::MajorityPq);
    assert_eq!(
        gateway.effective_anonymity_policy(),
        SorafsAnonymityStage::MajorityPq
    );
}
include!("runtime_tail_tests.rs");
