use super::*;

const PRODUCTION_SOURCE_V2: &str = include_str!("incremental_source_phase23_source_algebra.rs");
const TEST_SOURCE_V2: &str = include_str!("incremental_source_phase23_source_algebra_tests.rs");
const PARENT_SOURCE_V2: &str = include_str!("incremental_source_phase23.rs");
const GLOBAL_LOOKUP_REPLAY_SOURCE_V1: &str =
    include_str!("incremental_source_phase23_source_algebra/global_lookup_source_replay_v1.rs");
const GLOBAL_LOOKUP_REPLAY_TEST_SOURCE_V1: &str = include_str!(
    "incremental_source_phase23_source_algebra/global_lookup_source_replay_v1_tests.rs"
);

#[test]
fn exact_formula_mapping_and_memory_budgets_are_frozen() {
    assert_eq!(SOURCE_ALGEBRA_RECORDS_V2, 43);
    assert_eq!(SOURCE_ALGEBRA_EQUATIONS_V2, 2);
    assert_eq!(SOURCE_ALGEBRA_LIMBS_V2, 38);
    assert_eq!(SOURCE_ALGEBRA_RELATION_COORDINATES_V2, 3_268);
    assert_eq!(SOURCE_ALGEBRA_AGGREGATE_REPETITIONS_V2, 5);
    assert_eq!(SOURCE_ALGEBRA_CHALLENGE_PAIRS_V2, 190);
    assert_eq!(SOURCE_ALGEBRA_PRODUCT_COEFFICIENTS_V2, 262_144);
    assert_eq!(SOURCE_ALGEBRA_P_COEFFICIENTS_V2, 262_144);
    assert_eq!(SOURCE_ALGEBRA_H_COEFFICIENTS_V2, 131_072);
    assert_eq!(SOURCE_ALGEBRA_LOCAL_SCRATCH_BYTES_V2, 28_336_128);
    assert_eq!(SOURCE_ALGEBRA_WHOLE_NAMED_ROOT_BYTES_V2, 83_503_936);
    assert_eq!(
        SOURCE_ALGEBRA_LOCAL_SCRATCH_BYTES_V2,
        27 * PHASE23_RING_DEGREE_V1 * 8 + 3 * 8_192
    );
    assert_eq!(
        SOURCE_ALGEBRA_WHOLE_NAMED_ROOT_BYTES_V2,
        50_383_680 + 4_718_592 + 65_536 + 28_336_128
    );
    assert_eq!(
        ORDINARY_PRODUCT_FORMULA_V2,
        b"T[j,e,l]=ordinary(K[e,l]*r[j,l]);len(T)=2N"
    );
    assert_eq!(QUOTIENT_FORMULA_V2, b"H[j,e,l][i]=T[j,e,l][N+i];H[N-1]=0");
    assert_eq!(
        RELATION_FORMULA_V2,
        b"P=T+p_l*E+delta*M-C=(X^N+1)*H mod q_l"
    );
    assert_eq!(TOP_ZERO_FORMULA_V2, b"P[2N-1]=H[N-1]=0");
    assert_eq!(
        CENTERING_FORMULA_V2,
        b"M_l=m_if_m<=(p-1)/2_else_m-p;then_canonical_mod_q_l"
    );
    assert_eq!(EQUATION_ZERO_FORMULA_V2, b"e=0:K=B,E=e0,C=C0,delta=1");
    assert_eq!(EQUATION_ONE_FORMULA_V2, b"e=1:K=A,E=e1,C=C1,delta=0");
    assert_eq!(
        AGGREGATE_EMISSION_ORDER_V2,
        b"limb->repetition->block->P-low->P-high->H"
    );
}

#[test]
fn exact_record_equation_limb_map_has_one_order_and_one_digest() {
    let first = relation_coordinate_v2(0, 0, 0).unwrap();
    assert_eq!(first.ordinal, 0);
    assert_eq!(first.family, 1);
    assert_eq!(first.family_chunk, 0);
    assert_eq!(first.family_chunk_count, 1);
    assert_eq!(first.logical_value_count, 89);
    assert_eq!(first.equation.equation.tag_v2(), 0);
    assert_eq!(first.equation.key.tag_v2(), 1);
    assert_eq!(first.equation.error.tag_v2(), 1);
    assert_eq!(first.equation.ciphertext.tag_v2(), 1);
    assert_eq!(first.equation.delta, 1);
    assert_eq!(first.modulus, RELEASE_MODULI_V1[0]);

    let second_equation = relation_coordinate_v2(0, 1, 0).unwrap();
    assert_eq!(second_equation.equation.equation.tag_v2(), 1);
    assert_eq!(second_equation.equation.key.tag_v2(), 2);
    assert_eq!(second_equation.equation.error.tag_v2(), 2);
    assert_eq!(second_equation.equation.ciphertext.tag_v2(), 2);
    assert_eq!(second_equation.equation.delta, 0);

    let expected_families = [1_u8, 2, 3, 4, 5, 6];
    let boundary_ordinals = [0_u16, 1, 17, 33, 34, 42];
    for (ordinal, family) in boundary_ordinals.into_iter().zip(expected_families) {
        assert_eq!(
            relation_coordinate_v2(ordinal, 0, 0).unwrap().family,
            family
        );
    }
    let last = relation_coordinate_v2(42, 1, 37).unwrap();
    assert_eq!(last.ordinal, 42);
    assert_eq!(last.family, 6);
    assert_eq!(last.family_chunk, 0);
    assert_eq!(last.family_chunk_count, 1);
    assert_eq!(last.logical_value_count, 512);
    assert_eq!(last.limb, 37);
    assert_eq!(last.modulus, RELEASE_MODULI_V1[37]);
    assert!(relation_coordinate_v2(43, 0, 0).is_err());
    assert!(relation_coordinate_v2(0, 2, 0).is_err());
    assert!(relation_coordinate_v2(0, 0, 38).is_err());

    assert_eq!(
        exact_formula_digest_v2().unwrap(),
        [
            1, 158, 70, 56, 37, 50, 242, 53, 222, 70, 159, 142, 38, 220, 114, 36, 63, 58, 216, 122,
            97, 5, 243, 108, 226, 26, 210, 196, 61, 32, 184, 169,
        ]
    );
    assert_eq!(
        exact_mapping_digest_v2().unwrap(),
        [
            78, 226, 156, 64, 139, 231, 31, 104, 220, 66, 130, 117, 77, 163, 12, 112, 139, 37, 222,
            160, 183, 205, 144, 190, 236, 179, 225, 102, 39, 1, 27, 104,
        ]
    );

    let mut order = core::array::from_fn(|index| index as u16);
    let exact = mapping_digest_for_record_order_v2(&order).unwrap();
    order.swap(0, 1);
    assert_ne!(mapping_digest_for_record_order_v2(&order).unwrap(), exact);
    order[1] = 1;
    assert!(mapping_digest_for_record_order_v2(&order).is_err());
}

const ORACLE_N: usize = 4;
const ORACLE_Q: i64 = 97;
const ORACLE_P: i64 = 17;
const ORACLE_A: [i64; ORACLE_N] = [7, 18, 29, 40];
const ORACLE_B: [i64; ORACLE_N] = [13, 22, 31, 40];

struct OracleRecord {
    r: [i64; ORACLE_N],
    e0: [i64; ORACLE_N],
    e1: [i64; ORACLE_N],
    message: [i64; ORACLE_N],
    c0: [i64; ORACLE_N],
    c1: [i64; ORACLE_N],
}

fn oracle_mod(value: i64) -> i64 {
    value.rem_euclid(ORACLE_Q)
}

fn oracle_pow(mut base: i64, mut exponent: usize) -> i64 {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = oracle_mod(result * base);
        }
        base = oracle_mod(base * base);
        exponent >>= 1;
    }
    result
}

fn oracle_ordinary(left: &[i64; ORACLE_N], right: &[i64; ORACLE_N]) -> [i64; 2 * ORACLE_N] {
    let mut product = [0_i64; 2 * ORACLE_N];
    for (i, left_value) in left.iter().enumerate() {
        for (j, right_value) in right.iter().enumerate() {
            product[i + j] = oracle_mod(product[i + j] + left_value * right_value);
        }
    }
    product
}

fn oracle_negacyclic(left: &[i64; ORACLE_N], right: &[i64; ORACLE_N]) -> [i64; ORACLE_N] {
    let ordinary = oracle_ordinary(left, right);
    let mut product = [0_i64; ORACLE_N];
    for index in 0..ORACLE_N {
        product[index] = oracle_mod(ordinary[index] - ordinary[index + ORACLE_N]);
    }
    product
}

fn oracle_centered_message(record: usize, index: usize, wrong_boundary: bool) -> i64 {
    let value = ((3 * record + 5 * index) % ORACLE_P as usize) as i64;
    let boundary = if wrong_boundary { 9 } else { 8 };
    if value <= boundary {
        value
    } else {
        value - ORACLE_P
    }
}

fn oracle_record(record: usize) -> OracleRecord {
    let r = core::array::from_fn(|index| ((record + index) % 3) as i64 - 1);
    let e0 = core::array::from_fn(|index| ((record + 2 * index) % 5) as i64 - 2);
    let e1 = core::array::from_fn(|index| ((2 * record + index) % 5) as i64 - 2);
    let message = core::array::from_fn(|index| oracle_centered_message(record, index, false));
    let br = oracle_negacyclic(&ORACLE_B, &r);
    let ar = oracle_negacyclic(&ORACLE_A, &r);
    let c0 =
        core::array::from_fn(|index| oracle_mod(br[index] + ORACLE_P * e0[index] + message[index]));
    let c1 = core::array::from_fn(|index| oracle_mod(ar[index] + ORACLE_P * e1[index]));
    OracleRecord {
        r,
        e0,
        e1,
        message,
        c0,
        c1,
    }
}

struct OracleMutation {
    swapped_equations: bool,
    wrong_centering: bool,
    add_ciphertext: bool,
    negacyclic_t: bool,
}

fn oracle_aggregate(
    gamma: i64,
    beta: i64,
    record_order: &[usize; SOURCE_ALGEBRA_RECORDS_V2],
    mutation: OracleMutation,
) -> ([i64; 2 * ORACLE_N], [i64; ORACLE_N]) {
    let mut aggregate_r = [0_i64; ORACLE_N];
    let mut aggregate_error = [0_i64; ORACLE_N];
    let mut aggregate_message = [0_i64; ORACLE_N];
    let mut aggregate_ciphertext = [0_i64; ORACLE_N];
    for (stream_position, record_ordinal) in record_order.iter().copied().enumerate() {
        let record = oracle_record(record_ordinal);
        let weight = oracle_pow(gamma, stream_position);
        for index in 0..ORACLE_N {
            aggregate_r[index] = oracle_mod(aggregate_r[index] + weight * record.r[index]);
            let (error, ciphertext, message_scale) = if mutation.swapped_equations {
                (
                    record.e1[index] + beta * record.e0[index],
                    record.c1[index] + beta * record.c0[index],
                    beta,
                )
            } else {
                (
                    record.e0[index] + beta * record.e1[index],
                    record.c0[index] + beta * record.c1[index],
                    1,
                )
            };
            let message = if mutation.wrong_centering {
                oracle_centered_message(record_ordinal, index, true)
            } else {
                record.message[index]
            };
            aggregate_error[index] = oracle_mod(aggregate_error[index] + weight * error);
            aggregate_message[index] =
                oracle_mod(aggregate_message[index] + weight * message_scale * message);
            aggregate_ciphertext[index] =
                oracle_mod(aggregate_ciphertext[index] + weight * ciphertext);
        }
    }
    let aggregate_key = if mutation.swapped_equations {
        core::array::from_fn(|index| oracle_mod(ORACLE_A[index] + beta * ORACLE_B[index]))
    } else {
        core::array::from_fn(|index| oracle_mod(ORACLE_B[index] + beta * ORACLE_A[index]))
    };
    let product = if mutation.negacyclic_t {
        let low = oracle_negacyclic(&aggregate_key, &aggregate_r);
        let mut padded = [0_i64; 2 * ORACLE_N];
        padded[..ORACLE_N].copy_from_slice(&low);
        padded
    } else {
        oracle_ordinary(&aggregate_key, &aggregate_r)
    };
    let mut relation = product;
    for index in 0..ORACLE_N {
        let ciphertext_term = if mutation.add_ciphertext {
            aggregate_ciphertext[index]
        } else {
            -aggregate_ciphertext[index]
        };
        relation[index] = oracle_mod(
            relation[index]
                + ORACLE_P * aggregate_error[index]
                + aggregate_message[index]
                + ciphertext_term,
        );
    }
    let mut quotient = core::array::from_fn(|index| product[ORACLE_N + index]);
    quotient[ORACLE_N - 1] = 0;
    relation[2 * ORACLE_N - 1] = 0;
    (relation, quotient)
}

fn oracle_relation_is_exact(relation: &[i64; 2 * ORACLE_N], quotient: &[i64; ORACLE_N]) -> bool {
    quotient[ORACLE_N - 1] == 0
        && relation[2 * ORACLE_N - 1] == 0
        && (0..ORACLE_N).all(|index| {
            relation[index] == quotient[index] && relation[ORACLE_N + index] == quotient[index]
        })
}

fn no_oracle_mutation() -> OracleMutation {
    OracleMutation {
        swapped_equations: false,
        wrong_centering: false,
        add_ciphertext: false,
        negacyclic_t: false,
    }
}

#[test]
fn independent_tiny_oracle_pins_all_five_vectors() {
    let order = core::array::from_fn(|index| index);
    let kats = [
        (2, 3, [12, 34, 27, 0, 12, 34, 27, 0], [12, 34, 27, 0]),
        (5, 7, [58, 43, 11, 0, 58, 43, 11, 0], [58, 43, 11, 0]),
        (11, 13, [87, 54, 71, 0, 87, 54, 71, 0], [87, 54, 71, 0]),
        (17, 19, [55, 71, 34, 0, 55, 71, 34, 0], [55, 71, 34, 0]),
        (23, 29, [48, 87, 0, 0, 48, 87, 0, 0], [48, 87, 0, 0]),
    ];
    for (gamma, beta, expected_relation, expected_quotient) in kats {
        let (relation, quotient) = oracle_aggregate(gamma, beta, &order, no_oracle_mutation());
        assert_eq!(relation, expected_relation);
        assert_eq!(quotient, expected_quotient);
        assert!(oracle_relation_is_exact(&relation, &quotient));
    }
}

#[test]
fn hostile_order_top_zero_formula_equation_and_centering_mutations_fail() {
    let order = core::array::from_fn(|index| index);
    let expected = oracle_aggregate(2, 3, &order, no_oracle_mutation());

    let mut reordered = order;
    reordered.swap(0, 1);
    assert_ne!(
        oracle_aggregate(2, 3, &reordered, no_oracle_mutation()),
        expected
    );
    assert_ne!(
        oracle_aggregate(
            2,
            3,
            &order,
            OracleMutation {
                swapped_equations: true,
                ..no_oracle_mutation()
            }
        ),
        expected
    );
    assert_ne!(
        oracle_aggregate(
            2,
            3,
            &order,
            OracleMutation {
                wrong_centering: true,
                ..no_oracle_mutation()
            }
        ),
        expected
    );
    assert_ne!(
        oracle_aggregate(
            2,
            3,
            &order,
            OracleMutation {
                add_ciphertext: true,
                ..no_oracle_mutation()
            }
        ),
        expected
    );
    assert_ne!(
        oracle_aggregate(
            2,
            3,
            &order,
            OracleMutation {
                negacyclic_t: true,
                ..no_oracle_mutation()
            }
        ),
        expected
    );
    assert_eq!(oracle_centered_message(3, 0, false), -8);
    assert_eq!(oracle_centered_message(3, 0, true), 9);

    let (mut bad_relation, quotient) = expected;
    bad_relation[2 * ORACLE_N - 1] = 1;
    assert!(!oracle_relation_is_exact(&bad_relation, &quotient));
    let (relation, mut bad_quotient) = expected;
    bad_quotient[ORACLE_N - 1] = 1;
    assert!(!oracle_relation_is_exact(&relation, &bad_quotient));
}

#[test]
fn production_seals_flags_poison_order_and_privacy_stay_fail_closed() {
    let _ordered = OrderedCiphertextBundleSealV2::TestOnly;
    let _proof = RadixHyraxProofSealV2::TestOnly;
    assert!(!SOURCE_RELATION_POLYNOMIALS_CONSTRUCTED_V2);
    assert!(!SOURCE_ALGEBRA_VERIFIED_V2);
    assert!(!RADIX_PACKING_VERIFIED_V2);
    assert!(!RADIX_CARRY_VERIFIED_V2);
    assert!(!NEGACYCLIC_QUOTIENT_VERIFIED_V2);
    assert!(!PRIVATE_HYRAX_VERIFIED_V2);
    assert!(!Q_PCS_HANDOFF_COMPLETE_V2);
    assert!(!OPERATIONAL_RECEIPT_ACCEPTED_V2);
    assert!(!RELEASE_COMPLETE_V2);

    for impossible_field in [
        "ordered_43_ciphertexts: Infallible",
        "move_only_key_authority: Infallible",
        "packing: Infallible",
        "radix_carry: Infallible",
        "negacyclic_quotient: Infallible",
        "hyrax_bgv_equality: Infallible",
        "authenticated_replay: Infallible",
    ] {
        assert!(PRODUCTION_SOURCE_V2.contains(impossible_field));
    }
    assert!(!PRODUCTION_SOURCE_V2.contains("Phase23ContextCorrespondenceSealV1"));
    assert!(!PRODUCTION_SOURCE_V2.contains("phase23_rns_link_q_pcs"));
    assert!(!PRODUCTION_SOURCE_V2.contains("pub(crate)"));
    assert!(!PRODUCTION_SOURCE_V2.contains("pub fn"));
    assert!(!PRODUCTION_SOURCE_V2.contains("#[derive"));
    assert!(!PRODUCTION_SOURCE_V2.contains("impl core::fmt::Debug"));
    assert!(!PRODUCTION_SOURCE_V2.contains("Encode"));
    assert!(!PRODUCTION_SOURCE_V2.contains("Decode"));
    assert!(!PRODUCTION_SOURCE_V2.contains("into_parts"));
    assert!(!PRODUCTION_SOURCE_V2.contains("as_tuple"));
    assert_eq!(PRODUCTION_SOURCE_V2.matches("pub(super)").count(), 6);

    let preflight = PRODUCTION_SOURCE_V2
        .split("fn preflight_v2")
        .nth(1)
        .unwrap();
    assert!(preflight.starts_with("(mut self)"));
    assert!(
        preflight.find(".take()").unwrap() < preflight.find("exact_manifest_preflight_v2").unwrap()
    );
    let freeze = PRODUCTION_SOURCE_V2.split("fn freeze_v2").nth(1).unwrap();
    assert!(freeze.contains("mut self,"));
    assert!(freeze.find(".take()").unwrap() < freeze.find("live.owner.validate_v1()").unwrap());
    assert!(!PRODUCTION_SOURCE_V2.contains("fn preflight_v2(&mut self"));
    assert!(!PRODUCTION_SOURCE_V2.contains("fn freeze_v2(&mut self"));
    assert!(PARENT_SOURCE_V2.contains("fn into_source_algebra_prerequisite_v2("));
    assert!(PARENT_SOURCE_V2.contains("self,"));
}

#[test]
fn source_and_test_budgets_remain_bounded() {
    assert!(PRODUCTION_SOURCE_V2.lines().count() <= 1_200);
    assert!(TEST_SOURCE_V2.lines().count() <= 650);
    assert!(PRODUCTION_SOURCE_V2.len() <= 52_000);
    assert!(TEST_SOURCE_V2.len() <= 30_000);
    assert!(GLOBAL_LOOKUP_REPLAY_SOURCE_V1.lines().count() <= 800);
    assert!(GLOBAL_LOOKUP_REPLAY_TEST_SOURCE_V1.lines().count() <= 400);
    assert!(
        GLOBAL_LOOKUP_REPLAY_SOURCE_V1.lines().count()
            + GLOBAL_LOOKUP_REPLAY_TEST_SOURCE_V1.lines().count()
            <= 1_200
    );
}
