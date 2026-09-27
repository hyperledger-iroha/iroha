//! Publication descriptors preserve canonical admission material and capacity payload framing.
use super::{IntoSchema, Metadata, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::{
        AssertSorafsPublicationV1, InitializeSorafsProviderAdmissionV1, RegisterCapacityDeclaration,
    },
    sorafs::provider_admission::governance::{
        InitialProviderAdmissionCouncilV1, InitialProviderAdmissionV1,
    },
};
use std::any::TypeId;

#[test]
fn publication_exports_complete_canonical_instruction_descriptors() {
    let mut expected = InitializeSorafsProviderAdmissionV1::schema();
    RegisterCapacityDeclaration::update_schema_map(&mut expected);
    AssertSorafsPublicationV1::update_schema_map(&mut expected);
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted publication descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(find_missing_schema_references(&expected).is_empty());
    assert!(expected.contains_key::<InitialProviderAdmissionCouncilV1>());
    assert!(expected.contains_key::<InitialProviderAdmissionV1>());
    let Metadata::Struct(capacity) = expected.get::<RegisterCapacityDeclaration>().unwrap() else {
        panic!("capacity instruction must retain a named payload field");
    };
    assert_eq!(capacity.declarations.len(), 1);
    assert_eq!(capacity.declarations[0].name, "declaration");
    assert_eq!(capacity.declarations[0].ty, TypeId::of::<Vec<u8>>());
    let Metadata::Struct(provider) = expected.get::<InitialProviderAdmissionV1>().unwrap() else {
        panic!("initial provider must retain its owner and canonical material frame");
    };
    assert_eq!(provider.declarations.len(), 2);
    let material = provider
        .declarations
        .iter()
        .find(|field| field.name == "material")
        .unwrap();
    assert_eq!(material.ty, TypeId::of::<Vec<u8>>());
}
