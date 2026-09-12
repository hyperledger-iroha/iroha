//! Release-audit signoff authority assertions and fixture ownership.

use super::*;

pub(super) struct SignoffFixtures {
    pub(super) secp256k1_reviewer_key_pair: crate::KeyPair,
    pub(super) all_zero_reviewer_private_key: crate::PrivateKey,
    pub(super) signoff: BfvFullBootstrapReleaseAuditSignoffV1,
    pub(super) stale_generated_body_signoff: BfvFullBootstrapReleaseAuditSignoffV1,
    pub(super) all_zero_reviewer_public_key: crate::PublicKey,
}

pub(super) fn check(
    artifact_fixture: &ArtifactFixture,
    evidence_codec: &EvidenceFixtures,
    review_fixture: &ReviewDocuments,
) -> SignoffFixtures {
    let_row! { secp256k1_reviewer_key_pair = crate::KeyPair::try_from_seed(vec![0xA9; 32], crate::Algorithm::Secp256k1) .expect("fixture seed derives non-Ed25519 reviewer keypair") };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 121; (release_signoff_payload_v1( &artifact_fixture.evidence, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", secp256k1_reviewer_key_pair.public_key(), )), (sign_bfv_full_bootstrap_release_audit_signoff_v1( &artifact_fixture.evidence, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", secp256k1_reviewer_key_pair.private_key(), )) };
    let_row! { all_zero_reviewer_private_key_parse_err = crate::PrivateKey::from_bytes(crate::Algorithm::Ed25519, &[0_u8; 32]) .expect_err("public Ed25519 private-key parser must reject all-zero seed material") };
    assert_row! { all_zero_reviewer_private_key_parse_err .to_string() .contains("all zero"), "unexpected all-zero reviewer private-key parse error: {all_zero_reviewer_private_key_parse_err}" };
    let_row! { all_zero_reviewer_private_key = { let zero_seed = [0_u8; 32]; let signing_key = crate::signature::ed25519::PrivateKey::from_bytes(&zero_seed); crate::PrivateKey(Box::new(crate::secrecy::Secret::new( crate::PrivateKeyInner::Ed25519(signing_key), ))) } };
    assert_local_diag! { artifact_fixture.diagnostics; 123 => sign_bfv_full_bootstrap_release_audit_signoff_v1( &artifact_fixture.evidence, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", &all_zero_reviewer_private_key, ) };
    let_row! { signoff = sign_bfv_full_bootstrap_release_audit_signoff_v1( &artifact_fixture.evidence, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, "sora-zk-audit-wg-2026", review_fixture.reviewer_key_pair.private_key(), ) .expect("sign release audit evidence") };
    validate_release_signoff_v1(&signoff).expect("signed release audit signoff validates");
    validate_bfv_full_bootstrap_release_audit_signoff_for_evidence_v1(
        &signoff,
        &artifact_fixture.evidence,
    )
    .expect("signed release audit signoff matches evidence");
    mutation_row! { placeholder_circuit_id_signoff = signoff.clone(); placeholder_circuit_id_signoff.payload.circuit_id = "not-production-ready".to_owned(); assert_local_diag! { artifact_fixture.diagnostics; 124 => validate_release_signoff_v1(&placeholder_circuit_id_signoff) }; };
    assert_row! { (signoff.payload.generated_circuit_body_digest) == (bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.prover_key.generated_circuit_body_digest)) && (signoff.payload.centered_scale_round_source_chain_digest) == (artifact_fixture.evidence.centered_scale_round_source_chain_digest) && (signoff.payload.generated_circuit_body_digest) == (bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.verifier_key.generated_circuit_body_digest)) && (signoff.payload.prover_native_payload_digest) == (bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.prover_key.native_payload_digest)) && (signoff.payload.verifier_native_payload_digest) == (bfv_native_stark_digest_binding_hash_v1(artifact_fixture.evidence.verifier_key.native_payload_digest)), "{}", artifact_fixture.diagnostics.group_context(125, 5), };
    mutation_row! { stale_source_chain_signoff_payload = signoff.payload.clone(); stale_source_chain_signoff_payload.centered_scale_round_source_chain_digest = Hash::new(b"stale-release-audit-signoff-centered-source-chain-digest"); assert_local_diag! { artifact_fixture.diagnostics; 130 => validate_bfv_full_bootstrap_release_audit_signoff_payload_for_evidence_v1( &stale_source_chain_signoff_payload, &artifact_fixture.evidence, ) }; };
    let_row! { stale_source_chain_signature = SignatureOf::try_new( review_fixture.reviewer_key_pair.private_key(), &stale_source_chain_signoff_payload, ) .expect("fixture reviewer signs stale source-chain signoff payload") };
    let_row! { stale_source_chain_signoff = BfvFullBootstrapReleaseAuditSignoffV1 { payload: stale_source_chain_signoff_payload, signature: stale_source_chain_signature, ..signoff.clone() } };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 131; (validate_release_signoff_v1(&stale_source_chain_signoff)), (validate_bfv_full_bootstrap_release_audit_signoff_for_evidence_v1( &stale_source_chain_signoff, &artifact_fixture.evidence, )) };
    mutation_row! { stale_generated_body_signoff_payload = signoff.payload.clone(); stale_generated_body_signoff_payload.generated_circuit_body_digest = Hash::new(b"stale-release-audit-signoff-generated-circuit-body-digest"); assert_local_diag! { artifact_fixture.diagnostics; 133 => validate_bfv_full_bootstrap_release_audit_signoff_payload_for_evidence_v1( &stale_generated_body_signoff_payload, &artifact_fixture.evidence, ) }; };
    let_row! { stale_generated_body_signature = SignatureOf::try_new( review_fixture.reviewer_key_pair.private_key(), &stale_generated_body_signoff_payload, ) .expect("fixture reviewer signs stale generated-body signoff payload") };
    let_row! { stale_generated_body_signoff = BfvFullBootstrapReleaseAuditSignoffV1 { payload: stale_generated_body_signoff_payload, signature: stale_generated_body_signature, ..signoff.clone() } };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 134; (validate_release_signoff_v1(&stale_generated_body_signoff)), (validate_bfv_full_bootstrap_release_audit_signoff_for_evidence_v1( &stale_generated_body_signoff, &artifact_fixture.evidence, )) };
    mutation_row! { stale_native_payload_digest_signoff_payload = signoff.payload.clone(); stale_native_payload_digest_signoff_payload.prover_native_payload_digest = Hash::new(b"stale-release-audit-signoff-prover-native-payload-digest"); assert_local_diag! { artifact_fixture.diagnostics; 136 => validate_bfv_full_bootstrap_release_audit_signoff_payload_for_evidence_v1( &stale_native_payload_digest_signoff_payload, &artifact_fixture.evidence, ) }; };
    let_row! { stale_native_payload_digest_signature = SignatureOf::try_new( review_fixture.reviewer_key_pair.private_key(), &stale_native_payload_digest_signoff_payload, ) .expect("fixture reviewer signs stale native payload digest signoff payload") };
    let_row! { stale_native_payload_digest_signoff = BfvFullBootstrapReleaseAuditSignoffV1 { payload: stale_native_payload_digest_signoff_payload, signature: stale_native_payload_digest_signature, ..signoff.clone() } };
    assert_local_diag! { artifact_fixture.diagnostics; 137 => validate_bfv_full_bootstrap_release_audit_signoff_for_evidence_v1( &stale_native_payload_digest_signoff, &artifact_fixture.evidence, ) };
    validate_bfv_full_bootstrap_release_audit_signoff_for_artifacts_v1(
        &artifact_fixture.params,
        &artifact_fixture.material,
        &artifact_fixture.artifacts,
        &signoff,
    )
    .expect("signed release audit signoff matches governed artifacts");
    let_row! { all_zero_reviewer_public_key = crate::PublicKey(crate::PublicKeyCompact::new( crate::Algorithm::Ed25519, &[0_u8; 32], )) };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 138; (validate_bfv_full_bootstrap_release_audit_trusted_reviewer_inputs_v1( "sora-zk-audit-wg-2026", &all_zero_reviewer_public_key, )), (validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1( &all_zero_reviewer_public_key, )) };
    mutation_row! { all_zero_reviewer_signoff = signoff.clone(); all_zero_reviewer_signoff.payload.reviewer_public_key = all_zero_reviewer_public_key.clone(); assert_local_diag! { artifact_fixture.diagnostics; 140 => validate_release_signoff_v1(&all_zero_reviewer_signoff) }; };
    let_row! { empty_reviewer_public_key = crate::PublicKey(crate::PublicKeyCompact::new(crate::Algorithm::Ed25519, &[])) };
    assert_error_matrix_row! { artifact_fixture.diagnostics; 141; (validate_bfv_full_bootstrap_release_audit_trusted_reviewer_inputs_v1( "sora-zk-audit-wg-2026", &empty_reviewer_public_key, )), (validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1( &empty_reviewer_public_key, )) };
    for placeholder_reviewer_id in [
        "draft-audit-wg-2026",
        "sora-fake-auditor-2026",
        "todo-audit-reviewer",
        "pending-audit-reviewer",
        "not-production-ready-reviewer",
        "sample-audit-reviewer",
        "template-audit-reviewer",
        "example-audit-reviewer",
        "s-a-m-p-l-e-audit-reviewer",
        "t.e.m.p.l.a.t.e-audit-reviewer",
        "e_x_a_m_p_l_e-audit-reviewer",
        "p-l-a-c-e-h-o-l-d-e-r-audit-reviewer",
        "n-o-t-p-r-o-d-u-c-t-i-o-n-r-e-a-d-y-reviewer",
        "re\0place_before_production-reviewer",
        " re\0place_before_production-reviewer",
        "mock-audit-reviewer",
        "fixture-audit-reviewer",
    ] {
        let_row! { context = format!( "release audit trusted reviewer inputs must reject placeholder reviewer id `{placeholder_reviewer_id}`" ) };
        assert_call! { assert_error_contains; validate_bfv_full_bootstrap_release_audit_trusted_reviewer_inputs_v1( placeholder_reviewer_id, review_fixture.reviewer_key_pair.public_key(), ), artifact_fixture.diagnostics.static_expected_at(143), artifact_fixture.diagnostics.dynamic_context_at(143, &context), };
        let_row! { context = format!( "public trusted reviewer id preflight must reject placeholder reviewer id `{placeholder_reviewer_id}`" ) };
        assert_call! { assert_error_contains; validate_bfv_full_bootstrap_release_audit_trusted_reviewer_id_v1( placeholder_reviewer_id, ), artifact_fixture.diagnostics.static_expected_at(144), artifact_fixture.diagnostics.dynamic_context_at(144, &context), };
        let_row! { context = format!( "release audit signoff construction must reject placeholder reviewer id `{placeholder_reviewer_id}` before signing" ) };
        assert_call! { assert_error_contains; sign_bfv_full_bootstrap_release_audit_signoff_v1( &artifact_fixture.evidence, review_fixture.audit_report_digest, review_fixture.audit_evidence_archive_digest, placeholder_reviewer_id, review_fixture.reviewer_key_pair.private_key(), ), artifact_fixture.diagnostics.static_expected_at(145), artifact_fixture.diagnostics.dynamic_context_at(145, &context), };
    }
    assert_local_diag_clone_mutations! {
        artifact_fixture.diagnostics;
        case = signoff => validate_release_signoff_v1(@);
        146 => case.payload.native_circuit_fingerprint =
            Hash::new(b"stale-release-audit-signoff-native-circuit-fingerprint");
        147 => case.payload.proof_key_pair_commitment =
            Hash::new(b"pending BFV full-bootstrap proof-key pair commitment");
        148 => case.payload.generated_circuit_body_digest = Hash::prehashed([0_u8; Hash::LENGTH]);
    }
    let_row! { signoff_placeholder_digest_setters: [SignoffDigestSetter; 10] = [ ("release evidence digest", |signoff, digest| { signoff.payload.release_audit_evidence_digest = digest; }), ("artifact bundle digest", |signoff, digest| { signoff.payload.artifact_bundle_digest = digest; }), ("evaluator artifact set digest", |signoff, digest| { signoff.payload.evaluator_artifact_set_digest = digest; }), ("proof-key pair commitment", |signoff, digest| { signoff.payload.proof_key_pair_commitment = digest; }), ("prover-key digest", |signoff, digest| { signoff.payload.prover_key_digest = digest; }), ("verifier-key digest", |signoff, digest| { signoff.payload.verifier_key_digest = digest; }), ("native circuit fingerprint", |signoff, digest| { signoff.payload.native_circuit_fingerprint = digest; }), ("generated circuit body digest", |signoff, digest| { signoff.payload.generated_circuit_body_digest = digest; }), ("audit report digest", |signoff, digest| { signoff.payload.audit_report_digest = digest; }), ("evidence archive digest", |signoff, digest| { signoff.payload.audit_evidence_archive_digest = digest; }), ] };
    for (label, set_digest) in signoff_placeholder_digest_setters {
        for placeholder_preimage in BFV_FULL_BOOTSTRAP_PLACEHOLDER_MATERIAL_DIGEST_PREIMAGES {
            let mut placeholder_signoff = signoff.clone();
            set_digest(&mut placeholder_signoff, Hash::new(placeholder_preimage));
            let_row! { context = format!("release audit signoffs must reject placeholder {label} commitments") };
            assert_call! { assert_error_contains; validate_release_signoff_v1(&placeholder_signoff), artifact_fixture.diagnostics.static_expected_at(149), artifact_fixture.diagnostics.dynamic_context_at(149, &context), };
        }
        let mut delayed_placeholder_signoff = signoff.clone();
        set_digest(
            &mut delayed_placeholder_signoff,
            evidence_codec.delayed_material_placeholder_digest,
        );
        let_row! { context = format!("release audit signoffs must reject delayed placeholder {label} commitments") };
        assert_call! { assert_error_contains; validate_release_signoff_v1(&delayed_placeholder_signoff), artifact_fixture.diagnostics.static_expected_at(150), artifact_fixture.diagnostics.dynamic_context_at(150, &context), };
    }
    SignoffFixtures {
        secp256k1_reviewer_key_pair,
        all_zero_reviewer_private_key,
        signoff,
        stale_generated_body_signoff,
        all_zero_reviewer_public_key,
    }
}
