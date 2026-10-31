//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::RuntimeUpgradeEvent>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeEvent",
        "e22fa26f291df04f4e9c9a6b14a14235",
        "e22fa26f291df04f4e9c9a6b14a14235",
    );
    check::<super::RuntimeUpgradeScheduled>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeScheduled",
        "cf3ff439555a0f18c6d96d0deba42417",
        "cf3ff439555a0f18c6d96d0deba42417",
    );
    check::<super::RuntimeUpgradeAttested>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeAttested",
        "b2c898103f704fe7e6ef06b472934fc3",
        "b2c898103f704fe7e6ef06b472934fc3",
    );
    check::<super::RuntimeUpgradeLapsed>(
        "iroha_data_model::events::data::runtime_upgrade::model::RuntimeUpgradeLapsed",
        "313708b746ceb826a42b0dec9ab8764d",
        "313708b746ceb826a42b0dec9ab8764d",
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
