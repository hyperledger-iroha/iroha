//! Ensure parliament alternate counts propagate from user config into runtime governance config.
use iroha_config::parameters::user;
use iroha_data_model::governance::types::MAX_PARLIAMENT_BODY_TARGET_SEATS_V1;
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

#[test]
fn governance_rejects_unbounded_parliament_alternate_rosters() {
    let maximum =
        usize::try_from(MAX_PARLIAMENT_BODY_TARGET_SEATS_V1).expect("body bound fits usize");
    let at_bound = user::Governance {
        parliament_alternate_size: maximum,
        ..user::Governance::default()
    }
    .parse();
    assert_eq!(at_bound.parliament_alternate_size, maximum);

    let user_cfg = user::Governance {
        parliament_alternate_size: maximum + 1,
        ..user::Governance::default()
    };
    assert!(
        std::panic::catch_unwind(|| user_cfg.parse()).is_err(),
        "alternate rosters above the first-release body bound must fail at startup"
    );
}
