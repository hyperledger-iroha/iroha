//! Deterministic BFV arithmetic conformance material, without production qualification.
//!
//! This module exists only in the crypto unit-test crate. It uses crate-private arithmetic
//! implementations, retains all artifact and witness validation, and proves that a locally
//! signed test package cannot satisfy the independently registered production qualification gate.
//! Only canonical two-slot material is exported by the explicit regeneration test; the local
//! signing key and synthetic test package are never written to the fixture.
use super::*;
use crate::KeyPair;
use std::{path::PathBuf, sync::OnceLock};

fn sample_bfv_full_bootstrap_linear_transform_artifact_payload(
    params: &BfvParameters,
    role: BfvFullBootstrapCircuitArtifactRoleV1,
) -> Vec<u8> {
    let transform = BfvFullBootstrapLinearTransformV1 {
        input_slot_count: params.polynomial_degree,
        output_slot_count: params.polynomial_degree,
        diagonals: vec![BfvFullBootstrapLinearTransformDiagonalV1 {
            rotation_steps: 0,
            plaintext: encode_packed_plaintext_slots(
                params,
                &vec![1; usize::from(params.polynomial_degree)],
            )
            .expect("encode identity packed-slot mask"),
        }],
    };
    encode_bfv_full_bootstrap_linear_transform_artifact_v1(params, 1, role, &transform)
        .expect("encode full-bootstrap linear transform artifact")
}
fn sample_bfv_full_bootstrap_artifacts_for_secret(
    params: &BfvParameters,
    secret_key: &BfvSecretKey,
) -> BfvFullBootstrapCircuitArtifactBundleV1 {
    let accumulator = BfvFullBootstrapAccumulatorV1 {
        slot_count: params.polynomial_degree,
        test_vector: encode_packed_plaintext_slots(
            params,
            &vec![1; usize::from(params.polynomial_degree)],
        )
        .expect("encode full-bootstrap accumulator"),
    };
    let accumulator = encode_bfv_full_bootstrap_accumulator_artifact_v1(params, 1, &accumulator)
        .expect("encode accumulator artifact");
    let accumulator_digest = Hash::new(&accumulator);
    let proof_public_input_schema =
        encode_bfv_full_bootstrap_proof_public_input_schema_artifact_v1(
            params,
            1,
            &bfv_full_bootstrap_proof_public_input_schema_v1(),
        )
        .expect("encode proof public-input schema artifact");
    let proof_public_input_schema_digest = Hash::new(&proof_public_input_schema);
    let arithmetic_air_constraint_system =
        encode_bfv_full_bootstrap_arithmetic_air_constraint_system_artifact_v1(
            params,
            1,
            &bfv_full_bootstrap_arithmetic_air_constraint_system_material_v1(),
        )
        .expect("encode arithmetic AIR artifact");
    let coefficient_to_slot_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
        params,
        BfvFullBootstrapCircuitArtifactRoleV1::CoefficientToSlotKey,
    );
    let slot_to_coefficient_key = sample_bfv_full_bootstrap_linear_transform_artifact_payload(
        params,
        BfvFullBootstrapCircuitArtifactRoleV1::SlotToCoefficientKey,
    );
    let blind_rotation_key = bfv_full_bootstrap_blind_rotation_key_for_packed_left_rotation_v1(
        params,
        accumulator_digest,
        1,
    )
    .expect("build blind-rotation key");
    let blind_rotation_key =
        encode_bfv_full_bootstrap_blind_rotation_artifact_v1(params, 1, &blind_rotation_key)
            .expect("encode blind-rotation artifact");
    let sample_extraction = BfvFullBootstrapSampleExtractionV1 {
        source_slot_count: params.polynomial_degree,
        source_ciphertext_component_count: 2,
        extracted_coefficient_index: 0,
        output_ciphertext_component_count: 2,
    };
    let sample_extraction_key = bfv_full_bootstrap_sample_extraction_switch_key_from_seed_v1(
        params,
        secret_key,
        sample_extraction,
        b"zk-stark-bfv-full-bootstrap-sample-switch",
    )
    .expect("build sample-extraction switch key");
    let sample_extraction_key = encode_bfv_full_bootstrap_sample_extraction_switch_key_artifact_v1(
        params,
        1,
        &sample_extraction_key,
    )
    .expect("encode sample-extraction switch key artifact");
    let evaluator_artifact_set_digest = bfv_full_bootstrap_evaluator_artifact_set_digest_v1(
        params,
        1,
        &coefficient_to_slot_key,
        &slot_to_coefficient_key,
        &blind_rotation_key,
        &sample_extraction_key,
        &accumulator,
        &proof_public_input_schema,
        &arithmetic_air_constraint_system,
    )
    .expect("derive evaluator artifact-set digest");
    let prover_key_material = encode_bfv_full_bootstrap_native_stark_fri_prover_key_material_v1(
        BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
    )
    .expect("encode native prover material");
    let verifier_key_material =
        encode_bfv_full_bootstrap_native_stark_fri_verifier_key_material_v1(
            BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
        )
        .expect("encode native verifier material");
    let (prover_key, verifier_key) = bfv_full_bootstrap_proof_key_pair_from_key_material_v1(
        params,
        1,
        proof_public_input_schema_digest,
        evaluator_artifact_set_digest,
        &prover_key_material,
        &verifier_key_material,
    )
    .expect("build native proof-key pair");
    let prover_key = encode_bfv_full_bootstrap_proof_key_artifact_v1(
        params,
        1,
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        &prover_key,
    )
    .expect("encode prover-key artifact");
    let verifier_key = encode_bfv_full_bootstrap_proof_key_artifact_v1(
        params,
        1,
        BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
        &verifier_key,
    )
    .expect("encode verifier-key artifact");
    BfvFullBootstrapCircuitArtifactBundleV1 {
        coefficient_to_slot_key,
        slot_to_coefficient_key,
        blind_rotation_key,
        sample_extraction_key,
        accumulator,
        proof_public_input_schema,
        arithmetic_air_constraint_system,
        prover_key,
        verifier_key,
    }
}
fn local_signed_test_package_and_digest(
    params: &BfvParameters,
    material: &BfvFullBootstrapCircuitMaterialV1,
    artifacts: &BfvFullBootstrapCircuitArtifactBundleV1,
    reviewer_key_pair: &KeyPair,
) -> (BfvFullBootstrapReleaseAuditPackageV1, Hash) {
    let (generated_report_bytes, generated_archive_bytes) =
        bfv_full_bootstrap_release_audit_report_and_archive_bytes_for_artifacts_v1(
            params, material, artifacts,
        )
        .expect("sample full-bootstrap release audit generated report/archive bytes");
    let generated_report_body = generated_report_bytes
        .strip_prefix(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_REPORT_HEADER_V1)
        .expect("generated report bytes carry canonical header");
    let generated_archive_body = generated_archive_bytes
        .strip_prefix(BFV_FULL_BOOTSTRAP_RELEASE_AUDIT_ARCHIVE_HEADER_V1)
        .expect("generated archive bytes carry canonical header");
    let report_suffix = generated_report_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release audit report inventory v1")
        .expect("generated report body carries deterministic inventory prefix");
    let archive_suffix = generated_archive_body
        .strip_prefix(b"machine-generated BFV full-bootstrap release evidence archive inventory v1")
        .expect("generated archive body carries deterministic inventory prefix");
    let report_body = [
            b"external-review-approved: reviewer-id=local-conformance-signer-2026 BFV full-bootstrap rejection-test report v1"
                .as_slice(),
            report_suffix,
        ]
        .concat();
    let archive_body = [
            b"external-review-evidence-archive: reviewer-id=local-conformance-signer-2026 BFV full-bootstrap rejection-test archive v1"
                .as_slice(),
            archive_suffix,
        ]
        .concat();
    let report_bytes = bfv_full_bootstrap_release_audit_report_bytes_v1(&report_body)
        .expect("sample external-review report bytes");
    let archive_bytes = bfv_full_bootstrap_release_audit_archive_bytes_v1(&archive_body)
        .expect("sample external-review archive bytes");
    bfv_full_bootstrap_release_audit_external_review_package_and_digest_v1(
        params,
        material,
        artifacts,
        &report_bytes,
        &archive_bytes,
        "local-conformance-signer-2026",
        reviewer_key_pair.private_key(),
    )
    .expect("sample external-review full-bootstrap release audit package and digest")
}
fn build_conformance_materials() -> [BfvFullBootstrapExecutionProverInputMaterialV1; 2] {
    let params = ram_lfe_bfv_parameters_v1();
    let (secret_key, public_key, _relinearization_key) =
        keygen_from_seed(&params, b"zk-stark-bfv-full-bootstrap-keygen").expect("BFV keygen");
    let artifacts = sample_bfv_full_bootstrap_artifacts_for_secret(&params, &secret_key);
    let material = bfv_full_bootstrap_circuit_material_from_artifacts_v1(&params, 1, &artifacts)
        .expect("derive governed full-bootstrap material");
    let blind_rotation = decode_bfv_full_bootstrap_blind_rotation_artifact_v1(
        &params,
        &material,
        &artifacts.blind_rotation_key,
    )
    .expect("decode blind-rotation artifact");
    let bootstrap_key = full_bootstrap_key_from_material_v1(
        &params,
        &public_key,
        "zk-stark-bfv-full-bootstrap-refresh-key",
        material.clone(),
    )
    .expect("full-bootstrap key");
    let plaintext = encode_packed_plaintext_slots(
        &params,
        &(0..usize::from(params.polynomial_degree))
            .map(|slot| u64::try_from((slot * 13 + 11) % 257).expect("slot fits"))
            .collect::<Vec<_>>(),
    )
    .expect("encode packed BFV plaintext");
    let input = encrypt_from_seed(
        &params,
        &public_key,
        &plaintext,
        b"zk-stark-bfv-full-bootstrap-input",
    )
    .expect("encrypt BFV input");
    let galois_keys = blind_rotation
        .steps
        .iter()
        .map(|step| {
            galois_key_from_seed(
                &params,
                &secret_key,
                step.automorphism_power,
                b"zk-stark-bfv-full-bootstrap-galois",
            )
            .expect("Galois key")
        })
        .collect::<Vec<_>>();
    let reviewer_key_pair = KeyPair::try_from_seed(vec![0xC3; 32], Algorithm::Ed25519)
        .expect("fixture seed derives release reviewer keypair");
    let (release_audit_package, release_audit_package_digest) =
        local_signed_test_package_and_digest(&params, &material, &artifacts, &reviewer_key_pair);
    let input_bound =
        bfv_encrypted_zero_refresh_residual_multiple_bound(&params).expect("input residual bound");
    let missing_qualification = BfvError::ProductionQualificationUnavailable(
        BfvProductionQualificationBlockerV1::MissingRegisteredHeOrgLatticeNoiseAndQromEvidence,
    );
    assert_eq!(
        full_bootstrap_ciphertext_with_release_audited_artifacts_registered_rns_exact_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            &input,
            &release_audit_package,
            release_audit_package_digest,
            "local-conformance-signer-2026",
            reviewer_key_pair.public_key(),
        ),
        Err(missing_qualification.clone()),
        "a locally signed test package cannot qualify production execution",
    );
    assert_eq!(
        bfv_full_bootstrap_with_release_audited_artifacts_output_residual_multiple_bound_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            input_bound,
            &release_audit_package,
            release_audit_package_digest,
            "local-conformance-signer-2026",
            reviewer_key_pair.public_key(),
        ),
        Err(missing_qualification),
        "a locally signed test package cannot qualify a production noise bound",
    );
    let output = full_bootstrap_ciphertext_with_artifacts_registered_rns_exact_v1(
        &params,
        &bootstrap_key,
        &artifacts,
        &galois_keys,
        &input,
    )
    .expect("crate-private full-bootstrap arithmetic output");
    let output_bound = bfv_full_bootstrap_with_artifacts_output_residual_multiple_bound_v1(
        &params,
        &bootstrap_key,
        &artifacts,
        &galois_keys,
        input_bound,
    )
    .expect("crate-private full-bootstrap arithmetic bound");
    let prover_key = decode_bfv_full_bootstrap_proof_key_artifact_v1(
        &params,
        &material,
        BfvFullBootstrapCircuitArtifactRoleV1::ProverKey,
        &artifacts.prover_key,
    )
    .expect("decode prover key artifact");
    let verifier_key = decode_bfv_full_bootstrap_proof_key_artifact_v1(
        &params,
        &material,
        BfvFullBootstrapCircuitArtifactRoleV1::VerifierKey,
        &artifacts.verifier_key,
    )
    .expect("decode verifier key artifact");
    [0_u32, 1_u32].map(|slot_index| {
        let claim = bfv_full_bootstrap_execution_proof_claim_with_witness_digest_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            slot_index,
            input.clone(),
            output.clone(),
            BfvFullBootstrapExecutionProofBoundModeV1::ExactResidualMultiple,
            input_bound,
            output_bound,
        )
        .expect("derive execution proof claim");
        let witness_material = bfv_full_bootstrap_execution_witness_digest_material_v1(
            &params,
            &bootstrap_key,
            &artifacts,
            &galois_keys,
            &claim,
        )
        .expect("derive execution witness material");
        let proof_input =
            bfv_full_bootstrap_execution_proof_input_material_v1(&public_key, &witness_material)
                .expect("build execution proof input material");
        bfv_full_bootstrap_execution_prover_input_material_v1(
            &proof_input,
            &prover_key,
            &verifier_key,
        )
        .expect("build BFV execution prover input material")
    })
}

fn conformance_materials() -> &'static [BfvFullBootstrapExecutionProverInputMaterialV1; 2] {
    static MATERIALS: OnceLock<[BfvFullBootstrapExecutionProverInputMaterialV1; 2]> =
        OnceLock::new();
    MATERIALS.get_or_init(build_conformance_materials)
}

#[test]
fn local_signed_package_cannot_qualify_arithmetic_conformance_material() {
    let materials = conformance_materials();
    for (slot, material) in materials.iter().enumerate() {
        assert_eq!(
            material.proof_input_material.witness_material.slot_index,
            slot as u32
        );
        validate_bfv_full_bootstrap_execution_prover_input_material_v1(material)
            .expect("complete arithmetic relation validates");
    }
    assert_eq!(
        require_ram_lfe_bfv_production_qualification_v1(),
        Err(BfvError::ProductionQualificationUnavailable(
            BfvProductionQualificationBlockerV1::MissingRegisteredHeOrgLatticeNoiseAndQromEvidence,
        )),
    );
}

#[test]
#[ignore = "explicitly regenerates BFV arithmetic conformance material at IROHA_BFV_CONFORMANCE_OUTPUT"]
fn regenerate_arithmetic_conformance_material() {
    let output = PathBuf::from(
        std::env::var_os("IROHA_BFV_CONFORMANCE_OUTPUT")
            .expect("set an absolute conformance output path"),
    );
    assert!(
        output.is_absolute(),
        "conformance output path must be absolute"
    );
    let materials = conformance_materials();
    let bytes = norito::encode_canonical(materials).expect("encode canonical two-slot material");
    // Bound regeneration before filesystem writes; this is a material fixture, never release evidence.
    assert!(
        bytes.len() <= 16 * 1024 * 1024,
        "conformance material exceeds 16 MiB"
    );
    let decoded: [BfvFullBootstrapExecutionProverInputMaterialV1; 2] =
        norito::decode_canonical(&bytes).expect("canonical material roundtrip");
    for (actual, expected) in decoded.iter().zip(materials) {
        validate_bfv_full_bootstrap_execution_prover_input_material_v1(actual)
            .expect("decoded material retains the exact arithmetic relation");
        assert_eq!(actual, expected);
    }
    eprintln!(
        "BFV arithmetic conformance material: {} bytes, 2 slots",
        bytes.len()
    );
    std::fs::write(output, bytes).expect("write canonical arithmetic conformance material");
}
