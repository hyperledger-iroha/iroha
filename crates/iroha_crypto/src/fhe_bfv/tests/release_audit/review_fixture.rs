//! Release-audit review fixture assertions and fixture ownership.

use super::*;

pub(super) struct ReviewDocuments {
    pub(super) reviewer_key_pair: crate::KeyPair,
    pub(super) release_evidence_digest_hex: String,
    pub(super) generated_circuit_body_digest_hex: String,
    pub(super) generated_circuit_body: Vec<u8>,
    pub(super) generated_circuit_body_byte_length: String,
    pub(super) generated_circuit_body_hex: String,
    pub(super) native_prover_payload: Vec<u8>,
    pub(super) native_prover_payload_hex: String,
    pub(super) prover_native_payload_digest_hex: String,
    pub(super) native_verifier_payload: Vec<u8>,
    pub(super) native_verifier_payload_hex: String,
    pub(super) verifier_native_payload_digest_hex: String,
    pub(super) artifact_bundle_digest_hex: String,
    pub(super) evaluator_artifact_set_digest_hex: String,
    pub(super) centered_source_chain_digest_hex: String,
    pub(super) arithmetic_trace_profile_digest_hex: String,
    pub(super) arithmetic_air_constraint_system_digest_hex: String,
    pub(super) native_circuit_fingerprint_hex: String,
    pub(super) proof_key_pair_commitment_hex: String,
    pub(super) proof_profile_field_count: String,
    pub(super) proof_profile_public_opening_material_version: String,
    pub(super) proof_profile_public_opening_material_field_count: String,
    pub(super) proof_profile_n_log2: String,
    pub(super) proof_profile_blowup_log2: String,
    pub(super) proof_profile_fold_arity: String,
    pub(super) proof_profile_queries: String,
    pub(super) proof_profile_merkle_arity: String,
    pub(super) proof_profile_requires_canonical_base_transcript_label: String,
    pub(super) proof_profile_rejects_suffixed_transcript_label_aliases: String,
    pub(super) proof_profile_requires_verifier_owned_trace_material_digest: String,
    pub(super) proof_profile_validates_transcript_public_opening_material: String,
    pub(super) proof_profile_validates_merkle_path_shape: String,
    pub(super) proof_profile_validates_merkle_path_roots: String,
    pub(super) proof_profile_validates_fri_query_chain: String,
    pub(super) proof_profile_binds_first_fri_values: String,
    pub(super) proof_profile_binds_fri_queries_to_air_roots: String,
    pub(super) proof_profile_fields_fragment: String,
    pub(super) audit_report_bytes: Vec<u8>,
    pub(super) prover_key_digest_hex: String,
    pub(super) verifier_key_digest_hex: String,
    pub(super) proof_public_input_schema_artifact_hex: String,
    pub(super) arithmetic_air_constraint_system_artifact_hex: String,
    pub(super) coefficient_to_slot_key_artifact_hex: String,
    pub(super) slot_to_coefficient_key_artifact_hex: String,
    pub(super) blind_rotation_key_artifact_hex: String,
    pub(super) sample_extraction_key_artifact_hex: String,
    pub(super) accumulator_artifact_hex: String,
    pub(super) prover_key_artifact_hex: String,
    pub(super) verifier_key_artifact_hex: String,
    pub(super) audit_evidence_archive_bytes: Vec<u8>,
    pub(super) audit_report_digest: Hash,
    pub(super) audit_evidence_archive_digest: Hash,
}

pub(super) fn check(
    artifact_fixture: &ArtifactFixture,
    evidence_codec: &EvidenceFixtures,
) -> ReviewDocuments {
    let_row! { reviewer_key_pair = crate::KeyPair::try_from_seed(vec![0xA7; 32], crate::Algorithm::Ed25519) .expect("fixture seed derives reviewer Ed25519 keypair") };
    let release_evidence_digest_hex =
        hex::encode(<[u8; Hash::LENGTH]>::from(evidence_codec.digest));
    let_row! { generated_circuit_body_digest_hex = hex::encode(artifact_fixture.evidence.prover_key.generated_circuit_body_digest.to_le_bytes()) };
    let_row! { generated_circuit_body = native_generated_circuit_body_v1(BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1) .expect("canonical generated circuit body") };
    assert_eq_row! { generated_body_digest_for_test(&generated_circuit_body), artifact_fixture.evidence.prover_key.generated_circuit_body_digest, "{}", artifact_fixture.diagnostics.static_context_at(118) };
    let generated_circuit_body_byte_length = generated_circuit_body.len().to_string();
    let generated_circuit_body_hex = hex::encode(&generated_circuit_body);
    let_row! { native_prover_payload = encode_bfv_full_bootstrap_native_stark_fri_transparent_prover_payload_v1( BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1, ) .expect("canonical native prover payload") };
    assert_eq_row! { native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::ProverKey, &native_prover_payload), artifact_fixture.evidence.prover_key.native_payload_digest, "{}", artifact_fixture.diagnostics.static_context_at(119) };
    let native_prover_payload_hex = hex::encode(&native_prover_payload);
    let_row! { prover_native_payload_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.prover_key.native_payload_digest), )) };
    let_row! { native_verifier_payload = encode_bfv_full_bootstrap_native_stark_fri_verifier_key_payload_v1( BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1, ) .expect("canonical native verifier payload") };
    assert_eq_row! { native_payload_digest_for_test(BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey, &native_verifier_payload), artifact_fixture.evidence.verifier_key.native_payload_digest, "{}", artifact_fixture.diagnostics.static_context_at(120) };
    let native_verifier_payload_hex = hex::encode(&native_verifier_payload);
    let_row! { verifier_native_payload_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.verifier_key.native_payload_digest), )) };
    let_row! { artifact_bundle_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from(artifact_fixture.evidence.artifact_bundle_digest)) };
    let_row! { evaluator_artifact_set_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.evaluator_artifact_set_digest, )) };
    let_row! { centered_source_chain_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.centered_scale_round_source_chain_digest, )) };
    let_row! { arithmetic_trace_profile_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.arithmetic_trace_profile_digest, )) };
    let_row! { arithmetic_air_constraint_system_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.arithmetic_air_constraint_system_digest, )) };
    let_row! { native_circuit_fingerprint_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.prover_key.native_circuit_fingerprint, )) };
    let_row! { proof_key_pair_commitment_hex = hex::encode(<[u8; Hash::LENGTH]>::from( artifact_fixture.evidence.proof_key_pair_commitment, )) };
    let proof_profile_field_count = artifact_fixture
        .evidence
        .proof_profile
        .field_count
        .to_string();
    let_row! { proof_profile_public_opening_material_version = artifact_fixture.evidence .proof_profile .public_opening_material_version .to_string() };
    let_row! { proof_profile_public_opening_material_field_count = artifact_fixture.evidence .proof_profile .public_opening_material_field_count .to_string() };
    let proof_profile_n_log2 = artifact_fixture.evidence.proof_profile.n_log2.to_string();
    let proof_profile_blowup_log2 = artifact_fixture
        .evidence
        .proof_profile
        .blowup_log2
        .to_string();
    let proof_profile_fold_arity = artifact_fixture
        .evidence
        .proof_profile
        .fold_arity
        .to_string();
    let proof_profile_queries = artifact_fixture.evidence.proof_profile.queries.to_string();
    let proof_profile_merkle_arity = artifact_fixture
        .evidence
        .proof_profile
        .merkle_arity
        .to_string();
    let_row! { proof_profile_requires_canonical_base_transcript_label = artifact_fixture.evidence .proof_profile .requires_canonical_base_transcript_label .to_string() };
    let_row! { proof_profile_rejects_suffixed_transcript_label_aliases = artifact_fixture.evidence .proof_profile .rejects_suffixed_transcript_label_aliases .to_string() };
    let_row! { proof_profile_requires_verifier_owned_trace_material_digest = artifact_fixture.evidence .proof_profile .requires_verifier_owned_trace_material_digest .to_string() };
    let_row! { proof_profile_validates_transcript_public_opening_material = artifact_fixture.evidence .proof_profile .validates_transcript_public_opening_material .to_string() };
    let_row! { proof_profile_validates_merkle_path_shape = artifact_fixture.evidence .proof_profile .validates_merkle_path_shape .to_string() };
    let_row! { proof_profile_validates_merkle_path_roots = artifact_fixture.evidence .proof_profile .validates_merkle_path_roots .to_string() };
    let_row! { proof_profile_validates_fri_query_chain = artifact_fixture.evidence.proof_profile.validates_fri_query_chain.to_string() };
    let_row! { proof_profile_binds_first_fri_values = artifact_fixture.evidence .proof_profile .binds_first_fri_values_to_opened_air_values .to_string() };
    let_row! { proof_profile_binds_fri_queries_to_air_roots = artifact_fixture.evidence .proof_profile .binds_fri_queries_to_air_commitment_roots .to_string() };
    let mut proof_profile_fields_fragment = String::new();
    bfv_full_bootstrap_release_audit_append_proof_profile_fields_v1(
        &mut proof_profile_fields_fragment,
        &artifact_fixture.evidence.proof_profile,
    )
    .expect("proof-profile release-audit fixture fields");
    let_row! { audit_report_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1, b"external-review-approved: reviewer-id=sora-zk-audit-wg-2026 independent BFV full-bootstrap release audit report v1; release-evidence-digest=".as_slice(), release_evidence_digest_hex.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), generated_circuit_body_digest_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), proof_key_pair_commitment_hex.as_bytes(), proof_profile_fields_fragment.as_bytes(), ] .concat() };
    let_row! { prover_key_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from(artifact_fixture.evidence.prover_key.key_digest)) };
    let_row! { verifier_key_digest_hex = hex::encode(<[u8; Hash::LENGTH]>::from(artifact_fixture.evidence.verifier_key.key_digest)) };
    let proof_public_input_schema_artifact_hex =
        hex::encode(&artifact_fixture.artifacts.proof_public_input_schema);
    let_row! { arithmetic_air_constraint_system_artifact_hex = hex::encode(&artifact_fixture.artifacts.arithmetic_air_constraint_system) };
    let coefficient_to_slot_key_artifact_hex =
        hex::encode(&artifact_fixture.artifacts.coefficient_to_slot_key);
    let slot_to_coefficient_key_artifact_hex =
        hex::encode(&artifact_fixture.artifacts.slot_to_coefficient_key);
    let blind_rotation_key_artifact_hex =
        hex::encode(&artifact_fixture.artifacts.blind_rotation_key);
    let sample_extraction_key_artifact_hex =
        hex::encode(&artifact_fixture.artifacts.sample_extraction_key);
    let accumulator_artifact_hex = hex::encode(&artifact_fixture.artifacts.accumulator);
    let prover_key_artifact_hex = hex::encode(&artifact_fixture.artifacts.prover_key);
    let verifier_key_artifact_hex = hex::encode(&artifact_fixture.artifacts.verifier_key);
    let_row! { audit_evidence_archive_body = [ b"external-review-evidence-archive: reviewer-id=sora-zk-audit-wg-2026 BFV full-bootstrap prover verifier evidence v1; artifact-bundle-digest=".as_slice(), artifact_bundle_digest_hex.as_bytes(), b"; evaluator-artifact-set-digest=".as_slice(), evaluator_artifact_set_digest_hex.as_bytes(), b"; centered-source-chain-digest=".as_slice(), centered_source_chain_digest_hex.as_bytes(), b"; arithmetic-trace-profile-digest=".as_slice(), arithmetic_trace_profile_digest_hex.as_bytes(), b"; arithmetic-air-constraint-system-digest=".as_slice(), arithmetic_air_constraint_system_digest_hex.as_bytes(), proof_profile_fields_fragment.as_bytes(), b"; generated-circuit-body-digest384=".as_slice(), generated_circuit_body_digest_hex.as_bytes(), b"; generated-circuit-body-byte-length=".as_slice(), generated_circuit_body_byte_length.as_bytes(), b"; generated-circuit-body-hex=".as_slice(), generated_circuit_body_hex.as_bytes(), b"; coefficient-to-slot-key-artifact-hex=".as_slice(), coefficient_to_slot_key_artifact_hex.as_bytes(), b"; slot-to-coefficient-key-artifact-hex=".as_slice(), slot_to_coefficient_key_artifact_hex.as_bytes(), b"; blind-rotation-key-artifact-hex=".as_slice(), blind_rotation_key_artifact_hex.as_bytes(), b"; extraction-key-artifact-hex=".as_slice(), sample_extraction_key_artifact_hex.as_bytes(), b"; accumulator-artifact-hex=".as_slice(), accumulator_artifact_hex.as_bytes(), b"; proof-public-input-schema-artifact-hex=".as_slice(), proof_public_input_schema_artifact_hex.as_bytes(), b"; arithmetic-air-constraint-system-artifact-hex=".as_slice(), arithmetic_air_constraint_system_artifact_hex.as_bytes(), b"; native-prover-payload-hex=".as_slice(), native_prover_payload_hex.as_bytes(), b"; prover-native-payload-digest=".as_slice(), prover_native_payload_digest_hex.as_bytes(), b"; native-verifier-payload-hex=".as_slice(), native_verifier_payload_hex.as_bytes(), b"; verifier-native-payload-digest=".as_slice(), verifier_native_payload_digest_hex.as_bytes(), b"; prover-key-artifact-hex=".as_slice(), prover_key_artifact_hex.as_bytes(), b"; verifier-key-artifact-hex=".as_slice(), verifier_key_artifact_hex.as_bytes(), b"; native-circuit-fingerprint=".as_slice(), native_circuit_fingerprint_hex.as_bytes(), b"; proof-key-pair-commitment=".as_slice(), proof_key_pair_commitment_hex.as_bytes(), b"; prover-key-digest=".as_slice(), prover_key_digest_hex.as_bytes(), b"; verifier-key-digest=".as_slice(), verifier_key_digest_hex.as_bytes(), ] .concat() };
    let_row! { audit_evidence_archive_bytes = [ BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1, audit_evidence_archive_body.as_slice(), ] .concat() };
    let audit_report_digest = Hash::new(&audit_report_bytes);
    let audit_evidence_archive_digest = Hash::new(&audit_evidence_archive_bytes);
    ReviewDocuments {
        reviewer_key_pair,
        release_evidence_digest_hex,
        generated_circuit_body_digest_hex,
        generated_circuit_body,
        generated_circuit_body_byte_length,
        generated_circuit_body_hex,
        native_prover_payload,
        native_prover_payload_hex,
        prover_native_payload_digest_hex,
        native_verifier_payload,
        native_verifier_payload_hex,
        verifier_native_payload_digest_hex,
        artifact_bundle_digest_hex,
        evaluator_artifact_set_digest_hex,
        centered_source_chain_digest_hex,
        arithmetic_trace_profile_digest_hex,
        arithmetic_air_constraint_system_digest_hex,
        native_circuit_fingerprint_hex,
        proof_key_pair_commitment_hex,
        proof_profile_field_count,
        proof_profile_public_opening_material_version,
        proof_profile_public_opening_material_field_count,
        proof_profile_n_log2,
        proof_profile_blowup_log2,
        proof_profile_fold_arity,
        proof_profile_queries,
        proof_profile_merkle_arity,
        proof_profile_requires_canonical_base_transcript_label,
        proof_profile_rejects_suffixed_transcript_label_aliases,
        proof_profile_requires_verifier_owned_trace_material_digest,
        proof_profile_validates_transcript_public_opening_material,
        proof_profile_validates_merkle_path_shape,
        proof_profile_validates_merkle_path_roots,
        proof_profile_validates_fri_query_chain,
        proof_profile_binds_first_fri_values,
        proof_profile_binds_fri_queries_to_air_roots,
        proof_profile_fields_fragment,
        audit_report_bytes,
        prover_key_digest_hex,
        verifier_key_digest_hex,
        proof_public_input_schema_artifact_hex,
        arithmetic_air_constraint_system_artifact_hex,
        coefficient_to_slot_key_artifact_hex,
        slot_to_coefficient_key_artifact_hex,
        blind_rotation_key_artifact_hex,
        sample_extraction_key_artifact_hex,
        accumulator_artifact_hex,
        prover_key_artifact_hex,
        verifier_key_artifact_hex,
        audit_evidence_archive_bytes,
        audit_report_digest,
        audit_evidence_archive_digest,
    }
}
