//! Recursive structure of the actual persisted privacy storage owners.

use super::*;
use iroha_schema::{IntoSchema, Metadata};
use std::any::TypeId;

#[test]
fn every_privacy_storage_owner_exposes_its_recursive_schema() {
    fn owner<T: IntoSchema>() {
        assert!(T::schema().get::<T>().is_some());
    }
    owner::<PrivacyPgcAccountKeyV1>();
    owner::<PrivacyPgcAccountStateV1>();
    owner::<PrivacyPgcPoolInvariantKeyV1>();
    owner::<PrivacyPgcPoolInvariantV1>();
    owner::<PrivacyNullifierKeyV1>();
    owner::<PrivacyCommitmentKeyV1>();
    owner::<PrivacyRootKeyV1>();
    owner::<PrivacyRootProvenanceV1>();
    owner::<PrivacyRootHeadKeyV1>();
    owner::<PrivacyRootHeadRecordV1>();
    owner::<PrivacyStateItemRecordV1>();
}

#[test]
fn persisted_privacy_schema_retains_nested_provenance_and_frontier_layouts() {
    let head = PrivacyRootHeadRecordV1::schema();
    let Some(Metadata::Struct(fields)) = head.get::<PrivacyRootHeadRecordV1>() else {
        panic!("root head must retain its actual named fields");
    };
    assert_eq!(
        fields
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "epoch",
            "root",
            "provenance",
            "retention_anchor",
            "retained_root_count"
        ]
    );
    assert_eq!(
        fields.declarations[2].ty,
        TypeId::of::<PrivacyRootProvenanceV1>()
    );
    assert_eq!(
        fields.declarations[3].ty,
        TypeId::of::<Option<PrivacyRootRetentionAnchorV1>>()
    );
    assert!(matches!(
        head.get::<PrivacyRootProvenanceV1>(),
        Some(Metadata::Enum(_))
    ));
    assert!(matches!(
        head.get::<PrivacyRootRetentionAnchorV1>(),
        Some(Metadata::Struct(_))
    ));
    assert!(head.get::<PrivacyPgcPoolInvariantDigestV1>().is_some());

    let items = PrivacyStateItemRecordV1::schema();
    assert!(items.get::<PrivacyOrchardPoolStateV1>().is_some());
    assert!(items.get::<PrivacyPublicReserveOwnerV1>().is_some());
    assert!(
        items
            .get::<PrivacyProofManagedPoolAccumulatorStateV1>()
            .is_some()
    );
    assert!(
        items
            .get::<PrivacyProofManagedAccumulatorStateV1>()
            .is_some()
    );
    let Some(Metadata::Struct(frontier)) = items.get::<PrivacyFcmpAccumulatorStateV1>() else {
        panic!("FCMP frontier must retain its nested durable vectors");
    };
    let levels = frontier
        .declarations
        .iter()
        .find(|field| field.name == "levels")
        .unwrap();
    assert_eq!(levels.ty, TypeId::of::<Vec<Vec<[u8; 32]>>>());
    assert!(matches!(
        items.get::<Vec<Vec<[u8; 32]>>>(),
        Some(Metadata::Vec(_))
    ));
    assert!(matches!(items.get::<[u8; 32]>(), Some(Metadata::Array(meta)) if meta.len == 32));
}
