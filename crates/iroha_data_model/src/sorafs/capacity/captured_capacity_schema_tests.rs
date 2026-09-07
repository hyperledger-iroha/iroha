//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ProviderId>(
        "iroha_data_model::sorafs::capacity::ProviderId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDeclarationRecord>(
        "iroha_data_model::sorafs::capacity::CapacityDeclarationRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityTelemetryRecord>(
        "iroha_data_model::sorafs::capacity::CapacityTelemetryRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityFeeLedgerEntry>(
        "iroha_data_model::sorafs::capacity::CapacityFeeLedgerEntry",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeId>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeEvidence>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeEvidence",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeOutcome>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeOutcome",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeResolution>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeResolution",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeStatus>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CapacityDisputeRecord>(
        "iroha_data_model::sorafs::capacity::CapacityDisputeRecord",
    );
}
