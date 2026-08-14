// Core STARK verifier and transcript regression tests.
//
// Included by `zk_stark::tests` to preserve exact libtest names.
use super::*;
fn attach_valid_auxiliary_composition_values(envelope: &mut StarkVerifyEnvelopeV1) {
    let query_count = envelope.proof.queries.len();
    let leaf = 30_u64;
    let (comp_root, path) =
        stark_merkle_root_and_path_from_field_values_v1(&envelope.params, &[leaf], 0)
            .expect("derive auxiliary composition root");
    let comp_values = (0..query_count)
        .map(|_| StarkCompositionValueV1 {
            leaf,
            constant: 7,
            z_coeff: 0,
            aux_terms: vec![
                StarkCompositionTermV1 {
                    wire_index: 1,
                    value: 5,
                    coeff: 3,
                },
                StarkCompositionTermV1 {
                    wire_index: 3,
                    value: 2,
                    coeff: 4,
                },
            ],
            path: path.clone(),
        })
        .collect();
    envelope.proof.commits.comp_root = Some(comp_root);
    envelope.proof.comp_values = Some(comp_values);
}
#[test]
fn fq_addition_wraps_correctly() {
    let a = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
    let b = Fq::one();
    assert_eq!(a.add(b), Fq::zero());
}
#[test]
fn fq_subtraction_borrows_mod_prime() {
    let a = Fq::zero();
    let b = Fq::one();
    let expected = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
    assert_eq!(a.sub(b), expected);
}
#[test]
fn fq_multiplication_reduces() {
    let a = Fq::from_canonical_u64(2).unwrap();
    let b = Fq::from_canonical_u64(MOD_P_U64 - 1).unwrap();
    let product = a.mul(b);
    let expected = Fq::from_canonical_u64(MOD_P_U64 - 2).unwrap();
    assert_eq!(product, expected);
}
#[test]
fn fq_inverse_round_trip() {
    let element = Fq::from_canonical_u64(5).unwrap();
    let inv = element.inv().expect("invertible");
    assert_eq!(element.mul(inv), Fq::one());
}
#[test]
fn fq_new_reduces_large_inputs() {
    let value = u64::MAX;
    let reduced = Fq::new(value);
    let expected = Fq::from_canonical_u64(((value as u128) % MOD_P) as u64).unwrap();
    assert_eq!(reduced, expected);
}
#[test]
fn fq_from_canonical_rejects_out_of_range() {
    assert!(Fq::from_canonical_u64(MOD_P_U64).is_none());
}
#[test]
fn stark_fri_canonical_verifying_key_payload_validation_fails_closed() {
    const CIRCUIT_ID: &str = "soracloud:test-canonical-circuit";
    let valid = StarkFriVerifyingKeyV1 {
        version: 1,
        circuit_id: CIRCUIT_ID.to_owned(),
        n_log2: STARK_FRI_CONSENSUS_MIN_N_LOG2,
        blowup_log2: STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
        fold_arity: 2,
        queries: STARK_FRI_CONSENSUS_MIN_QUERIES,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
    };
    validate_stark_fri_canonical_verifying_key_payload(&valid, CIRCUIT_ID, "test")
        .expect("canonical STARK/FRI payload is accepted");
    let mut poseidon = valid.clone();
    poseidon.hash_fn = STARK_HASH_POSEIDON2_V1;
    validate_stark_fri_canonical_verifying_key_payload(&poseidon, CIRCUIT_ID, "test")
        .expect("Poseidon2 STARK/FRI payload is accepted");
    let mutations: [(&str, fn(&mut StarkFriVerifyingKeyV1)); 11] = [
        ("version", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.version = 2
        }),
        ("circuit", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.circuit_id = "other".to_owned()
        }),
        ("hash", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.hash_fn = 0xff;
        }),
        ("fold", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.fold_arity = 4;
        }),
        ("merkle", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.merkle_arity = 4;
        }),
        ("n_log2_floor", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.n_log2 = STARK_FRI_CONSENSUS_MIN_N_LOG2 - 1;
        }),
        ("blowup_floor", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.blowup_log2 = STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2 - 1;
        }),
        ("blowup_domain", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.blowup_log2 = payload.n_log2 + 1;
        }),
        ("queries_floor", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.queries = STARK_FRI_CONSENSUS_MIN_QUERIES - 1;
        }),
        ("domain_limit", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.n_log2 = MAX_DOMAIN_LOG2 + 1;
        }),
        ("query_limit", |payload: &mut StarkFriVerifyingKeyV1| {
            payload.queries = MAX_FRI_QUERIES as u16 + 1;
        }),
    ];
    for (label, mutate) in mutations {
        let mut invalid = valid.clone();
        mutate(&mut invalid);
        assert!(
            validate_stark_fri_canonical_verifying_key_payload(&invalid, CIRCUIT_ID, "test")
                .is_err(),
            "{label} mutation must fail closed"
        );
    }
    let overlong_circuit_id = "soracloud:".to_owned() + &"x".repeat(MAX_TRANSCRIPT_LABEL_LEN);
    for circuit_id in [
        "",
        " ",
        "soracloud:test canonical-circuit",
        "soracloud:test\tproduction-circuit",
        overlong_circuit_id.as_str(),
    ] {
        let mut invalid = valid.clone();
        invalid.circuit_id = circuit_id.to_owned();
        assert!(
            validate_stark_fri_canonical_verifying_key_payload(&invalid, circuit_id, "test")
                .is_err(),
            "matching noncanonical circuit id {circuit_id:?} must fail closed"
        );
    }
}
#[test]
fn stark_verifier_limits_cannot_relax_canonical_structure_caps() {
    let valid = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:canonical-limit-cap".to_owned(),
    };
    assert_eq!(
        validate_params(&valid, 5, 2, &StarkVerifierLimits::default()),
        Some(4)
    );
    let mut relaxed = StarkVerifierLimits::default();
    relaxed.max_domain_log2 = MAX_DOMAIN_LOG2 + 1;
    relaxed.max_blowup_log2 = MAX_DOMAIN_LOG2 + 1;
    relaxed.max_queries = MAX_FRI_QUERIES + 1;
    relaxed.max_merkle_depth = MAX_MERKLE_DEPTH + 1;
    relaxed.max_aux_terms = MAX_AUX_TERMS + 1;
    relaxed.max_air_width = MAX_AIR_WIDTH + 1;
    relaxed.max_domain_tag_len = MAX_DOMAIN_TAG_LEN + 1;
    relaxed.max_transcript_label_len = MAX_TRANSCRIPT_LABEL_LEN + 1;
    relaxed.max_envelope_bytes = MAX_ENVELOPE_BYTES + 1;
    let mut oversized_domain = valid.clone();
    oversized_domain.n_log2 = MAX_DOMAIN_LOG2 + 1;
    oversized_domain.queries = 1;
    assert!(
        validate_params(
            &oversized_domain,
            usize::from(MAX_DOMAIN_LOG2) + 2,
            1,
            &relaxed
        )
        .is_none(),
        "caller limits must not relax canonical domain depth"
    );
    let mut oversized_blowup = valid.clone();
    oversized_blowup.blowup_log2 = MAX_DOMAIN_LOG2 + 1;
    assert!(
        validate_params(&oversized_blowup, 5, 2, &relaxed).is_none(),
        "caller limits must not relax canonical blowup depth"
    );
    let mut impossible_blowup = valid.clone();
    impossible_blowup.blowup_log2 = valid.n_log2 + 1;
    assert!(
        validate_params(&impossible_blowup, 5, 2, &relaxed).is_none(),
        "verifier must reject blowup depth greater than the evaluation domain"
    );
    let mut too_many_queries = valid.clone();
    too_many_queries.n_log2 = 6;
    too_many_queries.queries = (MAX_FRI_QUERIES + 1) as u16;
    assert!(
        validate_params(&too_many_queries, 7, MAX_FRI_QUERIES + 1, &relaxed).is_none(),
        "caller limits must not relax canonical query count"
    );
    let mut overlong_domain_tag = valid.clone();
    overlong_domain_tag.domain_tag = "d".repeat(MAX_DOMAIN_TAG_LEN + 1);
    assert!(
        validate_params(&overlong_domain_tag, 5, 2, &relaxed).is_none(),
        "caller limits must not relax canonical domain-tag length"
    );
    let too_deep_path = MerklePath {
        dirs: vec![0; (MAX_MERKLE_DEPTH + 8) / 8],
        siblings: vec![[0; 32]; MAX_MERKLE_DEPTH + 1],
    };
    assert!(
        !merkle_path_depth_ok(&too_deep_path, MAX_MERKLE_DEPTH + 1, &relaxed),
        "caller limits must not relax canonical Merkle depth"
    );
    let overlong_transcript_label = "T".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1);
    assert!(
        validate_stark_transcript_label(
            &overlong_transcript_label,
            effective_max_transcript_label_len(&relaxed),
        )
        .is_err(),
        "caller limits must not relax canonical transcript-label length"
    );
    assert_eq!(effective_max_aux_terms(&relaxed), MAX_AUX_TERMS);
    assert_eq!(effective_max_air_width(&relaxed), MAX_AIR_WIDTH);
    assert_eq!(effective_max_envelope_bytes(&relaxed), MAX_ENVELOPE_BYTES);
}
#[test]
fn synthesized_envelope_verifies_sha256() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:sha256".to_owned(),
    };
    let bytes = prove_stark_fri_air_envelope_bytes(
        params,
        "IROHA-TEST-STARK".to_owned(),
        "stark/fri/sha256-goldilocks:test".to_owned(),
        [0x11; 32],
    )
    .expect("ok");
    assert!(verify_stark_fri_envelope(&bytes));
}
#[test]
fn public_generic_air_provers_reject_bfv_full_bootstrap_circuit_aliases() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:reserved-bfv-generic-air".to_owned(),
    };
    let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
    let composition_values = vec![0; rows.len()];
    let base_indices = [0_usize, 1];
    let canonical = iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1;
    let circuit_ids = [
        canonical.to_owned(),
        format!("stark/fri/sha256-goldilocks:{canonical}"),
        format!("stark/fri/sha256-goldilocks/{canonical}"),
        format!("stark/fri/poseidon2-goldilocks:{canonical}"),
    ];
    for circuit_id in circuit_ids {
        let err = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-BFV-GENERIC-AIR".to_owned(),
            circuit_id.clone(),
            [0xB4; 32],
        )
        .expect_err("generic AIR prover must reject BFV full-bootstrap circuit aliases");
        assert!(
            err.contains("BFV full-bootstrap"),
            "unexpected generic AIR rejection for {circuit_id}: {err}"
        );
        let err = prove_stark_fri_zero_composition_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-BFV-ZERO-AIR".to_owned(),
            circuit_id.clone(),
            [0xB5; 32],
            rows.clone(),
        )
        .expect_err("zero-composition AIR prover must reject BFV full-bootstrap circuit aliases");
        assert!(
            err.contains("BFV full-bootstrap"),
            "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
        );
        let err = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-BFV-EXPLICIT-AIR".to_owned(),
            circuit_id.clone(),
            [0xB6; 32],
            rows.clone(),
            composition_values.clone(),
        )
        .expect_err("explicit row/composition AIR prover must reject BFV aliases");
        assert!(
            err.contains("BFV full-bootstrap"),
            "unexpected explicit AIR rejection for {circuit_id}: {err}"
        );
        let err =
            prove_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_bytes(
                params.clone(),
                "IROHA-TEST-RESERVED-BFV-EXPLICIT-BASE-AIR".to_owned(),
                circuit_id.clone(),
                [0xB7; 32],
                rows.clone(),
                composition_values.clone(),
                &base_indices,
            )
            .expect_err("explicit-base AIR prover must reject BFV aliases");
        assert!(
            err.contains("BFV full-bootstrap"),
            "unexpected explicit-base AIR rejection for {circuit_id}: {err}"
        );
    }
}
#[test]
fn public_generic_air_provers_reject_zk_ace_circuit_aliases() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:reserved-zk-ace-generic-air".to_owned(),
    };
    let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
    let canonical = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
    let circuit_ids = [
        canonical.to_owned(),
        format!("stark/fri/sha256-goldilocks:{canonical}"),
        format!("stark/fri/sha256-goldilocks/{canonical}"),
        format!("stark/fri/poseidon2-goldilocks:{canonical}"),
    ];
    for circuit_id in circuit_ids {
        let err = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-ZK-ACE-GENERIC-AIR".to_owned(),
            circuit_id.clone(),
            [0xC4; 32],
        )
        .expect_err("generic AIR prover must reject ZK-ACE circuit aliases");
        assert!(
            err.contains("ZK-ACE"),
            "unexpected generic AIR rejection for {circuit_id}: {err}"
        );
        let err = prove_stark_fri_zero_composition_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-ZK-ACE-ZERO-AIR".to_owned(),
            circuit_id.clone(),
            [0xC5; 32],
            rows.clone(),
        )
        .expect_err("zero-composition AIR prover must reject ZK-ACE circuit aliases");
        assert!(
            err.contains("ZK-ACE"),
            "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
        );
    }
}
#[test]
fn public_generic_air_provers_reject_ivm_execution_circuit_aliases() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:reserved-ivm-generic-air".to_owned(),
    };
    let rows = vec![vec![0]; 1_usize << usize::from(params.n_log2)];
    let canonical = crate::zk::IVM_EXECUTION_V1_CIRCUIT_ID;
    let circuit_ids = [
        canonical.to_owned(),
        format!("stark/fri/sha256-goldilocks:{canonical}"),
        format!("stark/fri/sha256-goldilocks/{canonical}"),
        format!("stark/fri/poseidon2-goldilocks:{canonical}"),
    ];
    for circuit_id in circuit_ids {
        let err = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-IVM-GENERIC-AIR".to_owned(),
            circuit_id.clone(),
            [0xD4; 32],
        )
        .expect_err("generic AIR prover must reject IVM execution circuit aliases");
        assert!(
            err.contains("IVM execution"),
            "unexpected generic AIR rejection for {circuit_id}: {err}"
        );
        let err = prove_stark_fri_zero_composition_air_envelope_bytes(
            params.clone(),
            "IROHA-TEST-RESERVED-IVM-ZERO-AIR".to_owned(),
            circuit_id.clone(),
            [0xD5; 32],
            rows.clone(),
        )
        .expect_err("zero-composition AIR prover must reject IVM execution circuit aliases");
        assert!(
            err.contains("IVM execution"),
            "unexpected zero-composition AIR rejection for {circuit_id}: {err}"
        );
    }
}
#[test]
fn generic_binding_air_reserves_governance_vote_role_aliases() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:reserved-governance-generic-air".to_owned(),
    };
    for canonical in [
        crate::zk::GOVERNANCE_BALLOT_CIRCUIT_ID_V1,
        crate::zk::GOVERNANCE_TALLY_CIRCUIT_ID_V1,
    ] {
        for circuit_id in [
            canonical.to_owned(),
            format!("stark/fri/sha256-goldilocks:{canonical}"),
            format!("stark/fri/sha256-goldilocks/{canonical}"),
            format!("stark/fri/poseidon2-goldilocks:{canonical}"),
        ] {
            let err = validate_generic_stark_air_circuit_id(&circuit_id)
                .expect_err("governance vote roles must be reserved from generic AIR");
            assert!(
                err.contains("governance vote role"),
                "unexpected governance role rejection for {circuit_id}: {err}"
            );
            let air = StarkAirProofV1 {
                version: 1,
                circuit_id: circuit_id.clone(),
                public_digest: [0; 32],
                trace_root: [0; 32],
                composition_root: [0; 32],
                trace_width: STARK_BINDING_AIR_TRACE_WIDTH_V1,
                openings: Vec::new(),
            };
            assert!(
                !stark_air_context_matches_statement(
                    &params,
                    &air,
                    1_usize << usize::from(params.n_log2),
                    StarkAirVerificationContext::Binding,
                ),
                "generic verifier context must reject governance role {circuit_id}"
            );
        }
    }
    assert!(
        validate_generic_stark_air_circuit_id("stark/fri/sha256-goldilocks:vote-ballot-near-miss")
            .is_ok(),
        "reservation must match complete semantic role ids"
    );
}
#[test]
fn generic_air_circuit_classifier_reserves_every_soracloud_fhe_relation_alias() {
    use iroha_data_model::soracloud::{
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1, SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
    };
    for canonical in [
        SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    ] {
        for circuit_id in [
            canonical.to_owned(),
            format!("stark/fri/sha256-goldilocks:{canonical}"),
            format!("stark/fri/sha256-goldilocks/{canonical}"),
            format!("stark/fri/poseidon2-goldilocks:{canonical}"),
        ] {
            let err = validate_generic_stark_air_circuit_id(&circuit_id)
                .expect_err("Soracloud FHE relation must be reserved from generic AIR");
            assert!(
                err.contains("Soracloud") || err.contains("BFV full-bootstrap"),
                "unexpected Soracloud relation rejection for {circuit_id}: {err}"
            );
        }
    }
    assert!(
        validate_generic_stark_air_circuit_id(
            "stark/fri/sha256-goldilocks:soracloud_fhe_input_admission_v1_near_miss",
        )
        .is_ok(),
        "reservation must match complete relation ids, not unrelated prefixes"
    );
}
#[test]
fn synthesized_field_values_envelope_has_replayable_query_shape() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:field-values".to_owned(),
    };
    let values = vec![0; 1_usize << usize::from(params.n_log2)];
    let extra_query_roots = [[0xA1; 32], [0xA2; 32], [0xA3; 32]];
    let envelope = stark_synthesize_fri_envelope_from_field_values_v1(
        params.clone(),
        "IROHA-TEST-STARK".to_owned(),
        &values,
        &extra_query_roots,
    )
    .expect("synthesize field-value FRI envelope");
    let indices = validate_stark_fri_query_shape_and_indices_v1(
        &params,
        &envelope.transcript_label,
        &envelope.proof.commits.roots,
        &extra_query_roots,
        &envelope.proof.queries,
    )
    .expect("query shape replays");
    assert_eq!(indices.len(), usize::from(params.queries));
    assert!(
        indices
            .iter()
            .enumerate()
            .all(|(index, sampled)| !indices[..index].contains(sampled)),
        "query sampling must not repeat base indices"
    );
    let mut stale_merkle = envelope.clone();
    stale_merkle.proof.queries[0][0].path_y0.siblings[0][0] ^= 1;
    assert_eq!(
        validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &stale_merkle.transcript_label,
            &stale_merkle.proof.commits.roots,
            &extra_query_roots,
            &stale_merkle.proof.queries,
        )
        .expect_err("stale FRI Merkle openings must be rejected"),
        "FRI query Merkle root mismatch"
    );
    let mut stale_folded_merkle = envelope.clone();
    stale_folded_merkle.proof.queries[0][0].path_z.siblings[0][0] ^= 1;
    assert_eq!(
        validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &stale_folded_merkle.transcript_label,
            &stale_folded_merkle.proof.commits.roots,
            &extra_query_roots,
            &stale_folded_merkle.proof.queries,
        )
        .expect_err("stale folded FRI Merkle openings must be rejected"),
        "FRI query folded Merkle root mismatch"
    );
    let mut stale_fold = envelope;
    stale_fold.proof.queries[0][0].z = stale_fold.proof.queries[0][0].z.saturating_add(1);
    assert_eq!(
        validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &stale_fold.transcript_label,
            &stale_fold.proof.commits.roots,
            &extra_query_roots,
            &stale_fold.proof.queries,
        )
        .expect_err("stale FRI fold values must be rejected"),
        "FRI query fold relation mismatch"
    );
    let nonzero_values = vec![1; 1_usize << usize::from(params.n_log2)];
    assert!(
        stark_synthesize_fri_envelope_from_field_values_v1(
            params.clone(),
            "IROHA-TEST-STARK".to_owned(),
            &nonzero_values,
            &extra_query_roots,
        )
        .is_none(),
        "prover must reject non-zero final FRI values before emitting proof bytes"
    );
    let public_err = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
        params,
        "IROHA-TEST-STARK".to_owned(),
        "stark/fri/nonzero-final:test".to_owned(),
        [0xA5; 32],
        vec![vec![0]; 1_usize << 4],
        nonzero_values,
    )
    .expect_err("public AIR prover must reject non-zero final FRI values");
    assert_eq!(
        public_err, "STARK final FRI value must be zero",
        "non-zero final FRI values must fail during proof construction"
    );
}
#[test]
fn stark_fri_query_shape_rejects_unused_merkle_direction_bits() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:unused-merkle-dir-bits".to_owned(),
    };
    let values = vec![0; 1_usize << usize::from(params.n_log2)];
    let mut envelope = stark_synthesize_fri_envelope_from_field_values_v1(
        params.clone(),
        "IROHA-TEST-STARK-UNUSED-MERKLE-DIR-BITS".to_owned(),
        &values,
        &[],
    )
    .expect("synthesize field-value FRI envelope");
    let path = &mut envelope.proof.queries[0][0].path_y0;
    assert_eq!(path.dirs.len(), 1);
    path.dirs[0] |= 0b1000_0000;
    assert_eq!(
        validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &[],
            &envelope.proof.queries,
        )
        .expect_err("unused Merkle direction bits must be rejected"),
        "FRI query Merkle path depth mismatch"
    );
}
#[test]
fn without_replacement_query_schedule_uses_bound_specific_offsets() {
    assert_without_replacement_query_schedule_uses_bound_specific_offsets(
        STARK_HASH_SHA256_V1,
        "sha256",
    );
    assert_without_replacement_query_schedule_uses_bound_specific_offsets(
        STARK_HASH_POSEIDON2_V1,
        "poseidon2",
    );
}
fn assert_without_replacement_query_schedule_uses_bound_specific_offsets(
    hash_fn: u8,
    hash_label: &str,
) {
    let mut params = StarkFriParamsV1 {
        version: 1,
        n_log2: 3,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 4,
        merkle_arity: 2,
        hash_fn,
        domain_tag: String::new(),
    };
    let label = "IROHA-TEST-BOUNDED-STARK-QUERY-OFFSET";
    let roots = [[0x42; 32], [0x24; 32]];
    let domain = 1_usize << usize::from(params.n_log2);
    let query_number = 1;
    let remaining = domain - query_number;
    for nonce in 0_u32..4096 {
        params.domain_tag = format!("iroha:test:{hash_label}:bounded-query-offset:{nonce}");
        let domain_remodulo = derive_query_index(label, &params, &roots, query_number)
            .expect("query index")
            % remaining;
        let bounded = derive_bounded_query_offset(label, &params, &roots, query_number, remaining)
            .expect("bounded query offset");
        if domain_remodulo == bounded {
            continue;
        }
        let first_draw =
            derive_bounded_query_offset(label, &params, &roots, 0, domain).expect("first draw");
        let mut swaps = BTreeMap::new();
        let first_selected = first_draw;
        swaps.insert(first_draw, 0);
        let bounded_selected = swaps
            .get(&(query_number + bounded))
            .copied()
            .unwrap_or(query_number + bounded);
        let remodulo_selected = swaps
            .get(&(query_number + domain_remodulo))
            .copied()
            .unwrap_or(query_number + domain_remodulo);
        if bounded_selected == remodulo_selected {
            continue;
        }
        let indices = derive_query_indices_without_replacement(label, &params, &roots, 2, domain)
            .expect("without-replacement schedule");
        assert_eq!(indices[0], first_selected);
        assert_eq!(indices[1], bounded_selected);
        assert_ne!(
            indices[1], remodulo_selected,
            "query schedule must use a bound-specific draw, not domain sample modulo remaining"
        );
        return;
    }
    panic!("failed to find {hash_label} bounded-offset fixture that differs from domain remodulo");
}
#[test]
fn air_opening_first_fri_value_binding_uses_sampled_parity() {
    let empty_path = || MerklePath {
        dirs: Vec::new(),
        siblings: Vec::new(),
    };
    let opening = StarkAirOpeningV1 {
        index: 0,
        row: Vec::new(),
        next_row: Vec::new(),
        row_path: empty_path(),
        next_row_path: empty_path(),
        composition_value: 11,
        composition_path: empty_path(),
    };
    let decommit = FoldDecommitV1 {
        j: 0,
        y0: 11,
        y1: 17,
        path_y0: empty_path(),
        path_y1: empty_path(),
        z: 0,
        path_z: empty_path(),
    };
    validate_stark_air_opening_first_fri_value_v1(&opening, 0, &decommit)
        .expect("even sampled index binds y0");
    let mut odd_opening = opening.clone();
    odd_opening.index = 1;
    odd_opening.composition_value = 17;
    validate_stark_air_opening_first_fri_value_v1(&odd_opening, 1, &decommit)
        .expect("odd sampled index binds y1");
    assert_eq!(
        validate_stark_air_opening_first_fri_value_v1(&opening, 1, &decommit)
            .expect_err("mismatched sampled index must fail"),
        "AIR/FRI opening index mismatch"
    );
    let mut wrong_side_opening = odd_opening;
    wrong_side_opening.composition_value = 11;
    assert_eq!(
        validate_stark_air_opening_first_fri_value_v1(&wrong_side_opening, 1, &decommit)
            .expect_err("wrong FRI side must fail"),
        "AIR/FRI composition value mismatch"
    );
}
#[test]
fn synthesized_envelope_verifies_poseidon2() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_POSEIDON2_V1,
        domain_tag: "iroha:test:poseidon2".to_owned(),
    };
    let bytes = prove_stark_fri_air_envelope_bytes(
        params,
        "IROHA-TEST-STARK".to_owned(),
        "stark/fri/poseidon2-goldilocks:test".to_owned(),
        [0x22; 32],
    )
    .expect("ok");
    assert!(verify_stark_fri_envelope(&bytes));
}
#[test]
fn stark_fri_rejects_noncanonical_transcript_labels() {
    for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
        (
            STARK_HASH_SHA256_V1,
            "sha256",
            "iroha:test:canonical-transcript-label:sha256",
            "stark/fri/sha256-goldilocks:canonical-transcript-label",
            0x51_u8,
        ),
        (
            STARK_HASH_POSEIDON2_V1,
            "poseidon2",
            "iroha:test:canonical-transcript-label:poseidon2",
            "stark/fri/poseidon2-goldilocks:canonical-transcript-label",
            0x52_u8,
        ),
    ] {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn,
            domain_tag: domain_tag.to_owned(),
        };
        let bytes = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            format!("IROHA-TEST-STARK-CANONICAL-LABEL-{hash_label}"),
            circuit_id.to_owned(),
            [digest_byte; 32],
        )
        .expect("valid labeled STARK envelope");
        assert!(verify_stark_fri_envelope(&bytes));
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode labeled STARK envelope");
        let air = envelope.proof.air.as_ref().expect("AIR section");
        let extra_query_roots = [air.trace_root, air.composition_root, air.public_digest];
        let invalid_labels = [
            ("empty", String::new()),
            ("leading whitespace", " IROHA-TEST-STARK".to_owned()),
            ("embedded whitespace", "IROHA TEST STARK".to_owned()),
            ("control byte", "IROHA-TEST\nSTARK".to_owned()),
            ("non-ASCII", "IROHA-TEST-STARK-π".to_owned()),
            ("overlong", "A".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1)),
        ];
        for (label_case, invalid_label) in invalid_labels {
            let err = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                invalid_label.clone(),
                circuit_id.to_owned(),
                [digest_byte; 32],
            )
            .expect_err(
                "noncanonical STARK transcript labels must be rejected by proof construction",
            );
            assert!(
                err.contains("transcript label"),
                "{hash_label} {label_case} error should mention transcript labels, got: {err}"
            );
            assert_eq!(
                validate_stark_fri_query_shape_and_indices_v1(
                    &params,
                    &invalid_label,
                    &envelope.proof.commits.roots,
                    &extra_query_roots,
                    &envelope.proof.queries,
                )
                .expect_err("query replay must reject noncanonical transcript labels"),
                "FRI transcript label invalid"
            );
            envelope.transcript_label = invalid_label;
            let tampered =
                norito::to_bytes(&envelope).expect("encode noncanonical-label STARK envelope");
            assert!(
                !verify_stark_fri_envelope(&tampered),
                "{hash_label} verifier must reject {label_case} transcript labels"
            );
        }
    }
}
#[test]
fn stark_fri_rejects_noncanonical_circuit_ids() {
    for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
        (
            STARK_HASH_SHA256_V1,
            "sha256",
            "iroha:test:canonical-circuit-id:sha256",
            "stark/fri/sha256-goldilocks:canonical-circuit-id",
            0x61_u8,
        ),
        (
            STARK_HASH_POSEIDON2_V1,
            "poseidon2",
            "iroha:test:canonical-circuit-id:poseidon2",
            "stark/fri/poseidon2-goldilocks:canonical-circuit-id",
            0x62_u8,
        ),
    ] {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn,
            domain_tag: domain_tag.to_owned(),
        };
        let transcript_label = format!("IROHA-TEST-STARK-CANONICAL-CIRCUIT-{hash_label}");
        let bytes = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            transcript_label.clone(),
            circuit_id.to_owned(),
            [digest_byte; 32],
        )
        .expect("valid circuit-id STARK envelope");
        assert!(verify_stark_fri_envelope(&bytes));
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode circuit-id STARK envelope");
        let invalid_circuit_ids = [
            ("empty", String::new()),
            (
                "leading whitespace",
                " stark/fri/sha256-goldilocks:test".to_owned(),
            ),
            (
                "embedded whitespace",
                "stark/fri/sha256 goldilocks:test".to_owned(),
            ),
            (
                "control byte",
                "stark/fri/sha256-goldilocks:\ntest".to_owned(),
            ),
            ("non-ASCII", "stark/fri/sha256-goldilocks:π".to_owned()),
            ("overlong", "c".repeat(MAX_TRANSCRIPT_LABEL_LEN + 1)),
        ];
        for (id_case, invalid_circuit_id) in invalid_circuit_ids {
            let err = prove_stark_fri_air_envelope_bytes(
                params.clone(),
                transcript_label.clone(),
                invalid_circuit_id.clone(),
                [digest_byte; 32],
            )
            .expect_err("noncanonical STARK circuit ids must be rejected by proof construction");
            assert!(
                err.contains("circuit_id"),
                "{hash_label} {id_case} error should mention circuit_id, got: {err}"
            );
            envelope.proof.air.as_mut().expect("AIR section").circuit_id = invalid_circuit_id;
            let tampered =
                norito::to_bytes(&envelope).expect("encode noncanonical-circuit STARK envelope");
            assert!(
                !verify_stark_fri_envelope(&tampered),
                "{hash_label} verifier must reject {id_case} circuit ids"
            );
        }
    }
}
#[test]
fn stark_fri_rejects_noncanonical_domain_tags() {
    for (hash_fn, hash_label, domain_tag, circuit_id, digest_byte) in [
        (
            STARK_HASH_SHA256_V1,
            "sha256",
            "iroha:test:canonical-domain-tag:sha256",
            "stark/fri/sha256-goldilocks:canonical-domain-tag",
            0x71_u8,
        ),
        (
            STARK_HASH_POSEIDON2_V1,
            "poseidon2",
            "iroha:test:canonical-domain-tag:poseidon2",
            "stark/fri/poseidon2-goldilocks:canonical-domain-tag",
            0x72_u8,
        ),
    ] {
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn,
            domain_tag: domain_tag.to_owned(),
        };
        let transcript_label = format!("IROHA-TEST-STARK-CANONICAL-DOMAIN-{hash_label}");
        let bytes = prove_stark_fri_air_envelope_bytes(
            params.clone(),
            transcript_label.clone(),
            circuit_id.to_owned(),
            [digest_byte; 32],
        )
        .expect("valid domain-tag STARK envelope");
        assert!(verify_stark_fri_envelope(&bytes));
        let mut envelope: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode domain-tag STARK envelope");
        let invalid_domain_tags = [
            ("empty", String::new()),
            ("leading whitespace", " iroha:test:domain".to_owned()),
            ("embedded whitespace", "iroha:test domain".to_owned()),
            ("control byte", "iroha:test:\ndomain".to_owned()),
            ("non-ASCII", "iroha:test:π".to_owned()),
            ("overlong", "d".repeat(MAX_DOMAIN_TAG_LEN + 1)),
        ];
        for (tag_case, invalid_domain_tag) in invalid_domain_tags {
            let mut invalid_params = params.clone();
            invalid_params.domain_tag = invalid_domain_tag.clone();
            let err = prove_stark_fri_air_envelope_bytes(
                invalid_params,
                transcript_label.clone(),
                circuit_id.to_owned(),
                [digest_byte; 32],
            )
            .expect_err("noncanonical STARK domain tags must be rejected by proof construction");
            assert!(
                err.contains("domain tag"),
                "{hash_label} {tag_case} error should mention domain tag, got: {err}"
            );
            envelope.params.domain_tag = invalid_domain_tag;
            let tampered =
                norito::to_bytes(&envelope).expect("encode noncanonical-domain STARK envelope");
            assert!(
                !verify_stark_fri_envelope(&tampered),
                "{hash_label} verifier must reject {tag_case} domain tags"
            );
        }
    }
}
#[test]
fn synthesized_envelope_without_air_is_rejected() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:missing-air".to_owned(),
    };
    let bytes =
        synthesize_stark_fri_envelope_bytes(params, "IROHA-TEST-STARK".to_owned()).expect("ok");
    assert!(!verify_stark_fri_envelope(&bytes));
}
#[test]
fn air_envelope_verifies_and_rejects_tampered_opening() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:air".to_owned(),
    };
    let bytes = prove_stark_fri_air_envelope_bytes(
        params,
        "IROHA-TEST-STARK-AIR".to_owned(),
        "stark/fri/sha256-goldilocks:air-test".to_owned(),
        [0x42; 32],
    )
    .expect("air proof");
    assert!(verify_stark_fri_envelope(&bytes));
    let mut envelope: StarkVerifyEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode air envelope");
    let air = envelope.proof.air.as_mut().expect("air section");
    air.openings[0].row[1] ^= 1;
    let tampered = norito::to_bytes(&envelope).expect("encode tampered air envelope");
    assert!(!verify_stark_fri_envelope(&tampered));
}
#[test]
fn explicit_composition_air_envelope_binds_caller_rows_to_fri_queries() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 4,
        blowup_log2: 2,
        fold_arity: 2,
        queries: 2,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:zero-composition-air".to_owned(),
    };
    let domain = 1_usize << usize::from(params.n_log2);
    let rows = (0..domain)
        .map(|index| {
            vec![
                u64::try_from(index).expect("index fits u64"),
                u64::try_from(index * 3 + 1).expect("sample value fits u64"),
                7,
            ]
        })
        .collect::<Vec<_>>();
    let composition_values = vec![0; domain];
    let public_digest = [0x55; 32];
    let circuit_id = "stark/fri/custom-zero-air:test";
    let bytes = prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
        params.clone(),
        "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
        circuit_id.to_owned(),
        public_digest,
        rows.clone(),
        composition_values.clone(),
    )
    .expect("zero-composition AIR envelope");
    assert!(
        verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &public_digest,
            &rows,
            &composition_values,
        )
    );
    assert!(!verify_stark_fri_envelope(&bytes));
    let mut auxiliary_envelope: StarkVerifyEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode zero-composition AIR envelope");
    attach_valid_auxiliary_composition_values(&mut auxiliary_envelope);
    let auxiliary_bytes =
        norito::to_bytes(&auxiliary_envelope).expect("encode auxiliary AIR envelope");
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &auxiliary_bytes,
            circuit_id,
            &public_digest,
            &rows,
            &composition_values,
        ),
        "caller-owned explicit AIR must reject auxiliary generic composition commitments"
    );
    let mut drifted_rows = rows.clone();
    drifted_rows[0][0] ^= 1;
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &public_digest,
            &drifted_rows,
            &composition_values,
        )
    );
    let mut drifted_composition_values = composition_values.clone();
    drifted_composition_values[0] = 1;
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &public_digest,
            &rows,
            &drifted_composition_values,
        )
    );
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            "stark/fri/custom-zero-air:other",
            &public_digest,
            &rows,
            &composition_values,
        )
    );
    let wrong_public_digest = [0x56; 32];
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &wrong_public_digest,
            &rows,
            &composition_values,
        )
    );
    let envelope: StarkVerifyEnvelopeV1 =
        norito::decode_from_bytes(&bytes).expect("decode zero-composition AIR envelope");
    let air = envelope.proof.air.as_ref().expect("AIR section");
    assert_eq!(air.trace_width, 3);
    assert_eq!(air.public_digest, public_digest);
    assert_eq!(air.openings.len(), usize::from(params.queries));
    assert_eq!(
        envelope.proof.commits.roots.first(),
        Some(&air.composition_root)
    );
    let mut malformed_opening_params = envelope.params.clone();
    malformed_opening_params.domain_tag = "iroha:test:bad domain tag".to_owned();
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(
            &malformed_opening_params,
            air,
            air.openings.first().expect("AIR opening")
        )
        .expect_err("opening root replay must reject malformed STARK params"),
        "STARK opening commitment parameters invalid"
    );
    let mut impossible_opening_params = envelope.params.clone();
    impossible_opening_params.blowup_log2 = impossible_opening_params.n_log2 + 1;
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(
            &impossible_opening_params,
            air,
            air.openings.first().expect("AIR opening")
        )
        .expect_err("opening root replay must reject impossible FRI geometry"),
        "STARK opening commitment parameters invalid"
    );
    let extra_query_roots = [air.trace_root, air.composition_root, air.public_digest];
    let mut tight_opening_limits = StarkVerifierLimits::default();
    tight_opening_limits.max_domain_log2 = params.n_log2.saturating_sub(1);
    assert_eq!(
        validate_stark_air_opening_commitment_roots_with_limits_v1(
            &params,
            air,
            air.openings.first().expect("AIR opening"),
            &tight_opening_limits,
        )
        .expect_err("opening root replay must honor caller domain limits"),
        "STARK opening commitment parameters invalid"
    );
    let mut tight_query_limits = StarkVerifierLimits::default();
    tight_query_limits.max_queries = envelope.proof.queries.len().saturating_sub(1);
    assert_eq!(
        validate_stark_fri_query_shape_and_indices_with_limits_v1(
            &envelope.params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &extra_query_roots,
            &envelope.proof.queries,
            &tight_query_limits,
        )
        .expect_err("FRI query replay must honor caller query limits"),
        "FRI parameter/root/query shape mismatch"
    );
    let indices = validate_stark_fri_query_shape_and_indices_v1(
        &envelope.params,
        &envelope.transcript_label,
        &envelope.proof.commits.roots,
        &extra_query_roots,
        &envelope.proof.queries,
    )
    .expect("query shape replays");
    let mut tight_base_index_limits = StarkVerifierLimits::default();
    tight_base_index_limits.max_queries = envelope.proof.queries.len().saturating_sub(1);
    assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values_with_base_indices_with_limits(
                &bytes,
                &tight_base_index_limits,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values,
                &indices,
            ),
            "explicit base-index AIR verifier must honor caller query limits"
        );
    for (opening_number, (opening, index)) in
        air.openings.iter().zip(indices.iter().copied()).enumerate()
    {
        assert_eq!(usize::try_from(opening.index).ok(), Some(index));
        assert_eq!(opening.row, rows[index]);
        assert_eq!(opening.next_row, rows[(index + 1) % domain]);
        assert_eq!(opening.composition_value, 0);
        validate_stark_air_opening_commitment_roots_v1(&params, air, opening)
            .expect("opening binds to trace and composition roots");
        validate_stark_air_opening_first_fri_value_v1(
            opening,
            index,
            envelope.proof.queries[opening_number]
                .first()
                .expect("query chain carries first decommitment"),
        )
        .expect("opening binds to first FRI layer");
    }
    let mut retargeted_opening_index = air.openings.first().expect("AIR opening").clone();
    let original_opening_index =
        usize::try_from(retargeted_opening_index.index).expect("opening index fits usize");
    retargeted_opening_index.index =
        u32::try_from((original_opening_index + 1) % domain).expect("domain fits u32");
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_opening_index)
            .expect_err("opening index retarget must fail before root replay"),
        "opening Merkle path index mismatch"
    );
    let mut retargeted_row_path = air.openings.first().expect("AIR opening").clone();
    retargeted_row_path.row_path.dirs[0] ^= 1;
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_row_path)
            .expect_err("row Merkle path retarget must fail before root replay"),
        "opening Merkle path index mismatch"
    );
    let mut retargeted_next_row_path = air.openings.first().expect("AIR opening").clone();
    retargeted_next_row_path.next_row_path.dirs[0] ^= 1;
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_next_row_path)
            .expect_err("next-row Merkle path retarget must fail before root replay"),
        "opening Merkle path index mismatch"
    );
    let mut retargeted_composition_path = air.openings.first().expect("AIR opening").clone();
    retargeted_composition_path.composition_path.dirs[0] ^= 1;
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(&params, air, &retargeted_composition_path)
            .expect_err("composition Merkle path retarget must fail before root replay"),
        "opening Merkle path index mismatch"
    );
    let mut tampered = envelope;
    let tampered_air = tampered.proof.air.as_mut().expect("AIR section");
    tampered_air.openings[0].row[0] ^= 1;
    let tampered_opening = tampered_air.openings[0].clone();
    assert_eq!(
        validate_stark_air_opening_commitment_roots_v1(&params, tampered_air, &tampered_opening)
            .expect_err("tampered caller row must not match trace root"),
        "row Merkle root mismatch"
    );
    let mut noncanonical_composition = vec![0; domain];
    noncanonical_composition[0] = MOD_P_U64;
    assert_eq!(
        prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            params.clone(),
            "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
            "stark/fri/custom-zero-air:test".to_owned(),
            public_digest,
            rows.clone(),
            noncanonical_composition.clone(),
        )
        .expect_err("non-canonical composition values must be rejected"),
        "STARK AIR composition contains non-canonical field element"
    );
    assert!(
        stark_merkle_root_from_field_values_v1(&params, &noncanonical_composition).is_none(),
        "explicit AIR composition roots must reject non-canonical field elements"
    );
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &public_digest,
            &rows,
            &noncanonical_composition,
        ),
        "explicit AIR verification must reject non-canonical caller composition values"
    );
    let mut noncanonical_rows = rows;
    noncanonical_rows[domain - 1][1] = MOD_P_U64;
    assert_eq!(
        prove_stark_fri_air_envelope_from_rows_and_composition_values_bytes(
            params.clone(),
            "IROHA-TEST-ZERO-COMPOSITION-AIR".to_owned(),
            "stark/fri/custom-zero-air:test".to_owned(),
            public_digest,
            noncanonical_rows.clone(),
            composition_values.clone(),
        )
        .expect_err("non-canonical AIR rows must be rejected"),
        "STARK AIR row contains non-canonical field element"
    );
    assert!(
        stark_air_trace_root_from_rows_v1(&params, &noncanonical_rows).is_none(),
        "explicit AIR trace roots must reject non-canonical row field elements"
    );
    assert!(
        !verify_stark_fri_air_envelope_from_rows_and_composition_values(
            &bytes,
            circuit_id,
            &public_digest,
            &noncanonical_rows,
            &composition_values,
        ),
        "explicit AIR verification must reject non-canonical caller rows"
    );
}
#[test]
fn air_prover_rejects_more_queries_than_domain() {
    let params = StarkFriParamsV1 {
        version: 1,
        n_log2: 1,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 3,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: "iroha:test:repeated-air-query".to_owned(),
    };
    let err = prove_stark_fri_air_envelope_bytes(
        params,
        "IROHA-TEST-REPEATED-AIR-QUERY".to_owned(),
        "stark/fri/custom-repeated-query-air:test".to_owned(),
        [0x71; 32],
    )
    .expect_err("pigeonhole-small AIR query schedule must not emit proof bytes");
    assert!(
        err.contains("STARK query count exceeds domain size"),
        "unexpected impossible-query rejection: {err}"
    );
}
#[test]
fn air_envelope_skips_repeated_transcript_query_indices() {
    let mut params = StarkFriParamsV1 {
        version: 1,
        n_log2: 3,
        blowup_log2: 1,
        fold_arity: 2,
        queries: 4,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag: String::new(),
    };
    let domain = 1_usize << usize::from(params.n_log2);
    let values = vec![Fq::zero(); domain];
    for nonce in 0_u32..1024 {
        params.domain_tag = format!("iroha:test:skip-repeated-query:{nonce}");
        let envelope = synthesize_stark_fri_envelope_from_values(
            params.clone(),
            "IROHA-TEST-SKIP-REPEATED-AIR-QUERY".to_owned(),
            values.clone(),
            &[],
        )
        .expect("duplicate-free query schedule must synthesize");
        let raw_indices = (0..usize::from(params.queries))
            .map(|query_number| {
                derive_query_index(
                    &envelope.transcript_label,
                    &params,
                    &envelope.proof.commits.roots,
                    query_number,
                )
                .expect("query index")
                    % domain
            })
            .collect::<Vec<_>>();
        if !raw_indices
            .iter()
            .enumerate()
            .any(|(index, sampled)| raw_indices[..index].contains(sampled))
        {
            continue;
        }
        let replayed_indices = validate_stark_fri_query_shape_and_indices_v1(
            &params,
            &envelope.transcript_label,
            &envelope.proof.commits.roots,
            &[],
            &envelope.proof.queries,
        )
        .expect("colliding raw transcript samples are mapped without replacement");
        assert_eq!(replayed_indices.len(), usize::from(params.queries));
        assert!(
            !replayed_indices
                .iter()
                .enumerate()
                .any(|(index, sampled)| replayed_indices[..index].contains(sampled)),
            "replayed transcript query indices must be duplicate-free"
        );
        assert_ne!(
            raw_indices, replayed_indices,
            "fixture must exercise collision skipping"
        );
        let public_digest = [0x73; 32];
        let circuit_id = "stark/fri/custom-skip-repeated-query-air:test";
        let rows = (0..domain)
            .map(|index| {
                stark_air_row(index, &public_digest).expect("build duplicate-skip AIR row")
            })
            .collect::<Vec<_>>();
        let composition_values = (0..domain)
            .map(|index| {
                stark_air_composition_value(
                    index,
                    domain,
                    &public_digest,
                    &rows[index],
                    &rows[(index + 1) % domain],
                )
                .expect("build duplicate-skip AIR composition value")
            })
            .collect::<Vec<_>>();
        let composition_values_u64 = composition_values
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>();
        let bytes = prove_stark_fri_air_envelope_from_rows_and_composition_values_fq_bytes(
            params.clone(),
            "IROHA-TEST-SKIP-REPEATED-AIR-QUERY".to_owned(),
            circuit_id.to_owned(),
            public_digest,
            rows.clone(),
            composition_values.clone(),
            None,
        )
        .expect("colliding raw samples must still produce a duplicate-free AIR envelope");
        assert!(
            verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &bytes,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values_u64,
            )
        );
        let mut duplicate_opening: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode duplicate-free AIR envelope");
        duplicate_opening.proof.queries[1] = duplicate_opening.proof.queries[0].clone();
        let first_opening = duplicate_opening
            .proof
            .air
            .as_ref()
            .expect("duplicate-free AIR section")
            .openings[0]
            .clone();
        duplicate_opening
            .proof
            .air
            .as_mut()
            .expect("duplicate-free AIR section")
            .openings[1] = first_opening;
        let duplicate_opening_bytes =
            norito::to_bytes(&duplicate_opening).expect("encode duplicate AIR opening");
        assert!(
            !verify_stark_fri_air_envelope_from_rows_and_composition_values(
                &duplicate_opening_bytes,
                circuit_id,
                &public_digest,
                &rows,
                &composition_values_u64,
            ),
            "duplicate raw query/opening replay must not satisfy skipped transcript samples"
        );
        return;
    }
    panic!("failed to find small-domain transcript query collision fixture");
}
// Full-bootstrap fixtures and remaining rejection profiles stay in this lexical module.
include!("bfv_full_bootstrap_tests.rs");
