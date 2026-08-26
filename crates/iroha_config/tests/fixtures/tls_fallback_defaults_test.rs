#[test]
fn retired_p2p_transport_downgrade_knobs_are_unknown() {
    let mut network = Table::new();
    for key in [
        "tls_enabled",
        "tls_fallback_to_plain",
        "tls_inbound_only",
        "prefer_ws_fallback",
        "tls_only_v1_3",
    ] {
        network.insert(key.to_owned(), TomlValue::Boolean(false));
    }
    network.insert(
        "tls_listen_address".to_owned(),
        TomlValue::String("127.0.0.1:1337".to_owned()),
    );
    let mut layer = Table::new();
    layer.insert("network".to_owned(), TomlValue::Table(network));
    let error = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .with_toml_source(TomlSource::inline(layer))
        .read_and_complete::<UserConfig>()
        .expect_err("retired downgrade knobs must not remain compatibility aliases");
    let message = strip_ansi_codes(&format!("{error:?}"));
    for key in [
        "tls_enabled",
        "tls_fallback_to_plain",
        "tls_listen_address",
        "tls_inbound_only",
        "prefer_ws_fallback",
        "tls_only_v1_3",
    ] {
        let expected = format!("unknown parameter: `network.{key}`");
        assert_contains!(message, expected.as_str());
    }
}
