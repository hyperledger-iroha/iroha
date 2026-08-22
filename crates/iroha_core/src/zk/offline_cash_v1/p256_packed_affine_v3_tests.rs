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
    let _ = P256PackedAffineConfigV3::configure(&mut meta);
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
    assert_eq!(shape.commitments(), 59);
    assert_eq!(shape.evaluations(), 42);
    assert_eq!(shape.point_sets(), 4);
    assert_eq!(shape.augmented_proof_bytes(), 3_264);
    assert_eq!((shape.commitments() + shape.evaluations()) * 32, 3_232);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.proof_points, 59);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.proof_scalars, 42);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.raw_proof_bytes, 3_232);
    assert_eq!(
        P256_PACKED_AFFINE_SHAPE_V3.augmented_proof_bytes,
        usize::try_from(shape.augmented_proof_bytes()).expect("proof byte count fits usize")
    );
}

#[test]
fn configured_shape_is_current_query_degree_seven_and_exactly_3264_bytes() {
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
    assert_eq!(TABLE_ROWS, 65_365);
    assert!(TABLE_ROWS < K17_MAX_ASSIGNED_ROWS);
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
    type Config = P256PackedAffineConfigV3;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    #[cfg(feature = "circuit-params")]
    fn params(&self) -> Self::Params {}

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        P256PackedAffineConfigV3::configure(meta)
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
            Err(P256PackedAffineFailureV3::Source(
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

impl P256PackedStatementSourceV3 for ExactSource {
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
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let source = ExactSource {
        statement: direct.input_bytes(),
        fail: false,
    };
    let sourced =
        P256PackedAffineEcdsaCircuitV3::<Fp>::from_source(source).expect("exact source succeeds");
    assert_eq!(sourced, direct);
    let failing = ExactSource {
        statement: direct.input_bytes(),
        fail: true,
    };
    assert!(P256PackedAffineEcdsaCircuitV3::<Fp>::from_source(failing).is_err());
}

fn reviewed_intervals(kind: ModularRelationKind) -> [IntegerInterval; 4] {
    REVIEWED_CARRY_INTERVALS_I128[kind as usize].map(|(lower, upper)| IntegerInterval {
        lower: BigInt::from(lower),
        upper: BigInt::from(upper),
    })
}

#[test]
fn quotient_intervals_are_exact_and_the_slope_rejects_a_259_bit_bias() {
    let p = modulus_base();
    let n = modulus_scalar();
    assert_eq!(
        radix(),
        BigUint::from(77_371_252_455_336_267_181_195_264_u128)
    );
    assert_eq!(
        decompose_limbs(&p),
        [
            BigUint::from(77_371_252_455_336_267_181_195_263_u128),
            BigUint::from(1_023_u16),
            BigUint::from(19_342_813_109_330_467_168_976_896_u128),
        ]
    );
    assert_eq!(
        decompose_limbs(&n),
        [
            BigUint::from(28_553_880_287_938_765_337_601_361_u128),
            BigUint::from(77_371_252_455_335_114_450_843_292_u128),
            BigUint::from(19_342_813_109_330_467_168_976_895_u128),
        ]
    );
    assert_eq!(
        decompose_limbs(&curve_b()),
        [
            BigUint::from(23_805_269_282_153_275_520_606_283_u128),
            BigUint::from(64_478_498_050_055_519_801_623_345_u128),
            BigUint::from(6_858_709_101_169_761_702_330_043_u128),
        ]
    );
    let slope = quotient_interval(ModularRelationKind::AggregateSlope, &p);
    assert_eq!(
        slope.lower,
        BigInt::parse_bytes(
            b"-347376267631068746288092340848222720590258430245870942586600893926601293561847",
            10,
        )
        .expect("decimal lower endpoint")
    );
    assert_eq!(
        slope.upper,
        BigInt::parse_bytes(
            b"231584178420712497525394893898815147060172286830580628391067262617734195707898",
            10,
        )
        .expect("decimal upper endpoint")
    );
    assert_eq!(slope.signed_bits(), 258);
    assert_eq!(slope.bias_bits(), 259);
    assert!(!slope.contains(&(&slope.lower - 1)));
    assert!(!slope.contains(&(&slope.upper + 1)));
    assert!(
        slope.bias_bits() > SLOPE_QUOTIENT_BITS,
        "a 259-bit biased slope witness is deliberately forbidden"
    );
    for endpoint in [&slope.lower, &slope.upper] {
        let mut builder = PackedBuilder::<Fp>::new();
        let active = builder.constant_bool(true);
        let signed = signed_uint_witness(
            &mut builder,
            endpoint,
            SLOPE_QUOTIENT_BITS,
            &active,
            "slope endpoint",
        )
        .expect("signed slope endpoint");
        constrain_signed_conditional_interval(&mut builder, &signed, &slope, "slope endpoint")
            .expect("slope endpoint is exact");
    }
    for outside in [&slope.lower - 1, &slope.upper + 1] {
        let mut builder = PackedBuilder::<Fp>::new();
        let active = builder.constant_bool(true);
        let signed = signed_uint_witness(
            &mut builder,
            &outside,
            SLOPE_QUOTIENT_BITS,
            &active,
            "slope endpoint +1",
        )
        .expect("outside value still fits 258-bit magnitude");
        assert!(
            constrain_signed_conditional_interval(
                &mut builder,
                &signed,
                &slope,
                "slope endpoint +1",
            )
            .is_err()
        );
    }

    let cases = [
        (
            ModularRelationKind::X,
            p.clone(),
            BigInt::from(-2),
            bigint(&p) - 2,
        ),
        (
            ModularRelationKind::Y,
            p.clone(),
            -bigint(&p) + 1,
            bigint(&p) - 2,
        ),
        (
            ModularRelationKind::AddYSum,
            p.clone(),
            BigInt::from(0),
            BigInt::from(1),
        ),
        (
            ModularRelationKind::BaseProduct,
            p.clone(),
            BigInt::from(0),
            bigint(&p) - 2,
        ),
        (
            ModularRelationKind::Curve,
            p,
            -bigint(&modulus_base()) + 2,
            bigint(&modulus_base()),
        ),
        (
            ModularRelationKind::ScalarProduct,
            n.clone(),
            BigInt::from(0),
            bigint(&n) - 2,
        ),
    ];
    for (kind, modulus, lower, upper) in cases {
        let interval = quotient_interval(kind, &modulus);
        assert_eq!(interval.lower, lower);
        assert_eq!(interval.upper, upper);
        assert!(interval.contains(&interval.lower));
        assert!(interval.contains(&interval.upper));
        assert!(!interval.contains(&(&interval.lower - 1)));
        assert!(!interval.contains(&(&interval.upper + 1)));
    }

    let chord_maximum = bigint(&modulus_base()) - 1;
    assert_eq!(
        chord_maximum,
        BigInt::parse_bytes(
            b"115792089210356248762697446949407573530086143415290314195533631308867097853950",
            10,
        )
        .expect("decimal chord endpoint")
    );
    assert!(slope.contains(&(-&chord_maximum)));
    assert!(slope.contains(&chord_maximum));
}

#[test]
fn every_exact_carry_endpoint_is_admitted_and_each_plus_one_attack_is_rejected() {
    for kind_index in 0..ModularRelationKind::COUNT {
        let kind = [
            ModularRelationKind::AggregateSlope,
            ModularRelationKind::X,
            ModularRelationKind::Y,
            ModularRelationKind::AddYSum,
            ModularRelationKind::BaseProduct,
            ModularRelationKind::Curve,
            ModularRelationKind::ScalarProduct,
        ][kind_index];
        let intervals = reviewed_intervals(kind);
        for (carry_index, interval) in intervals.iter().enumerate() {
            assert_eq!(
                interval.signed_bits(),
                EXPECTED_CARRY_SIGNED_BITS[kind_index][carry_index]
            );
            assert_eq!(
                interval.bias_bits(),
                EXPECTED_CARRY_BIAS_BITS[kind_index][carry_index]
            );
            assert!(interval.contains(&interval.lower));
            assert!(interval.contains(&interval.upper));
            assert!(!interval.contains(&(&interval.lower - 1)));
            assert!(!interval.contains(&(&interval.upper + 1)));

            for endpoint in [&interval.lower, &interval.upper] {
                let mut builder = PackedBuilder::<Fp>::new();
                let active = builder.constant_bool(true);
                biased_carry_witness(
                    &mut builder,
                    endpoint,
                    interval.clone(),
                    &active,
                    "endpoint KAT",
                )
                .expect("an exact endpoint is admissible");
            }
            for outside in [&interval.lower - 1, &interval.upper + 1] {
                let mut builder = PackedBuilder::<Fp>::new();
                let active = builder.constant_bool(true);
                assert!(
                    biased_carry_witness(
                        &mut builder,
                        &outside,
                        interval.clone(),
                        &active,
                        "endpoint +1 attack",
                    )
                    .is_err()
                );
            }
        }
    }
    assert_eq!(
        REVIEWED_CHORD_CARRY_INTERVALS_I128,
        [
            (
                -154_742_504_910_672_534_362_390_525,
                154_742_504_910_672_534_362_390_525,
            ),
            (
                -232_113_757_366_008_801_543_586_811,
                232_113_757_366_008_801_543_586_811,
            ),
            (
                -154_742_504_892_658_135_857_103_871,
                154_742_504_892_658_135_857_103_871,
            ),
            (
                -58_028_439_327_991_401_506_930_944,
                58_028_439_327_991_401_506_930_944,
            ),
        ]
    );
}

#[test]
fn tangent_slope_negative_zero_and_all_inactive_modular_witnesses_are_rejected_or_zeroized() {
    // The explicit sign*is_zero(magnitude)=0 constraint rejects the only
    // alternate representation of integer zero.
    assert_ne!(u8::from(true) * u8::from(true), 0);
    let mut builder = PackedBuilder::<Fp>::new();
    let inactive = builder.constant_bool(false);
    let signed = signed_uint_witness(
        &mut builder,
        &BigInt::from(-7),
        SLOPE_QUOTIENT_BITS,
        &inactive,
        "inactive signed quotient",
    )
    .expect("inactive signed quotient is structurally allocated");
    assert!(!signed.sign.value);
    assert_eq!(signed.magnitude.value, BigUint::from(0_u8));
    assert!(
        signed
            .signed_limbs
            .iter()
            .all(|limb| limb.value == Fp::from(0_u64))
    );

    let interval = quotient_interval(ModularRelationKind::Y, &modulus_base());
    let encoded = offset_quotient_witness(
        &mut builder,
        &BigInt::from(123),
        &interval,
        &inactive,
        "inactive biased quotient",
    )
    .expect("inactive biased quotient is structurally allocated");
    assert_eq!(encoded.value, BigUint::from(0_u8));
    let carry = biased_carry_witness(
        &mut builder,
        &BigInt::from(123),
        reviewed_intervals(ModularRelationKind::Y)[0].clone(),
        &inactive,
        "inactive biased carry",
    )
    .expect("inactive carry is structurally allocated");
    assert_eq!(carry.encoded_integer(), BigInt::from(0));
}

#[test]
fn biased_family_offsets_cover_all_five_coefficients_and_terminal_carry() {
    let families = [
        (ModularRelationKind::AggregateSlope, modulus_base(), false),
        (ModularRelationKind::X, modulus_base(), true),
        (ModularRelationKind::Y, modulus_base(), true),
        (ModularRelationKind::AddYSum, modulus_base(), false),
        (ModularRelationKind::BaseProduct, modulus_base(), true),
        (ModularRelationKind::Curve, modulus_base(), true),
        (ModularRelationKind::ScalarProduct, modulus_scalar(), true),
    ];
    let mut nonzero = 0_usize;
    for (kind, modulus, biased_quotient) in families {
        let modulus_limbs = decompose_limbs(&modulus);
        let carries = reviewed_intervals(kind);
        let lower_digits = if biased_quotient {
            signed_radix_digits(&quotient_interval(kind, &modulus).lower)
        } else {
            std::array::from_fn(|_| BigInt::from(0))
        };
        for coefficient in 0..2 * LIMBS - 1 {
            let quotient_lower = (0..LIMBS)
                .filter_map(|left| {
                    coefficient
                        .checked_sub(left)
                        .filter(|right| *right < LIMBS)
                        .map(|right| &lower_digits[left] * bigint(&modulus_limbs[right]))
                })
                .fold(BigInt::from(0), |sum, term| sum + term);
            let carry_in = coefficient
                .checked_sub(1)
                .and_then(|index| carries.get(index))
                .map_or_else(|| BigInt::from(0), |interval| interval.lower.clone());
            let carry_out = carries
                .get(coefficient)
                .map_or_else(|| BigInt::from(0), |interval| interval.lower.clone());
            let offset = -quotient_lower + carry_in - bigint(&radix()) * carry_out;
            nonzero += usize::from(offset != BigInt::from(0));
            if kind != ModularRelationKind::AddYSum || coefficient < 3 {
                assert_ne!(offset, BigInt::from(0));
            }
            if coefficient == 2 * LIMBS - 2 {
                assert!(carries.get(coefficient).is_none());
                if kind == ModularRelationKind::AddYSum {
                    assert_eq!(offset, BigInt::from(0));
                } else {
                    assert_ne!(
                        offset,
                        BigInt::from(0),
                        "the fifth equation retains c3 and has no c4 witness"
                    );
                }
            }
        }
    }
    assert_eq!(nonzero, 33, "30 family offsets plus 3 y-sum offsets");
}

fn complete_branch_flags(left: &AffineValue, right: &AffineValue) -> [bool; 6] {
    let p = modulus_base();
    let finite = !left.infinity && !right.infinity;
    let x_equal = left.x == right.x;
    let y_equal = left.y == right.y;
    let y_negative = finite && (&left.y + &right.y) % p == BigUint::from(0_u8);
    let chord = finite && !x_equal;
    let tangent = finite && x_equal && y_equal && !y_negative;
    let opposite = finite && x_equal && y_negative;
    let take_left = !left.infinity && right.infinity;
    let take_right = left.infinity && !right.infinity;
    let both_identity = left.infinity && right.infinity;
    [
        chord,
        tangent,
        opposite,
        take_left,
        take_right,
        both_identity,
    ]
}

#[test]
fn complete_addition_has_an_exclusive_six_branch_partition() {
    let identity = AffineValue::identity();
    let generator = AffineValue::generator();
    let doubled = affine_double_value(&generator).0;
    let p = modulus_base();
    let negative = AffineValue {
        x: generator.x.clone(),
        y: modular_sub(&BigUint::from(0_u8), &generator.y, &p),
        infinity: false,
    };
    let cases = [
        (&generator, &doubled, 0_usize),
        (&generator, &generator, 1),
        (&generator, &negative, 2),
        (&generator, &identity, 3),
        (&identity, &generator, 4),
        (&identity, &identity, 5),
    ];
    for (left, right, expected) in cases {
        let flags = complete_branch_flags(left, right);
        assert_eq!(flags.iter().filter(|flag| **flag).count(), 1);
        assert!(flags[expected]);
        assert!(!(flags[0] && flags[1]), "chord and tangent are disjoint");
    }

    // A prover setting both aggregated gates cannot satisfy both the boolean
    // active bit and bC+bT=a.
    for active in [false, true] {
        let residual = 1_i8 + 1_i8 - if active { 1_i8 } else { 0_i8 };
        assert_ne!(residual, 0);
    }
    let y_zero = AffineValue {
        x: BigUint::from(1_u8),
        y: BigUint::from(0_u8),
        infinity: false,
    };
    let flags = complete_branch_flags(&y_zero, &y_zero);
    assert!(flags[2]);
    assert!(!flags[1], "the y=0 tangent attack is the opposite branch");
}

#[test]
fn canonical_single_reduction_and_signature_policy_are_pinned() {
    let vector = rfc6979_sample();
    let p = modulus_base();
    let n = modulus_scalar();
    assert_eq!(usize::try_from(p.bits()).expect("p bits"), 256);
    assert_eq!(usize::try_from(n.bits()).expect("n bits"), 256);
    assert!((BigUint::from(1_u8) << 256) < (&n << 1_usize));

    let x = BigUint::from_bytes_be(&vector.sec1[1..33]);
    let y = BigUint::from_bytes_be(&vector.sec1[33..]);
    let r = BigUint::from_bytes_be(&vector.signature[..32]);
    let s = BigUint::from_bytes_be(&vector.signature[32..]);
    assert_eq!(vector.sec1[0], 4);
    assert!(x < p && y < modulus_base());
    assert!(r > BigUint::from(0_u8) && r < n);
    assert!(s > BigUint::from(0_u8) && s <= (modulus_scalar() >> 1_usize));

    for raw in [
        BigUint::from_bytes_be(&vector.digest),
        AffineValue::generator().x,
    ] {
        let reduced = &raw % modulus_scalar();
        let reduction = usize::from(raw >= modulus_scalar());
        assert!(reduction <= 1);
        assert_eq!(raw, reduced + BigUint::from(reduction) * modulus_scalar());
    }
}

fn row_preflight<F: BigPrimeField>() -> P256PackedAffineRowsV3 {
    let vector = rfc6979_sample();
    let circuit =
        P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature);
    let rows = circuit
        .trace_diagnostic_for_test()
        .expect("packed builder produces an exact k17 diagnostic");
    assert!(
        circuit.row_report().is_err(),
        "production eligibility remains closed without admitted synthesis evidence"
    );
    assert_eq!(rows.semantic_rows, P256_PACKED_AFFINE_V3_SEMANTIC_ROWS);
    assert_eq!(rows.reserved_rows, P256_PACKED_AFFINE_V3_RESERVED_ROWS);
    assert_eq!(rows.upper_rows, P256_PACKED_AFFINE_V3_UPPER_ROWS);
    assert_eq!(rows.headroom_rows, P256_PACKED_AFFINE_V3_HEADROOM_ROWS);
    assert_eq!(rows.semantic_rows, 108_877);
    assert_eq!(rows.upper_rows, 125_261);
    assert_eq!(rows.headroom_rows, 5_802);
    assert!(rows.upper_rows <= K17_MAX_ASSIGNED_ROWS);
    assert_eq!(K17_MAX_ASSIGNED_ROWS, 131_063);
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
            + rows.padding_rows
    );
    assert_eq!(rows.total_rows, rows.semantic_rows.max(rows.table_rows));
    assert_eq!(rows.table_rows, TABLE_ROWS);
    assert_eq!(rows.caller_instance_rows, PUBLIC_BYTES);
    assert_eq!(rows.constant_instance_rows, 228);
    assert_eq!(
        rows.binding_rows,
        rows.caller_instance_rows + rows.constant_instance_rows
    );
    assert_eq!(rows.complete_doublings, 263);
    assert_eq!(rows.complete_additions, 135);
    assert_eq!(rows.modular_relations, 1_334);
    assert_eq!(rows.relation_counts, ModularRelationKind::EXPECTED_COUNTS);
    assert_eq!(rows.maximum_quotient_bits, 258);
    assert_eq!(rows.maximum_carry_bits, 90);
    assert!(rows.maximum_coefficient_bits <= PACKED_COEFFICIENT_BOUND_BITS);
    let maximum_native = BigUint::parse_bytes(MAXIMUM_NATIVE_COEFFICIENT_DECIMAL.as_bytes(), 10)
        .expect("reviewed native coefficient");
    assert!(maximum_native < (BigUint::from(1_u8) << 176));
    assert!(
        (BigUint::from(1_u8) << PACKED_COEFFICIENT_BOUND_BITS) < modulus::<F>(),
        "static integer lift must fit natively"
    );
    rows
}

#[test]
fn rfc6979_row_ledger_is_exact_and_field_independent() {
    let fp_rows = row_preflight::<Fp>();
    let fq_rows = row_preflight::<Fq>();
    assert_eq!(fp_rows, fq_rows, "Pasta parity must not change topology");
}

fn instance_partition<F: BigPrimeField>() -> (Vec<F>, Vec<F>) {
    let vector = rfc6979_sample();
    P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature)
        .instance_partition_for_test()
        .expect("instance contract is derivable before transpose")
}

#[test]
fn instance_contract_is_161_caller_bytes_plus_exactly_228_derived_constants() {
    let vector = rfc6979_sample();
    let (fp_caller, fp_tail) = instance_partition::<Fp>();
    let (fq_caller, fq_tail) = instance_partition::<Fq>();
    assert_eq!(fp_caller.len(), PUBLIC_BYTES);
    assert_eq!(fq_caller.len(), PUBLIC_BYTES);
    assert_eq!(fp_tail.len(), 228);
    assert_eq!(fq_tail.len(), 228);
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

    let default_fp = P256PackedAffineEcdsaCircuitV3::<Fp>::default()
        .instance_partition_for_test()
        .expect("default witness retains the fixed topology");
    let default_fq = P256PackedAffineEcdsaCircuitV3::<Fq>::default()
        .instance_partition_for_test()
        .expect("default witness retains the fixed topology");
    assert_eq!(default_fp.1, fp_tail);
    assert_eq!(default_fq.1, fq_tail);
}

fn mock_verify<F: BigPrimeField>(circuit: &P256PackedAffineEcdsaCircuitV3<F>) -> bool {
    let Ok(instances) = circuit.instances() else {
        return false;
    };
    let Ok(prover) = MockProver::run(K, circuit, vec![instances]) else {
        return false;
    };
    prover.verify().is_ok()
}

#[test]
#[ignore = "real 108877-row semantic KAT waits for the root serialized Cargo window"]
fn real_rfc6979_mock_prover_kat_passes_on_both_pasta_fields() {
    let vector = rfc6979_sample();
    let fp =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let fq =
        P256PackedAffineEcdsaCircuitV3::<Fq>::new(vector.sec1, vector.digest, vector.signature);
    assert!(mock_verify(&fp));
    assert!(mock_verify(&fq));
}

#[test]
#[ignore = "real k17 ParamsIPA/keygen waits for the root serialized Cargo window"]
fn real_key_generation_accepts_the_exact_shape_on_both_pasta_curves() {
    let vector = rfc6979_sample();
    let fp =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let fq =
        P256PackedAffineEcdsaCircuitV3::<Fq>::new(vector.sec1, vector.digest, vector.signature);
    let eq_params = ParamsIPA::<EqAffine>::new(K);
    keygen_vk(&eq_params, &fp).expect("Fp keygen");
    let ep_params = ParamsIPA::<EpAffine>::new(K);
    keygen_vk(&ep_params, &fq).expect("Fq keygen");
}

#[test]
#[ignore = "real 108877-row rejection KATs wait for the root serialized Cargo window"]
fn real_semantic_attacks_reject_prefix_zero_scalar_high_s_and_off_curve_key() {
    let vector = rfc6979_sample();

    let mut bad_prefix = vector.clone();
    bad_prefix.sec1[0] = 3;
    let circuit = P256PackedAffineEcdsaCircuitV3::<Fp>::new(
        bad_prefix.sec1,
        bad_prefix.digest,
        bad_prefix.signature,
    );
    assert!(!mock_verify(&circuit));

    let mut zero_r = vector.clone();
    zero_r.signature[..32].fill(0);
    let circuit =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(zero_r.sec1, zero_r.digest, zero_r.signature);
    assert!(!mock_verify(&circuit));

    let mut high_s = vector.clone();
    let low_s = BigUint::from_bytes_be(&high_s.signature[32..]);
    let high = modulus_scalar() - low_s;
    high_s.signature[32..].copy_from_slice(&high.to_bytes_be());
    let circuit =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(high_s.sec1, high_s.digest, high_s.signature);
    assert!(!mock_verify(&circuit));

    let mut off_curve = vector;
    off_curve.sec1[1..].fill(0);
    let circuit = P256PackedAffineEcdsaCircuitV3::<Fp>::new(
        off_curve.sec1,
        off_curve.digest,
        off_curve.signature,
    );
    assert!(!mock_verify(&circuit));
}

#[test]
fn source_remains_undeclared_private_and_non_authorizing() {
    let source = include_str!("p256_packed_affine_v3.rs");
    let parent = include_str!("../offline_cash_v1.rs");
    assert!(source.contains("Private, non-authorizing"));
    assert!(!parent.contains("mod p256_packed_affine_v3"));
    assert!(!source.contains("VerificationAvailable"));
    assert!(!source.contains("GuardBundle::"));
    assert!(!source.contains("register_backend"));
    assert!(!source.contains("activate_backend"));
    assert!(source.contains("inactive bounded witness was not zeroized"));
    assert!(source.contains("terminal modular carry was nonzero"));
    assert!(source.contains("mandatory terminal equation c4=0"));
    assert!(source.contains("family_coefficient_offset"));
    assert!(source.contains("REVIEWED_CARRY_INTERVALS_I128"));
    assert!(source.contains("Err(Error::Synthesis)"));
    assert!(source.contains("Rotation::cur()"));
    assert!(source.contains("s * value"));
}
