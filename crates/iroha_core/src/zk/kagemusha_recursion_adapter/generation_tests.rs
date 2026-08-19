use super::*;
use halo2_proofs::arithmetic::Field;
use iroha_data_model::offline::{
    KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4, KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
};
use snark_verifier::util::arithmetic::PrimeCurveAffine as _;
use std::{cell::Cell, mem, rc::Rc};
fn encode_with_alternate_norito_layout<T: norito::NoritoSerialize>(value: &T) -> Vec<u8> {
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    norito::to_bytes(value).expect("encode alternate-layout Kagemusha recursion value")
}
fn candidate_step_two_evidence_fixture() -> KagemushaCandidateRecursiveStepTwoEvidenceV4 {
    let candidate_sha256 = [0x01; 32];
    let manifest_sha256 = [0x02; 32];
    let role_digests = [
        [0x11; 32], [0x12; 32], [0x13; 32], [0x14; 32], [0x15; 32], [0x16; 32], [0x17; 32],
        [0x18; 32],
    ];
    let key_set_sha256 = kagemusha_candidate_step_two_key_set_sha256_v4(
        candidate_sha256,
        manifest_sha256,
        role_digests,
    );
    KagemushaCandidateRecursiveStepTwoEvidenceV4 {
        candidate_sha256,
        manifest_sha256,
        step_eq_proving_key_framed_sha256: role_digests[0],
        step_eq_proving_key_payload_sha256: role_digests[1],
        step_eq_verifying_key_framed_sha256: role_digests[2],
        step_eq_verifying_key_payload_sha256: role_digests[3],
        step_ep_proving_key_framed_sha256: role_digests[4],
        step_ep_proving_key_payload_sha256: role_digests[5],
        step_ep_verifying_key_framed_sha256: role_digests[6],
        step_ep_verifying_key_payload_sha256: role_digests[7],
        initialization_key_set_sha256: key_set_sha256,
        append_key_set_sha256: key_set_sha256,
        terminal_key_set_sha256: key_set_sha256,
        initialization_pair_sha256: [0x21; 32],
        initialization_bundle_digest: [0x22; 32],
        append_pair_sha256: [0x23; 32],
        append_bound_parent_bundle_digest: [0x22; 32],
        initialization_proof_step_count: 1,
        initialization_parent_count: 0,
        append_proof_step_count: 2,
        append_parent_count: 1,
        terminal_verified_pair_count: 2,
    }
}
#[test]
fn candidate_step_two_evidence_gate_rejects_shape_key_and_parent_substitution() {
    let evidence = candidate_step_two_evidence_fixture();
    evidence.validate().expect("exact two-step evidence");
    let mut substitutions = Vec::new();
    let mut wrong_init_parent_count = evidence.clone();
    wrong_init_parent_count.initialization_parent_count = 1;
    substitutions.push(wrong_init_parent_count);
    let mut wrong_append_step = evidence.clone();
    wrong_append_step.append_proof_step_count = 3;
    substitutions.push(wrong_append_step);
    let mut wrong_key_set = evidence.clone();
    wrong_key_set.append_key_set_sha256[0] ^= 1;
    substitutions.push(wrong_key_set);
    let mut wrong_parent = evidence.clone();
    wrong_parent.append_bound_parent_bundle_digest[0] ^= 1;
    substitutions.push(wrong_parent);
    let mut incomplete_terminal_verification = evidence;
    incomplete_terminal_verification.terminal_verified_pair_count = 1;
    substitutions.push(incomplete_terminal_verification);
    for substituted in substitutions {
        assert!(substituted.validate().is_err());
    }
}
#[test]
fn source_runtime_heavy_residency_is_strictly_eq_then_ep() {
    let residency = KagemushaSourceRuntimeHeavyResidencyV4::default();
    {
        let _eq = residency
            .enter(KagemushaPastaCycleParityV1::StepEq)
            .expect("enter Eq residency");
        assert!(
            residency
                .enter(KagemushaPastaCycleParityV1::StepEp)
                .is_err(),
            "Ep cannot become resident while Eq is live"
        );
    }
    {
        let _ep = residency
            .enter(KagemushaPastaCycleParityV1::StepEp)
            .expect("enter Ep residency after Eq drops");
    }
    residency.assert_released().expect("all material dropped");
    assert_eq!(
        *residency.events.borrow(),
        vec![
            (KagemushaPastaCycleParityV1::StepEq, true),
            (KagemushaPastaCycleParityV1::StepEq, false),
            (KagemushaPastaCycleParityV1::StepEp, true),
            (KagemushaPastaCycleParityV1::StepEp, false),
        ]
    );
}
#[test]
fn source_runtime_heavy_permit_recovers_after_worker_panic() {
    let panicked = std::panic::catch_unwind(|| {
        let _permit = lock_kagemusha_source_runtime_heavy_v4();
        panic!("source runtime permit poison fixture");
    });
    assert!(panicked.is_err(), "fixture must poison the permit once");
    let _permit = lock_kagemusha_source_runtime_heavy_v4();
    assert!(!KAGEMUSHA_SOURCE_RUNTIME_HEAVY_PERMIT_V4.is_poisoned());
}
fn valid_step_circuit_params_v4() -> KagemushaStepCircuitParamsV4 {
    valid_step_circuit_params_for_k_v4(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4)
}
fn valid_step_circuit_params_for_k_v4(k: u32) -> KagemushaStepCircuitParamsV4 {
    let public_input_limbs = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
        .map(|layout| layout.instance_column_limbs)
        .unwrap_or_else(|_| {
            KagemushaPastaPublicLayoutV4::for_ipa_round_count(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4)
                .expect("production public layout")
                .instance_column_limbs
        });
    KagemushaStepCircuitParamsV4 {
        version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
        k,
        num_advice_per_phase: KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4.to_vec(),
        num_lookup_advice_per_phase: KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4.to_vec(),
        num_fixed: 1,
        lookup_bits: k - 1,
        num_instance_columns: 1,
        public_input_limbs,
        minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
        max_parent_proof_bytes: KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
    }
}
fn first_release_generation_params_v4() -> KagemushaStepCircuitParamsV4 {
    KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()
        .expect("reviewed first-release generation profile")
}
#[test]
fn v5_compact_eq_header_uses_the_authenticated_layout_length() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let relation = source
        .split_once("fn constrain_kagemusha_compact_eq_header_v5(")
        .expect("compact Eq header relation")
        .1
        .split_once("fn constrain_kagemusha_output_frontier_v4")
        .expect("end compact Eq header relation")
        .0;
    assert!(relation.contains("layout: &KagemushaPastaPublicLayoutV4"));
    assert!(relation.contains("usize::try_from(layout.instance_column_limbs)"));
    assert!(relation.contains("compact.len() != expected_compact_len"));
    assert!(!relation.contains("compact.len() != 64"));
}
#[test]
fn v5_lookup_shape_ignores_only_trailing_zero_phases() {
    assert!(kagemusha_lookup_phase_columns_fit_v5(&[1, 0, 0], &[1]));
    assert!(kagemusha_lookup_phase_columns_fit_v5(&[1], &[1, 0, 0]));
    assert!(kagemusha_lookup_phase_columns_fit_v5(&[0, 0, 0], &[]));
    assert!(!kagemusha_lookup_phase_columns_fit_v5(&[2, 0, 0], &[1]));
    assert!(!kagemusha_lookup_phase_columns_fit_v5(&[1, 0, 1], &[1, 1]));
    assert!(!kagemusha_lookup_phase_columns_fit_v5(&[1, 1], &[1, 0, 1]));
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
#[test]
fn v5_k17_shape_probe_profile_tracks_production_constants() {
    let advice = KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4[0];
    let lookup = KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4[0];
    let params = kagemusha_k17_shape_probe_params_v5(advice, lookup);
    assert_eq!(
        params.num_advice_per_phase.as_slice(),
        KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4
    );
    assert_eq!(
        params.num_lookup_advice_per_phase.as_slice(),
        KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4
    );
    let layout = params.validate().expect("production k17 probe profile");
    assert_eq!(layout.accumulator_limbs, 38);
    assert_eq!(layout.live_selector_offset, 65);
    assert_eq!(layout.instance_column_limbs, 66);
}
#[test]
fn v6_k17_audit_inventory_formulas_match_an_independent_fixture() {
    let counts = KagemushaK17AuditCountsV6 {
        sources: 2,
        equations: 2,
        terms: 3,
        stages: 2,
        stage_equations: 2,
        max_terms_per_equation: 2,
        invalid_equation_source_indices: 0,
        protocol_points: 2,
        protocol_source_indices: 2,
        invalid_protocol_source_indices: 0,
    };
    let inventory =
        KagemushaK17AuditInventoryV6::from_counts(counts, 131_063).expect("small exact inventory");
    inventory.validate("fixture").expect("consistent fixture");
    assert_eq!(inventory.audit_poseidon_elements, 23);
    assert_eq!(inventory.audit_poseidon_permutations, 12);
    assert_eq!(inventory.protocol_poseidon_elements, 17);
    assert_eq!(inventory.protocol_poseidon_permutations, 9);
    assert_eq!(inventory.total_non_native_poseidon_permutations, 21);
    assert_eq!(inventory.legacy_v5_raw_audit_bytes, 300);
    assert_eq!(inventory.legacy_v5_raw_audit_sha256_blocks, 5);
    assert_eq!(inventory.legacy_v5_raw_audit_rows_five_lanes, 65_527);
    assert_eq!(inventory.legacy_v5_raw_audit_required_k17_lanes, 1);
    assert_eq!(inventory.compressed_source_audit_bytes, 236);
    assert_eq!(inventory.compressed_source_audit_sha256_blocks, 4);
    assert_eq!(inventory.compressed_source_audit_rows_five_lanes, 65_527);
    assert_eq!(inventory.compressed_source_audit_required_k17_lanes, 1);
    assert_eq!(inventory.legacy_v1_protocol_bytes, 186);
    assert_eq!(inventory.legacy_v1_protocol_sha256_blocks, 4);
    assert_eq!(inventory.legacy_v5_raw_combined_sha256_blocks, 9);
    assert_eq!(inventory.legacy_v5_raw_combined_rows_five_lanes, 65_527);
    assert_eq!(inventory.legacy_v5_raw_combined_required_k17_lanes, 1);
    assert_eq!(inventory.compressed_source_combined_sha256_blocks, 8);
    assert_eq!(inventory.compressed_source_combined_rows_five_lanes, 65_527);
    assert_eq!(inventory.compressed_source_combined_required_k17_lanes, 1);
}
#[test]
fn v6_k17_audit_inventory_exposes_the_authenticated_source_lower_bound() {
    let counts = KagemushaK17AuditCountsV6 {
        sources: 1_867,
        equations: 1,
        terms: 1_867,
        stages: 1,
        stage_equations: 1,
        max_terms_per_equation: 1_867,
        invalid_equation_source_indices: 0,
        protocol_points: 626,
        protocol_source_indices: 626,
        invalid_protocol_source_indices: 0,
    };
    let inventory = KagemushaK17AuditInventoryV6::from_counts(counts, 131_063)
        .expect("authenticated lower-bound inventory");
    assert_eq!(inventory.audit_poseidon_permutations, 3_740);
    assert_eq!(inventory.protocol_poseidon_permutations, 633);
    assert_eq!(inventory.legacy_v5_raw_audit_bytes, 186_755);
    assert_eq!(inventory.legacy_v5_raw_audit_sha256_blocks, 2_919);
    assert_eq!(inventory.legacy_v5_raw_audit_required_k17_lanes, 53);
    assert_eq!(inventory.compressed_source_audit_bytes, 127_011);
    assert_eq!(inventory.compressed_source_audit_sha256_blocks, 1_985);
    assert_eq!(inventory.compressed_source_audit_required_k17_lanes, 36);
    assert_eq!(inventory.legacy_v1_protocol_bytes, 20_154);
    assert_eq!(inventory.legacy_v1_protocol_sha256_blocks, 316);
    assert_eq!(inventory.legacy_v5_raw_combined_sha256_blocks, 3_235);
    assert_eq!(inventory.legacy_v5_raw_combined_required_k17_lanes, 58);
    assert_eq!(inventory.compressed_source_combined_sha256_blocks, 2_301);
    assert_eq!(inventory.compressed_source_combined_required_k17_lanes, 42);
}
#[test]
fn v6_k17_audit_inventory_rejects_inconsistent_or_overflowing_counts() {
    let inconsistent = KagemushaK17AuditCountsV6 {
        sources: 1,
        equations: 2,
        terms: 1,
        stages: 1,
        stage_equations: 1,
        max_terms_per_equation: 1,
        invalid_equation_source_indices: 1,
        protocol_points: 1,
        protocol_source_indices: 0,
        invalid_protocol_source_indices: 1,
    };
    let inventory = KagemushaK17AuditInventoryV6::from_counts(inconsistent, 131_063)
        .expect("inconsistent counts still have measurable geometry");
    assert!(inventory.validate("fixture").is_err());

    let overflow = KagemushaK17AuditCountsV6 {
        sources: u64::MAX,
        ..inconsistent
    };
    assert!(KagemushaK17AuditInventoryV6::from_counts(overflow, 131_063).is_err());
    assert!(KagemushaK17AuditInventoryV6::from_counts(inconsistent, 63).is_err());
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
#[test]
fn v5_k17_shape_capture_requires_the_production_degree() {
    let required = halo2_base::gates::circuit::BaseCircuitParams {
        k: 17,
        num_advice_per_phase: vec![175],
        num_fixed: 1,
        num_lookup_advice_per_phase: vec![19, 0, 0],
        lookup_bits: Some(16),
        num_instance_columns: 1,
    };
    let captured = kagemusha_k17_capture_required_shape_v5("StepEqLive", &required)
        .expect("reviewed populated role shape");
    assert_eq!(captured.k, 17);
    assert_eq!(captured.lookup_bits, Some(16));
    assert_eq!(captured.widths().expect("captured widths"), (175, 19));
    let mut stale = required;
    stale.k = 16;
    stale.lookup_bits = Some(15);
    assert!(
        kagemusha_k17_capture_required_shape_v5("StepEqLive", &stale)
            .expect_err("stale degree must fail closed")
            .contains("unsupported shape")
    );
}
#[test]
fn v5_generator_final_protocol_compile_uses_direct_instances() {
    let config = format!("{:?}", kagemusha_ipa_compile_config_v4(73));
    assert!(config.contains("query_instance: false"));
    assert!(config.contains("num_instance: [73]"));
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let generator = source
        .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
        .expect("artifact generator")
        .1
        .split_once("/// Produce and immediately self-verify one concrete V4 StepEq proof.")
        .expect("end artifact generator")
        .0;
    assert!(
        generator.contains("let compile_config = || kagemusha_ipa_compile_config_v4(public_len);")
    );
    assert_eq!(generator.matches("compile_config()").count(), 2);
    assert!(!generator.contains("Config::ipa().with_num_instance"));
}
#[test]
fn v5_compact_step_count_is_witness_bound_not_fixed() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let header = source
        .split_once("fn constrain_kagemusha_compact_eq_header_v5(")
        .expect("StepEq compact-header relation")
        .1
        .split_once("fn constrain_kagemusha_output_frontier_v4")
        .expect("end StepEq compact-header relation")
        .0;
    let signature = header
        .split_once(") -> Result<(), String>")
        .expect("StepEq compact-header signature")
        .0;
    assert!(
        !signature.contains("proof_step_count"),
        "runtime step data must not enter the circuit through a Rust constant"
    );
    assert_eq!(
        header
            .matches("KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5")
            .count(),
        2,
        "the compact step cell must appear only in the operation copy constraint and range check"
    );
    assert!(header.contains("let operation_step ="));
    assert!(header.contains("ctx.constrain_equal("));
    assert!(header.contains("range.range_check("));
    assert!(
        !header.contains("Fp::from(u64::from(proof_step_count))"),
        "keygen step one must never be assigned into fixed columns"
    );
}
#[test]
fn v5_runtime_prover_retains_raw_vks_and_stages_pk_then_terminal_vks() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let prover_fields = source
        .split_once("pub(crate) struct KagemushaPastaCycleProverV4 {")
        .expect("runtime prover fields")
        .1
        .split_once("impl std::ops::Deref for KagemushaPastaCycleProverV4")
        .expect("end runtime prover fields")
        .0;
    assert!(prover_fields.contains("step_eq_verifying_key_bytes: Vec<u8>"));
    assert!(prover_fields.contains("step_ep_verifying_key_bytes: Vec<u8>"));
    assert!(!prover_fields.contains("step_eq_verifying_key:"));
    assert!(!prover_fields.contains("step_ep_verifying_key:"));
    let prove = source
        .split_once("    fn prove_step_v4(")
        .expect("runtime staged prover")
        .1
        .split_once("/// Circuit-side parent-proof")
        .expect("end runtime staged prover")
        .0;
    let eq_pk = prove
        .find("let step_eq_proving_key =")
        .expect("Eq PK parse");
    let eq_consume = prove[eq_pk..]
        .find("let (step_eq_proof_bytes, step_eq_verifying_key) = prove_step_eq_v4")
        .map(|offset| eq_pk + offset)
        .expect("Eq consuming proof");
    let eq_vk_drop = prove[eq_consume..]
        .find("drop(step_eq_verifying_key)")
        .map(|offset| eq_consume + offset)
        .expect("returned Eq VK drop");
    let ep_circuit = prove[eq_vk_drop..]
        .find("let step_ep = build_kagemusha_step_ep_circuit_v5")
        .map(|offset| eq_vk_drop + offset)
        .expect("Ep circuit after Eq consumption");
    let ep_vk_drop = prove[ep_circuit..]
        .find("drop(step_ep_verifying_key)")
        .map(|offset| ep_circuit + offset)
        .expect("returned Ep VK drop");
    let terminal_vks = prove[ep_vk_drop..]
        .find("let step_eq_terminal_verifying_key =")
        .map(|offset| ep_vk_drop + offset)
        .expect("terminal VK parse after both PKs");
    assert!(!prove.contains("step_eq_proving_key.get_vk()"));
    assert!(!prove.contains("step_ep_proving_key.get_vk()"));
    assert!(eq_pk < eq_consume && eq_consume < eq_vk_drop);
    assert!(eq_vk_drop < ep_circuit && ep_circuit < ep_vk_drop && ep_vk_drop < terminal_vks);
}
#[test]
fn source_qualification_defers_full_proving_key_parse_until_prover_use() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    for (start, end) in [
        (
            "fn qualify_kagemusha_eq_artifacts_v4(",
            "fn qualify_kagemusha_ep_artifacts_v4(",
        ),
        (
            "fn qualify_kagemusha_ep_artifacts_v4(",
            "pub(super) fn qualify_kagemusha_authenticated_artifact_source_v4(",
        ),
    ] {
        let qualification = source
            .split_once(start)
            .expect("parity qualification")
            .1
            .split_once(end)
            .expect("end parity qualification")
            .0;
        assert!(qualification.contains("preflight_kagemusha_pk_from_source_v4("));
        assert!(!qualification.contains("load_kagemusha_eq_proving_key_from_source_v4("));
        assert!(!qualification.contains("load_kagemusha_ep_proving_key_from_source_v4("));
        assert!(!qualification.contains("ProvingKey::read"));
    }
    let eq_runtime = source
        .split_once("fn load_kagemusha_source_eq_prover_material_v4(")
        .expect("Eq source prover loader")
        .1
        .split_once("fn load_kagemusha_source_ep_prover_material_v4(")
        .expect("end Eq source prover loader")
        .0;
    let ep_runtime = source
        .split_once("fn load_kagemusha_source_ep_prover_material_v4(")
        .expect("Ep source prover loader")
        .1
        .split_once("fn load_kagemusha_source_eq_recursion_material_v4(")
        .expect("end Ep source prover loader")
        .0;
    assert!(eq_runtime.contains("load_kagemusha_eq_proving_key_from_qualified_source_v4"));
    assert!(ep_runtime.contains("load_kagemusha_ep_proving_key_from_qualified_source_v4"));
}
#[test]
fn v5_spool_parsers_contain_vendored_halo2_reader_panics() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    for (start, end) in [
        (
            "fn parse_kagemusha_eq_pk_spool_v5(",
            "fn parse_kagemusha_ep_pk_spool_v5(",
        ),
        (
            "fn parse_kagemusha_ep_pk_spool_v5(",
            "pub(crate) struct KagemushaPastaCycleProverV4",
        ),
    ] {
        let parser = source
            .split_once(start)
            .expect("proving-key spool parser")
            .1
            .split_once(end)
            .expect("end proving-key spool parser")
            .0;
        assert!(parser.contains("catch_unwind"));
        assert!(parser.contains("proving-key reader panicked"));
    }
}
#[test]
fn receipt_pk_authentication_is_streaming_and_confined_to_the_catalog_budget() {
    struct TrackingReader {
        bytes: Vec<u8>,
        position: usize,
        largest_request: usize,
    }
    impl std::io::Read for TrackingReader {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            self.largest_request = self.largest_request.max(output.len());
            let available = self.bytes.len().saturating_sub(self.position);
            let count = available.min(output.len());
            output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }
    let prefix_len = 2 * KAGEMUSHA_PK_STREAM_AUTHENTICATION_BUFFER_BYTES_V5 + 17;
    let bytes = (0..prefix_len)
        .map(|index| u8::try_from(index % 251).expect("test byte fits u8"))
        .collect::<Vec<_>>();
    let expected: [u8; 32] = Sha256::digest(&bytes).into();
    let mut reader = TrackingReader {
        bytes: bytes.clone(),
        position: 0,
        largest_request: 0,
    };
    assert_eq!(
        hash_kagemusha_pk_embedded_vk_prefix_v5(
            &mut reader,
            u64::try_from(prefix_len).expect("test prefix length fits u64"),
            "test",
        )
        .expect("hash bounded embedded VK prefix"),
        expected
    );
    assert!(
        reader.largest_request <= KAGEMUSHA_PK_STREAM_AUTHENTICATION_BUFFER_BYTES_V5,
        "receipt PK authentication must never request PK-sized memory"
    );
    let mut truncated = std::io::Cursor::new(&bytes[..bytes.len() - 1]);
    assert!(
        hash_kagemusha_pk_embedded_vk_prefix_v5(
            &mut truncated,
            u64::try_from(prefix_len).expect("test prefix length fits u64"),
            "truncated",
        )
        .is_err(),
        "a truncated embedded VK prefix must fail closed"
    );
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let receipt_verifier = source
        .split_once("pub fn verify_candidate_recursive_step_two_receipt_v4<F>(")
        .expect("receipt verifier")
        .1
        .split_once("struct KagemushaEqBootstrapSeedV4")
        .expect("end receipt verifier")
        .0;
    assert_eq!(
        receipt_verifier
            .matches("authenticate_kagemusha_receipt_pk_spool_v5(")
            .count(),
        2,
        "both PK roles must use the bounded receipt authenticator"
    );
    assert!(!receipt_verifier.contains("from_candidate_artifact_spool_loader("));
    assert!(!receipt_verifier.contains("parse_kagemusha_eq_pk_spool_v5("));
    assert!(!receipt_verifier.contains("parse_kagemusha_ep_pk_spool_v5("));
    assert!(!receipt_verifier.contains("ProvingKey::read"));
    assert_eq!(
        receipt_verifier
            .matches("KagemushaPastaCycleTerminalVerifierV4::from_validated_artifact_loader(")
            .count(),
        1,
        "the six bounded roles must be parsed once"
    );
}
#[test]
fn v5_scalar_audit_prepass_is_witness_only() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let prepass = source
        .split_once("fn collect_kagemusha_scalar_audits_v4<C>(")
        .expect("scalar audit prepass")
        .1
        .split_once("fn scalar_field_parent_count_v4")
        .expect("end scalar audit prepass")
        .0;
    assert!(prepass.contains("BaseCircuitBuilder::<C::ScalarExt>::new(true)"));
    assert!(!prepass.contains("BaseCircuitBuilder::<C::ScalarExt>::new(false)"));
}
#[test]
fn v5_populated_circuits_reuse_the_v1_measurement_graph() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    for (start, end) in [
        (
            "impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqCircuitV4",
            "/// Production StepEp circuit type",
        ),
        (
            "impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpCircuitV4",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        ),
    ] {
        let implementation = source
            .split_once(start)
            .expect("populated circuit implementation")
            .1
            .split_once(end)
            .expect("end populated circuit implementation")
            .0;
        assert!(implementation.contains("type FloorPlanner = halo2_proofs::circuit::V1"));
        assert!(implementation.contains("fn synthesize_for_measurement("));
        assert!(implementation.contains("self.builder.reset_synthesis_state()"));
        assert!(implementation.contains("halo2_proofs::release_allocator_slack()"));
        assert!(!implementation.contains("SimpleFloorPlanner"));
    }
}
#[test]
fn v5_generator_never_builds_or_retains_both_parity_circuits() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let generator = source
        .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
        .expect("artifact generator")
        .1
        .split_once("/// Produce and immediately self-verify one concrete V4 StepEq proof.")
        .expect("end artifact generator")
        .0;
    assert!(!generator.contains("build_kagemusha_step_circuits_v4("));
    assert!(!generator.contains("build_kagemusha_step_circuits_with_mode_v4("));
    assert!(generator.contains("keygen_vk_consuming_with("));
    assert!(generator.contains("keygen_pk_consuming_with("));
    assert!(!generator.contains("drop(step_eq_live_keygen_circuit)"));
    assert!(!generator.contains("drop(step_ep_live_keygen_circuit)"));
    assert!(generator.contains("drop(step_eq_verifying_key)"));
    assert!(generator.contains("drop(step_ep_verifying_key)"));
    let eq_seed = generator
        .find("let step_eq_seed = kagemusha_eq_bootstrap_seed_v4")
        .expect("Eq seed generation");
    let eq_spool = generator
        .find("let step_eq_parameter_spool = kagemusha_eq_parameters_bytes_v4")
        .expect("compressed Eq parameter spool");
    let eq_drop = generator
        .find("drop(step_eq_params);")
        .expect("Eq parameters released before Ep construction");
    let ep_params = generator
        .find("let step_ep_params = ParamsIPA::<EpAffine>::new")
        .expect("Ep parameter construction");
    let ep_seed = generator
        .find("let step_ep_seed = kagemusha_ep_bootstrap_seed_v4")
        .expect("Ep seed generation");
    let eq_reparse = generator
        .find("let step_eq_params = parse_kagemusha_params_v4::<EqAffine>")
        .expect("Eq parameter reconstruction");
    assert!(
        eq_seed < eq_spool
            && eq_spool < eq_drop
            && eq_drop < ep_params
            && ep_params < ep_seed
            && ep_seed < eq_reparse
    );
    let eq_stream = generator
        .find("failed to stream Kagemusha V5 Eq processed proving key")
        .expect("Eq PK stream");
    let ep_live = generator[eq_stream..]
        .find("let step_ep_live_circuit = build_kagemusha_step_ep_circuit_v5")
        .map(|offset| eq_stream + offset)
        .expect("Ep live circuit after Eq PK consumption");
    assert!(eq_stream < ep_live);
}
#[test]
fn v5_generator_uses_one_disposable_rayon_worker() {
    assert_eq!(KAGEMUSHA_GENERATION_RAYON_THREADS_V5, 1);
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let wrapper = source
        .split_once("pub fn generate_kagemusha_pasta_cycle_artifacts_v4(")
        .expect("public artifact generator")
        .1
        .split_once("fn generate_kagemusha_pasta_cycle_artifacts_in_pool_v5(")
        .expect("bounded generator body")
        .0;
    assert!(wrapper.contains(".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)"));
    assert!(wrapper.contains("pool.install(move ||"));
}
#[test]
fn v5_shape_probe_uses_one_disposable_rayon_worker() {
    assert_eq!(KAGEMUSHA_GENERATION_RAYON_THREADS_V5, 1);
    let source = include_str!("k17_probe.rs");
    let wrapper = source
        .split_once("pub fn run_kagemusha_k17_shape_probe_v5(")
        .expect("public populated-shape probe")
        .1
        .split_once("fn run_kagemusha_k17_shape_probe_in_pool_v5(")
        .expect("bounded populated-shape probe body")
        .0;
    assert!(wrapper.contains(".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)"));
    assert!(wrapper.contains("pool.install(move ||"));
    let body = source
        .split_once("fn run_kagemusha_k17_shape_probe_in_pool_v5(")
        .expect("bounded populated-shape probe body")
        .1;
    assert!(body.contains("KagemushaK17ShapeProbeScopeV5::enter()"));
    assert!(
        body.matches("halo2_proofs::release_allocator_slack();")
            .count()
            >= 2
    );
}
#[test]
fn v5_shape_probe_releases_setup_slack_before_the_first_populated_circuit() {
    let source = include_str!("k17_probe.rs");
    let iteration = source
        .split_once("fn kagemusha_k17_shape_probe_iteration_v5(")
        .expect("populated-shape probe iteration")
        .1
        .split_once("pub fn run_kagemusha_k17_shape_probe_v5(")
        .expect("public probe wrapper after iteration")
        .0;
    let release = iteration
        .find("halo2_proofs::release_allocator_slack();")
        .expect("setup allocator-pressure relief");
    let first_build = iteration
        .find("let step_eq = build_kagemusha_step_eq_circuit_v5(")
        .expect("first populated StepEq circuit build");
    assert!(release < first_build);
}
#[test]
fn v6_audit_inventory_probe_exits_before_the_first_populated_circuit() {
    let source = include_str!("k17_probe.rs");
    let iteration = source
        .split_once("fn kagemusha_k17_shape_probe_iteration_v5(")
        .expect("shared k17 diagnostic iteration")
        .1
        .split_once("pub fn run_kagemusha_k17_audit_inventory_probe_v6(")
        .expect("audit-inventory wrapper after iteration")
        .0;
    let inventory_return = iteration
        .find("KagemushaK17ProbeIterationOutcomeV5::AuditInventory")
        .expect("audit-inventory early return");
    let first_build = iteration
        .find("let step_eq = build_kagemusha_step_eq_circuit_v5(")
        .expect("first populated StepEq circuit build");
    assert!(inventory_return < first_build);

    let wrapper = source
        .split_once("pub fn run_kagemusha_k17_audit_inventory_probe_v6(")
        .expect("audit-inventory wrapper")
        .1
        .split_once("pub fn run_kagemusha_k17_shape_probe_v5(")
        .expect("populated-shape wrapper after inventory wrapper")
        .0;
    assert!(wrapper.contains(".num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)"));
    assert!(wrapper.contains("KagemushaK17ShapeProbeScopeV5::enter()"));
    assert!(wrapper.contains("populated_step_circuits=0"));
}
#[test]
fn serialized_v7_is_the_only_operator_visible_audit_candidate() {
    const RETIRED: &str = concat!("probe-compact-k17-", "ipa-audit-bridge");
    let binary = include_str!("../../bin/kagemusha_recursive_spend_v4_memory_benchmark.rs");
    let wrapper = include_str!("../../../../../scripts/run_kagemusha_v4_generation_benchmark.py");
    let facade = include_str!("../kagemusha_v2.rs");
    assert!(!binary.contains(RETIRED));
    assert!(!wrapper.contains(RETIRED));
    assert!(!facade.contains("run_kagemusha_k17_ipa_audit_bridge_probe_v7"));

    let selected = include_str!("serialized_audit_bridge_v7.rs");
    assert!(selected.contains("KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7: bool = false"));
    assert!(selected.contains("if !KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7"));
}
#[test]
fn serialized_v7_native_vector_has_one_reviewed_freeze_seam() {
    let vector = include_str!("serialized_audit_vector_v7.rs");
    let serialized = include_str!("../kagemusha_serialized_audit_v7.rs");
    assert_eq!(
        vector.matches("impl<F: ff::PrimeField> super::kagemusha_serialized_audit_v7::KagemushaNativeAuditVectorSourceV7<F>").count(),
        1,
        "the compiler provenance capability must have one reviewed implementation"
    );
    assert_eq!(
        vector
            .matches("KagemushaFrozenNativeAuditVectorV7::from_reviewed_source(")
            .count(),
        1,
        "the canonical assigned vector builder must own the only production freeze call"
    );
    assert_eq!(
        vector
            .matches("KagemushaReviewedNativeAuditSourceV7(elements)")
            .count(),
        1,
        "the exact pre-audit vector must be frozen directly at the builder tail"
    );
    assert_eq!(
        serialized
            .matches("pub(super) fn from_reviewed_source<S>(")
            .count(),
        1,
        "the frozen-vector constructor must not be duplicated"
    );
}

#[test]
fn serialized_v7_absent_parent_canonicalizes_the_full_parsed_instance() {
    let source = include_str!("serialized_audit_builder_v7.rs");
    let induction = source
        .split_once("fn constrain_kagemusha_serialized_parent_induction_v7")
        .expect("serialized parent-induction function")
        .1
        .split_once("fn constrain_kagemusha_serialized_challenge_and_native_evaluation_v7")
        .expect("end of serialized parent-induction function")
        .0;
    assert!(induction.contains("for value in &parent.instance_cells"));
    assert!(induction.contains("Existing(parent.present), Existing(*value)"));
    assert!(induction.contains("ctx.constrain_equal(&selected, value)"));
    assert!(
        induction
            .find("for value in &parent.instance_cells")
            .expect("full parent-column zeroing")
            < induction
                .find("slots.push(KagemushaAssignedParentSlotV7")
                .expect("transitive parent tuple construction"),
        "all 70 parsed cells must be canonicalized before the digest tuple is assembled"
    );
}

#[test]
fn serialized_v7_atomic_acceptance_terminally_decides_both_openings() {
    let source = include_str!("serialized_audit_builder_v7.rs");
    let verifier = source
        .split_once("fn verify_kagemusha_serialized_atomic_pair_v7")
        .expect("serialized atomic verifier")
        .1
        .split_once("fn create_kagemusha_serialized_eq_proof_v7")
        .expect("end of serialized atomic verifier")
        .0;
    assert_eq!(
        verifier
            .matches("verify_and_decide_eq_accumulation_v4")
            .count(),
        1
    );
    assert_eq!(
        verifier
            .matches("verify_and_decide_ep_accumulation_v4")
            .count(),
        1
    );
    assert_eq!(
        verifier
            .matches("KagemushaIpaAccumulationProofV4::initialization(manifest.k)")
            .count(),
        2
    );
    let challenge_check = verifier
        .find("public challenge is not commitment-derived")
        .expect("commitment-derived challenge check");
    let eq_decision = verifier
        .find("verify_and_decide_eq_accumulation_v4")
        .expect("terminal Eq decision");
    let success = verifier
        .rfind("Ok(KagemushaSerializedVerifiedPairV7")
        .expect("atomic success");
    assert!(challenge_check < eq_decision && eq_decision < success);
}
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
#[test]
fn v5_candidate_spool_identity_rejects_wrong_bindings() {
    let candidate = [0x31; 32];
    let manifest = [0x52; 32];
    validate_kagemusha_candidate_spool_identity_v5(candidate, manifest, candidate, manifest)
        .expect("exact candidate binding");
    assert!(
        validate_kagemusha_candidate_spool_identity_v5(candidate, manifest, [0x32; 32], manifest,)
            .is_err(),
        "a different candidate digest must fail closed"
    );
    assert!(
        validate_kagemusha_candidate_spool_identity_v5(candidate, manifest, candidate, [0x53; 32],)
            .is_err(),
        "a different candidate manifest digest must fail closed"
    );
}
#[test]
fn kagemusha_params_require_the_canonical_transparent_derivation() {
    use halo2_proofs::{
        halo2curves::pasta::EqAffine,
        poly::{
            commitment::{Params as _, ParamsProver as _},
            ipa::commitment::ParamsIPA,
        },
    };
    const TEST_K: u32 = 4;
    let params = ParamsIPA::<EqAffine>::new(TEST_K);
    let mut canonical = Vec::new();
    params
        .write(&mut canonical)
        .expect("serialize canonical transparent parameters");
    let digest: [u8; 32] = Sha256::digest(&canonical).into();
    let derived = canonical_kagemusha_params_for_digest_v4::<EqAffine>(TEST_K, digest, "test")
        .expect("canonical parameter digest");
    assert_eq!(derived.k(), TEST_K);
    let mut substituted_digest = digest;
    substituted_digest[0] ^= 1;
    assert!(
        canonical_kagemusha_params_for_digest_v4::<EqAffine>(TEST_K, substituted_digest, "test",)
            .expect_err("signed but substituted generators must fail")
            .contains("canonical transparent IPA derivation")
    );
    assert!(
        std::panic::catch_unwind(|| {
            canonical_kagemusha_params_for_digest_v4::<EqAffine>(u32::MAX, digest, "test")
        })
        .expect("untrusted degree must be rejected without panicking")
        .expect_err("untrusted degree must fail")
        .contains("exceeds the fixed maximum")
    );
}
#[test]
fn runtime_profile_validation_never_regenerates_a_bootstrap_key() {
    let source = include_str!("../kagemusha_recursion_adapter.rs");
    let runtime_validation = source
        .split_once("fn validate_kagemusha_profile_protocol_v4<C>(")
        .expect("runtime profile validator")
        .1
        .split_once("fn terminal_validate_kagemusha_eq_bootstrap_v4(")
        .expect("end of runtime profile validator")
        .0;
    assert!(!runtime_validation.contains("keygen_vk"));
    assert!(!runtime_validation.contains("kagemusha_bootstrap_verifying_key_v1"));
    assert!(!runtime_validation.contains("validate_bootstrap_protocol"));
    assert!(runtime_validation.contains("kagemusha_compiled_protocol_structure_sha256"));
    assert!(runtime_validation.contains("KagemushaStepBootstrapV4::decode_authenticated"));
}
#[test]
fn v4_halo2_reader_preflight_rejects_untrusted_inner_degrees_and_counts() {
    use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
    let params = valid_step_circuit_params_v4();
    let malicious_degree = u32::MAX.to_le_bytes();
    assert!(
        parse_kagemusha_params_v4::<EqAffine>(&malicious_degree, params.k, "test params")
            .expect_err("untrusted ParamsIPA degree must fail before its reader")
            .contains("does not match authenticated degree")
    );
    let mut malicious_vk = vec![KAGEMUSHA_HALO2_KEY_VERSION_V4];
    malicious_vk.extend_from_slice(&u32::MAX.to_le_bytes());
    malicious_vk.push(KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4);
    malicious_vk.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        parse_kagemusha_eq_vk_v4(&malicious_vk, params.clone())
            .expect_err("untrusted VK degree must fail before its reader")
            .contains("does not match authenticated degree")
    );
    assert!(
        parse_kagemusha_eq_pk_v4(&malicious_vk, params.clone())
            .expect_err("untrusted PK degree must fail before its reader")
            .contains("does not match authenticated degree")
    );
    malicious_vk[1..5].copy_from_slice(&params.k.to_le_bytes());
    let shape = kagemusha_processed_key_shape_v4::<EqAffine>(&params, "test VK")
        .expect("bounded authenticated key shape");
    assert!(
        validate_kagemusha_processed_vk_encoding_v4(&malicious_vk, shape, "test VK")
            .expect_err("untrusted fixed count must fail before the VK reader")
            .contains("fixed-commitment count")
    );
    let reviewed = first_release_generation_params_v4();
    let configured =
        configured_kagemusha_eq_vk_wire_shape_v4(&reviewed).expect("reviewed configured shape");
    let configured_ep =
        configured_kagemusha_ep_vk_wire_shape_v4(&reviewed).expect("reviewed Ep shape");
    assert_eq!(configured, configured_ep);
    assert_eq!(configured.advice_columns, 411);
    assert_eq!(configured.base_fixed_columns, 9);
    assert_eq!(configured.selectors, 330);
    assert_eq!(configured.permutation_columns, 297);
    assert_eq!(configured.instance_columns, 1);
    let reviewed_shape = kagemusha_processed_key_shape_v4::<EqAffine>(&reviewed, "reviewed")
        .expect("reviewed key shape");
    let reviewed_ep_shape = kagemusha_processed_key_shape_v4::<EpAffine>(&reviewed, "reviewed Ep")
        .expect("reviewed Ep key shape");
    assert_eq!(reviewed_shape.domain_rows, 1 << 17);
    assert_eq!(reviewed_shape.fixed_polynomials, 339);
    assert_eq!(reviewed_shape.permutation_polynomials, 297);
    assert_eq!(
        reviewed_shape.fixed_polynomials + reviewed_shape.permutation_polynomials,
        636
    );
    assert_eq!(reviewed_shape.point_bytes, 32);
    assert_eq!(reviewed_shape.scalar_bytes, mem::size_of::<Fp>());
    assert_eq!(reviewed_ep_shape.domain_rows, reviewed_shape.domain_rows);
    assert_eq!(
        reviewed_ep_shape.fixed_polynomials,
        reviewed_shape.fixed_polynomials
    );
    assert_eq!(
        reviewed_ep_shape.permutation_polynomials,
        reviewed_shape.permutation_polynomials
    );
    assert_eq!(reviewed_ep_shape.point_bytes, 32);
    assert_eq!(reviewed_ep_shape.scalar_bytes, mem::size_of::<Fq>());
    assert_eq!(
        reviewed_shape
            .proving_key_bytes("Eq")
            .expect("exact compact V5 PK length"),
        5_347_763_078
    );
    assert_eq!(
        reviewed_ep_shape
            .proving_key_bytes("Ep")
            .expect("exact compact V5 Ep PK length"),
        5_347_763_078
    );
    assert!(
        reviewed_shape
            .proving_key_bytes("Eq")
            .expect("exact compact V5 PK length")
            <= KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
    );
}
#[test]
fn v5_bootstrap_constraint_system_matches_step_for_both_parities() {
    use halo2_proofs::plonk::{Circuit as _, ConstraintSystem};
    let params = first_release_generation_params_v4();
    let mut eq_bootstrap = ConstraintSystem::<Fp>::default();
    KagemushaStepEqProtocolBootstrapCircuitV5::configure_with_params(
        &mut eq_bootstrap,
        params.clone(),
    );
    let mut eq_step = ConstraintSystem::<Fp>::default();
    KagemushaStepEqCircuitV4::configure_with_params(&mut eq_step, params.clone());
    assert_eq!(
        format!("{:?}", eq_bootstrap.pinned()),
        format!("{:?}", eq_step.pinned()),
        "StepEq bootstrap must configure the complete production graph"
    );
    let mut ep_bootstrap = ConstraintSystem::<Fq>::default();
    KagemushaStepEpProtocolBootstrapCircuitV5::configure_with_params(
        &mut ep_bootstrap,
        params.clone(),
    );
    let mut ep_step = ConstraintSystem::<Fq>::default();
    KagemushaStepEpCircuitV4::configure_with_params(&mut ep_step, params);
    assert_eq!(
        format!("{:?}", ep_bootstrap.pinned()),
        format!("{:?}", ep_step.pinned()),
        "StepEp bootstrap must configure the complete production graph"
    );
}
#[test]
fn v5_generation_profile_bounds_cover_configured_augmented_proof() {
    let params = first_release_generation_params_v4();
    let preflight =
        preflight_kagemusha_generation_v4(&params, &params).expect("reviewed V5 preflight");
    assert_eq!(
        preflight.step_eq_proof_size_bytes,
        KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4
    );
    assert_eq!(
        preflight.step_ep_proof_size_bytes,
        KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4
    );
    assert_eq!(
        preflight.max_recursive_pair_bytes,
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4
    );
    assert!(preflight.step_eq_proof_size_bytes < KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4);
    assert!(
        preflight.max_recursive_pair_bytes
            < KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
    );
    let drift = validate_kagemusha_generation_proof_sizes_v5(
        KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4 - 1,
        KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
    )
    .expect_err("computed proof-size drift must fail before allocation");
    assert!(drift.contains("differ from the reviewed"));
    let oversized = validate_kagemusha_generation_proof_sizes_v5(
        KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4 + 1,
        KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
    )
    .expect_err("oversized computed proof must fail before allocation");
    assert!(oversized.contains("absolute Step ceiling"));
    let initialization = KagemushaPastaCycleProofPairV4 {
        version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
        proof_step_count: 1,
        public_inputs: KagemushaCompactPublicInputsV5 {
            common_header: vec![0_u128; KAGEMUSHA_COMPACT_HEADER_WITHOUT_SELECTOR_CELLS_V5 + 1],
            parent_eq_lineage_accumulator: None,
            parent_ep_lineage_accumulator: None,
            parent_eq_deferred_chunks: [[0_u128; 2]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
            parent_ep_deferred_chunks: [[0_u128; 2]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        },
        step_eq_proof_bytes: vec![0_u8; preflight.step_eq_proof_size_bytes as usize],
        step_ep_proof_bytes: vec![0_u8; preflight.step_ep_proof_size_bytes as usize],
        step_eq_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(params.k)
            .expect("Eq initialization marker"),
        step_ep_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(params.k)
            .expect("Ep initialization marker"),
    };
    let initialization_bytes =
        norito::encode_canonical(&initialization).expect("canonical initialization pair");
    assert_eq!(
        u32::try_from(initialization_bytes.len()).expect("initialization length"),
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4
    );
    validate_kagemusha_generation_initialization_pair_bytes_v5(
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4,
        preflight.max_recursive_pair_bytes,
    )
    .expect("reviewed initialization pair");
    let initialization_drift = validate_kagemusha_generation_initialization_pair_bytes_v5(
        KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_INITIALIZATION_BYTES_V4 + 1,
        preflight.max_recursive_pair_bytes,
    )
    .expect_err("initialization pair drift must fail before publication");
    assert!(initialization_drift.contains("differs from the reviewed"));
    assert!(
        u32::try_from(initialization_bytes.len()).expect("initialization length")
            < preflight.max_recursive_pair_bytes
    );
}
#[test]
fn v4_proving_key_preflight_checks_every_polynomial_length_and_vector_count() {
    let shape = KagemushaProcessedKeyShapeV4 {
        k: 0,
        domain_rows: 1,
        fixed_polynomials: 1,
        permutation_polynomials: 1,
        point_bytes: 1,
        scalar_bytes: 1,
    };
    let mut vk_prefix = vec![KAGEMUSHA_HALO2_KEY_VERSION_V4];
    vk_prefix.extend_from_slice(&shape.k.to_le_bytes());
    vk_prefix.push(KAGEMUSHA_HALO2_UNCOMPRESSED_SELECTORS_V4);
    vk_prefix.extend_from_slice(&1_u32.to_le_bytes());
    vk_prefix.extend_from_slice(&[0; 2]);
    let append_polynomial = |bytes: &mut Vec<u8>| {
        bytes.extend_from_slice(&1_u32.to_be_bytes());
        bytes.push(0);
    };
    let mut malicious_polynomial = vk_prefix.clone();
    malicious_polynomial.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        validate_kagemusha_processed_pk_encoding_v4(&malicious_polynomial, shape, "test PK",)
            .expect_err("untrusted polynomial length must fail before the PK reader")
            .contains("l0 polynomial length")
    );
    let mut malicious_fixed_count = vk_prefix.clone();
    for _ in 0..3 {
        append_polynomial(&mut malicious_fixed_count);
    }
    malicious_fixed_count.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(
        validate_kagemusha_processed_pk_encoding_v4(&malicious_fixed_count, shape, "test PK",)
            .expect_err("untrusted fixed-vector count must fail before the PK reader")
            .contains("fixed-value polynomials count")
    );
    let mut malicious_permutation_count = vk_prefix.clone();
    for _ in 0..3 {
        append_polynomial(&mut malicious_permutation_count);
    }
    for _ in 0..2 {
        malicious_permutation_count.extend_from_slice(&1_u32.to_be_bytes());
        append_polynomial(&mut malicious_permutation_count);
    }
    malicious_permutation_count.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(
            validate_kagemusha_processed_pk_encoding_v4(
                &malicious_permutation_count,
                shape,
                "test PK",
            )
            .expect_err("untrusted permutation count must fail before the PK reader")
            .contains("permutation Lagrange polynomials count")
        );
    let mut canonical = malicious_permutation_count;
    canonical.truncate(canonical.len() - 4);
    for _ in 0..2 {
        canonical.extend_from_slice(&1_u32.to_be_bytes());
        append_polynomial(&mut canonical);
    }
    validate_kagemusha_processed_pk_encoding_v4(&canonical, shape, "test PK")
        .expect("complete bounded structural encoding");
}
#[test]
fn v4_role_loader_releases_each_payload_on_success_and_error() {
    struct TrackedPayload {
        bytes: Vec<u8>,
        live: Rc<Cell<bool>>,
        drops: Rc<Cell<u32>>,
    }
    impl KagemushaArtifactPayloadBytesV4 for TrackedPayload {
        fn payload_bytes(&self) -> &[u8] {
            &self.bytes
        }
    }
    impl Drop for TrackedPayload {
        fn drop(&mut self) {
            assert!(self.live.replace(false), "payload must be live before drop");
            self.drops.set(self.drops.get() + 1);
        }
    }
    let live = Rc::new(Cell::new(false));
    let drops = Rc::new(Cell::new(0));
    let mut loads = 0_u32;
    let mut load = |_: KagemushaPastaCycleParityV1, _: KagemushaPastaCycleArtifactKindV4| {
        assert!(
            !live.replace(true),
            "the previous raw role must drop before the next load"
        );
        loads += 1;
        Ok(TrackedPayload {
            bytes: vec![u8::try_from(loads).expect("small test load count")],
            live: Rc::clone(&live),
            drops: Rc::clone(&drops),
        })
    };
    let first = with_kagemusha_artifact_payload_v4(
        &mut load,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        |bytes| Ok(bytes[0]),
    )
    .expect("first parsed role");
    assert_eq!(first, 1);
    assert!(!live.get());
    let error = with_kagemusha_artifact_payload_v4(
        &mut load,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
        |_| Err::<(), _>("expected parser failure".to_owned()),
    )
    .expect_err("parser failure must propagate");
    assert_eq!(error, "expected parser failure");
    assert!(!live.get());
    assert_eq!(loads, 2);
    assert_eq!(drops.get(), 2);
}
fn output_frontier_binding_builder(
    profile: [u64; 3],
    input_frontier: u64,
    result_frontier: u64,
    recipient_index: u64,
    change_index: u64,
    dummy_index: u64,
    topup_leaf_index: u64,
) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> {
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    let mut builder = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(8)
        .use_lookup_bits(7);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let zero = ctx.load_witness(Fp::ZERO);
    let [is_init, is_append, is_redemption] =
        profile.map(|value| ctx.load_witness(Fp::from(value)));
    let input_frontier = ctx.load_witness(Fp::from(input_frontier));
    let result_frontier = ctx.load_witness(Fp::from(result_frontier));
    let mut output =
        [zero; crate::zk::kagemusha_v2::KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4];
    output[7] = ctx.load_witness(Fp::from(recipient_index));
    output[9] = ctx.load_witness(Fp::from(change_index));
    output[10] = ctx.load_witness(Fp::from(dummy_index));
    let bindings = crate::zk::kagemusha_step_transition::NamedTransitionBindings {
        operation: crate::zk::kagemusha_step_transition::AssignedKagemushaStepOperationV4 {
            limbs: vec![zero; KAGEMUSHA_STEP_OPERATION_LIMBS_V4]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("exact Kagemusha operation limb count")),
            fields: vec![zero; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| unreachable!("exact Kagemusha operation field count")),
        },
        is_init,
        is_append,
        is_redemption,
        has_change: zero,
        input_root: zero,
        output_root: zero,
        input_next_zero_leaf_index: input_frontier,
        output_next_zero_leaf_index: result_frontier,
        input_commitments: [zero; 2],
        input_nullifiers: [zero; 2],
        recipient_commitment: zero,
        change_commitment: zero,
        statement_digest_limbs: [zero; 8],
        init_payer_tag_limbs: [zero; 8],
        init_operation_tag_limbs: [zero; 8],
    };
    let topup_leaf_index = ctx.load_witness(Fp::from(topup_leaf_index));
    constrain_kagemusha_output_frontier_v4(ctx, &range, &bindings, &output, topup_leaf_index);
    builder.calculate_params(Some(9));
    builder
}
fn assert_frontier_binding(
    expected_satisfied: bool,
    profile: [u64; 3],
    input: u64,
    result: u64,
    recipient: u64,
    change: u64,
    dummy: u64,
    topup: u64,
) {
    let builder =
        output_frontier_binding_builder(profile, input, result, recipient, change, dummy, topup);
    let verification =
        halo2_proofs::dev::MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("frontier binding mock prover")
            .verify();
    assert_eq!(verification.is_ok(), expected_satisfied);
}
#[test]
fn v4_eq_frontier_copy_constraints_reject_every_index_substitution() {
    assert_frontier_binding(true, [1, 0, 0], 0, 8, 7, 0, 8, 7);
    assert_frontier_binding(false, [1, 0, 0], 0, 8, 7, 0, 8, 6);
    assert_frontier_binding(true, [0, 1, 0], 7, 8, 7, 0, 8, 0);
    assert_frontier_binding(false, [0, 1, 0], 7, 8, 6, 0, 8, 0);
    assert_frontier_binding(true, [0, 0, 1], 7, 8, 0, 7, 8, 0);
    assert_frontier_binding(false, [0, 0, 1], 7, 8, 0, 6, 8, 0);
    assert_frontier_binding(false, [0, 1, 0], 7, 9, 7, 0, 8, 0);
}
#[test]
fn v4_params_reject_default_k12_and_stale_public_layout() {
    assert!(KagemushaStepCircuitParamsV4::default().validate().is_err());
    let valid = valid_step_circuit_params_v4();
    let layout = valid.validate().expect("valid V4 lower-bound layout");
    assert_eq!(layout.accumulator_limbs, 38);
    assert_eq!(layout.instance_column_limbs, 66);
    assert_eq!(layout.live_selector_offset, 65);
    let mut k12 = valid.clone();
    k12.k = 12;
    assert!(k12.validate().is_err());
    let mut legacy_fixed_degree_layout = valid;
    legacy_fixed_degree_layout.public_input_limbs = 4_156;
    assert!(legacy_fixed_degree_layout.validate().is_err());
}
#[test]
fn v5_generation_preflight_pins_compact_k17_key_sizes_before_allocation() {
    use halo2_proofs::halo2curves::pasta::EqAffine;
    let absolute = KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4;
    assert_eq!(
        select_kagemusha_generation_memory_limit_v4(absolute * 4, None)
            .expect("omitted lowering selects the absolute ceiling"),
        absolute
    );
    assert_eq!(
        select_kagemusha_generation_memory_limit_v4(absolute, None)
            .expect("omitted lowering selects half of physical RAM"),
        absolute / 2
    );
    assert_eq!(
        select_kagemusha_generation_memory_limit_v4(absolute, Some(absolute / 4))
            .expect("an operator can lower the ceiling"),
        absolute / 4
    );
    assert!(
        select_kagemusha_generation_memory_limit_v4(absolute, Some(absolute / 2 + 1)).is_err(),
        "an operator cannot raise the in-process ceiling"
    );
    assert!(
        select_kagemusha_generation_memory_limit_v4(absolute, Some(0)).is_err(),
        "zero cannot disable the in-process ceiling"
    );
    assert!(
        checked_kagemusha_generation_product_v4(&[u64::MAX, 2], "test")
            .expect_err("working-set arithmetic must fail closed")
            .contains("overflow")
    );
    let reviewed = first_release_generation_params_v4();
    let shape = kagemusha_processed_key_shape_v4::<EqAffine>(&reviewed, "Eq")
        .expect("reviewed Eq encoding shape");
    assert_eq!(
        kagemusha_params_encoded_bytes_v4::<EqAffine>(reviewed.k, "Eq")
            .expect("reviewed parameter length"),
        8_388_676
    );
    assert_eq!(
        shape.verifier_key_bytes("Eq").expect("reviewed VK length"),
        20_362
    );
    assert_eq!(
        shape.proving_key_bytes("Eq").expect("reviewed PK length"),
        5_347_763_078
    );
    let preflight = preflight_kagemusha_generation_v4(&reviewed, &reviewed)
        .expect("compact k17 profile passes before ParamsIPA allocation");
    assert_eq!(preflight.layout.instance_column_limbs, 66);
    assert_eq!(preflight.estimated_peak_bytes, 53_108_563_136);
    assert!(preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4);
    assert!(
        preflight.estimated_peak_bytes <= KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5,
        "the reviewed staged lifecycle must remain within 56 GiB"
    );
    let mut stale_proof_size = reviewed.clone();
    stale_proof_size.max_parent_proof_bytes += 1;
    assert!(preflight_kagemusha_generation_v4(&stale_proof_size, &stale_proof_size).is_err());
    let mut stale = reviewed;
    stale.version = 4;
    assert!(preflight_kagemusha_generation_v4(&stale, &stale).is_err());
}
#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn linux_memory_capacity_is_cgroup_aware_and_strictly_parsed() {
    let memberships = parse_linux_memory_cgroup_memberships_v4(
        "0::/tenant.slice/iroha\n7:cpu,cpuacct:/tenant.slice/iroha\n",
    )
    .expect("parse cgroup-v2 membership");
    assert_eq!(
        memberships,
        vec![LinuxMemoryCgroupMembershipV4 {
            version: LinuxMemoryCgroupVersionV4::V2,
            path: std::path::PathBuf::from("/tenant.slice/iroha"),
        }]
    );
    let legacy = parse_linux_memory_cgroup_memberships_v4(
        "4:memory,blkio:/containers/iroha\n5:cpu:/containers/iroha\n",
    )
    .expect("parse cgroup-v1 memory membership");
    assert_eq!(legacy[0].version, LinuxMemoryCgroupVersionV4::V1);
    let mounts = parse_linux_memory_cgroup_mounts_v4(
            "36 25 0:32 / /sys/fs/cgroup rw,nosuid,nodev,noexec,relatime - cgroup2 cgroup rw\n\
             37 25 0:33 /containers /sys/fs/cgroup/memory rw,nosuid,nodev,noexec,relatime - cgroup cgroup rw,memory\n",
        )
        .expect("parse cgroup mountinfo");
    assert_eq!(mounts.len(), 2);
    assert_eq!(mounts[0].version, LinuxMemoryCgroupVersionV4::V2);
    assert_eq!(mounts[1].version, LinuxMemoryCgroupVersionV4::V1);
    assert_eq!(
        parse_linux_cgroup_memory_limit_v4("8589934592\n", LinuxMemoryCgroupVersionV4::V2,)
            .expect("finite v2 limit"),
        Some(8 * 1024 * 1024 * 1024)
    );
    assert_eq!(
        parse_linux_cgroup_memory_limit_v4("max\n", LinuxMemoryCgroupVersionV4::V2)
            .expect("unlimited v2 limit"),
        None
    );
    assert_eq!(
            parse_linux_cgroup_memory_limit_v4(
                "9223372036854771712\n",
                LinuxMemoryCgroupVersionV4::V1,
            )
            .expect("unlimited v1 sentinel"),
            None
        );
    for malformed in ["", "0\n", "01\n", "8", "8 \n", "8\n9\n"] {
        assert!(
            parse_linux_cgroup_memory_limit_v4(malformed, LinuxMemoryCgroupVersionV4::V2,).is_err(),
            "malformed cgroup limit must fail closed: {malformed:?}"
        );
    }
    let host = 128 * 1024 * 1024 * 1024;
    let cgroup = 8 * 1024 * 1024 * 1024;
    assert_eq!(
        select_linux_physical_capacity_v4(host, Some(cgroup))
            .expect("finite cgroup lowers host capacity"),
        cgroup
    );
    assert_eq!(
        select_linux_physical_capacity_v4(host, None)
            .expect("unlimited cgroup retains host capacity"),
        host
    );
    let capacity = KagemushaGenerationMemoryCapacityV1 {
        effective_physical_capacity_bytes: cgroup,
        safety_ceiling_bytes: select_kagemusha_generation_memory_limit_v4(cgroup, None)
            .expect("derive cgroup-aware generation ceiling"),
    };
    assert_eq!(capacity.safety_ceiling_bytes(), 4 * 1024 * 1024 * 1024);
    assert_eq!(
        capacity.canonical_record(),
        "iroha.kagemusha.memory-capacity.v1 physical=8589934592 ceiling=4294967296 absolute=68719476736 profile=self-physical-footprint-v1 policy=half-effective-physical-cap-absolute-v1"
    );
}
#[test]
fn v4_memory_monitor_initialization_failure_is_fail_closed() {
    fn unavailable_footprint() -> Result<u64, String> {
        Err("injected footprint failure".to_owned())
    }
    let error = start_kagemusha_generation_memory_monitor_v4(
        KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4,
        unavailable_footprint,
    )
    .expect_err("a monitor without a first physical-footprint sample must fail");
    assert!(error.contains("injected footprint failure"));
}
#[test]
fn v4_memory_monitor_refuses_an_initial_sample_over_the_ceiling() {
    fn oversized_footprint() -> Result<u64, String> {
        Ok(2_048)
    }
    let error = start_kagemusha_generation_memory_monitor_v4(1_024, oversized_footprint)
        .expect_err("an already-oversized process must not receive a memory guard");
    assert!(error.contains("already exceeds"));
}
#[test]
fn macos_native_resource_value_validation_is_strict() {
    assert_eq!(
        validate_kagemusha_macos_native_resource_value_v4(
            137_438_953_472,
            std::mem::size_of::<u64>(),
            std::mem::size_of::<u64>(),
            "physical-memory",
        )
        .expect("canonical macOS physical memory"),
        137_438_953_472
    );
    assert!(
        validate_kagemusha_macos_native_resource_value_v4(
            0,
            std::mem::size_of::<u64>(),
            std::mem::size_of::<u64>(),
            "physical-memory",
        )
        .is_err(),
        "zero native resource values must fail closed"
    );
    assert!(
        validate_kagemusha_macos_native_resource_value_v4(
            1,
            std::mem::size_of::<u32>(),
            std::mem::size_of::<u64>(),
            "physical-memory",
        )
        .is_err(),
        "ABI-size drift must fail closed"
    );
}
#[cfg(target_os = "macos")]
#[test]
fn macos_native_resource_queries_match_the_public_abi() {
    assert_eq!(
        std::mem::size_of::<KagemushaMacosRusageInfoV0>(),
        96,
        "rusage_info_v0 ABI drift"
    );
    assert!(kagemusha_physical_memory_bytes_v4().expect("macOS physical-memory query") > 0);
    assert!(
        kagemusha_process_physical_footprint_bytes_v4().expect("macOS physical-footprint query")
            > 0
    );
}
static POST_HANDSHAKE_OVER_CAP_SAMPLES_V4: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
static POST_HANDSHAKE_FAILURE_SAMPLES_V4: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
fn post_handshake_over_cap_sampler_v4() -> Result<u64, String> {
    if POST_HANDSHAKE_OVER_CAP_SAMPLES_V4.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
        Ok(1)
    } else {
        Ok(2_048)
    }
}
fn post_handshake_failure_sampler_v4() -> Result<u64, String> {
    if POST_HANDSHAKE_FAILURE_SAMPLES_V4.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
        Ok(1)
    } else {
        Err("injected post-handshake sampler failure".to_owned())
    }
}
#[test]
fn v4_memory_monitor_post_handshake_failures_abort_the_process() {
    const CHILD_MODE_ENV: &str = "IROHA_TEST_KAGEMUSHA_MEMORY_MONITOR_CHILD_V4";
    if let Ok(mode) = std::env::var(CHILD_MODE_ENV) {
        let sampler: fn() -> Result<u64, String> = match mode.as_str() {
            "over-cap" => post_handshake_over_cap_sampler_v4,
            "sampler-failure" => post_handshake_failure_sampler_v4,
            _ => panic!("unknown memory-monitor child mode"),
        };
        start_kagemusha_generation_memory_monitor_v4(1_024, sampler)
            .expect("the child monitor must complete its first handshake");
        std::thread::sleep(std::time::Duration::from_secs(2));
        panic!("the mandatory monitor did not abort after {mode}");
    }
    for mode in ["over-cap", "sampler-failure"] {
        let status = std::process::Command::new(
            std::env::current_exe().expect("current unit-test executable"),
        )
        .arg("v4_memory_monitor_post_handshake_failures_abort_the_process")
        .arg("--nocapture")
        .env(CHILD_MODE_ENV, mode)
        .status()
        .expect("spawn isolated memory-monitor regression child");
        assert!(!status.success(), "{mode} child must terminate fail-closed");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            assert_eq!(status.signal(), Some(6), "{mode} child must abort");
        }
    }
}
#[test]
fn qualification_verifier_callers_have_a_static_memory_contract() {
    let adapter = include_str!("../kagemusha_recursion_adapter.rs");
    let verifier_signature = adapter
        .split_once("pub fn verify_candidate_recursive_step_two_receipt_v4")
        .expect("qualification verifier definition")
        .1
        .split_once("where")
        .expect("qualification verifier generic boundary")
        .0;
    assert!(verifier_signature.contains("qualification_memory_contract"));
    let generator_signature = adapter
        .split_once("pub fn generate_candidate_recursive_step_two_receipt_v4")
        .expect("qualification generator definition")
        .1
        .split_once("where")
        .expect("qualification generator generic boundary")
        .0;
    assert!(generator_signature.contains("memory_guard"));
    let bundle = include_str!("../../bin/kagemusha_recursive_spend_v4_bundle.rs");
    let catalog =
        include_str!("../../smartcontracts/isi/offline/kagemusha_terminal_registry_v4.rs");
    let kagami = include_str!("../../../../iroha_kagami/src/kagemusha.rs");
    for (role, source) in [
        ("bundle", bundle),
        ("runtime catalog", catalog),
        ("Kagami", kagami),
    ] {
        let calls = source
            .split("verify_candidate_recursive_step_two_receipt_v4(")
            .skip(1)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 1, "unexpected {role} verifier caller count");
        let call_prefix = calls[0].chars().take(800).collect::<String>();
        assert!(
            call_prefix.contains("qualification_memory_contract"),
            "{role} verifier caller must pass an explicit memory contract"
        );
    }
    assert_eq!(
        bundle
            .matches("start_kagemusha_generation_memory_guard_v4(")
            .count(),
        1,
        "the bundle command dispatcher must start exactly one guard"
    );
    for operator_path in [
        "fn build_candidate(",
        "fn publish_staged_candidate(",
        "fn validate_candidate(",
        "fn finalize_release(",
    ] {
        let signature = bundle
            .split_once(operator_path)
            .unwrap_or_else(|| panic!("missing guarded operator path {operator_path}"))
            .1
            .split_once(") ->")
            .expect("operator-path signature boundary")
            .0;
        assert!(
            signature.contains("memory_guard"),
            "{operator_path} must receive the active memory guard"
        );
    }
    assert_eq!(
        kagami
            .matches("start_kagemusha_generation_memory_guard_v4(")
            .count(),
        2,
        "Kagami verify and promote must each start a guard"
    );
    assert_eq!(
        catalog
            .matches("KagemushaQualificationMemoryContractV4::for_runtime_catalog(")
            .count(),
        1,
        "the runtime catalog must use its separate decoded-memory contract"
    );
}
#[test]
fn v5_auxiliary_capacity_rejects_sha_and_dense_overflow_with_diagnostics() {
    use crate::zk::{
        kagemusha_cycle_loader::{LIMB_BITS, LIMBS},
        kagemusha_dense_msm::KagemushaDenseMsmSourceV5,
    };
    use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::CurveAffineExt as _};
    use halo2_ecc::{
        bigint::ProperCrtUint,
        fields::{FieldChip as _, fp::FpChip},
    };
    use halo2_proofs::halo2curves::pasta::EpAffine;
    fn params_with_usable_rows(usable_rows: usize) -> KagemushaStepCircuitParamsV4 {
        let mut params = valid_step_circuit_params_v4();
        let domain_rows = 1_u32
            .checked_shl(params.k)
            .expect("k17 domain rows fit u32");
        params.minimum_unusable_rows = domain_rows
            .checked_sub(u32::try_from(usable_rows).expect("test usable rows fit u32"))
            .expect("test usable rows fit the k17 domain");
        params.validate().expect("test capacity parameters");
        params
    }
    let sha_jobs = KagemushaSha256JobsV4::<Fp>::default();
    let empty_dense = KagemushaDenseMsmJobsV5::<EpAffine>::default();
    let (sha_jobs_count, sha_blocks, sha_required_rows) =
        sha_jobs.capacity_profile().expect("empty SHA profile");
    let (dense_jobs_count, dense_sources, dense_required_rows) = empty_dense
        .capacity_profile()
        .expect("empty dense-MSM profile");
    let sha_usable_rows = sha_required_rows - 1;
    let sha_error = validate_kagemusha_auxiliary_capacity_v5(
        &sha_jobs,
        &empty_dense,
        &params_with_usable_rows(sha_usable_rows),
        "StepEqCapacityTest",
    )
    .expect_err("the Table16 footprint must exceed the reduced row budget");
    assert_eq!(
        sha_error,
        format!(
            "Kagemusha V5 StepEqCapacityTest auxiliary capacity exceeds {sha_usable_rows} usable rows: SHA jobs={sha_jobs_count}, blocks={sha_blocks}, required_rows={sha_required_rows}; dense jobs={dense_jobs_count}, sources={dense_sources}, required_rows={dense_required_rows}"
        )
    );
    let mut builder = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(17)
        .use_lookup_bits(16);
    let range = builder.range_chip();
    let scalar_chip = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
    let point = EpAffine::generator();
    let (x, y) = point.into_coordinates();
    let coefficient = scalar_chip.load_private(builder.main(0), Fq::ONE);
    let coefficient: ProperCrtUint<Fp> = scalar_chip
        .enforce_less_than(builder.main(0), coefficient)
        .into();
    let source = KagemushaDenseMsmSourceV5 {
        point,
        x: builder.main(0).load_witness(x),
        y: builder.main(0).load_witness(y),
        coefficient,
    };
    let sources = vec![source; 400];
    let mut dense_jobs = KagemushaDenseMsmJobsV5::default();
    dense_jobs
        .queue_constrained(builder.main(0), &scalar_chip, &sources)
        .expect("repeated small dense-MSM source fixture");
    let (dense_jobs_count, dense_sources, dense_required_rows) = dense_jobs
        .capacity_profile()
        .expect("populated dense-MSM profile");
    assert!(dense_required_rows > sha_required_rows);
    let dense_error = validate_kagemusha_auxiliary_capacity_v5(
        &sha_jobs,
        &dense_jobs,
        &params_with_usable_rows(sha_required_rows),
        "StepEpCapacityTest",
    )
    .expect_err("the dense-MSM footprint must exceed the SHA-sized row budget");
    assert_eq!(
        dense_error,
        format!(
            "Kagemusha V5 StepEpCapacityTest auxiliary capacity exceeds {sha_required_rows} usable rows: SHA jobs={sha_jobs_count}, blocks={sha_blocks}, required_rows={sha_required_rows}; dense jobs={dense_jobs_count}, sources={dense_sources}, required_rows={dense_required_rows}"
        )
    );
}
