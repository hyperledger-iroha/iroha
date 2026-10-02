//! Native gateway action and permission descriptors contain claims, never proof capabilities.
use super::{IntoSchema, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::MutateSorafsStreamTokenGateway,
    sorafs::stream_token_gateway::{
        StreamTokenGatewayAdmissionReadbackV1, StreamTokenGatewayAdmissionRecordV1,
        StreamTokenGatewayAdmissionRequestV1,
        native::{
            StreamTokenGatewayActionV1, StreamTokenGatewayCheckSubjectV1,
            StreamTokenGatewayCheckV1, StreamTokenGatewayFinalityFloorV1,
            StreamTokenGatewayPolicyV1, StreamTokenGatewayRequestV1,
        },
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsStreamTokenGateway, CanManageSorafsStreamTokenGateway,
    CanOperateSorafsStreamTokenGateway,
};

#[test]
fn native_stream_token_gateway_exports_complete_claim_and_permission_descriptors() {
    let mut expected = MutateSorafsStreamTokenGateway::schema();
    StreamTokenGatewayAdmissionReadbackV1::update_schema_map(&mut expected);
    CanManageSorafsStreamTokenGateway::update_schema_map(&mut expected);
    CanOperateSorafsStreamTokenGateway::update_schema_map(&mut expected);
    CanCheckSorafsStreamTokenGateway::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted native gateway descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(expected.contains_key::<StreamTokenGatewayRequestV1>());
    assert!(expected.contains_key::<StreamTokenGatewayActionV1>());
    assert!(expected.contains_key::<StreamTokenGatewayCheckV1>());
    assert!(expected.contains_key::<StreamTokenGatewayCheckSubjectV1>());
    assert!(expected.contains_key::<StreamTokenGatewayFinalityFloorV1>());
    assert!(expected.contains_key::<StreamTokenGatewayPolicyV1>());
    assert!(expected.contains_key::<StreamTokenGatewayAdmissionRequestV1>());
    assert!(expected.contains_key::<StreamTokenGatewayAdmissionRecordV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
}
