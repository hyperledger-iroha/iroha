//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::RuntimeUpgradeEvent>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEvent",
        "e22fa26f291df04f4e9c9a6b14a14235",
        "e22fa26f291df04f4e9c9a6b14a14235",
    );
    check::<super::RuntimeUpgradeProposed>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeProposed",
        "902691a361dc4211daf45e297c278542",
        "902691a361dc4211daf45e297c278542",
    );
    check::<super::RuntimeUpgradeActivated>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeActivated",
        "581ca8c95a9a7cd4d3895a555c3701bf",
        "581ca8c95a9a7cd4d3895a555c3701bf",
    );
    check::<super::RuntimeUpgradeCanceled>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeCanceled",
        "ec7930ecea35b157aedaacfb1d01b346",
        "ec7930ecea35b157aedaacfb1d01b346",
    );
}
