//! Independent integer oracles and adversarial checks for canonical RNS division.
//!
//! These tests exercise the reusable private polynomial relation and its separate
//! public diagnostic wrapper. Reference artifacts expose copied witness cells;
//! they establish neither committed proof soundness nor private-witness custody.

use fastpq_prover::{
    GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1,
    air::{
        IntegerAirField, NoObserver, ReferenceArtifact, ReferenceChallenges, SemanticAir,
        WorkLimits, build_reference, check_reference,
    },
    gadgets::rns_chain_air::{
        RnsChainAirError, RnsChainRequirements, RnsChainWitness, RnsScaleDownAir, RnsScaleDownPlan,
    },
};

const LARGE_PRIMES: [u64; 3] = [
    576_494_471_325_024_257,
    576_507_820_688_146_433,
    576_561_218_140_635_137,
];

const fn limits() -> WorkLimits {
    WorkLimits {
        max_trace_rows: 512,
        max_trace_cells: 100_000_000,
        max_constraints: 50_000_000,
        max_statement_bytes: 1_000_000,
        max_payload_bytes: 2_000_000_000,
        max_work_units: 4_000_000_000,
    }
}

const fn exact_limits(requirements: RnsChainRequirements) -> WorkLimits {
    WorkLimits {
        max_trace_rows: requirements.rows,
        max_trace_cells: requirements.trace_cells,
        max_constraints: requirements.constraints,
        max_statement_bytes: requirements.statement_bytes,
        max_payload_bytes: requirements.payload_bytes,
        max_work_units: requirements.work_units,
    }
}

fn residue(value: u128, modulus: u64) -> u64 {
    u64::try_from(value % u128::from(modulus)).expect("residue is below a u64 modulus")
}

fn mul_mod(left: u64, right: u64, modulus: u64) -> u64 {
    residue(u128::from(left) * u128::from(right), modulus)
}

fn add_mod(left: u64, right: u64, modulus: u64) -> u64 {
    residue(u128::from(left) + u128::from(right), modulus)
}

fn product_mod(factors: &[u64], modulus: u64) -> u64 {
    factors.iter().fold(1, |value, &factor| {
        mul_mod(value, factor % modulus, modulus)
    })
}

#[derive(Clone, Copy)]
enum Boundary {
    Zero,
    Half,
    AboveHalf,
    Last,
}

impl Boundary {
    const fn negative(self) -> bool {
        matches!(self, Self::AboveHalf | Self::Last)
    }

    /// Residue of a known integer in [0,D), without constructing D or Garner digits.
    fn residue(self, dropped: &[u64], modulus: u64) -> u64 {
        let product = product_mod(dropped, modulus);
        let last = add_mod(product, modulus - 1, modulus);
        let half = mul_mod(last, modulus.div_ceil(2), modulus);
        match self {
            Self::Zero => 0,
            Self::Half => half,
            Self::AboveHalf => add_mod(half, 1, modulus),
            Self::Last => last,
        }
    }
}

/// Construct c=t*x+D*z in each limb; expected y=z+t*[x>D/2] follows over integers.
fn boundary_inputs(
    kept: &[u64],
    dropped: &[u64],
    scale: u64,
    boundaries: &[Boundary],
) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
    let input = |modulus| {
        boundaries
            .iter()
            .enumerate()
            .map(|(index, &boundary)| {
                let quotient = u64::try_from(index).expect("short public fixture") + 7;
                add_mod(
                    mul_mod(scale % modulus, boundary.residue(dropped, modulus), modulus),
                    mul_mod(product_mod(dropped, modulus), quotient % modulus, modulus),
                    modulus,
                )
            })
            .collect::<Vec<_>>()
    };
    let kept_input = kept.iter().flat_map(|&modulus| input(modulus)).collect();
    let dropped_input = dropped.iter().flat_map(|&modulus| input(modulus)).collect();
    let output = kept
        .iter()
        .flat_map(|&modulus| {
            boundaries
                .iter()
                .enumerate()
                .map(move |(index, &boundary)| {
                    let quotient = u64::try_from(index).expect("short public fixture") + 7;
                    add_mod(
                        quotient % modulus,
                        if boundary.negative() {
                            scale % modulus
                        } else {
                            0
                        },
                        modulus,
                    )
                })
        })
        .collect();
    (kept_input, dropped_input, output)
}

fn assert_rows(air: &RnsScaleDownAir, witness: &RnsChainWitness, extension: bool) {
    let schema = air.schema();
    assert_eq!(witness.rows(), schema.trace_rows);
    assert_eq!(witness.width(), schema.width);
    assert_eq!(
        schema.numerator_degree_bound,
        2 * (schema.trace_rows - 1) + 1
    );
    let mut numerators = vec![0; schema.constraints];
    for row in 0..schema.trace_rows {
        let values = witness.row(row).expect("declared row");
        air.plan()
            .evaluate_row(values, &mut numerators)
            .expect("row geometry");
        assert!(numerators.iter().all(|&value| value == 0), "row {row}");
        for (ordinal, &column) in air.public_columns().iter().enumerate() {
            assert_eq!(
                values[column],
                air.public_value(ordinal, row).expect("public column")
            );
        }
        if extension {
            let values: Vec<_> = values
                .iter()
                .map(|&value| GoldilocksFp4V1::from_base(value).expect("canonical field cell"))
                .collect();
            let mut extension_numerators = vec![GoldilocksFp4V1::ZERO; schema.constraints];
            air.plan()
                .evaluate_row(&values, &mut extension_numerators)
                .expect("extension geometry");
            assert!(
                extension_numerators
                    .iter()
                    .all(|&value| value == GoldilocksFp4V1::ZERO)
            );
        }
    }
    assert!(witness.row(schema.trace_rows).is_none());
}

fn fixture(
    kept: &[u64],
    dropped: &[u64],
    scale: u64,
    boundaries: &[Boundary],
) -> (RnsScaleDownAir, RnsChainWitness) {
    let (kept_input, dropped_input, output) = boundary_inputs(kept, dropped, scale, boundaries);
    let plan = RnsScaleDownPlan::new(kept, dropped, scale, boundaries.len(), limits())
        .expect("public arithmetic plan");
    let witness = plan
        .build_witness(&kept_input, &dropped_input)
        .expect("native witness");
    let air = plan
        .bind(&kept_input, &dropped_input, &output)
        .expect("independent expected output");
    (air, witness)
}

/// Only this diagnostic helper copies the clearing owner into non-clearing columns.
fn reference_columns(witness: RnsChainWitness) -> Vec<Vec<u64>> {
    let mut columns = vec![vec![0; witness.rows()]; witness.width()];
    for (column, output) in columns.iter_mut().enumerate() {
        for (row, value) in output.iter_mut().enumerate() {
            *value = witness.row(row).expect("declared row")[column];
        }
    }
    drop(witness);
    columns
}

#[test]
fn small_integer_division_matches_every_centered_class_and_reordered_bases() {
    for dropped in [vec![3], vec![3, 5], vec![5, 3], vec![3, 5, 7]] {
        let product: u64 = dropped.iter().product();
        let kept = [11_u64, 13];
        let scale = 2_u64;
        let coefficients = usize::try_from(product).expect("small product");
        let integers: Vec<_> = (0..product)
            .map(|value| i128::from(scale) * i128::from(value) + i128::from(product) * 17)
            .collect();
        let endpoints = |primes: &[u64]| {
            primes
                .iter()
                .flat_map(|&prime| {
                    integers.iter().map(move |&value| {
                        u64::try_from(value.rem_euclid(i128::from(prime)))
                            .expect("canonical residue")
                    })
                })
                .collect::<Vec<_>>()
        };
        let kept_input = endpoints(&kept);
        let dropped_input = endpoints(&dropped);
        let output: Vec<_> = kept
            .iter()
            .flat_map(|&prime| {
                integers.iter().enumerate().map(move |(index, &value)| {
                    let unsigned = i128::try_from(index).expect("small integer class");
                    let centered = if unsigned > i128::from(product / 2) {
                        unsigned - i128::from(product)
                    } else {
                        unsigned
                    };
                    let numerator = value - i128::from(scale) * centered;
                    assert_eq!(numerator % i128::from(product), 0);
                    u64::try_from((numerator / i128::from(product)).rem_euclid(i128::from(prime)))
                        .expect("canonical expected output")
                })
            })
            .collect();
        let plan =
            RnsScaleDownPlan::new(&kept, &dropped, scale, coefficients, limits()).expect("plan");
        let witness = plan
            .build_witness(&kept_input, &dropped_input)
            .expect("witness");
        let air = plan
            .bind(&kept_input, &dropped_input, &output)
            .expect("public binding");
        assert_rows(&air, &witness, false);
    }
}

#[test]
fn large_chain_half_boundaries_and_full_u64_scales_match_known_integer_outputs() {
    let kept = [97, 193];
    let boundaries = [
        Boundary::Zero,
        Boundary::Half,
        Boundary::AboveHalf,
        Boundary::Last,
    ];
    for count in 1..=3 {
        let dropped = &LARGE_PRIMES[..count];
        for scale in [17, GOLDILOCKS_MODULUS_V1 + 2, u64::MAX] {
            let (air, witness) = fixture(&kept, dropped, scale, &boundaries);
            assert_rows(&air, &witness, true);
        }
    }
    assert!(
        LARGE_PRIMES
            .iter()
            .try_fold(1_u128, |value, &prime| value.checked_mul(u128::from(prime)))
            .is_none()
    );
    // A canonical digit 96 in base 97 exceeds kept modulus 3. The composite
    // must reduce this shared digit before its canonical multiplication gate.
    let (air, witness) = fixture(&[3, 5], &[97], 2, &[Boundary::Last]);
    assert_rows(&air, &witness, true);
}

#[test]
fn sixty_four_prime_geometry_evaluates_a_product_beyond_u128() {
    let dropped = [
        3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
        97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181,
        191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281,
        283, 293, 307, 311, 313,
    ];
    assert!(
        dropped
            .iter()
            .try_fold(1_u128, |value, &prime| value.checked_mul(u128::from(prime)))
            .is_none()
    );
    let (air, witness) = fixture(&[331], &dropped, 2, &[Boundary::AboveHalf]);
    assert_eq!(air.plan().requirements().coefficients, 1);
    assert_rows(&air, &witness, false);
}

#[test]
fn each_public_ceiling_is_exact_and_one_under_is_rejected_before_inputs() {
    let requirements = RnsScaleDownPlan::new(&[97], &[3, 5], 2, 3, limits())
        .expect("public plan")
        .requirements();
    let exact = exact_limits(requirements);
    let plan = RnsScaleDownPlan::new(&[97], &[3, 5], 2, 3, exact).expect("exact ceilings");
    assert_eq!(plan.requirements(), requirements);
    let mut under = [exact; 6];
    under[0].max_trace_rows -= 1;
    under[1].max_trace_cells -= 1;
    under[2].max_constraints -= 1;
    under[3].max_statement_bytes -= 1;
    under[4].max_payload_bytes -= 1;
    under[5].max_work_units -= 1;
    for bound in under {
        assert!(matches!(
            RnsScaleDownPlan::new(&[97], &[3, 5], 2, 3, bound),
            Err(RnsChainAirError::Limit { actual, max, .. }) if actual == max + 1
        ));
    }
    assert!(RnsScaleDownPlan::new(&[97], &[3], 2, 0, limits()).is_err());
    assert!(matches!(
        RnsScaleDownPlan::new(&[97], &[3], 2, usize::MAX, limits()),
        Err(RnsChainAirError::Overflow | RnsChainAirError::Limit { .. })
    ));
    for (kept, dropped, scale) in [
        (vec![], vec![3], 2),
        (vec![97], vec![], 2),
        (vec![97], vec![3, 3], 2),
        (vec![97], vec![9], 2),
        (vec![97], vec![97], 2),
        (vec![97], vec![3], 3),
        (vec![97], vec![3], 0),
        (vec![97], vec![3; 65], 2),
    ] {
        assert!(RnsScaleDownPlan::new(&kept, &dropped, scale, 1, limits()).is_err());
    }
}

#[test]
fn canonical_inputs_shape_and_polynomial_cell_mutations_are_enforced() {
    let plan = RnsScaleDownPlan::new(&[5], &[3], 2, 3, limits()).expect("plan");
    assert!(matches!(
        plan.build_witness(&[5, 0, 0], &[0, 0, 0]),
        Err(RnsChainAirError::NonCanonical)
    ));
    assert!(matches!(
        plan.build_witness(&[0, 0, 0], &[3, 0, 0]),
        Err(RnsChainAirError::NonCanonical)
    ));
    assert!(plan.build_witness(&[], &[0, 0, 0]).is_err());
    assert!(plan.build_witness(&[0, 0, 0], &[]).is_err());
    let (air, mut witness) = fixture(
        &[5],
        &[3],
        2,
        &[Boundary::Zero, Boundary::Half, Boundary::Last],
    );
    assert_rows(&air, &witness, true);
    let width = witness.width();
    let mut numerators = vec![0; air.schema().constraints];
    // Fixed public probes cover endpoints plus the entire auxiliary extent;
    // the private integer helper separately tests each exact carry/product family.
    let probes: std::collections::BTreeSet<_> = air
        .public_columns()
        .iter()
        .copied()
        .chain((0..129).map(|index| index * (width - 1) / 128))
        .collect();
    for column in probes {
        let old = witness.row(0).expect("row")[column];
        witness.row_mut(0).expect("mutable row")[column] = old.add(1);
        air.plan()
            .evaluate_row(witness.row(0).expect("row"), &mut numerators)
            .expect("mutant geometry");
        assert!(
            numerators.iter().any(|&value| value != 0),
            "unconstrained column {column}"
        );
        witness.row_mut(0).expect("mutable row")[column] = old;
    }
    assert!(
        witness
            .row(3)
            .expect("padding")
            .iter()
            .all(|&value| value == 0)
    );
    for column in [0, 1, width / 2, width - 1] {
        witness.row_mut(3).expect("padding")[column] = 1;
        air.plan()
            .evaluate_row(witness.row(3).expect("padding"), &mut numerators)
            .expect("padding geometry");
        assert!(
            numerators.iter().any(|&value| value != 0),
            "padding column {column}"
        );
        witness.row_mut(3).expect("padding")[column] = 0;
    }
    assert!(
        air.plan()
            .evaluate_row(&witness.row(0).expect("row")[..width - 1], &mut numerators)
            .is_err()
    );
    assert!(
        air.plan()
            .evaluate_row(witness.row(0).expect("row"), &mut [])
            .is_err()
    );
    assert!(witness.row_mut(4).is_none());
}

#[test]
fn public_reference_binds_endpoints_active_rows_scale_bytes_and_exact_degree() {
    let boundaries = [Boundary::Zero, Boundary::Half, Boundary::Last];
    let (air, witness) = fixture(&[5], &[3], 2, &boundaries);
    let schema = air.schema();
    let challenges = ReferenceChallenges::derive(&[43; 32], schema.constraints);
    let columns = reference_columns(witness);
    let artifact = build_reference(&air, columns, &challenges, limits(), &NoObserver)
        .expect("quadratic shared evaluator and interpolation");
    check_reference(&air, &artifact, &challenges, limits(), &NoObserver)
        .expect("independent reference check");

    let mut zero_row = artifact.trace().to_vec();
    for column in &mut zero_row {
        column[0] = 0;
    }
    let zero_row = ReferenceArtifact::from_parts(
        artifact.identity().to_owned(),
        artifact.statement().to_vec(),
        zero_row,
        artifact.quotient().to_vec(),
    );
    assert!(check_reference(&air, &zero_row, &challenges, limits(), &NoObserver).is_err());

    let mut statement = artifact.statement().to_vec();
    statement[0] ^= 1;
    let changed_statement = ReferenceArtifact::from_parts(
        artifact.identity().to_owned(),
        statement,
        artifact.trace().to_vec(),
        artifact.quotient().to_vec(),
    );
    assert!(check_reference(&air, &changed_statement, &challenges, limits(), &NoObserver).is_err());

    let (kept, dropped, mut output) = boundary_inputs(&[5], &[3], 2, &boundaries);
    output[0] = (output[0] + 1) % 5;
    let altered = RnsScaleDownPlan::new(&[5], &[3], 2, 3, limits())
        .expect("same plan")
        .bind(&kept, &dropped, &output)
        .expect("canonical different public output");
    assert!(check_reference(&altered, &artifact, &challenges, limits(), &NoObserver).is_err());

    // Both scales have the same single Goldilocks embedding. Canonical Norito
    // parameter bytes must distinguish them even with all-zero endpoints.
    let small = RnsScaleDownPlan::new(&[5], &[97], 2, 1, limits())
        .expect("small scale")
        .bind(&[0], &[0], &[0])
        .expect("small public binding");
    let large = RnsScaleDownPlan::new(&[5], &[97], GOLDILOCKS_MODULUS_V1 + 2, 1, limits())
        .expect("full scale")
        .bind(&[0], &[0], &[0])
        .expect("full public binding");
    assert_ne!(small.statement_bytes(), large.statement_bytes());

    let ordered = RnsScaleDownPlan::new(&[5], &[3, 7], 2, 1, limits())
        .expect("ordered bases")
        .bind(&[0], &[0, 0], &[0])
        .expect("ordered statement");
    let reversed = RnsScaleDownPlan::new(&[5], &[7, 3], 2, 1, limits())
        .expect("reversed bases")
        .bind(&[0], &[0, 0], &[0])
        .expect("reversed statement");
    assert_ne!(ordered.statement_bytes(), reversed.statement_bytes());
    assert!(air.public_value(air.public_columns().len(), 0).is_err());
    assert!(air.public_value(0, schema.trace_rows).is_err());
}

#[test]
fn explicit_witness_clear_zeroes_all_cells_without_changing_geometry() {
    let (air, mut witness) = fixture(&[5], &[3], 2, &[Boundary::Half]);
    assert_rows(&air, &witness, false);
    assert!(witness.row(0).expect("row").iter().any(|&value| value != 0));
    let geometry = (witness.rows(), witness.width());
    zeroize::Zeroize::zeroize(&mut witness);
    assert_eq!((witness.rows(), witness.width()), geometry);
    for row in 0..witness.rows() {
        assert!(
            witness
                .row(row)
                .expect("cleared row")
                .iter()
                .all(|&value| value == 0)
        );
    }
    // A cleared/inactive row satisfies the private kernel, but cannot replace
    // the diagnostic wrapper's authenticated active row.
    let mut numerators = vec![0; air.schema().constraints];
    air.plan()
        .evaluate_row(witness.row(0).expect("cleared row"), &mut numerators)
        .expect("cleared geometry");
    assert!(numerators.iter().all(|&value| value == 0));
    assert_eq!(air.public_columns()[0], 0);
    assert_eq!(air.public_value(0, 0).expect("active row"), 1);
}

#[path = "rns_chain_air/bgv_digit_cases.rs"]
mod bgv_digit_cases;
#[path = "rns_chain_air/extension_cases.rs"]
mod extension_cases;
