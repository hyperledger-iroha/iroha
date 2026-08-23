use super::*;

use halo2_base::halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{ConstraintSystem, keygen_vk},
    poly::{Rotation, commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use halo2_base::utils::modulus;
use zeroize::Zeroize;

use crate::zk::pasta_ipa_recursion::{PastaIpaInstanceQueryV1, pasta_ipa_augmented_proof_shape_v1};

#[derive(Clone, Debug)]
struct TestVector {
    sec1: [u8; 65],
    digest: [u8; 32],
    signature: [u8; 64],
}

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("fixture is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("fixture has exactly {N} bytes"))
}

fn rfc6979_sample() -> TestVector {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let digest =
        decode_hex::<32>("AF2BDBE1AA9B6EC1E2ADE1D694F41FC71A831D0268E9891562113D8A62ADD1BF");
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let low_s =
        decode_hex::<32>("0834E36AD29A83BF2BC9385E491D6099C8FDF9D1ED67AA7EA5F51F93782857A9");
    let mut sec1 = [0_u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&x);
    sec1[33..].copy_from_slice(&y);
    let mut signature = [0_u8; 64];
    signature[..32].copy_from_slice(&r);
    signature[32..].copy_from_slice(&low_s);
    TestVector {
        sec1,
        digest,
        signature,
    }
}

fn assert_configured_shape<F: BigPrimeField>() {
    let mut meta = ConstraintSystem::<F>::default();
    let _ = P256PackedAffineConfigV2::configure(&mut meta);
    assert_eq!(meta.degree(), 7);
    assert_eq!(meta.num_advice_columns(), 8);
    assert_eq!(meta.num_instance_columns(), 1);
    assert_eq!(meta.num_fixed_columns(), 4);
    assert_eq!(meta.num_selectors(), 0);
    assert_eq!(meta.advice_queries().len(), 8);
    assert_eq!(meta.instance_queries().len(), 1);
    assert_eq!(meta.fixed_queries().len(), 4);
    assert!(
        meta.advice_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.instance_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert!(
        meta.fixed_queries()
            .iter()
            .all(|(_, rotation)| *rotation == Rotation::cur())
    );
    assert_eq!(meta.permutation().get_columns().len(), 8);
    assert_eq!(meta.lookups().len(), 2);
    let shape = pasta_ipa_augmented_proof_shape_v1(&meta, K, PastaIpaInstanceQueryV1::Direct)
        .expect("shared exact shape calculator");
    assert_eq!(shape.commitments(), 57);
    assert_eq!(shape.evaluations(), 42);
    assert_eq!(shape.point_sets(), 4);
    assert_eq!(shape.augmented_proof_bytes(), 3_200);
    assert_eq!(
        P256_PACKED_AFFINE_SHAPE_V2.augmented_proof_bytes,
        usize::try_from(shape.augmented_proof_bytes()).expect("proof byte count fits usize")
    );
}

#[test]
fn configured_shape_is_current_query_degree_seven_and_exactly_3200_bytes() {
    assert_configured_shape::<Fp>();
    assert_configured_shape::<Fq>();
}

fn assert_two_coordinate_range_sound<F: BigPrimeField>() {
    for source_bits in RANGE_CHUNK_BITS {
        let source_tag = F::from(u64::try_from(source_bits).expect("range tag fits u64"));
        for table_bits in RANGE_CHUNK_BITS {
            let table_tag = F::from(u64::try_from(table_bits).expect("range tag fits u64"));
            for integer in [0_u64, 1, (1_u64 << table_bits) - 1] {
                let table_value = F::from(integer);
                let candidate = (table_tag * table_value) * source_tag.invert().unwrap();
                let first_matches = source_tag * candidate == table_tag * table_value;
                let second_matches =
                    source_tag * source_tag * candidate == table_tag * table_tag * table_value;
                assert!(first_matches);
                assert_eq!(
                    second_matches,
                    integer == 0 || source_bits == table_bits,
                    "fractional cross-tag value must fail the second coordinate"
                );
                if second_matches {
                    assert_eq!(candidate, table_value);
                }
            }
        }
    }
    let arbitrary = F::from(0x1234_u64);
    assert_eq!(F::ZERO * arbitrary, F::ZERO);
    assert_eq!(F::ZERO * F::ZERO * arbitrary, F::ZERO);
}

#[test]
fn two_coordinate_typed_range_tuple_is_field_sound() {
    assert_two_coordinate_range_sound::<Fp>();
    assert_two_coordinate_range_sound::<Fq>();
    assert_eq!(
        TABLE_ROWS,
        1 + RANGE_CHUNK_BITS
            .into_iter()
            .map(|bits| 1_usize << bits)
            .sum::<usize>()
    );
    assert!(TABLE_ROWS < K16_MAX_ASSIGNED_ROWS);
}

fn selector_value<F: BigPrimeField>(opcode: u64, roots: &[u64]) -> F {
    roots.iter().fold(F::ONE, |value, root| {
        value * (F::from(opcode) - F::from(*root))
    })
}

fn machine_residuals<F: BigPrimeField>(
    v: [F; ADVICE_COLUMNS],
    public: F,
    opcode: u64,
    tag: u64,
) -> Vec<F> {
    let q_bind = selector_value::<F>(opcode, &Q_BIND_ROOTS);
    let q_range = F::from(tag) * (F::from(opcode) - F::from(3_u64));
    let q_sparse = selector_value::<F>(opcode, &Q_SPARSE_ROOTS);
    let q_dense = selector_value::<F>(opcode, &Q_DENSE_ROOTS);
    let q_select = selector_value::<F>(opcode, &Q_SELECT_ROOTS);
    let q_sign = selector_value::<F>(opcode, &Q_SIGN_ROOTS);
    let range_recomposition = v[0]
        + v[1] * F::from(1_u64 << 15)
        + v[2] * F::from(1_u64 << 30)
        + v[3] * F::from(1_u64 << 45)
        + v[4] * biguint_to_fe::<F>(&(BigUint::from(1_u8) << 60_usize))
        + v[5] * biguint_to_fe::<F>(&(BigUint::from(1_u8) << 75_usize))
        - v[6];
    let sparse = v[3] * v[1] * v[2] + v[6] - v[5];
    let dense = v[0] * v[1] + v[2] * v[3] + v[4] * v[7] + v[6] - v[5];
    let select_zero = v[0] + v[6] * (v[1] - v[0]) - v[2];
    let select_one = v[3] + v[6] * (v[4] - v[3]) - v[5];
    let sign_zero = v[5] - v[2] * (F::ONE - F::from(2_u64) * v[3]);
    let sign_one = v[4] - v[0] * (F::ONE - F::from(2_u64) * v[3]);
    vec![
        q_bind * (v[7] - public),
        q_range * range_recomposition,
        q_range * (F::ONE - v[7]) * v[6],
        q_sparse * sparse,
        q_dense * dense,
        q_select * select_zero,
        q_select * select_one,
        q_sign * sign_zero,
        q_sign * sign_one,
        q_sign * v[3] * (v[3] - F::ONE),
        q_sign * (F::ONE - v[7]) * v[2],
        q_sign * (F::ONE - v[7]) * v[0],
        q_sign * (F::ONE - v[7]) * v[3],
    ]
}

fn assert_zero_residuals<F: BigPrimeField>(residuals: &[F], context: &str) {
    assert!(
        residuals.iter().all(|residual| *residual == F::ZERO),
        "nonzero packed-machine residual in {context}"
    );
}

fn assert_selector_and_overlap_invariants<F: BigPrimeField>() {
    let expected = [
        [false, false, false, false, false],
        [true, true, true, true, true],
        [false, false, false, false, false],
        [false, true, false, false, false],
        [false, false, true, false, false],
        [false, false, false, true, false],
        [true, true, true, true, true],
        [false, true, false, false, true],
    ];
    for opcode in 0_u64..=7 {
        let actual = [
            selector_value::<F>(opcode, &Q_BIND_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SPARSE_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_DENSE_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SELECT_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SIGN_ROOTS) != F::ZERO,
        ];
        assert_eq!(
            actual,
            expected[usize::try_from(opcode).expect("opcode fits usize")]
        );
    }

    // Fixed-zero unusable/blinding rows may contain arbitrary poisoned advice
    // and an arbitrary queried instance. Every gate selector and both lookup
    // coordinates must nevertheless be zero.
    let poisoned = std::array::from_fn(|index| {
        F::from(u64::try_from(0x1234_usize + 17 * index).expect("poison fits u64"))
    });
    assert_zero_residuals(
        &machine_residuals(poisoned, F::from(0xfeed_u64), 0, 0),
        "fixed-zero poisoned row",
    );
    for lane in [poisoned[0], poisoned[4]] {
        assert_eq!(F::ZERO * lane, F::ZERO);
        assert_eq!(F::ZERO * F::ZERO * lane, F::ZERO);
    }

    // Bind deliberately overlaps every semantic selector except Range. Only
    // v7 is populated, so all overlapping raw relations are identities.
    let public = F::from(0xbeef_u64);
    let mut bind = [F::ZERO; ADVICE_COLUMNS];
    bind[7] = public;
    assert_zero_residuals(&machine_residuals(bind, public, 1, 0), "Bind overlap");

    // A lookup-bearing Sparse row zeros q_range through (opcode - 3).
    let mut sparse = [F::ZERO; ADVICE_COLUMNS];
    sparse[0] = F::from(17_u64);
    sparse[1] = F::from(3_u64);
    sparse[2] = F::from(5_u64);
    sparse[3] = F::from(7_u64);
    sparse[4] = F::from(19_u64);
    sparse[6] = F::from(11_u64);
    sparse[5] = sparse[3] * sparse[1] * sparse[2] + sparse[6];
    assert_zero_residuals(
        &machine_residuals(sparse, F::ZERO, 3, 15),
        "Sparse/Range-tag overlap",
    );

    // Sign deliberately overlaps Sparse. Lane zero is precisely the Sparse
    // cubic sign*(-2)*m + m - signed; lane one shares sign and active.
    for sign_value in [false, true] {
        let sign = F::from(u64::from(sign_value));
        let first = F::from(29_u64);
        let second = F::from(31_u64);
        let signed_first = if sign_value { -first } else { first };
        let signed_second = if sign_value { -second } else { second };
        let sign_row = [
            second,
            -F::from(2_u64),
            first,
            sign,
            signed_second,
            signed_first,
            first,
            F::ONE,
        ];
        assert_zero_residuals(
            &machine_residuals(sign_row, F::ZERO, 7, 0),
            "Sign/Sparse overlap",
        );
    }
    let inactive_sign = [
        F::ZERO,
        -F::from(2_u64),
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
    ];
    assert_zero_residuals(
        &machine_residuals(inactive_sign, F::ZERO, 7, 0),
        "inactive Sign zeroization",
    );
    assert_zero_residuals(
        &machine_residuals([F::ZERO; ADVICE_COLUMNS], F::ZERO, 2, 15),
        "inactive Range zeroization",
    );

    // Wide carry rows use the Dense opcode and equation exactly.
    let left = F::from(41_u64);
    let right = F::from(43_u64);
    let carry_in = F::ONE;
    let carry_out = F::ZERO;
    let constant = left + right + carry_in;
    let wide_as_dense = [
        left,
        F::ONE,
        right,
        F::ONE,
        carry_out,
        constant,
        carry_in,
        -biguint_to_fe::<F>(&radix()),
    ];
    assert_zero_residuals(
        &machine_residuals(wide_as_dense, F::ZERO, 4, 0),
        "Wide-as-Dense overlap",
    );
}

#[test]
fn selector_truth_table_and_all_opcode_overlaps_are_field_sound() {
    assert_selector_and_overlap_invariants::<Fp>();
    assert_selector_and_overlap_invariants::<Fq>();
}

#[derive(Clone, Debug, Default)]
struct PoisonedFixedZeroCircuit<F>(PhantomData<F>);

impl<F: BigPrimeField> Circuit<F> for PoisonedFixedZeroCircuit<F> {
    type Config = P256PackedAffineConfigV2;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256PackedAffineConfigV2::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "poisoned fixed-zero row",
            |mut region| {
                for (column, advice) in config.advice.iter().enumerate() {
                    raw_assign_advice(
                        &mut region,
                        *advice,
                        0,
                        Value::known(Assigned::Trivial(F::from(
                            u64::try_from(0x7000_usize + column).expect("poison fits u64"),
                        ))),
                    );
                }
                raw_assign_fixed(&mut region, config.opcode, 0, F::ZERO);
                raw_assign_fixed(&mut region, config.range_tag, 0, F::ZERO);
                raw_assign_fixed(&mut region, config.table_first, 0, F::ZERO);
                raw_assign_fixed(&mut region, config.table_second, 0, F::ZERO);
                Ok(())
            },
        )
    }
}

#[test]
fn configured_machine_accepts_poisoned_advice_when_all_fixed_controls_are_zero() {
    for verify in [
        MockProver::run(5, &PoisonedFixedZeroCircuit::<Fp>::default(), vec![vec![]])
            .expect("Fp poisoned-row synthesis")
            .verify()
            .is_ok(),
        MockProver::run(5, &PoisonedFixedZeroCircuit::<Fq>::default(), vec![vec![]])
            .expect("Fq poisoned-row synthesis")
            .verify()
            .is_ok(),
    ] {
        assert!(verify, "fixed-zero controls must annihilate every relation");
    }
}

#[test]
fn unsupported_range_width_fails_before_it_can_zero_the_range_selector() {
    fn fail<F: BigPrimeField>() {
        let mut builder = PackedBuilder::<F>::new();
        let active = builder.constant_bool(true);
        let _zero = builder.zero();
        builder
            .bounded(BigUint::from(0_u8), 7, &active, "unsupported test width")
            .expect("host bit bound is otherwise valid");
        assert!(matches!(
            builder.finish(),
            Err(P256PackedAffineFailureV2::Source(
                "range chunk width has no typed table tag"
            ))
        ));
    }
    fail::<Fp>();
    fail::<Fq>();
}

#[test]
fn complete_affine_host_cases_cover_identity_inverse_double_and_y_zero() {
    let identity = AffineValue::identity();
    let generator = AffineValue::generator();
    assert_eq!(affine_add_value(&identity, &generator).0, generator);
    assert_eq!(affine_add_value(&generator, &identity).0, generator);
    let modulus = modulus_base();
    let negative = AffineValue {
        x: generator.x.clone(),
        y: modular_sub(&BigUint::from(0_u8), &generator.y, &modulus),
        infinity: false,
    };
    assert!(affine_add_value(&generator, &negative).0.infinity);
    assert_eq!(
        affine_add_value(&generator, &generator).0,
        affine_double_value(&generator).0
    );
    let exceptional = AffineValue {
        x: BigUint::from(1_u8),
        y: BigUint::from(0_u8),
        infinity: false,
    };
    assert!(affine_double_value(&exceptional).0.infinity);
}

#[derive(Clone)]
struct ExactSource {
    statement: [u8; PUBLIC_BYTES],
    fail: bool,
}

impl P256PackedStatementSourceV2 for ExactSource {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; PUBLIC_BYTES],
    ) -> Result<(), &'static str> {
        if self.fail {
            destination.zeroize();
            return Err("injected source failure");
        }
        destination.copy_from_slice(&self.statement);
        self.statement.zeroize();
        Ok(())
    }
}

#[test]
fn exact_source_is_single_frame_and_failure_is_closed() {
    let vector = rfc6979_sample();
    let direct =
        P256PackedAffineEcdsaCircuitV2::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let source = ExactSource {
        statement: direct.input_bytes(),
        fail: false,
    };
    let sourced =
        P256PackedAffineEcdsaCircuitV2::<Fp>::from_source(source).expect("exact source succeeds");
    assert_eq!(sourced, direct);
    let failing = ExactSource {
        statement: direct.input_bytes(),
        fail: true,
    };
    assert!(P256PackedAffineEcdsaCircuitV2::<Fp>::from_source(failing).is_err());
}

fn row_preflight<F: BigPrimeField>() -> P256PackedAffineRowsV2 {
    let vector = rfc6979_sample();
    let circuit =
        P256PackedAffineEcdsaCircuitV2::<F>::new(vector.sec1, vector.digest, vector.signature);
    let rows = circuit
        .trace_diagnostic_for_test()
        .expect("packed builder produces an exact diagnostic");
    assert_eq!(
        V2_MINIMUM_CUBIC_SPARSE_ROWS,
        72 * 263 + 90 * 135 + 9 + 3 * 9
    );
    assert_eq!(
        V2_MINIMUM_RANGE_ROWS,
        7 * 1_334 + 3 * 1_340 + (9 * 263 + 12 * 135 + 6 * 3) + 196
    );
    assert_eq!(V2_MINIMUM_QUOTIENT_CARRY_SIGN_ROWS, 6 * 1_334);
    assert_eq!(V2_MINIMUM_SELECTION_ROWS, 13_950_usize.div_ceil(2));
    assert_eq!(V2_MINIMUM_CANONICAL_WIDE_ROWS, 3 * 1_340);
    assert_eq!(P256_PACKED_AFFINE_V2_STATIC_MINIMUM_ROWS, 67_841);
    assert!(
        P256_PACKED_AFFINE_V2_STATIC_MINIMUM_ROWS > K16_MAX_ASSIGNED_ROWS,
        "the source-derived lower bound must keep V2 closed"
    );
    assert!(
        rows.total_rows >= P256_PACKED_AFFINE_V2_STATIC_MINIMUM_ROWS,
        "{rows:#?}"
    );
    assert!(rows.total_rows > K16_MAX_ASSIGNED_ROWS, "{rows:#?}");
    assert!(circuit.row_report().is_err(), "V2 must fail closed at k=16");
    assert_eq!(
        rows.semantic_rows,
        rows.binding_rows
            + rows.range_rows
            + rows.sparse_rows
            + rows.lookup_only_rows
            + rows.dense_rows
            + rows.wide_rows
            + rows.sign_rows
            + rows.selection_rows
    );
    assert_eq!(rows.total_rows, rows.semantic_rows.max(rows.table_rows));
    assert_eq!(rows.table_rows, TABLE_ROWS);
    assert_eq!(rows.caller_instance_rows, PUBLIC_BYTES);
    assert_eq!(
        rows.binding_rows,
        rows.caller_instance_rows + rows.constant_instance_rows
    );
    assert_eq!(rows.complete_doublings, 263);
    assert_eq!(rows.complete_additions, 135);
    assert_eq!(rows.modular_relations, 1_334);
    assert!(rows.maximum_quotient_bits <= QUOTIENT_BITS);
    assert!(rows.maximum_carry_bits <= CARRY_BITS);
    assert!(rows.maximum_coefficient_bits <= PACKED_COEFFICIENT_BOUND_BITS);
    assert!(
        (BigUint::from(1_u8) << PACKED_COEFFICIENT_BOUND_BITS) < modulus::<F>(),
        "static integer lift must fit natively"
    );
    rows
}

#[test]
fn rfc6979_row_preflight_proves_v2_infeasible_on_both_pasta_fields() {
    let fp_rows = row_preflight::<Fp>();
    let fq_rows = row_preflight::<Fq>();
    assert_eq!(fp_rows, fq_rows, "Pasta parity must not change topology");
}

fn instance_partition<F: BigPrimeField>() -> (Vec<F>, Vec<F>) {
    let vector = rfc6979_sample();
    P256PackedAffineEcdsaCircuitV2::<F>::new(vector.sec1, vector.digest, vector.signature)
        .instance_partition_for_test()
        .expect("instance contract is derivable before transpose")
}

#[test]
fn instance_contract_is_161_caller_bytes_plus_a_witness_independent_constant_tail() {
    let vector = rfc6979_sample();
    let (fp_caller, fp_tail) = instance_partition::<Fp>();
    let (fq_caller, fq_tail) = instance_partition::<Fq>();
    assert_eq!(fp_caller.len(), PUBLIC_BYTES);
    assert_eq!(fq_caller.len(), PUBLIC_BYTES);
    assert_eq!(fp_tail.len(), fq_tail.len());
    for (index, byte) in vector
        .sec1
        .into_iter()
        .chain(vector.digest)
        .chain(vector.signature)
        .enumerate()
    {
        assert_eq!(fp_caller[index], Fp::from(u64::from(byte)));
        assert_eq!(fq_caller[index], Fq::from(u64::from(byte)));
    }

    let default_fp = P256PackedAffineEcdsaCircuitV2::<Fp>::default()
        .instance_partition_for_test()
        .expect("default witness retains the fixed topology");
    let default_fq = P256PackedAffineEcdsaCircuitV2::<Fq>::default()
        .instance_partition_for_test()
        .expect("default witness retains the fixed topology");
    assert_eq!(default_fp.1, fp_tail);
    assert_eq!(default_fq.1, fq_tail);
}

#[test]
#[ignore = "ParamsIPA allocation is retained for the serialized build window"]
fn v2_row_cap_fails_closed_before_mock_proving_or_key_generation() {
    assert!(
        MockProver::run(
            K,
            &P256PackedAffineEcdsaCircuitV2::<Fp>::default(),
            vec![vec![]],
        )
        .is_err()
    );
    assert!(
        MockProver::run(
            K,
            &P256PackedAffineEcdsaCircuitV2::<Fq>::default(),
            vec![vec![]],
        )
        .is_err()
    );
    let eq_params = ParamsIPA::<EqAffine>::new(K);
    assert!(keygen_vk(&eq_params, &P256PackedAffineEcdsaCircuitV2::<Fp>::default()).is_err());
    let ep_params = ParamsIPA::<EpAffine>::new(K);
    assert!(keygen_vk(&ep_params, &P256PackedAffineEcdsaCircuitV2::<Fq>::default()).is_err());
}

#[test]
fn source_remains_private_and_non_authorizing() {
    let source = include_str!("p256_packed_affine_v2.rs");
    assert!(source.contains("Private, non-authorizing"));
    assert!(!source.contains("VerificationAvailable"));
    assert!(!source.contains("GuardBundle::"));
    assert!(!source.contains("register_backend"));
    assert!(source.contains("inactive bounded witness was not zeroized"));
    assert!(source.contains("P256_PACKED_AFFINE_V2_STATIC_MINIMUM_ROWS"));
    assert!(source.contains("s * value"));
}
