//! The account authority has one complete public schema and no decoded runtime capability.
use super::{IntoSchema, MetaMap, Metadata, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
    sorafs::final_promotion_account_custody::{
        FinalPromotionAccountCustodyActionV1, FinalPromotionAccountCustodyCheckV1,
        FinalPromotionAccountCustodyExecutionV1, FinalPromotionAccountCustodyRecordV1,
    },
};

use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotionAccountCustody, CanManageSorafsFinalPromotionAccountCustody,
};

fn account_schema() -> MetaMap {
    let mut schema = MutateSorafsFinalPromotionAccountCustody::schema();
    FinalPromotionAccountCustodyRecordV1::update_schema_map(&mut schema);
    CanManageSorafsFinalPromotionAccountCustody::update_schema_map(&mut schema);
    CanCheckSorafsFinalPromotionAccountCustody::update_schema_map(&mut schema);
    schema
}

#[test]
fn account_custody_export_contains_complete_canonical_instruction_and_record_descriptors() {
    let expected = account_schema();
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted {}",
            descriptor.type_name
        );
    }
    assert!(find_missing_schema_references(&expected).is_empty());
    for name in [
        "MutateSorafsFinalPromotionAccountCustody",
        "FinalPromotionAccountCustodyActionV1",
        "FinalPromotionAccountCustodyCheckV1",
        "FinalPromotionAccountCustodyRevocationV1",
        "FinalPromotionAccountCustodyExecutionV1",
        "FinalPromotionAccountCustodyRecordV1",
        "CanManageSorafsFinalPromotionAccountCustody",
        "CanCheckSorafsFinalPromotionAccountCustody",
    ] {
        assert_eq!(
            expected
                .iter()
                .filter(|(_, value)| value.type_name == name)
                .count(),
            1,
            "{name}"
        );
    }
}

#[test]
fn account_custody_export_excludes_receipt_operations_and_runtime_authority() {
    let schema = account_schema();
    for excluded in [
        "SignerCustodyPolicyV1",
        "SignerCustodyControlStateV1",
        "SignerCustodyRecordV1",
        "FinalPromotionCustodyRecordV1",
        "FinalPromotionOperationRecordV1",
        "SignerFinalPromotionRequestV1",
        "SignerOperationReservationV1",
        "VerifiedSignerCustodyV1",
        "FinalPromotionAuthoritySnapshotV1",
        "PreparedFinalPromotionTransactionV1",
        "FinalPromotionTransactionSigningScopeV1",
    ] {
        assert!(
            schema.iter().all(|(_, value)| value.type_name != excluded),
            "{excluded}"
        );
    }
    let Metadata::Enum(actions) = schema
        .get::<FinalPromotionAccountCustodyActionV1>()
        .unwrap()
    else {
        panic!("action enum")
    };
    assert_eq!(
        actions
            .variants
            .iter()
            .map(|v| (v.tag.as_str(), v.discriminant))
            .collect::<Vec<_>>(),
        [("configure", 0), ("enroll", 1), ("revoke", 2), ("check", 3)]
    );
    for metadata in [
        schema
            .get::<FinalPromotionAccountCustodyRecordV1>()
            .unwrap(),
        schema
            .get::<FinalPromotionAccountCustodyExecutionV1>()
            .unwrap(),
        schema.get::<FinalPromotionAccountCustodyCheckV1>().unwrap(),
    ] {
        let Metadata::Struct(fields) = metadata else {
            panic!("closed public record")
        };
        for field in &fields.declarations {
            assert!(
                ![
                    "block_hash",
                    "signature",
                    "signatures",
                    "operation_id",
                    "reservation",
                    "audit",
                    "verified"
                ]
                .contains(&field.name.as_str()),
                "{}",
                field.name
            );
        }
    }
}

#[test]
fn account_custody_export_preserves_byteframe_and_provenance_owners() {
    let schema = account_schema();
    let Metadata::Struct(record) = schema
        .get::<FinalPromotionAccountCustodyRecordV1>()
        .unwrap()
    else {
        panic!("record")
    };
    let field = |name: &str| {
        record
            .declarations
            .iter()
            .find(|value| value.name == name)
            .unwrap()
            .ty
    };
    assert_eq!(field("control_state"), core::any::TypeId::of::<Vec<u8>>());
    assert_eq!(
        field("enrollment"),
        core::any::TypeId::of::<Option<Vec<u8>>>()
    );
    assert_eq!(
        field("execution"),
        core::any::TypeId::of::<FinalPromotionAccountCustodyExecutionV1>()
    );
}

#[test]
fn account_custody_permissions_have_distinct_single_deployment_schemas() {
    let schema = account_schema();
    assert_ne!(
        core::any::TypeId::of::<CanManageSorafsFinalPromotionAccountCustody>(),
        core::any::TypeId::of::<CanCheckSorafsFinalPromotionAccountCustody>()
    );
    for descriptor in [
        schema
            .get::<CanManageSorafsFinalPromotionAccountCustody>()
            .unwrap(),
        schema
            .get::<CanCheckSorafsFinalPromotionAccountCustody>()
            .unwrap(),
    ] {
        let Metadata::Struct(fields) = descriptor else {
            panic!("deployment permission struct");
        };
        assert_eq!(fields.declarations.len(), 1);
        assert_eq!(fields.declarations[0].name, "deployment_id");
        assert_eq!(fields.declarations[0].ty, core::any::TypeId::of::<String>());
    }
}
