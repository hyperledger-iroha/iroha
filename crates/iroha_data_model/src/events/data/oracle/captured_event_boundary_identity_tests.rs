//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::FeedEventRecord>(
        "iroha_data_model::events::data::oracle::FeedEventRecord",
        "ab5f4991a6fd1600f69bde82d62505a3",
        "ab5f4991a6fd1600f69bde82d62505a3",
    );
    check::<super::TwitterBindingRecorded>(
        "iroha_data_model::events::data::oracle::TwitterBindingRecorded",
        "10b3b122ed9a52a4ee086ba5542ed4b9",
        "10b3b122ed9a52a4ee086ba5542ed4b9",
    );
    check::<super::TwitterBindingRevoked>(
        "iroha_data_model::events::data::oracle::TwitterBindingRevoked",
        "e10be1966980d4d7c0ed64457e6fb8c5",
        "e10be1966980d4d7c0ed64457e6fb8c5",
    );
    check::<super::OracleChangeProposed>(
        "iroha_data_model::events::data::oracle::OracleChangeProposed",
        "aedd120e29d8daf4ca8f753351821ce5",
        "aedd120e29d8daf4ca8f753351821ce5",
    );
    check::<super::OracleChangeStageUpdated>(
        "iroha_data_model::events::data::oracle::OracleChangeStageUpdated",
        "fbf8552f46cdead8ffd479a357a34f96",
        "fbf8552f46cdead8ffd479a357a34f96",
    );
    check::<super::DefiOracleAttestationRecorded>(
        "iroha_data_model::events::data::oracle::DefiOracleAttestationRecorded",
        "4ae3f308fc5c72b1d5ade4170056d0f4",
        "4ae3f308fc5c72b1d5ade4170056d0f4",
    );
    check::<super::OracleEvent>(
        "iroha_data_model::events::data::oracle::OracleEvent",
        "5c85e3a741617d527a87d684cbd53c11",
        "5c85e3a741617d527a87d684cbd53c11",
    );
}
