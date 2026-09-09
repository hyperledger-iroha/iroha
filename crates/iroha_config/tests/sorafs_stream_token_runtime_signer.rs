//! Validate the sole hardware stream-token configuration and independent public trust.
use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::account::AccountId;
use std::{fmt::Write as _, path::PathBuf};
fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}
fn parse_overlay(source: &str) -> Result<ActualConfig, String> {
    let table = source
        .parse()
        .map_err(|error| format!("inline TOML must parse: {error}"))?;
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .map_err(|error| format!("{error:?}"))?
        .parse()
        .map_err(|error| format!("{error:?}"))
}
fn public_key_hex(seed: u8) -> String {
    let key_pair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test Ed25519 keypair");
    hex::encode(key_pair.public_key().to_bytes().1)
}
fn native_signer_bindings() -> String {
    [
        ("proof_outcome", "proof-outcome", 0x52),
        ("repair", "repair", 0x53),
        ("reserve", "reserve", 0x54),
        ("orderbook", "orderbook", 0x55),
    ]
    .into_iter()
    .fold(String::new(), |mut bindings, (role, handle_role, seed)| {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test Ed25519 keypair");
        let public_key_hex = hex::encode(key_pair.public_key().to_bytes().1);
        let authority = AccountId::new(key_pair.public_key().clone())
            .to_i105_for_discriminant(defaults::common::CHAIN_DISCRIMINANT)
            .expect("test authority must encode as I105");
        let policy_digest_hex = hex::encode([seed; 32]);
        write!(
            bindings,
            r#"
[sorafs.storage.native_transaction_signers.{role}]
handle = "software://sorafs/{handle_role}/stream-token-primary"
authority = "{authority}"
algorithm = "ed25519"
public_key_hex = "{public_key_hex}"
revision = 1
policy_digest_hex = "{policy_digest_hex}""#
        )
        .expect("writing to a String cannot fail");
        bindings.push('\n');
        bindings
    })
}
const HARDWARE_PREFIX: &str = "sorafs.storage.stream_tokens.hardware";
const PROVIDER_HEX: &str = "abababababababababababababababababababababababababababababababab";
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}
fn hardware_fields() -> Vec<(&'static str, String)> {
    [
        (
            "runtime_handle",
            quoted("hsm://sorafs/stream-token/primary"),
        ),
        ("key_handle", quoted("pkcs11:production/stream-token/key-7")),
        ("service_id", quoted("stream-primary")),
        ("administrator_id", quoted("stream-security-primary")),
        ("public_key_hex", quoted(&public_key_hex(0x42))),
        ("key_revision", "7".to_owned()),
        ("policy_revision", "9".to_owned()),
        ("policy_digest_hex", quoted(&"b4".repeat(32))),
        ("attester.service_id", quoted("custody-authority-primary")),
        (
            "attester.administrator_id",
            quoted("custody-security-primary"),
        ),
        ("attester.public_key_hex", quoted(&public_key_hex(0x43))),
        ("attester.key_revision", "3".to_owned()),
        ("attester.policy_revision", "5".to_owned()),
        ("attester.policy_digest_hex", quoted(&"c6".repeat(32))),
        ("attester.active_from_unix_ms", "800000".to_owned()),
        ("attester.active_until_unix_ms", "2000000".to_owned()),
        ("attester.max_validity_ms", "1000000".to_owned()),
        ("attester.max_anchor_age_ms", "10000".to_owned()),
        (
            "observer.runtime_handle",
            quoted("finalized-source:prod/stream-token/primary"),
        ),
        ("observer.service_id", quoted("state-observer-primary")),
        (
            "observer.administrator_id",
            quoted("state-observer-security-primary"),
        ),
        ("observer.public_key_hex", quoted(&public_key_hex(0x44))),
        ("observer.key_revision", "11".to_owned()),
        ("observer.policy_revision", "13".to_owned()),
        ("observer.policy_digest_hex", quoted(&"d7".repeat(32))),
        ("observer.active_from_unix_ms", "900000".to_owned()),
        ("observer.active_until_unix_ms", "1900000".to_owned()),
        ("observer.max_state_age_ms", "300000".to_owned()),
    ]
    .into()
}
fn hardware_tables(fields: &[(&str, String)]) -> String {
    let mut source = String::new();
    for prefix in ["", "attester.", "observer."] {
        let suffix = prefix.strip_suffix('.').unwrap_or(prefix);
        write!(
            source,
            "\n[{HARDWARE_PREFIX}{}]\n",
            if suffix.is_empty() {
                String::new()
            } else {
                format!(".{suffix}")
            }
        )
        .expect("String write");
        for (field, value) in fields {
            if let Some(leaf) = field
                .strip_prefix(prefix)
                .filter(|leaf| !leaf.contains('.'))
            {
                writeln!(source, "{leaf} = {value}").expect("String write");
            }
        }
    }
    source
}
fn enabled_with_fields(fields: &[(&str, String)]) -> String {
    format!(
        r#"
[sorafs.storage]
enabled = true
provider_id_hex = "{PROVIDER_HEX}"
[sorafs.storage.stream_tokens]
enabled = true
admission_provider_handle = "sealed-cas:prod/stream-token/gateway-admission/v1"
admission_provider_revision = 7
admission_provider_policy_digest_hex = "{}"
{}{}
"#,
        "a5".repeat(32),
        hardware_tables(fields),
        native_signer_bindings()
    )
}
fn enabled_overlay() -> String {
    enabled_with_fields(&hardware_fields())
}
fn replaced_field(field: &str, value: String) -> String {
    let mut fields = hardware_fields();
    fields
        .iter_mut()
        .find(|(name, _)| *name == field)
        .expect("known field")
        .1 = value;
    enabled_with_fields(&fields)
}
fn rejects(source: &str, expected: &str) {
    let error = parse_overlay(source).expect_err("configuration must fail closed");
    assert!(error.contains(expected), "expected {expected}; got {error}");
}
#[test]
fn enabled_stream_tokens_parse_one_exact_non_secret_runtime_binding() {
    let actual = parse_overlay(&enabled_overlay()).expect("complete independent hardware binding");
    let storage = &actual.torii.sorafs_storage;
    assert_eq!(
        storage.provider_id.as_ref().expect("provider").as_bytes(),
        &[0xab; 32]
    );
    let tokens = &storage.stream_tokens;
    assert!(tokens.enabled);
    let hardware = tokens.hardware.as_ref().expect("complete hardware config");
    assert_eq!(hardware.runtime_handle, "hsm://sorafs/stream-token/primary");
    assert_eq!(hardware.key_handle, "pkcs11:production/stream-token/key-7");
    assert_eq!(hardware.service_id, "stream-primary");
    assert_eq!(hardware.administrator_id, "stream-security-primary");
    assert_eq!(hex::encode(hardware.public_key), public_key_hex(0x42));
    assert_eq!((hardware.key_revision, hardware.policy_revision), (7, 9));
    assert_eq!(hardware.policy_digest, [0xb4; 32]);
    let attester = &hardware.attester;
    assert_eq!(attester.authority.service_id, "custody-authority-primary");
    assert_eq!(
        attester.authority.administrator_id,
        "custody-security-primary"
    );
    assert_eq!(
        hex::encode(attester.authority.public_key),
        public_key_hex(0x43)
    );
    assert_eq!(
        (
            attester.authority.key_revision,
            attester.authority.policy_revision
        ),
        (3, 5)
    );
    assert_eq!(attester.authority.policy_digest, [0xc6; 32]);
    assert_eq!(
        (
            attester.authority.active_from_unix_ms,
            attester.authority.active_until_unix_ms
        ),
        (800000, 2000000)
    );
    assert_eq!(
        (attester.max_validity_ms, attester.max_anchor_age_ms),
        (1000000, 10000)
    );
    let observer = &hardware.observer;
    assert_eq!(
        observer.runtime_handle,
        "finalized-source:prod/stream-token/primary"
    );
    assert_eq!(observer.authority.service_id, "state-observer-primary");
    assert_eq!(
        observer.authority.administrator_id,
        "state-observer-security-primary"
    );
    assert_eq!(
        hex::encode(observer.authority.public_key),
        public_key_hex(0x44)
    );
    assert_eq!(
        (
            observer.authority.key_revision,
            observer.authority.policy_revision
        ),
        (11, 13)
    );
    assert_eq!(observer.authority.policy_digest, [0xd7; 32]);
    assert_eq!(
        (
            observer.authority.active_from_unix_ms,
            observer.authority.active_until_unix_ms
        ),
        (900000, 1900000)
    );
    assert_eq!(observer.max_state_age_ms, 300000);
    assert_eq!(
        tokens.admission_provider_handle.as_deref(),
        Some("sealed-cas:prod/stream-token/gateway-admission/v1")
    );
    assert_eq!(tokens.admission_provider_revision, Some(7));
    assert_eq!(tokens.admission_provider_policy_digest, Some([0xa5; 32]));
    assert_eq!(tokens.admission_max_pending, 65_536);
    assert_eq!(tokens.admission_max_tracked_tokens, 65_536);
    assert_eq!(tokens.admission_reconcile_max_items, 256);
    assert_eq!(tokens.admission_lease_ttl_ms, 120_000);
}
#[path = "sorafs_stream_token_runtime_signer/hardware_config_tests.rs"]
mod hardware_config_tests;
#[test]
fn legacy_stream_token_signing_key_path_is_rejected_without_disclosing_it() {
    let retired_path = "/run/secrets/retired-stream-token-seed";
    let table = format!(
        r#"
[sorafs.storage.stream_tokens]
signing_key_path = "{retired_path}"
"#
    )
    .parse()
    .expect("legacy inline TOML should parse");
    let error = base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect_err("legacy stream-token seed path must be rejected");
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("unknown parameter")
            && diagnostic.contains("sorafs.storage.stream_tokens.signing_key_path"),
        "unexpected legacy field diagnostic: {diagnostic}"
    );
    assert!(
        !diagnostic.contains(retired_path),
        "unknown-field diagnostics must not disclose the retired path"
    );
}
#[test]
fn retired_sorafs_environment_aliases_remain_unvisited() {
    const RETIRED_ALIASES: [(&str, &str); 25] = [
        ("SORAFS_TELEMETRY_REQUIRE_SUBMITTER", "true"),
        ("SORAFS_TELEMETRY_REQUIRE_NONCE", "true"),
        ("SORAFS_TELEMETRY_MAX_WINDOW_GAP_SECS", "1"),
        ("SORAFS_TELEMETRY_REJECT_ZERO_CAPACITY", "true"),
        ("SORAFS_TELEMETRY_SUBMITTERS", "retired"),
        ("SORAFS_TELEMETRY_PER_PROVIDER_SUBMITTERS", "retired"),
        ("SORAFS_STORAGE_ENABLED", "true"),
        ("SORAFS_STORAGE_DATA_DIR", "/run/retired"),
        ("SORAFS_STREAM_TOKENS_ENABLED", "true"),
        ("GOV_SORAFS_PIN_POLICY_MIN_REPLICAS_FLOOR", "2"),
        ("GOV_SORAFS_PIN_POLICY_MAX_REPLICAS_CEILING", "4"),
        ("GOV_SORAFS_PIN_POLICY_MAX_RETENTION_EPOCH", "100"),
        ("GOV_SORAFS_PIN_POLICY_ALLOWED_STORAGE_CLASSES", "hot"),
        ("GOV_SORAFS_PIN_POLICY_REQUIRE_COUNCIL_SIGNATURES", "true"),
        ("GOV_SORAFS_PENALTY_UTILISATION_FLOOR_BPS", "5000"),
        ("GOV_SORAFS_PENALTY_UPTIME_FLOOR_BPS", "9000"),
        ("GOV_SORAFS_PENALTY_POR_FLOOR_BPS", "9000"),
        ("GOV_SORAFS_PENALTY_STRIKE_THRESHOLD", "3"),
        ("GOV_SORAFS_PENALTY_BOND_BPS", "100"),
        ("GOV_SORAFS_PENALTY_COOLDOWN_WINDOWS", "2"),
        ("GOV_SORAFS_PENALTY_MAX_PDP_FAILURES", "3"),
        ("GOV_SORAFS_PENALTY_MAX_POTR_BREACHES", "3"),
        ("TORII_SORAFS_DISCOVERY_ENABLED", "true"),
        (
            "TORII_SORAFS_PUBLISH_GATEWAY_BASE_URL",
            "https://retired.example",
        ),
        ("TORII_SORAFS_ADMISSION_DIR", "/run/retired-admission"),
    ];
    let env = RETIRED_ALIASES
        .iter()
        .fold(MockEnv::new(), |env, (alias, value)| {
            env.set(*alias, *value)
        });
    let actual: ActualConfig = base_reader()
        .with_env(env.clone())
        .read_and_complete::<UserConfig>()
        .expect("retired environment aliases are not schema inputs")
        .parse()
        .expect("retired environment aliases cannot alter V1 configuration");
    assert!(!actual.torii.sorafs_storage.stream_tokens.enabled);
    let unvisited = env.unvisited();
    for (alias, _) in RETIRED_ALIASES {
        assert!(
            unvisited.contains(alias),
            "retired environment alias must remain unvisited: {alias}"
        );
    }
}
#[test]
fn sorafs_configuration_has_no_production_environment_bindings() {
    let source = [
        include_str!("../src/parameters/user.rs"),
        include_str!("../src/parameters/user/stream_token_hardware.rs"),
        include_str!("../src/parameters/user/stream_token_admission.rs"),
    ]
    .join("\n");
    let bindings = source
        .lines()
        .filter(|line| line.contains("env =") && line.contains("SORAFS_"))
        .collect::<Vec<_>>();
    assert!(
        bindings.is_empty(),
        "SoraFS V1 behavior must be configured through canonical TOML only; found environment bindings: {bindings:?}"
    );
}
