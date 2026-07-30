//! Validate the V1 hard cut to runtime-only stream-token signing.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::account::AccountId;

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
    .map(|(role, handle_role, seed)| {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test Ed25519 keypair");
        let public_key_hex = hex::encode(key_pair.public_key().to_bytes().1);
        let authority = AccountId::new(key_pair.public_key().clone())
            .to_i105_for_discriminant(defaults::common::CHAIN_DISCRIMINANT)
            .expect("test authority must encode as I105");
        let policy_digest_hex = hex::encode([seed; 32]);
        format!(
            r#"
[sorafs.storage.native_transaction_signers.{role}]
handle = "hsm://sorafs/{handle_role}/stream-token-primary"
authority = "{authority}"
algorithm = "ed25519"
public_key_hex = "{public_key_hex}"
revision = 1
policy_digest_hex = "{policy_digest_hex}"
"#
        )
    })
    .collect::<Vec<_>>()
    .join("")
}

fn enabled_overlay(handle: &str, public_key_hex: &str) -> String {
    let native_signers = native_signer_bindings();
    format!(
        r#"
[sorafs.storage]
enabled = true

[sorafs.storage.stream_tokens]
enabled = true
signer_handle = "{handle}"
signer_public_key_hex = "{public_key_hex}"
{native_signers}
"#
    )
}

#[test]
fn enabled_stream_tokens_parse_one_exact_non_secret_runtime_binding() {
    let public_key_hex = public_key_hex(0x42);
    let actual = parse_overlay(&enabled_overlay(
        "pkcs11:prod/stream-token/v4",
        &public_key_hex,
    ))
    .expect("valid runtime signer binding");
    let tokens = &actual.torii.sorafs_storage.stream_tokens;

    assert!(tokens.enabled);
    assert_eq!(
        tokens.signer_handle.as_deref(),
        Some("pkcs11:prod/stream-token/v4")
    );
    assert_eq!(
        tokens.signer_public_key,
        Some(
            hex::decode(public_key_hex)
                .expect("test public key hex")
                .try_into()
                .expect("32-byte test public key")
        )
    );
}

#[test]
fn stream_token_runtime_binding_rejects_incomplete_disabled_and_non_production_forms() {
    let valid_public_key = public_key_hex(0x43);
    for (label, source, expected) in [
        (
            "storage disabled",
            format!(
                r#"
[sorafs.storage.stream_tokens]
enabled = true
signer_handle = "pkcs11:prod/stream-token/v1"
signer_public_key_hex = "{valid_public_key}"
"#
            ),
            "requires storage.enabled",
        ),
        (
            "missing handle",
            format!(
                r#"
[sorafs.storage]
enabled = true
[sorafs.storage.stream_tokens]
enabled = true
signer_public_key_hex = "{valid_public_key}"
"#
            ),
            "signer_handle is required",
        ),
        (
            "missing public key",
            r#"
[sorafs.storage]
enabled = true
[sorafs.storage.stream_tokens]
enabled = true
signer_handle = "pkcs11:prod/stream-token/v1"
"#
            .to_owned(),
            "signer_public_key_hex is required",
        ),
        (
            "dormant binding",
            format!(
                r#"
[sorafs.storage.stream_tokens]
enabled = false
signer_handle = "pkcs11:prod/stream-token/v1"
signer_public_key_hex = "{valid_public_key}"
"#
            ),
            "binding is forbidden while issuance is disabled",
        ),
        (
            "development handle",
            enabled_overlay("pkcs11:test/stream-token/v1", &valid_public_key),
            "must be a production runtime handle",
        ),
        (
            "whitespace handle",
            enabled_overlay("pkcs11:prod/stream token/v1", &valid_public_key),
            "must be a production runtime handle",
        ),
    ] {
        let error = parse_overlay(&source).expect_err(label);
        assert!(
            error.contains(expected),
            "{label} produced unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn stream_token_runtime_binding_rejects_noncanonical_or_invalid_ed25519_keys() {
    let valid_public_key = public_key_hex(0x44);
    for (label, public_key, expected) in [
        (
            "uppercase",
            valid_public_key.to_ascii_uppercase(),
            "canonical lowercase non-zero 32-byte hex",
        ),
        (
            "zero",
            "00".repeat(32),
            "canonical lowercase non-zero 32-byte hex",
        ),
        (
            "wrong length",
            "11".repeat(31),
            "canonical lowercase non-zero 32-byte hex",
        ),
        (
            "invalid Ed25519 point",
            "ff".repeat(32),
            "is not a valid Ed25519 public key",
        ),
        (
            "small-order Ed25519 point",
            format!("01{}", "00".repeat(31)),
            "is not a valid Ed25519 public key",
        ),
    ] {
        let error = parse_overlay(&enabled_overlay("pkcs11:prod/stream-token/v1", &public_key))
            .expect_err(label);
        assert!(
            error.contains(expected),
            "{label} produced unexpected diagnostic: {error}"
        );
        assert!(
            !error.contains(&public_key),
            "{label} diagnostic must not echo configured key material"
        );
    }
}

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
    let source = include_str!("../src/parameters/user.rs");
    let bindings = source
        .lines()
        .filter(|line| line.contains("env =") && line.contains("SORAFS_"))
        .collect::<Vec<_>>();

    assert!(
        bindings.is_empty(),
        "SoraFS V1 behavior must be configured through canonical TOML only; found environment bindings: {bindings:?}"
    );
}
