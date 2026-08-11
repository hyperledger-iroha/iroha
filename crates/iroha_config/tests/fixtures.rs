#![allow(clippy::assertions_on_constants)]
//! Test fixtures exercising `iroha_config` parameter loading and validation.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Mutex, MutexGuard, Once},
    time::Duration,
};

use assertables::assert_contains;
use error_stack::{Report, ResultExt};
use expect_test::expect_file;
use iroha_config::parameters::user::ParseError;
#[allow(unused_imports)]
use iroha_config::parameters::{
    actual::{
        BlockSync, DaManifestPolicy, DataspaceGossip, DataspaceGossipFallback, FraudRiskBand,
        LaneProfile, NexusFeeSettlementMode, NexusStorage, OperatorAuthLockout,
        OperatorTokenFallback, OperatorTokenSource, OracleChangeThresholds, OracleEconomics,
        OracleGovernance, OracleTwitterBinding, Queue, Root as Config, SoranetVpn, Streaming,
        StreamingSoranetAccessKind, StreamingSoravpn, StreamingSync, ToriiOperatorAuth,
        TransactionGossiper,
    },
    defaults,
    user::{Root as UserConfig, ToriiSoranetPrivacyIngest},
};
use iroha_config_base::{
    env::MockEnv,
    read::ConfigReader,
    toml::{TomlSource, WriteExt as _},
};
use iroha_crypto::{Algorithm, ExposedPrivateKey, Hash, KeyPair, PrivateKey, PublicKey};
use iroha_data_model::account::AccountId;
use soranet_pq::MlKemSuite;
use thiserror::Error;
use toml::{Table, Value as TomlValue};
use url::Url;

fn fixtures_dir() -> PathBuf {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"))
            .expect("tests run relative to crate root");
    });
    PathBuf::from("tests/fixtures")
}

fn parse_env(raw: impl AsRef<str>) -> HashMap<String, String> {
    raw.as_ref()
        .lines()
        .map(|line| {
            let mut items = line.split('=');
            let key = items
                .next()
                .expect("line should be in {key}={value} format");
            let value = items
                .next()
                .expect("line should be in {key}={value} format");
            (key.to_string(), value.to_string())
        })
        .collect()
}

fn test_env_from_file(p: impl AsRef<Path>) -> MockEnv {
    let contents = fs::read_to_string(p).expect("the path should be valid");
    let map = parse_env(contents);
    MockEnv::with_map(map)
}

fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

struct AddressRuntimeGuard {
    chain_discriminant: u16,
    _lock: MutexGuard<'static, ()>,
}

impl AddressRuntimeGuard {
    fn capture() -> Self {
        static ADDRESS_RUNTIME_LOCK: Mutex<()> = Mutex::new(());
        let lock = ADDRESS_RUNTIME_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self {
            chain_discriminant: iroha_data_model::account::address::chain_discriminant(),
            _lock: lock,
        }
    }
}

impl Drop for AddressRuntimeGuard {
    fn drop(&mut self) {
        iroha_data_model::account::address::set_chain_discriminant(self.chain_discriminant);
    }
}

#[derive(Error, Debug)]
#[error("failed to load config from fixtures")]
struct FixtureConfigLoadError;

include!("fixtures/soranet_transport_identity_tests.rs");

fn load_config_from_fixtures(path: impl AsRef<Path>) -> Result<Config, FixtureConfigLoadError> {
    let config = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join(path))
        .change_context(FixtureConfigLoadError)?
        .read_and_complete::<UserConfig>()
        .change_context(FixtureConfigLoadError)?
        .parse()
        .change_context(FixtureConfigLoadError)?;

    Ok(config)
}

#[allow(dead_code)]
fn load_user_config_from_fixtures(
    path: impl AsRef<Path>,
) -> Result<UserConfig, FixtureConfigLoadError> {
    ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join(path))
        .change_context(FixtureConfigLoadError)?
        .read_and_complete::<UserConfig>()
        .change_context(FixtureConfigLoadError)
}

#[test]
fn quic_datagram_buffers_default_to_one_mib() {
    assert_eq!(
        defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get(),
        1024 * 1024
    );
    assert_eq!(
        defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get(),
        1024 * 1024
    );
    assert!(
        defaults::network::QUIC_DATAGRAM_RECEIVE_BUFFER_BYTES.get()
            >= defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get()
    );
    assert!(
        defaults::network::QUIC_DATAGRAM_SEND_BUFFER_BYTES.get()
            >= defaults::network::QUIC_DATAGRAM_MAX_PAYLOAD_BYTES.get()
    );
}

/// This test not only asserts that the minimal set of fields is enough;
/// it also gives an insight into every single default value
#[test]
#[allow(clippy::too_many_lines)]
fn minimal_config_snapshot() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");

    // Snapshot updated to include new Sumeragi fields and other defaults
    expect_file!["fixtures/minimal_config_snapshot.txt"].assert_debug_eq(&config);
}

#[test]
fn torii_receipt_signer_parses() {
    let config =
        load_config_from_fixtures("torii_receipt_signer.toml").expect("config should be valid");
    let signer = config
        .torii
        .receipt_signer
        .expect("receipt signer should be configured");
    let expected = PublicKey::from_str(
        "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB",
    )
    .expect("receipt public key");
    assert_eq!(signer.public_key(), &expected);
    assert_eq!(
        signer
            .public_key()
            .try_algorithm()
            .expect("fixture receipt public key must be well-formed"),
        Algorithm::Ed25519
    );
}

#[test]
fn torii_ram_lfe_parses() {
    let config = load_config_from_fixtures("torii_ram_lfe.toml").expect("config should be valid");
    let runtime = config
        .torii
        .ram_lfe
        .expect("RAM-LFE runtime should be configured");
    assert_eq!(runtime.programs.len(), 1);

    let program = &runtime.programs[0];
    let expected_program_id = "phone_retail".parse().expect("program id");
    assert_eq!(program.program_id, expected_program_id);
    assert_eq!(program.secret.as_bytes(), &[0x01, 0x02, 0x03, 0x04]);
    let debug = format!("{runtime:?}");
    assert!(debug.contains("REDACTED RAM-LFE secret"));
    assert!(!debug.contains("01020304"));
    assert!(!debug.contains("4e525430"));
    assert_eq!(
        program.receipt_ttl,
        Some(Duration::from_millis(30_000)),
        "receipt ttl should parse as milliseconds"
    );
}

#[test]
fn ivm_banner_defaults_enabled() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");

    assert!(config.ivm.banner.show, "banner should default to on");
    assert!(config.ivm.banner.beep, "beep should default to on");
}

#[test]
fn torii_max_content_len_defaults_to_sixty_four_megabytes() {
    let config = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("config should be valid");

    assert_eq!(
        config.torii.max_content_len.0,
        defaults::torii::MAX_CONTENT_LEN.0,
        "minimal configs should inherit the runtime Torii body-cap default"
    );
}

#[test]
fn ivm_banner_override_applies() {
    let config =
        load_config_from_fixtures("ivm_banner_override.toml").expect("config should be valid");

    assert!(
        !config.ivm.banner.show,
        "override should disable banner rendering"
    );
    assert!(
        !config.ivm.banner.beep,
        "override should disable beep rendering"
    );
}

#[test]
fn nexus_lane_requires_alias() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("   ".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_zero_axt_slot_length() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 0,
            max_clock_skew_ms:
                iroha_config::parameters::defaults::nexus::axt::CLOCK_SKEW_MS_DEFAULT,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots:
                iroha_config::parameters::defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_relay_worker_requires_lane_relay_burn() {
    use iroha_config::parameters::user::{Nexus, NexusRelayWorker};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        relay_worker: NexusRelayWorker {
            enabled: true,
            ..NexusRelayWorker::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let error = format!("{:?}", emitter.into_result().expect_err("invalid config"));
    assert!(error.contains("nexus.relay_worker.enabled"));
}

#[test]
fn nexus_relay_worker_parses_with_lane_relay_burn() {
    use iroha_config::parameters::actual::NexusFeeSettlementMode;
    use iroha_config::parameters::user::{Nexus, NexusFees, NexusRelayWorker};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        fees: NexusFees {
            settlement_mode: "lane_relay_burn".to_owned(),
            ..NexusFees::default()
        },
        relay_worker: NexusRelayWorker {
            enabled: true,
            max_retry_attempts: 3,
            ..NexusRelayWorker::default()
        },
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter).expect("valid config");
    emitter.into_result().expect("no parse errors");
    assert!(parsed.relay_worker.enabled);
    assert_eq!(parsed.relay_worker.max_retry_attempts.get(), 3);
    assert_eq!(
        parsed.fees.settlement_mode,
        NexusFeeSettlementMode::LaneRelayBurn
    );
    assert_eq!(
        parsed.fees.sponsor_vault_custody_account_id,
        defaults::nexus::fees::sponsor_vault_custody_account_id()
    );
}

#[test]
fn nexus_rejects_out_of_range_axt_slot_length() {
    let result = load_config_from_fixtures("bad.nexus_axt_slot_length_too_large.toml");
    assert!(
        result.is_err(),
        "slot length above guardrail must be rejected"
    );
}

#[test]
fn nexus_rejects_negative_axt_slot_length() {
    let result = load_config_from_fixtures("bad.nexus_axt_slot_length_negative.toml");
    assert!(result.is_err(), "negative slot length must be rejected");
}

#[test]
fn nexus_rejects_axt_clock_skew_above_slot_length() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 1_000,
            max_clock_skew_ms: 2_000,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots:
                iroha_config::parameters::defaults::nexus::axt::REPLAY_RETENTION_SLOTS,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_zero_axt_replay_retention_slots() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, NexusAxt};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        axt: NexusAxt {
            slot_length_ms: 1_000,
            max_clock_skew_ms: 0,
            proof_cache_ttl_slots:
                iroha_config::parameters::defaults::nexus::axt::PROOF_CACHE_TTL_SLOTS,
            replay_retention_slots: 0,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_rejects_out_of_range_axt_replay_retention_slots() {
    let result = load_config_from_fixtures("bad.nexus_axt_replay_retention_too_large.toml");
    assert!(
        result.is_err(),
        "replay retention above guardrail must be rejected"
    );
}

#[test]
fn nexus_axt_fields_load_from_fixture() {
    let config = load_config_from_fixtures("nexus_axt_full.toml").expect("config should be valid");
    assert_eq!(config.nexus.axt.slot_length_ms.get(), 1_000);
    assert_eq!(config.nexus.axt.max_clock_skew_ms, 250);
    assert_eq!(config.nexus.axt.proof_cache_ttl_slots.get(), 8);
    assert_eq!(config.nexus.axt.replay_retention_slots.get(), 256);
}

#[test]
fn nexus_multilane_requires_enable_flag() {
    let result = load_config_from_fixtures("bad.nexus_multilane_disabled.toml");
    let err = result.expect_err("multi-lane catalogs must require nexus.enabled");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("nexus.enabled"),
        "error should point at nexus.enabled being required (got {debug})"
    );
    assert!(
        debug.contains("multi-lane"),
        "error should mention multi-lane catalogs (got {debug})"
    );
}

#[test]
fn nexus_lane_overrides_rejected_when_disabled() {
    let result = load_config_from_fixtures("bad.nexus_lane_overrides_disabled.toml");
    let err = result.expect_err("lane overrides must be rejected when nexus is disabled");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("nexus.enabled"),
        "error should point at nexus.enabled being required (got {debug})"
    );
    assert!(
        debug.contains("single-lane"),
        "error should explain that overrides are ignored in single-lane mode (got {debug})"
    );
}

#[test]
fn sumeragi_v2_rejects_unknown_v1_actor_and_global_rbc_fields() {
    let report = load_config_from_fixtures("bad.sumeragi_legacy_v1_fields.toml")
        .expect_err("retired v1 actor/global-RBC schema must be rejected");
    let message = format!("{report:?}");
    assert!(
        message.contains("collectors")
            || message.contains("advanced")
            || message.contains("recovery"),
        "diagnostic should identify a retired v1 table: {message}",
    );
}

#[test]
fn retired_plan_journal_toggle_fails_during_config_parse_before_runtime_storage() {
    let report = load_config_from_fixtures("bad.retired_plan_journal_toggle.toml")
        .expect_err("the first release must not expose a journal-disabled runtime path");
    let message = strip_ansi_codes(&format!("{report:?}"));
    assert_contains!(message, "unknown parameter: `queue.plan_journal_enabled`");
}

#[test]
fn nexus_lane_relay_emergency_requires_nexus_enabled() {
    use iroha_config::parameters::user::{LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: false,
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            ..LaneRelayEmergency::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("lane relay emergency should require nexus.enabled");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.enabled requires nexus.enabled = true"
    );
}

#[test]
fn nexus_lane_relay_emergency_rejects_zero_threshold() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            multisig_threshold: 0,
            multisig_members: 5,
            max_ttl_blocks: 20,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("zero threshold must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.multisig_threshold must be > 0"
    );
}

#[test]
fn nexus_lane_relay_emergency_rejects_threshold_above_members() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, LaneRelayEmergency, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        lane_relay_emergency: LaneRelayEmergency {
            enabled: true,
            multisig_threshold: 6,
            multisig_members: 5,
            max_ttl_blocks: 20,
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("threshold above members must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(
        debug,
        "nexus.lane_relay_emergency.multisig_threshold 6 must be <= multisig_members 5"
    );
}

#[test]
fn nexus_storage_weights_require_full_budget() {
    use iroha_config::parameters::user::{Nexus, NexusStorage, NexusStorageWeights};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        storage: NexusStorage {
            disk_budget_weights: NexusStorageWeights {
                kura_blocks_bps: 9_000,
                wsv_snapshots_bps: 0,
                sorafs_bps: 0,
                soranet_spool_bps: 0,
                soravpn_spool_bps: 0,
            },
            ..NexusStorage::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("invalid storage weights must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "nexus.storage.disk_budget_weights");
}

#[test]
fn nexus_storage_weights_require_positive_subsystem_shares() {
    use iroha_config::parameters::user::{Nexus, NexusStorage, NexusStorageWeights};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        storage: NexusStorage {
            disk_budget_weights: NexusStorageWeights {
                kura_blocks_bps: 4_000,
                wsv_snapshots_bps: 2_500,
                sorafs_bps: 1_500,
                soranet_spool_bps: 2_000,
                soravpn_spool_bps: 0,
            },
            ..NexusStorage::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter
        .into_result()
        .expect_err("a zero subsystem storage share must be rejected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "nexus.storage.disk_budget_weights");
    assert_contains!(debug, "soravpn_spool_bps");
    assert_contains!(debug, "greater than zero");
}

#[test]
fn nexus_profile_template_enables_multilane_defaults() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("defaults/nexus/config.toml");

    let source = fs::read_to_string(&config_path).expect("read Nexus signing profile");
    let mut table: toml::Table = toml::from_str(&source).expect("parse Nexus signing profile");
    let expected_hash = table
        .get_mut("genesis")
        .and_then(TomlValue::as_table_mut)
        .and_then(|genesis| genesis.get_mut("expected_hash"))
        .expect("Nexus signing profile expected-hash placeholder");
    assert_eq!(
        expected_hash.as_str(),
        Some("REPLACE_WITH_GENESIS_EXPECTED_HASH")
    );
    // Substitute only inside this inspection test; the checked-in profile must fail runtime
    // normalization until an operator provisions the signed genesis hash.
    *expected_hash = TomlValue::String(
        Hash::new(b"iroha-config non-runtime Nexus profile inspection").to_string(),
    );

    let config = ConfigReader::new()
        .with_toml_source(iroha_config_base::toml::TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .change_context(FixtureConfigLoadError)
        .and_then(|user| user.parse().change_context(FixtureConfigLoadError))
        .expect("Nexus profile config should parse");

    assert!(
        config.nexus.enabled,
        "Nexus profile must set nexus.enabled = true"
    );
    assert_eq!(config.nexus.lane_catalog.lane_count().get(), 3);
    assert_eq!(
        config.nexus.dataspace_catalog.entries().len(),
        3,
        "profile should ship dataspace catalog entries for each lane"
    );
    let lane_aliases: Vec<_> = config
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .map(|lane| lane.alias.as_str())
        .collect();
    assert_eq!(lane_aliases, ["core", "governance", "zk"]);
    let dataspace_aliases: Vec<_> = config
        .nexus
        .dataspace_catalog
        .entries()
        .iter()
        .map(|entry| entry.alias.as_str())
        .collect();
    assert_eq!(dataspace_aliases, ["universal", "governance", "zk"]);
    assert_eq!(config.nexus.routing_policy.rules.len(), 2);
    assert!(
        !config.nexus.lane_relay_emergency.enabled,
        "Nexus profile must leave lane relay emergency overrides disabled by default"
    );
    assert_eq!(
        config.nexus.lane_relay_emergency.multisig_threshold.get(),
        3
    );
    assert_eq!(config.nexus.lane_relay_emergency.multisig_members.get(), 5);
}

#[test]
fn lane_profile_home_applies_throttles() {
    let config = load_config_from_fixtures("home_lane_profile.toml")
        .expect("config should be valid with lane profile override");
    let expected_limits = LaneProfile::Home.derived_limits();
    let network = &config.network;

    assert_eq!(network.lane_profile, LaneProfile::Home);
    assert_eq!(network.max_incoming, expected_limits.max_incoming);
    assert_eq!(
        network.max_total_connections,
        expected_limits.max_total_connections
    );
    assert_eq!(
        network.low_priority_bytes_per_sec,
        expected_limits.low_priority_bytes_per_sec
    );
    assert_eq!(
        network.low_priority_rate_per_sec,
        expected_limits.low_priority_rate_per_sec
    );
}

#[test]
fn streaming_soranet_overrides_apply() {
    let config = load_config_from_fixtures("streaming_soranet_override.toml")
        .expect("config should load with soranet overrides");
    let soranet = &config.streaming.soranet;

    assert!(
        soranet.enabled,
        "override should keep SoraNet provisioning enabled"
    );
    assert_eq!(
        soranet.exit_multiaddr, "/dns/test-exit/quic",
        "override exit multiaddr should propagate"
    );
    assert_eq!(
        soranet.padding_budget_ms,
        Some(42),
        "override padding budget should propagate"
    );
    assert_eq!(
        soranet.access_kind,
        StreamingSoranetAccessKind::ReadOnly,
        "access policy override should convert into runtime enum"
    );
    assert_eq!(
        soranet.channel_salt, "custom.seed.v1",
        "channel salt override should propagate as provided string"
    );
    assert_eq!(
        soranet.provision_window_segments,
        defaults::streaming::soranet::PROVISION_WINDOW_SEGMENTS,
        "provision window should default when not overridden"
    );
    assert_eq!(
        soranet.provision_queue_capacity,
        defaults::streaming::soranet::PROVISION_QUEUE_CAPACITY,
        "provision queue capacity should default when not overridden"
    );
}

#[test]
fn streaming_bundled_requires_build_flag() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("streaming_bundled.toml");
    result.expect("streaming_bundled config should load on bundled builds");
}

#[test]
fn streaming_bundle_width_above_tables_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width.toml");
    let err = result.expect_err("bundle width above available bundled tables must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("1..=3"),
        "error should report the available bundled width from the tables (got {debug})"
    );
}

#[test]
fn streaming_bundle_width_below_minimum_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width_small.toml");
    let err = result.expect_err("bundle width below minimum must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("at least 2"),
        "error should report the minimum bundled width requirement (got {debug})"
    );
}

#[test]
fn streaming_bundle_width_zero_rejected() {
    assert!(
        norito::streaming::BUNDLED_RANS_BUILD_AVAILABLE,
        "Bundled rANS must be compiled in for the first release; rebuild with ENABLE_RANS_BUNDLES=1"
    );
    let result = load_config_from_fixtures("bad.streaming_bundle_width_zero.toml");
    let err = result.expect_err("zero bundle width must be rejected");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("bundle_width"),
        "error should surface the bundle_width guard (got {debug})"
    );
    assert!(
        debug.contains("1..=3"),
        "error should report the available bundled width from the tables (got {debug})"
    );
}

#[test]
fn streaming_invalid_kyber_suite_rejected() {
    let result = load_config_from_fixtures("bad.streaming_kyber_suite.toml");
    assert!(
        result.is_err(),
        "invalid streaming.kyber_suite must be rejected"
    );
}

#[test]
fn soranet_handshake_kem_suite_override() {
    let config = load_config_from_fixtures("soranet_handshake_kem_suite_override.toml")
        .expect("config should load with handshake override");
    assert_eq!(
        config.network.soranet_handshake.kem_id,
        MlKemSuite::MlKem512.kem_id(),
        "override should downshift the KEM suite id"
    );
}

#[test]
fn soranet_handshake_invalid_kem_suite_rejected() {
    let result = load_config_from_fixtures("bad.soranet_handshake_kem_suite.toml");
    assert!(
        result.is_err(),
        "invalid network.soranet_handshake.kem_suite must be rejected"
    );
}

#[test]
fn soranet_handshake_zero_difficulty_rejected() {
    let result = load_config_from_fixtures("bad.soranet_handshake_zero_difficulty.toml");
    assert!(
        result.is_err(),
        "zero-difficulty SoraNet admission must be rejected"
    );
}

#[test]
fn routing_policy_dataspace_resolution() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{
        DataSpaceDescriptor, LaneDescriptor, Nexus, RouteMatcher, RoutingPolicy, RoutingRule,
    };
    use iroha_config_base::util::Emitter;
    use iroha_data_model::nexus::DataSpaceId;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(2).expect("nonzero"),
        lane_catalog: vec![
            LaneDescriptor {
                index: Some(0),
                alias: Some("primary".into()),
                dataspace: Some("universal".into()),
                description: None,
                ..LaneDescriptor::default()
            },
            LaneDescriptor {
                index: Some(1),
                alias: Some("alpha".into()),
                dataspace: Some("alpha".into()),
                description: None,
                ..LaneDescriptor::default()
            },
        ],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0100000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: None,
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(1),
            default_dataspace: Some("alpha".into()),
            rules: vec![RoutingRule {
                lane: Some(0),
                dataspace: Some("universal".into()),
                matcher: RouteMatcher::default(),
            }],
        },
        ..Nexus::default()
    };

    let parsed = nexus
        .parse(&mut emitter)
        .expect("routing policy should parse");
    assert!(emitter.into_result().is_ok());

    assert_eq!(parsed.routing_policy.default_dataspace, DataSpaceId::new(1));
    assert_eq!(
        parsed.routing_policy.rules[0].dataspace,
        Some(DataSpaceId::UNIVERSAL)
    );
}

#[test]
fn routing_policy_lane_dataspace_mismatch_rejected() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{
        DataSpaceDescriptor, LaneDescriptor, Nexus, RoutingPolicy,
    };
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            dataspace: Some("universal".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0100000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: None,
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("alpha".into()),
            rules: Vec::new(),
        },
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter);
    assert!(
        parsed.is_none(),
        "mismatched default dataspace must be rejected"
    );
    let err = emitter
        .into_result()
        .expect_err("routing policy mismatch should surface parse errors");
    let debug = format!("{err:?}");
    assert!(
        debug.contains("routing default dataspace"),
        "error should mention mismatched default dataspace (got {debug})"
    );
}

#[test]
fn dataspace_fault_tolerance_zero_rejected() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{DataSpaceDescriptor, LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0100000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: Some(0),
            fee_sponsor_program_id: None,
        }],
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter);
    assert!(parsed.is_none(), "fault_tolerance=0 must be rejected");
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "fault_tolerance must be >= 1");
}

#[test]
fn dataspace_manifest_hash_required_for_non_universal() {
    use iroha_config::parameters::user::{DataSpaceDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: None,
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: None,
        }],
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "must specify `manifest_hash`");
}

#[test]
fn dataspace_explicit_id_must_match_manifest_hash() {
    use iroha_config::parameters::user::{DataSpaceDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0200000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: None,
        }],
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "does not match manifest_hash-derived id");
}

#[test]
fn dataspace_fee_sponsor_program_id_parses() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{
        DataSpaceDescriptor, LaneDescriptor, Nexus, RoutingPolicy,
    };
    use iroha_config_base::util::Emitter;
    use iroha_data_model::nexus::DataSpaceId;

    let mut emitter = Emitter::<ParseError>::new();
    let program_id = format!(
        "{}/default",
        defaults::nexus::fees::SPONSOR_VAULT_CUSTODY_ACCOUNT_ID
    );
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            dataspace: Some("alpha".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0100000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: Some(program_id.clone()),
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("alpha".into()),
            ..RoutingPolicy::default()
        },
        ..Nexus::default()
    };

    let parsed = nexus
        .parse(&mut emitter)
        .expect("dataspace fee sponsor should parse");
    assert!(emitter.into_result().is_ok());
    assert_eq!(
        parsed
            .dataspace_fee_sponsor_program_ids
            .get(&DataSpaceId::new(1))
            .map(ToString::to_string),
        Some(program_id)
    );
}

#[test]
fn dataspace_fee_sponsor_program_id_rejects_malformed_literal() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{
        DataSpaceDescriptor, LaneDescriptor, Nexus, RoutingPolicy,
    };
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: true,
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            dataspace: Some("alpha".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("alpha".into()),
            id: Some(1),
            manifest_hash: Some(
                "0100000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: Some("missing-program-separator".into()),
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("alpha".into()),
            ..RoutingPolicy::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "fee_sponsor_program_id");
}

#[test]
fn dataspace_fee_sponsor_program_id_requires_nexus_enabled() {
    use iroha_config::parameters::user::{DataSpaceDescriptor, Nexus};
    use iroha_config_base::util::Emitter;
    use iroha_data_model::nexus::DataSpaceId;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        enabled: false,
        dataspace_catalog: vec![DataSpaceDescriptor {
            alias: Some("universal".into()),
            id: Some(DataSpaceId::UNIVERSAL.as_u64()),
            manifest_hash: None,
            description: None,
            fault_tolerance: None,
            fee_sponsor_program_id: Some(format!(
                "{}/default",
                defaults::nexus::fees::SPONSOR_VAULT_CUSTODY_ACCOUNT_ID
            )),
        }],
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    let err = emitter.into_result().expect_err("parse error expected");
    let debug = strip_ansi_codes(&format!("{err:?}"));
    assert_contains!(debug, "nexus.enabled");
}

#[test]
fn routing_policy_unknown_dataspace_rejected() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{LaneDescriptor, Nexus, RoutingPolicy};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        routing_policy: RoutingPolicy {
            default_lane: Some(0),
            default_dataspace: Some("unknown".into()),
            ..RoutingPolicy::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn lane_registry_rejects_zero_poll_interval() {
    use std::{num::NonZeroU32, time::Duration};

    use iroha_config::parameters::user::{LaneDescriptor, LaneRegistryConfig, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("core".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        registry: LaneRegistryConfig {
            poll_interval_ms: Duration::from_secs(0).into(),
            ..LaneRegistryConfig::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn governance_default_module_must_exist() {
    use std::num::NonZeroU32;

    use iroha_config::parameters::user::{GovernanceCatalogConfig, LaneDescriptor, Nexus};
    use iroha_config_base::util::Emitter;

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        governance: GovernanceCatalogConfig {
            default_module: Some("missing".into()),
            ..GovernanceCatalogConfig::default()
        },
        ..Nexus::default()
    };

    assert!(nexus.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn governance_catalog_trims_and_parses_modules() {
    use std::{collections::BTreeMap, num::NonZeroU32};

    use iroha_config::parameters::user::{
        GovernanceCatalogConfig, GovernanceModule, LaneDescriptor, Nexus,
    };
    use iroha_config_base::util::Emitter;

    let mut modules = BTreeMap::new();
    modules.insert(
        " parliament ".into(),
        GovernanceModule {
            module_type: Some(" council ".into()),
            params: {
                let mut params = BTreeMap::new();
                params.insert(" quorum ".into(), " 67 ".into());
                params
            },
        },
    );

    let mut emitter = Emitter::<ParseError>::new();
    let nexus = Nexus {
        lane_count: NonZeroU32::new(1).expect("nonzero"),
        lane_catalog: vec![LaneDescriptor {
            index: Some(0),
            alias: Some("primary".into()),
            description: None,
            ..LaneDescriptor::default()
        }],
        governance: GovernanceCatalogConfig {
            default_module: Some("parliament".into()),
            modules,
        },
        ..Nexus::default()
    };

    let parsed = nexus.parse(&mut emitter).expect("governance should parse");
    assert!(emitter.into_result().is_ok());
    let catalog = parsed.governance;
    assert_eq!(catalog.default_module.as_deref(), Some("parliament"));
    let module = catalog
        .modules
        .get("parliament")
        .expect("module should be trimmed");
    assert_eq!(module.module_type.as_deref(), Some("council"));
    assert_eq!(module.params.get("quorum"), Some(&"67".to_string()));
}

#[test]
fn config_with_genesis() {
    let _config =
        load_config_from_fixtures("minimal_alone_with_genesis.toml").expect("should be valid");
}

#[test]
fn parse_does_not_apply_chain_discriminant_runtime_setting() {
    let _runtime_guard = AddressRuntimeGuard::capture();

    iroha_data_model::account::address::set_chain_discriminant(0x02F1);

    let config = load_config_from_fixtures("minimal_chain_discriminant.toml")
        .expect("config with chain discriminant should parse");

    assert_eq!(*config.common.chain_discriminant.value(), 777);
    assert_eq!(
        config.gov.bond_escrow_account,
        defaults::governance::bond_escrow_account_id()
    );
    assert_eq!(
        config.gov.citizenship_escrow_account,
        defaults::governance::citizenship_escrow_account_id()
    );
    assert_eq!(
        config.gov.slash_receiver_account,
        defaults::governance::slash_receiver_account_id()
    );
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        0x02F1
    );
}

#[test]
fn self_is_presented_in_trusted_peers() {
    let config =
        load_config_from_fixtures("minimal_alone_with_genesis.toml").expect("valid config");

    assert!(
        config
            .common
            .trusted_peers
            .value()
            .clone()
            .into_non_empty_vec()
            .contains(config.common.peer.id())
    );
}

#[test]
fn missing_fields() {
    let error = load_config_from_fixtures("bad.missing_fields.toml")
        .expect_err("should fail without missing fields");

    let msg = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(msg, "missing parameter: `chain`");
    assert_contains!(msg, "missing parameter: `public_key`");
    assert_contains!(msg, "missing parameter: `network.address`");
}

#[test]
fn sorafs_penalty_and_telemetry_roundtrip() {
    let config = load_config_from_fixtures("sorafs_penalty_and_telemetry.toml")
        .expect("config should parse with SoraFS governance overrides");

    let penalty = config.gov.sorafs_penalty;
    assert_eq!(penalty.utilisation_floor_bps, 7600);
    assert_eq!(penalty.uptime_floor_bps, 9650);
    assert_eq!(penalty.por_success_floor_bps, 9800);
    assert_eq!(penalty.strike_threshold, 4);
    assert_eq!(penalty.penalty_bond_bps, 1800);
    assert_eq!(penalty.cooldown_windows, 3);
    assert_eq!(penalty.max_pdp_failures, 1);
    assert_eq!(penalty.max_potr_breaches, 2);
    assert_eq!(penalty.cooldown_window_secs(1_800), 5_400);

    let telemetry = &config.gov.sorafs_telemetry;
    assert!(!telemetry.require_submitter);
    assert!(telemetry.require_nonce);
    assert!(telemetry.reject_zero_capacity);
    assert_eq!(telemetry.max_window_gap, Duration::from_secs(7_200));
    let expected: Vec<_> = defaults::governance::sorafs_telemetry::submitters()
        .iter()
        .map(|id| {
            AccountId::parse_encoded(id)
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("default submitter must parse")
        })
        .collect();
    assert_eq!(telemetry.submitters, expected);
}

#[test]
fn sorafs_penalty_unknown_field_rejected() {
    let error = load_config_from_fixtures("bad.sorafs_penalty_unknown.toml")
        .expect_err("unknown penalty field should be rejected");

    let msg = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        msg,
        "unknown parameter: `gov.sorafs_penalty.unexpected_penalty_knob`"
    );
}

#[test]
fn sorafs_telemetry_unknown_field_rejected() {
    let error = load_config_from_fixtures("bad.sorafs_telemetry_unknown.toml")
        .expect_err("unknown telemetry field should be rejected");

    let msg = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        msg,
        "unknown parameter: `gov.sorafs_telemetry.unknown_submitter_field`"
    );
}

#[test]
fn sorafs_site_binding_zero_entry_limit_is_rejected() {
    assert!(
        load_config_from_fixtures("bad.sorafs_site_bindings_zero_sites.toml").is_err(),
        "site binding entry limits must remain non-zero"
    );
}

/// Aims the purpose of checking that every single provided env variable is consumed and parsed
/// into a valid config.
#[test]
fn full_envs_set_is_consumed() {
    let env = test_env_from_file(fixtures_dir().join("full.env"));

    // Read, complete, and fully parse into the actual config to ensure all
    // env-backed fields (including nested sections) are queried and consumed.
    let config = ConfigReader::new()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("should be fine to read user view")
        .parse()
        .expect("should parse into actual config");
    assert_eq!(
        config.streaming.key_material.identity().algorithm(),
        iroha_crypto::Algorithm::Ed25519
    );

    // Ensure every provided variable was consumed by the reader.
    assert_eq!(env.unvisited(), HashSet::new());
    // NOTE: The config now includes many additional env-backed knobs with defaults
    // (e.g., `PIPELINE_*`, `NORITO_*`, `ZK_*`, etc.). The reader probes them even
    // if not present in the environment. That makes `env.unknown()` non-empty in
    // this test scenario. We intentionally no longer assert on `unknown()` here.
}

#[test]
fn config_from_file_and_env() {
    let env = test_env_from_file(fixtures_dir().join("minimal_file_and_env.env"));

    ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_file_and_env.toml"))
        .expect("files are fine")
        .read_and_complete::<UserConfig>()
        .expect("should be fine")
        .parse()
        .expect("should be fine, again");
}

#[test]
fn full_config_parses_fine() {
    let cfg = load_config_from_fixtures("full.toml").expect("should be fine");
    let sorafs = &cfg.torii.sorafs_discovery;
    println!("sorafs parsed {sorafs:?}");
    assert!(
        sorafs.discovery_enabled,
        "sorafs.discovery.discovery_enabled not parsed"
    );
    assert_eq!(
        sorafs.known_capabilities,
        vec![
            "torii_gateway".to_string(),
            "chunk_range_fetch".to_string(),
            "vendor_reserved".to_string()
        ]
    );
    assert_eq!(
        sorafs.replay_checkpoint_path,
        PathBuf::from("sorafs_discovery/test-provider-advert-replay.to")
    );
    assert_eq!(sorafs.replay_checkpoint_max_entries.get(), 4_096);
    let admission = sorafs
        .admission
        .as_ref()
        .expect("sorafs.discovery.admission.envelopes_dir missing");
    assert_eq!(
        admission.envelopes_dir,
        PathBuf::from("tests/fixtures/sorafs_admission")
    );
    assert_eq!(admission.trusted_council_keys.len(), 1);
    assert_eq!(admission.signature_threshold.get(), 1);

    let alias_policy = cfg.torii.sorafs_alias_cache;
    assert_eq!(alias_policy.positive_ttl.as_secs(), 600);
    assert_eq!(alias_policy.refresh_window.as_secs(), 120);
    let site_bindings = &cfg.torii.sorafs_gateway.site_bindings;
    assert_eq!(
        site_bindings.path,
        Some(PathBuf::from("site-bindings/test.json"))
    );
    assert_eq!(site_bindings.max_bytes.get(), 4_096);
    assert_eq!(site_bindings.max_sites.get(), 7);
    let storage = &cfg.torii.sorafs_storage;
    assert!(storage.enabled, "sorafs.storage.enabled not parsed");
    assert_eq!(storage.data_dir, PathBuf::from("./storage/sorafs"));
    assert_eq!(storage.max_capacity_bytes.0, 107_374_182_400);
    assert_eq!(storage.max_parallel_fetches, 64);
    assert_eq!(storage.max_pins, 20000);
    assert_eq!(storage.por_sample_interval_secs, 900);
    assert_eq!(storage.alias.as_deref(), Some("tenant.alpha"));
    assert_eq!(
        storage.adverts.stake_pointer.as_deref(),
        Some("stake.pool.default")
    );
    assert_eq!(storage.adverts.availability, "warm");
    assert_eq!(storage.adverts.max_latency_ms, 750);
    assert_eq!(
        storage.adverts.topics,
        vec![
            "sorafs.sf1.primary:global".to_string(),
            "sorafs.sf1.backup:eu".to_string()
        ]
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn taira_config_enables_untrusted_cid_hosting() {
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .join("configs/soranexus/taira/config.toml");

    let raw = fs::read_to_string(&config_path).expect("Taira config should exist");
    let doc: TomlValue = toml::from_str(&raw).expect("Taira config should be valid TOML");

    assert!(
        doc.get("settlement")
            .and_then(TomlValue::as_table)
            .and_then(|settlement| settlement.get("offline"))
            .is_none(),
        "Taira must not model universal offline-wallet support as backend configuration"
    );

    let dataspaces = doc
        .get("nexus")
        .and_then(TomlValue::as_table)
        .and_then(|nexus| nexus.get("dataspace_catalog"))
        .and_then(TomlValue::as_array)
        .expect("nexus.dataspace_catalog should be configured");
    let external_dataspace = dataspaces
        .iter()
        .find(|entry| {
            entry
                .get("alias")
                .and_then(TomlValue::as_str)
                .is_some_and(|alias| alias == "is")
        })
        .expect("Taira profile should include the external `is` dataspace");
    assert_eq!(
        external_dataspace.get("id").and_then(TomlValue::as_integer),
        Some(6_647_857_470_246_403_404),
        "external dataspace id should match its manifest hash"
    );
    let mobile_dataspace = dataspaces
        .iter()
        .find(|entry| {
            entry
                .get("alias")
                .and_then(TomlValue::as_str)
                .is_some_and(|alias| alias == "is2")
        })
        .expect("Taira profile should include the mobile `is2` dataspace");
    assert_eq!(
        mobile_dataspace.get("id").and_then(TomlValue::as_integer),
        Some(8_477_022_798_449_861_195),
        "mobile dataspace id should match its manifest hash"
    );

    let nexus = doc
        .get("nexus")
        .and_then(TomlValue::as_table)
        .expect("nexus should be configured");
    assert_eq!(
        nexus.get("lane_count").and_then(TomlValue::as_integer),
        Some(5),
        "Taira profile should reserve routes for both BOI dataspaces"
    );
    let lanes = nexus
        .get("lane_catalog")
        .and_then(TomlValue::as_array)
        .expect("nexus.lane_catalog should be configured");
    assert!(
        lanes.iter().any(|lane| {
            lane.get("alias")
                .and_then(TomlValue::as_str)
                .is_some_and(|alias| alias == "external-poc")
                && lane
                    .get("dataspace")
                    .and_then(TomlValue::as_str)
                    .is_some_and(|dataspace| dataspace == "is")
        }),
        "Taira profile should bind the external `is` dataspace to its routing container"
    );
    assert!(
        lanes.iter().any(|lane| {
            lane.get("alias")
                .and_then(TomlValue::as_str)
                .is_some_and(|alias| alias == "boi-mobile")
                && lane
                    .get("dataspace")
                    .and_then(TomlValue::as_str)
                    .is_some_and(|dataspace| dataspace == "is2")
        }),
        "Taira profile should bind the mobile dataspace to its dedicated route"
    );
    let routing_rules = nexus
        .get("routing_policy")
        .and_then(TomlValue::as_table)
        .and_then(|policy| policy.get("rules"))
        .and_then(TomlValue::as_array)
        .expect("nexus.routing_policy.rules should be configured");
    let has_is_instruction_route = |instruction: &str| {
        routing_rules.iter().any(|rule| {
            rule.get("lane").and_then(TomlValue::as_integer) == Some(3)
                && rule.get("dataspace").and_then(TomlValue::as_str) == Some("is")
                && rule
                    .get("matcher")
                    .and_then(TomlValue::as_table)
                    .and_then(|matcher| matcher.get("instruction"))
                    .and_then(TomlValue::as_str)
                    == Some(instruction)
        })
    };
    assert!(
        has_is_instruction_route("smartcontract::deploy"),
        "Taira profile should route smartcontract::deploy to the external `is` dataspace routing container"
    );
    for retired in ["shield", "zk::zk_transfer", "unshield"] {
        assert!(
            !has_is_instruction_route(retired),
            "Taira profile must not retain retired generic confidential route {retired}"
        );
    }

    let block = doc
        .get("sumeragi")
        .and_then(TomlValue::as_table)
        .and_then(|sumeragi| sumeragi.get("block"))
        .and_then(TomlValue::as_table)
        .expect("sumeragi.block should be configured");
    assert_eq!(
        block
            .get("max_transactions")
            .and_then(TomlValue::as_integer),
        Some(96),
        "Taira profile should cap total proposal size"
    );
    assert_eq!(
        block
            .get("max_ivm_transactions")
            .and_then(TomlValue::as_integer),
        None,
        "Sumeragi v2 profiles must not use the retired IVM transaction-count cap"
    );
    assert_eq!(
        block
            .get("max_payload_bytes")
            .and_then(TomlValue::as_integer),
        Some(21 * 1024 * 1024),
        "Taira profile should cap proposal payload bytes"
    );
    assert_eq!(
        block
            .get("proposal_queue_scan_multiplier")
            .and_then(TomlValue::as_integer),
        Some(4),
        "Taira profile should keep enough scan budget for cheap txs"
    );

    let queues = doc
        .get("sumeragi")
        .and_then(TomlValue::as_table)
        .and_then(|sumeragi| sumeragi.get("queues"))
        .and_then(TomlValue::as_table)
        .expect("sumeragi.queues should be configured");
    assert_eq!(
        queues
            .get("authenticated_non_validator_sources")
            .and_then(TomlValue::as_integer),
        Some(2),
        "Taira should reserve two independent authenticated non-validator ingress lanes"
    );
    assert_eq!(
        queues.get("body_bytes").and_then(TomlValue::as_integer),
        Some(301 * 1024 * 1024),
        "Taira aggregate canonical wire-byte budget should isolate its seven ingress source lanes"
    );
    assert_eq!(
        queues
            .get("body_source_bytes")
            .and_then(TomlValue::as_integer),
        Some(43 * 1024 * 1024),
        "Taira should retain one canonical outer-ingress wire-byte quota per source"
    );

    let untrusted = doc
        .get("sorafs")
        .and_then(TomlValue::as_table)
        .and_then(|sorafs| sorafs.get("gateway"))
        .and_then(TomlValue::as_table)
        .and_then(|gateway| gateway.get("untrusted_hosting"))
        .and_then(TomlValue::as_table)
        .expect("sorafs.gateway.untrusted_hosting should be configured");

    assert_eq!(
        untrusted.get("enabled").and_then(TomlValue::as_bool),
        Some(true),
        "Taira profile should enable CID-host routing"
    );
    assert_eq!(
        untrusted
            .get("path_gateway_redirect")
            .and_then(TomlValue::as_bool),
        Some(true)
    );
    assert_eq!(
        untrusted
            .get("redirect_html_only")
            .and_then(TomlValue::as_bool),
        Some(true)
    );

    let suffixes = untrusted
        .get("cid_host_suffixes")
        .and_then(TomlValue::as_table)
        .expect("CID host suffixes should be configured");
    assert_eq!(
        suffixes.get("live").and_then(TomlValue::as_str),
        Some("sorafs.sora.org")
    );
    assert_eq!(
        suffixes.get("taira").and_then(TomlValue::as_str),
        Some("sorafs.taira.sora.org")
    );

    let runtime = doc
        .get("soracloud_runtime")
        .and_then(TomlValue::as_table)
        .expect("soracloud_runtime should be configured");
    assert_eq!(
        runtime.get("production_mode").and_then(TomlValue::as_bool),
        Some(true),
        "Taira profile should run the Soracloud runtime in production posture"
    );
    assert_eq!(
        runtime
            .get("inrou")
            .and_then(TomlValue::as_table)
            .and_then(|inrou| inrou.get("enabled"))
            .and_then(TomlValue::as_bool),
        Some(true),
        "Soracloud production mode requires Inrou to be explicitly enabled"
    );
    assert_eq!(
        runtime
            .get("submission")
            .and_then(TomlValue::as_table)
            .and_then(|submission| submission.get("fee_payer"))
            .and_then(TomlValue::as_str),
        Some("sponsor"),
        "Taira Soracloud submissions should use the exact genesis sponsor program"
    );
}

#[test]
fn crypto_section_defaults_applied() {
    use iroha_crypto::Algorithm;

    let cfg = load_config_from_fixtures("minimal_with_trusted_peers.toml")
        .expect("minimal config should be valid");
    let crypto = &cfg.crypto;

    assert_eq!(
        crypto.enable_sm_openssl_preview,
        defaults::crypto::ENABLE_SM_OPENSSL_PREVIEW
    );
    assert_eq!(crypto.default_hash, defaults::crypto::DEFAULT_HASH);
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1]
    );
    assert_eq!(
        crypto.sm2_distid_default,
        defaults::crypto::SM2_DISTID_DEFAULT
    );
    assert_eq!(crypto.allowed_curve_ids, vec![1, 4]);
}

#[test]
fn crypto_section_respects_env_overrides() {
    use iroha_crypto::Algorithm;

    let (default_hash, allowed_signing_env) = if cfg!(feature = "sm") {
        ("sm3-256", "ed25519,secp256k1,sm2")
    } else {
        ("blake2b-256", "ed25519,secp256k1")
    };

    let mut env = MockEnv::new()
        .set("CRYPTO_DEFAULT_HASH", default_hash)
        .set("CRYPTO_ALLOWED_SIGNING", allowed_signing_env)
        .set("CRYPTO_SM2_DISTID_DEFAULT", "CN12345678901234")
        .set("CRYPTO_CURVES_ALLOWED_IDS", "1,4");

    env = env.set(
        "CRYPTO_SM_OPENSSL_PREVIEW",
        if cfg!(feature = "sm-ffi-openssl") {
            "true"
        } else {
            "false"
        },
    );

    let cfg = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("minimal_with_trusted_peers.toml"))
        .expect("base file should be valid")
        .read_and_complete::<UserConfig>()
        .expect("user view with env overrides")
        .parse()
        .expect("actual config with env overrides");
    let crypto = &cfg.crypto;

    assert_eq!(crypto.default_hash, default_hash);
    assert_eq!(crypto.sm2_distid_default, "CN12345678901234");
    assert_eq!(
        crypto.enable_sm_openssl_preview,
        cfg!(feature = "sm-ffi-openssl")
    );

    #[cfg(feature = "sm")]
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1, Algorithm::Sm2]
    );

    #[cfg(not(feature = "sm"))]
    assert_eq!(
        crypto.allowed_signing,
        vec![Algorithm::Ed25519, Algorithm::Secp256k1]
    );

    assert_eq!(crypto.allowed_curve_ids, vec![1, 4]);
}

#[test]
fn fraud_monitoring_config_overrides_and_defaults() {
    let cfg = load_config_from_fixtures("fraud_monitoring.toml")
        .expect("fraud monitoring config should parse");
    let fraud = &cfg.fraud_monitoring;

    assert!(fraud.enabled);

    let endpoints: Vec<&str> = fraud.service_endpoints.iter().map(Url::as_str).collect();
    assert_eq!(
        endpoints,
        vec![
            "https://fraud.local/assess",
            "https://fraud.secondary/verify"
        ],
    );

    assert_eq!(
        fraud.connect_timeout,
        defaults::fraud_monitoring::CONNECT_TIMEOUT,
    );
    assert_eq!(fraud.request_timeout, Duration::from_millis(1_800));
    assert_eq!(fraud.missing_assessment_grace, Duration::from_secs(5),);
    assert_eq!(fraud.required_minimum_band, Some(FraudRiskBand::Medium));
}

#[test]
fn sumeragi_v2_explicit_schema_parses() {
    use iroha_config::parameters::actual::NodeRole;

    let cfg = load_config_from_fixtures("sumeragi_v2.toml")
        .expect("first-release v2 configuration should parse");

    assert_eq!(
        cfg.network
            .max_total_connections
            .map(std::num::NonZeroUsize::get),
        Some(32)
    );
    assert_eq!(cfg.sumeragi.role, NodeRole::Observer);
    assert_eq!(cfg.sumeragi.block.max_transactions.get(), 333);
    assert_eq!(cfg.sumeragi.block.max_payload_bytes.get(), 8 * 1024 * 1024);
    assert_eq!(cfg.sumeragi.block.proposal_queue_scan_multiplier.get(), 3);
    assert_eq!(cfg.sumeragi.queues.commands.get(), 512);
    assert_eq!(
        cfg.sumeragi
            .queues
            .authenticated_non_validator_sources
            .get(),
        2
    );
    assert_eq!(cfg.sumeragi.queues.bodies.get(), 96);
    assert_eq!(cfg.sumeragi.queues.body_bytes.get(), 68 * 1024 * 1024);
    assert_eq!(
        cfg.sumeragi.queues.body_source_bytes.get(),
        17 * 1024 * 1024
    );
    assert_eq!(cfg.sumeragi.queues.chunks.get(), 768);
    assert_eq!(cfg.sumeragi.queues.ready_bodies.get(), 48);
    assert_eq!(cfg.sumeragi.keys.activation_lead_blocks, 2);
    assert_eq!(cfg.sumeragi.keys.overlap_grace_blocks, 12);
    assert_eq!(cfg.sumeragi.keys.expiry_grace_blocks, 3);
    assert!(cfg.sumeragi.keys.require_hsm);
    assert_eq!(cfg.sumeragi.keys.allowed_hsm_providers.len(), 2);
    let shared = cfg
        .sumeragi
        .v2_config(
            Duration::from_secs(1),
            iroha_data_model::block::consensus_v2::ConsensusMode::Npos,
        )
        .expect("node-local settings must satisfy the v2 runtime contract");
    assert_eq!(shared.block_cadence_ms, 1_000);
    assert_eq!(shared.limits.max_queue_scan, 999);
}

#[test]
fn sumeragi_v2_rejects_queue_and_key_policy_errors() {
    for (fixture, expected) in [
        (
            "bad.sumeragi_command_queue_too_small.toml",
            "sumeragi.queues.commands must be at least 8",
        ),
        (
            "bad.sumeragi_body_source_bytes_too_small.toml",
            "sumeragi.queues.body_source_bytes must isolate max-payload envelopes, 65536 bytes of fixed headroom per envelope, 33800 recommended payload-completion manifest bytes, 1048576 lane-progress bytes, 4194304 lane-completion bytes, 65536 certified-fence-escape bytes, and 65536 timeout-vote bytes (minimum 33850376, configured 16777216)",
        ),
        (
            "bad.sumeragi_body_queue_too_small.toml",
            "sumeragi.queues.bodies must reserve five positions for at least one validator, three per authenticated non-validator source, and two anonymous positions (minimum 13, configured 9)",
        ),
        (
            "bad.sumeragi_body_bytes_too_small.toml",
            "sumeragi.queues.body_bytes must reserve one validator, every configured authenticated non-validator source, and the anonymous source partition (minimum 138412032, configured 138412031)",
        ),
        (
            "bad.sumeragi_empty_hsm_provider.toml",
            "sumeragi.keys.allowed_hsm_providers must not contain empty names",
        ),
    ] {
        let report = load_config_from_fixtures(fixture)
            .expect_err("invalid first-release v2 configuration must fail closed");
        assert_contains!(format!("{report:?}"), expected);
    }
}

#[test]
fn sumeragi_v2_does_not_accept_retired_environment_toggles() {
    let baseline = ConfigReader::new()
        .with_env(MockEnv::new())
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<UserConfig>()
        .expect("read user config")
        .parse()
        .expect("parse actual config");

    let with_retired_env = ConfigReader::new()
        .with_env(
            MockEnv::new()
                .set("SUMERAGI_COLLECTORS_K", "99")
                .set("SUMERAGI_VNEXT_SUSPICION_TIMEOUT_MS", "1")
                .set("SUMERAGI_RBC_CHUNK_MAX_BYTES", "1"),
        )
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<UserConfig>()
        .expect("retired environment names are not schema inputs")
        .parse()
        .expect("retired environment names cannot alter v2 config");

    assert_eq!(
        baseline
            .sumeragi
            .v2_config(
                Duration::from_secs(1),
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            )
            .expect("baseline v2 config"),
        with_retired_env
            .sumeragi
            .v2_config(
                Duration::from_secs(1),
                iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
            )
            .expect("v2 config with irrelevant environment"),
    );
}
#[cfg(feature = "gost")]
#[test]
fn gost_config_rejects_tc26_consensus_keys() {
    let error = load_config_from_fixtures("gost_with_trusted_peers.toml")
        .expect_err("gost consensus keys should be rejected");
    let message = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(
        message,
        "public_key/private_key must be BLS-normal for consensus"
    );
}

#[test]
fn pipeline_workers_env_parses() {
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};
    use iroha_config_base::env::MockEnv;

    // Default: use minimal base file so required params are satisfied,
    // then ensure workers fall back to defaults (0 = auto)
    let cfg = ConfigReader::new()
        .with_env(MockEnv::new())
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user view")
        .parse();
    assert!(cfg.is_ok());

    // Override via env
    let env = MockEnv::new().set("PIPELINE_WORKERS", "7");
    let cfg2: Actual = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("read user config with env")
        .parse()
        .expect("parse actual config with env");
    assert_eq!(cfg2.pipeline.workers, 7);
}

#[test]
fn logger_level_env_accepts_lowercase() {
    use iroha_config::{
        logger::Level,
        parameters::{actual::Root as Actual, user::Root as User},
    };
    use iroha_config_base::env::MockEnv;

    let env = MockEnv::new().set("LOG_LEVEL", "info");
    let cfg: Actual = ConfigReader::new()
        .with_env(env)
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config with env")
        .parse()
        .expect("actual config with lowercase log level env");

    assert_eq!(cfg.logger.level, Level::INFO);
}

#[test]
fn tls_fallback_defaults_to_tls_only() {
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};

    let cfg: Actual = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config")
        .parse()
        .expect("actual config");

    assert!(!cfg.network.tls_enabled);
    assert!(
        !cfg.network.tls_fallback_to_plain,
        "plaintext fallback must stay opt-in when TLS-over-TCP is enabled"
    );
}

include!("fixtures/trusted_proxy_defaults_test.rs");

#[test]
fn torii_internal_api_trust_defaults_to_exact_loopback_hosts() {
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};

    let cfg: Actual = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config")
        .parse()
        .expect("actual config");

    assert_eq!(
        cfg.torii.internal_api_trusted_cidrs,
        ["127.0.0.1/32", "::1/128"],
    );
    assert!(cfg.torii.api_rate_limit_bypass_cidrs.is_empty());
}

#[test]
fn network_defaults_carry_maximal_sumeragi_v2_progress_frames() {
    const MAX_CERTIFIED_BODY_RESPONSE_BYTES: usize = 16_811_581;

    assert_eq!(
        defaults::network::MAX_FRAME_BYTES.get(),
        17 * 1024 * 1024 + defaults::network::DEFAULT_AEAD_FRAME_OVERHEAD_BYTES
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_CONSENSUS,
        defaults::network::MAX_PLAINTEXT_FRAME_BYTES
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
        defaults::network::MAX_PLAINTEXT_FRAME_BYTES
    );
    assert!(
        defaults::network::MAX_FRAME_BYTES.get() > MAX_CERTIFIED_BODY_RESPONSE_BYTES,
        "the encrypted frame cap must retain room for the P2P wrapper and AEAD overhead"
    );
    assert_eq!(
        defaults::network::MAX_FRAME_BYTES_CONTROL.get(),
        2 * 1024 * 1024,
        "consensus-safety proposals and timeout certificates use the control topic"
    );
}

include!("fixtures/sumeragi_v2_default_profile_test.rs");

// type alias used through fixtures for newer error-stack API
type Result<T, E> = core::result::Result<T, Report<E>>;
