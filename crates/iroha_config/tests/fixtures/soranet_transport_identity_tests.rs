const FIXTURE_SORANET_TRANSPORT_PUBLIC_KEY: &str =
    "ed0120D9F6AEF1813164294D1D9C0662FEB9C7F7861B4DFFE385680331093DA4ABD10B";
const FIXTURE_SORANET_TRANSPORT_PRIVATE_KEY: &str =
    "802620134C4527B3852AE2218A8F079B301C651EAD8C7567B96BD7A9BE8DB366E46B89";
const FIXTURE_STREAMING_PUBLIC_KEY: &str =
    "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB";
const FIXTURE_STREAMING_PRIVATE_KEY: &str =
    "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F";

fn fixture_soranet_transport_key_pair() -> KeyPair {
    let public_key = FIXTURE_SORANET_TRANSPORT_PUBLIC_KEY
        .parse::<PublicKey>()
        .expect("fixture SoraNet transport public key");
    let private_key = FIXTURE_SORANET_TRANSPORT_PRIVATE_KEY
        .parse::<PrivateKey>()
        .expect("fixture SoraNet transport private key");
    KeyPair::new(public_key, private_key).expect("matching fixture SoraNet transport key pair")
}

fn fixture_streaming_key_pair() -> KeyPair {
    KeyPair::new(
        FIXTURE_STREAMING_PUBLIC_KEY
            .parse::<PublicKey>()
            .expect("fixture streaming public key"),
        FIXTURE_STREAMING_PRIVATE_KEY
            .parse::<PrivateKey>()
            .expect("fixture streaming private key"),
    )
    .expect("matching fixture streaming key pair")
}

fn soranet_transport_layer(key_pair: &KeyPair) -> Table {
    Table::new()
        .write(
            "soranet_transport_public_key",
            key_pair.public_key().to_string(),
        )
        .write(
            "soranet_transport_private_key",
            ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        )
}

fn canonical_test_base_table() -> Table {
    let source = fs::read_to_string(fixtures_dir().join("base.toml"))
        .expect("read dedicated SoraNet transport test base config");
    let mut table: Table = toml::from_str(&source).expect("parse transport test base config");
    let hash_body = Hash::new(b"iroha-config dedicated SoraNet transport tests").to_string();
    let canonical = norito::literal::format("hash", &hash_body.to_ascii_uppercase());
    table
        .get_mut("genesis")
        .and_then(TomlValue::as_table_mut)
        .expect("base config genesis table")
        .insert("expected_hash".into(), TomlValue::String(canonical));
    table
}

#[test]
fn soranet_transport_identity_is_required_even_with_streaming_identity() {
    let error = ConfigReader::new()
        .with_env(MockEnv::new())
        .read_toml_with_extends(fixtures_dir().join("bad.missing_fields.toml"))
        .expect("empty fixture should be readable")
        .read_and_complete::<UserConfig>()
        .expect_err("dedicated SoraNet transport identity must be required");
    let message = strip_ansi_codes(&format!("{error:?}"));
    assert_contains!(message, "missing parameter: `soranet_transport_public_key`");
    assert_contains!(
        message,
        "missing parameter: `soranet_transport_private_key`"
    );
}

#[test]
fn soranet_transport_identity_env_pair_populates_actual_common() {
    let key_pair = fixture_soranet_transport_key_pair();
    let env = MockEnv::new()
        .set(
            "P2P_SORANET_TRANSPORT_PUBLIC_KEY",
            key_pair.public_key().to_string(),
        )
        .set(
            "P2P_SORANET_TRANSPORT_PRIVATE_KEY",
            ExposedPrivateKey(key_pair.private_key().clone()).to_string(),
        );
    let config = ConfigReader::new()
        .with_env(env.clone())
        .with_toml_source(TomlSource::inline(canonical_test_base_table()))
        .read_and_complete::<UserConfig>()
        .expect("transport env pair should complete user config")
        .parse()
        .expect("transport env pair should parse");

    assert_eq!(config.common.soranet_transport_key_pair, key_pair);
    assert_eq!(
        config.common.soranet_transport_key_pair.algorithm(),
        Algorithm::Ed25519
    );
    assert_ne!(
        config.common.soranet_transport_key_pair.public_key(),
        config.streaming.key_material.identity().public_key()
    );
    assert!(!env.unvisited().contains("P2P_SORANET_TRANSPORT_PUBLIC_KEY"));
    assert!(
        !env.unvisited()
            .contains("P2P_SORANET_TRANSPORT_PRIVATE_KEY")
    );
}

#[test]
fn soranet_transport_identity_rejects_mismatched_pair_without_disclosing_keys() {
    let mut layer = soranet_transport_layer(&fixture_soranet_transport_key_pair());
    layer.insert(
        "soranet_transport_private_key".into(),
        TomlValue::String(FIXTURE_STREAMING_PRIVATE_KEY.to_owned()),
    );
    let error = ConfigReader::new()
        .with_env(MockEnv::new())
        .with_toml_source(TomlSource::inline(canonical_test_base_table()))
        .with_toml_source(TomlSource::inline(layer))
        .read_and_complete::<UserConfig>()
        .expect("mismatched pair remains syntactically valid")
        .parse()
        .expect_err("mismatched SoraNet transport pair must fail");
    let message = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(message, "Invalid dedicated SoraNet transport identity");
    assert_contains!(message, "[REDACTED]");
    assert!(!message.contains(FIXTURE_SORANET_TRANSPORT_PUBLIC_KEY));
    assert!(!message.contains(FIXTURE_STREAMING_PRIVATE_KEY));
}

#[test]
fn soranet_transport_identity_rejects_non_ed25519_pair() {
    let bls = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal)
        .expect("derive non-Ed25519 transport test pair");
    let error = ConfigReader::new()
        .with_env(MockEnv::new())
        .with_toml_source(TomlSource::inline(canonical_test_base_table()))
        .with_toml_source(TomlSource::inline(soranet_transport_layer(&bls)))
        .read_and_complete::<UserConfig>()
        .expect("BLS pair remains syntactically valid")
        .parse()
        .expect_err("BLS SoraNet transport pair must fail");
    let message = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(message, "Invalid dedicated SoraNet transport identity");
    assert_contains!(
        message,
        "soranet_transport_public_key/private_key must be Ed25519"
    );
}

#[test]
fn soranet_transport_identity_rejects_streaming_public_key_reuse() {
    let error = ConfigReader::new()
        .with_env(MockEnv::new())
        .with_toml_source(TomlSource::inline(canonical_test_base_table()))
        .with_toml_source(TomlSource::inline(soranet_transport_layer(
            &fixture_streaming_key_pair(),
        )))
        .read_and_complete::<UserConfig>()
        .expect("reused pair remains syntactically valid")
        .parse()
        .expect_err("SoraNet transport key reuse must fail");
    let message = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(message, "Invalid dedicated SoraNet transport identity");
    assert_contains!(
        message,
        "soranet_transport_public_key must not reuse streaming.identity_public_key"
    );
}

#[test]
fn extra_fields() {
    let error = load_config_from_fixtures("bad.extra_fields.toml")
        .expect_err("should fail with extra field");

    let msg = strip_ansi_codes(&format!("{error:?}"));

    assert_contains!(msg, "Found unrecognised parameters");
    assert_contains!(msg, "unknown parameter: `bar`");
    assert_contains!(msg, "unknown parameter: `foo`");
}
