//! Validate the public `SoraFS` provider-ingest finalized-archive hard cut.

use std::path::PathBuf;

use iroha_config::parameters::{actual::Root as ActualConfig, defaults, user::Root as UserConfig};
use iroha_config_base::{env::MockEnv, read::ConfigReader, toml::TomlSource};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::account::AccountId;

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .with_env(MockEnv::new())
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

fn completion_signer_public_key_hex() -> String {
    let key_pair =
        KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519).expect("test Ed25519 keypair");
    hex::encode(key_pair.public_key().to_bytes().1)
}

fn proof_signer_binding() -> String {
    let key_pair =
        KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519).expect("test Ed25519 keypair");
    let public_key_hex = hex::encode(key_pair.public_key().to_bytes().1);
    let authority = AccountId::new(key_pair.public_key().clone())
        .to_i105_for_discriminant(defaults::common::CHAIN_DISCRIMINANT)
        .expect("test authority must encode as I105");
    format!(
        r#"
[sorafs.storage.native_transaction_signers.proof_outcome]
handle = "hsm://sorafs/proof-outcome/provider-ingest-primary"
authority = "{authority}"
algorithm = "ed25519"
public_key_hex = "{public_key_hex}"
revision = 1
policy_digest_hex = "{}"
"#,
        "53".repeat(32)
    )
}

fn enabled_overlay(archive_policy: &str) -> String {
    let proof_signer = proof_signer_binding();
    format!(
        r#"
[sorafs.storage]
enabled = true
provider_id_hex = "{}"

{proof_signer}

[sorafs.storage.provider_ingest_runtime]
enabled = true
authenticated_source_fetch_handle = "network.sorafs.authenticated-source.primary"
authenticated_source_fetch_revision = 5
authenticated_source_fetch_policy_digest_hex = "{}"
completion_signer_resolver_handle = "hsm.sorafs.provider-completion.primary"
completion_signer_resolver_revision = 6
completion_signer_resolver_policy_digest_hex = "{}"
completion_signer_handle = "pkcs11.sorafs.provider-completion.key-primary"
completion_signer_adapter_revision = 3
completion_signer_policy_id_hex = "{}"
completion_signer_policy_revision = 1
completion_signer_policy_digest_hex = "{}"
completion_signer_algorithm = "ed25519"
completion_signer_public_key_hex = "{}"
checkpoint_store_handle = "sealed.sorafs.provider-ingest.primary"
checkpoint_store_revision = 7
checkpoint_store_policy_digest_hex = "{}"
max_page_rows = 2
max_pages_per_tick = 2

[sorafs.storage.provider_ingest_runtime.finalized_archive]
{archive_policy}
"#,
        "51".repeat(32),
        "b1".repeat(32),
        "b2".repeat(32),
        "a1".repeat(32),
        "a2".repeat(32),
        completion_signer_public_key_hex(),
        "a7".repeat(32),
    )
}

fn valid_archive_policy() -> &'static str {
    r#"
relative_root = "provider-ingest/finalized-v1"
max_record_bytes = 8388608
max_archive_entries = 1234
max_total_bytes = 33554432
max_providers_per_anchor = 4
max_orders_per_provider = 4
max_total_orders_per_anchor = 16
max_page_rows = 2
max_kura_tip_lag_blocks = 0
"#
}

#[test]
fn explicit_nested_archive_policy_projects_exact_relative_root_and_bounds() {
    let actual = parse_overlay(&enabled_overlay(valid_archive_policy()))
        .expect("valid explicit provider-ingest archive policy");
    let runtime = actual
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_ref()
        .expect("enabled provider-ingest runtime");
    let archive = &runtime.finalized_archive;

    assert_eq!(runtime.authenticated_source_fetch_revision, 5);
    assert_eq!(runtime.authenticated_source_fetch_policy_digest, [0xB1; 32]);
    assert_eq!(runtime.completion_signer_resolver_revision, 6);
    assert_eq!(runtime.completion_signer_resolver_policy_digest, [0xB2; 32]);
    assert_eq!(
        archive.relative_root,
        PathBuf::from("provider-ingest/finalized-v1")
    );
    assert_eq!(archive.max_record_bytes, 8 * 1024 * 1024);
    assert_eq!(archive.max_archive_entries, 1_234);
    assert_eq!(archive.max_total_bytes, 32 * 1024 * 1024);
    assert_eq!(archive.max_providers_per_anchor, 4);
    assert_eq!(archive.max_orders_per_provider, 4);
    assert_eq!(archive.max_total_orders_per_anchor, 16);
    assert_eq!(archive.max_page_rows, 2);
    assert_eq!(archive.max_kura_tip_lag_blocks, 0);
    assert!(
        archive.retention_authority.is_none(),
        "manual/no-retention must remain the default"
    );
}

#[test]
fn enabled_retention_projects_only_credential_free_exact_authority_binding() {
    let policy = format!(
        "{}\nretention_enabled = true\nretention_authority_handle = \"sealed://sorafs/provider-ingest/archive-retention-primary\"\nretention_authority_revision = 9\nretention_authority_policy_digest_hex = \"{}\"\n",
        valid_archive_policy(),
        "d9".repeat(32),
    );
    let actual =
        parse_overlay(&enabled_overlay(&policy)).expect("valid archive retention authority policy");
    let binding = actual
        .torii
        .sorafs_storage
        .provider_ingest_runtime
        .as_ref()
        .expect("enabled provider-ingest runtime")
        .finalized_archive
        .retention_authority
        .as_ref()
        .expect("enabled retention authority");
    assert_eq!(
        binding.handle,
        "sealed://sorafs/provider-ingest/archive-retention-primary"
    );
    assert_eq!(binding.revision, 9);
    assert_eq!(binding.policy_digest, [0xD9; 32]);
}

#[test]
fn retention_authority_is_all_or_nothing_and_rejects_test_marked_handles() {
    for (extra, expected) in [
        (
            "retention_authority_handle = \"sealed://sorafs/provider-ingest/archive-retention-primary\"",
            "require retention_enabled = true",
        ),
        (
            "retention_enabled = true",
            "retention_authority_handle is required",
        ),
        (
            "retention_enabled = true\nretention_authority_handle = \"sealed://sorafs/provider-ingest/test\"\nretention_authority_revision = 1\nretention_authority_policy_digest_hex = \"d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9\"",
            "credential-free production runtime handle",
        ),
        (
            "retention_enabled = true\nretention_authority_handle = \"sealed://sorafs/provider-ingest/archive-retention-primary\"\nretention_authority_revision = 0\nretention_authority_policy_digest_hex = \"d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9d9\"",
            "retention_authority_revision must be nonzero",
        ),
    ] {
        let policy = format!("{}\n{extra}\n", valid_archive_policy());
        let error = parse_overlay(&enabled_overlay(&policy))
            .expect_err("invalid retention authority policy must fail closed");
        assert!(error.contains(expected), "unexpected diagnostic: {error}");
    }
}

#[test]
fn archive_policy_rejects_unsafe_roots_and_inconsistent_bounds() {
    for (policy, expected) in [
        (
            valid_archive_policy()
                .replace("provider-ingest/finalized-v1", "../provider-ingest-escape"),
            "relative_root must be a non-empty normalized relative path",
        ),
        (
            valid_archive_policy().replace("max_record_bytes = 8388608", "max_record_bytes = 0"),
            "max_record_bytes must be within",
        ),
        (
            valid_archive_policy().replace("max_archive_entries = 1234", "max_archive_entries = 0"),
            "max_archive_entries must be within",
        ),
        (
            valid_archive_policy().replace("max_total_bytes = 33554432", "max_total_bytes = 1"),
            "max_total_bytes must cover one maximum record",
        ),
        (
            valid_archive_policy()
                .replace("max_orders_per_provider = 4", "max_orders_per_provider = 1"),
            "max_total_orders_per_anchor must fit the provider and per-provider order ceilings",
        ),
        (
            valid_archive_policy().replace("max_page_rows = 2", "max_page_rows = 5"),
            "max_page_rows must not exceed max_orders_per_provider",
        ),
        (
            valid_archive_policy().replace(
                "max_kura_tip_lag_blocks = 0",
                "max_kura_tip_lag_blocks = 10001",
            ),
            "max_kura_tip_lag_blocks must be within",
        ),
    ] {
        let error = parse_overlay(&enabled_overlay(&policy))
            .expect_err("unsafe finalized archive policy must fail closed");
        assert!(
            error.contains(expected),
            "unexpected finalized archive diagnostic: {error}"
        );
    }
}

#[test]
fn retired_snapshot_and_top_level_lag_selectors_are_rejected() {
    for selector in [
        "max_snapshot_rows = 256",
        "max_snapshot_bytes = 134217728",
        "max_finalized_lag_blocks = 2",
    ] {
        let source = format!(
            r"
[sorafs.storage.provider_ingest_runtime]
{selector}
"
        );
        let error = parse_overlay(&source).expect_err("retired selector must be rejected");
        assert!(
            error.contains(selector.split_once(' ').expect("selector name").0),
            "unknown-field diagnostic must identify the retired selector: {error}"
        );
    }
}
