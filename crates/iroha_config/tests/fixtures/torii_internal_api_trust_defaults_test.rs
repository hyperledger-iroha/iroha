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
