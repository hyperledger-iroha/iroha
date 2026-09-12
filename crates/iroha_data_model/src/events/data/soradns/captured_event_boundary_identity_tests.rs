//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::SoradnsDirectoryEvent>(
        "iroha_data_model::events::data::soradns::model::SoradnsDirectoryEvent",
        "6c7f7b5d1df8cf74f65b5a8714013024",
        "6c7f7b5d1df8cf74f65b5a8714013024",
    );
}
