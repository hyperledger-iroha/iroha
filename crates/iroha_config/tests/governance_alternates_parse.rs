//! Ensure parliament alternate counts propagate from user config into runtime governance config.
use iroha_config::parameters::user;
#[test]
fn governance_parses_parliament_alternate_size() {
    // User config with explicit alternate size
    let user_cfg = user::Governance {
        parliament_alternate_size: 5,
        ..user::Governance::default()
    };
    let actual_cfg = user_cfg.parse();
    assert_eq!(actual_cfg.parliament_alternate_size, 5);
    // The canonical default is explicit rather than inherited from another size.
    let default_user = user::Governance::default();
    let actual_default = default_user.parse();
    assert_eq!(actual_default.parliament_alternate_size, 21);
}
