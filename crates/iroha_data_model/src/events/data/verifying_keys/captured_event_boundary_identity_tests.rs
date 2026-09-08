//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::VerifyingKeyEvent>(
        "iroha_data_model::events::data::verifying_keys::model::VerifyingKeyEvent",
        "473d7af304a98ff04f8717def96daa18",
        "473d7af304a98ff04f8717def96daa18",
    );
    check::<super::VerifyingKeyRegistered>(
        "iroha_data_model::events::data::verifying_keys::model::VerifyingKeyRegistered",
        "4c616525d3862bc900e2df7ba16c62c1",
        "4c616525d3862bc900e2df7ba16c62c1",
    );
    check::<super::VerifyingKeyUpdated>(
        "iroha_data_model::events::data::verifying_keys::model::VerifyingKeyUpdated",
        "910eb844fa2896e87dbfefe9dba753f9",
        "910eb844fa2896e87dbfefe9dba753f9",
    );
}
