//! Proof wire contracts and canonical field admission at the codec boundary.

use super::*;

#[test]
fn proof_roundtrip_smoke() {
    let proof = Proof {
        protocol_version: PROTOCOL_VERSION,
        parameter: "fastpq-state-transition-stark-v1".to_string(),
        trace_commitment: digest384(6).into(),
        public_io: PublicIO {
            dsid: [0; 16],
            slot: 42,
            old_root: [1; 32],
            new_root: [2; 32],
            perm_root: [3; 32],
            tx_set_hash: [4; 32],
            ordering_hash: [5; 32],
        },
        trace_root: wire_digest384(7),
        air_trace_root: wire_digest384(8),
        air_composition_root: wire_digest384(9),
        lde_root: wire_digest384(10),
        lde_domain_size: 1,
        lookup_grand_product: 11,
        lookup_challenge: 12,
        alphas: fp4_values(&[13, 14]),
        betas: vec![fp4(15), fp4(16)],
        fri_layers: vec![wire_digest384(17), wire_digest384(18)],
        queries: vec![QueryOpening {
            index: 0,
            value: fp4(123),
            chunk_values: vec![fp4(123)],
            merkle_path: Vec::new(),
        }],
        air_openings: vec![AirConstraintOpening {
            index: 0,
            current_row: vec![1, 2],
            next_row: vec![3, 4],
            current_row_path: Vec::new(),
            next_row_path: Vec::new(),
            composition_value: fp4(456),
            composition_path: Vec::new(),
        }],
        fri_queries: vec![FriQueryOpening {
            initial_index: 0,
            rounds: vec![FriRoundOpening {
                round: 0,
                index: 0,
                values: vec![fp4(456)],
                folded_value: fp4(456),
                merkle_path: Vec::new(),
            }],
            final_index: 0,
            final_values: vec![fp4(456)],
            final_merkle_path: Vec::new(),
        }],
    };
    let first = norito::core::to_bytes(&proof).expect("encode proof");
    let second = norito::core::to_bytes(&proof).expect("re-encode proof deterministically");
    assert_eq!(first, second);
}
#[test]
fn proof_norito_roundtrip_decodes_original() {
    let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
    let encoded = norito::core::to_bytes(&proof).expect("encode proof");
    let decoded: Proof = norito::decode_from_bytes(&encoded).expect("decode proof");
    assert_eq!(decoded, proof);
}
#[test]
fn release_schema_identities_reject_the_pre_release_proof_header() {
    let public_io_schema = norito::core::schema_hash_for_name(PUBLIC_IO_SCHEMA_NAME);
    assert_eq!(
        norito::schema::identity::frame_hash::<PublicIO>(),
        public_io_schema
    );
    assert_eq!(
        <PublicIO as norito::NoritoSchema>::frame_name(),
        PUBLIC_IO_SCHEMA_NAME
    );
    let public_io = PublicIO::default();
    let public_io_bytes = norito::core::to_bytes(&public_io).expect("encode final public IO");
    for retired_name in [
        "fastpq_prover::proof::PublicIO",
        "fastpq_prover::proof::PublicIOV1",
    ] {
        let mut retired = public_io_bytes.clone();
        let retired_schema = norito::core::schema_hash_for_name(retired_name);
        retired[6..22].copy_from_slice(&retired_schema);
        assert!(
            norito::decode_from_bytes::<PublicIO>(&retired).is_err(),
            "retired public-IO schema {retired_name} must not decode as final V1"
        );
    }

    let proof_schema = norito::core::schema_hash_for_name(PROOF_SCHEMA_NAME);
    assert_eq!(
        norito::schema::identity::frame_hash::<Proof>(),
        proof_schema
    );
    assert_eq!(
        <Proof as norito::NoritoSchema>::frame_name(),
        PROOF_SCHEMA_NAME
    );
    let proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
    let encoded = norito::core::to_bytes(&proof).expect("encode release proof");
    assert_eq!(&encoded[6..22], proof_schema.as_slice());
    for retired_name in [
        "fastpq_prover::proof::Proof",
        "fastpq_prover::proof::ProofV1",
    ] {
        let mut retired = encoded.clone();
        let retired_schema = norito::core::schema_hash_for_name(retired_name);
        retired[6..22].copy_from_slice(&retired_schema);
        assert!(
            norito::decode_from_bytes::<Proof>(&retired).is_err(),
            "retired proof schema {retired_name} must not decode as final V1"
        );
    }
}
fn proof_with_every_goldilocks_container() -> Proof {
    let mut proof = materialise_sample_artifact(sample_backend_artifact()).unwrap();
    proof.betas = vec![fp4(23)];
    proof.queries[0].merkle_path = vec![wire_digest384(24)];
    proof.air_openings[0].current_row = vec![25];
    proof.air_openings[0].next_row = vec![26];
    proof.air_openings[0].current_row_path = vec![wire_digest384(27)];
    proof.air_openings[0].next_row_path = vec![wire_digest384(28)];
    proof.air_openings[0].composition_path = vec![wire_digest384(29)];
    proof.fri_queries[0].rounds = vec![FriRoundOpening {
        round: 0,
        index: 0,
        values: vec![fp4(30)],
        folded_value: fp4(31),
        merkle_path: vec![wire_digest384(32)],
    }];
    proof.fri_queries[0].final_merkle_path = vec![wire_digest384(33)];
    validate_canonical_goldilocks_elements(&proof).unwrap();
    proof
}

#[test]
fn proof_roots_and_paths_use_canonical_digest_carriers() {
    let proof = proof_with_every_goldilocks_container();
    fn assert_digest(_: GoldilocksDigest384V1) {}
    fn assert_digest_path(_: &[GoldilocksDigest384V1]) {}

    assert_digest(proof.trace_root);
    assert_digest(proof.air_trace_root);
    assert_digest(proof.air_composition_root);
    assert_digest(proof.lde_root);
    assert_digest_path(&proof.fri_layers);
    assert_digest_path(&proof.queries[0].merkle_path);
    assert_digest_path(&proof.air_openings[0].current_row_path);
    assert_digest_path(&proof.air_openings[0].next_row_path);
    assert_digest_path(&proof.air_openings[0].composition_path);
    assert_digest_path(&proof.fri_queries[0].rounds[0].merkle_path);
    assert_digest_path(&proof.fri_queries[0].final_merkle_path);
}

#[test]
fn proof_norito_decode_rejects_noncanonical_extension_elements() {
    let baseline = proof_with_every_goldilocks_container();
    for location in 0..4 {
        for coefficient in 0..4 {
            let mut proof = baseline.clone();
            let value = match location {
                0 => &mut proof.betas[0],
                1 => &mut proof.fri_queries[0].rounds[0].values[0],
                2 => &mut proof.fri_queries[0].rounds[0].folded_value,
                _ => &mut proof.fri_queries[0].final_values[0],
            };
            let mut coefficients = value.coefficients();
            coefficients[coefficient] = GOLDILOCKS_MODULUS;
            *value = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
            let bytes = norito::core::to_bytes(&proof).expect("encode adversarial proof");
            assert!(
                norito::decode_from_bytes::<Proof>(&bytes).is_err(),
                "Fp4 location {location}, coefficient {coefficient} must fail at wire decode"
            );
        }
    }
}

#[test]
fn canonical_preflight_covers_transcript_scalars() {
    let proof = proof_with_every_goldilocks_container();
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_grand_product", &[], |proof| {
        proof.lookup_grand_product = GOLDILOCKS_MODULUS;
    });
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_challenge", &[], |proof| {
        proof.lookup_challenge = GOLDILOCKS_MODULUS;
    });
    for lane in 0..4 {
        let mut coefficients = [0; 4];
        coefficients[lane] = GOLDILOCKS_MODULUS;
        let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
        assert_noncanonical_goldilocks_rejected(&proof, "alphas", &[0, lane], |proof| {
            proof.alphas[0] = invalid;
        });
        assert_noncanonical_goldilocks_rejected(&proof, "betas", &[0, lane], |proof| {
            proof.betas[0] = invalid;
        });
    }
}

#[test]
fn canonical_preflight_covers_query_values() {
    let proof = proof_with_every_goldilocks_container();
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_grand_product", &[], |proof| {
        proof.lookup_grand_product = GOLDILOCKS_MODULUS;
    });
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_challenge", &[], |proof| {
        proof.lookup_challenge = GOLDILOCKS_MODULUS;
    });
    for lane in 0..4 {
        let mut coefficients = [0; 4];
        coefficients[lane] = GOLDILOCKS_MODULUS;
        let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
        assert_noncanonical_goldilocks_rejected(&proof, "queries.value", &[0, lane], |proof| {
            proof.queries[0].value = invalid;
        });
        assert_noncanonical_goldilocks_rejected(
            &proof,
            "queries.chunk_values",
            &[0, 0, lane],
            |proof| {
                proof.queries[0].chunk_values[0] = invalid;
            },
        );
    }
}

#[test]
fn canonical_preflight_covers_air_values() {
    let proof = proof_with_every_goldilocks_container();
    assert_noncanonical_goldilocks_rejected(&proof, "air_openings.current_row", &[0, 0], |proof| {
        proof.air_openings[0].current_row[0] = GOLDILOCKS_MODULUS
    });
    assert_noncanonical_goldilocks_rejected(&proof, "air_openings.next_row", &[0, 0], |proof| {
        proof.air_openings[0].next_row[0] = GOLDILOCKS_MODULUS
    });
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_grand_product", &[], |proof| {
        proof.lookup_grand_product = GOLDILOCKS_MODULUS;
    });
    assert_noncanonical_goldilocks_rejected(&proof, "lookup_challenge", &[], |proof| {
        proof.lookup_challenge = GOLDILOCKS_MODULUS;
    });
    for lane in 0..4 {
        let mut coefficients = [0; 4];
        coefficients[lane] = GOLDILOCKS_MODULUS;
        let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
        assert_noncanonical_goldilocks_rejected(
            &proof,
            "air_openings.composition_value",
            &[0, lane],
            |proof| proof.air_openings[0].composition_value = invalid,
        );
    }
}

#[test]
fn canonical_preflight_covers_fri_values() {
    let proof = proof_with_every_goldilocks_container();
    assert_noncanonical_goldilocks_rejected(
        &proof,
        "fri_queries.rounds.values",
        &[0, 0, 0, 0],
        |proof| {
            proof.fri_queries[0].rounds[0].values[0] =
                GoldilocksFp4V1::from_coefficients_unchecked_for_test([
                    GOLDILOCKS_MODULUS,
                    0,
                    0,
                    0,
                ]);
        },
    );
    assert_noncanonical_goldilocks_rejected(
        &proof,
        "fri_queries.rounds.folded_value",
        &[0, 0, 0],
        |proof| {
            proof.fri_queries[0].rounds[0].folded_value =
                GoldilocksFp4V1::from_coefficients_unchecked_for_test([
                    GOLDILOCKS_MODULUS,
                    0,
                    0,
                    0,
                ]);
        },
    );
    assert_noncanonical_goldilocks_rejected(
        &proof,
        "fri_queries.final_values",
        &[0, 0, 0],
        |proof| {
            proof.fri_queries[0].final_values[0] =
                GoldilocksFp4V1::from_coefficients_unchecked_for_test([
                    GOLDILOCKS_MODULUS,
                    0,
                    0,
                    0,
                ]);
        },
    );
}
