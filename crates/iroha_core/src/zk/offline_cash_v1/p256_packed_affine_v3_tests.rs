use super::*;

use ff::Field as _;
use halo2_base::halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{ConstraintSystem, FirstPhase, keygen_vk},
    poly::{Rotation, commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use halo2_base::utils::modulus;
use sha2::{Digest as _, Sha256};
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

fn zero_s_sample() -> TestVector {
    let mut vector = rfc6979_sample();
    vector.signature[32..].fill(0);
    vector
}

fn canonical_tampered_signature_sample() -> TestVector {
    let mut vector = rfc6979_sample();
    // Preserve P1363 width, nonzero/canonical r and s, and low-S while changing
    // the signed scalar. This reaches the actual ECDSA equality instead of a
    // parser or policy shortcut.
    vector.signature[63] ^= 1;
    vector
}

fn host_scalar_multiply(mut scalar: BigUint, mut point: AffineValue) -> AffineValue {
    let mut accumulator = AffineValue::identity();
    while scalar != BigUint::from(0_u8) {
        if (&scalar & BigUint::from(1_u8)) == BigUint::from(1_u8) {
            accumulator = affine_add_value(&accumulator, &point).0;
        }
        point = affine_double_value(&point).0;
        scalar >>= 1_usize;
    }
    accumulator
}

fn bounded_host_ecdsa_accepts(vector: &TestVector) -> bool {
    let p = modulus_base();
    let n = modulus_scalar();
    let x = BigUint::from_bytes_be(&vector.sec1[1..33]);
    let y = BigUint::from_bytes_be(&vector.sec1[33..]);
    let r = BigUint::from_bytes_be(&vector.signature[..32]);
    let s = BigUint::from_bytes_be(&vector.signature[32..]);
    if vector.sec1[0] != 4
        || x >= p
        || y >= modulus_base()
        || r == BigUint::from(0_u8)
        || r >= n
        || s == BigUint::from(0_u8)
        || s > (modulus_scalar() >> 1_usize)
    {
        return false;
    }

    let x_squared = (&x * &x) % &p;
    let x_cubed = (&x_squared * &x) % &p;
    let three_x = (BigUint::from(3_u8) * &x) % &p;
    if (&y * &y) % &p != modular_sub(&(x_cubed + curve_b()), &three_x, &p) {
        return false;
    }

    let inverse = modular_inverse(&s, &n);
    let z = BigUint::from_bytes_be(&vector.digest) % &n;
    let u1 = (z * &inverse) % &n;
    let u2 = (&r * inverse) % &n;
    let left = host_scalar_multiply(u1, AffineValue::generator());
    let right = host_scalar_multiply(
        u2,
        AffineValue {
            x,
            y,
            infinity: false,
        },
    );
    let result = affine_add_value(&left, &right).0;
    !result.infinity && result.x % n == r
}

fn assert_configured_shape<F: BigPrimeField>() {
    let mut meta = ConstraintSystem::<F>::default();
    let _ = P256PackedAffineConfigV3::configure(&mut meta);
    assert_eq!(meta.degree(), 10);
    assert_eq!(meta.num_advice_columns(), 16);
    assert_eq!(meta.num_instance_columns(), 1);
    assert_eq!(meta.num_fixed_columns(), 4);
    assert_eq!(meta.num_selectors(), 0);
    assert_eq!(meta.advice_queries().len(), 16);
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
    assert_eq!(meta.permutation().get_columns().len(), 16);
    assert_eq!(meta.lookups().len(), 4);
    let shape = pasta_ipa_augmented_proof_shape_v1(&meta, K, PastaIpaInstanceQueryV1::Direct)
        .expect("shared exact shape calculator");
    assert_eq!(shape.commitments(), 74);
    assert_eq!(shape.evaluations(), 68);
    assert_eq!(shape.point_sets(), 4);
    // The shared calculator also reports a legacy augmented size, but the
    // clean Offline Cash V1 protocol carries the ordinary Poseidon proof only.
    assert_eq!((shape.commitments() + shape.evaluations()) * 32, 4_544);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.proof_points, 74);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.proof_scalars, 68);
    assert_eq!(P256_PACKED_AFFINE_SHAPE_V3.ordinary_proof_bytes, 4_544);

    // Processed PK framing is exact: verifier bytes, three singleton
    // polynomials, two fixed-polynomial vectors, two permutation-polynomial
    // vectors, and four vector-count headers. Pin the worst case using the
    // separately authenticated VK ceiling so the release cap cannot silently
    // regress to the old eight-equality-column estimate.
    let processed_polynomials = 3_u64
        + 2 * u64::try_from(meta.num_fixed_columns()).expect("fixed count fits u64")
        + 2 * u64::try_from(meta.permutation().get_columns().len())
            .expect("permutation count fits u64");
    let polynomial_frame_bytes = 4_u64 + (1_u64 << K) * 32;
    let governed_processed_pk_upper =
        iroha_data_model::offline::OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
            + processed_polynomials * polynomial_frame_bytes
            + 4 * 4;
    assert_eq!(processed_polynomials, 43);
    assert_eq!(governed_processed_pk_upper, 90_243_260);
    assert!(governed_processed_pk_upper > 64 * 1024 * 1024);
    assert!(
        governed_processed_pk_upper
            <= iroha_data_model::offline::OFFLINE_CASH_P256_V3_PROVING_KEY_MAX_BYTES_V1
    );
}

#[test]
fn configured_shape_is_k16_dual_lane_and_exactly_4544_ordinary_bytes() {
    assert_configured_shape::<Fp>();
    assert_configured_shape::<Fq>();
}

#[test]
fn two_lane_transpose_pairs_only_identical_fixed_geometry_without_alias_leakage() {
    fn logical(opcode: Opcode, range_bits: usize, id: usize, value: u64) -> AssignedRow<Fp> {
        let mut row = AssignedRow::zero(opcode);
        row.range_bits = range_bits;
        row.set(
            0,
            CellVar {
                id,
                value: Fp::from(value),
            },
        );
        row.set(
            7,
            CellVar {
                id: id + 1,
                value: Fp::from(value + 1),
            },
        );
        row
    }

    let packed = pack_identical_logical_rows_v3(vec![
        logical(Opcode::Range, 15, 10, 101),
        logical(Opcode::Range, 15, 20, 201),
        logical(Opcode::Range, 14, 30, 301),
        logical(Opcode::Sparse, 15, 40, 401),
    ])
    .expect("valid fixed-geometry packing");
    assert_eq!(
        packed.len(),
        3,
        "only the equal Range/15 pair may share a row"
    );
    let paired = packed
        .iter()
        .find(|row| row.opcode == Opcode::Range && row.range_bits == 15)
        .expect("paired Range/15 row");
    assert_eq!(paired.values[0], Fp::from(101));
    assert_eq!(paired.values[7], Fp::from(102));
    assert_eq!(paired.values[8], Fp::from(201));
    assert_eq!(paired.values[15], Fp::from(202));
    assert_eq!(paired.aliases, vec![(10, 0), (11, 7), (20, 8), (21, 15)]);
    assert!(packed.iter().all(|row| {
        row.aliases
            .iter()
            .all(|(_, column)| *column < ADVICE_COLUMNS)
            && (row.opcode != Opcode::Bind)
    }));

    assert!(matches!(
        pack_identical_logical_rows_v3(vec![logical(Opcode::Bind, 0, 50, 501)]),
        Err(P256PackedAffineFailureV3::Source(
            "public Bind row reached the private two-lane packer"
        ))
    ));
    assert_eq!(K16_MAX_ASSIGNED_ROWS - TABLE_ROWS, 162);
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
    assert!(TABLE_ROWS < K16_MAX_ASSIGNED_ROWS);
}

#[test]
fn typed_range_table_helper_pins_sentinel_count_endpoints_and_order() {
    assert_eq!(RANGE_CHUNK_BITS, [2, 4, 6, 8, 9, 10, 11, 12, 13, 14, 15]);
    let rows = typed_range_table_rows_v3().collect::<Vec<_>>();
    assert_eq!(rows.len(), TABLE_ROWS);
    assert_eq!(rows.first(), Some(&(0_u64, 0_u64)));

    let mut offset = 1_usize;
    for bits in RANGE_CHUNK_BITS {
        let tag = u64::try_from(bits).expect("range width fits u64");
        let values = 1_usize << bits;
        assert_eq!(rows[offset], (0, 0));
        assert_eq!(rows[offset + 1], (tag, tag * tag));
        let maximum = u64::try_from(values - 1).expect("typed range maximum fits u64");
        assert_eq!(
            rows[offset + values - 1],
            (tag * maximum, tag * tag * maximum)
        );
        for (value, pair) in rows[offset..offset + values].iter().enumerate() {
            let value = u64::try_from(value).expect("typed range value fits u64");
            assert_eq!(*pair, (tag * value, tag * tag * value));
        }
        offset += values;
    }
    assert_eq!(offset, rows.len());
}

fn selector_value<F: BigPrimeField>(opcode: u64, roots: &[u64]) -> F {
    roots.iter().fold(F::ONE, |value, root| {
        value * (F::from(opcode) - F::from(*root))
    })
}

fn machine_residuals<F: BigPrimeField>(
    v: [F; LOGICAL_LANE_COLUMNS],
    public: F,
    opcode: u64,
    tag: u64,
) -> Vec<F> {
    let q_bind = selector_value::<F>(opcode, &Q_BIND_ROOTS);
    let q_range = [3_u64, 6, 8, 9]
        .into_iter()
        .fold(F::from(tag), |value, root| {
            value * (F::from(opcode) - F::from(root))
        });
    let q_sparse = selector_value::<F>(opcode, &Q_SPARSE_ROOTS);
    let q_dense = selector_value::<F>(opcode, &Q_DENSE_ROOTS);
    let q_select = selector_value::<F>(opcode, &Q_SELECT_ROOTS);
    let q_sign = selector_value::<F>(opcode, &Q_SIGN_ROOTS);
    let q_lookup_boolean = selector_value::<F>(opcode, &Q_LOOKUP_BOOLEAN_ROOTS);
    let q_lookup_sign = selector_value::<F>(opcode, &Q_LOOKUP_SIGN_ROOTS);
    let q_lookup_select = selector_value::<F>(opcode, &Q_LOOKUP_SELECT_ROOTS);
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
    let lookup_sign_zero = v[5] - v[3] * (F::ONE - F::from(2_u64) * v[1]);
    let lookup_sign_one = v[7] - v[6] * (F::ONE - F::from(2_u64) * v[1]) - public;
    let lookup_select = v[1] + v[5] * (v[2] - v[1]) - v[3];
    let mut residuals = vec![
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
        q_lookup_sign * lookup_sign_zero,
        q_lookup_sign * lookup_sign_one,
        q_lookup_sign * v[1] * (v[1] - F::ONE),
        q_lookup_sign * (F::ONE - v[2]) * v[3],
        q_lookup_sign * (F::ONE - v[2]) * v[6],
        q_lookup_sign * (F::ONE - v[2]) * v[1],
        q_lookup_select * lookup_select,
        q_lookup_select * v[5] * (v[5] - F::ONE),
    ];
    for column in [1_usize, 2, 3, 5, 6, 7] {
        let value = if column == 7 {
            v[column] - public
        } else {
            v[column]
        };
        residuals.push(q_lookup_boolean * value * (value - F::ONE));
    }
    residuals
}

fn assert_zero_residuals<F: BigPrimeField>(residuals: &[F], context: &str) {
    assert!(
        residuals.iter().all(|residual| *residual == F::ZERO),
        "nonzero packed-machine residual in {context}"
    );
}

fn assert_selector_and_overlap_invariants<F: BigPrimeField>() {
    let expected = [
        [false, false, false, false, false, false, false, false],
        [true, true, true, true, true, true, true, true],
        [false, false, false, false, false, false, false, false],
        [false, true, false, false, false, false, false, false],
        [false, false, true, false, false, false, false, false],
        [false, false, false, true, false, false, false, false],
        [false, false, false, false, false, true, false, false],
        [false, true, false, false, true, false, false, false],
        [false, false, false, false, false, false, true, false],
        [false, false, false, false, false, false, false, true],
    ];
    for opcode in 0_u64..=9 {
        let actual = [
            selector_value::<F>(opcode, &Q_BIND_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SPARSE_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_DENSE_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SELECT_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_SIGN_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_LOOKUP_BOOLEAN_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_LOOKUP_SIGN_ROOTS) != F::ZERO,
            selector_value::<F>(opcode, &Q_LOOKUP_SELECT_ROOTS) != F::ZERO,
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
    let mut bind = [F::ZERO; LOGICAL_LANE_COLUMNS];
    bind[7] = public;
    assert_zero_residuals(&machine_residuals(bind, public, 1, 0), "Bind overlap");

    // A lookup-bearing Sparse row zeros q_range through its opcode root.
    let mut sparse = [F::ZERO; LOGICAL_LANE_COLUMNS];
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
        &machine_residuals([F::ZERO; LOGICAL_LANE_COLUMNS], F::ZERO, 2, 15),
        "inactive Range zeroization",
    );

    // Each lookup overlay has one and only one purpose-built selector while
    // retaining the two typed-range cells in v0/v4.
    let lookup_boolean = [
        F::from(17_u64),
        F::ONE,
        F::ZERO,
        F::ONE,
        F::from(19_u64),
        F::ZERO,
        F::ONE,
        F::ONE,
    ];
    assert_zero_residuals(
        &machine_residuals(lookup_boolean, F::ZERO, 6, 15),
        "LookupBoolean overlay",
    );

    let lookup_sign = [
        F::from(17_u64),
        F::ONE,
        F::ONE,
        F::from(23_u64),
        F::from(19_u64),
        -F::from(23_u64),
        F::from(29_u64),
        -F::from(29_u64),
    ];
    assert_zero_residuals(
        &machine_residuals(lookup_sign, F::ZERO, 8, 15),
        "LookupSign overlay",
    );

    let lookup_select = [
        F::from(17_u64),
        F::from(11_u64),
        F::from(13_u64),
        F::from(13_u64),
        F::from(19_u64),
        F::ONE,
        F::ZERO,
        F::ZERO,
    ];
    assert_zero_residuals(
        &machine_residuals(lookup_select, F::ZERO, 9, 15),
        "LookupSelect overlay",
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

fn assert_invalid_signature_frames_stay_source_exact_and_row_bounded<F: BigPrimeField>() {
    for (label, vector) in [
        ("zero-s", zero_s_sample()),
        (
            "canonical tampered signature",
            canonical_tampered_signature_sample(),
        ),
    ] {
        let direct =
            P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature);
        let sourced = P256PackedAffineEcdsaCircuitV3::<F>::from_source(ExactSource {
            statement: direct.input_bytes(),
            fail: false,
        })
        .unwrap_or_else(|_| panic!("{label} exact source frame"));
        assert_eq!(
            sourced, direct,
            "{label} must not be rewritten by source IO"
        );

        let rows = direct
            .trace_diagnostic_for_test()
            .unwrap_or_else(|error| panic!("{label} bounded host trace: {error:?}"));
        assert_eq!(rows.semantic_rows, P256_PACKED_AFFINE_V3_SEMANTIC_ROWS);
        assert_eq!(rows.upper_rows, P256_PACKED_AFFINE_V3_UPPER_ROWS);
        assert_eq!(rows.headroom_rows, P256_PACKED_AFFINE_V3_HEADROOM_ROWS);
        assert!(rows.upper_rows <= K16_MAX_ASSIGNED_ROWS);
        assert_eq!(
            direct.row_report().expect("bounded production row report"),
            rows,
            "{label} production geometry must match the diagnostic trace"
        );
    }
}

#[test]
fn zero_s_and_canonical_tamper_are_exact_bounded_frames_on_both_pasta_fields() {
    let valid = rfc6979_sample();
    let zero_s = zero_s_sample();
    let tampered = canonical_tampered_signature_sample();
    let scalar_modulus = modulus_scalar();

    assert_eq!(
        BigUint::from_bytes_be(&zero_s.signature[32..]),
        BigUint::from(0_u8)
    );
    assert_ne!(tampered.signature, valid.signature);
    let tampered_r = BigUint::from_bytes_be(&tampered.signature[..32]);
    let tampered_s = BigUint::from_bytes_be(&tampered.signature[32..]);
    assert!(tampered_r > BigUint::from(0_u8) && tampered_r < scalar_modulus);
    assert!(tampered_s > BigUint::from(0_u8));
    assert!(tampered_s <= (modulus_scalar() >> 1_usize));
    assert!(bounded_host_ecdsa_accepts(&valid));
    assert!(!bounded_host_ecdsa_accepts(&zero_s));
    assert!(!bounded_host_ecdsa_accepts(&tampered));

    assert_invalid_signature_frames_stay_source_exact_and_row_bounded::<Fp>();
    assert_invalid_signature_frames_stay_source_exact_and_row_bounded::<Fq>();
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
        .expect("packed builder produces an exact k16 diagnostic");
    eprintln!("P256 V3 common-k16 two-lane row ledger: {rows:?}");
    assert_eq!(
        circuit.row_report().expect("bounded production row report"),
        rows,
        "production geometry must match the exact diagnostic trace"
    );
    assert_eq!(rows.semantic_rows, P256_PACKED_AFFINE_V3_SEMANTIC_ROWS);
    assert_eq!(rows.reserved_rows, P256_PACKED_AFFINE_V3_RESERVED_ROWS);
    assert_eq!(rows.upper_rows, P256_PACKED_AFFINE_V3_UPPER_ROWS);
    assert_eq!(rows.headroom_rows, P256_PACKED_AFFINE_V3_HEADROOM_ROWS);
    assert_eq!(rows.semantic_rows, 64_886);
    assert_eq!(rows.reserved_rows, 479);
    assert_eq!(rows.upper_rows, 65_365);
    assert_eq!(rows.headroom_rows, 162);
    assert!(rows.upper_rows <= K16_MAX_ASSIGNED_ROWS);
    assert_eq!(K16_MAX_ASSIGNED_ROWS, 65_527);
    assert_eq!(rows.binding_rows, 396);
    assert_eq!(rows.range_rows, 12_775);
    assert_eq!(rows.sparse_rows, 15_563);
    assert_eq!(rows.lookup_only_rows, 9_265);
    assert_eq!(rows.dense_rows, 23_078);
    assert_eq!(rows.wide_rows, 3_809);
    assert_eq!(rows.sign_rows, 0);
    assert_eq!(rows.selection_rows, 0);
    assert_eq!(rows.padding_rows, 0);
    assert_eq!(rows.table_padding_rows, 479);
    assert_eq!(rows.zero_tests, 3_214);
    assert_eq!(rows.canonical_checks, 2_539);
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
    assert_eq!(rows.table_rows, 65_365);
    assert_eq!(rows.total_rows, 65_365);
    assert_eq!(rows.caller_instance_rows, PUBLIC_BYTES);
    assert_eq!(rows.constant_instance_rows, 235);
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

#[test]
#[ignore = "developer-only topology measurement"]
fn report_physical_equality_alias_demand() {
    let vector = rfc6979_sample();
    let circuit =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let trace = circuit
        .build_trace_unbounded_for_test()
        .expect("valid fixture has a complete unbounded diagnostic trace");
    let mut occurrences = HashMap::<usize, usize>::new();
    for row in &trace.rows_data {
        for (variable, _) in &row.aliases {
            *occurrences.entry(*variable).or_default() += 1;
        }
    }

    let mut histograms = BTreeMap::<u64, BTreeMap<usize, usize>>::new();
    let mut distinct_histograms = BTreeMap::<u64, BTreeMap<usize, usize>>::new();
    let mut maximum = BTreeMap::<u64, usize>::new();
    for row in &trace.rows_data {
        if row.opcode == Opcode::Disabled {
            continue;
        }
        let repeated = row
            .aliases
            .iter()
            .filter(|(variable, _)| occurrences[variable] > 1)
            .count();
        let distinct = row
            .aliases
            .iter()
            .filter(|(variable, _)| occurrences[variable] > 1)
            .map(|(variable, _)| *variable)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        *histograms
            .entry(row.opcode as u64)
            .or_default()
            .entry(repeated)
            .or_default() += 1;
        *distinct_histograms
            .entry(row.opcode as u64)
            .or_default()
            .entry(distinct)
            .or_default() += 1;
        maximum
            .entry(row.opcode as u64)
            .and_modify(|current| *current = (*current).max(repeated))
            .or_insert(repeated);
    }
    eprintln!("alias-position histograms by opcode: {histograms:#?}");
    eprintln!("distinct repeated-variable histograms by opcode: {distinct_histograms:#?}");
    eprintln!("maximum repeated alias positions by opcode: {maximum:#?}");

    let builder = circuit
        .build_builder_diagnostic()
        .expect("valid fixture has a complete builder");
    eprintln!(
        "logical builder rows: range={} sparse={} dense={} wide={} sign={} select={}",
        builder.range_rows.len(),
        builder.sparse_rows.len(),
        builder.dense_rows.len(),
        builder.wide_rows.len(),
        builder.sign_lanes.len(),
        builder.selects.len(),
    );
    let mut range_shapes = BTreeMap::<Vec<usize>, usize>::new();
    for row in &builder.range_rows {
        *range_shapes
            .entry(row.bounded.chunks.iter().map(|chunk| chunk.bits).collect())
            .or_default() += 1;
    }
    eprintln!("range logical-row chunk shapes: {range_shapes:#?}");
    let mut normalized_range_groups = BTreeMap::<(usize, usize), usize>::new();
    let mut active_ids_by_width = BTreeMap::<usize, std::collections::BTreeSet<usize>>::new();
    for row in &builder.range_rows {
        let width = if row.bounded.chunks.len() == 6 {
            90
        } else {
            row.bounded.bits
        };
        *normalized_range_groups
            .entry((width, row.bounded.active.id))
            .or_default() += 1;
        active_ids_by_width
            .entry(width)
            .or_default()
            .insert(row.bounded.active.id);
    }
    let normalized_range_batch_rows = normalized_range_groups
        .iter()
        .map(|((width, _), count)| count.div_ceil(4) * if *width == 90 { 6 } else { 2 })
        .sum::<usize>();
    let distinct_active_ids_by_width = active_ids_by_width
        .into_iter()
        .map(|(width, ids)| (width, ids.len()))
        .collect::<BTreeMap<_, _>>();
    eprintln!("normalized range groups: {normalized_range_groups:#?}");
    eprintln!("distinct active ids by normalized width: {distinct_active_ids_by_width:#?}");
    eprintln!("normalized four-lane Table15 range rows: {normalized_range_batch_rows}");

    let mut global_occurrences = HashMap::<usize, usize>::new();
    let mut count_cell = |cell: CellVar<Fp>| {
        *global_occurrences.entry(cell.id).or_default() += 1;
    };
    for cell in builder
        .caller_instances
        .iter()
        .chain(&builder.constant_instances)
    {
        count_cell(*cell);
    }
    for row in &builder.range_rows {
        count_cell(row.bounded.cell);
        count_cell(row.bounded.active);
        for chunk in &row.bounded.chunks {
            count_cell(chunk.cell);
        }
    }
    for row in &builder.sparse_rows {
        for cell in [row.left, row.right, row.gate, row.accumulator, row.output] {
            count_cell(cell);
        }
    }
    for row in &builder.dense_rows {
        for (left, right) in row.products {
            count_cell(left);
            count_cell(right);
        }
        count_cell(row.accumulator);
        count_cell(row.output);
    }
    for row in &builder.wide_rows {
        for cell in [
            row.left,
            row.right,
            row.carry_in,
            row.carry_out,
            row.constant,
        ] {
            count_cell(cell);
        }
    }
    for lane in &builder.sign_lanes {
        for cell in [lane.magnitude, lane.sign, lane.signed, lane.active] {
            count_cell(cell);
        }
    }
    for lane in &builder.selects {
        for cell in [lane.left, lane.bit, lane.right, lane.output] {
            count_cell(cell);
        }
    }
    drop(count_cell);

    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum SparseChainOrientation {
        Positive,
        Reverse,
        Singleton,
    }
    let sparse_link =
        |left: &SparseRow<Fp>, right: &SparseRow<Fp>, orientation: SparseChainOrientation| {
            if left.gate.id != right.gate.id {
                return false;
            }
            let intermediate = match orientation {
                SparseChainOrientation::Positive if right.accumulator.id == left.output.id => {
                    left.output.id
                }
                SparseChainOrientation::Reverse if right.output.id == left.accumulator.id => {
                    left.accumulator.id
                }
                _ => return false,
            };
            global_occurrences.get(&intermediate) == Some(&2)
        };
    let mut sparse_chain_histogram = BTreeMap::<(SparseChainOrientation, usize), usize>::new();
    let mut sparse_packed_rows = 0_usize;
    let mut sparse_singleton_or_unpaired_rows = 0_usize;
    let mut sparse_covered_rows = 0_usize;
    let mut index = 0_usize;
    while index < builder.sparse_rows.len() {
        let orientation = builder
            .sparse_rows
            .get(index + 1)
            .and_then(|next| {
                let current = &builder.sparse_rows[index];
                [
                    SparseChainOrientation::Positive,
                    SparseChainOrientation::Reverse,
                ]
                .into_iter()
                .find(|orientation| sparse_link(current, next, *orientation))
            })
            .unwrap_or(SparseChainOrientation::Singleton);
        let mut end = index + 1;
        if orientation != SparseChainOrientation::Singleton {
            while let Some(next) = builder.sparse_rows.get(end) {
                if !sparse_link(&builder.sparse_rows[end - 1], next, orientation) {
                    break;
                }
                end += 1;
            }
        }
        let length = end - index;
        *sparse_chain_histogram
            .entry((orientation, length))
            .or_default() += 1;
        sparse_packed_rows += length.div_ceil(2);
        sparse_singleton_or_unpaired_rows += length % 2;
        sparse_covered_rows += length;
        index = end;
    }
    eprintln!("sparse chain histogram: {sparse_chain_histogram:#?}");
    eprintln!(
        "sparse two-cubic rows={sparse_packed_rows} singleton-or-unpaired={sparse_singleton_or_unpaired_rows} uncovered={}",
        builder.sparse_rows.len() - sparse_covered_rows,
    );
    let mut sign_group_sizes = BTreeMap::<usize, usize>::new();
    let mut signs_by_key = BTreeMap::<(usize, usize), usize>::new();
    for lane in &builder.sign_lanes {
        *signs_by_key
            .entry((lane.sign.id, lane.active.id))
            .or_default() += 1;
    }
    for size in signs_by_key.into_values() {
        *sign_group_sizes.entry(size).or_default() += 1;
    }
    eprintln!("sign logical group-size histogram: {sign_group_sizes:#?}");
    let mut select_group_sizes = BTreeMap::<usize, usize>::new();
    let mut selects_by_bit = BTreeMap::<usize, usize>::new();
    for lane in &builder.selects {
        *selects_by_bit.entry(lane.bit.id).or_default() += 1;
    }
    for size in selects_by_bit.into_values() {
        *select_group_sizes.entry(size).or_default() += 1;
    }
    eprintln!("select logical group-size histogram: {select_group_sizes:#?}");
    let zero = builder.constants[&BigInt::from(0)].id;
    let one = builder.constants[&BigInt::from(1)].id;
    let mut dense_patterns = BTreeMap::<String, usize>::new();
    for row in &builder.dense_rows {
        let ids = [
            row.products[0].0.id,
            row.products[0].1.id,
            row.products[1].0.id,
            row.products[1].1.id,
            row.products[2].0.id,
            row.output.id,
            row.accumulator.id,
            row.products[2].1.id,
        ];
        let pattern = ids
            .map(|id| {
                if id == zero {
                    '0'
                } else if id == one {
                    '1'
                } else if builder.constants.values().any(|cell| cell.id == id) {
                    'c'
                } else {
                    'v'
                }
            })
            .into_iter()
            .collect::<String>();
        *dense_patterns.entry(pattern).or_default() += 1;
    }
    eprintln!("dense logical-row constant patterns: {dense_patterns:#?}");

    // A candidate A10/F3/P6/L4 schedule replaces every three-limb Wide
    // relation with an 18-row, four-slot Table15 trace.  Two exact sums share
    // the trace: A6..A9 hold their left/right chunks, while one packed carry
    // state in P0 represents both carry bits.  This diagnostic proves which
    // ordinary range rows that fused trace subsumes and measures the exact
    // residual batching demand; it does not change the production circuit.
    assert_eq!(builder.wide_rows.len() % LIMBS, 0);
    let exact_sum_checks = builder.wide_rows.len() / LIMBS;
    assert_eq!(exact_sum_checks, builder.canonical_checks);
    let range_by_cell = builder
        .range_rows
        .iter()
        .map(|row| (row.bounded.cell.id, row))
        .collect::<HashMap<_, _>>();
    let mut exact_sum_range_ids = std::collections::BTreeSet::<usize>::new();
    let mut exact_sum_range_uses = BTreeMap::<usize, usize>::new();
    let mut exact_sum_constant_classes = BTreeMap::<[usize; LIMBS], usize>::new();
    let mut exact_sum_missing_range_cells = std::collections::BTreeSet::<usize>::new();
    let mut exact_sum_lookup_uses = 0_usize;
    for check in builder.wide_rows.chunks_exact(LIMBS) {
        let constant_class = std::array::from_fn(|index| check[index].constant.id);
        *exact_sum_constant_classes
            .entry(constant_class)
            .or_default() += 1;
        for row in check {
            for cell in [row.left, row.right] {
                *exact_sum_range_uses.entry(cell.id).or_default() += 1;
                if let Some(range) = range_by_cell.get(&cell.id) {
                    exact_sum_range_ids.insert(cell.id);
                    exact_sum_lookup_uses += range.bounded.chunks.len();
                } else {
                    exact_sum_missing_range_cells.insert(cell.id);
                }
            }
        }
    }
    let exact_sum_operand_uses = exact_sum_checks * 2 * LIMBS;
    let exact_sum_duplicate_operand_uses = exact_sum_operand_uses - exact_sum_range_uses.len();
    let exact_sum_unique_lookup_uses = exact_sum_range_ids
        .iter()
        .map(|id| range_by_cell[id].bounded.chunks.len())
        .sum::<usize>();
    let exact_sum_pairs = exact_sum_checks.div_ceil(2);
    let exact_sum_fused_rows = exact_sum_pairs * LIMBS * 6;
    let exact_sum_partial_rows = exact_sum_pairs * LIMBS * 5;
    let exact_sum_recompose_rows = exact_sum_pairs * LIMBS;
    eprintln!(
        "fused Table15 exact sums: checks={exact_sum_checks} pairs={exact_sum_pairs} rows={exact_sum_fused_rows} partial5={exact_sum_partial_rows} recompose={exact_sum_recompose_rows} operand-uses={exact_sum_operand_uses} unique-operands={} duplicate-uses={exact_sum_duplicate_operand_uses} lookup-uses={exact_sum_lookup_uses} unique-lookups={exact_sum_unique_lookup_uses} lookup-capacity={} missing={exact_sum_missing_range_cells:?}",
        exact_sum_range_ids.len(),
        exact_sum_fused_rows * 4,
    );
    eprintln!("fused Table15 exact-sum constant classes: {exact_sum_constant_classes:#?}");

    let mut remaining_range_groups = BTreeMap::<(usize, usize), usize>::new();
    let mut remaining_range_rows = 0_usize;
    let mut remaining_large_batches = 0_usize;
    let mut remaining_small_batches = 0_usize;
    for row in &builder.range_rows {
        if exact_sum_range_ids.contains(&row.bounded.cell.id) {
            continue;
        }
        let width = if row.bounded.chunks.len() == 6 {
            90
        } else {
            row.bounded.bits
        };
        *remaining_range_groups
            .entry((width, row.bounded.active.id))
            .or_default() += 1;
    }
    for ((width, _), count) in &remaining_range_groups {
        let batches = count.div_ceil(4);
        if *width == 90 {
            remaining_large_batches += batches;
            remaining_range_rows += batches * 6;
        } else {
            remaining_small_batches += batches;
            remaining_range_rows += batches * 2;
        }
    }
    let remaining_full_overlay_rows = remaining_large_batches * 5 + remaining_small_batches;
    eprintln!(
        "post-fusion ordinary Table15 range: groups={} large-batches={remaining_large_batches} small-batches={remaining_small_batches} rows={remaining_range_rows} full-overlay={remaining_full_overlay_rows}",
        remaining_range_groups.len(),
    );
    eprintln!("post-fusion ordinary range groups: {remaining_range_groups:#?}");

    // Remove the exact signed-magnitude helper emitted once by each of the 398
    // AggregateSlope relations.  The subtraction is mechanically pinned to
    // the already-reported row patterns, so a builder change fails instead of
    // silently improving the dense lower bound.
    let dense_pattern = |row: &DenseRow<Fp>| {
        [
            row.products[0].0.id,
            row.products[0].1.id,
            row.products[1].0.id,
            row.products[1].1.id,
            row.products[2].0.id,
            row.output.id,
            row.accumulator.id,
            row.products[2].1.id,
        ]
        .map(|id| {
            if id == zero {
                '0'
            } else if id == one {
                '1'
            } else if builder.constants.values().any(|cell| cell.id == id) {
                'c'
            } else {
                'v'
            }
        })
        .into_iter()
        .collect::<String>()
    };
    let mut slope_removal_budget = BTreeMap::from([
        (String::from("vv0001v0"), 1_194_usize),
        (String::from("vv000000"), 1_194_usize),
        (String::from("vv000v00"), 1_592_usize),
        (String::from("v10c0000"), 398_usize),
    ]);
    let mut slope_removed_boolean_ids = std::collections::BTreeSet::<usize>::new();
    let mut post_slope_dense = Vec::<&DenseRow<Fp>>::new();
    for row in &builder.dense_rows {
        let pattern = dense_pattern(row);
        if let Some(remaining) = slope_removal_budget.get_mut(&pattern) {
            if *remaining > 0 {
                *remaining -= 1;
                if pattern == "vv0001v0" {
                    assert!(
                        slope_removed_boolean_ids.insert(row.accumulator.id),
                        "each removed is-zero flag must be a distinct boolean witness"
                    );
                }
                continue;
            }
        }
        post_slope_dense.push(row);
    }
    assert!(
        slope_removal_budget
            .values()
            .all(|remaining| *remaining == 0),
        "the signed-magnitude helper pattern budget must remain exact: {slope_removal_budget:?}"
    );
    assert_eq!(post_slope_dense.len(), builder.dense_rows.len() - 4_378);
    assert_eq!(slope_removed_boolean_ids.len(), 1_194);

    let is_nontrivial = |id: usize| id != zero && id != one;
    let is_nontrivial_constant =
        |id: usize| is_nontrivial(id) && builder.constants.values().any(|cell| cell.id == id);
    let is_variable = |id: usize| is_nontrivial(id) && !is_nontrivial_constant(id);
    let mut dense_shape_histogram = BTreeMap::<(usize, usize, usize), usize>::new();
    let mut dense_advice_occurrences = 0_usize;
    let mut dense_nonzero_products = 0_usize;
    let mut dense_fixed_fold_savings = 0_usize;
    let mut dense_global_occurrences = HashMap::<usize, usize>::new();
    for row in &post_slope_dense {
        let mut row_occurrences = 0_usize;
        let mut row_products = 0_usize;
        let mut row_constant_classes = std::collections::BTreeSet::<usize>::new();
        for (left, right) in row.products {
            if left.id == zero || right.id == zero {
                continue;
            }
            row_products += 1;
            for factor in [left, right] {
                if is_variable(factor.id) {
                    row_occurrences += 1;
                    *dense_global_occurrences.entry(factor.id).or_default() += 1;
                } else if is_nontrivial_constant(factor.id) {
                    row_constant_classes.insert(factor.id);
                }
            }
        }
        for cell in [row.accumulator, row.output] {
            if is_variable(cell.id) {
                row_occurrences += 1;
                *dense_global_occurrences.entry(cell.id).or_default() += 1;
            } else if is_nontrivial_constant(cell.id) {
                row_constant_classes.insert(cell.id);
            }
        }
        // At most one nontrivial constant coefficient can be supplied by the
        // one schedule fixed column not occupied by the typed lookup table or
        // opcode.  Every additional class needs an advice occurrence.
        let fixed_fold_saving = usize::from(!row_constant_classes.is_empty());
        let unfused_constant_occurrences = row_constant_classes.len();
        row_occurrences += unfused_constant_occurrences - fixed_fold_saving;
        dense_fixed_fold_savings += fixed_fold_saving;
        dense_advice_occurrences += row_occurrences;
        dense_nonzero_products += row_products;
        *dense_shape_histogram
            .entry((row_products, row_occurrences, row_constant_classes.len()))
            .or_default() += 1;
    }
    let dense_six_cell_lower = dense_advice_occurrences.div_ceil(6);
    eprintln!(
        "post-slope dense native ledger: rows={} nonzero-products={dense_nonzero_products} advice-occurrences={dense_advice_occurrences} fixed-fold-savings={dense_fixed_fold_savings} six-cell-row-lower={dense_six_cell_lower}",
        post_slope_dense.len(),
    );
    eprintln!(
        "post-slope dense shape histogram (products, advice, constant-classes): {dense_shape_histogram:#?}"
    );

    let mut linked_dense_state_ids = std::collections::BTreeSet::<usize>::new();
    let mut dense_output_to_row = HashMap::<usize, usize>::new();
    let mut dense_accumulator_to_row = HashMap::<usize, usize>::new();
    for (row_index, row) in post_slope_dense.iter().enumerate() {
        dense_output_to_row.insert(row.output.id, row_index);
        dense_accumulator_to_row.insert(row.accumulator.id, row_index);
    }
    for (&id, &count) in &dense_global_occurrences {
        if count == 2
            && global_occurrences.get(&id) == Some(&2)
            && dense_output_to_row.contains_key(&id)
            && dense_accumulator_to_row.contains_key(&id)
        {
            linked_dense_state_ids.insert(id);
        }
    }
    eprintln!(
        "post-slope dense state links: ids={} removable-current-row-occurrences={} residual-six-cell-lower={}",
        linked_dense_state_ids.len(),
        linked_dense_state_ids.len(),
        dense_advice_occurrences
            .saturating_sub(linked_dense_state_ids.len())
            .div_ceil(6),
    );

    // Prove the exact sparse overlay geometry.  Physical packets preserve the
    // source order, so the old accumulator is queried from the state column at
    // Rotation(-1).  A linked two-cubic packet therefore assigns only its
    // common gate, four factors, and new state in P6.  At a source-chain break
    // the old accumulator must be the authenticated zero constant.  A trailing
    // singleton keeps the unfused five-cell relation and fits a P5 partial row.
    // This keeps both boundary states constrained without the invalid
    // assumption that the terminal state of every same-gate chain is zero.
    let sparse_cell_cost = |cells: &[CellVar<Fp>]| {
        cells
            .iter()
            .filter(|cell| cell.id != zero && cell.id != one)
            .map(|cell| cell.id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    };
    let mut sparse_orientations = Vec::with_capacity(builder.sparse_rows.len());
    let mut sparse_carried_state = zero;
    for row in &builder.sparse_rows {
        let orientation = if row.accumulator.id == sparse_carried_state {
            SparseChainOrientation::Positive
        } else if row.output.id == sparse_carried_state {
            SparseChainOrientation::Reverse
        } else {
            // The previous modular relation handed a nonzero partial sum to
            // its Dense tail.  Every new modular relation restarts from zero.
            if row.accumulator.id == zero {
                SparseChainOrientation::Positive
            } else if row.output.id == zero {
                SparseChainOrientation::Reverse
            } else {
                panic!(
                    "sparse source order lost its authenticated state boundary: acc={} out={}",
                    row.accumulator.id, row.output.id
                );
            }
        };
        sparse_carried_state = match orientation {
            SparseChainOrientation::Positive => row.output.id,
            SparseChainOrientation::Reverse => row.accumulator.id,
            SparseChainOrientation::Singleton => unreachable!(),
        };
        sparse_orientations.push(orientation);
    }

    let sparse_old_new =
        |row: &SparseRow<Fp>, orientation: SparseChainOrientation| match orientation {
            SparseChainOrientation::Positive => (row.accumulator.id, row.output.id),
            SparseChainOrientation::Reverse => (row.output.id, row.accumulator.id),
            SparseChainOrientation::Singleton => unreachable!(),
        };
    let mut sparse_pair_current_costs = BTreeMap::<usize, usize>::new();
    let mut sparse_singleton_costs = BTreeMap::<usize, usize>::new();
    let mut sparse_gate_ids = std::collections::BTreeSet::<usize>::new();
    let mut sparse_pair_rows_exact = 0_usize;
    let mut sparse_singleton_rows_exact = 0_usize;
    let mut sparse_partial_spare_cells = 0_usize;
    let mut sparse_previous_new = None;
    let mut sparse_index = 0_usize;
    while sparse_index < builder.sparse_rows.len() {
        let row = &builder.sparse_rows[sparse_index];
        let orientation = sparse_orientations[sparse_index];
        sparse_gate_ids.insert(row.gate.id);
        let pair = builder
            .sparse_rows
            .get(sparse_index + 1)
            .filter(|_| sparse_orientations[sparse_index + 1] == orientation)
            .filter(|next| sparse_link(row, next, orientation));
        if let Some(next) = pair {
            sparse_gate_ids.insert(next.gate.id);
            let (old, _) = sparse_old_new(row, orientation);
            let (_, new) = sparse_old_new(next, orientation);
            assert!(
                old == zero || old == one || sparse_previous_new == Some(old),
                "nonconstant sparse old state is not available at Rotation(-1): old={old} previous={sparse_previous_new:?}"
            );
            let mut current = vec![
                row.gate,
                row.left,
                row.right,
                next.left,
                next.right,
                match orientation {
                    SparseChainOrientation::Positive => next.output,
                    SparseChainOrientation::Reverse => next.accumulator,
                    SparseChainOrientation::Singleton => unreachable!(),
                },
            ];
            if old != zero && old != one && sparse_previous_new != Some(old) {
                current.push(match orientation {
                    SparseChainOrientation::Positive => row.accumulator,
                    SparseChainOrientation::Reverse => row.output,
                    SparseChainOrientation::Singleton => unreachable!(),
                });
            }
            let current_cost = sparse_cell_cost(&current);
            assert!(
                current_cost <= 6,
                "two-cubic sparse packet escaped P6: {current_cost}"
            );
            *sparse_pair_current_costs.entry(current_cost).or_default() += 1;
            sparse_pair_rows_exact += 1;
            sparse_previous_new = Some(new);
            sparse_index += 2;
        } else {
            let cost =
                sparse_cell_cost(&[row.left, row.right, row.gate, row.accumulator, row.output]);
            assert!(cost <= 5, "singleton sparse row escaped P5: {cost}");
            *sparse_singleton_costs.entry(cost).or_default() += 1;
            sparse_singleton_rows_exact += 1;
            sparse_partial_spare_cells += 5 - cost;
            let (_, new) = sparse_old_new(row, orientation);
            sparse_previous_new = Some(new);
            sparse_index += 1;
        }
    }
    assert_eq!(sparse_pair_rows_exact, 12_762);
    assert_eq!(sparse_singleton_rows_exact, 5_598);
    eprintln!(
        "sparse A10 rotated-state ledger: paired={sparse_pair_rows_exact} pair-current-costs={sparse_pair_current_costs:?} singleton={sparse_singleton_rows_exact} singleton-costs={sparse_singleton_costs:?} singleton-partial-spare={sparse_partial_spare_cells} unique-gates={} max-rotation=-1",
        sparse_gate_ids.len(),
    );

    // Every remaining one-lane Sign relation is a pure booleanity witness.
    // The exact AggregateSlope removal above identifies its 1,194 is-zero
    // flags from the paired Dense equations, instead of granting an anonymous
    // row-count discount.  A sparse packet already assigns its gate, so one
    // occurrence of each matching gate can enforce b*(b-1)=0 without another
    // advice cell.  Count unique identities: repeated same-gate packets do not
    // earn duplicate capacity.
    let pure_boolean_ids = builder
        .sign_lanes
        .iter()
        .filter(|lane| lane.magnitude.id == zero && lane.signed.id == zero && lane.active.id == one)
        .map(|lane| lane.sign.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(pure_boolean_ids.len(), 11_061);
    assert!(slope_removed_boolean_ids.is_subset(&pure_boolean_ids));
    let post_slope_boolean_ids = pure_boolean_ids
        .difference(&slope_removed_boolean_ids)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(post_slope_boolean_ids.len(), 9_867);
    let sparse_gate_boolean_ids = sparse_gate_ids
        .intersection(&post_slope_boolean_ids)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();

    let mut post_slope_select_groups = BTreeMap::<usize, usize>::new();
    for lane in &builder.selects {
        *post_slope_select_groups.entry(lane.bit.id).or_default() += 1;
    }
    let removed_slope_select_groups = post_slope_select_groups
        .values()
        .filter(|size| **size == 3)
        .count();
    assert_eq!(removed_slope_select_groups, 398);
    let select_header_ids = post_slope_select_groups
        .iter()
        .filter(|(_, size)| **size != 3)
        .map(|(bit, _)| *bit)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(select_header_ids.len(), 917);
    eprintln!(
        "sparse/boolean identity ledger: post-slope-booleans={} unique-sparse-gate-coverage={} residual-booleans={} select-headers={} singleton-spare={sparse_partial_spare_cells}",
        post_slope_boolean_ids.len(),
        sparse_gate_boolean_ids.len(),
        post_slope_boolean_ids.len() - sparse_gate_boolean_ids.len(),
        select_header_ids.len(),
    );

    // Contract only adjacent Dense equations whose shared accumulator is a
    // globally-two-use identity.  Each contracted packet remains one explicit
    // polynomial equation with at most three nonzero products.  A packet may
    // use at most P6 persistent cells and A4 globally-single-use scratch cells;
    // two packets may share a physical row only when their union obeys both
    // limits, so the row still emits two independent constraints rather than
    // an unsound sum/fold of the equations.
    #[derive(Clone, Debug)]
    struct DensePacketDiagnostic {
        products: Vec<(usize, usize)>,
        linear: BTreeMap<usize, i32>,
        forced_persistent: std::collections::BTreeSet<usize>,
        source_rows: usize,
    }
    let dense_constant_ids = builder
        .constants
        .values()
        .map(|cell| cell.id)
        .collect::<std::collections::BTreeSet<_>>();
    let dense_poly_for_row = |row: &DenseRow<Fp>| {
        let products = row
            .products
            .iter()
            .filter(|(left, right)| left.id != zero && right.id != zero)
            .map(|(left, right)| (left.id, right.id))
            .collect::<Vec<_>>();
        let mut linear = BTreeMap::<usize, i32>::new();
        *linear.entry(row.accumulator.id).or_default() += 1;
        *linear.entry(row.output.id).or_default() -= 1;
        linear.retain(|_, coefficient| *coefficient != 0);
        DensePacketDiagnostic {
            products,
            linear,
            forced_persistent: std::collections::BTreeSet::new(),
            source_rows: 1,
        }
    };
    let merge_dense_packets = |left: &DensePacketDiagnostic, right: &DensePacketDiagnostic| {
        let mut products = left.products.clone();
        products.extend(right.products.iter().copied());
        let mut linear = left.linear.clone();
        for (id, coefficient) in &right.linear {
            *linear.entry(*id).or_default() += coefficient;
        }
        linear.retain(|_, coefficient| *coefficient != 0);
        let forced_persistent = left
            .forced_persistent
            .union(&right.forced_persistent)
            .copied()
            .collect();
        DensePacketDiagnostic {
            products,
            linear,
            forced_persistent,
            source_rows: left.source_rows + right.source_rows,
        }
    };
    let packet_ids = |packet: &DensePacketDiagnostic| {
        packet
            .products
            .iter()
            .flat_map(|(left, right)| [*left, *right])
            .chain(packet.linear.keys().copied())
            .filter(|id| *id != zero && *id != one)
            .collect::<std::collections::BTreeSet<_>>()
    };
    let packet_cost = |packets: &[&DensePacketDiagnostic], fold_one_fixed_class: bool| {
        let ids = packets
            .iter()
            .flat_map(|packet| packet_ids(packet))
            .collect::<std::collections::BTreeSet<_>>();
        let forced_persistent = packets
            .iter()
            .flat_map(|packet| packet.forced_persistent.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        let mut constants = ids
            .iter()
            .filter(|id| dense_constant_ids.contains(id))
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        // F2 supplies one authenticated coefficient class on native rows.
        // The pairing pass deliberately uses the stricter no-fold cost, so
        // two packets never depend on choosing incompatible fixed values.
        if fold_one_fixed_class {
            constants.pop_first();
        }
        let persistent = ids
            .iter()
            .filter(|id| {
                !dense_constant_ids.contains(id)
                    && (forced_persistent.contains(id)
                        || global_occurrences.get(id).copied().unwrap_or_default() > 1)
            })
            .count()
            + constants.len();
        let scratch = ids
            .iter()
            .filter(|id| {
                !dense_constant_ids.contains(id)
                    && !forced_persistent.contains(id)
                    && global_occurrences.get(id).copied().unwrap_or_default() <= 1
            })
            .count();
        (persistent, scratch)
    };

    let dense_rows = post_slope_dense;
    let split_dense_source = |row_index: usize, product_mask: usize| {
        let row = dense_rows[row_index];
        let source = dense_poly_for_row(row);
        let product_count = source.products.len();
        let all_products = (1_usize << product_count) - 1;
        if product_count < 2 || product_mask == 0 || product_mask == all_products {
            return None;
        }
        let synthetic = builder
            .next_id
            .checked_add(row_index)
            .expect("diagnostic synthetic identity fits usize");
        let mut first_products = Vec::new();
        let mut second_products = Vec::new();
        for (index, product) in source.products.iter().copied().enumerate() {
            if product_mask & (1 << index) == 0 {
                second_products.push(product);
            } else {
                first_products.push(product);
            }
        }
        let mut first_linear = BTreeMap::<usize, i32>::new();
        *first_linear.entry(row.accumulator.id).or_default() += 1;
        *first_linear.entry(synthetic).or_default() -= 1;
        first_linear.retain(|_, coefficient| *coefficient != 0);
        let mut second_linear = BTreeMap::<usize, i32>::new();
        *second_linear.entry(synthetic).or_default() += 1;
        *second_linear.entry(row.output.id).or_default() -= 1;
        second_linear.retain(|_, coefficient| *coefficient != 0);
        let forced_persistent = std::collections::BTreeSet::from([synthetic]);
        Some((
            DensePacketDiagnostic {
                products: first_products,
                linear: first_linear,
                forced_persistent: forced_persistent.clone(),
                source_rows: 1,
            },
            DensePacketDiagnostic {
                products: second_products,
                linear: second_linear,
                forced_persistent,
                source_rows: 0,
            },
        ))
    };
    let mut outputs = HashMap::<usize, Vec<usize>>::new();
    let mut accumulators = HashMap::<usize, Vec<usize>>::new();
    for (row_index, row) in dense_rows.iter().enumerate() {
        outputs.entry(row.output.id).or_default().push(row_index);
        accumulators
            .entry(row.accumulator.id)
            .or_default()
            .push(row_index);
    }
    let mut dense_adjacency = vec![Vec::<usize>::new(); dense_rows.len()];
    let mut exact_dense_links = 0_usize;
    for id in &linked_dense_state_ids {
        let output_rows = outputs.get(id).expect("linked output row exists");
        let accumulator_rows = accumulators.get(id).expect("linked accumulator row exists");
        assert_eq!(output_rows.len(), 1);
        assert_eq!(accumulator_rows.len(), 1);
        let left = output_rows[0];
        let right = accumulator_rows[0];
        assert_ne!(left, right);
        dense_adjacency[left].push(right);
        dense_adjacency[right].push(left);
        exact_dense_links += 1;
    }
    assert_eq!(exact_dense_links, linked_dense_state_ids.len());
    assert!(dense_adjacency.iter().all(|neighbors| neighbors.len() <= 2));

    let mut visited_dense = vec![false; dense_rows.len()];
    let mut dense_paths = Vec::<Vec<usize>>::new();
    for start in 0..dense_rows.len() {
        if visited_dense[start] || dense_adjacency[start].len() > 1 {
            continue;
        }
        let mut path = Vec::new();
        let mut previous = None;
        let mut current = start;
        loop {
            assert!(
                !visited_dense[current],
                "dense state-link graph contains a cycle"
            );
            visited_dense[current] = true;
            path.push(current);
            let next = dense_adjacency[current]
                .iter()
                .copied()
                .find(|candidate| Some(*candidate) != previous);
            let Some(next) = next else { break };
            previous = Some(current);
            current = next;
        }
        dense_paths.push(path);
    }
    assert!(
        visited_dense.iter().all(|visited| *visited),
        "dense state-link graph contains a closed cycle"
    );

    // Test the strongest occurrence-oriented static alternative directly:
    //
    //   p0*p1 + p2*p3 + p4*p5 + a0 + a1 + a2 + a3 = 0.
    //
    // Product factors occupy six fixed role cells, including authenticated
    // zero factors in unused slots. Repeated factor values are deliberately
    // charged as distinct permutation copies. Additive identities are charged
    // separately: with P6/L4 they cannot be copied through the permutation,
    // while with P10 they can. This distinguishes a coefficient/sign problem
    // from the equality-column topology problem instead of treating equal
    // source values as one freely addressable register.
    let mut static_dense_packets = Vec::<DensePacketDiagnostic>::new();
    for path in &dense_paths {
        let mut candidate = None::<DensePacketDiagnostic>;
        for row_index in path {
            let source = dense_poly_for_row(dense_rows[*row_index]);
            let merged = candidate.as_ref().map_or_else(
                || source.clone(),
                |packet| merge_dense_packets(packet, &source),
            );
            if merged.products.len() <= 3 {
                candidate = Some(merged);
            } else {
                static_dense_packets.push(
                    candidate
                        .replace(source)
                        .expect("a source Dense row has at most three products"),
                );
            }
        }
        if let Some(packet) = candidate {
            static_dense_packets.push(packet);
        }
    }
    let static_product_terms = static_dense_packets
        .iter()
        .map(|packet| packet.products.len())
        .sum::<usize>();
    assert_eq!(static_product_terms, 78_367);
    let mut static_product_histogram = BTreeMap::<usize, usize>::new();
    let mut static_linear_shape_histogram = BTreeMap::<(usize, usize, usize), usize>::new();
    let mut static_packet_cross_tab = BTreeMap::<(usize, usize, usize), usize>::new();
    let mut static_zero_linear_by_products = BTreeMap::<usize, usize>::new();
    let mut static_linear_coefficient_histogram = BTreeMap::<i32, usize>::new();
    let mut static_negative_identity_ids = std::collections::BTreeSet::<usize>::new();
    let mut static_nonunit_packets = 0_usize;
    let mut static_all_plus_packets = 0_usize;
    let mut static_p6_external_aliases = 0_usize;
    let mut static_max_linear_occurrences = 0_usize;
    for packet in &static_dense_packets {
        *static_product_histogram
            .entry(packet.products.len())
            .or_default() += 1;
        let mut positive = 0_usize;
        let mut negative = 0_usize;
        let mut copied_linear = 0_usize;
        let mut nonunit = false;
        for (id, coefficient) in packet.linear.iter().filter(|(id, _)| **id != zero) {
            *static_linear_coefficient_histogram
                .entry(*coefficient)
                .or_default() += 1;
            nonunit |= coefficient.unsigned_abs() != 1;
            if *coefficient > 0 {
                positive +=
                    usize::try_from(*coefficient).expect("positive Dense coefficient fits usize");
            } else {
                negative += usize::try_from(coefficient.unsigned_abs())
                    .expect("Dense coefficient magnitude fits usize");
                static_negative_identity_ids.insert(*id);
            }
            if dense_constant_ids.contains(id)
                || global_occurrences.get(id).copied().unwrap_or_default() > 1
            {
                copied_linear += usize::try_from(coefficient.unsigned_abs())
                    .expect("Dense coefficient magnitude fits usize");
            }
        }
        let linear_occurrences = positive + negative;
        static_max_linear_occurrences = static_max_linear_occurrences.max(linear_occurrences);
        *static_linear_shape_histogram
            .entry((positive, negative, copied_linear))
            .or_default() += 1;
        *static_packet_cross_tab
            .entry((packet.products.len(), positive, negative))
            .or_default() += 1;
        if positive == 0 && negative == 0 {
            *static_zero_linear_by_products
                .entry(packet.products.len())
                .or_default() += 1;
        }
        static_nonunit_packets += usize::from(nonunit);
        static_all_plus_packets += usize::from(negative == 0 && linear_occurrences <= 4);

        // An unused product pair can authenticate at most one A-slot value:
        // the other P cell must be the copy-constrained zero factor that makes
        // the otherwise-unused product vanish. Everything beyond those local
        // copy bridges needs a rotated register or an additional row.
        let local_copy_bridges = 3 - packet.products.len();
        static_p6_external_aliases += copied_linear.saturating_sub(local_copy_bridges);
    }
    assert!(static_max_linear_occurrences <= 4);

    // A single static P10 row gives every occurrence an authenticated copy
    // and uses the verifier challenge sampled after all ten first-phase
    // advice commitments.  Standalone native rows use the one-plus-two fixed
    // product split
    //
    //   E_0 = p0*p1 + a0 - a1
    //   E_1 = p2*p3 + p4*p5 + a2 - a3
    //   E_0 + theta * E_1 - instance = 0.
    //
    // The instance is nonzero only on Bind rows; ordinary native rows are
    // placed after the direct-instance prefix.  A three-product packet is
    // split around one equality-bound intermediate, while two independent
    // packets use E_0/E_1.  The split is fixed and no witness chooses a
    // coefficient.  Repeated factors occupy distinct P cells and every copy
    // is authenticated through the P10 permutation.
    assert_eq!(static_packet_cross_tab.values().sum::<usize>(), 39_530);
    assert_eq!(
        static_zero_linear_by_products.values().sum::<usize>(),
        11_636
    );
    assert!(
        static_packet_cross_tab
            .keys()
            .all(|(products, positive, negative)| {
                *products <= 3 && *positive <= 1 && *negative <= 1
            })
    );

    let static_packet_count_by_products = (0_usize..=3)
        .map(|products| {
            static_packet_cross_tab
                .iter()
                .filter(|((count, _, _), _)| *count == products)
                .map(|(_, count)| *count)
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    let static_zero_count_by_products = (0_usize..=3)
        .map(|products| {
            static_zero_linear_by_products
                .get(&products)
                .copied()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();

    // Greedy is exact for the fixed one-plus-two split: P3 packets consume
    // both equations through an intermediate and are standalone; P2 pairs
    // with P1/P0, and the remaining P1/P0 suffix pairs arbitrarily.
    let static_fixed_split_rows = |mut counts: [usize; 4]| {
        let mut pairs = BTreeMap::<(usize, usize), usize>::new();
        let three_product_rows = counts[3];
        counts[3] = 0;
        for (left, right) in [(2, 1), (2, 0)] {
            let paired = counts[left].min(counts[right]);
            counts[left] -= paired;
            counts[right] -= paired;
            pairs.insert((left, right), paired);
        }
        let paired_ones = counts[1] / 2;
        counts[1] -= paired_ones * 2;
        pairs.insert((1, 1), paired_ones);
        let paired_one_zero = counts[1].min(counts[0]);
        counts[1] -= paired_one_zero;
        counts[0] -= paired_one_zero;
        pairs.insert((1, 0), paired_one_zero);
        let paired_zeros = counts[0] / 2;
        counts[0] -= paired_zeros * 2;
        pairs.insert((0, 0), paired_zeros);
        let rows =
            three_product_rows + pairs.values().sum::<usize>() + counts.into_iter().sum::<usize>();
        (rows, pairs, counts)
    };

    // A zero-product, zero-linear packet is the zero polynomial obtained by
    // eliminating globally-two-use internal states.  It carries no remaining
    // relation and is removed rather than consuming an unconstrained row.
    let zero_polynomial_packets = static_zero_count_by_products[0];
    let mut initial_counts: [usize; 4] = static_packet_count_by_products
        .clone()
        .try_into()
        .expect("four product-count classes");
    initial_counts[0] -= zero_polynomial_packets;

    // Construct every overlapping row with fixed roles.  Exact-sum partial
    // rows use P0 for their carry state.  A two-source Sparse packet uses
    // P1..P5 plus A3: A3 is its Boolean gate, P5 is the new sparse state, and
    // P5@-1 is the old state.  Positive and reverse packets have separate
    // mutually-exclusive selectors, so the sign of old-new is
    // verifier-selected.  A Sparse singleton does not consume A3: P1..P3 are
    // gate/left/right, P4 is a canonical 0/1 direction constant, and P5 is the
    // new state.  Its fixed equation is
    //
    //   gate*left*right + (1-2*direction)*(old-new) = 0.
    //
    // It shares the exact-sum selector; on a range-only row gate/left/right
    // are authenticated zero and the state is merely propagated.  Select rows
    // use P0..P5 for the two triples (left,right,output) and A3 for their common
    // Boolean bit.  Pair-gate and Select-bit values remain sound Table15
    // lookups (0 or 1).  Each pair/Select row displaces exactly one tag-15
    // chunk from A3; four displaced chunks use one added full lookup row.
    assert!(
        builder
            .sparse_rows
            .iter()
            .all(|row| row.gate.value == Fp::from(0) || row.gate.value == Fp::from(1))
    );
    assert!(
        builder
            .selects
            .iter()
            .all(|lane| lane.bit.value == Fp::from(0) || lane.bit.value == Fp::from(1))
    );
    let mask = |columns: &[usize]| {
        columns
            .iter()
            .fold(0_u16, |mask, column| mask | (1_u16 << column))
    };
    let p0 = mask(&[0]);
    let p1_to_p5 = mask(&[1, 2, 3, 4, 5]);
    let p0_to_p5 = mask(&[0, 1, 2, 3, 4, 5]);
    let a0_to_a2 = mask(&[6, 7, 8]);
    let a3 = mask(&[9]);
    let a0_to_a3 = a0_to_a2 | a3;
    assert_eq!((p0 | p1_to_p5) & a0_to_a3, 0);
    assert_eq!(p1_to_p5 & (p0 | a0_to_a3), 0);
    assert_eq!((p1_to_p5 | a3) & (p0 | a0_to_a2), 0);
    assert_eq!((p0_to_p5 | a3) & a0_to_a2, 0);
    assert_eq!(p0_to_p5 & a0_to_a3, 0);
    assert_eq!((p0_to_p5 | a0_to_a3).count_ones(), 10);
    let static_select_pair_rows = builder.selects.len().saturating_sub(398 * 3) / 2;
    assert_eq!(static_select_pair_rows, 6_975);
    assert_eq!(sparse_pair_rows_exact + sparse_singleton_rows_exact, 18_360);
    assert_eq!(exact_sum_partial_rows, 19_050);
    assert!(remaining_large_batches * 5 >= static_select_pair_rows);
    let static_displaced_tag15_chunks = sparse_pair_rows_exact + static_select_pair_rows;
    let static_recovery_range_rows = static_displaced_tag15_chunks.div_ceil(4);
    let static_recovery_zero_padding =
        static_recovery_range_rows * 4 - static_displaced_tag15_chunks;
    assert_eq!(static_displaced_tag15_chunks, 19_737);
    assert_eq!(static_recovery_range_rows, 4_935);
    assert_eq!(static_recovery_zero_padding, 3);
    assert_eq!(static_displaced_tag15_chunks % 4, 1);

    // The original full lookup rows not used by Select, plus the recovery
    // rows, have all six P cells free.  They host every zero-linear P3 packet
    // under the one static equation p0*p1+p2*p3+p4*p5=0.  No P1/P2 special
    // opcode is needed.  Zero-polynomial packets disappear after exact state
    // elimination; every remaining packet uses the fixed standalone split.
    let static_full_dense_capacity = remaining_full_overlay_rows
        .saturating_sub(static_select_pair_rows)
        + static_recovery_range_rows;
    let static_dense_p3_overlays = static_zero_count_by_products[3];
    assert_eq!(static_full_dense_capacity, 10_205);
    assert_eq!(static_dense_p3_overlays, 6_911);
    assert!(static_dense_p3_overlays <= static_full_dense_capacity);
    initial_counts[3] -= static_dense_p3_overlays;
    let (static_dense_standalone, static_standalone_pair_histogram, static_standalone_unmatched) =
        static_fixed_split_rows(initial_counts);
    assert_eq!(static_dense_standalone, 21_079);

    // Sparse gates are checked Boolean in their own selector and each Select
    // row redundantly checks its shared bit.  Only the residual pure Boolean
    // identities remain metadata.  Unused full/recovery rows carry six at a
    // time under sum(theta^i * b_i*(b_i-1))=0; padding cells are permutation
    // copies of the authenticated zero.  All b_i were committed before theta,
    // so this is a degree-five-in-the-challenge sound RLC, not a deterministic
    // sum of constraints.
    let static_residual_boolean_cells =
        post_slope_boolean_ids.len() - sparse_gate_boolean_ids.len();
    let static_unused_full_rows = static_full_dense_capacity - static_dense_p3_overlays;
    let static_metadata_capacity = static_unused_full_rows * 6;
    let static_metadata_rows = static_residual_boolean_cells.div_ceil(6);
    assert_eq!(static_residual_boolean_cells, 9_732);
    assert_eq!(static_unused_full_rows, 3_294);
    assert_eq!(static_metadata_rows, 1_622);
    assert!(static_residual_boolean_cells <= static_metadata_capacity);

    let static_governed_rows = 396
        + exact_sum_fused_rows
        + remaining_range_rows
        + static_recovery_range_rows
        + static_dense_standalone;
    assert_eq!(static_governed_rows, 64_164);
    let static_assigned_rows = static_governed_rows.max(TABLE_ROWS);
    assert_eq!(static_assigned_rows, 65_365);
    assert_eq!(K16_MAX_ASSIGNED_ROWS - static_assigned_rows, 162);

    // Model the proposed transcript shape independently of the production
    // configuration.  P0..P5 and A2..A3 are equality-enabled; A0..A1 are
    // single-row scratch.  All ten columns remain FirstPhase, so theta is
    // sampled only after every operand commitment.  A range anchor queries
    // the four lookup lanes across its exact six-row chunk block.  An exact
    // sum anchor additionally reads the preceding limb anchor's packed carry
    // at Rotation(-6); sparse old/new states are both current-row cells and do
    // not claim an unassigned reset through Rotation(-1).
    let mut p8_meta = ConstraintSystem::<Fp>::default();
    let p8_advice: [Column<Advice>; 10] =
        std::array::from_fn(|_| p8_meta.advice_column_in(FirstPhase));
    for column in [
        p8_advice[0],
        p8_advice[1],
        p8_advice[2],
        p8_advice[3],
        p8_advice[4],
        p8_advice[5],
        p8_advice[8],
        p8_advice[9],
    ] {
        p8_meta.enable_equality(column);
    }
    let p8_instance = p8_meta.instance_column();
    // Opcode/compressed row selector, packed range scale, packed Table15, and
    // the witness-independent packed exact-sum constant.
    let p8_fixed: [Column<Fixed>; 4] = std::array::from_fn(|_| p8_meta.fixed_column());
    let p8_theta = p8_meta.challenge_usable_after(FirstPhase);
    p8_meta.create_gate("diagnostic P8 six-row range and native RLC", |meta| {
        let value = p8_advice.map(|column| meta.query_advice(column, Rotation::cur()));
        let public = meta.query_instance(p8_instance, Rotation::cur());
        let opcode = meta.query_fixed(p8_fixed[0], Rotation::cur());
        let range_scale = meta.query_fixed(p8_fixed[1], Rotation::cur());
        let packed_constant = meta.query_fixed(p8_fixed[3], Rotation::cur());
        let previous_packed_carry = meta.query_advice(p8_advice[4], Rotation(-6));
        let theta = meta.query_challenge(p8_theta);

        let chunk_radix = Expression::Constant(Fp::from(1_u64 << LOOKUP_BITS));
        let recomposed = std::array::from_fn::<_, 4, _>(|lane| {
            (-5..=0).fold(Expression::Constant(Fp::ZERO), |accumulator, rotation| {
                accumulator * chunk_radix.clone()
                    + meta.query_advice(p8_advice[6 + lane], Rotation(rotation))
            })
        });
        let recomposition_rlc = recomposed.iter().zip(&value[..4]).fold(
            Expression::Constant(Fp::ZERO),
            |accumulator, (recomposed, original)| {
                accumulator * theta.clone() + recomposed.clone() - original.clone()
            },
        );

        let exact_pair_scale =
            Expression::Constant(biguint_to_fe::<Fp>(&(BigUint::from(1_u8) << 90_usize)));
        let exact_limb_radix = Expression::Constant(biguint_to_fe::<Fp>(&radix()));
        let packed_exact_sum = value[0].clone()
            + value[1].clone()
            + exact_pair_scale.clone() * (value[2].clone() + value[3].clone())
            + previous_packed_carry
            - packed_constant
            - exact_limb_radix * value[4].clone();
        let carry = value[4].clone();
        let packed_carry_domain = carry.clone()
            * (carry.clone() - Expression::Constant(Fp::ONE))
            * (carry.clone() - exact_pair_scale.clone())
            * (carry - exact_pair_scale - Expression::Constant(Fp::ONE));

        let native_first =
            value[0].clone() * value[1].clone() + value[6].clone() - value[7].clone();
        let native_second = value[2].clone() * value[3].clone()
            + value[4].clone() * value[5].clone()
            + value[8].clone()
            - value[9].clone();
        vec![
            native_first + theta * native_second,
            opcode.clone() * (value[9].clone() - public),
            range_scale * recomposition_rlc,
            opcode.clone() * packed_exact_sum,
            // A production selector for this quartic has at most six roots;
            // multiplying by the quartic therefore remains degree ten.
            opcode * packed_carry_domain,
        ]
    });
    for (index, column) in p8_advice[6..10].iter().copied().enumerate() {
        p8_meta.lookup_any(format!("diagnostic packed Table15 lane {index}"), |meta| {
            let scale = meta.query_fixed(p8_fixed[1], Rotation::cur());
            let input = scale * meta.query_advice(column, Rotation::cur());
            let table = meta.query_fixed(p8_fixed[2], Rotation::cur());
            vec![(input, table)]
        });
    }
    p8_meta.set_minimum_degree(10);
    let p8_shape = pasta_ipa_augmented_proof_shape_v1(&p8_meta, K, PastaIpaInstanceQueryV1::Direct)
        .expect("diagnostic P8 six-row FirstPhase RLC shape");
    assert_eq!(p8_meta.num_challenges(), 1);
    assert_eq!(p8_meta.advice_column_phase(), vec![0; 10]);
    assert_eq!(p8_meta.challenge_phase(), vec![0]);
    assert_eq!(p8_meta.permutation().get_columns().len(), 8);
    assert_eq!(p8_meta.lookups().len(), 4);
    assert_eq!(p8_meta.advice_queries().len(), 31);
    assert_eq!(p8_meta.fixed_queries().len(), 4);
    assert!(p8_shape.ordinary_proof_bytes() <= 4_544);

    let p8_processed_pk_polynomials = 3 + 2 * 4 + 2 * 8;
    let p8_raw_pk_polynomial_bytes = p8_processed_pk_polynomials * (1_usize << K) * 32;
    let p8_processed_pk_frame_bytes = p8_processed_pk_polynomials * 4 + 4 * 4;
    let p8_processed_pk_governed_upper =
        p8_raw_pk_polynomial_bytes + p8_processed_pk_frame_bytes + 64 * 1024;
    assert_eq!(p8_processed_pk_polynomials, 27);
    assert_eq!(p8_raw_pk_polynomial_bytes, 56_623_104);
    assert_eq!(p8_processed_pk_frame_bytes, 124);
    assert_eq!(p8_processed_pk_governed_upper, 56_688_764);
    assert!(p8_processed_pk_governed_upper < 64 * 1024 * 1024);

    // One packed fixed table remains type-sound.  Width i uses scale 2^(16i),
    // so every nonzero encoded value lies in a disjoint integer interval below
    // 2^175 and therefore below both Pasta moduli.  Dense/Bind rows set scale
    // zero, making every otherwise arbitrary lookup-column value hit the
    // authenticated zero sentinel.
    let mut packed_table_values = std::collections::HashSet::<BigUint>::new();
    let mut packed_table_rows = 1_usize;
    for (index, bits) in RANGE_CHUNK_BITS.into_iter().enumerate() {
        let scale = BigUint::from(1_u8) << (16 * index);
        for value in 1_u64..(1_u64 << bits) {
            let encoded = &scale * value;
            assert!(
                packed_table_values.insert(encoded),
                "packed typed Table15 values must not collide"
            );
        }
        packed_table_rows += 1_usize << bits;
    }
    assert_eq!(packed_table_rows, TABLE_ROWS);
    let packed_table_maximum = (BigUint::from(1_u8) << (16 * (RANGE_CHUNK_BITS.len() - 1)))
        * ((BigUint::from(1_u8) << LOOKUP_BITS) - BigUint::from(1_u8));
    assert!(packed_table_maximum < modulus::<Fp>());
    assert!(packed_table_maximum < modulus::<Fq>());

    // Seven mutually-exclusive simple row selectors are compressed into one
    // authenticated fixed column at degree ten.  The two explicit fixed
    // columns are the per-row range scale and the packed table, giving F3.
    // The selector roles are: native/Bind, exact-sum range plus Sparse
    // singleton, positive Sparse pair, reverse Sparse pair, Select,
    // zero-linear Dense3, and Boolean metadata.
    let mut selector_meta = ConstraintSystem::<Fp>::default();
    let selector_values: [Column<Advice>; 3] =
        std::array::from_fn(|_| selector_meta.advice_column());
    let _range_scale = selector_meta.fixed_column();
    let _packed_table = selector_meta.fixed_column();
    let row_selectors = std::array::from_fn::<_, 7, _>(|_| selector_meta.selector());
    for (index, selector) in row_selectors.into_iter().enumerate() {
        selector_meta.create_gate(
            format!("diagnostic compressed row selector {index}"),
            |meta| {
                let q = meta.query_selector(selector);
                let left = meta.query_advice(selector_values[0], Rotation::cur());
                let middle = meta.query_advice(selector_values[1], Rotation::cur());
                let right = meta.query_advice(selector_values[2], Rotation::cur());
                if index == 2 || index == 3 {
                    vec![q * left * middle * right]
                } else {
                    vec![q * left * middle]
                }
            },
        );
    }
    selector_meta.set_minimum_degree(10);
    let selector_assignments = (0..7)
        .map(|selector| (0..8).map(|row| row == selector).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let (compressed_selector_meta, compressed_selector_polys) =
        selector_meta.compress_selectors(selector_assignments);
    assert_eq!(compressed_selector_polys.len(), 1);
    assert_eq!(compressed_selector_meta.num_fixed_columns(), 3);
    assert_eq!(compressed_selector_meta.degree(), 10);

    eprintln!(
        "counterfactual P10 allocation plus instantiated P8/F4 shape: packet-cross-tab(products,+,-)={static_packet_cross_tab:?} zero-linear-by-products={static_zero_linear_by_products:?} zero-polynomials={zero_polynomial_packets} sparse-partial={} select-full={static_select_pair_rows} displaced-tag15={static_displaced_tag15_chunks} recovery-range={static_recovery_range_rows} recovery-zero-pad={static_recovery_zero_padding} full-dense-capacity={static_full_dense_capacity} dense-p3-overlays={static_dense_p3_overlays} metadata-cells={static_residual_boolean_cells} metadata-rows={static_metadata_rows} metadata-capacity={static_metadata_capacity} standalone-pairs={static_standalone_pair_histogram:?} standalone-unmatched={static_standalone_unmatched:?} dense-standalone={static_dense_standalone} governed-semantic={static_governed_rows} assigned-with-table={static_assigned_rows} headroom={} compressed-selectors={} p8-proof-points={} p8-proof-scalars={} p8-proof-bytes={} p8-point-sets={} p8-raw-pk-polynomial-bytes={p8_raw_pk_polynomial_bytes} p8-processed-pk-governed-upper={p8_processed_pk_governed_upper}",
        sparse_pair_rows_exact + sparse_singleton_rows_exact,
        K16_MAX_ASSIGNED_ROWS - static_assigned_rows,
        compressed_selector_polys.len(),
        p8_shape.commitments(),
        p8_shape.evaluations(),
        p8_shape.ordinary_proof_bytes(),
        p8_shape.point_sets(),
    );
    assert!(
        static_governed_rows <= K16_MAX_ASSIGNED_ROWS,
        "sound P10 static RLC schedule exceeds k16: {static_governed_rows}"
    );

    let static_product_row_lower = static_product_terms.div_ceil(3);
    let static_packet_row_lower = static_dense_packets.len().div_ceil(2);
    let static_p10_dual_constraint_lower = static_product_row_lower.max(static_packet_row_lower);
    let static_no_dense_overlay_base = 396_usize + 37_754 + 7_492;
    let static_p10_governed_lower = static_no_dense_overlay_base + static_p10_dual_constraint_lower;
    let static_all_plus_normalization_lower = static_negative_identity_ids.len();
    eprintln!(
        "dense single-static-form obstruction: packets={} products={static_product_terms} product-hist={static_product_histogram:?} linear-shapes(+,-,copied)={static_linear_shape_histogram:?} coefficient-hist={static_linear_coefficient_histogram:?} all-plus-packets={static_all_plus_packets} nonunit-packets={static_nonunit_packets} unique-negative-identities={} all-plus-normalization-row-lower={static_all_plus_normalization_lower} p6-external-aliases={static_p6_external_aliases} p10-dual-row-lower={static_p10_dual_constraint_lower} no-overlay-governed-lower={static_p10_governed_lower}",
        static_dense_packets.len(),
        static_negative_identity_ids.len(),
    );
    assert!(
        static_p10_governed_lower > K16_MAX_ASSIGNED_ROWS,
        "the occurrence-oriented static form unexpectedly fits k16 and must be implemented"
    );

    // Find a minimum-cardinality sound partition for each exact state path.  A
    // source P7 equation is not rejected prematurely: a neighboring linear
    // equation may cancel their globally-two-use state and bring the merged
    // polynomial back to P6.  A three-product source can also be split around
    // one fresh, copy-constrained intermediate t:
    //
    //   selected_products + accumulator - t = 0
    //   remaining_products + t - output = 0
    //
    // Eliminating t recovers the source equation exactly.  Both fragments are
    // charged as rows and independently checked against products<=3/P6/A4.
    #[derive(Clone, Copy, Debug)]
    enum DensePartitionChoice {
        Root,
        Interval { start: usize },
        Split { start: usize, product_mask: usize },
    }
    #[derive(Clone, Copy, Debug)]
    struct DensePartitionBest {
        packets: usize,
        choice: DensePartitionChoice,
    }
    let legal_dense_splits = |row_index: usize| {
        let source = dense_poly_for_row(dense_rows[row_index]);
        let all_products = (1_usize << source.products.len()) - 1;
        (1..all_products)
            .filter_map(|product_mask| {
                let (first, second) = split_dense_source(row_index, product_mask)?;
                let (first_persistent, first_scratch) = packet_cost(&[&first], true);
                let (second_persistent, second_scratch) = packet_cost(&[&second], true);
                (first.products.len() <= 3
                    && first_persistent <= 6
                    && first_scratch <= 4
                    && second.products.len() <= 3
                    && second_persistent <= 6
                    && second_scratch <= 4)
                    .then_some((product_mask, first, second))
            })
            .collect::<Vec<_>>()
    };
    let mut dense_packets = Vec::<DensePacketDiagnostic>::new();
    let mut dense_packet_source_histogram = BTreeMap::<usize, usize>::new();
    let mut dense_source_shape_histogram = BTreeMap::<(usize, usize, usize), usize>::new();
    let mut dense_source_cover_count = vec![0_usize; dense_rows.len()];
    let mut dense_p7_source_rows = std::collections::BTreeSet::<usize>::new();
    let mut dense_unfit_source_rows = 0_usize;
    let mut dense_split_eligible_sources = 0_usize;
    for row_index in 0..dense_rows.len() {
        let source = dense_poly_for_row(dense_rows[row_index]);
        let (persistent, scratch) = packet_cost(&[&source], true);
        *dense_source_shape_histogram
            .entry((source.products.len(), persistent, scratch))
            .or_default() += 1;
        if persistent == 7 {
            dense_p7_source_rows.insert(row_index);
        }
        if source.products.len() > 3 || persistent > 6 || scratch > 4 {
            dense_unfit_source_rows += 1;
            dense_split_eligible_sources += usize::from(!legal_dense_splits(row_index).is_empty());
        }
    }
    let mut dense_split_sources_chosen = 0_usize;
    let mut dense_p7_split_sources_chosen = 0_usize;
    let mut dense_p7_interval_sources_chosen = 0_usize;
    for (path_index, path) in dense_paths.iter().enumerate() {
        let mut best = vec![None::<DensePartitionBest>; path.len() + 1];
        best[0] = Some(DensePartitionBest {
            packets: 0,
            choice: DensePartitionChoice::Root,
        });
        for start in 0..path.len() {
            let Some(prefix) = best[start] else {
                continue;
            };
            let mut candidate = None::<DensePacketDiagnostic>;
            for end in start..path.len() {
                let row_packet = dense_poly_for_row(dense_rows[path[end]]);
                candidate = Some(match candidate {
                    Some(ref packet) => merge_dense_packets(packet, &row_packet),
                    None => row_packet,
                });
                let packet = candidate.as_ref().expect("candidate packet exists");
                if packet.products.len() > 3 {
                    break;
                }
                let (persistent, scratch) = packet_cost(&[packet], true);
                if persistent > 6 || scratch > 4 {
                    // A longer interval may cancel another linear state, so a
                    // P/A miss is not monotone and cannot stop this scan.
                    continue;
                }
                let boundary = end + 1;
                let proposed = prefix.packets + 1;
                let replace = best[boundary].is_none_or(|current| proposed < current.packets);
                if replace {
                    best[boundary] = Some(DensePartitionBest {
                        packets: proposed,
                        choice: DensePartitionChoice::Interval { start },
                    });
                }
            }

            for (product_mask, _, _) in legal_dense_splits(path[start]) {
                let boundary = start + 1;
                let proposed = prefix.packets + 2;
                let replace = best[boundary].is_none_or(|current| proposed < current.packets);
                if replace {
                    best[boundary] = Some(DensePartitionBest {
                        packets: proposed,
                        choice: DensePartitionChoice::Split {
                            start,
                            product_mask,
                        },
                    });
                }
            }
        }

        if best[path.len()].is_none() {
            let reachable = best
                .iter()
                .rposition(Option::is_some)
                .expect("empty prefix is reachable");
            let first_sources = path[reachable..]
                .iter()
                .take(4)
                .map(|row_index| {
                    let packet = dense_poly_for_row(dense_rows[*row_index]);
                    let (persistent, scratch) = packet_cost(&[&packet], true);
                    (
                        *row_index,
                        packet.products.len(),
                        persistent,
                        scratch,
                        legal_dense_splits(*row_index).len(),
                    )
                })
                .collect::<Vec<_>>();
            panic!(
                "dense A10 partition failed: path={path_index} reachable={reachable}/{} first-sources=(row,products,P,A,legal-splits){first_sources:?}",
                path.len()
            );
        }

        let expected_packets = best[path.len()]
            .expect("terminal path boundary is reachable")
            .packets;
        let mut choices = Vec::<(usize, DensePartitionChoice)>::new();
        let mut end = path.len();
        while end > 0 {
            let choice = best[end]
                .expect("reachable terminal has a predecessor")
                .choice;
            let start = match choice {
                DensePartitionChoice::Root => unreachable!(),
                DensePartitionChoice::Interval { start }
                | DensePartitionChoice::Split { start, .. } => start,
            };
            assert!(start < end);
            choices.push((end, choice));
            end = start;
        }
        choices.reverse();
        let packets_before_path = dense_packets.len();
        for (end, choice) in choices {
            match choice {
                DensePartitionChoice::Root => unreachable!(),
                DensePartitionChoice::Interval { start } => {
                    let mut rows = path[start..end].iter();
                    let first = *rows.next().expect("partition interval is nonempty");
                    let mut packet = dense_poly_for_row(dense_rows[first]);
                    dense_source_cover_count[first] += 1;
                    dense_p7_interval_sources_chosen +=
                        usize::from(dense_p7_source_rows.contains(&first));
                    for row_index in rows {
                        dense_source_cover_count[*row_index] += 1;
                        dense_p7_interval_sources_chosen +=
                            usize::from(dense_p7_source_rows.contains(row_index));
                        packet = merge_dense_packets(
                            &packet,
                            &dense_poly_for_row(dense_rows[*row_index]),
                        );
                    }
                    let (persistent, scratch) = packet_cost(&[&packet], true);
                    assert!(packet.products.len() <= 3 && persistent <= 6 && scratch <= 4);
                    *dense_packet_source_histogram
                        .entry(packet.source_rows)
                        .or_default() += 1;
                    dense_packets.push(packet);
                }
                DensePartitionChoice::Split {
                    start,
                    product_mask,
                } => {
                    assert_eq!(end, start + 1);
                    dense_source_cover_count[path[start]] += 1;
                    let (first, second) = split_dense_source(path[start], product_mask)
                        .expect("chosen split is structurally valid");
                    for packet in [first, second] {
                        let (persistent, scratch) = packet_cost(&[&packet], true);
                        assert!(packet.products.len() <= 3 && persistent <= 6 && scratch <= 4);
                        *dense_packet_source_histogram
                            .entry(packet.source_rows)
                            .or_default() += 1;
                        dense_packets.push(packet);
                    }
                    dense_split_sources_chosen += 1;
                    dense_p7_split_sources_chosen +=
                        usize::from(dense_p7_source_rows.contains(&path[start]));
                }
            }
        }
        assert_eq!(dense_packets.len() - packets_before_path, expected_packets);
    }
    assert!(
        dense_source_cover_count.iter().all(|count| *count == 1),
        "dense partition must preserve every source equation exactly once"
    );
    assert_eq!(
        dense_p7_interval_sources_chosen + dense_p7_split_sources_chosen,
        dense_p7_source_rows.len(),
        "every P7 source must be rescued by a legal interval contraction or an exact split"
    );

    let mut packet_buckets = BTreeMap::<(usize, usize), Vec<usize>>::new();
    for (index, packet) in dense_packets.iter().enumerate() {
        packet_buckets
            .entry(packet_cost(&[packet], false))
            .or_default()
            .push(index);
    }
    let mut dense_physical_bins = Vec::<Vec<usize>>::new();
    while let Some(left_key) = packet_buckets.keys().next_back().copied() {
        let left = packet_buckets
            .get_mut(&left_key)
            .expect("selected packet bucket exists")
            .pop()
            .expect("selected packet bucket is nonempty");
        if packet_buckets[&left_key].is_empty() {
            packet_buckets.remove(&left_key);
        }
        let mut bin = vec![left];
        let partner_key = packet_buckets
            .keys()
            .rev()
            .copied()
            .find(|(persistent, scratch)| {
                left_key.0 + *persistent <= 6 && left_key.1 + *scratch <= 4
            });
        if let Some(partner_key) = partner_key {
            let right = packet_buckets
                .get_mut(&partner_key)
                .expect("selected partner bucket exists")
                .pop()
                .expect("selected partner bucket is nonempty");
            if packet_buckets[&partner_key].is_empty() {
                packet_buckets.remove(&partner_key);
            }
            bin.push(right);
        }
        dense_physical_bins.push(bin);
    }
    let mut dense_bin_costs = BTreeMap::<(usize, usize, usize), usize>::new();
    let mut dense_partial_overlay_rows = 0_usize;
    let mut dense_partial_overlay_spare = 0_usize;
    for bin in &dense_physical_bins {
        let refs = bin
            .iter()
            .map(|index| &dense_packets[*index])
            .collect::<Vec<_>>();
        let (persistent, scratch) = packet_cost(&refs, true);
        assert!(persistent <= 6 && scratch <= 4);
        *dense_bin_costs
            .entry((bin.len(), persistent, scratch))
            .or_default() += 1;
        // Range lookup rows occupy all four scratch columns.  Only bins with
        // no A4 demand and at most five P cells may use a fused partial row.
        if scratch == 0 && persistent <= 5 {
            dense_partial_overlay_rows += 1;
            dense_partial_overlay_spare += 5 - persistent;
        }
    }

    // Cell capacity alone does not prove that a static Plonkish gate can wire
    // every packet.  Canonicalize the exact quadratic graph in each physical
    // bin under all renamings of the P6 equality registers and A4 local
    // registers.  The resulting class count is the minimum number of native
    // wiring templates before any further algebraic normalization; it keeps a
    // numerically successful allocation from being mistaken for an
    // implementable F3/degree-ten gate schedule.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum DenseTemplateAtom {
        One,
        FoldedFixed,
        Persistent(usize),
        Scratch(usize),
    }
    type DenseTemplateEquation = (
        Vec<(DenseTemplateAtom, DenseTemplateAtom)>,
        Vec<(DenseTemplateAtom, i32)>,
    );
    type DenseTemplateBin = Vec<DenseTemplateEquation>;
    fn next_permutation(values: &mut [usize]) -> bool {
        let Some(pivot) = (0..values.len().saturating_sub(1))
            .rev()
            .find(|index| values[*index] < values[*index + 1])
        else {
            return false;
        };
        let successor = (pivot + 1..values.len())
            .rev()
            .find(|index| values[*index] > values[pivot])
            .expect("nonterminal permutation has a pivot successor");
        values.swap(pivot, successor);
        values[pivot + 1..].reverse();
        true
    }
    let canonical_dense_bin = |bin: &[usize]| {
        let packets = bin
            .iter()
            .map(|index| &dense_packets[*index])
            .collect::<Vec<_>>();
        let ids = packets
            .iter()
            .flat_map(|packet| packet_ids(packet))
            .collect::<std::collections::BTreeSet<_>>();
        let forced_persistent = packets
            .iter()
            .flat_map(|packet| packet.forced_persistent.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        let folded_fixed = ids
            .iter()
            .find(|id| dense_constant_ids.contains(id))
            .copied();
        let persistent_ids = ids
            .iter()
            .filter(|id| {
                Some(**id) != folded_fixed
                    && (dense_constant_ids.contains(id)
                        || forced_persistent.contains(id)
                        || global_occurrences.get(id).copied().unwrap_or_default() > 1)
            })
            .copied()
            .collect::<Vec<_>>();
        let scratch_ids = ids
            .iter()
            .filter(|id| {
                Some(**id) != folded_fixed
                    && !dense_constant_ids.contains(id)
                    && !forced_persistent.contains(id)
                    && global_occurrences.get(id).copied().unwrap_or_default() <= 1
            })
            .copied()
            .collect::<Vec<_>>();
        assert!(persistent_ids.len() <= 6 && scratch_ids.len() <= 4);

        let mut persistent_permutation = (0..persistent_ids.len()).collect::<Vec<_>>();
        let mut best_signature = None::<DenseTemplateBin>;
        loop {
            let persistent_labels = persistent_ids
                .iter()
                .copied()
                .zip(persistent_permutation.iter().copied())
                .collect::<HashMap<_, _>>();
            let mut scratch_permutation = (0..scratch_ids.len()).collect::<Vec<_>>();
            loop {
                let scratch_labels = scratch_ids
                    .iter()
                    .copied()
                    .zip(scratch_permutation.iter().copied())
                    .collect::<HashMap<_, _>>();
                let atom = |id: usize| {
                    if id == one {
                        DenseTemplateAtom::One
                    } else if Some(id) == folded_fixed {
                        DenseTemplateAtom::FoldedFixed
                    } else if let Some(index) = persistent_labels.get(&id) {
                        DenseTemplateAtom::Persistent(*index)
                    } else {
                        DenseTemplateAtom::Scratch(
                            *scratch_labels
                                .get(&id)
                                .expect("every nonzero dense identity has a register class"),
                        )
                    }
                };
                let mut signature = packets
                    .iter()
                    .map(|packet| {
                        let mut products = packet
                            .products
                            .iter()
                            .map(|(left, right)| {
                                let mut pair = [atom(*left), atom(*right)];
                                pair.sort_unstable();
                                (pair[0], pair[1])
                            })
                            .collect::<Vec<_>>();
                        products.sort_unstable();
                        let mut linear = packet
                            .linear
                            .iter()
                            .filter(|(id, _)| **id != zero)
                            .map(|(id, coefficient)| (atom(*id), *coefficient))
                            .collect::<Vec<_>>();
                        linear.sort_unstable();
                        (products, linear)
                    })
                    .collect::<DenseTemplateBin>();
                signature.sort_unstable();
                if best_signature
                    .as_ref()
                    .is_none_or(|current| signature < *current)
                {
                    best_signature = Some(signature);
                }
                if !next_permutation(&mut scratch_permutation) {
                    break;
                }
            }
            if !next_permutation(&mut persistent_permutation) {
                break;
            }
        }
        best_signature.expect("identity permutation search emits one signature")
    };
    let mut dense_template_histogram = BTreeMap::<DenseTemplateBin, usize>::new();
    for bin in &dense_physical_bins {
        *dense_template_histogram
            .entry(canonical_dense_bin(bin))
            .or_default() += 1;
    }
    let mut dense_template_frequency = dense_template_histogram
        .iter()
        .map(|(template, count)| (*count, template))
        .collect::<Vec<_>>();
    dense_template_frequency.sort_unstable_by(|left, right| right.cmp(left));
    let dense_template_top = dense_template_frequency
        .into_iter()
        .take(12)
        .collect::<Vec<_>>();
    eprintln!(
        "dense static wiring classes: unique={} top12={dense_template_top:?}",
        dense_template_histogram.len(),
    );
    assert!(
        dense_physical_bins.len() <= 32_647,
        "sound A10 dense allocation exceeds the k16 closure target: {}",
        dense_physical_bins.len()
    );
    eprintln!(
        "dense A10 deterministic allocation: source-rows={} source-shapes={dense_source_shape_histogram:?} unfit-sources={dense_unfit_source_rows} split-eligible={dense_split_eligible_sources} p7-sources={} p7-via-interval={dense_p7_interval_sources_chosen} p7-via-split={dense_p7_split_sources_chosen} split-sources-chosen={dense_split_sources_chosen} links={} paths={} contracted-packets={} packet-source-rows={dense_packet_source_histogram:?} physical-bins={} pair-bins={} partial-compatible={} partial-spare={} bin-costs={dense_bin_costs:?}",
        dense_rows.len(),
        dense_p7_source_rows.len(),
        exact_dense_links,
        dense_paths.len(),
        dense_packets.len(),
        dense_physical_bins.len(),
        dense_physical_bins
            .iter()
            .filter(|bin| bin.len() == 2)
            .count(),
        dense_partial_overlay_rows,
        dense_partial_overlay_spare,
    );

    let post_slope_booleans = post_slope_boolean_ids.len();
    let select_headers = select_header_ids.len();
    let full_overlay_rows = remaining_full_overlay_rows;
    let partial_overlay_rows = exact_sum_partial_rows;
    let select_pair_rows = 6_975_usize;
    assert!(full_overlay_rows >= select_pair_rows);
    let sparse_pairs_in_full = full_overlay_rows - select_pair_rows;
    let standalone_sparse_rows = sparse_pair_rows_exact - sparse_pairs_in_full;
    let sparse_rows_in_partial = sparse_singleton_rows_exact;
    assert!(sparse_rows_in_partial <= partial_overlay_rows);
    let partial_rows_after_sparse = partial_overlay_rows - sparse_rows_in_partial;
    let residual_boolean_cells = post_slope_booleans.saturating_sub(sparse_gate_boolean_ids.len());
    let metadata_cells = residual_boolean_cells + select_headers;
    let metadata_spare_without_dense = sparse_partial_spare_cells + partial_rows_after_sparse * 5;
    assert!(metadata_spare_without_dense >= metadata_cells);
    let metadata_only_rows = metadata_cells
        .saturating_sub(sparse_partial_spare_cells)
        .div_ceil(5);
    let dense_overlay_rows_available = partial_rows_after_sparse - metadata_only_rows;
    assert!(dense_partial_overlay_rows >= dense_overlay_rows_available);
    let standalone_dense_rows = dense_physical_bins
        .len()
        .saturating_sub(dense_overlay_rows_available);
    let governed_rows = 396
        + exact_sum_fused_rows
        + remaining_range_rows
        + standalone_sparse_rows
        + standalone_dense_rows;
    assert!(
        governed_rows <= K16_MAX_ASSIGNED_ROWS,
        "mechanical A10 schedule exceeds k16: {governed_rows}"
    );
    eprintln!(
        "A10 governed allocation: bind=396 range={} select-overlay={select_pair_rows} sparse-full={sparse_pairs_in_full} sparse-standalone={standalone_sparse_rows} sparse-partial={sparse_rows_in_partial} post-slope-booleans={post_slope_booleans} sparse-gate-coverage={} select-headers={select_headers} residual-metadata={metadata_cells} metadata-only-partial={metadata_only_rows} dense-partial-available={dense_overlay_rows_available} dense-standalone={standalone_dense_rows} governed={governed_rows} headroom={}",
        exact_sum_fused_rows + remaining_range_rows,
        sparse_gate_boolean_ids.len(),
        K16_MAX_ASSIGNED_ROWS - governed_rows,
    );
}

fn instance_partition<F: BigPrimeField>() -> (Vec<F>, Vec<F>) {
    let vector = rfc6979_sample();
    P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature)
        .instance_partition_for_test()
        .expect("instance contract is derivable before transpose")
}

#[test]
fn instance_contract_is_161_caller_bytes_plus_exactly_235_derived_constants() {
    let vector = rfc6979_sample();
    let (fp_caller, fp_tail) = instance_partition::<Fp>();
    let (fq_caller, fq_tail) = instance_partition::<Fq>();
    assert_eq!(fp_caller.len(), PUBLIC_BYTES);
    assert_eq!(fq_caller.len(), PUBLIC_BYTES);
    assert_eq!(fp_tail.len(), 235);
    assert_eq!(fq_tail.len(), 235);
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

#[test]
fn privately_declared_source_hash_is_pinned_as_source_only_evidence() {
    // This hard-coded pin identifies the settled reviewed source bytes. The
    // runnable topology/tail, semantic KAT, rejection KAT, and key-generation
    // tests below independently exercise runtime evidence.
    const SOURCE_ONLY_SHA256_HEX: &str =
        "9c54b4b7a6decdd707af47d371d9b786352fb4b35c9d16662d5f5496fe1f02cd";
    let actual: [u8; 32] = Sha256::digest(include_bytes!("p256_packed_affine_v3.rs")).into();
    assert_eq!(actual, decode_hex::<32>(SOURCE_ONLY_SHA256_HEX));
}

fn update_usize_identity(hasher: &mut Sha256, value: usize) {
    hasher.update(
        u64::try_from(value)
            .expect("canonical identity value fits u64")
            .to_le_bytes(),
    );
}

fn canonical_runtime_topology_tail_digest<F: BigPrimeField>(
    topology: &CanonicalTraceTopologyV3,
    constant_tail: &[F],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"iroha.offline_cash.p256_packed_affine_v3.topology_tail.v2");
    update_usize_identity(&mut hasher, topology.rows.len());
    update_usize_identity(&mut hasher, topology.equality_classes);
    for row in &topology.rows {
        hasher.update((row.opcode as u64).to_le_bytes());
        update_usize_identity(&mut hasher, row.range_bits);
        for class in row.equality_alias_classes {
            hasher.update(
                class
                    .map_or(u64::MAX, |class| {
                        u64::try_from(class).expect("canonical equality class fits u64")
                    })
                    .to_le_bytes(),
            );
        }
    }
    hasher.update(b"typed-range-table");
    update_usize_identity(&mut hasher, TABLE_ROWS);
    let mut table_rows = 0_usize;
    for (first, second) in typed_range_table_rows_v3() {
        hasher.update(first.to_le_bytes());
        hasher.update(second.to_le_bytes());
        table_rows += 1;
    }
    assert_eq!(table_rows, TABLE_ROWS);
    hasher.update(b"verifier-derived-constant-tail");
    update_usize_identity(&mut hasher, constant_tail.len());
    for value in constant_tail {
        hasher.update(value.to_repr().as_ref());
    }
    hasher.finalize().into()
}

fn assert_without_witnesses_topology_and_constant_tail<F: BigPrimeField>()
-> CanonicalTraceTopologyV3 {
    let vector = rfc6979_sample();
    let valid =
        P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature);
    let without_witnesses = valid.without_witnesses();
    let (valid_rows, valid_topology) = valid
        .trace_and_topology_for_test()
        .expect("valid witness has the reviewed full-trace topology");
    let (empty_rows, empty_topology) = without_witnesses
        .trace_and_topology_for_test()
        .expect("without_witnesses has the reviewed full-trace topology");
    // `maximum_coefficient_bits` is a witness-observed diagnostic rather than
    // circuit geometry: the all-zero key-generation witness can legitimately
    // exercise a smaller accumulator than the RFC6979 KAT. Compare every row
    // and relation invariant after excluding only that bounded observation.
    assert!(valid_rows.maximum_coefficient_bits <= PACKED_COEFFICIENT_BOUND_BITS);
    assert!(empty_rows.maximum_coefficient_bits <= PACKED_COEFFICIENT_BOUND_BITS);
    let mut valid_geometry = valid_rows;
    let mut empty_geometry = empty_rows;
    valid_geometry.maximum_coefficient_bits = 0;
    empty_geometry.maximum_coefficient_bits = 0;
    assert_eq!(valid_geometry, empty_geometry);
    assert_eq!(
        valid_topology, empty_topology,
        "every opcode, range tag, and canonical equality-alias class must match"
    );
    assert_eq!(valid_topology.rows.len(), valid_rows.total_rows);
    assert_eq!(
        valid_topology
            .rows
            .iter()
            .filter(|row| row.opcode == Opcode::Disabled)
            .count(),
        valid_rows.padding_rows + valid_rows.table_padding_rows,
        "the descriptor must retain every disabled padding row"
    );
    let (_, valid_tail) = valid
        .instance_partition_for_test()
        .expect("valid constant tail is derivable");
    let (empty_caller, empty_tail) = without_witnesses
        .instance_partition_for_test()
        .expect("without_witnesses constant tail is derivable");
    assert!(empty_caller.iter().all(|value| *value == F::ZERO));
    assert_eq!(valid_tail, empty_tail);
    valid_topology
}

#[test]
fn without_witnesses_preserves_topology_and_constant_tail_on_both_pasta_fields() {
    let fp = assert_without_witnesses_topology_and_constant_tail::<Fp>();
    let fq = assert_without_witnesses_topology_and_constant_tail::<Fq>();
    assert_eq!(fp, fq, "the canonical full trace must be field-independent");
}

#[test]
fn canonical_runtime_topology_tail_digests_are_nonzero_and_reciprocal() {
    let vector = rfc6979_sample();
    let fp =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature);
    let fq =
        P256PackedAffineEcdsaCircuitV3::<Fq>::new(vector.sec1, vector.digest, vector.signature);
    let (_, fp_topology) = fp
        .trace_and_topology_for_test()
        .expect("Fp canonical topology recording trace");
    let (_, fq_topology) = fq
        .trace_and_topology_for_test()
        .expect("Fq canonical topology recording trace");
    assert_eq!(fp_topology, fq_topology);
    let (_, fp_tail) = fp
        .instance_partition_for_test()
        .expect("Fp canonical constant tail");
    let (_, fq_tail) = fq
        .instance_partition_for_test()
        .expect("Fq canonical constant tail");
    let fp_digest = canonical_runtime_topology_tail_digest(&fp_topology, &fp_tail);
    let fq_digest = canonical_runtime_topology_tail_digest(&fq_topology, &fq_tail);
    assert_ne!(fp_digest, [0_u8; 32]);
    assert_ne!(fq_digest, [0_u8; 32]);
    eprintln!("Fp topology/tail digest: {}", hex::encode(fp_digest));
    eprintln!("Fq topology/tail digest: {}", hex::encode(fq_digest));
}

fn mock_verify<F: BigPrimeField>(circuit: &P256PackedAffineEcdsaCircuitV3<F>) -> bool {
    let Ok(instances) = circuit.instances() else {
        return false;
    };
    let Ok(prover) = MockProver::run(K, circuit, vec![instances]) else {
        return false;
    };
    match prover.verify() {
        Ok(()) => true,
        Err(errors) => {
            eprintln!("P256 V3 mock verification errors: {errors:#?}");
            false
        }
    }
}

fn assert_mock_rejects<F: BigPrimeField>(vector: &TestVector, label: &str) {
    let circuit =
        P256PackedAffineEcdsaCircuitV3::<F>::new(vector.sec1, vector.digest, vector.signature);
    assert!(!mock_verify(&circuit), "{label} must fail verification");
}

#[test]
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
fn real_key_generation_accepts_the_exact_shape_on_both_pasta_curves() {
    let vector = rfc6979_sample();
    let fp =
        P256PackedAffineEcdsaCircuitV3::<Fp>::new(vector.sec1, vector.digest, vector.signature)
            .without_witnesses();
    let fq =
        P256PackedAffineEcdsaCircuitV3::<Fq>::new(vector.sec1, vector.digest, vector.signature)
            .without_witnesses();
    let eq_params = ParamsIPA::<EqAffine>::new(K);
    keygen_vk(&eq_params, &fp).expect("Fp keygen");
    let ep_params = ParamsIPA::<EpAffine>::new(K);
    keygen_vk(&ep_params, &fq).expect("Fq keygen");
}

#[test]
fn real_semantic_attacks_reject_with_signature_negatives_on_both_pasta_fields() {
    let vector = rfc6979_sample();

    let mut bad_prefix = vector.clone();
    bad_prefix.sec1[0] = 3;
    assert_mock_rejects::<Fp>(&bad_prefix, "compressed SEC1 prefix");

    let mut zero_r = vector.clone();
    zero_r.signature[..32].fill(0);
    assert_mock_rejects::<Fp>(&zero_r, "zero-r");

    let mut high_s = vector.clone();
    let low_s = BigUint::from_bytes_be(&high_s.signature[32..]);
    let high = modulus_scalar() - low_s;
    high_s.signature[32..].copy_from_slice(&high.to_bytes_be());
    assert_mock_rejects::<Fp>(&high_s, "high-S");

    let mut off_curve = vector.clone();
    off_curve.sec1[1..].fill(0);
    assert_mock_rejects::<Fp>(&off_curve, "off-curve public key");

    // Reuse the same bounded fixtures on both Pasta fields. The zero-s case
    // reaches the explicit nonzero/inverse constraints; the tampered case is
    // still canonical and low-S, so only the ECDSA equality can reject it.
    let zero_s = zero_s_sample();
    let tampered = canonical_tampered_signature_sample();
    assert_mock_rejects::<Fp>(&zero_s, "Fp zero-s");
    assert_mock_rejects::<Fp>(&tampered, "Fp canonical tampered signature");
    assert_mock_rejects::<Fq>(&zero_s, "Fq zero-s");
    assert_mock_rejects::<Fq>(&tampered, "Fq canonical tampered signature");
}

#[test]
fn source_is_private_and_authority_is_authenticated_by_role_parity_artifacts() {
    let source = include_str!("p256_packed_affine_v3.rs");
    let parent = include_str!("../offline_cash_v1.rs");
    assert!(source.contains("authenticated role/parity artifact boundary"));
    assert!(source.contains("internal ordinary"));
    assert!(source.contains("real k=16 synthesis/KAT, key generation"));
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod p256_packed_affine_v3;")
            .count(),
        1
    );
    assert!(!parent.contains("pub mod p256_packed_affine_v3"));
    assert!(!source.contains(concat!("non-", "authorizing")));
    assert!(!source.contains("VerificationAvailable"));
    assert!(!source.contains("GuardBundle::"));
    assert!(!source.contains("register_backend"));
    assert!(!source.contains("activate_backend"));
    assert_eq!(source.matches("typed_range_table_rows_v3()").count(), 2);
    assert!(
        source.contains("for (offset, (first, second)) in typed_range_table_rows_v3().enumerate()")
    );
    assert!(source.contains("inactive bounded witness was not zeroized"));
    assert!(source.contains("fn uint_is_zero<F: BigPrimeField>"));
    assert!(source.contains("fn uint_equal<F: BigPrimeField>"));
    assert!(source.contains("fn gate_uint_zero<F: BigPrimeField>"));
    assert_eq!(source.matches("uint_is_zero(builder,").count(), 7);
    assert_eq!(source.matches("uint_equal(builder,").count(), 3);
    assert_eq!(source.matches("gate_uint_zero(builder,").count(), 2);
    assert!(source.contains("let s_zero = uint_is_zero(builder, &s);"));
    assert!(source.contains("let s_nonzero = builder.bool_not(&s_zero);"));
    assert!(source.contains("let s_inverse_value = modular_inverse(&s.value, &scalar_modulus);"));
    assert!(source.contains("let r_matches = uint_equal(builder, &x_mod_n, &r);"));
    assert_eq!(source.matches("s_nonzero,").count(), 1);
    assert_eq!(source.matches("r_matches,").count(), 1);
    assert_eq!(
        source
            .matches("let negative_zero = builder.mul(sign.cell, zero.cell);")
            .count(),
        1
    );
    assert_eq!(
        source
            .matches("builder.assert_zero(negative_zero);")
            .count(),
        1
    );
    assert!(source.contains("terminal modular carry was nonzero"));
    assert!(source.contains("mandatory terminal equation c4=0"));
    assert_eq!(
        source
            .matches("for coefficient in 0..2 * LIMBS - 1 {")
            .count(),
        1
    );
    assert_eq!(
        source.matches("if coefficient < carries.len() {").count(),
        1
    );
    assert_eq!(
        source
            .matches("builder.realize_zero_sum(&terms, witness)?;")
            .count(),
        1
    );
    assert!(source.contains("family_coefficient_offset"));
    assert!(source.contains("REVIEWED_CARRY_INTERVALS_I128"));
    assert!(source.contains("Err(Error::Synthesis)"));
    assert!(source.contains("Rotation::cur()"));
    assert!(source.contains("s * value"));
}
