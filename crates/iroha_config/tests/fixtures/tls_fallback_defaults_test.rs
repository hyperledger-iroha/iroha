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
