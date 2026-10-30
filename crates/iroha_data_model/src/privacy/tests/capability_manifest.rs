// Focused canonical Exact12 public capability-manifest tests.
#[test]
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
    let generic_rows = [
        (
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV1,
            "zk_ace_authorization_action_v1",
            "authorization_action",
            0,
        ),
        (
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            "anonymous_pgc_payment_action_v1",
            "payment_action",
            6,
        ),
        (
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            "verange_range_proof_v1",
            "component",
            1,
        ),
        (
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            "zk_ams_admission_and_provisioning_v1",
            "admission_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::VegaExistingCredentialZkV1,
            "vega_credential_presentation_v1",
            "presentation_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1,
            "jindo_polynomial_evaluation_v1",
            "component",
            0,
        ),
        (
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            "bootle_lantern_credential_presentation_v1",
            "presentation_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            "orchard_note_action_v1",
            "note_action",
            7,
        ),
        (
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            "fcmp_membership_payment_v1",
            "payment_action",
            2,
        ),
        (
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            "ivm_private_note_action_v1",
            "note_action",
            7,
        ),
        (
            PrivacyProtocolIdV1::PqMaspStarkV1,
            "pq_masp_note_action_v1",
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
        assert_eq!(row.operation_schema.canonical_label(), operation_schema);
        assert_eq!(row.execution_mode.canonical_label(), execution_mode);
        assert_eq!(row.privacy_feature_mask.bits(), feature_mask);
    }
    let x509 = &manifest.protocols[5];
    assert_eq!(
        x509.protocol_id,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
    );
    assert_eq!(
        x509.operation_schema,
        PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1
    );
    assert_eq!(
        x509.execution_mode,
        PrivacyExecutionModeV1::PresentationAction
    );
    assert_eq!(x509.privacy_feature_mask.bits(), 2);
    assert_eq!(
        x509.readiness,
        PrivacyCapabilityReadinessV1::Unavailable(
            PrivacyCapabilityUnavailableReasonV1::CompiledProfile(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable
            )
        ),
        "a public operation mapping must not invent an executable X509 implementation"
    );
    assert!(!x509.is_network_available());
}
#[test]
fn exact12_capability_manifest_is_canonical_content_bound_and_committed() {
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
    assert!(json.contains("\"readiness\":\"available\""));
    assert!(!json.contains("qualification"));
    assert!(!json.contains("available-experimental"));
    assert!(!json.contains("limitation"));
    assert_eq!(manifest.network_id, capability_snapshot().network_id);
    let pgc = &manifest.protocols[1];
    assert_eq!(pgc.readiness, PrivacyCapabilityReadinessV1::Available);
    assert!(pgc.is_network_available());
    for row in manifest
        .protocols
        .iter()
        .filter(|row| row.protocol_id != pgc.protocol_id)
    {
        assert!(
            !row.is_network_available(),
            "this fixture has no executable compiled profile for the other protocols"
        );
    }
}
#[test]
fn revised_jindo_is_available_with_compiled_defaults_and_no_registered_policy() {
    let mut snapshot = capability_snapshot();
    let jindo_profile = profile(&envelope(statement_for(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV1,
    )));
    snapshot.protocols[6].compiled_profile =
        PrivacyCompiledProfileResultV1::Available(jindo_profile);
    snapshot.protocols[6].policy = None;
    snapshot.validate().expect("synthetic actual Jindo profile");
    let manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("project Jindo capability");
    let jindo = &manifest.protocols[6];
    assert_eq!(jindo.readiness, PrivacyCapabilityReadinessV1::Available);
    assert!(jindo.is_network_available());
    assert_eq!(
        jindo.effective_protocol_limits_v1(),
        Some(jindo_profile.protocol_limits)
    );
    assert_eq!(jindo.policy, None);
}
#[test]
fn exact12_capability_manifest_rejects_derived_field_and_digest_substitution() {
    let manifest = exact12_capability_manifest();
    let mut operation = manifest.clone();
    operation.protocols[0].operation_schema = PrivacyOperationSchemaV1::PqMaspNoteActionV1;
    redigest_exact12_capability_manifest(&mut operation);
    assert!(matches!(
        operation.validate(),
        Err(
            PrivacyExact12CapabilityManifestValidationErrorV1::ProtocolRow {
                source: PrivacyExact12CapabilityRowValidationErrorV1::OperationSchemaMismatch { .. },
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
        PrivacyCapabilityUnavailableReasonV1::CompiledProfile(
            PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
        ),
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
fn exact12_capability_manifest_rejects_false_compiled_jindo_availability() {
    let mut snapshot = capability_snapshot();
    snapshot.protocols[6].compiled_profile = PrivacyCompiledProfileResultV1::Unavailable(
        PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
    );
    let mut manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("project unavailable Jindo");
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

#[test]
fn capability_network_identity_is_content_bound_without_release_evidence() {
    let manifest = exact12_capability_manifest();
    let mut other = manifest.clone();
    other.network_id = network_id(0x95);
    assert!(matches!(
        other.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ManifestDigestMismatch { .. })
    ));
    redigest_exact12_capability_manifest(&mut other);
    other
        .validate()
        .expect("a different network has a distinct canonical manifest");
    assert_ne!(manifest.manifest_digest, other.manifest_digest);
    assert_ne!(manifest.network_id, other.network_id);
    // The digest is content identity. The authenticated transport must also compare this
    // field with its expected network; a valid manifest alone is never authentication.
    let json = norito::json::to_json(&manifest).expect("canonical JSON");
    let old_authority = json.replacen('{', "{\"qualification\":null,", 1);
    assert!(norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&old_authority).is_err());
}
#[test]
fn joint_security_failure_survives_canonical_capability_projection() {
    let mut snapshot = capability_snapshot();
    snapshot.protocols[5].compiled_profile = PrivacyCompiledProfileResultV1::Unavailable(
        PrivacyCompiledProfileUnavailableReasonV1::JointSecurityReductionIncomplete,
    );
    let manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("explicit unsupported row");
    assert!(!manifest.protocols[5].is_network_available());
    assert!(
        manifest.protocols[1].is_network_available(),
        "unrelated complete capability is unaffected"
    );
    let bytes = manifest.canonical_bytes().expect("canonical archive");
    assert_eq!(
        norito::decode_from_bytes::<PrivacyExact12CapabilityManifestV1>(&bytes).unwrap(),
        manifest
    );
    let json = norito::json::to_json(&manifest).unwrap();
    assert!(json.contains("joint-security-reduction-incomplete"));
    assert_eq!(
        norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&json).unwrap(),
        manifest
    );
}

#[test]
fn optional_protocol_policy_is_content_bound_but_never_availability_authority() {
    let mut snapshot = capability_snapshot();
    snapshot.protocols[5].policy = Some(PrivacyProtocolPolicyV1 {
        protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        current_limits: PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1,
        pending_update: None,
    });
    let manifest = snapshot
        .exact12_capability_manifest_v1()
        .expect("policy for an unavailable implementation");
    assert!(!manifest.protocols[5].is_network_available());
    assert_eq!(manifest.protocols[5].effective_protocol_limits_v1(), None);
    assert!(manifest.protocols[1].is_network_available());
    let mut substituted = manifest.clone();
    substituted.protocols[5].policy = None;
    assert!(matches!(
        substituted.validate(),
        Err(PrivacyExact12CapabilityManifestValidationErrorV1::ManifestDigestMismatch { .. })
    ));
    redigest_exact12_capability_manifest(&mut substituted);
    substituted
        .validate()
        .expect("absence of optional policy is valid and content-distinct");
    assert!(!substituted.protocols[5].is_network_available());
    assert_ne!(manifest.manifest_digest, substituted.manifest_digest);
    let json = norito::json::to_json(&manifest).unwrap();
    assert!(!json.contains("lifecycle"));
    assert!(!json.contains("activation"));
    let retired = json.replacen("\"policy\":", "\"activation\":", 1);
    assert_ne!(retired, json);
    assert!(norito::json::from_json::<PrivacyExact12CapabilityManifestV1>(&retired).is_err());
}
