// Focused canonical Exact12 public capability-manifest tests.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the capability rows form one closed Exact12 manifest matrix"
)]
fn exact12_capability_manifest_has_canonical_generic_and_x509_operation_rows() {
    let manifest = exact12_capability_manifest();
    assert_eq!(
        PRIVACY_EXACT12_CAPABILITY_MANIFEST_SCHEMA_NAME_V1,
        "iroha.privacy.exact12-capability-manifest.v1"
    );
    assert_eq!(
        manifest.version,
        PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1
    );
    assert!(!manifest.manifest_digest.is_zero());
    assert_eq!(manifest.protocols.len(), PrivacyProtocolIdV1::COUNT);
    let generic_rows: [(PrivacyProtocolIdV1, &[&str], &str, u8); 11] = [
        (
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            &["zk_ace_authorization_action_v1"],
            "authorization_action",
            0,
        ),
        (
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            &["anonymous_pgc_payment_action_v1"],
            "payment_action",
            6,
        ),
        (
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            &["verange_range_proof_v1"],
            "component",
            1,
        ),
        (
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            &[
                "zk_ams_batch_admission_action_v1",
                "zk_ams_provision_account_action_v1",
            ],
            "admission_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            &["vega_credential_presentation_v1"],
            "presentation_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
            &["jindo_polynomial_evaluation_v1"],
            "component",
            0,
        ),
        (
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            &["bootle_lantern_credential_presentation_v1"],
            "presentation_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            &["orchard_note_action_v1"],
            "note_action",
            7,
        ),
        (
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            &["fcmp_membership_payment_v1"],
            "payment_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            &["ivm_private_note_action_v1"],
            "note_action",
            7,
        ),
        (
            PrivacyProtocolIdV1::PqMaspStarkV0,
            &["pq_masp_note_action_v1"],
            "note_action",
            31,
        ),
    ];
    for (protocol_id, operation_schema, execution_mode, feature_mask) in generic_rows {
        let row = manifest
            .protocols
            .iter()
            .find(|row| row.protocol_id == protocol_id)
            .expect("generic Exact12 row");
        assert_eq!(
            row.operation_schemas
                .iter()
                .map(PrivacyOperationSchemaV1::canonical_label)
                .collect::<Vec<_>>(),
            operation_schema
        );
        assert_eq!(row.execution_mode.canonical_label(), execution_mode);
        assert_eq!(row.privacy_feature_mask.bits(), feature_mask);
    }
    assert_eq!(
        manifest
            .protocols
            .iter()
            .flat_map(|row| row.operation_schemas.iter())
            .count(),
        13,
        "Exact12 has twelve protocols and thirteen closed action schemas"
    );
    let x509 = &manifest.protocols[5];
    assert_eq!(
        x509.protocol_id,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
    );
    assert_eq!(
        x509.operation_schemas,
        PrivacyOperationSchemaSetV1::one(PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1)
    );
    assert_eq!(
        x509.execution_mode,
        PrivacyExecutionModeV1::PresentationAction
    );
    assert_eq!(x509.privacy_feature_mask.bits(), 2);
    assert_eq!(
        x509.readiness,
        PrivacyCapabilityReadinessV1::Unavailable(
            PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable
        ),
        "a public operation mapping must not open the ZK-X509 readiness gate"
    );
    assert!(!x509.is_network_available());
}

#[test]
fn exact12_operation_union_has_typed_protocol_and_ledger_effect_mappings() {
    let manifest = exact12_capability_manifest();
    let operations = manifest
        .protocols
        .iter()
        .flat_map(|row| row.operation_schemas.iter())
        .collect::<Vec<_>>();
    assert_eq!(operations.len(), 13);
    for operation in operations {
        let row = manifest
            .protocols
            .iter()
            .find(|row| row.operation_schemas.contains(operation))
            .expect("operation belongs to one Exact12 row");
        assert_eq!(operation.protocol_id(), row.protocol_id);
        assert!(!operation.ledger_effect_kind().canonical_label().is_empty());
    }
    assert_eq!(
        PrivacyOperationSchemaV1::VeRangeRangeProofV1.ledger_effect_kind(),
        PrivacyLedgerEffectKindV1::VerificationOnly
    );
    assert_eq!(
        PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1.ledger_effect_kind(),
        PrivacyLedgerEffectKindV1::ZkAmsBatchAdmission
    );
    assert_eq!(
        PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1.ledger_effect_kind(),
        PrivacyLedgerEffectKindV1::ZkAmsProvisionAccount
    );
}
#[test]
fn exact12_capability_manifest_is_canonical_self_authenticating_and_committed() {
    let manifest = exact12_capability_manifest();
    manifest.validate().expect("valid Exact12 manifest");
    assert_eq!(
        manifest
            .computed_manifest_digest()
            .expect("compute manifest digest"),
        manifest.manifest_digest
    );
    let canonical = manifest
        .canonical_bytes()
        .expect("canonical manifest bytes");
    assert_eq!(
        canonical,
        norito::encode_canonical(&manifest).expect("direct canonical manifest encoding")
    );
    let decoded: PrivacyExact12CapabilityManifestV1 =
        norito::decode_from_bytes(&canonical).expect("decode canonical manifest");
    assert_eq!(decoded, manifest);
    decoded.validate().expect("validate decoded manifest");
    let json = norito::json::to_json(&manifest).expect("manifest JSON");
    let decoded_json: PrivacyExact12CapabilityManifestV1 =
        norito::json::from_json(&json).expect("decode manifest JSON");
    assert_eq!(decoded_json, manifest);
    assert!(json.contains("missing-distribution-wide-knowledge-soundness-evidence"));
    let pgc = &manifest.protocols[1];
    assert_eq!(pgc.readiness, PrivacyCapabilityReadinessV1::Available);
    assert_eq!(
        pgc.activation_state,
        PrivacyCapabilityActivationStateV1::Active
    );
    assert!(pgc.is_network_available());
    for row in manifest
        .protocols
        .iter()
        .filter(|row| row.protocol_id != pgc.protocol_id)
    {
        assert!(
            !row.is_network_available(),
            "local readiness without committed Active state is not network availability"
        );
    }
}
#[test]
fn revised_jindo_is_explicitly_experimental_and_never_falsely_certified() {
    let mut snapshot = capability_snapshot();
    let jindo_activation = activation(&envelope(statement_for(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
    )));
    snapshot.protocols[6].compiled_profile =
        PrivacyCompiledProfileResultV1::Available(compiled_profile_snapshot(&jindo_activation));
    snapshot
        .validate()
        .expect("synthetic available Jindo profile");
    let manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("project Jindo capability");
    let jindo = &manifest.protocols[6];
    assert_eq!(
        jindo.readiness,
        PrivacyCapabilityReadinessV1::AvailableExperimental
    );
    assert_eq!(
        jindo.limitation,
        Some(PrivacyCapabilityLimitationV1::MissingDistributionWideKnowledgeSoundnessEvidence)
    );
    assert!(
        !jindo.is_network_available(),
        "Jindo has no committed activation"
    );
}
#[test]
fn exact12_capability_manifest_rejects_derived_field_and_digest_substitution() {
    let manifest = exact12_capability_manifest();
    let mut operation = manifest.clone();
    operation.protocols[0].operation_schemas =
        PrivacyOperationSchemaSetV1::one(PrivacyOperationSchemaV1::PqMaspNoteActionV1);
    redigest_exact12_capability_manifest(&mut operation);
    assert!(matches!(
        operation.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::OperationSchemasMismatch { .. },
                ..
            }
        )
    ));
    let mut zk_ams_operation = manifest.clone();
    zk_ams_operation.protocols[3].operation_schemas =
        PrivacyOperationSchemaSetV1::one(PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1);
    redigest_exact12_capability_manifest(&mut zk_ams_operation);
    assert!(matches!(
        zk_ams_operation.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::OperationSchemasMismatch { .. },
                ..
            }
        )
    ));
    let mut execution = manifest.clone();
    execution.protocols[0].execution_mode = PrivacyExecutionModeV1::NoteAction;
    redigest_exact12_capability_manifest(&mut execution);
    assert!(matches!(
        execution.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::ExecutionModeMismatch { .. },
                ..
            }
        )
    ));
    let mut feature_mask = manifest.clone();
    feature_mask.protocols[0].privacy_feature_mask = PrivacyFeatureMaskV1::new(1);
    redigest_exact12_capability_manifest(&mut feature_mask);
    assert!(matches!(
        feature_mask.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::FeatureMaskMismatch { .. },
                ..
            }
        )
    ));
    let mut readiness = manifest.clone();
    readiness.protocols[1].readiness = PrivacyCapabilityReadinessV1::Unavailable(
        PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
    );
    redigest_exact12_capability_manifest(&mut readiness);
    assert!(matches!(
        readiness.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::ReadinessMismatch { .. },
                ..
            }
        )
    ));
    let mut activation = manifest.clone();
    activation.protocols[1].activation_state = PrivacyCapabilityActivationStateV1::Suspended;
    redigest_exact12_capability_manifest(&mut activation);
    assert!(matches!(
        activation.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::ActivationStateMismatch { .. },
                ..
            }
        )
    ));
    let mut limitation = manifest.clone();
    limitation.protocols[6].limitation = None;
    redigest_exact12_capability_manifest(&mut limitation);
    assert!(matches!(
        limitation.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::LimitationMismatch { .. },
                ..
            }
        )
    ));
    let mut digest = manifest;
    digest.manifest_digest.0[0] ^= 0x80;
    assert!(matches!(
        digest.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ManifestDigestMismatch { .. })
    ));
}
#[test]
fn exact12_capability_manifest_rejects_shape_zero_digest_and_json_adversaries() {
    let manifest = exact12_capability_manifest();
    let mut missing = manifest.clone();
    missing.protocols.pop();
    redigest_exact12_capability_manifest(&mut missing);
    assert!(matches!(
        missing.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolCount { .. })
    ));
    let mut duplicate = manifest.clone();
    duplicate.protocols[2] = duplicate.protocols[1];
    redigest_exact12_capability_manifest(&mut duplicate);
    assert!(matches!(
        duplicate.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolOrder { .. })
    ));
    let mut zero_digest = manifest.clone();
    zero_digest.manifest_digest = PrivacyExact12CapabilityManifestDigestV1::new([0; 32]);
    assert_eq!(
        zero_digest.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ZeroManifestDigest)
    );
    let canonical = norito::json::to_json(&manifest).expect("canonical manifest JSON");
    let unknown = canonical.replacen('{', "{\"legacy_catalog_available\":true,", 1);
    assert!(
        norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&unknown).is_err(),
        "unknown availability fields must fail closed"
    );
    let duplicate_version = canonical.replacen('{', "{\"version\":1,", 1);
    assert!(
        norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&duplicate_version).is_err(),
        "duplicate manifest fields must fail closed"
    );
    let execution_alias = canonical.replacen("authorization_action", "authorization", 1);
    assert!(
        norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&execution_alias).is_err(),
        "execution-mode aliases must fail closed"
    );
}
#[test]
fn exact12_capability_manifest_rejects_false_jindo_certification() {
    let mut snapshot = capability_snapshot();
    let jindo_activation = activation(&envelope(statement_for(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
    )));
    snapshot.protocols[6].compiled_profile =
        PrivacyCompiledProfileResultV1::Available(compiled_profile_snapshot(&jindo_activation));
    let mut manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("project available-experimental Jindo");
    manifest.protocols[6].readiness = PrivacyCapabilityReadinessV1::Available;
    redigest_exact12_capability_manifest(&mut manifest);
    assert!(matches!(
        manifest.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::ReadinessMismatch { .. },
                ..
            }
        )
    ));
}
