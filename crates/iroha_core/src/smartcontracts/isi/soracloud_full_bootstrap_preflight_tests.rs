//! Direct material preflight controls, without bootstrap output or proof construction.
//!
//! The original audited-prover regression still fails during execution-fixture setup at the
//! production-qualification gate (native23: 0 passed, 1 failed). These separate tests exercise
//! its eleven malformed-context cases through the same production preflight helper. Local
//! audit-package fixtures are unqualified test data; an unmutated fixture must still reject.

use super::*;

struct MaterialOnlyFixture {
    params: BfvParameters,
    evaluation_keys: BfvEvaluationKeyBundle,
    transcript: BfvEvaluationKeyRefreshTranscriptV1,
    artifacts: BfvFullBootstrapCircuitArtifactBundleV1,
    release_audit_package: BfvFullBootstrapReleaseAuditPackageV1,
    release_audit_package_digest: Hash,
    reviewer_key_pair: KeyPair,
}

fn material_only_fixture() -> MaterialOnlyFixture {
    let params = ram_lfe_bfv_parameters_v1();
    let mut evaluation_keys = sample_bfv_evaluation_key_bundle();
    let canonical_vk_box = sample_fhe_full_bootstrap_execution_vk_box();
    let (secret_key, _public_key, _relinearization_key) =
        keygen_from_seed(&params, b"soracloud-fhe-test-keygen")
            .expect("sample full-bootstrap artifact keygen");
    let prover_key_material = encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1(
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    )
    .expect("sample native full-bootstrap prover-key material");
    let verifier_key_material =
        sample_full_bootstrap_native_verifier_material_for_core_vk(&canonical_vk_box);
    let artifacts = sample_full_bootstrap_circuit_artifacts_for_secret_and_proof_keys(
        &params,
        &secret_key,
        &prover_key_material,
        &verifier_key_material,
    );
    let material = sample_full_bootstrap_material_for_artifacts(&params, &artifacts);
    let transcript = sample_full_bootstrap_bfv_refresh_transcript();
    install_full_bootstrap_material(
        &mut evaluation_keys,
        &params,
        &transcript.public_key,
        material.clone(),
    );
    add_full_bootstrap_blind_rotation_galois_keys(
        &mut evaluation_keys,
        &params,
        &material,
        &artifacts,
    );
    // Exercise the actual canonical material bytes before the direct preflight calls.
    let key_bytes = norito::encode_canonical(&evaluation_keys)
        .expect("canonical material-only evaluation-key bytes");
    let transcript_bytes = norito::encode_canonical(&transcript)
        .expect("canonical material-only refresh-transcript bytes");
    let decoded_keys: BfvEvaluationKeyBundle =
        norito::decode_canonical(&key_bytes).expect("decode material-only evaluation-key bytes");
    let decoded_transcript: BfvEvaluationKeyRefreshTranscriptV1 =
        norito::decode_canonical(&transcript_bytes)
            .expect("decode material-only refresh-transcript bytes");
    assert_eq!(decoded_keys, evaluation_keys);
    assert_eq!(decoded_transcript, transcript);
    let reviewer_key_pair = checked_keypair();
    // This existing local package is deliberately unqualified, even though its shape is valid.
    let (release_audit_package, release_audit_package_digest) =
        sample_full_bootstrap_release_audit_package_and_digest(
            &params,
            &decoded_keys,
            &artifacts,
            &reviewer_key_pair,
        );
    MaterialOnlyFixture {
        params,
        evaluation_keys: decoded_keys,
        transcript: decoded_transcript,
        artifacts,
        release_audit_package,
        release_audit_package_digest,
        reviewer_key_pair,
    }
}

#[test]
fn unmutated_material_reaches_closed_production_qualification_gate() {
    let fixture = material_only_fixture();
    let error = validate_soracloud_fhe_full_bootstrap_release_audit_package_for_evaluation_keys_v1(
        "FHE full-bootstrap execution proof",
        &fixture.params,
        &fixture.evaluation_keys,
        &fixture.transcript,
        &fixture.artifacts,
        &fixture.release_audit_package,
        fixture.release_audit_package_digest,
        "sora-zk-audit-wg-2026",
        fixture.reviewer_key_pair.public_key(),
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("local material must remain rejected by production qualification");
    assert_invalid_parameter_contains(error.clone(), "release audit package failed validation");
    assert_invalid_parameter_contains(
        error,
        "BFV production qualification unavailable: MissingRegisteredHeOrgLatticeNoiseAndQromEvidence",
    );
}

#[test]
fn malformed_key_context_rejects_before_audit_package_or_execution() {
    let MaterialOnlyFixture {
        params,
        evaluation_keys,
        transcript,
        artifacts,
        release_audit_package,
        release_audit_package_digest,
        reviewer_key_pair,
    } = material_only_fixture();
    let preflight = |keys, refresh_transcript, package, required_refresh_mode| {
        validate_soracloud_fhe_full_bootstrap_release_audit_package_for_evaluation_keys_v1(
            "FHE full-bootstrap execution proof",
            &params,
            keys,
            refresh_transcript,
            &artifacts,
            package,
            release_audit_package_digest,
            "sora-zk-audit-wg-2026",
            reviewer_key_pair.public_key(),
            required_refresh_mode,
        )
    };
    let mut missing_bootstrap_keys = evaluation_keys.clone();
    missing_bootstrap_keys.bootstrap_key = None;
    let err = preflight(
        &missing_bootstrap_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("missing bootstrap key must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "requires bootstrap key material");
    let mut refresh_only_keys = evaluation_keys.clone();
    refresh_only_keys
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .mode = BfvBootstrapKeyMode::RefreshOnlyV1;
    let err = preflight(
        &refresh_only_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("refresh-only bootstrap key must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "requires FullBootstrapV1 bootstrap key material");
    let mut missing_material_keys = evaluation_keys.clone();
    missing_material_keys
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .full_bootstrap_material = None;
    let err = preflight(
        &missing_material_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("missing governed material must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "requires governed full-bootstrap material");
    let mut missing_public_key_digest_keys = evaluation_keys.clone();
    missing_public_key_digest_keys
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .public_key_digest = None;
    let err = preflight(
        &missing_public_key_digest_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("missing governed public-key digest must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "requires governed bootstrap public-key digest");
    let mut stale_public_key_digest_keys = evaluation_keys.clone();
    stale_public_key_digest_keys
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .public_key_digest = Some(Hash::new(b"stale-execution-governed-bootstrap-public-key"));
    let err = preflight(
        &stale_public_key_digest_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("stale governed public-key digest must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "refresh transcript public-key digest");
    let mut malformed_bootstrap_key_entries = evaluation_keys.clone();
    malformed_bootstrap_key_entries
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .max_refresh_rounds = 1;
    let err = preflight(
        &malformed_bootstrap_key_entries,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err(
        "malformed governed bootstrap-key entries must fail before execution proof generation",
    );
    assert_invalid_parameter_contains(err, "max_refresh_rounds");
    let mut malformed_zero_refresh_entries = evaluation_keys.clone();
    malformed_zero_refresh_entries
        .bootstrap_key
        .as_mut()
        .expect("sample carries bootstrap key")
        .zero_refresh
        .c0
        .push(1);
    let err = preflight(&malformed_zero_refresh_entries, &transcript, &release_audit_package, BfvRefreshTranscriptModeV1::ExactLift).expect_err("malformed governed bootstrap-key zero-refresh entries must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "must not carry encrypted-zero zero_refresh material");
    let mut all_zero_seed_transcript = transcript.clone();
    all_zero_seed_transcript.rotation_transcripts[0].seed =
        vec![0; BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES];
    let err = preflight(
        &evaluation_keys,
        &all_zero_seed_transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("all-zero transcript seed must fail before execution proof generation");
    assert_invalid_parameter_contains(
        err.clone(),
        "refresh transcript failed exact-lift validation",
    );
    assert_invalid_parameter_contains(err, "rotation_transcripts.seed");
    let mut stale_package_with_all_zero_transcript = release_audit_package.clone();
    stale_package_with_all_zero_transcript.record_digest =
        Hash::new(b"stale-execution-package-digest-behind-transcript-inventory-error");
    let err =
preflight(&evaluation_keys, &all_zero_seed_transcript, &stale_package_with_all_zero_transcript, BfvRefreshTranscriptModeV1::ExactLift)
            .expect_err(
                "malformed transcript inventory must fail before stale execution release package validation",
            );
    let debug = format!("{err:?}");
    assert_invalid_parameter_contains(
        err.clone(),
        "refresh transcript failed exact-lift validation",
    );
    assert_invalid_parameter_contains(err, "rotation_transcripts.seed");
    assert!(
        !debug.contains("release audit package failed validation"),
        "transcript inventory diagnostics must not be masked by stale package validation: {debug}"
    );
    let mut stale_transcript_body = transcript.clone();
    stale_transcript_body.rotation_transcripts[0]
        .seed
        .extend_from_slice(b"-stale-execution-audited-prover");
    let err = preflight(
        &evaluation_keys,
        &stale_transcript_body,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::ExactLift,
    )
    .expect_err("stale transcript body must fail before execution proof generation");
    assert_invalid_parameter_contains(err, "refresh transcript failed exact-lift validation");
    let err = preflight(
        &evaluation_keys,
        &transcript,
        &release_audit_package,
        BfvRefreshTranscriptModeV1::BoundedNoise,
    )
    .expect_err(
        "bounded-noise execution proof generation must reject exact-lift release transcripts",
    );
    assert_invalid_parameter_contains(err, "refresh transcript failed bounded-noise validation");
}
