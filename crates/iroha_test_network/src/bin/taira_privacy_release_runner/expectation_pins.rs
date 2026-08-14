//! Compiled expectation-pin enforcement for the native release runner.
use iroha_core::privacy_release_evidence::{
    privacy_release_expectation_capture_open_v1, privacy_release_expectation_fixture_matches_v1,
};
use super::*;
/// Build the shape-only provisional evidence replaced by native capture.
pub(super) fn empty_expected_evidence(
    protocol_id: PrivacyProtocolIdV1,
    case_kind: PrivacyReleaseCaseKindV1,
) -> PrivacyReleaseStageEvidenceV1 {
    let resources = privacy_release_resource_facts_v1(protocol_id, case_kind)
        .expect("every exact-12 release stage has canonical resource facts");
    let proof_artifacts = (0..privacy_release_proof_artifact_count_v1(protocol_id, case_kind))
        .map(|artifact_ordinal| {
            let canonical_proof_bytes = vec![artifact_ordinal.saturating_add(1)];
            PrivacyReleaseProofArtifactEvidenceV1 {
                artifact_ordinal,
                proof_sha256: sha256_bytes(&canonical_proof_bytes),
                canonical_proof_bytes,
                proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                    protocol_id,
                    case_kind,
                    artifact_ordinal,
                )
                .expect("closed stage artifact has one canonical ceiling"),
            }
        })
        .collect();
    PrivacyReleaseStageEvidenceV1 {
        schema_version: PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1,
        stage_ordinal: privacy_release_stage_ordinal_v1(protocol_id, case_kind),
        protocol_id,
        case_kind,
        protocol_descriptor: privacy_release_protocol_descriptor_v1(protocol_id).to_owned(),
        public_statement_sha256: [0; 32],
        proof_artifacts,
        failure_class: PrivacyReleaseFailureClassV1::NotApplicable,
        resources,
    }
}
/// Reject capture once any capture-owned source pin is populated.
pub(super) fn require_capture_open_v1() -> Result<(), DynError> {
    if privacy_release_expectation_capture_open_v1() {
        return Ok(());
    }
    Err("native release capture is disabled after any capture-owned source pin is populated".into())
}
/// Securely load and cross-codec validate a newly captured expectation pair.
pub(super) fn load_capture_pair_v1(
    norito_path: &Path,
    json_path: &Path,
) -> Result<(PrivacyReleaseExpectationsV1, SecureInputV1, SecureInputV1), DynError> {
    let norito = secure_read(
        norito_path,
        MAX_EXPECTATIONS_NORITO_BYTES,
        "expectations Norito",
    )?;
    let json = secure_read(json_path, MAX_EXPECTATIONS_JSON_BYTES, "expectations JSON")?;
    if norito.identity == json.identity {
        return Err("expectations Norito and JSON projections alias one inode".into());
    }
    let expectations: PrivacyReleaseExpectationsV1 = decode_canonical_norito(
        &norito.bytes,
        MAX_EXPECTATIONS_NORITO_BYTES,
        "expectations Norito",
    )?;
    let json_expectations: PrivacyReleaseExpectationsV1 =
        decode_canonical_json(&json.bytes, "expectations JSON")?;
    if expectations != json_expectations {
        return Err("expectations JSON is not typed-equal to authoritative Norito".into());
    }
    validate_expectations(&expectations)?;
    Ok((expectations, norito, json))
}
/// Securely load, cross-codec validate, and compiled-pin the expectation pair.
pub(super) fn load_pinned_pair_v1(
    norito_path: &Path,
    json_path: &Path,
) -> Result<(PrivacyReleaseExpectationsV1, SecureInputV1, SecureInputV1), DynError> {
    let (expectations, norito, json) = load_capture_pair_v1(norito_path, json_path)?;
    if !privacy_release_expectation_fixture_matches_v1(norito.sha256, json.sha256) {
        return Err(
            "native expectation pair does not match both compiled release fixture pins".into(),
        );
    }
    Ok((expectations, norito, json))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_loader_rejects_fake_nrt_bytes() {
        let directory = tempfile::tempdir().expect("temporary expectation fixture directory");
        let physical_directory = directory
            .path()
            .canonicalize()
            .expect("physical expectation fixture directory");
        let norito_path = physical_directory.join("expectations.norito");
        let json_path = physical_directory.join("expectations.json");
        fs::write(&norito_path, b"NRT0\0not-canonical-norito")
            .expect("write fake expectation Norito");
        fs::write(&json_path, b"{}\n").expect("write expectation JSON");
        let error = load_capture_pair_v1(&norito_path, &json_path)
            .err()
            .expect("fake expectation Norito must reject");
        assert!(error.to_string().contains("bounded canonical Norito"));
    }
}
