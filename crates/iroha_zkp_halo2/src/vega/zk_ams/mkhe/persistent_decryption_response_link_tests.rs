use super::*;
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, sponge::keccak256};
const POINT_WIRE_V1: &str = "8025a4e3128f042d728e58b7e09a51b72585be4435f4e94aac8517f2e158b3eae6";
fn point_v1() -> Point {
    Point::from_non_identity_wire_bytes_exact(&hex::decode(POINT_WIRE_V1).expect("literal hex"))
        .expect("literal canonical T256 point")
}
fn axes_v1() -> ResponseLinkAxesV1 {
    ResponseLinkAxesV1 {
        profile_digest: [0x11; 32],
        roster_digest: [0x22; 32],
        key_context_digest: [0x33; 32],
        cpk_transcript_digest: [0x44; 32],
        decryption_statement_digest: [0x55; 32],
        public_key_first_message_digest: [0x66; 32],
        share_first_message_digest: [0x77; 32],
        epoch: 9,
        party_index: 3,
    }
}
fn challenge_v1() -> SparseChallengeV1 {
    SparseChallengeV1 {
        seed: [0x88; 32],
        terms: core::array::from_fn(|index| SparseChallengeTermV1 {
            shift: (index * 97) as u32,
            sign: if index.is_multiple_of(2) { 1 } else { -1 },
        }),
    }
}
fn source_v1() -> PersistentDecryptionResponseLinkSourceV1 {
    PersistentDecryptionResponseLinkSourceV1 {
        axes: axes_v1(),
        cpk_secret_commitments: [point_v1(); RESPONSE_LINK_CHUNKS_V1],
        seal: ResponseLinkSourceSealV1::TestOnly,
    }
}
fn response_use_v1<'a>(
    source: &'a PersistentDecryptionResponseLinkSourceV1,
    response: &'a [i64],
) -> PersistentDecryptionResponseLinkResponseFixedUseV1<'a> {
    PersistentDecryptionResponseLinkResponseFixedUseV1 {
        challenge_stage: PersistentDecryptionResponseLinkChallengeFixedUseV1 {
            mask_stage: PersistentDecryptionResponseLinkMaskCommittedUseV1 {
                source,
                mask_commitments: [point_v1(); RESPONSE_LINK_CHUNKS_V1],
                seal: ResponseLinkUseSealV1::TestOnly,
            },
            challenge: challenge_v1(),
        },
        secret_response: response,
    }
}
fn literal_negacyclic_product_v1(terms: &[(usize, i64)], witness: &[i64]) -> Vec<i64> {
    let mut output = vec![0_i64; witness.len()];
    for &(shift, sign) in terms {
        for (source, coefficient) in witness.iter().copied().enumerate() {
            let destination = source + shift;
            if destination < witness.len() {
                output[destination] += sign * coefficient;
            } else {
                output[destination - witness.len()] -= sign * coefficient;
            }
        }
    }
    output
}
fn projected_equation_v1(
    beta: Scalar,
    terms: &[SparseChallengeTermV1],
    secret: &[i64],
    mask: &[i64],
    response: &[i64],
) -> Scalar {
    let powers = powers_v1(beta, secret.len());
    let mut result = Scalar::zero();
    for index in 0..secret.len() {
        result += powers[index] * scalar_from_signed_v1(mask[index]);
        result += adjoint_weight_v1(&powers, terms, index).unwrap()
            * scalar_from_signed_v1(secret[index]);
        result -= powers[index] * scalar_from_signed_v1(response[index]);
    }
    result
}
fn structural_proof_wire_v1() -> Vec<u8> {
    let point = hex::decode(POINT_WIRE_V1).expect("literal point");
    let mut wire = vec![0_u8; RESPONSE_LINK_TAIL_BYTES_V1];
    wire[..4].copy_from_slice(&RESPONSE_LINK_WIRE_TAG_V1);
    wire[4] = RESPONSE_LINK_VERSION_V1;
    wire[5] = RESPONSE_LINK_WIRE_FLAGS_V1;
    wire[6] = RESPONSE_LINK_CHUNKS_V1 as u8;
    let mut cursor = RESPONSE_LINK_HEADER_BYTES_V1;
    for _ in 0..RESPONSE_LINK_CHUNKS_V1 {
        wire[cursor..cursor + 33].copy_from_slice(&point);
        cursor += 33;
    }
    for _ in 0..41 {
        wire[cursor..cursor + 33].copy_from_slice(&point);
        cursor += 33;
    }
    cursor += 3 * 32;
    for _ in 0..28 {
        wire[cursor..cursor + 33].copy_from_slice(&point);
        cursor += 33;
    }
    cursor += 2 * 32;
    assert_eq!(cursor, RESPONSE_LINK_TAIL_BYTES_V1);
    wire
}
#[test]
fn independent_tiny_polynomial_oracle_matches_adjoint_equation() {
    let secret = [1, -1, 0, 1, 0, -1, 1, 0];
    let mask = [7, -2, 4, 1, -3, 9, 0, 5];
    let literal_terms = [(0, 1), (2, -1), (7, 1)];
    let terms = literal_terms.map(|(shift, sign)| SparseChallengeTermV1 {
        shift: shift as u32,
        sign: sign as i8,
    });
    let product = literal_negacyclic_product_v1(&literal_terms, &secret);
    let response: Vec<_> = mask.iter().zip(&product).map(|(y, ds)| y + ds).collect();
    let beta = Scalar::from_u64(7);
    assert!(projected_equation_v1(beta, &terms, &secret, &mask, &response).is_zero());
    let mut mutated = response;
    mutated[3] += 1;
    assert!(!projected_equation_v1(beta, &terms, &secret, &mask, &mutated).is_zero());
}
#[test]
fn release_constraint_has_exact_sixteen_vector_openings_and_no_scalars() {
    let response = vec![0_i64; RESPONSE_LINK_RING_DEGREE_V1];
    let constraint = response_link_constraint_v1(Scalar::from_u64(7), challenge_v1(), &response)
        .expect("release response-link constraint");
    assert_eq!(
        constraint.highest_a_index,
        Some(RESPONSE_LINK_CHUNK_COEFFICIENTS_V1 - 1)
    );
    assert_eq!(
        constraint.highest_c_index,
        Some(RESPONSE_LINK_COMMITMENTS_V1 - 1)
    );
    assert_eq!(constraint.highest_v_index, None);
    assert_eq!(constraint.wcg.len(), RESPONSE_LINK_COMMITMENTS_V1);
    assert!(
        constraint
            .wcg
            .iter()
            .all(|opening| opening.len() == RESPONSE_LINK_CHUNK_COEFFICIENTS_V1)
    );
    assert!(constraint.wl.is_empty() && constraint.wr.is_empty() && constraint.wo.is_empty());
    assert!(constraint.wv.is_empty() && constraint.c.is_zero());
}
#[test]
fn zero_divisor_regression_never_inverts_sparse_challenge() {
    // Over F_2, (1+X)(1+X+X^2+X^3)=X^4+1=0.  The adjoint identity
    // remains valid because the response link evaluates D*s and never divides by D.
    let terms = [(0, 1), (1, 1)];
    let witness = [1, 1, 1, 1];
    let product = literal_negacyclic_product_v1(&terms, &witness);
    assert!(product.iter().all(|coefficient| coefficient % 2 == 0));
    let typed = terms.map(|(shift, sign)| SparseChallengeTermV1 {
        shift: shift as u32,
        sign: sign as i8,
    });
    let mask = [3, -1, 2, 4];
    let response: Vec<_> = mask.iter().zip(&product).map(|(y, ds)| y + ds).collect();
    assert!(
        projected_equation_v1(Scalar::from_u64(11), &typed, &witness, &mask, &response,).is_zero()
    );
    assert!(!RESPONSE_LINK_SOUNDNESS_RECORD_V1.4);
}
#[test]
fn signed_small_lift_and_wide_smudge_separation_are_exact() {
    assert_eq!(RESPONSE_LINK_SECRET_MASK_BOUND_V1, 20 * (1 << 24));
    assert_eq!(RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1, 335_544_300);
    assert_eq!(
        RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1 as u64
            + RESPONSE_LINK_SECRET_MASK_BOUND_V1 as u64
            + RESPONSE_LINK_CHALLENGE_WEIGHT_V1 as u64,
        RESPONSE_LINK_INTEGER_LIFT_BOUND_V1
    );
    assert_eq!(
        2 * RESPONSE_LINK_SECRET_RESPONSE_BOUND_V1 as u64
            + 2 * RESPONSE_LINK_CHALLENGE_WEIGHT_V1 as u64,
        RESPONSE_LINK_INTEGER_LIFT_BOUND_V1
    );
    let modulus_high = u64::from_be_bytes(VEGA_T256_SCALAR_MODULUS_BE_V1[..8].try_into().unwrap());
    assert!(RESPONSE_LINK_INTEGER_LIFT_BOUND_V1 < modulus_high);
    assert!(SIGNED_SMALL_SEPARATION_V1.starts_with(b"only signed-i64 secret_response"));
    assert!(
        SIGNED_SMALL_SEPARATION_V1
            .windows(15)
            .any(|window| window == b"smudge_response")
    );
}
#[test]
fn purpose_transcript_codec_kat_is_deterministic_and_order_bound() {
    let expected_header = [
        0x52, 0, 15, b's', b'e', b'c', b'r', b'e', b't', b'-', b'r', b'e', b's', b'p', b'o', b'n',
        b's', b'e', 0, 0, 0, 0, 0, 0x10, 0, 0,
    ];
    assert_eq!(
        frame_header_v1(b"secret-response", RESPONSE_LINK_RING_DEGREE_V1 * 8).unwrap(),
        expected_header
    );
    assert_eq!(
        hex::encode(keccak256(b"abc")),
        "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    );
    let response = vec![0_i64; RESPONSE_LINK_RING_DEGREE_V1];
    let source = source_v1();
    let first_use = response_use_v1(&source, &response);
    let first = ResponseLinkTranscriptSeedV1::new_v1(&first_use).unwrap();
    let second_use = response_use_v1(&source, &response);
    let second = ResponseLinkTranscriptSeedV1::new_v1(&second_use).unwrap();
    assert_eq!(first.beta.to_le_bytes(), second.beta.to_le_bytes());
    assert_eq!(first.binding_digest_v1(), second.binding_digest_v1());
    let mut changed_source = source_v1();
    changed_source.axes.share_first_message_digest[0] ^= 1;
    let changed_use = response_use_v1(&changed_source, &response);
    let changed = ResponseLinkTranscriptSeedV1::new_v1(&changed_use).unwrap();
    assert_ne!(first.binding_digest_v1(), changed.binding_digest_v1());
    assert_ne!(first.beta.to_le_bytes(), changed.beta.to_le_bytes());
    let mut changed_response = response;
    changed_response[RESPONSE_LINK_RING_DEGREE_V1 - 1] = 1;
    let changed_use = response_use_v1(&source, &changed_response);
    let changed = ResponseLinkTranscriptSeedV1::new_v1(&changed_use).unwrap();
    assert_ne!(first.binding_digest_v1(), changed.binding_digest_v1());
}
#[test]
fn exact_proof_codec_kat_rejects_every_boundary_mutation() {
    let wire = structural_proof_wire_v1();
    let proof = PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&wire)
        .expect("structurally canonical proof");
    assert_eq!(proof.wire_v1(), wire.as_slice());
    assert_eq!(proof.core_v1().len(), RESPONSE_LINK_CORE_BYTES_V1);
    for end in [0, 1, RESPONSE_LINK_TAIL_BYTES_V1 - 1] {
        assert!(
            PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&wire[..end])
                .is_err()
        );
    }
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&trailing).is_err());
    for offset in [0, 4, 5, 6] {
        let mut changed = wire.clone();
        changed[offset] ^= 1;
        assert!(
            PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&changed).is_err()
        );
    }
    for offset in [
        RESPONSE_LINK_HEADER_BYTES_V1,
        RESPONSE_LINK_HEADER_BYTES_V1 + RESPONSE_LINK_MASK_COMMITMENT_BYTES_V1,
    ] {
        let mut changed = wire.clone();
        changed[offset..offset + 33].fill(0);
        assert!(
            PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&changed).is_err()
        );
    }
    let scalar_offset =
        RESPONSE_LINK_HEADER_BYTES_V1 + RESPONSE_LINK_MASK_COMMITMENT_BYTES_V1 + 41 * 33;
    let mut noncanonical = VEGA_T256_SCALAR_MODULUS_BE_V1;
    noncanonical.reverse();
    let mut changed = wire;
    changed[scalar_offset..scalar_offset + 32].copy_from_slice(&noncanonical);
    assert!(PersistentDecryptionResponseLinkProofV1::from_wire_bytes_exact_v1(&changed).is_err());
}
#[test]
fn accounting_soundness_and_every_operational_gate_remain_false() {
    assert_eq!(
        RESPONSE_LINK_RESOURCE_RECORD_V1,
        [
            2_437,
            2_708,
            33_032_907,
            521_525,
            34_075_957,
            21_664,
            132_300_000,
            136_038_231,
            9_437_183,
            508_000,
        ]
    );
    assert_eq!(RESPONSE_LINK_SOUNDNESS_RECORD_V1.0, 131_071);
    assert_eq!(RESPONSE_LINK_SOUNDNESS_RECORD_V1.1, 238);
    assert_eq!(RESPONSE_LINK_SOUNDNESS_RECORD_V1.2, 671_088_640);
    assert!(!RESPONSE_LINK_SOUNDNESS_RECORD_V1.3);
    assert!(!RESPONSE_LINK_SOUNDNESS_RECORD_V1.5);
    assert_eq!(
        WORKER_HEAP_CAP_BYTES_V1 - PROVER_PEAK_HEAP_BOUND_BYTES_V1,
        PROVER_HEAP_MARGIN_BYTES_V1
    );
    assert_eq!(
        WORKER_HEAP_CAP_BYTES_V1 - VERIFIER_PEAK_HEAP_BOUND_BYTES_V1,
        VERIFIER_HEAP_MARGIN_BYTES_V1
    );
    assert!(CONDITIONAL_TWO_FORK_ASSUMPTIONS_V1.starts_with(b"ROM forking"));
    assert!(
        CONDITIONAL_TWO_FORK_ASSUMPTIONS_V1
            .windows(20)
            .any(|window| window == b"no CY-to-RNS-first-m")
    );
    for gate in [
        STATE_HOOK_WIRED_V1,
        DIRECT_EQUALITY_VERIFIED_V1,
        ATOMIC_REPLAY_WIRED_V1,
        VERIFIED_RECEIPT_CONSUMED_V1,
        PRODUCTION_RSS_QUALIFIED_V1,
        PRODUCTION_KAT_QUALIFIED_V1,
        ZERO_KNOWLEDGE_ACCEPTED_V1,
        RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
}
#[test]
fn privacy_typestate_order_and_source_budgets_are_static() {
    let production = include_str!("persistent_decryption_response_link.rs");
    let tests = include_str!("persistent_decryption_response_link_tests.rs");
    let parent = include_str!("persistent_decryption_equality.rs");
    assert!(production.lines().count() <= 800);
    assert!(tests.lines().count() <= 400);
    assert!(production.lines().count() + tests.lines().count() <= 1_200);
    assert_eq!(
        parent
            .matches("mod persistent_decryption_response_link;")
            .count(),
        1
    );
    assert_eq!(
        parent
            .matches("persistent_decryption_response_link.rs")
            .count(),
        1
    );
    assert!(!production.contains("pub struct"));
    assert!(!production.contains("pub enum"));
    assert!(!production.contains("impl Clone for PersistentDecryptionResponseLink"));
    assert!(
        production.contains("response: &'a PersistentDecryptionResponseLinkResponseFixedUseV1")
    );
    assert!(production.contains("Vec::new(),\n    )?)"));
    for seal in [
        "enum ResponseLinkSourceSealV1",
        "enum ResponseLinkUseSealV1",
        "enum ResponseLinkIntegrationSealV1",
    ] {
        let body = production
            .split(seal)
            .nth(1)
            .unwrap()
            .split("}\n")
            .next()
            .unwrap();
        assert!(body.contains("Production"));
        assert!(body.contains("Infallible"));
    }
    let order = [
        "b\"cpk-secret-commitments\"",
        "b\"mask-commitments\"",
        "b\"public-key-first-message\"",
        "b\"share-first-message\"",
        "b\"sparse-challenge\"",
        "b\"secret-response\"",
        "b\"response-projection-beta\"",
    ];
    let mut cursor = 0;
    for frame in order {
        let next = production[cursor..]
            .find(frame)
            .expect("required transcript frame order")
            + cursor;
        cursor = next + frame.len();
    }
    assert!(production.contains("does not close persistent-decryption audit bit 7"));
    assert!(production.contains("Production lacks `C^Y` before `D`"));
}
