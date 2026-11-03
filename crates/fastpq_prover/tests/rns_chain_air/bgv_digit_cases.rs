//! Public BGV digit-layout correspondence for the shared centered-extension AIR.
//!
//! These tests exercise native coefficient conversion and the polynomial
//! relation. They generate no key, authenticate no ciphertext or key material,
//! and establish no complete key-switch or committed-proof claim.

use fastpq_prover::gadgets::rns_chain_air::RnsCenteredExtensionPlan;
use iroha_fhe::{
    bgv::{BgvParameters, BgvProfile},
    rns_chain::RnsBasis,
};

use super::{SemanticAir, limits};

fn unsigned_classes(product: u128, degree: usize) -> Vec<u128> {
    let half = product / 2;
    (0..degree)
        .map(|coefficient| match coefficient % 8 {
            0 => 0,
            1 => 1,
            2 => half - 1,
            3 => half,
            4 => half + 1,
            5 => product - 2,
            6 => product - 1,
            _ => (7 * u128::try_from(coefficient).expect("small fixture index") + 3) % product,
        })
        .collect()
}

fn centered_residue(value: u128, product: u128, modulus: u64) -> u64 {
    // At most two 30-bit source primes: the independent integer fits i128.
    let value = i128::try_from(value).expect("bounded direct integer");
    let product = i128::try_from(product).expect("two-prime direct product");
    let centered = if value > product / 2 {
        value - product
    } else {
        value
    };
    u64::try_from(centered.rem_euclid(i128::from(modulus))).expect("canonical target residue")
}

#[test]
fn actual_bgv_levels_match_digit_ownership_remaining_main_and_special_targets() {
    let parameters =
        BgvParameters::derive(4, 257, 1, (30, 5), (31, 2), 2).expect("public small BGV parameters");
    let profile = BgvProfile::new(parameters).expect("actual BGV admission");
    let parameters = profile.parameters();
    let degree = profile.degree();
    assert_eq!(degree, 16);
    assert_eq!(profile.top_level(), 4);
    assert_eq!(parameters.digit_primes, 2);
    assert_eq!(parameters.special_primes.len(), 2);
    let mut observed = Vec::new();

    for level in 0..=profile.top_level() {
        let active = level + 1;
        let extended_primes: Vec<_> = parameters.main_primes[..active]
            .iter()
            .chain(&parameters.special_primes)
            .copied()
            .collect();
        for first in (0..active).step_by(parameters.digit_primes) {
            let length = parameters.digit_primes.min(active - first);
            let own = first..first + length;
            let source_primes = &parameters.main_primes[own.clone()];
            let target_limbs: Vec<_> = (0..extended_primes.len())
                .filter(|limb| !own.contains(limb))
                .collect();
            let targets: Vec<_> = target_limbs
                .iter()
                .map(|&limb| extended_primes[limb])
                .collect();
            observed.push((level, first, length, target_limbs.clone()));
            assert_eq!(targets.len() + length, active + profile.special_count());

            let product: u128 = source_primes
                .iter()
                .map(|&prime| u128::from(prime))
                .product();
            let classes = unsigned_classes(product, degree);
            let source: Vec<_> = source_primes
                .iter()
                .flat_map(|&prime| {
                    classes.iter().map(move |&value| {
                        u64::try_from(value % u128::from(prime)).expect("source residue")
                    })
                })
                .collect();
            let expected: Vec<_> = targets
                .iter()
                .flat_map(|&prime| {
                    classes
                        .iter()
                        .map(move |&value| centered_residue(value, product, prime))
                })
                .collect();

            // This is the exact native owner called by BGV's hybrid switch.
            // Expected values above instead use direct centered integer arithmetic.
            let basis = RnsBasis::new(source_primes).expect("digit basis");
            let conversions: Vec<_> = targets
                .iter()
                .map(|&prime| basis.conversion_target(prime).expect("target modulus"))
                .collect();
            let mut native = vec![0; targets.len() * degree];
            basis
                .convert_centered(&source, degree, &conversions, &mut native)
                .expect("actual native digit extension");
            assert_eq!(native, expected, "level {level}, digit starting at {first}");

            // Own source limbs copy unchanged. Remaining main and special
            // limbs receive only their designated centered-conversion output.
            let mut extended = vec![u64::MAX; extended_primes.len() * degree];
            extended[own.start * degree..own.end * degree].copy_from_slice(&source);
            for (target, &limb) in target_limbs.iter().enumerate() {
                extended[limb * degree..(limb + 1) * degree]
                    .copy_from_slice(&native[target * degree..(target + 1) * degree]);
            }
            for (limb, &prime) in extended_primes.iter().enumerate() {
                let key_limb = if limb < active {
                    limb
                } else {
                    parameters.main_primes.len() + limb - active
                };
                let key_prime = parameters
                    .main_primes
                    .iter()
                    .chain(&parameters.special_primes)
                    .nth(key_limb)
                    .expect("full switching-key limb");
                assert_eq!(*key_prime, prime);
                for (coefficient, &value) in classes.iter().enumerate() {
                    assert_eq!(
                        extended[limb * degree + coefficient],
                        centered_residue(value, product, prime),
                        "level {level}, digit {first}, limb {limb}, coefficient {coefficient}"
                    );
                }
            }

            let plan = RnsCenteredExtensionPlan::new(source_primes, &targets, degree, limits())
                .expect("bounded shared extension relation");
            let mut witness = plan.build_witness(&source).expect("native-backed witness");
            let air = plan
                .bind(&source, &expected)
                .expect("direct integer endpoints");
            let schema = air.schema();
            assert_eq!(schema.trace_rows, degree);
            let mut numerators = vec![0; schema.constraints];
            for row in 0..degree {
                let values = witness.row(row).expect("coefficient row");
                air.plan()
                    .evaluate_row(values, &mut numerators)
                    .expect("shared polynomial evaluation");
                assert!(numerators.iter().all(|&value| value == 0));
                for (ordinal, &column) in air.public_columns().iter().enumerate() {
                    assert_eq!(values[column], air.public_value(ordinal, row).unwrap());
                }
            }
            // Mutate actual linked input and output endpoint bits, retaining
            // every auxiliary cell. Neither substitution may satisfy the AIR.
            for column in [
                air.public_columns()[1],
                air.public_columns()[air.public_columns().len() - 64],
            ] {
                witness.row_mut(0).unwrap()[column] ^= 1;
                air.plan()
                    .evaluate_row(witness.row(0).unwrap(), &mut numerators)
                    .unwrap();
                assert!(numerators.iter().any(|&value| value != 0));
                witness.row_mut(0).unwrap()[column] ^= 1;
            }
            air.plan()
                .evaluate_row(witness.row(0).unwrap(), &mut numerators)
                .unwrap();
            assert!(numerators.iter().all(|&value| value == 0));
        }
    }
    assert_eq!(
        observed,
        [
            (0, 0, 1, vec![1, 2]),
            (1, 0, 2, vec![2, 3]),
            (2, 0, 2, vec![2, 3, 4]),
            (2, 2, 1, vec![0, 1, 3, 4]),
            (3, 0, 2, vec![2, 3, 4, 5]),
            (3, 2, 2, vec![0, 1, 4, 5]),
            (4, 0, 2, vec![2, 3, 4, 5, 6]),
            (4, 2, 2, vec![0, 1, 4, 5, 6]),
            (4, 4, 1, vec![0, 1, 2, 3, 5, 6]),
        ]
    );
}
