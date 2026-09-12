//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::SpaceDirectoryEvent>(
        "iroha_data_model::events::data::space_directory::model::SpaceDirectoryEvent",
        "dc99ee080f56901693be103e51c5683c",
        "dc99ee080f56901693be103e51c5683c",
    );
    check::<super::SpaceDirectoryManifestActivated>(
        "iroha_data_model::events::data::space_directory::model::SpaceDirectoryManifestActivated",
        "140ecf5ee054efd0320a366d67ac969a",
        "140ecf5ee054efd0320a366d67ac969a",
    );
    check::<super::SpaceDirectoryManifestExpired>(
        "iroha_data_model::events::data::space_directory::model::SpaceDirectoryManifestExpired",
        "15bb04775de33b40422d3f436c5a8e2b",
        "15bb04775de33b40422d3f436c5a8e2b",
    );
    check::<super::SpaceDirectoryManifestRevoked>(
        "iroha_data_model::events::data::space_directory::model::SpaceDirectoryManifestRevoked",
        "1ad12fa9643f525a1d8dc50dfde2dd72",
        "1ad12fa9643f525a1d8dc50dfde2dd72",
    );
}
