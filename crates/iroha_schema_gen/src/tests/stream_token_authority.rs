//! Native stream-token descriptors expose typed claims without runtime verified capabilities.
use super::{IntoSchema, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::stream_token_authority::{
        StreamTokenAuthorityActionV1, StreamTokenAuthorityRequestV1, StreamTokenCheckV1,
        StreamTokenNativeOperationV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamToken, CanOperateSorafsStreamToken,
};

#[test]
fn native_stream_token_exports_complete_claim_and_permission_descriptors() {
    let mut expected = MutateSorafsStreamTokenAuthority::schema();
    StreamTokenNativeOperationV1::update_schema_map(&mut expected);
    CanCheckSorafsStreamToken::update_schema_map(&mut expected);
    CanOperateSorafsStreamToken::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted native stream-token descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(expected.contains_key::<StreamTokenAuthorityRequestV1>());
    assert!(expected.contains_key::<StreamTokenAuthorityActionV1>());
    assert!(expected.contains_key::<StreamTokenCheckV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
    for excluded in [
        "VerifiedStreamTokenAuthorityCheckV1",
        "StreamTokenAuthoritySnapshotV1",
        "VerifiedSignerCustodyV1",
    ] {
        assert!(
            expected
                .iter()
                .all(|(_, entry)| entry.type_name != excluded),
            "runtime verification capability leaked into public claim schema: {excluded}"
        );
    }
}
