//! Test-only P-256 arithmetic public fixed basis; production policy is unchanged.
//!
//! Wire as a child of `main_fixed_coset` for access to its closed test owner.
//! All rows originate from the authoritative verifier-only fixed provider.
//! The arithmetic row builder makes every omitted column an affine function
//! of this declared public basis, including native-domain padding. Equality
//! on all native rows therefore fixes the degree-below-native interpolants;
//! the same affine recipes apply to coefficients and every quotient coset.
//! Constant terms enter coefficient index zero only. The exhaustive provider
//! tests bind these identities without a per-proof value-classification pass.

use super::*;
use crate::privacy_engines::zk_x509::{
    p256_aggregate_adapter::{
        P256MainAdapterV1, P256MainRegistrationV1, P256MainVerifierFixedSourceV1,
    },
    p256_air::{P256_BASE_MODULUS_BE_V1, P256_SCALAR_MODULUS_BE_V1},
    private_table::{PrivateTableV1, inspection, zeroize_fields_v1},
};
use rayon::prelude::*;

const WIDTH: usize = 134;
const BASIS_WIDTH: usize = 43;
const NATIVE_LOG: u8 = 19;
const NATIVE_ROWS: usize = 1 << NATIVE_LOG;
const BASIS: [usize; BASIS_WIDTH] = [
    0, 1, 15, 17, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
    39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 83, 90, 91, 92, 122, 123, 126,
];
// The full output matrix remains allocated. One additional 43-column owner
// list and two bounded view arrays are the only new simultaneous descriptors.
const EXTRA_METADATA: usize = core::mem::size_of::<Candidate<'static>>()
    + BASIS_WIDTH * core::mem::size_of::<Vec<F>>()
    + core::mem::size_of::<[&mut [F]; WIDTH]>()
    + core::mem::size_of::<[&[F]; BASIS_WIDTH]>()
    + core::mem::size_of::<[Recipe; WIDTH]>();

#[derive(Clone, Copy, Debug, Default)]
struct Recipe {
    constant: F,
    canonicality: F,
    count: usize,
    terms: [(usize, F); 3],
}

impl Recipe {
    fn add(&mut self, original_column: usize, factor: F) {
        let basis = BASIS
            .iter()
            .position(|&column| column == original_column)
            .unwrap();
        assert!(self.count < self.terms.len());
        self.terms[self.count] = (basis, factor);
        self.count += 1;
    }

    fn apply(
        &self,
        basis: &[&[F]; BASIS_WIDTH],
        canonicality: &[F],
        row: usize,
        coefficients: bool,
    ) -> F {
        let mut value = if coefficients && row != 0 {
            F::ZERO
        } else {
            self.constant
        };
        if self.canonicality != F::ZERO {
            value = value.add(canonicality[row].mul(self.canonicality));
        }
        for &(column, factor) in &self.terms[..self.count] {
            value = value.add(basis[column][row].mul(factor));
        }
        value
    }
}

fn recipes() -> [Recipe; WIDTH] {
    let inverse_limb = F(65535).inv().unwrap();
    let mut result = [Recipe::default(); WIDTH];
    for column in 0..WIDTH {
        let recipe = &mut result[column];
        if BASIS.contains(&column) || column == 84 {
            continue;
        }
        match column {
            2 => {
                recipe.add(17, inverse_limb);
                recipe.add(0, F::ZERO.sub(F::ONE));
                recipe.add(1, F::ZERO.sub(F::ONE));
            }
            3..=18 => {
                let limb = column - 3;
                let offset = 30 - 2 * limb;
                let p = F(u64::from(u16::from_be_bytes(
                    P256_BASE_MODULUS_BE_V1[offset..offset + 2]
                        .try_into()
                        .unwrap(),
                )));
                let n = F(u64::from(u16::from_be_bytes(
                    P256_SCALAR_MODULUS_BE_V1[offset..offset + 2]
                        .try_into()
                        .unwrap(),
                )));
                recipe.add(17, n.mul(inverse_limb));
                recipe.add(15, p.sub(n));
            }
            51..=66 => {
                recipe.add(19 + column - 51, F::ONE);
                recipe.add(35 + column - 51, F::ONE);
            }
            67..=82 => recipe.add(19 + column - 67, F::ONE),
            85 => {
                recipe.add(19, F::ONE);
                recipe.add(35, F::ONE);
            }
            86 => {
                recipe.add(34, F::ONE);
                recipe.add(50, F::ONE);
            }
            87 => recipe.add(19, F::ONE),
            88 => recipe.add(50, F::ONE),
            89 => {
                recipe.constant = F::ONE;
                recipe.add(17, F::ZERO.sub(inverse_limb));
            }
            90..=121 => {
                let slot = (column - 90) / 4;
                match (column - 90) % 4 {
                    0 => recipe.add(90, F::ONE),
                    1 => recipe.add(91, F::ONE),
                    2 => {
                        recipe.add(92, F::ONE);
                        recipe.add(90, F::ZERO.sub(F((slot / 4) as u64)));
                    }
                    3 => recipe.add(90, F((4 - slot % 4) as u64)),
                    _ => unreachable!(),
                }
            }
            124 | 133 => {
                recipe.constant = F::ONE;
                recipe.add(123, F::ZERO.sub(F::ONE));
            }
            125 | 127 | 129 => recipe.canonicality = F::ONE,
            128 | 130 => {
                recipe.add(126, F::ONE);
                recipe.canonicality = F(((column - 126) / 2) as u64);
            }
            131 => recipe.add(122, F::ONE),
            132 => recipe.add(123, F::ONE),
            _ => unreachable!("unclassified public fixed column {column}"),
        }
    }
    result
}

fn reconstruct(full: &mut [Vec<F>], coefficients: bool, recipes: &[Recipe; WIDTH]) {
    assert_eq!(full.len(), WIDTH);
    let rows = full[0].len();
    assert!(full.iter().all(|column| column.len() == rows));
    let mut views: [&mut [F]; WIDTH] = core::array::from_fn(|_| -> &mut [F] { &mut [] });
    for (view, column) in views.iter_mut().zip(full) {
        *view = column;
    }
    let basis: [&[F]; BASIS_WIDTH] = core::array::from_fn(|index| -> &[F] {
        core::mem::take::<&mut [F]>(&mut views[BASIS[index]])
    });
    let canonicality = core::mem::take(&mut views[84]);
    canonicality
        .par_iter_mut()
        .enumerate()
        .for_each(|(row, target)| {
            *target = basis[4..20]
                .iter()
                .fold(F::ZERO, |sum, column| sum.add(column[row]));
        });
    let canonicality: &[F] = canonicality;
    views
        .par_iter_mut()
        .enumerate()
        .for_each(|(column, output)| {
            let recipe = &recipes[column];
            for (row, target) in output.iter_mut().enumerate() {
                *target = recipe.apply(&basis, canonicality, row, coefficients);
            }
        });
}

fn registration(signature: usize) -> P256MainRegistrationV1 {
    P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::Arithmetic, 0).unwrap()
}

fn native_matrix(
    provider: &P256MainVerifierFixedSourceV1,
    signature: usize,
) -> ZeroizingBaseColumnsV1 {
    let mut result = ZeroizingBaseColumnsV1(Vec::new());
    result.0.try_reserve_exact(WIDTH).unwrap();
    assert_eq!(result.0.capacity(), WIDTH);
    for _ in 0..WIDTH {
        let mut column = Vec::new();
        column.try_reserve_exact(NATIVE_ROWS).unwrap();
        assert_eq!(column.capacity(), NATIVE_ROWS);
        column.resize(NATIVE_ROWS, F::ZERO);
        result.0.push(column);
    }
    provider
        .fill_arithmetic_fixed_matrix_v1(registration(signature), &mut result.0)
        .unwrap();
    result
}

fn cache_prefix(columns: &[Vec<F>], count: usize) -> Vec<PrivateTableV1<F>> {
    let mut result = Vec::new();
    result.try_reserve_exact(count).unwrap();
    assert_eq!(result.capacity(), count);
    for original in &columns[..count] {
        let mut column = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
        column.try_reserve_exact(original.len()).unwrap();
        assert_eq!(column.capacity(), original.len());
        column.extend_from_slice(original);
        result.push(column);
    }
    result
}

fn cache_bytes(columns: &[PrivateTableV1<F>]) -> usize {
    core::mem::size_of_val(columns)
        + columns
            .iter()
            .map(|column| column.capacity() * core::mem::size_of::<F>())
            .sum::<usize>()
}

fn prepare_basis(
    mut native: ZeroizingBaseColumnsV1,
    policy: MainBoundedTransformPolicyV1,
) -> (ZeroizingBaseColumnsV1, ZeroizingBaseColumnsV1) {
    // Preserve the original helper's terminal uncertainty priority before any
    // matrix admission, allocation or owner transfer.
    check_completion_v1(goldilocks_transform_completion_uncertain_v1()).unwrap();
    // Same complete matrix shape/canonicality admission as the original path,
    // before moving a column or dispatching. Only this sealed public fixture
    // producer can reach the candidate; no witness input is classified.
    assert_eq!(native.len(), WIDTH);
    assert!(native.iter().all(|column| column.len() == NATIVE_ROWS
        && column.capacity() == NATIVE_ROWS
        && column.iter().all(|value| F::canonical(value.0).is_some())));
    let mut basis = ZeroizingBaseColumnsV1(Vec::new());
    basis.0.try_reserve_exact(BASIS_WIDTH).unwrap();
    assert_eq!(basis.0.capacity(), BASIS_WIDTH);
    for column in BASIS {
        basis.0.push(core::mem::take(&mut native.0[column]));
    }
    super::super::main_initial_fixed_coefficients_native_with_v1(
        &mut basis.0,
        goldilocks_primitive_root_v1(NATIVE_LOG).unwrap(),
        policy,
        &mut super::super::main_native_transform::ExactNativeTransformV1,
        goldilocks_transform_completion_uncertain_v1,
    )
    .unwrap();
    (basis, native)
}

struct Candidate<'a> {
    basis: MainFixedCosetV1<'a>,
    full: ZeroizingBaseColumnsV1,
    recipes: [Recipe; WIDTH],
}

impl<'a> Candidate<'a> {
    fn new(
        basis: ZeroizingBaseColumnsV1,
        full: ZeroizingBaseColumnsV1,
        cache: &'a [PrivateTableV1<F>],
        policy: MainBoundedTransformPolicyV1,
    ) -> Self {
        let mut basis = basis;
        Self {
            basis: MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut basis.0))
                .unwrap()
                .with_transform_policy_v1(policy)
                .unwrap()
                .with_coefficient_prefix_v1(cache)
                .unwrap(),
            full,
            recipes: recipes(),
        }
    }

    fn move_basis_to_full(&mut self) {
        for (index, column) in BASIS.into_iter().enumerate() {
            assert!(self.full.0[column].is_empty());
            core::mem::swap(&mut self.full.0[column], &mut self.basis.columns.0[index]);
        }
    }

    fn move_basis_from_full(&mut self) {
        for (index, column) in BASIS.into_iter().enumerate() {
            assert!(self.basis.columns.0[index].is_empty());
            core::mem::swap(&mut self.full.0[column], &mut self.basis.columns.0[index]);
        }
    }

    fn evaluate(&mut self, stripe: main_quotient_stripes::MainQuotientStripeV1) -> &[Vec<F>] {
        if !self.full.0[BASIS[0]].is_empty() {
            self.move_basis_from_full();
        }
        self.basis.evaluate_v1(stripe).unwrap();
        self.move_basis_to_full();
        reconstruct(&mut self.full.0, false, &self.recipes);
        assert!(self.full.0.iter().all(|column| column.len() == stripe.rows));
        &self.full.0
    }

    fn reconstruct_initial_coefficients(&mut self) -> &[Vec<F>] {
        self.move_basis_to_full();
        reconstruct(&mut self.full.0, true, &self.recipes);
        &self.full.0
    }
}

impl Drop for Candidate<'_> {
    fn drop(&mut self) {
        for column in &mut self.full.0 {
            zeroize_fields_v1(column);
        }
        self.full.0.clear();
        // During admission/refusal/unwind, some or all basis owners may still
        // reside in the transform owner instead of the published full matrix.
        for column in &mut self.basis.columns.0 {
            zeroize_fields_v1(column);
        }
        self.basis.columns.0.clear();
    }
}

#[test]
fn public_arithmetic_basis_has_unique_complete_recipe_and_bounded_owner_charge() {
    assert_eq!(
        BASIS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        BASIS_WIDTH
    );
    let recipes = recipes();
    assert_eq!(
        recipes
            .iter()
            .enumerate()
            .filter(|(column, _)| !BASIS.contains(column))
            .count(),
        91
    );
    assert!(EXTRA_METADATA < NATIVE_ROWS * core::mem::size_of::<F>());
    // Conservative 26-vs-27 cache keeps all new descriptors strictly inside
    // the original cache allowance without relying on an unmeasured remainder.
    let per_column =
        NATIVE_ROWS * core::mem::size_of::<F>() + core::mem::size_of::<PrivateTableV1<F>>();
    assert!(26 * per_column + EXTRA_METADATA < 27 * per_column);
}

#[test]
fn public_arithmetic_basis_owner_erases_every_cell_after_refusal_or_partial_move_unwind() {
    const ROWS: usize = 8;
    for phase in 0..5 {
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut full = ZeroizingBaseColumnsV1(
                    (0..WIDTH)
                        .map(|column| {
                            let mut values = Vec::new();
                            values.try_reserve_exact(ROWS).unwrap();
                            values.extend((0..ROWS).map(|row| F((column * ROWS + row + 1) as u64)));
                            assert_eq!(values.len(), values.capacity());
                            values
                        })
                        .collect(),
                );
                let pointers = full
                    .0
                    .iter()
                    .map(|column| column.as_ptr() as usize)
                    .collect::<std::collections::BTreeSet<_>>();
                let basis = BASIS
                    .iter()
                    .map(|&column| core::mem::take(&mut full.0[column]))
                    .collect();
                let mut candidate = Candidate {
                    basis: MainFixedCosetV1::new_v1(3, basis).unwrap(),
                    full,
                    recipes: recipes(),
                };
                match phase {
                    0 => {}
                    1 => {
                        candidate.reconstruct_initial_coefficients();
                    }
                    2 => {
                        candidate.reconstruct_initial_coefficients();
                        candidate.move_basis_from_full();
                    }
                    3 => {
                        // An unwind at any owner-transfer point must retain
                        // every allocation in exactly one clearing owner.
                        for (index, &column) in BASIS[..BASIS_WIDTH / 2].iter().enumerate() {
                            core::mem::swap(
                                &mut candidate.full.0[column],
                                &mut candidate.basis.columns.0[index],
                            );
                        }
                    }
                    4 => {
                        let mut stripe =
                            main_quotient_stripes::MainQuotientStripeV1::new_v1(3, 4, 0).unwrap();
                        stripe.ordinal = stripe.count;
                        candidate.evaluate(stripe);
                        unreachable!("invalid stripe must be refused before dispatch");
                    }
                    _ => unreachable!(),
                }
                let retained = candidate
                    .full
                    .0
                    .iter()
                    .chain(candidate.basis.columns.0.iter())
                    .filter(|column| !column.is_empty())
                    .map(|column| {
                        assert_eq!(column.len(), column.capacity());
                        column.as_ptr() as usize
                    })
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(retained, pointers);
                panic!("injected public basis owner unwind at phase {phase}");
            })
        });
        assert!(result.is_err());
        assert_eq!(
            erased.iter().map(|entry| entry.cells).sum::<usize>(),
            WIDTH * ROWS,
            "phase={phase}"
        );
        assert!(
            erased.iter().all(|entry| entry.nonzero_after == 0),
            "phase={phase}"
        );
    }
}

#[test]
#[ignore = "complete authoritative native P-256 public fixed rows, both verifier roles"]
fn public_arithmetic_basis_matches_every_authoritative_native_row() {
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let recipes = recipes();
    for signature in [0, 4] {
        for row in 0..NATIVE_ROWS {
            let expected = provider.fixed_row_v1(registration(signature), row).unwrap();
            assert_eq!(expected.len(), WIDTH);
            let basis_values: [F; BASIS_WIDTH] = core::array::from_fn(|i| expected[BASIS[i]]);
            let basis = core::array::from_fn(|i| core::slice::from_ref(&basis_values[i]));
            let canonicality = [basis_values[4..20]
                .iter()
                .fold(F::ZERO, |sum, value| sum.add(*value))];
            assert_eq!(
                canonicality[0], expected[84],
                "signature={signature}, row={row}, col84"
            );
            for column in 0..WIDTH {
                if BASIS.contains(&column) || column == 84 {
                    continue;
                }
                assert_eq!(
                    recipes[column].apply(&basis, &canonicality, 0, false),
                    expected[column],
                    "signature={signature}, row={row}, column={column}"
                );
            }
        }
    }
}

#[test]
#[ignore = "full native19 coefficient and four-coset CPU parity for the43-column public fixed basis"]
fn public_arithmetic_basis_preserves_all_coefficients_cosets_and_erasure() {
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let policy = MainBoundedTransformPolicyV1::cpu_v1();
    for signature in [0, 4] {
        let native = native_matrix(&provider, signature);
        let mut original = ZeroizingBaseColumnsV1(native.0.clone());
        super::super::main_initial_fixed_coefficients_native_with_v1(
            &mut original.0,
            goldilocks_primitive_root_v1(NATIVE_LOG).unwrap(),
            policy,
            &mut super::super::main_native_transform::ExactNativeTransformV1,
            || false,
        )
        .unwrap();
        let (basis, full) = prepare_basis(native, policy);
        let cache = cache_prefix(&basis.0, 26);
        let mut candidate = Candidate::new(basis, full, &cache, policy);
        assert_eq!(candidate.reconstruct_initial_coefficients(), &original.0);
        let original_cache = cache_prefix(&original.0, 27);
        let mut oracle = MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut original.0))
            .unwrap()
            .with_coefficient_prefix_v1(&original_cache)
            .unwrap();
        for ordinal in 0..4 {
            let stripe =
                main_quotient_stripes::MainQuotientStripeV1::new_v1(NATIVE_LOG, 21, ordinal)
                    .unwrap();
            assert_eq!(
                candidate.evaluate(stripe),
                oracle.evaluate_v1(stripe).unwrap()
            );
        }
        let ((), erased) = inspection::observe_v1(|| drop(candidate));
        assert_eq!(
            erased.iter().map(|entry| entry.cells).sum::<usize>(),
            WIDTH * NATIVE_ROWS
        );
        assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        assert!(cache_bytes(&cache) + EXTRA_METADATA < cache_bytes(&original_cache));
    }
}

#[test]
#[ignore = "actual Metal complete public fixed basis A/B including reconstruction, cache and erasure"]
fn public_arithmetic_basis_complete_cost_preserves_all_four_cosets() {
    use std::time::{Duration, Instant};
    const ROUNDS: usize = 4;
    assert!(!cfg!(debug_assertions));
    assert_eq!(rayon::current_num_threads(), 20);
    assert!(rayon::current_thread_index().is_none());
    assert_eq!(
        fastpq_prover::goldilocks_transform::available_goldilocks_transform_backend_v1(),
        Some(Backend::Metal)
    );
    let provider = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let native = native_matrix(&provider, 0);
    let policy = MainBoundedTransformPolicyV1::for_test_v1(NATIVE_ROWS, 8);
    // Build all four independent CPU reference matrices before either arm.
    // These test-only owners (~2 GiB) are excluded from the candidate budget
    // and timing, and never supply candidate basis coefficients. Retaining
    // them avoids interleaving CPU FFTs with a measured four-stripe operation.
    let mut expected_coefficients = ZeroizingBaseColumnsV1(native.0.clone());
    expected_coefficients.0.par_iter_mut().for_each(|column| {
        goldilocks_ifft_coarse_v1(column, goldilocks_primitive_root_v1(NATIVE_LOG).unwrap())
            .unwrap()
    });
    let expected: [ZeroizingBaseColumnsV1; 4] = core::array::from_fn(|ordinal| {
        let stripe =
            main_quotient_stripes::MainQuotientStripeV1::new_v1(NATIVE_LOG, 21, ordinal).unwrap();
        let mut values = ZeroizingBaseColumnsV1(Vec::new());
        values.0.try_reserve_exact(WIDTH).unwrap();
        for coefficients in &expected_coefficients.0 {
            values
                .0
                .push(stripe.evaluate_v1(coefficients).unwrap().into_vec_v1());
        }
        values
    });
    let reference_bytes = expected
        .iter()
        .map(|matrix| {
            matrix.0.capacity() * core::mem::size_of::<Vec<F>>()
                + matrix
                    .0
                    .iter()
                    .map(|column| column.capacity() * core::mem::size_of::<F>())
                    .sum::<usize>()
        })
        .sum::<usize>();
    drop(expected_coefficients);
    // Warm the actual shared inverse/forward kernels, queue and twiddles before
    // either arm receives a sample. Public setup is excluded in both arms.
    let mut warm = ZeroizingBaseColumnsV1(native.0[..8].to_vec());
    super::super::main_initial_fixed_coefficients_native_with_v1(
        &mut warm.0,
        goldilocks_primitive_root_v1(NATIVE_LOG).unwrap(),
        policy,
        &mut super::super::main_native_transform::ExactNativeTransformV1,
        goldilocks_transform_completion_uncertain_v1,
    )
    .unwrap();
    let mut warm_owner = MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut warm.0))
        .unwrap()
        .with_transform_policy_v1(policy)
        .unwrap();
    for ordinal in 0..4 {
        warm_owner
            .evaluate_v1(
                main_quotient_stripes::MainQuotientStripeV1::new_v1(NATIVE_LOG, 21, ordinal)
                    .unwrap(),
            )
            .unwrap();
    }
    drop(warm_owner);
    let mut samples = [[Duration::ZERO; ROUNDS]; 2];
    for round in 0..ROUNDS {
        for reduced in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            let observed =
                super::super::super::super::prover_observation::ObservationV1::begin_v1();
            fastpq_prover::enable_queue_depth_stats(true);
            fastpq_prover::take_column_staging_stats().unwrap();
            let started = Instant::now();
            let mut original = ZeroizingBaseColumnsV1(native.0.clone());
            let mut full = ZeroizingBaseColumnsV1(Vec::new());
            if reduced {
                (original, full) = prepare_basis(original, policy);
            } else {
                super::super::main_initial_fixed_coefficients_native_with_v1(
                    &mut original.0,
                    goldilocks_primitive_root_v1(NATIVE_LOG).unwrap(),
                    policy,
                    &mut super::super::main_native_transform::ExactNativeTransformV1,
                    goldilocks_transform_completion_uncertain_v1,
                )
                .unwrap();
            }
            let cache = cache_prefix(&original.0, if reduced { 26 } else { 27 });
            let charged_cache = cache_bytes(&cache) + if reduced { EXTRA_METADATA } else { 0 };
            let (mut candidate, mut oracle) = if reduced {
                (Some(Candidate::new(original, full, &cache, policy)), None)
            } else {
                (
                    None,
                    Some(
                        MainFixedCosetV1::new_v1(NATIVE_LOG, core::mem::take(&mut original.0))
                            .unwrap()
                            .with_transform_policy_v1(policy)
                            .unwrap()
                            .with_coefficient_prefix_v1(&cache)
                            .unwrap(),
                    ),
                )
            };
            for ordinal in 0..4 {
                let stripe =
                    main_quotient_stripes::MainQuotientStripeV1::new_v1(NATIVE_LOG, 21, ordinal)
                        .unwrap();
                let actual = if let Some(candidate) = &mut candidate {
                    candidate.evaluate(stripe)
                } else {
                    oracle.as_mut().unwrap().evaluate_v1(stripe).unwrap()
                };
                // Both arms include the same full canonicality and exact
                // parity scans in one uninterrupted timed operation. This
                // additional reference-comparison overhead is test-only.
                assert_eq!(actual.len(), WIDTH);
                assert!(
                    actual
                        .iter()
                        .all(|column| column.iter().all(|value| F::canonical(value.0).is_some()))
                );
                for (column, (actual, expected)) in
                    actual.iter().zip(&expected[ordinal].0).enumerate()
                {
                    assert!(actual == expected, "stripe={ordinal}, column={column}");
                }
            }
            drop(candidate);
            drop(oracle);
            drop(cache);
            let complete = started.elapsed();
            let staging = fastpq_prover::take_column_staging_stats().unwrap().fft();
            fastpq_prover::enable_queue_depth_stats(false);
            let receipt = observed.finish_v1().public_text_v1();
            samples[usize::from(reduced)][round] = complete;
            println!(
                "x509_fixed_basis_cost reduced={reduced} round={round} native_log={NATIVE_LOG} stripes=4 width={WIDTH} basis_width={BASIS_WIDTH} cached={} charged_cache_bytes={charged_cache} additional_metadata_bytes={EXTRA_METADATA} test_only_reference_bytes={reference_bytes} complete_seconds={:.9} metal_batches={} wait_ms={:.6} gpu_ms={:.6} pre_gpu_queue_ms={:.6} parity=all_fields_exact; contiguous_wall_time_includes_allocation_copy_fullcanonical_admission_initial_IFFT_cachecopy_fourstripes_reconstruction_all_owner_erasure_and_identical_full_reference_comparison_overhead; excludes_public_fixture_build_and_independent_CPU_reference_setup_allocation_and_erasure; reference_memory_test_only_outside_candidate_budget; candidate_extra_descriptors_fit_inside_one_removed_cache_column; counters={receipt}",
                if reduced { 26 } else { 27 },
                complete.as_secs_f64(),
                staging.batches,
                staging.wait_ms,
                staging.gpu_execution_ms,
                staging.pre_gpu_queue_ms
            );
        }
    }
    for timings in &mut samples {
        timings.sort_unstable();
    }
    let medians =
        samples.map(|timings| (timings[1].as_secs_f64() + timings[2].as_secs_f64()) / 2.0);
    println!(
        "x509_fixed_basis_median original_seconds={:.9} basis_seconds={:.9} speedup={:.6}; contiguous_fourstripes_with_identical_test_only_full_reference_comparison_overhead; component_only_not_a_full_proof_claim",
        medians[0],
        medians[1],
        medians[0] / medians[1]
    );
}
