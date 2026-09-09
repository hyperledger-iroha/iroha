//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::ProofEvent>(
        "iroha_data_model::events::data::proof::model::ProofEvent",
        "10be4548153f8415bfa0110e4657da66",
        "10be4548153f8415bfa0110e4657da66",
    );
    check::<super::ProofVerified>(
        "iroha_data_model::events::data::proof::model::ProofVerified",
        "54e6e5f90967d09db2ea6ad62f3a20c2",
        "54e6e5f90967d09db2ea6ad62f3a20c2",
    );
    check::<super::ProofRejected>(
        "iroha_data_model::events::data::proof::model::ProofRejected",
        "1af3316d28ed9e892f72a34e8a332d56",
        "1af3316d28ed9e892f72a34e8a332d56",
    );
    check::<super::ProofPruned>(
        "iroha_data_model::events::data::proof::model::ProofPruned",
        "e7db1c7f1ef18bd9abd8dbb49dd1eb9c",
        "e7db1c7f1ef18bd9abd8dbb49dd1eb9c",
    );
    check::<super::ProofPruneOrigin>(
        "iroha_data_model::events::data::proof::model::ProofPruneOrigin",
        "eb77f8795cd404da3776a1d8acae9c99",
        "eb77f8795cd404da3776a1d8acae9c99",
    );
}
