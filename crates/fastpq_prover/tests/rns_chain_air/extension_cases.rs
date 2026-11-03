//! Independent centered-integer oracles and mutations for ordered RNS extension.
//!
//! These tests do not use native conversion to compute expected outputs. The
//! public reference copies witness cells and is not a hiding or committed proof.

use fastpq_prover::gadgets::rns_chain_air::{RnsCenteredExtensionAir, RnsCenteredExtensionPlan};

use super::{
    Boundary, GoldilocksFp4V1, IntegerAirField, LARGE_PRIMES, NoObserver, ReferenceArtifact,
    ReferenceChallenges, RnsChainAirError, RnsChainWitness, SemanticAir, add_mod, build_reference,
    check_reference, exact_limits, limits, product_mod, reference_columns,
};

fn assert_extension_rows(
    air: &RnsCenteredExtensionAir,
    witness: &RnsChainWitness,
    extension_field: bool,
) {
    let schema = air.schema();
    assert_eq!(
        (witness.rows(), witness.width()),
        (schema.trace_rows, schema.width)
    );
    assert_eq!(
        schema.numerator_degree_bound,
        2 * (schema.trace_rows - 1) + 1
    );
    let mut residues = vec![0; schema.constraints];
    for row in 0..schema.trace_rows {
        let values = witness.row(row).expect("declared row");
        air.plan()
            .evaluate_row(values, &mut residues)
            .expect("row geometry");
        assert!(residues.iter().all(|&value| value == 0), "row {row}");
        for (ordinal, &column) in air.public_columns().iter().enumerate() {
            assert_eq!(
                values[column],
                air.public_value(ordinal, row).expect("public endpoint")
            );
        }
        if extension_field {
            let values: Vec<_> = values
                .iter()
                .map(|&value| GoldilocksFp4V1::from_base(value).expect("canonical field cell"))
                .collect();
            let mut extension_residues = vec![GoldilocksFp4V1::ZERO; schema.constraints];
            air.plan()
                .evaluate_row(&values, &mut extension_residues)
                .expect("extension geometry");
            assert!(
                extension_residues
                    .iter()
                    .all(|&value| value == GoldilocksFp4V1::ZERO)
            );
        }
    }
    assert!(witness.row(schema.trace_rows).is_none());
}

fn extension_fixture(
    source_primes: &[u64],
    targets: &[u64],
    boundaries: &[Boundary],
) -> (RnsCenteredExtensionAir, RnsChainWitness) {
    let source: Vec<_> = source_primes
        .iter()
        .flat_map(|&prime| {
            boundaries
                .iter()
                .map(move |&boundary| boundary.residue(source_primes, prime))
        })
        .collect();
    let output: Vec<_> = targets
        .iter()
        .flat_map(|&target| {
            boundaries.iter().map(move |&boundary| {
                let unsigned = boundary.residue(source_primes, target);
                if boundary.negative() {
                    add_mod(
                        unsigned,
                        target - product_mod(source_primes, target),
                        target,
                    )
                } else {
                    unsigned
                }
            })
        })
        .collect();
    let plan = RnsCenteredExtensionPlan::new(source_primes, targets, boundaries.len(), limits())
        .expect("public centered extension plan");
    let witness = plan
        .build_witness(&source)
        .expect("native witness generation");
    let air = plan
        .bind(&source, &output)
        .expect("independent centered output binding");
    (air, witness)
}

#[test]
fn every_small_centered_class_preserves_composite_overlapping_and_repeated_targets() {
    let targets = [3_u64, 5, 7, 9, 97, 3];
    for source_primes in [vec![3_u64], vec![3, 5], vec![5, 3], vec![3, 5, 7]] {
        let product: u64 = source_primes.iter().product();
        let coefficients = usize::try_from(product).expect("small complete integer class");
        let source: Vec<_> = source_primes
            .iter()
            .flat_map(|&prime| (0..product).map(move |value| value % prime))
            .collect();
        let output: Vec<_> = targets
            .iter()
            .flat_map(|&target| {
                (0..product).map(move |value| {
                    let centered = if value > product / 2 {
                        i128::from(value) - i128::from(product)
                    } else {
                        i128::from(value)
                    };
                    u64::try_from(centered.rem_euclid(i128::from(target)))
                        .expect("canonical residue")
                })
            })
            .collect();
        let plan = RnsCenteredExtensionPlan::new(&source_primes, &targets, coefficients, limits())
            .expect("all native-admitted target shapes");
        let witness = plan.build_witness(&source).expect("native witness");
        let air = plan
            .bind(&source, &output)
            .expect("independent output binding");
        assert_extension_rows(&air, &witness, false);
    }
}

#[test]
fn large_chain_boundaries_and_cross_modulus_digits_match_known_centered_integers() {
    let boundaries = [
        Boundary::Zero,
        Boundary::Half,
        Boundary::AboveHalf,
        Boundary::Last,
    ];
    for count in 1..=3 {
        let source = &LARGE_PRIMES[..count];
        let targets = [3, 9, 97, source[0], 9];
        let (air, witness) = extension_fixture(source, &targets, &boundaries);
        assert_extension_rows(&air, &witness, true);
    }
    assert!(
        LARGE_PRIMES
            .iter()
            .try_fold(1_u128, |value, &prime| {
                value.checked_mul(u128::from(prime))
            })
            .is_none()
    );
    // Source digit 96 is not canonical modulo 3 or 9. Its explicit reduction
    // must precede multiplication; centered 96 in base 97 is the integer -1.
    let (air, witness) = extension_fixture(&[97], &[3, 9, 97, 3], &[Boundary::Last]);
    assert_extension_rows(&air, &witness, true);
}

#[test]
fn sixty_four_source_primes_and_more_than_sixty_four_targets_have_exact_geometry() {
    let source = [
        3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
        97, 101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181,
        191, 193, 197, 199, 211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281,
        283, 293, 307, 311, 313,
    ];
    assert!(
        source
            .iter()
            .try_fold(1_u128, |value, &prime| {
                value.checked_mul(u128::from(prime))
            })
            .is_none()
    );
    let (air, witness) = extension_fixture(&source, &[331], &[Boundary::AboveHalf]);
    assert_eq!(air.plan().requirements().width, 4_037_701);
    assert_eq!(air.plan().requirements().constraints, 8_517_870);
    assert_extension_rows(&air, &witness, false);
    drop((air, witness));

    let targets = [9_u64; 65];
    let (air, witness) = extension_fixture(&[97], &targets, &[Boundary::Last]);
    let terms = 1 + targets.len();
    assert_eq!(
        air.plan().requirements().width,
        1_869 * terms + 452 + 1_570 * targets.len() + 67
    );
    assert_eq!(
        air.plan().requirements().constraints,
        3_948 * terms + 780 + 3_371 * targets.len() + 67
    );
    assert_extension_rows(&air, &witness, false);
}

#[test]
fn all_six_public_caps_and_the_explicit_relation_domain_are_enforced() {
    let requirements = RnsCenteredExtensionPlan::new(&[97], &[3], 3, limits())
        .expect("public plan")
        .requirements();
    assert_eq!(
        (requirements.width, requirements.constraints),
        (5_827, 12_114)
    );
    let exact = exact_limits(requirements);
    assert_eq!(
        RnsCenteredExtensionPlan::new(&[97], &[3], 3, exact)
            .expect("exact public caps")
            .requirements(),
        requirements
    );
    let mut under = [exact; 6];
    under[0].max_trace_rows -= 1;
    under[1].max_trace_cells -= 1;
    under[2].max_constraints -= 1;
    under[3].max_statement_bytes -= 1;
    under[4].max_payload_bytes -= 1;
    under[5].max_work_units -= 1;
    for cap in under {
        assert!(matches!(RnsCenteredExtensionPlan::new(&[97], &[3], 3, cap),
            Err(RnsChainAirError::Limit { actual, max, .. }) if actual == max + 1));
    }
    for (source, targets, count) in [
        (vec![], vec![3], 1),
        (vec![3], vec![], 1),
        (vec![3], vec![9], 0),
        (vec![3, 3], vec![9], 1),
        (vec![9], vec![3], 1),
        (vec![2], vec![3], 1),
        (vec![3; 65], vec![9], 1),
    ] {
        assert!(RnsCenteredExtensionPlan::new(&source, &targets, count, limits()).is_err());
    }
    for target in [0, 1, 2, 4, (1_u64 << 62) + 1, u64::MAX] {
        assert!(RnsCenteredExtensionPlan::new(&[3], &[target], 1, limits()).is_err());
    }
    assert!(matches!(
        RnsCenteredExtensionPlan::new(&[3], &[9], usize::MAX, limits()),
        Err(RnsChainAirError::Overflow | RnsChainAirError::Limit { .. })
    ));
}

#[test]
fn noncanonical_endpoints_wrong_shapes_and_mutated_shared_cells_are_rejected() {
    let plan = RnsCenteredExtensionPlan::new(&[97], &[3, 9], 3, limits()).expect("plan");
    assert!(matches!(
        plan.build_witness(&[97, 0, 0]),
        Err(RnsChainAirError::NonCanonical)
    ));
    assert!(plan.build_witness(&[0, 0]).is_err());
    assert!(plan.build_witness(&[0; 4]).is_err());
    assert!(matches!(
        plan.bind(&[0; 3], &[3, 0, 0, 0, 0, 0]),
        Err(RnsChainAirError::NonCanonical)
    ));
    let plan = RnsCenteredExtensionPlan::new(&[97], &[3, 9], 3, limits()).expect("plan");
    assert!(plan.bind(&[0; 3], &[0; 5]).is_err());

    let (air, mut witness) = extension_fixture(
        &[97],
        &[3, 9],
        &[Boundary::Zero, Boundary::Half, Boundary::Last],
    );
    assert_extension_rows(&air, &witness, true);
    let width = witness.width();
    let mut residues = vec![0; air.schema().constraints];
    // Cover every endpoint bit plus public probes across the full private
    // extent. The shared integer unit suite separately mutates every gate cell.
    let probes: std::collections::BTreeSet<_> = air
        .public_columns()
        .iter()
        .copied()
        .chain((0..129).map(|index| index * (width - 1) / 128))
        .collect();
    for column in probes {
        let old = witness.row(0).expect("row")[column];
        witness.row_mut(0).expect("row")[column] = old.add(1);
        air.plan()
            .evaluate_row(witness.row(0).expect("row"), &mut residues)
            .expect("mutant geometry");
        assert!(
            residues.iter().any(|&value| value != 0),
            "unconstrained column {column}"
        );
        witness.row_mut(0).expect("row")[column] = old;
    }
    // The half input 48 has distinct residues 0 mod 3 and 3 mod 9. Swap the
    // complete output words, rather than merely changing diagnostic labels.
    let first_output = 1 + 64;
    let second_output = first_output + 64;
    let original = witness.row(1).expect("half row").to_vec();
    for bit in 0..64 {
        witness
            .row_mut(1)
            .expect("half row")
            .swap(first_output + bit, second_output + bit);
    }
    air.plan()
        .evaluate_row(witness.row(1).expect("half row"), &mut residues)
        .expect("swap geometry");
    assert!(residues.iter().any(|&value| value != 0));
    witness
        .row_mut(1)
        .expect("half row")
        .copy_from_slice(&original);
    for column in [0, 1, width / 2, width - 1] {
        witness.row_mut(3).expect("padding")[column] = 1;
        air.plan()
            .evaluate_row(witness.row(3).expect("padding"), &mut residues)
            .expect("padding geometry");
        assert!(residues.iter().any(|&value| value != 0));
        witness.row_mut(3).expect("padding")[column] = 0;
    }
    assert!(
        air.plan()
            .evaluate_row(&witness.row(0).expect("row")[..width - 1], &mut residues)
            .is_err()
    );
    assert!(
        air.plan()
            .evaluate_row(witness.row(0).expect("row"), &mut [])
            .is_err()
    );
    assert!(witness.row_mut(witness.rows()).is_none());
}

#[test]
fn public_reference_binds_activity_ordered_bases_and_the_actual_output_words() {
    let boundaries = [Boundary::Zero, Boundary::Half, Boundary::Last];
    let (air, witness) = extension_fixture(&[3], &[5], &boundaries);
    let challenges = ReferenceChallenges::derive(&[79; 32], air.schema().constraints);
    let artifact = build_reference(
        &air,
        reference_columns(witness),
        &challenges,
        limits(),
        &NoObserver,
    )
    .expect("shared quadratic polynomial and interpolation");
    check_reference(&air, &artifact, &challenges, limits(), &NoObserver).expect("valid reference");
    let mut inactive = artifact.trace().to_vec();
    for column in &mut inactive {
        column[0] = 0;
    }
    let inactive = ReferenceArtifact::from_parts(
        artifact.identity().to_owned(),
        artifact.statement().to_vec(),
        inactive,
        artifact.quotient().to_vec(),
    );
    assert!(check_reference(&air, &inactive, &challenges, limits(), &NoObserver).is_err());
    let altered = RnsCenteredExtensionPlan::new(&[3], &[5], 3, limits())
        .expect("same plan")
        .bind(&[0, 1, 2], &[1, 1, 4])
        .expect("canonical changed output");
    assert!(check_reference(&altered, &artifact, &challenges, limits(), &NoObserver).is_err());
    let mut statement = artifact.statement().to_vec();
    statement[0] ^= 1;
    let altered_statement = ReferenceArtifact::from_parts(
        artifact.identity().to_owned(),
        statement,
        artifact.trace().to_vec(),
        artifact.quotient().to_vec(),
    );
    assert!(check_reference(&air, &altered_statement, &challenges, limits(), &NoObserver).is_err());

    let ordered = RnsCenteredExtensionPlan::new(&[3, 5], &[7, 9], 1, limits())
        .expect("ordered plan")
        .bind(&[0, 0], &[0, 0])
        .expect("ordered statement");
    let source_reversed = RnsCenteredExtensionPlan::new(&[5, 3], &[7, 9], 1, limits())
        .expect("reversed sources")
        .bind(&[0, 0], &[0, 0])
        .expect("source order statement");
    let target_reversed = RnsCenteredExtensionPlan::new(&[3, 5], &[9, 7], 1, limits())
        .expect("reversed targets")
        .bind(&[0, 0], &[0, 0])
        .expect("target order statement");
    assert_ne!(ordered.statement_bytes(), source_reversed.statement_bytes());
    assert_ne!(ordered.statement_bytes(), target_reversed.statement_bytes());
    assert!(air.public_value(air.public_columns().len(), 0).is_err());
    assert!(air.public_value(0, air.schema().trace_rows).is_err());
}

#[test]
fn explicit_extension_witness_clear_preserves_geometry_and_erases_every_cell() {
    let (air, mut witness) = extension_fixture(&[97], &[3, 9, 97], &[Boundary::Last]);
    assert_extension_rows(&air, &witness, false);
    assert!(
        witness
            .row(0)
            .expect("active row")
            .iter()
            .any(|&value| value != 0)
    );
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
    let mut residues = vec![0; air.schema().constraints];
    air.plan()
        .evaluate_row(witness.row(0).expect("cleared row"), &mut residues)
        .expect("cleared geometry");
    assert!(residues.iter().all(|&value| value == 0));
    assert_eq!(air.public_columns()[0], 0);
    assert_eq!(air.public_value(0, 0).expect("required active row"), 1);
}
