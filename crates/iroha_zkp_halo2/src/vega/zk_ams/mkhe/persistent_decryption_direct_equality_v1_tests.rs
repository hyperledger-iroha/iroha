use super::*;
use sha2::{Digest as _, Sha256};

const POINT_WIRE_V1: &str = "8025a4e3128f042d728e58b7e09a51b72585be4435f4e94aac8517f2e158b3eae6";

fn sha256_hex_v1(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn point_v1() -> Point {
    Point::from_non_identity_wire_bytes_exact(&hex::decode(POINT_WIRE_V1).expect("literal hex"))
        .expect("literal canonical T256 point")
}

fn digest_v1(value: u8) -> [u8; 32] {
    [value; 32]
}

fn independent_t256_residue_v1(modulus: u64) -> u64 {
    let modulus_wide = u128::from(modulus);
    let radix = (1_u128 << 64) % modulus_wide;
    VEGA_T256_SCALAR_MODULUS_BE_V1
        .chunks_exact(8)
        .fold(0_u64, |accumulator, chunk| {
            let word = u64::from_be_bytes(chunk.try_into().expect("eight-byte chunk"));
            ((u128::from(accumulator) * radix + u128::from(word)) % modulus_wide) as u64
        })
}

fn language_v1() -> PersistentDecryptionDirectEqualityLanguageV1 {
    PersistentDecryptionDirectEqualityLanguageV1 {
        axes: DirectEqualityStatementAxesV1 {
            profile_digest: digest_v1(1),
            roster_digest: digest_v1(2),
            binding_set_root: digest_v1(3),
            collective_public_key_digest: digest_v1(4),
            key_context_digest: digest_v1(5),
            cpk_transcript_digest: digest_v1(6),
            public_contribution_set_digest: digest_v1(7),
            decryption_statement_digest: digest_v1(8),
            ciphertext_digest: digest_v1(9),
            persistent_use_digest: digest_v1(10),
            secret_identity_digest: digest_v1(11),
            generator_basis_digest: digest_v1(12),
            commitment_set_digest: digest_v1(13),
            commitment_context_digest: digest_v1(14),
            persistent_equation_contract_digest: digest_v1(15),
            legacy_short_solution_assumption_digest: digest_v1(16),
            party: ZkAmsMkhePartyIdV1::new(digest_v1(17)).expect("nonzero party"),
            epoch: 18,
            ciphertext_record_index: 19,
            sample_index: 20,
            party_index: 3,
            level: 1,
        },
        persistent_secret_commitments: [point_v1(); DIRECT_EQUALITY_SECRET_COMMITMENTS_V1],
        rns_limbs: core::array::from_fn(|index| DirectEqualityRnsLimbAxisV1 {
            limb_index: u8::try_from(index).expect("38 limbs"),
            modulus: RELEASE_MODULI_V1[index],
            plaintext_modulus_residue: independent_t256_residue_v1(RELEASE_MODULI_V1[index]),
            ciphertext_c1_digest: digest_v1(u8::try_from(index + 20).expect("small index")),
            decryption_share_digest: digest_v1(u8::try_from(index + 70).expect("small index")),
        }),
        artifacts: DirectEqualityArtifactDigestSlotsV1 {
            theorem_digest: digest_v1(110),
            circuit_digest: digest_v1(111),
            backend_digest: digest_v1(112),
        },
        witness_shape: DIRECT_EQUALITY_PRIVATE_WITNESS_SHAPE_V1,
        witness_seal: DirectEqualityWitnessSealV1::TestOnly,
        backend_seal: DirectEqualityBackendSealV1::TestOnly,
        integration_seal: DirectEqualityIntegrationSealV1::TestOnly,
    }
}

fn negacyclic_integer_product_v1(left: &[i64; 4], right: &[i64; 4]) -> [i64; 4] {
    let mut output = [0_i64; 4];
    for (left_index, left_value) in left.iter().copied().enumerate() {
        for (right_index, right_value) in right.iter().copied().enumerate() {
            let destination = left_index + right_index;
            if destination < 4 {
                output[destination] += left_value * right_value;
            } else {
                output[destination - 4] -= left_value * right_value;
            }
        }
    }
    output
}

fn evaluate_mod_v1(polynomial: &[i64; 4], point: i64, modulus: i64) -> i64 {
    polynomial.iter().rev().fold(0, |value, coefficient| {
        (value * point + coefficient).rem_euclid(modulus)
    })
}

#[test]
fn f17_n4_counterexample_decisively_rejects_the_shortcut() {
    let modulus = 17_i64;
    let d = [1_i64, 1, 0, 0];
    let d_prime = [-1_i64, 0, 1, 0];
    let difference = core::array::from_fn(|index| d[index] - d_prime[index]);
    let annihilator = [13_i64, 15, 16, 8];
    let product = negacyclic_integer_product_v1(&difference, &annihilator);

    assert_eq!(difference, [2, 1, -1, 0]);
    assert_eq!(product, [34, 51, 34, 17]);
    assert!(
        difference
            .iter()
            .any(|value| value.rem_euclid(modulus) != 0)
    );
    assert!(
        annihilator
            .iter()
            .any(|value| value.rem_euclid(modulus) != 0)
    );
    assert!(product.iter().all(|value| value.rem_euclid(modulus) == 0));
    assert_eq!((2_i64.pow(4) + 1).rem_euclid(modulus), 0);
    assert_eq!(evaluate_mod_v1(&d, 2, modulus), 3);
    assert_eq!(evaluate_mod_v1(&d_prime, 2, modulus), 3);

    assert_eq!(SHORTCUT_FIELD_MODULUS_V1, modulus);
    assert_eq!(SHORTCUT_D_V1, d);
    assert_eq!(SHORTCUT_D_PRIME_V1, d_prime);
    assert_eq!(SHORTCUT_DIFFERENCE_V1, difference);
    assert_eq!(SHORTCUT_ANNIHILATOR_V1, annihilator);
    assert_eq!(SHORTCUT_INTEGER_PRODUCT_V1, product);
    assert_eq!(SHORTCUT_EVALUATION_POINT_V1, 2);
    assert_eq!(SHORTCUT_COMMON_EVALUATION_V1, 3);

    let mut hostile_annihilator = annihilator;
    hostile_annihilator[0] += 1;
    let hostile_product = negacyclic_integer_product_v1(&difference, &hostile_annihilator);
    assert!(
        hostile_product
            .iter()
            .any(|value| value.rem_euclid(modulus) != 0)
    );
    let mut hostile_d_prime = d_prime;
    hostile_d_prime[0] += 1;
    assert_ne!(evaluate_mod_v1(&hostile_d_prime, 2, modulus), 3);
}

#[test]
fn direct_language_has_exact_axes_and_rejects_axis_mutations() {
    let exact = language_v1();
    assert!(exact.has_exact_frozen_shape_v1());

    let mut wrong_limb = language_v1();
    wrong_limb.rns_limbs[17].limb_index = 18;
    assert!(!wrong_limb.has_exact_frozen_shape_v1());

    let mut wrong_modulus = language_v1();
    wrong_modulus.rns_limbs[0].modulus -= 1;
    assert!(!wrong_modulus.has_exact_frozen_shape_v1());

    let mut wrong_residue = language_v1();
    wrong_residue.rns_limbs[37].plaintext_modulus_residue ^= 1;
    assert!(!wrong_residue.has_exact_frozen_shape_v1());

    let mut missing_artifact = language_v1();
    missing_artifact.artifacts.backend_digest = [0; 32];
    assert!(!missing_artifact.has_exact_frozen_shape_v1());

    let mut split_wide_witness = language_v1();
    split_wide_witness.witness_shape.shared_wide_z_limb_count = 37;
    assert!(!split_wide_witness.has_exact_frozen_shape_v1());
}

#[test]
fn cap_ledger_arithmetic_is_independently_recomputed() {
    let worker_cap = 160_u64 * 1_048_576;
    let proof_cap = 32_u64 * 1_048_576;
    let pointer_bytes = 42_u64;
    let existing_manifest = 498_u64;
    let prover_existing = 77_707_146_u64;
    let verifier_existing = 77_317_655_u64;
    let work_cap = 100_000_000_000_u64;
    let existing_work = 69_492_485_649_u64;

    assert_eq!(worker_cap, 167_772_160);
    assert_eq!(proof_cap, 33_554_432);
    assert_eq!(existing_manifest + pointer_bytes, 540);
    assert_eq!(worker_cap - prover_existing, 90_065_014);
    assert_eq!(worker_cap - verifier_existing, 90_454_505);
    assert_eq!(work_cap - existing_work, 30_507_514_351);

    let ledger = &DIRECT_EQUALITY_CAP_LEDGER_V1;
    assert_eq!(ledger.direct_proof_cap_bytes, proof_cap);
    assert_eq!(
        ledger.future_manifest_bytes,
        existing_manifest + pointer_bytes
    );
    assert_eq!(ledger.future_pointer_ordinal, 3);
    assert_eq!(ledger.future_pointer_bytes, pointer_bytes);
    assert_eq!(ledger.prover_existing_bytes, prover_existing);
    assert_eq!(ledger.prover_remaining_bytes, worker_cap - prover_existing);
    assert_eq!(ledger.verifier_existing_bytes, verifier_existing);
    assert_eq!(
        ledger.verifier_remaining_bytes,
        worker_cap - verifier_existing
    );
    assert_eq!(ledger.remaining_work, work_cap - existing_work);
    assert_eq!(ledger.one_party_max_total_bytes, 106_431_059);
    assert!(ledger.one_party_max_total_bytes <= worker_cap);
    assert!(ledger.sequential_noncoexistence_required);
}

#[test]
fn repetition_fallback_is_over_cap_until_nine_sound_rounds() {
    let per_round_bytes = 33_032_907_u64;
    let pointer_bytes = 42_u64;
    let fallback_manifest_fixed = 456_u64;
    let per_round_work = 69_492_485_649_u64;
    let coordinates = 38_u64 * 131_072;
    let final_choices = 2_u64 * 131_072 - 38;
    let per_round_bits_hundredths = 1_800_u32;
    let loss_hundredths = 2_916_u32;
    let eight_round_bits = 8 * per_round_bits_hundredths - loss_hundredths;
    let nine_round_bits = 9 * per_round_bits_hundredths - loss_hundredths;

    assert_eq!(32_u64 * 1_048_576 - per_round_bytes, 521_525);
    assert_eq!(per_round_bytes * 3, 99_098_721);
    assert_eq!(per_round_bytes * 9, 297_296_163);
    assert_eq!(fallback_manifest_fixed + pointer_bytes * 3, 582);
    assert_eq!(fallback_manifest_fixed + pointer_bytes * 9, 834);
    assert_eq!(per_round_work * 3, 208_477_456_947);
    assert_eq!(per_round_work * 9, 625_432_370_841);
    assert_eq!(coordinates, 4_980_736);
    assert_eq!(final_choices, 262_106);
    assert_eq!(eight_round_bits, 11_484);
    assert_eq!(nine_round_bits, 13_284);
    assert!(eight_round_bits < 12_800);
    assert!(nine_round_bits >= 12_800);
    assert!(116_177_911_296_u64 > 100_000_000_000);

    let ledger = &REPETITION_FALLBACK_LEDGER_V1;
    assert_eq!(ledger.per_round_envelope_bytes, per_round_bytes);
    assert_eq!(ledger.three_round_bytes, per_round_bytes * 3);
    assert_eq!(ledger.three_round_manifest_bytes, 582);
    assert_eq!(ledger.three_round_work, per_round_work * 3);
    assert_eq!(ledger.nine_round_bytes, per_round_bytes * 9);
    assert_eq!(ledger.nine_round_manifest_bytes, 834);
    assert_eq!(ledger.nine_round_work, per_round_work * 9);
    assert_eq!(ledger.three_round_shared_min_work, 116_177_911_296);
    assert_eq!(ledger.coordinates, coordinates);
    assert_eq!(ledger.grinding_attempts, 120);
    assert_eq!(ledger.final_choices, final_choices);
    assert_eq!(ledger.eight_round_bits_hundredths, eight_round_bits);
    assert_eq!(ledger.nine_round_bits_hundredths, nine_round_bits);
    assert!(!FALLBACK_EIGHT_ROUNDS_SUFFICIENT_V1);
    assert!(FALLBACK_NINE_ROUNDS_SUFFICIENT_V1);
}

#[test]
fn every_artifact_and_operational_gate_remains_closed() {
    assert_eq!(UNPINNED_ARTIFACT_DIGEST_SLOTS_V1.theorem_digest, [0; 32]);
    assert_eq!(UNPINNED_ARTIFACT_DIGEST_SLOTS_V1.circuit_digest, [0; 32]);
    assert_eq!(UNPINNED_ARTIFACT_DIGEST_SLOTS_V1.backend_digest, [0; 32]);
    for gate in [
        THEOREM_PINNED_V1,
        CIRCUIT_PINNED_V1,
        BACKEND_IMPLEMENTED_V1,
        INTEGRATION_WIRED_V1,
        DIRECT_EQUALITY_VERIFIED_V1,
        ATOMIC_REPLAY_WIRED_V1,
        VERIFIED_RECEIPT_CONSUMED_V1,
        PRODUCTION_KAT_QUALIFIED_V1,
        PRODUCTION_RSS_QUALIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        PERSISTENT_DECRYPTION_AUDIT_BIT_7_CLOSED_V1,
        RELEASE_READY_V1,
        RESPONSE_LINK_AUTHORIZES_DIRECT_EQUALITY_V1,
        D_INVERSION_PERMITTED_V1,
        CYCLOTOMIC_CANCELLATION_PERMITTED_V1,
    ] {
        assert!(!gate);
    }
    assert!(SEQUENTIAL_NONCOEXISTENCE_REQUIRED_V1);
    assert!(SEQUENTIAL_NONCOEXISTENCE_CONTRACT_V1.starts_with(
        b"the direct-equality backend stage and the existing staged-decryption prover peak"
    ));
    assert!(
        DIRECT_EQUALITY_LANGUAGE_V1
            .windows(8)
            .any(|word| word == b"ternary ")
    );
    assert!(
        DIRECT_EQUALITY_LANGUAGE_V1
            .windows(24)
            .any(|word| word == b"same s and z are used in")
    );
    assert!(
        RESPONSE_LINK_STATUS_V1.starts_with(b"persistent_decryption_response_link is auxiliary")
    );
}

#[test]
fn frozen_inputs_and_parent_declaration_are_exact() {
    let expected_parent = "370c605f7d740f1b91310942999ab2690d1c29f21496d53d7090cc0130e3e64d";
    let expected_response_link = "cdc5aabf77ed20abf402b3921d73471fb315bd77b21466eb95880cfa84d97530";
    let expected_response_link_tests =
        "03e59fb4bd35ca3de976e93da598d3a392926ff01ea9b6b3005dad65b69d6efd";
    const DECLARATION: &str = concat!(
        "#[path = \"persistent_decryption_direct_equality_v1.rs\"]\n",
        "mod persistent_decryption_direct_equality_v1;\n",
        "\n",
    );
    let parent = include_str!("persistent_decryption_equality.rs");
    let response_link = include_str!("persistent_decryption_response_link.rs");
    let response_link_tests = include_str!("persistent_decryption_response_link_tests.rs");

    assert_eq!(parent.matches(DECLARATION).count(), 1);
    let restored_parent = parent.replacen(DECLARATION, "", 1);
    assert_eq!(PARENT_BEFORE_DECLARATION_SHA256_V1, expected_parent);
    assert_eq!(sha256_hex_v1(restored_parent.as_bytes()), expected_parent);
    assert_eq!(AUXILIARY_RESPONSE_LINK_SHA256_V1, expected_response_link);
    assert_eq!(
        sha256_hex_v1(response_link.as_bytes()),
        expected_response_link
    );
    assert_eq!(
        AUXILIARY_RESPONSE_LINK_TESTS_SHA256_V1,
        expected_response_link_tests
    );
    assert_eq!(
        sha256_hex_v1(response_link_tests.as_bytes()),
        expected_response_link_tests
    );
}

#[test]
fn privacy_api_and_source_budgets_are_fail_closed() {
    let production = include_str!("persistent_decryption_direct_equality_v1.rs");
    let tests = include_str!("persistent_decryption_direct_equality_v1_tests.rs");
    assert!(production.lines().count() <= 700);
    assert!(tests.lines().count() <= 450);
    assert!(production.lines().count() + tests.lines().count() <= 1_150);

    let code_lines = production
        .lines()
        .map(str::trim_start)
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>();
    assert!(!code_lines.iter().any(|line| line.starts_with("pub ")));
    assert!(!code_lines.iter().any(|line| line.starts_with("pub(")));
    assert!(!production.contains("derive(Clone"));
    assert!(!production.contains("impl Clone"));
    assert!(!production.contains("Vec<"));
    assert!(!production.contains("Vec::"));
    assert!(!production.contains("raw_"));
    assert!(!production.contains("from_wire"));
    assert!(!production.contains("to_wire"));
    assert!(!production.contains("Encode"));
    assert!(!production.contains("Decode"));
    assert!(!production.contains("PersistentDecryptionDirectEqualityProofV1"));
    assert!(!production.contains("VerifiedPersistentDecryptionDirectEqualityReceiptV1"));
    assert!(!production.contains("fn verify_direct_equality"));

    for seal in [
        "enum DirectEqualityWitnessSealV1",
        "enum DirectEqualityBackendSealV1",
        "enum DirectEqualityIntegrationSealV1",
    ] {
        let body = production
            .split(seal)
            .nth(1)
            .expect("seal declaration")
            .split("}\n")
            .next()
            .expect("seal body");
        assert!(body.contains("Production"));
        assert!(body.contains("Infallible"));
    }
    assert!(production.contains("auxiliary and\n//! non-authorizing"));
    assert!(production.contains("does not close persistent-decryption audit bit 7"));
    assert_eq!(production.matches("mod tests;").count(), 1);
}
