//! Ensure citizen service discipline fields roundtrip from user to actual config.

use std::collections::BTreeMap;

use iroha_config::parameters::{defaults, user};

#[test]
fn citizen_service_defaults_and_overrides_parse() {
    let default_actual = user::Governance::default().parse();
    assert_eq!(
        default_actual.citizen_service.seat_cooldown_blocks,
        defaults::governance::citizen_service::SEAT_COOLDOWN_BLOCKS
    );
    assert_eq!(
        default_actual.citizen_service.max_seats_per_epoch,
        defaults::governance::citizen_service::MAX_SEATS_PER_EPOCH
    );
    assert_eq!(
        default_actual.citizen_service.free_declines_per_epoch,
        defaults::governance::citizen_service::FREE_DECLINES_PER_EPOCH
    );

    let mut role_multipliers = BTreeMap::new();
    role_multipliers.insert("council".to_string(), 3);
    let user_cfg = user::Governance {
        citizen_service: user::CitizenServiceDiscipline {
            seat_cooldown_blocks: 42,
            max_seats_per_epoch: 2,
            free_declines_per_epoch: 0,
            decline_slash_bps: 111,
            no_show_slash_bps: 222,
            misconduct_slash_bps: 333,
            role_bond_multipliers: role_multipliers.clone(),
        },
        ..user::Governance::default()
    };
    let parsed = user_cfg.parse();
    assert_eq!(parsed.citizen_service.seat_cooldown_blocks, 42);
    assert_eq!(parsed.citizen_service.max_seats_per_epoch, 2);
    assert_eq!(parsed.citizen_service.free_declines_per_epoch, 0);
    assert_eq!(parsed.citizen_service.decline_slash_bps, 111);
    assert_eq!(parsed.citizen_service.no_show_slash_bps, 222);
    assert_eq!(parsed.citizen_service.misconduct_slash_bps, 333);
    assert_eq!(
        parsed.citizen_service.bond_multiplier_for_role("council"),
        3
    );
    assert_eq!(
        parsed
            .citizen_service
            .bond_multiplier_for_role("policy_jury"),
        1
    );
}
