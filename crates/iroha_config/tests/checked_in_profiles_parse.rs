//! The checked-in peer configuration samples parse with the node configuration schema.
//!
//! Every sample under `defaults/kagami/` and the public Taira validator template must stay
//! admissible: a retired table or key in a sample is an unknown parameter and fails here.

use std::path::{Path, PathBuf};

use iroha_config::parameters::{actual::Root as ActualConfig, user::Root as UserConfig};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use toml::{Table, Value};

/// Node key pair of `tests/fixtures/base.toml`, reused for secret substitutions.
const NODE_PRIVATE_KEY: &str =
    "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F";
const TRANSPORT_PUBLIC_KEY: &str =
    "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B";
const TRANSPORT_PRIVATE_KEY: &str =
    "802620134C4527B3852AE2218A8F079B301C651EAD8C7567B96BD7A9BE8DB366E46B89";
const STREAMING_PUBLIC_KEY: &str =
    "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB";
const STREAMING_PRIVATE_KEY: &str =
    "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F";
const EXPECTED_HASH: &str =
    "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn read_table(relative: &str) -> Table {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .parse()
        .unwrap_or_else(|error| panic!("parse {} as TOML: {error}", path.display()))
}

fn sub_table<'a>(table: &'a mut Table, key: &str) -> &'a mut Table {
    table
        .get_mut(key)
        .and_then(Value::as_table_mut)
        .unwrap_or_else(|| panic!("sample must have a `{key}` table"))
}

/// Replace the runtime-only genesis hash file with an inline hash.
fn inline_expected_hash(table: &mut Table) {
    let genesis = sub_table(table, "genesis");
    genesis
        .remove("expected_hash_file")
        .expect("samples resolve the genesis hash from a runtime file");
    genesis.insert("expected_hash".into(), EXPECTED_HASH.into());
}

fn parse(table: Table, sample: &str) -> ActualConfig {
    ConfigReader::new()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .unwrap_or_else(|error| panic!("{sample} must match the configuration schema: {error:?}"))
        .parse()
        .unwrap_or_else(|error| panic!("{sample} must parse: {error:?}"))
}

#[test]
fn kagami_dev_profile_peer_configs_parse() {
    for index in 0..4 {
        let sample = format!("defaults/kagami/iroha3-dev/peer{index}.toml");
        let mut table = read_table(&sample);
        inline_expected_hash(&mut table);
        let config = parse(table, &sample);
        assert_eq!(
            config.sumeragi.role,
            iroha_config::parameters::actual::NodeRole::Validator,
            "{sample} is a validator"
        );
        assert!(
            config.sumeragi.local.is_empty(),
            "{sample} keeps §9.3 defaults"
        );
    }
}

#[test]
fn taira_validator_template_parses_once_runtime_secrets_are_bound() {
    const SAMPLE: &str = "configs/soranexus/taira/config.toml";
    let mut table = read_table(SAMPLE);
    // Bind the runtime-only secrets and placeholders the template leaves to deployment.
    table
        .remove("private_key_file")
        .expect("Taira reads the validator key from a runtime file");
    table.insert("private_key".into(), NODE_PRIVATE_KEY.into());
    table
        .remove("soranet_transport_private_key_file")
        .expect("Taira reads the transport key from a runtime file");
    table.insert(
        "soranet_transport_public_key".into(),
        TRANSPORT_PUBLIC_KEY.into(),
    );
    table.insert(
        "soranet_transport_private_key".into(),
        TRANSPORT_PRIVATE_KEY.into(),
    );
    let streaming = sub_table(&mut table, "streaming");
    streaming
        .remove("identity_private_key_file")
        .expect("Taira reads the streaming key from a runtime file");
    streaming.insert("identity_public_key".into(), STREAMING_PUBLIC_KEY.into());
    streaming.insert("identity_private_key".into(), STREAMING_PRIVATE_KEY.into());
    inline_expected_hash(&mut table);
    let config = parse(table, SAMPLE);
    assert_eq!(
        config.sumeragi.role,
        iroha_config::parameters::actual::NodeRole::Validator
    );
    assert!(config.sumeragi.local.is_empty());
}
