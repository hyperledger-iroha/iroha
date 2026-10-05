//! MSM against the vendored `best_multiexp`.
//!
//! `best_multiexp` (`halo2curves-axiom` `msm_best`) is the MSM behind every
//! vendored commitment and IPA round. For each case the vendored result must
//! equal, byte for byte after normalisation, `msm_public` and `msm_secret` and
//! (where listed) `FixedBaseTable::msm_public` / `msm_secret`, in every pool.
//!
//! Cases:
//!
//! - sizes 0..=9 and around every power of two up to 2^12, including both
//!   sides of the vendored switch from the complete `msm_parallel` path to the
//!   batch-affine Booth path (about 2,060 points); 2^13 and 2^14 by default,
//!   2^15 and 2^16 as ignored release tests;
//! - scalars: zero, one, minus one, equal, alternating signs, powers of two,
//!   short (1..=64-bit and 128-bit), near the modulus, Booth window boundaries
//!   and random full-width values;
//! - bases: random, repeated, negated pairs, arithmetic progressions,
//!   cancelling triples and identity points (which move the vendored MSM to
//!   its complete path);
//! - memory budgets down to the smallest feasible plan, and fixed-base tables
//!   with automatic and explicit windows.

use halo2_axiom::{
    arithmetic::best_multiexp,
    halo2curves::{
        ff::{Field, PrimeField},
        group::{Curve, Group, GroupEncoding, prime::PrimeCurveAffine},
    },
};
use iroha_pasta::msm::{FixedBaseTable, MemoryBudget, MsmError, msm_public, msm_secret};
use iroha_plonk_oracle::{
    convert::{CurveBridge, Pallas, VendoredCurve, Vesta, native_affines, native_scalars},
    pools::same_on_each_pool,
};
use rand_chacha::ChaCha20Rng;
use rand_core::RngCore;

use crate::data_rng;

/// Budgets every random case is also run with; each must either fit or fail
/// with [`MsmError::Budget`].
const BUDGETS: [usize; 4] = [4 << 20, 512 << 10, 64 << 10, 16 << 10];

/// One MSM case: vendored inputs, optionally with a fixed-base table.
struct Case<'a, B: CurveBridge> {
    label: String,
    scalars: &'a [B::VScalar],
    bases: &'a [B::Vendored],
    tables: &'a [FixedBaseTable<B::Native>],
    budgets: bool,
}

/// Runs one case in every pool and returns the common result encoding.
fn check<B: CurveBridge>(case: &Case<'_, B>) -> [u8; 32] {
    let scalars = native_scalars::<B>(case.scalars);
    let bases = native_affines::<B>(case.bases);
    same_on_each_pool(&case.label, |threads| {
        let label = format!("{} at {threads} threads", case.label);
        let expected = best_multiexp(case.scalars, case.bases)
            .to_affine()
            .to_bytes();
        let budget = MemoryBudget::DEFAULT;
        let public = msm_public::<B::Native>(&scalars, &bases, budget).expect("msm_public");
        assert_eq!(
            public.to_affine().to_bytes(),
            expected,
            "{label}: msm_public"
        );
        let secret = msm_secret::<B::Native>(&scalars, &bases, budget).expect("msm_secret");
        assert_eq!(
            secret.to_affine().to_bytes(),
            expected,
            "{label}: msm_secret"
        );
        for table in case.tables {
            let window = table.window();
            let public = table
                .msm_public(&scalars, budget)
                .expect("fixed-base public");
            assert_eq!(
                public.to_affine().to_bytes(),
                expected,
                "{label}: fixed-base msm_public, window {window}"
            );
            let secret = table
                .msm_secret(&scalars, budget)
                .expect("fixed-base secret");
            assert_eq!(
                secret.to_affine().to_bytes(),
                expected,
                "{label}: fixed-base msm_secret, window {window}"
            );
        }
        if case.budgets {
            for bytes in BUDGETS {
                let budget = MemoryBudget::new(bytes);
                for (name, result) in [
                    ("public", msm_public::<B::Native>(&scalars, &bases, budget)),
                    ("secret", msm_secret::<B::Native>(&scalars, &bases, budget)),
                ] {
                    match result {
                        Ok(point) => assert_eq!(
                            point.to_affine().to_bytes(),
                            expected,
                            "{label}: msm_{name} within {bytes} bytes"
                        ),
                        Err(MsmError::Budget(_)) => {}
                        Err(error) => panic!("{label}: msm_{name} within {bytes} bytes: {error}"),
                    }
                }
            }
        }
        expected
    })
}

/// Distinct random bases of curve `B` (an arithmetic progression from a random
/// start with a random step, so generation stays cheap).
fn random_bases<B: CurveBridge>(n: usize, rng: &mut ChaCha20Rng) -> Vec<B::Vendored> {
    let step = VendoredCurve::<B>::random(&mut *rng);
    let mut current = VendoredCurve::<B>::random(&mut *rng);
    let projective: Vec<VendoredCurve<B>> = (0..n)
        .map(|_| {
            current += step;
            current
        })
        .collect();
    let mut affine = vec![B::Vendored::identity(); n];
    VendoredCurve::<B>::batch_normalize(&projective, &mut affine);
    affine
}

/// `n` scalars of one named kind.
fn scalars_of_kind<B: CurveBridge>(kind: &str, n: usize, rng: &mut ChaCha20Rng) -> Vec<B::VScalar> {
    let shared = B::VScalar::random(&mut *rng);
    (0..n)
        .map(|i| match kind {
            "zero" => B::VScalar::ZERO,
            "one" => B::VScalar::ONE,
            "minus_one" => -B::VScalar::ONE,
            "equal" => shared,
            "alternating" => {
                if i % 2 == 0 {
                    shared
                } else {
                    -shared
                }
            }
            "powers_of_two" => B::VScalar::from(2).pow_vartime([(i % 255) as u64]),
            "u64" => B::VScalar::from(rng.next_u64() >> (i % 64)),
            "u128" => B::VScalar::from_u128(
                (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64()),
            ),
            "near_modulus" => -B::VScalar::from(rng.next_u64() & 0xffff) - B::VScalar::ONE,
            "booth_boundaries" => {
                // 2^(cw) - 1, 2^(cw) and 2^(cw - 1) for the window widths the
                // vendored and native planners choose.
                let shift = [8, 10, 11, 12, 13, 15, 16][i % 7] * (1 + i % 9);
                let power = B::VScalar::from(2).pow_vartime([(shift % 255) as u64]);
                match i % 3 {
                    0 => power - B::VScalar::ONE,
                    1 => power,
                    _ => power * B::VScalar::TWO_INV,
                }
            }
            "byte_patterns" => {
                let mut repr = [0_u8; 32];
                for byte in &mut repr {
                    *byte = [0x00, 0xff, 0x80, 0x7f, 0x01][(rng.next_u32() % 5) as usize];
                }
                repr[31] &= 0x3f;
                Option::from(B::VScalar::from_repr(repr)).unwrap_or(B::VScalar::ONE)
            }
            "mixed" => match i % 9 {
                0 => B::VScalar::ZERO,
                1 => B::VScalar::ONE,
                2 => -B::VScalar::ONE,
                3 => shared,
                4 => -shared,
                5 => B::VScalar::from(rng.next_u64() & 0xffff),
                6 => B::VScalar::from(rng.next_u64()),
                7 => -B::VScalar::from(rng.next_u64()),
                _ => B::VScalar::random(&mut *rng),
            },
            _ => B::VScalar::random(&mut *rng),
        })
        .collect()
}

/// Scalar kinds every structured case runs.
const SCALAR_KINDS: [&str; 13] = [
    "random",
    "zero",
    "one",
    "minus_one",
    "equal",
    "alternating",
    "powers_of_two",
    "u64",
    "u128",
    "near_modulus",
    "booth_boundaries",
    "byte_patterns",
    "mixed",
];

/// Random scalars and bases at every size in `sizes`.
fn random_sizes<B: CurveBridge>(sizes: &[usize]) {
    let mut rng = data_rng(&format!("msm random {}", B::NAME));
    for &n in sizes {
        let scalars = scalars_of_kind::<B>("random", n, &mut rng);
        let bases = random_bases::<B>(n, &mut rng);
        check::<B>(&Case {
            label: format!("{} random n={n}", B::NAME),
            scalars: &scalars,
            bases: &bases,
            tables: &[],
            budgets: n <= 1 << 12,
        });
    }
}

/// Default sizes: tiny, around powers of two, and the vendored path switch.
fn default_sizes() -> Vec<usize> {
    let mut sizes: Vec<usize> = (0..=9).collect();
    for power in 4..=12 {
        let n = 1_usize << power;
        sizes.extend([n - 1, n, n + 1]);
    }
    sizes.extend([
        31,
        33,
        47,
        65,
        1000,
        2050,
        2059,
        2060,
        2061,
        2100,
        3000,
        1 << 13,
        1 << 14,
    ]);
    sizes.sort_unstable();
    sizes.dedup();
    sizes
}

#[test]
fn random_inputs_match_best_multiexp() {
    let sizes = default_sizes();
    random_sizes::<Vesta>(&sizes);
    random_sizes::<Pallas>(&sizes);
}

#[test]
#[ignore = "2^15 and 2^16 points on both curves; run in release"]
fn random_inputs_match_best_multiexp_large() {
    random_sizes::<Vesta>(&[1 << 15, 1 << 16]);
    random_sizes::<Pallas>(&[1 << 15, 1 << 16]);
}

/// Every scalar kind against random bases, with fixed-base tables.
fn scalar_kinds<B: CurveBridge>() {
    let mut rng = data_rng(&format!("msm scalar kinds {}", B::NAME));
    for n in [7, 64, 300, 2100] {
        let bases = random_bases::<B>(n, &mut rng);
        let native = native_affines::<B>(&bases);
        let budget = MemoryBudget::DEFAULT;
        let mut tables = vec![FixedBaseTable::<B::Native>::new(&native, budget).expect("table")];
        for window in [4, 13] {
            tables.push(FixedBaseTable::with_window(&native, window, budget).expect("table"));
        }
        for kind in SCALAR_KINDS {
            let scalars = scalars_of_kind::<B>(kind, n, &mut rng);
            check::<B>(&Case {
                label: format!("{} scalars {kind} n={n}", B::NAME),
                scalars: &scalars,
                bases: &bases,
                tables: &tables,
                budgets: false,
            });
        }
    }
}

#[test]
fn scalar_kinds_match_best_multiexp() {
    scalar_kinds::<Vesta>();
    scalar_kinds::<Pallas>();
}

/// Bases of one named adversarial shape (vendored affine points).
fn bases_of_kind<B: CurveBridge>(kind: &str, n: usize, rng: &mut ChaCha20Rng) -> Vec<B::Vendored> {
    let random = random_bases::<B>(n, rng);
    let p = random
        .first()
        .copied()
        .unwrap_or_else(B::Vendored::generator);
    match kind {
        "repeated" => vec![p; n],
        "negated_pairs" => (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    random[i]
                } else {
                    -random[i - 1]
                }
            })
            .collect(),
        "progression" => {
            // i * G, so bucket sums meet doublings and multiples of each other.
            let g = VendoredCurve::<B>::generator();
            let mut current = VendoredCurve::<B>::identity();
            let projective: Vec<VendoredCurve<B>> = (0..n)
                .map(|_| {
                    current += g;
                    current
                })
                .collect();
            let mut affine = vec![B::Vendored::identity(); n];
            VendoredCurve::<B>::batch_normalize(&projective, &mut affine);
            affine
        }
        "cancelling_triples" => (0..n)
            .map(|i| match i % 3 {
                0 | 1 => random[i],
                _ => (-(random[i - 2].to_curve() + random[i - 1])).to_affine(),
            })
            .collect(),
        "sparse_identities" => (0..n)
            .map(|i| {
                if i % 5 == 3 {
                    B::Vendored::identity()
                } else {
                    random[i]
                }
            })
            .collect(),
        "all_identity" => vec![B::Vendored::identity(); n],
        "single_point" => (0..n)
            .map(|i| {
                if i == n / 2 {
                    p
                } else {
                    B::Vendored::identity()
                }
            })
            .collect(),
        _ => random,
    }
}

/// Base kinds every adversarial case runs.
const BASE_KINDS: [&str; 7] = [
    "repeated",
    "negated_pairs",
    "progression",
    "cancelling_triples",
    "sparse_identities",
    "all_identity",
    "single_point",
];

/// Adversarial bases crossed with the scalar kinds that stress bucket
/// conflicts (equal, alternating and random scalars).
fn adversarial_bases<B: CurveBridge>(sizes: &[usize]) {
    let mut rng = data_rng(&format!("msm adversarial {}", B::NAME));
    for &n in sizes {
        for kind in BASE_KINDS {
            let bases = bases_of_kind::<B>(kind, n, &mut rng);
            let native = native_affines::<B>(&bases);
            let tables = [
                FixedBaseTable::<B::Native>::new(&native, MemoryBudget::DEFAULT).expect("table"),
            ];
            for scalar_kind in ["equal", "one", "alternating", "random", "mixed"] {
                let scalars = scalars_of_kind::<B>(scalar_kind, n, &mut rng);
                let result = check::<B>(&Case {
                    label: format!("{} bases {kind} scalars {scalar_kind} n={n}", B::NAME),
                    scalars: &scalars,
                    bases: &bases,
                    tables: &tables,
                    budgets: false,
                });
                // Equal scalars on (P, -P) pairs cancel exactly.
                let cancels = kind == "all_identity"
                    || (kind == "negated_pairs"
                        && n % 2 == 0
                        && matches!(scalar_kind, "equal" | "one"));
                if cancels {
                    assert_eq!(
                        result, [0; 32],
                        "{kind}/{scalar_kind} n={n} is the identity"
                    );
                }
            }
        }
    }
}

#[test]
fn adversarial_bases_match_best_multiexp() {
    let sizes = [2, 3, 9, 64, 257, 2100];
    adversarial_bases::<Vesta>(&sizes);
    adversarial_bases::<Pallas>(&sizes);
}

#[test]
fn native_rejects_mismatched_lengths_before_any_work() {
    let bases = native_affines::<Vesta>(&[<Vesta as CurveBridge>::Vendored::generator()]);
    let result = msm_public::<iroha_pasta::Eq>(&[], &bases, MemoryBudget::DEFAULT);
    assert!(matches!(result, Err(MsmError::LengthMismatch(_))));
}

#[test]
fn case_generators_are_deterministic_and_shaped() {
    let mut first = data_rng("shapes");
    let mut second = data_rng("shapes");
    assert_eq!(
        random_bases::<Vesta>(5, &mut first),
        random_bases::<Vesta>(5, &mut second)
    );
    let mut rng = data_rng("shapes");
    let repeated = bases_of_kind::<Pallas>("repeated", 4, &mut rng);
    assert!(repeated.windows(2).all(|pair| pair[0] == pair[1]));
    let pairs = bases_of_kind::<Pallas>("negated_pairs", 4, &mut rng);
    assert_eq!(pairs[1], -pairs[0]);
    let triples = bases_of_kind::<Vesta>("cancelling_triples", 3, &mut rng);
    assert!(bool::from(
        (triples[0].to_curve() + triples[1] + triples[2]).is_identity()
    ));
    let alternating = scalars_of_kind::<Vesta>("alternating", 4, &mut rng);
    assert_eq!(alternating[1], -alternating[0]);
    assert!(default_sizes().contains(&2060));
    assert_eq!(SCALAR_KINDS.len(), 13);
}
