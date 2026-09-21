//! Native provider custody descriptors retain opaque frames and exclude runtime qualification.
use super::{IntoSchema, MetaMap, Metadata, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::MutateSorafsStreamTokenCustody,
    sorafs::stream_token_custody::{
        SorafsStreamTokenCustodyActionV1, SorafsStreamTokenCustodyRevocationV1,
        StreamTokenCustodyControlRecordV1,
    },
};
use iroha_executor_data_model::permission::sorafs::CanManageSorafsStreamTokenCustody;
use std::any::TypeId;

fn custody_schema() -> MetaMap {
    let mut schemas = MutateSorafsStreamTokenCustody::schema();
    StreamTokenCustodyControlRecordV1::update_schema_map(&mut schemas);
    CanManageSorafsStreamTokenCustody::update_schema_map(&mut schemas);
    schemas
}

#[test]
fn stream_token_custody_export_contains_the_complete_canonical_public_schema() {
    let expected = custody_schema();
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted provider custody descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(expected.contains_key::<SorafsStreamTokenCustodyActionV1>());
    assert!(expected.contains_key::<SorafsStreamTokenCustodyRevocationV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
}

#[test]
fn stream_token_custody_export_preserves_opaque_frames_without_runtime_proof() {
    let schemas = custody_schema();
    let Metadata::Enum(action) = schemas.get::<SorafsStreamTokenCustodyActionV1>().unwrap() else {
        panic!("native custody action must retain its public enum descriptor");
    };
    // Configuration and enrollment carry complete canonical Manifest frames. Publishing native
    // DTO descriptors must not introduce a competing expanded representation of those frames.
    for tag in ["configure", "enroll"] {
        let variant = action
            .variants
            .iter()
            .find(|variant| variant.tag == tag)
            .unwrap();
        assert_eq!(variant.ty, Some(TypeId::of::<Vec<u8>>()), "{tag}");
    }
    let revocation = action
        .variants
        .iter()
        .find(|variant| variant.tag == "revoke")
        .unwrap();
    assert_eq!(
        revocation.ty,
        Some(TypeId::of::<SorafsStreamTokenCustodyRevocationV1>())
    );
    let Metadata::Struct(record) = schemas.get::<StreamTokenCustodyControlRecordV1>().unwrap()
    else {
        panic!("native custody record must retain named public fields");
    };
    let control = record
        .declarations
        .iter()
        .find(|field| field.name == "control_state")
        .unwrap();
    assert_eq!(control.ty, TypeId::of::<Vec<u8>>());
    for excluded in [
        "StreamTokenCustodyControlSnapshotV1",
        "ControlIndexV1",
        "SignerCustodyPolicyV1",
        "SignerCustodyControlStateV1",
        "SignerCustodyRecordV1",
        "VerifiedSignerCustodyV1",
        "SignerStreamTokenReceiptV1",
        "SignerOperationIntentV1",
        "SignerOperationReservationV1",
        "SignerOperationCommitmentV1",
    ] {
        assert!(
            schemas.iter().all(|(_, entry)| entry.type_name != excluded),
            "opaque frame contents, runtime proof or per-token operation state leaked: {excluded}"
        );
    }
}
