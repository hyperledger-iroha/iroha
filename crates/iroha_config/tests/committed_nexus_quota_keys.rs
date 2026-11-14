//! A configuration file cannot set an uploaded-model or DA ingest quota.
//!
//! The plaintext and chunk ceilings of one uploaded-model bundle and the count
//! and byte ceilings of one account's DA ingests in a quota window are the
//! committed `nexus_quota_policy_v1` of a root that recognizes it and its
//! protocol default otherwise. The node schema has no `[nexus.uploaded_models]`
//! section and `[nexus.da]` keeps the length of the quota window only. The
//! signed V1 commitments hold the four quota slots at their first-release
//! values, which equal the protocol default, so every chain that never set
//! these keys keeps its signed bytes.
use iroha_config::parameters::{
    actual::{NexusConsensusQuotaSlotsV1, Root as ActualConfig},
    user::Root as UserConfig,
};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use iroha_data_model::nexus_quota_policy::NexusQuotaPolicyV1;
use std::path::PathBuf;

/// Every retired quota key, with a value that differs from the protocol default.
const QUOTA_KEYS: [(&str, &str, &str); 4] = [
    (
        "nexus.uploaded_models",
        "max_plaintext_bytes_per_model",
        "7",
    ),
    ("nexus.uploaded_models", "max_chunk_count_per_model", "7"),
    ("nexus.da", "ingest_quota_max_count_per_account", "7"),
    ("nexus.da", "ingest_quota_max_bytes_per_account", "7"),
];

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn inline_source(source: &str) -> TomlSource {
    let table: toml::Table = source.parse().expect("inline TOML should parse");
    TomlSource::inline(table)
}

#[test]
fn configuration_file_cannot_set_a_nexus_quota() {
    for (section, key, value) in QUOTA_KEYS {
        let error = base_reader()
            .with_toml_source(inline_source(&format!("[{section}]\n{key} = {value}\n")))
            .read_and_complete::<UserConfig>()
            .expect_err("a quota is committed policy, not node configuration");
        let diagnostic = format!("{error:?}");
        // `[nexus.uploaded_models]` is gone from the schema, so the reader names the section;
        // `[nexus.da]` keeps its window length, so the reader names the key.
        let named = if section == "nexus.uploaded_models" {
            format!("`{section}`")
        } else {
            format!("`{section}.{key}`")
        };
        assert!(
            diagnostic.contains("unknown parameter") && diagnostic.contains(&named),
            "{section}.{key}: {diagnostic}"
        );
    }
    // The window length of the same section is still node configuration.
    let config: ActualConfig = base_reader()
        .with_toml_source(inline_source(
            "[nexus.da]\ningest_quota_window_blocks = 3\n",
        ))
        .read_and_complete::<UserConfig>()
        .expect("the quota window is node configuration")
        .parse()
        .expect("config should parse");
    assert_eq!(config.nexus.da.ingest_quota_window_blocks.get(), 3);
}

#[test]
fn signed_quota_slots_are_the_protocol_default() {
    // Activating `nexus_quota_policy_v1` writes its protocol default, and a root
    // that does not recognize it applies the same values. Both equal the slots
    // every first-release genesis signed.
    let slots = NexusConsensusQuotaSlotsV1::FROZEN;
    let policy = NexusQuotaPolicyV1::protocol_default();
    assert_eq!(
        slots.max_plaintext_bytes_per_model,
        policy.max_uploaded_model_plaintext_bytes
    );
    assert_eq!(
        slots.max_chunk_count_per_model,
        policy.max_uploaded_model_chunks
    );
    assert_eq!(
        slots.ingest_quota_max_count_per_account,
        policy.max_da_ingests_per_account
    );
    assert_eq!(
        slots.ingest_quota_max_bytes_per_account,
        policy.max_da_ingest_bytes_per_account
    );
}
