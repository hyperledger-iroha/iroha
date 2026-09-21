//! Public native authority descriptors retain canonical nested owners and exclude runtime proof.
use super::{IntoSchema, MetaMap, Metadata, find_missing_schema_references};
use iroha_data_model::{
    isi::sorafs::MutateSorafsFinalPromotionAuthority,
    sorafs::final_promotion_authority::{
        FinalPromotionCheckSubjectV1, FinalPromotionCheckV1, FinalPromotionCompleteV1,
        FinalPromotionCompletedV1, FinalPromotionCustodyRecordV1, FinalPromotionExecutionV1,
        FinalPromotionOperationRecordV1,
    },
};
use iroha_executor_data_model::permission::sorafs::{
    CanCheckSorafsFinalPromotion, CanManageSorafsFinalPromotionCustody,
    CanOperateSorafsFinalPromotion,
};

fn authority_schema() -> MetaMap {
    let mut schemas = MutateSorafsFinalPromotionAuthority::schema();
    FinalPromotionCustodyRecordV1::update_schema_map(&mut schemas);
    FinalPromotionOperationRecordV1::update_schema_map(&mut schemas);
    CanManageSorafsFinalPromotionCustody::update_schema_map(&mut schemas);
    CanOperateSorafsFinalPromotion::update_schema_map(&mut schemas);
    CanCheckSorafsFinalPromotion::update_schema_map(&mut schemas);
    schemas
}

#[test]
fn final_promotion_export_contains_the_complete_canonical_public_schema() {
    let expected = authority_schema();
    let exported = crate::build_schemas();
    let exported: std::collections::BTreeMap<_, _> = exported.iter().collect();
    for (id, descriptor) in expected.iter() {
        assert_eq!(
            exported.get(id).copied(),
            Some(descriptor),
            "missing or substituted public descriptor: {}",
            descriptor.type_name
        );
    }
    assert!(find_missing_schema_references(&expected).is_empty());
    // The native Check and shared Manifest request/operation descriptors retain their owners.
    // No direct Manifest dependency or duplicate schema definitions are needed in the generator.
    for name in [
        "SignerOperationActionV1",
        "SignerOperationAuditHeadV1",
        "SignerOperationIntentV1",
        "SignerOperationReservationV1",
        "SignerOperationCommitmentV1",
        "SignerOperationCustodyV1",
        "SignerFinalPromotionRequestV1",
        "FinalPromotionCheckV1",
        "FinalPromotionCheckSubjectV1",
    ] {
        assert_eq!(
            expected
                .iter()
                .filter(|(_, entry)| entry.type_name == name)
                .count(),
            1,
            "canonical nested operation descriptor must occur exactly once: {name}"
        );
    }
}

#[test]
fn final_promotion_export_excludes_runtime_capabilities_and_unreleased_signatures() {
    let schemas = authority_schema();
    for excluded in [
        "StreamTokenCustodyPolicyV1",
        "StreamTokenCustodyControlStateV1",
        "FinalPromotionAuthoritySnapshotV1",
        "FinalPromotionOperationHeadV1",
        "ControlIndexV1",
        "OperationIndexV1",
        "SignerFinalPromotionReceiptV1",
        "VerifiedSignerCustodyV1",
        "VerifiedFinalPromotionSignerReceiptV1",
        "SignerFinalPromotionExpectedV1",
        "PreparedFinalPromotionStatementV1",
    ] {
        assert!(
            schemas.iter().all(|(_, entry)| entry.type_name != excluded),
            "runtime proof or retired schema must not enter native public records: {excluded}"
        );
    }
    for metadata in [
        schemas.get::<FinalPromotionCompleteV1>().unwrap(),
        schemas.get::<FinalPromotionCompletedV1>().unwrap(),
        schemas.get::<FinalPromotionOperationRecordV1>().unwrap(),
        schemas.get::<FinalPromotionExecutionV1>().unwrap(),
    ] {
        let Metadata::Struct(fields) = metadata else {
            panic!("native operation records must expose named public fields");
        };
        for field in &fields.declarations {
            assert!(
                ![
                    "block_hash",
                    "signature",
                    "signatures",
                    "statement_signature",
                    "audit_signature",
                    "provenance_signature",
                    "response_signature",
                ]
                .contains(&field.name.as_str()),
                "native mutation/record cannot publish unreleased proof or its own block hash: {}",
                field.name
            );
        }
    }
}

#[test]
fn final_promotion_check_export_keeps_the_public_request_and_original_record_owners() {
    let schemas = authority_schema();
    let Metadata::Struct(check) = schemas.get::<FinalPromotionCheckV1>().unwrap() else {
        panic!("Check must retain its closed public record");
    };
    let request_type = check
        .declarations
        .iter()
        .find(|field| field.name == "request")
        .unwrap()
        .ty;
    let (_, request) = schemas.iter().find(|(id, _)| **id == request_type).unwrap();
    assert_eq!(request.type_name, "SignerFinalPromotionRequestV1");
    let Metadata::Enum(subject) = schemas.get::<FinalPromotionCheckSubjectV1>().unwrap() else {
        panic!("Check must retain a closed phase enum");
    };
    assert_eq!(subject.variants.len(), 6);
    for variant in &subject.variants[1..] {
        assert_eq!(
            variant.ty,
            Some(std::any::TypeId::of::<FinalPromotionOperationRecordV1>()),
            "{} must reuse exact original native coordinates",
            variant.tag
        );
    }
    let Metadata::Struct(operation) = schemas.get::<FinalPromotionOperationRecordV1>().unwrap()
    else {
        panic!("original native operation record required");
    };
    assert!(
        operation.declarations.iter().all(|field| {
            ![
                "challenge",
                "subject",
                "phase",
                "minimum_height",
                "minimum_block_hash",
            ]
            .contains(&field.name.as_str())
        }),
        "no-write Check coordinates must not enter permanent operation history"
    );
}

#[test]
fn receipt_check_schema_pins_canonical_operator_and_distinct_observer_permission() {
    let schemas = authority_schema();
    let Metadata::Struct(check) = schemas.get::<FinalPromotionCheckV1>().unwrap() else {
        panic!("closed Check fields required");
    };
    assert_eq!(
        check
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        [
            "challenge",
            "network_id",
            "expected_operator",
            "minimum_height",
            "minimum_block_hash",
            "request",
            "subject"
        ]
    );
    assert_eq!(
        check.declarations[2].ty,
        std::any::TypeId::of::<iroha_data_model::account::AccountId>()
    );
    let Metadata::Struct(permission) = schemas.get::<CanCheckSorafsFinalPromotion>().unwrap()
    else {
        panic!("closed observer deployment permission required");
    };
    assert_eq!(permission.declarations.len(), 1);
    assert_eq!(permission.declarations[0].name, "deployment_id");
    assert_eq!(
        permission.declarations[0].ty,
        std::any::TypeId::of::<String>()
    );
    assert_ne!(
        std::any::TypeId::of::<CanCheckSorafsFinalPromotion>(),
        std::any::TypeId::of::<CanOperateSorafsFinalPromotion>()
    );
    assert_ne!(std::any::TypeId::of::<CanCheckSorafsFinalPromotion>(), std::any::TypeId::of::<iroha_executor_data_model::permission::sorafs::CanCheckSorafsFinalPromotionAccountCustody>());
    let exported = crate::build_schemas();
    assert_eq!(
        exported.get::<CanCheckSorafsFinalPromotion>(),
        schemas.get::<CanCheckSorafsFinalPromotion>()
    );
}
