#[test]
fn expectations_reject_every_malformed_ordered_proof_artifact_shape() {
    let ordinary_index = usize::from(privacy_release_stage_ordinal_v1(
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ));
    let pgc_maximum_index = usize::from(privacy_release_stage_ordinal_v1(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ));
    let zk_ams_maximum_index = usize::from(privacy_release_stage_ordinal_v1(
        PrivacyProtocolIdV1::IrohaZkAmsV1,
        PrivacyReleaseCaseKindV1::MaximumShapeResource,
    ));
    assert_eq!(
        canonical_expectations_v1().stages[ordinary_index]
            .evidence
            .proof_artifacts
            .len(),
        1
    );
    assert_eq!(
        canonical_expectations_v1().stages[pgc_maximum_index]
            .evidence
            .proof_artifacts
            .len(),
        2
    );
    assert_eq!(
        canonical_expectations_v1().stages[zk_ams_maximum_index]
            .evidence
            .proof_artifacts
            .len(),
        2
    );

    let mut malformed = Vec::new();

    let mut zero = canonical_expectations_v1();
    zero.stages[ordinary_index].evidence.proof_artifacts.clear();
    malformed.push(zero);

    let mut extra = canonical_expectations_v1();
    let mut extra_artifact = extra.stages[ordinary_index].evidence.proof_artifacts[0].clone();
    extra_artifact.artifact_ordinal = 1;
    extra.stages[ordinary_index]
        .evidence
        .proof_artifacts
        .push(extra_artifact);
    malformed.push(extra);

    let mut missing_required = canonical_expectations_v1();
    missing_required.stages[pgc_maximum_index]
        .evidence
        .proof_artifacts
        .pop();
    malformed.push(missing_required);

    let mut extra_required = canonical_expectations_v1();
    let mut third = extra_required.stages[zk_ams_maximum_index]
        .evidence
        .proof_artifacts[1]
        .clone();
    third.artifact_ordinal = 2;
    extra_required.stages[zk_ams_maximum_index]
        .evidence
        .proof_artifacts
        .push(third);
    malformed.push(extra_required);

    let mut reordered = canonical_expectations_v1();
    reordered.stages[pgc_maximum_index]
        .evidence
        .proof_artifacts
        .swap(0, 1);
    malformed.push(reordered);

    let mut duplicate_ordinal = canonical_expectations_v1();
    duplicate_ordinal.stages[pgc_maximum_index]
        .evidence
        .proof_artifacts[1]
        .artifact_ordinal = 0;
    malformed.push(duplicate_ordinal);

    let mut non_contiguous = canonical_expectations_v1();
    non_contiguous.stages[zk_ams_maximum_index]
        .evidence
        .proof_artifacts[1]
        .artifact_ordinal = 2;
    malformed.push(non_contiguous);

    let mut zero_hash = canonical_expectations_v1();
    zero_hash.stages[ordinary_index].evidence.proof_artifacts[0].proof_sha256 = [0; 32];
    malformed.push(zero_hash);

    let mut zero_bytes = canonical_expectations_v1();
    let artifact = &mut zero_bytes.stages[ordinary_index].evidence.proof_artifacts[0];
    artifact.canonical_proof_bytes.clear();
    refresh_artifact_hash(artifact);
    malformed.push(zero_bytes);

    let mut zero_ceiling = canonical_expectations_v1();
    zero_ceiling.stages[ordinary_index].evidence.proof_artifacts[0].proof_bytes_ceiling = 0;
    malformed.push(zero_ceiling);

    let mut substituted_ceiling = canonical_expectations_v1();
    let artifact = &mut substituted_ceiling.stages[ordinary_index]
        .evidence
        .proof_artifacts[0];
    artifact.proof_bytes_ceiling = artifact
        .proof_bytes_ceiling
        .checked_sub(1)
        .expect("FCMP++ ceiling is nonzero");
    malformed.push(substituted_ceiling);

    let mut over_ceiling = canonical_expectations_v1();
    let artifact = &mut over_ceiling.stages[ordinary_index].evidence.proof_artifacts[0];
    artifact.canonical_proof_bytes.resize(
        usize::try_from(artifact.proof_bytes_ceiling).expect("FCMP++ ceiling fits usize") + 1,
        0x5a,
    );
    refresh_artifact_hash(artifact);
    malformed.push(over_ceiling);

    let mut unbounded = canonical_expectations_v1();
    unbounded.stages[ordinary_index].evidence.proof_artifacts[0].proof_bytes_ceiling =
        PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1 + 1;
    malformed.push(unbounded);

    let mut hash_mismatch = canonical_expectations_v1();
    hash_mismatch.stages[ordinary_index]
        .evidence
        .proof_artifacts[0]
        .proof_sha256[0] ^= 1;
    malformed.push(hash_mismatch);

    let mut byte_mutation = canonical_expectations_v1();
    byte_mutation.stages[ordinary_index]
        .evidence
        .proof_artifacts[0]
        .canonical_proof_bytes[0] ^= 1;
    malformed.push(byte_mutation);

    for expectations in malformed {
        assert!(validate_expectations(&expectations).is_err());
    }
}

#[test]
fn hash_refreshed_corrupt_proof_cannot_replace_frozen_production_evidence() {
    let expectations = canonical_expectations_v1();
    let mut measured = measured_from_expectations(&expectations);
    validate_measured_against_expectations(&measured, &expectations).unwrap();

    let stage_index = usize::from(privacy_release_stage_ordinal_v1(
        PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    ));
    let artifact = &mut measured[stage_index].evidence.proof_artifacts[0];
    artifact.canonical_proof_bytes[0] ^= 1;
    refresh_artifact_hash(artifact);
    assert!(
        validate_stage_evidence(
            &measured[stage_index].evidence,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
            stage_index,
        )
        .is_ok(),
        "hash-refreshed bytes are structurally self-consistent"
    );
    assert!(validate_measured_against_expectations(&measured, &expectations).is_err());
}
