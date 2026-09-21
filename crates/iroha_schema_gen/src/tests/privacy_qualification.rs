//! Publish the complete qualification closure and its exact canonical consumer identities.
use super::{IntoSchema, Metadata, find_missing_schema_references};
use iroha_data_model::privacy::{PrivacyExact12QualificationRecordV1, PrivacyReleaseSdkConsumerV1};

#[test]
fn privacy_qualification_export_contains_the_complete_native_schema() {
    let expected = PrivacyExact12QualificationRecordV1::schema();
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted qualification descriptor: {}",
            descriptor.type_name,
        );
    }
    assert!(expected.contains_key::<PrivacyReleaseSdkConsumerV1>());
    assert!(find_missing_schema_references(&expected).is_empty());
}

#[test]
fn privacy_qualification_export_preserves_all_ten_consumer_tags_and_indices() {
    let schemas = crate::build_schemas();
    let Metadata::Enum(consumers) = schemas.get::<PrivacyReleaseSdkConsumerV1>().unwrap() else {
        panic!("release consumers must have their native closed enum schema");
    };
    let expected = [
        ("kotlin_jvm", 0_u32),
        ("kotlin_android", 1),
        ("java_source_kotlin", 10),
        ("swift_c_bridge", 3),
        ("javascript_napi", 4),
        ("python_pyo3", 5),
        ("csharp", 6),
        ("cli", 7),
        ("openapi", 8),
        ("genesis_tooling", 9),
    ];
    assert_eq!(PrivacyReleaseSdkConsumerV1::ALL.len(), 10);
    assert_eq!(consumers.variants.len(), expected.len());
    for (variant, (tag, discriminant)) in consumers.variants.iter().zip(expected) {
        assert_eq!(variant.tag, tag);
        assert_eq!(variant.discriminant, discriminant);
        assert_eq!(variant.ty, None);
    }
}
