//! Release-audit evidence codec assertions and fixture ownership.

use super::*;

pub(super) struct EvidenceFixtures {
    pub(super) delayed_material_placeholder_digest: Hash,
    pub(super) digest: Hash,
}

pub(super) fn check(artifact_fixture: &ArtifactFixture) -> EvidenceFixtures {
    assert_eq_row! { bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, ) .expect("derive release audit evidence from canonical artifact-bundle bytes"), artifact_fixture.evidence, "{}", artifact_fixture.diagnostics.static_context_at(0) };
    let_row! { (decoded_artifacts_from_derivation_bytes, decoded_evidence_from_derivation_bytes) = bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, ) .expect("derive decoded release audit evidence and artifacts from bytes") };
    assert_row! { (decoded_artifacts_from_derivation_bytes) == (artifact_fixture.artifacts) && (decoded_evidence_from_derivation_bytes) == (artifact_fixture.evidence), "{}", artifact_fixture.diagnostics.group_context(1, 2), };
    let_row! { ( decoded_artifacts_from_derivation_digest_bytes, decoded_evidence_from_derivation_digest_bytes, evidence_digest_from_derivation_bytes, ) = bfv_full_bootstrap_release_audit_evidence_and_digest_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, ) .expect("derive decoded release audit evidence, artifacts, and digest from bytes") };
    assert_row! { (decoded_artifacts_from_derivation_digest_bytes) == (artifact_fixture.artifacts) && (decoded_evidence_from_derivation_digest_bytes) == (artifact_fixture.evidence) && (evidence_digest_from_derivation_bytes) == (artifact_fixture.evidence_digest), "{}", artifact_fixture.diagnostics.group_context(3, 3), };
    let_row! { ( decoded_artifacts_from_derivation_digests_bytes, decoded_evidence_from_derivation_digests_bytes, artifact_bundle_digest_from_derivation_bytes, evidence_digest_from_derivation_digests_bytes, ) = bfv_full_bootstrap_release_audit_evidence_and_digests_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, ) .expect("derive decoded release audit evidence, artifacts, and admitted digests from bytes") };
    assert_row! { (decoded_artifacts_from_derivation_digests_bytes) == (artifact_fixture.artifacts) && (decoded_evidence_from_derivation_digests_bytes) == (artifact_fixture.evidence) && (artifact_bundle_digest_from_derivation_bytes) == (artifact_fixture.artifact_bundle_digest) && (evidence_digest_from_derivation_digests_bytes) == (artifact_fixture.evidence_digest), "{}", artifact_fixture.diagnostics.group_context(6, 4), };
    validate_bfv_full_bootstrap_release_audit_evidence_for_artifact_bundle_bytes_v1(
        &artifact_fixture.params,
        &artifact_fixture.material,
        &artifact_fixture.artifact_bundle_bytes,
        &artifact_fixture.evidence,
    )
    .expect("release audit evidence must validate against canonical artifact-bundle bytes");
    assert_row! { (bfv_full_bootstrap_release_audit_evidence_digest_from_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, ) .expect("digest release audit evidence from canonical artifact-bundle bytes")) == (artifact_fixture.evidence_digest) && (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &artifact_fixture.evidence_bytes, ) .expect("canonical release audit evidence bytes validate against artifact-bundle bytes")) == (artifact_fixture.evidence), "{}", artifact_fixture.diagnostics.group_context(10, 2), };
    let_row! { (decoded_artifacts_from_evidence_bytes, decoded_evidence_for_artifact_bytes) = validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &artifact_fixture.evidence_bytes, ) .expect("canonical release audit evidence/artifact bytes validate together") };
    assert_row! { (decoded_artifacts_from_evidence_bytes) == (artifact_fixture.artifacts) && (decoded_evidence_for_artifact_bytes) == (artifact_fixture.evidence), "{}", artifact_fixture.diagnostics.group_context(12, 2), };
    let_row! { ( decoded_artifacts_from_evidence_bytes_with_digest, decoded_evidence_for_artifact_bytes_with_digest, evidence_digest_for_artifact_bytes, ) = validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &artifact_fixture.evidence_bytes, ) .expect("canonical release audit evidence/artifact bytes validate with digest") };
    assert_row! { (decoded_artifacts_from_evidence_bytes_with_digest) == (artifact_fixture.artifacts) && (decoded_evidence_for_artifact_bytes_with_digest) == (artifact_fixture.evidence) && (evidence_digest_for_artifact_bytes) == (artifact_fixture.evidence_digest), "{}", artifact_fixture.diagnostics.group_context(14, 3), };
    let_row! { ( decoded_artifacts_from_evidence_bytes_with_digests, decoded_evidence_for_artifact_bytes_with_digests, artifact_bundle_digest_for_evidence_bytes, evidence_digest_for_artifact_bytes_with_digests, ) = validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &artifact_fixture.evidence_bytes, ) .expect("canonical release audit evidence/artifact bytes validate with admitted digests") };
    assert_row! { (decoded_artifacts_from_evidence_bytes_with_digests) == (artifact_fixture.artifacts) && (decoded_evidence_for_artifact_bytes_with_digests) == (artifact_fixture.evidence) && (artifact_bundle_digest_for_evidence_bytes) == (artifact_fixture.artifact_bundle_digest) && (evidence_digest_for_artifact_bytes_with_digests) == (artifact_fixture.evidence_digest), "{}", artifact_fixture.diagnostics.group_context(17, 4), };
    let_row! { compressed_artifact_bundle_bytes = norito::to_compressed_bytes(&artifact_fixture.artifacts, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit source artifact bundle") };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 21; (bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &artifact_fixture.evidence, )), (bfv_full_bootstrap_release_audit_evidence_digest_from_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_and_digest_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_and_digests_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &compressed_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )) };
    let_row! { compressed_evidence_bytes = norito::to_compressed_bytes(&artifact_fixture.evidence, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit evidence") };
    assert_ne_row! { compressed_evidence_bytes, artifact_fixture.evidence_bytes, "compressed release audit evidence bytes must differ from canonical bytes" };
    assert_eq_row! { decode_trusted_compressed_fixture(&compressed_evidence_bytes, &artifact_fixture.evidence) .expect("compressed release audit evidence bytes decode structurally"), artifact_fixture.evidence, "{}", artifact_fixture.diagnostics.static_context_at(31) };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 32; (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &compressed_evidence_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &compressed_evidence_bytes, )), (validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &compressed_evidence_bytes, )), (validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &compressed_evidence_bytes, )) };
    let_row! { binary_split_placeholder_artifact_bundle_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit artifact bundle".to_vec() };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 36; (bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_and_digest_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, )), (bfv_full_bootstrap_release_audit_evidence_and_digests_from_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )), (validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &binary_split_placeholder_artifact_bundle_bytes, &artifact_fixture.evidence_bytes, )) };
    let_row! { binary_split_placeholder_evidence_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit evidence bytes".to_vec() };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 44; (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &binary_split_placeholder_evidence_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &binary_split_placeholder_evidence_bytes, )), (validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &binary_split_placeholder_evidence_bytes, )), (validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &binary_split_placeholder_evidence_bytes, )) };
    mutation_row! { stale_byte_evidence = artifact_fixture.evidence.clone(); stale_byte_evidence.artifact_bundle_digest = Hash::new(b"stale-release-audit-byte-bound-artifact-bundle-digest"); assert_local_diag! { artifact_fixture.diagnostics; 48 => validate_bfv_full_bootstrap_release_audit_evidence_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &stale_byte_evidence, ) }; };
    let_row! { stale_evidence_bytes = norito::to_bytes(&stale_byte_evidence).expect("encode stale release audit evidence") };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 49; (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &stale_evidence_bytes, )), (validate_bfv_full_bootstrap_release_audit_evidence_bytes_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &stale_evidence_bytes, )), (validate_release_evidence_bytes_and_digest_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &stale_evidence_bytes, )), (validate_release_evidence_bytes_and_digests_for_artifact_bundle_bytes_decoded_v1( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifact_bundle_bytes, &stale_evidence_bytes, )) };
    mutation_row! { downgraded_key_artifacts = artifact_fixture.artifacts.clone(); let_row! { mut downgraded_prover_key = decode_sample_full_bootstrap_proof_key_artifact(&artifact_fixture.artifacts.prover_key) }; downgraded_prover_key.validates_merkle_path_roots = false; let_row! { downgraded_prover_key_payload = norito::to_bytes(&downgraded_prover_key).expect("encode downgraded prover key") }; downgraded_key_artifacts.prover_key = sample_full_bootstrap_artifact_payload( &artifact_fixture.params, BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, &downgraded_prover_key_payload, ); let_row! { downgraded_key_material = BfvFullBootstrapCircuitMaterialV1 { prover_key_digest: Hash::new(&downgraded_key_artifacts.prover_key), prover_key_material_commitment: downgraded_prover_key.key_material_commitment, ..artifact_fixture.material.clone() } }; assert_local_diag! { artifact_fixture.diagnostics; 53 => release_evidence_v1(&artifact_fixture.params, &downgraded_key_material, &downgraded_key_artifacts) }; };
    mutation_row! { placeholder_circuit_id_evidence = artifact_fixture.evidence.clone(); placeholder_circuit_id_evidence.circuit_id = "replace_before_production".to_owned(); assert_local_diag! { artifact_fixture.diagnostics; 54 => validate_release_evidence_v1(&placeholder_circuit_id_evidence) }; };
    mutation_row! { wrong_circuit_id_evidence = artifact_fixture.evidence.clone(); wrong_circuit_id_evidence.circuit_id = "soracloud_fhe_full_bootstrap_material_v1".to_owned(); assert_local_diag! { artifact_fixture.diagnostics; 55 => validate_release_evidence_v1(&wrong_circuit_id_evidence) }; };
    let_row! { registered_profile_placeholder_setters: [ReleaseAuditEvidenceDigestSetter; 4] = [ ("parameter digest", |evidence, digest| { evidence.parameter_digest = digest; }), ("RNS modulus-chain digest", |evidence, digest| { evidence.rns_modulus_chain_digest = digest; }), ( "key-switch decomposition-chain digest", |evidence, digest| { evidence.key_switch_decomposition_chain_digest = digest; }, ), ( "centered scale-round source-chain digest", |evidence, digest| { evidence.centered_scale_round_source_chain_digest = digest; }, ), ] };
    for (label, set_digest) in registered_profile_placeholder_setters {
        let mut placeholder_profile_digest_evidence = artifact_fixture.evidence.clone();
        set_digest(
            &mut placeholder_profile_digest_evidence,
            Hash::new(b"replace before production"),
        );
        assert_call! { assert_error_contains; validate_release_evidence_v1( &placeholder_profile_digest_evidence, ), artifact_fixture.diagnostics.static_expected_at(56), artifact_fixture.diagnostics.dynamic_context_at(56, &format!( "release audit evidence must reject placeholder {label} before registered-profile or cross-field mismatch" )) };
    }
    assert_local_diag_clone_mutations! {
        artifact_fixture.diagnostics;
        case = artifact_fixture.evidence => validate_release_evidence_v1(@);
        57 => case.slot_to_coefficient_key_digest = case.coefficient_to_slot_key_digest;
        58 => case.evaluator_artifact_set_digest = case.artifact_bundle_digest;
        59 => case.evaluator_artifact_set_digest =
            Hash::new(b"stale-release-audit-evaluator-artifact-set-digest");
        60 => case.artifact_bundle_digest =
            Hash::new(b"stale-release-audit-artifact-bundle-digest");
        61 => case.proof_key_pair_commitment =
            Hash::new(b"pending BFV full-bootstrap proof-key pair commitment");
        62 => case.prover_key.key_material_commitment =
            Hash::new(b"placeholder full-bootstrap prover-key commitment");
        63 => case.verifier_key.key_digest = Hash::new(b"not for production");
    }
    let_row! { delayed_material_placeholder_digest = Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_DELAY_PREFIXES[0], b"pending BFV full-bootstrap proof-key pair commitment", ]) };
    assert_local_diag_clone_mutations! {
        artifact_fixture.diagnostics;
        case = artifact_fixture.evidence => validate_release_evidence_v1(@);
        64 => case.artifact_bundle_digest = delayed_material_placeholder_digest;
        65 => case.parameter_digest = Hash::new(b"profile-drift-release-audit-parameter-digest-v1");
    }
    let_row! { registered_profile_drift_setters: [RegisteredProfileDriftSetter; 3] = [ ( "RNS modulus-chain digest", |evidence, digest| { evidence.rns_modulus_chain_digest = digest; }, "does not match registered parameters", ), ( "key-switch decomposition-chain digest", |evidence, digest| { evidence.key_switch_decomposition_chain_digest = digest; }, "does not match registered parameters", ), ( "centered scale-round source-chain digest", |evidence, digest| { evidence.centered_scale_round_source_chain_digest = digest; evidence .proof_profile .centered_scale_round_source_chain_digest = digest; evidence.prover_key.centered_scale_round_source_chain_digest = digest; evidence .verifier_key .centered_scale_round_source_chain_digest = digest; }, "canonical BFV centered scale-round source chain", ), ] };
    for (label, set_digest, expected_error) in registered_profile_drift_setters {
        let mut stale_profile_digest_evidence = artifact_fixture.evidence.clone();
        set_digest(
            &mut stale_profile_digest_evidence,
            Hash::new(format!("profile-drift-release-audit-{label}-v1").as_bytes()),
        );
        assert_call! { assert_error_contains; validate_release_evidence_v1(&stale_profile_digest_evidence), artifact_fixture.diagnostics.dynamic_expected_at(66, expected_error), artifact_fixture.diagnostics.dynamic_context_at( 66, &format!("release audit evidence must reject BFV {label} drift"), ), };
        assert_call! { assert_error_contains; release_evidence_digest_v1(&stale_profile_digest_evidence), artifact_fixture.diagnostics.dynamic_expected_at(67, expected_error), artifact_fixture.diagnostics.dynamic_context_at( 67, &format!("release audit evidence digesting must reject BFV {label} drift"), ), };
    }
    mutation_row! { stale_schema_artifact_digest_evidence = artifact_fixture.evidence.clone(); stale_schema_artifact_digest_evidence.proof_public_input_schema_digest = Hash::new(b"profile-drift-release-audit-schema-artifact-digest-v1"); assert_local_diag! { artifact_fixture.diagnostics; 68 => validate_release_evidence_v1(&stale_schema_artifact_digest_evidence) }; };
    mutation_row! { stale_air_artifact_digest_evidence = artifact_fixture.evidence.clone(); stale_air_artifact_digest_evidence.arithmetic_air_constraint_system_artifact_digest = Hash::new(b"profile-drift-release-audit-air-artifact-digest-v1"); assert_local_diag! { artifact_fixture.diagnostics; 69 => validate_release_evidence_v1(&stale_air_artifact_digest_evidence) }; };
    assert_row! { (artifact_fixture.evidence.version) == (BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_VERSION_V1) && (artifact_fixture.evidence.field_count) == (BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_FIELD_COUNT_V1) && (artifact_fixture.evidence.circuit_id) == (BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1) && (artifact_fixture.evidence.parameter_digest) == (artifact_fixture.material.parameter_digest) && (artifact_fixture.evidence.artifact_bundle_digest) == (circuit_artifact_bundle_digest(&artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts) .expect("artifact bundle digest")) && (artifact_fixture.evidence.evaluator_artifact_set_digest) == (bfv_full_bootstrap_evaluator_artifact_set_digest_from_governed_material_v1(&artifact_fixture.material) .expect("evaluator artifact set digest")) && (artifact_fixture.evidence.proof_public_input_schema_digest) == (Hash::new(&artifact_fixture.artifacts.proof_public_input_schema)) && (artifact_fixture.evidence.proof_public_input_schema_digest) == (canonical_bfv_full_bootstrap_proof_public_input_schema_artifact_digest_v1( &artifact_fixture.params, artifact_fixture.material.max_bootstrap_depth, ) .expect("canonical proof public-input schema artifact digest")) && (artifact_fixture.evidence.arithmetic_trace_profile_digest) == (bfv_full_bootstrap_arithmetic_trace_profile_digest_v1() .expect("canonical arithmetic trace profile digest")) && (artifact_fixture.evidence.arithmetic_air_constraint_system_digest) == (bfv_full_bootstrap_arithmetic_air_constraint_system_digest_v1() .expect("canonical arithmetic AIR digest")) && (artifact_fixture.evidence.arithmetic_air_constraint_system_artifact_digest) == (canonical_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_digest_v1( &artifact_fixture.params, artifact_fixture.material.max_bootstrap_depth, ) .expect("canonical arithmetic AIR artifact digest")) && (artifact_fixture.evidence.prover_key.key_role) == (BfvFullBootstrapCircuitArtifactRoleV1::ProverKey) && (artifact_fixture.evidence.verifier_key.key_role) == (BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey) && (artifact_fixture.evidence.prover_key.key_digest) == (Hash::new(&artifact_fixture.artifacts.prover_key)) && (artifact_fixture.evidence.verifier_key.key_digest) == (Hash::new(&artifact_fixture.artifacts.verifier_key)) && (artifact_fixture.evidence.prover_key.native_payload_kind) == (BFV_FULL_BOOTSTRAP_NATIVE_PROVER_PAYLOAD_KIND_V1) && (artifact_fixture.evidence.verifier_key.native_payload_kind) == (BFV_FULL_BOOTSTRAP_NATIVE_VERIFIER_PAYLOAD_KIND_V1) && (artifact_fixture.evidence.prover_key.native_circuit_fingerprint) == (artifact_fixture.evidence.verifier_key.native_circuit_fingerprint) && (artifact_fixture.evidence.prover_key.generated_circuit_body_digest) == (artifact_fixture.evidence.verifier_key.generated_circuit_body_digest) && (artifact_fixture.evidence.prover_key.generated_circuit_body_digest) == (generated_body_digest_for_test(native_generated_circuit_body_v1(BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1) .expect("canonical generated circuit body"))), "{}", artifact_fixture.diagnostics.group_context(70, 20), };
    mutation_row! { stale_generated_body_digest_evidence = artifact_fixture.evidence.clone(); stale_generated_body_digest_evidence .verifier_key .generated_circuit_body_digest = generated_body_digest_for_test(b"stale-release-audit-generated-circuit-body-digest"); assert_local_diag! { artifact_fixture.diagnostics; 90 => validate_release_evidence_v1(&stale_generated_body_digest_evidence) }; };
    let mut matched_stale_generated_body_digest_evidence = artifact_fixture.evidence.clone();
    let_row! { matched_stale_generated_body_digest = generated_body_digest_for_test(b"stale-release-audit-generated-circuit-body-digest") };
    matched_stale_generated_body_digest_evidence
        .prover_key
        .generated_circuit_body_digest = matched_stale_generated_body_digest;
    matched_stale_generated_body_digest_evidence
        .verifier_key
        .generated_circuit_body_digest = matched_stale_generated_body_digest;
    assert_error_matrix_row! { artifact_fixture.diagnostics; 91; (validate_release_evidence_v1(&matched_stale_generated_body_digest_evidence)), (release_evidence_digest_v1(&matched_stale_generated_body_digest_evidence)) };
    assert_ne_row! { artifact_fixture.evidence.prover_key.native_payload_digest, artifact_fixture.evidence.verifier_key.native_payload_digest, "release audit evidence must keep prover/verifier native payload digests distinct" };
    let mut stale_prover_native_payload_digest_evidence = artifact_fixture.evidence.clone();
    stale_prover_native_payload_digest_evidence
        .prover_key
        .native_payload_digest = native_payload_digest_for_test(
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        b"audited release native prover payload bytes with wrong digest v1",
    );
    assert_error_matrix_row! { artifact_fixture.diagnostics; 93; (validate_release_evidence_v1(&stale_prover_native_payload_digest_evidence)), (release_evidence_digest_v1(&stale_prover_native_payload_digest_evidence)) };
    mutation_row! { stale_verifier_native_payload_digest_evidence = artifact_fixture.evidence.clone(); stale_verifier_native_payload_digest_evidence .verifier_key .native_payload_digest = native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, b"audited release native verifier payload bytes with wrong digest v1"); assert_local_diag! { artifact_fixture.diagnostics; 95 => validate_release_evidence_v1(&stale_verifier_native_payload_digest_evidence) }; };
    assert_row! { (artifact_fixture.evidence.proof_profile.backend) == (BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1) && (artifact_fixture.evidence.proof_profile.queries) == (BFV_FULL_BOOTSTRAP_NATIVE_STARK_FRI_QUERIES_V1) && (artifact_fixture.evidence.proof_profile.air_evaluation_material_version) == (BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_VERSION_V1) && (artifact_fixture.evidence.proof_profile.air_evaluation_material_field_count) == (BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_FIELD_COUNT_V1) && (artifact_fixture.evidence .proof_profile .proof_input_material_digest_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_EXECUTION_PROOF_INPUT_MATERIAL_DIGEST_DOMAIN) && (artifact_fixture.evidence .proof_profile .prover_input_material_digest_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_EXECUTION_PROVER_INPUT_MATERIAL_DIGEST_DOMAIN) && (artifact_fixture.evidence .proof_profile .air_evaluation_material_digest_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_EVALUATION_MATERIAL_DIGEST_DOMAIN) && (artifact_fixture.evidence .proof_profile .arithmetic_trace_material_digest_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_ARITHMETIC_TRACE_MATERIAL_DIGEST_DOMAIN) && (artifact_fixture.evidence .proof_profile .arithmetic_air_constraint_system_digest_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_ARITHMETIC_AIR_CONSTRAINT_SYSTEM_DIGEST_DOMAIN), "{}", artifact_fixture.diagnostics.group_context(96, 9), };
    assert_row! { artifact_fixture.evidence .proof_profile .separates_release_prover_material_domains };
    assert_row! { (artifact_fixture.evidence .proof_profile .proof_key_material_commitment_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_PROOF_KEY_MATERIAL_COMMITMENT_DOMAIN) && (artifact_fixture.evidence .proof_profile .proof_key_pair_commitment_domain .as_slice()) == (BFV_FULL_BOOTSTRAP_PROOF_KEY_PAIR_COMMITMENT_DOMAIN), "{}", artifact_fixture.diagnostics.group_context(105, 2), };
    assert_row! { artifact_fixture.evidence .proof_profile .separates_proof_key_material_and_pair_commitments };
    assert_row! { artifact_fixture.evidence .proof_profile .validates_air_evaluation_material_digest };
    assert_row! { artifact_fixture.evidence .proof_profile .validates_air_evaluation_trace_material_digest };
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .requires_zero_air_composition_values
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .supports_exact_residual_multiple
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .supports_bounded_noise
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .validates_artifact_bound_prover_input
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .rejects_stale_galois_key_set_replay
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .rejects_stale_proof_key_artifacts
    );
    assert_row! { artifact_fixture.evidence .proof_profile .derives_opening_schedule_from_statement_hash };
    assert_row! { artifact_fixture.evidence .proof_profile .derives_opening_schedule_from_trace_material_digest };
    assert_row! { artifact_fixture.evidence .proof_profile .bounds_opening_schedule_rejection_sampling };
    assert_row! { artifact_fixture.evidence .proof_profile .validates_transcript_public_padding_openings };
    assert_row! { artifact_fixture.evidence .proof_profile .requires_verifier_owned_trace_material_digest };
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .validates_merkle_path_shape
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .validates_merkle_path_roots
    );
    assert!(
        artifact_fixture
            .evidence
            .proof_profile
            .validates_fri_query_chain
    );
    assert_row! { artifact_fixture.evidence .proof_profile .binds_first_fri_values_to_opened_air_values };
    assert_row! { artifact_fixture.evidence .proof_profile .binds_fri_queries_to_air_commitment_roots };
    let evidence_bytes =
        norito::to_bytes(&artifact_fixture.evidence).expect("encode audit evidence");
    let_row! { decoded = norito::decode_from_bytes::<BfvFullBootstrapReleaseAuditEvidenceV1>(&evidence_bytes) .expect("decode audit evidence") };
    assert_eq!(
        decoded,
        artifact_fixture.evidence,
        "{}",
        artifact_fixture.diagnostics.static_context_at(107)
    );
    let_row! { safely_decoded_evidence = decode_bfv_full_bootstrap_release_audit_evidence_bytes_v1(&evidence_bytes) .expect("safe canonical release audit evidence decode") };
    assert_eq_row! { safely_decoded_evidence, artifact_fixture.evidence, "{}", artifact_fixture.diagnostics.static_context_at(108) };
    let digest = release_evidence_digest_v1(&artifact_fixture.evidence)
        .expect("release audit evidence digest");
    assert_row! { (digest) == (Hash::new_from_chunks(&[ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_EVIDENCE_DIGEST_DOMAIN, evidence_bytes.as_slice(), ])) && (digest) == (release_evidence_digest_v1(&decoded) .expect("repeat decoded release audit evidence digest")) && (digest) == (bfv_full_bootstrap_release_audit_evidence_digest( &artifact_fixture.params, &artifact_fixture.material, &artifact_fixture.artifacts ) .expect("derive and digest release audit evidence")) && (digest) == (bfv_full_bootstrap_release_audit_evidence_digest_from_bytes_v1( &evidence_bytes ) .expect("canonical release audit evidence byte digest")), "{}", artifact_fixture.diagnostics.group_context(109, 4), };
    let_row! { compressed_evidence_bytes = norito::to_compressed_bytes(&artifact_fixture.evidence, Some(norito::CompressionConfig::default())) .expect("encode compressed release audit evidence") };
    assert_ne_row! { compressed_evidence_bytes, evidence_bytes, "compressed release audit evidence must differ from canonical v1 bytes" };
    let_row! { decoded_compressed_evidence = decode_trusted_compressed_fixture(&compressed_evidence_bytes, &artifact_fixture.evidence) .expect("compressed release audit evidence must decode structurally") };
    assert_eq_row! { decoded_compressed_evidence, artifact_fixture.evidence, "{}", artifact_fixture.diagnostics.static_context_at(113) };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 114; (decode_bfv_full_bootstrap_release_audit_evidence_bytes_v1(&compressed_evidence_bytes)), (bfv_full_bootstrap_release_audit_evidence_digest_from_bytes_v1( &compressed_evidence_bytes, )) };
    let_row! { binary_split_placeholder_evidence_bytes = b"t\xffo\xffd\xffo pending BFV full-bootstrap release audit evidence bytes".to_vec() };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 116; (decode_bfv_full_bootstrap_release_audit_evidence_bytes_v1( &binary_split_placeholder_evidence_bytes, )), (bfv_full_bootstrap_release_audit_evidence_digest_from_bytes_v1( &binary_split_placeholder_evidence_bytes, )) };
    EvidenceFixtures {
        delayed_material_placeholder_digest,
        digest,
    }
}
