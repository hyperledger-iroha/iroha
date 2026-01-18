//! Validate trusted_peers_pop coverage and parsing rules.

use std::{path::PathBuf, str::FromStr};

use iroha_config::parameters::user::Root as UserConfig;
use iroha_config_base::read::ConfigReader;
use iroha_config_base::toml::TomlSource;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, bls_normal_pop_prove};

const BASE_PUBLIC_KEY: &str = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2";
const BASE_PRIVATE_KEY: &str =
    "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F";

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn base_keypair() -> KeyPair {
    let public_key = PublicKey::from_str(BASE_PUBLIC_KEY).expect("base public key");
    let private_key = PrivateKey::from_str(BASE_PRIVATE_KEY).expect("base private key");
    KeyPair::new(public_key, private_key).expect("base key pair")
}

fn build_user_config(inline_toml: &str) -> UserConfig {
    let table: toml::Table = inline_toml.parse().expect("inline toml");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("user config should read")
}

#[test]
fn trusted_peers_pop_accepts_complete_roster() {
    let base = base_keypair();
    let other = KeyPair::from_seed(b"trusted-peers-pop-ok".to_vec(), Algorithm::BlsNormal);
    let base_pop_hex = hex::encode(bls_normal_pop_prove(base.private_key()).expect("pop"));
    let other_pop_hex = hex::encode(bls_normal_pop_prove(other.private_key()).expect("pop"));
    let inline = format!(
        r#"
trusted_peers = [
  "{base_pk}@127.0.0.1:1338",
  "{other_pk}@127.0.0.1:1339",
]

[[trusted_peers_pop]]
public_key = "{base_pk}"
pop_hex = "{base_pop_hex}"

[[trusted_peers_pop]]
public_key = "{other_pk}"
pop_hex = "{other_pop_hex}"
"#,
        base_pk = base.public_key(),
        other_pk = other.public_key(),
        base_pop_hex = base_pop_hex,
        other_pop_hex = other_pop_hex,
    );
    let user_cfg = build_user_config(&inline);
    assert!(user_cfg.parse().is_ok());
}

#[test]
fn trusted_peers_pop_requires_full_roster() {
    let base = base_keypair();
    let other = KeyPair::from_seed(b"trusted-peers-pop-missing".to_vec(), Algorithm::BlsNormal);
    let base_pop_hex = hex::encode(bls_normal_pop_prove(base.private_key()).expect("pop"));
    let inline = format!(
        r#"
trusted_peers = [
  "{base_pk}@127.0.0.1:1338",
  "{other_pk}@127.0.0.1:1339",
]

[[trusted_peers_pop]]
public_key = "{base_pk}"
pop_hex = "{base_pop_hex}"
"#,
        base_pk = base.public_key(),
        other_pk = other.public_key(),
        base_pop_hex = base_pop_hex,
    );
    let user_cfg = build_user_config(&inline);
    assert!(user_cfg.parse().is_err());
}

#[test]
fn trusted_peers_pop_missing_rejects_config() {
    let base = base_keypair();
    let inline = format!(
        r#"
trusted_peers = [
  "{base_pk}@127.0.0.1:1338",
]
trusted_peers_pop = []
"#,
        base_pk = base.public_key(),
    );
    let user_cfg = build_user_config(&inline);
    assert!(user_cfg.parse().is_err());
}

#[test]
fn trusted_peers_pop_rejects_invalid_hex() {
    let base = base_keypair();
    let inline = format!(
        r#"
trusted_peers = [
  "{base_pk}@127.0.0.1:1338",
]

[[trusted_peers_pop]]
public_key = "{base_pk}"
pop_hex = "not-hex"
"#,
        base_pk = base.public_key(),
    );
    let user_cfg = build_user_config(&inline);
    assert!(user_cfg.parse().is_err());
}

#[test]
fn trusted_peers_pop_rejects_extraneous_keys() {
    let base = base_keypair();
    let extra = KeyPair::from_seed(b"trusted-peers-pop-extra".to_vec(), Algorithm::BlsNormal);
    let base_pop_hex = hex::encode(bls_normal_pop_prove(base.private_key()).expect("pop"));
    let extra_pop_hex = hex::encode(bls_normal_pop_prove(extra.private_key()).expect("pop"));
    let inline = format!(
        r#"
trusted_peers = [
  "{base_pk}@127.0.0.1:1338",
]

[[trusted_peers_pop]]
public_key = "{base_pk}"
pop_hex = "{base_pop_hex}"

[[trusted_peers_pop]]
public_key = "{extra_pk}"
pop_hex = "{extra_pop_hex}"
"#,
        base_pk = base.public_key(),
        base_pop_hex = base_pop_hex,
        extra_pk = extra.public_key(),
        extra_pop_hex = extra_pop_hex,
    );
    let user_cfg = build_user_config(&inline);
    assert!(user_cfg.parse().is_err());
}
