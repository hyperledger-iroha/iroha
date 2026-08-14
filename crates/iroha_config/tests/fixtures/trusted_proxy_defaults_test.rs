// Default-profile coverage for Torii's trusted-proxy transport policy.
#[test]
fn torii_transport_trusted_proxy_cidrs_default_to_empty() {
    use iroha_config::parameters::{actual::Root as Actual, user::Root as User};
    use iroha_config_base::read::ConfigReader;
    let cfg: Actual = ConfigReader::new()
        .read_toml_with_extends(fixtures_dir().join("base.toml"))
        .expect("base file should be valid")
        .read_and_complete::<User>()
        .expect("user config")
        .parse()
        .expect("actual config");
    assert!(
        cfg.torii.transport.trusted_proxy_cidrs.is_empty(),
        "trusted proxy CIDRs should default to empty until operators opt in"
    );
}
