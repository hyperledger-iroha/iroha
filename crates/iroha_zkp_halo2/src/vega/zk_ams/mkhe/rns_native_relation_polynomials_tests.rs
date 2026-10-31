//! Independent ordinary-convolution, quotient, resource and clearing checks.
use super::*;
use iroha_fhe::modular::{ModularArithmetic, mod_pow_u64};

struct InputsV1 {
    key: Vec<u64>,
    randomizer: Vec<u64>,
    error: Vec<u64>,
    message: Vec<u64>,
    ciphertext: Vec<u64>,
    mask: Vec<u64>,
}
impl InputsV1 {
    fn view(&self) -> RelationPolynomialInputsV1<'_> {
        RelationPolynomialInputsV1 {
            key: &self.key,
            randomizer: &self.randomizer,
            error: &self.error,
            message: &self.message,
            ciphertext: &self.ciphertext,
            mask: &self.mask,
        }
    }
}
fn oracle_plain_modulus(q: u64) -> u64 {
    VEGA_T256_SCALAR_MODULUS_BE_V1
        .iter()
        .fold(0_u64, |value, &byte| {
            ((u128::from(value) * 256 + u128::from(byte)) % u128::from(q)) as u64
        })
}
fn oracle_product(left: &[u64], right: &[u64], q: u64) -> Vec<u64> {
    let mut output = vec![0; left.len() + right.len()];
    for (i, &a) in left.iter().enumerate() {
        for (j, &b) in right.iter().enumerate() {
            output[i + j] = ((u128::from(output[i + j]) + u128::from(a) * u128::from(b))
                % u128::from(q)) as u64;
        }
    }
    output
}
fn inputs_from_product(
    geometry: GeometryV1,
    key: Vec<u64>,
    randomizer: Vec<u64>,
    product: &[u64],
) -> InputsV1 {
    let n = geometry.degree;
    let q = u128::from(geometry.modulus);
    let p = u128::from(oracle_plain_modulus(geometry.modulus));
    let error: Vec<_> = (0..n)
        .map(|i| geometry.modulus - 1 - (i as u64 % 3))
        .collect();
    let message: Vec<_> = (0..n)
        .map(|i| (i as u64 * 7 + 5) % geometry.modulus)
        .collect();
    let mut mask: Vec<_> = (0..n)
        .map(|i| geometry.modulus - 1 - i as u64 % 7)
        .collect();
    mask[n - 1] = 0;
    let ciphertext = (0..n)
        .map(|i| {
            ((u128::from(product[i]) + q - u128::from(product[n + i])
                + p * u128::from(error[i])
                + u128::from(message[i]))
                % q) as u64
        })
        .collect();
    InputsV1 {
        key,
        randomizer,
        error,
        message,
        ciphertext,
        mask,
    }
}
fn fixture(geometry: GeometryV1) -> (InputsV1, Vec<u64>) {
    let key: Vec<_> = (0..geometry.degree)
        .map(|i| (17 * i as u64 + 3) % geometry.modulus)
        .collect();
    let randomizer: Vec<_> = (0..geometry.degree)
        .map(|i| geometry.modulus - 1 - i as u64 % 5)
        .collect();
    let product = oracle_product(&key, &randomizer, geometry.modulus);
    (
        inputs_from_product(geometry, key, randomizer, &product),
        product,
    )
}
fn evaluate(values: &[u64], x: u64, q: u64) -> u64 {
    values.iter().rev().fold(0_u64, |acc, &coefficient| {
        ((u128::from(acc) * u128::from(x) + u128::from(coefficient)) % u128::from(q)) as u64
    })
}
fn assert_output(
    geometry: GeometryV1,
    inputs: &InputsV1,
    ordinary: &[u64],
    actual: &RelationPolynomialsV1,
) {
    let n = geometry.degree;
    let q = geometry.modulus;
    assert_eq!(actual.product.0.len(), 2 * n);
    assert_eq!(actual.quotient.0.len(), n);
    for i in 0..n {
        let expected =
            ((u128::from(ordinary[n + i]) + u128::from(inputs.mask[i])) % u128::from(q)) as u64;
        assert_eq!(actual.quotient.0[i], expected);
        assert_eq!(actual.product.0[i], expected);
        assert_eq!(actual.product.0[n + i], expected);
    }
    assert_eq!(actual.quotient.0[n - 1], 0);
    for x in [0, 1, 2, 7, q - 1] {
        let factor = (u128::from(mod_pow_u64(x, n as u64, q)) + 1) % u128::from(q);
        assert_eq!(
            evaluate(&actual.product.0, x, q),
            ((factor * u128::from(evaluate(&actual.quotient.0, x, q))) % u128::from(q)) as u64
        );
        // Independently evaluate K*R+pE+M-C+(x^N+1)S from the input slices.
        let unmasked = (u128::from(evaluate(&inputs.key, x, q))
            * u128::from(evaluate(&inputs.randomizer, x, q))
            + u128::from(oracle_plain_modulus(q)) * u128::from(evaluate(&inputs.error, x, q))
            + u128::from(evaluate(&inputs.message, x, q))
            + u128::from(q)
            - u128::from(evaluate(&inputs.ciphertext, x, q)))
            % u128::from(q);
        assert_eq!(
            evaluate(&actual.product.0, x, q),
            ((unmasked + factor * u128::from(evaluate(&inputs.mask, x, q))) % u128::from(q)) as u64
        );
    }
}
fn tiny() -> GeometryV1 {
    GeometryV1 {
        degree: 8,
        modulus: 97,
        root: 8,
    }
}

#[test]
fn ordinary_product_and_masked_quotient_match_independent_integer_oracle() {
    let geometry = tiny();
    let (inputs, ordinary) = fixture(geometry);
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let output = construct_with_geometry_v1(&mut budget, geometry, inputs.view()).unwrap();
    assert_output(geometry, &inputs, &ordinary, &output);
}

#[test]
fn all_forty_native_moduli_match_dense_independent_convolution() {
    for limb in 0..40 {
        let geometry = GeometryV1 {
            degree: 16,
            modulus: ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb],
            root: mod_pow_u64(
                ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb],
                (ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / 16) as u64,
                ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb],
            ),
        };
        let (inputs, ordinary) = fixture(geometry);
        let output = construct_with_geometry_v1(
            &mut RnsNativeProofResourceBudgetV1::default(),
            geometry,
            inputs.view(),
        )
        .unwrap();
        assert_output(geometry, &inputs, &ordinary, &output);
    }
}

#[test]
#[ignore = "explicit full-native40 N=131072 arithmetic qualification; component only"]
fn maximum_native40_geometry_matches_independent_sparse_convolution() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    for limb in 0..40 {
        let n = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
        let q = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[limb];
        let geometry = GeometryV1 {
            degree: n,
            modulus: q,
            root: ZK_AMS_MKHE_RNS_NATIVE_NEGACYCLIC_ROOTS_V1[limb],
        };
        let key: Vec<_> = (0..n)
            .map(|i| (17 * i as u64 + limb as u64 + 1) % q)
            .collect();
        let mut randomizer = vec![0; n];
        randomizer[0] = q - 1;
        randomizer[n - 1] = q - 2;
        let mut ordinary = vec![0; 2 * n];
        for (i, &a) in key.iter().enumerate() {
            for (j, b) in [(0, q - 1), (n - 1, q - 2)] {
                ordinary[i + j] = ((u128::from(ordinary[i + j]) + u128::from(a) * u128::from(b))
                    % u128::from(q)) as u64;
            }
        }
        let inputs = inputs_from_product(geometry, key, randomizer, &ordinary);
        let output =
            construct_native_relation_polynomials_v1(&mut budget, limb, 4, inputs.view()).unwrap();
        assert_output(geometry, &inputs, &ordinary, &output);
        assert!(output.reservation.belongs_to_v1(&budget));
        drop(output);
        assert_eq!(budget.live_bytes().unwrap(), 0);
    }
}

#[test]
fn native_wrapper_rejects_non_native_coordinates_and_lengths_before_admission() {
    let (inputs, _) = fixture(tiny());
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    for (limb, repetition) in [(40, 0), (0, 5), (0, 0)] {
        assert!(matches!(
            construct_native_relation_polynomials_v1(&mut budget, limb, repetition, inputs.view()),
            Err(RelationPolynomialErrorV1::Geometry)
        ));
    }
    assert_eq!(budget.consumed().unwrap(), 0);
    assert_eq!(budget.peak_bytes().unwrap(), 0);
}

#[test]
fn each_relation_input_substitution_is_rejected_without_mutating_inputs() {
    for changed in 0..5 {
        let (mut inputs, _) = fixture(tiny());
        let target = match changed {
            0 => &mut inputs.key,
            1 => &mut inputs.randomizer,
            2 => &mut inputs.error,
            3 => &mut inputs.message,
            _ => &mut inputs.ciphertext,
        };
        target[0] = (target[0] + 1) % 97;
        let before: Vec<_> = inputs
            .view()
            .slices_v1()
            .iter()
            .map(|s| s.to_vec())
            .collect();
        let mut budget = RnsNativeProofResourceBudgetV1::default();
        assert!(matches!(
            construct_with_geometry_v1(&mut budget, tiny(), inputs.view()),
            Err(RelationPolynomialErrorV1::Relation)
        ));
        assert_eq!(
            inputs
                .view()
                .slices_v1()
                .iter()
                .map(|s| s.to_vec())
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(budget.live_bytes().unwrap(), 0);
    }
}

#[test]
fn canonical_inputs_top_mask_and_root_are_checked_before_allocating() {
    for changed in 0..7 {
        let (mut inputs, _) = fixture(tiny());
        if changed == 6 {
            inputs.mask[7] = 1;
        } else {
            let target = match changed {
                0 => &mut inputs.key,
                1 => &mut inputs.randomizer,
                2 => &mut inputs.error,
                3 => &mut inputs.message,
                4 => &mut inputs.ciphertext,
                _ => &mut inputs.mask,
            };
            target[0] = 97;
        }
        let mut budget = RnsNativeProofResourceBudgetV1::default();
        assert!(matches!(
            construct_with_geometry_v1(&mut budget, tiny(), inputs.view()),
            Err(RelationPolynomialErrorV1::Source)
        ));
        assert_eq!(budget.live_bytes().unwrap(), 0);
    }
    let (inputs, _) = fixture(tiny());
    let mut wrong = tiny();
    wrong.root = 1;
    assert!(matches!(
        construct_with_geometry_v1(
            &mut RnsNativeProofResourceBudgetV1::default(),
            wrong,
            inputs.view()
        ),
        Err(RelationPolynomialErrorV1::Geometry)
    ));
}

#[test]
fn resource_admission_is_original_cumulative_and_exact_at_named_boundary() {
    let geometry = tiny();
    let (inputs, _) = fixture(geometry);
    let required = admission_v1(geometry).unwrap();
    let total = required.retained + required.scratch;
    let mut refused = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(total - 1);
    assert!(matches!(
        construct_with_geometry_v1(&mut refused, geometry, inputs.view()),
        Err(RelationPolynomialErrorV1::Resource(
            RnsNativeResourceErrorV1::WorkspaceLimit
        ))
    ));
    assert_eq!(refused.consumed().unwrap(), 0);
    assert_eq!(refused.peak_bytes().unwrap(), 0);
    let mut exact = RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(total);
    for iteration in 1..=2 {
        let output = construct_with_geometry_v1(&mut exact, geometry, inputs.view()).unwrap();
        assert!(output.reservation.belongs_to_v1(&exact));
        assert_eq!(exact.consumed().unwrap(), iteration * required.work);
        assert_eq!(exact.live_bytes().unwrap(), required.retained);
        assert_eq!(exact.peak_bytes().unwrap(), total);
        drop(output);
        assert_eq!(exact.live_bytes().unwrap(), 0);
    }
    let mut exhausted = RnsNativeProofResourceBudgetV1::default();
    exhausted
        .charge(
            super::super::rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1 - required.work
                + 1,
        )
        .unwrap();
    assert!(matches!(
        construct_with_geometry_v1(&mut exhausted, geometry, inputs.view()),
        Err(RelationPolynomialErrorV1::Resource(
            RnsNativeResourceErrorV1::WorkLimit
        ))
    ));
    assert_eq!(exhausted.peak_bytes().unwrap(), 0);
}

#[test]
fn field_work_count_matches_instrumented_shared_transform() {
    struct Counted {
        inner: FixedModulus,
        operations: std::cell::Cell<u64>,
    }
    impl ModularArithmetic for Counted {
        fn modulus(&self) -> u64 {
            self.inner.modulus
        }
        fn add(&self, a: u64, b: u64) -> u64 {
            self.operations.set(self.operations.get() + 1);
            self.inner.add_canonical(a, b)
        }
        fn sub(&self, a: u64, b: u64) -> u64 {
            self.operations.set(self.operations.get() + 1);
            self.inner.sub_canonical(a, b)
        }
        fn mul(&self, a: u64, b: u64) -> u64 {
            self.operations.set(self.operations.get() + 1);
            self.inner.multiply(a, b)
        }
    }
    let arithmetic = Counted {
        inner: FixedModulus::derive(97).unwrap(),
        operations: std::cell::Cell::new(0),
    };
    let mut left = vec![1; 16];
    let mut right = vec![2; 16];
    cyclic_product_in_place_with(&mut left, &mut right, &arithmetic, 8, 85, 91).unwrap();
    let length = 16_u64;
    let log = 4_u64;
    assert_eq!(
        arithmetic.operations.get(),
        3 * (2 * length * log + log * (log + 3) / 2) + 2 * length
    );
    let geometry = tiny();
    let setup = 2 * power_products_v1(95) + power_products_v1(16) + power_products_v1(8) + 64;
    assert_eq!(
        admission_v1(geometry).unwrap().work,
        arithmetic.operations.get() + 5 * 8 + setup + 7 * 8 + 2
    );
}

#[test]
fn all_allocated_secret_buffers_clear_on_success_relation_error_and_unwind() {
    for outcome in 0..4 {
        let (mut inputs, _) = fixture(tiny());
        if outcome == 1 {
            inputs.ciphertext[0] = (inputs.ciphertext[0] + 1) % 97;
        }
        CLEAR_AUDIT_V1.with(|audit| *audit.borrow_mut() = Some(Vec::new()));
        FAULT_V1.with(|fault| {
            fault.set(match outcome {
                2 => Some(FaultV1::Allocated),
                3 => Some(FaultV1::Product),
                _ => None,
            })
        });
        let mut budget = RnsNativeProofResourceBudgetV1::default();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = construct_with_geometry_v1(&mut budget, tiny(), inputs.view());
            if outcome == 0 {
                assert!(result.is_ok());
            } else if outcome == 1 {
                assert!(matches!(result, Err(RelationPolynomialErrorV1::Relation)));
            }
            drop(result);
        }));
        let mut events = CLEAR_AUDIT_V1.with(|audit| audit.borrow_mut().take().unwrap());
        FAULT_V1.with(|fault| fault.set(None));
        assert_eq!(result.is_err(), outcome >= 2);
        events.sort();
        assert_eq!(events, vec![(8, true), (16, true), (16, true)]);
        assert_eq!(budget.live_bytes().unwrap(), 0);
    }
}

#[test]
fn allocation_refusal_clears_earlier_buffers_and_releases_original_reservation() {
    for failure in 0..3 {
        let (inputs, _) = fixture(tiny());
        CLEAR_AUDIT_V1.with(|audit| *audit.borrow_mut() = Some(Vec::new()));
        ALLOCATION_FAILURE_V1.with(|remaining| remaining.set(Some(failure)));
        let mut budget = RnsNativeProofResourceBudgetV1::default();
        let result = construct_with_geometry_v1(&mut budget, tiny(), inputs.view());
        let mut events = CLEAR_AUDIT_V1.with(|audit| audit.borrow_mut().take().unwrap());
        ALLOCATION_FAILURE_V1.with(|remaining| remaining.set(None));
        assert!(matches!(result, Err(RelationPolynomialErrorV1::Allocation)));
        events.sort();
        assert_eq!(
            events,
            match failure {
                0 => vec![],
                1 => vec![(16, true)],
                _ => vec![(8, true), (16, true)],
            }
        );
        assert_eq!(budget.live_bytes().unwrap(), 0);
        assert_eq!(
            budget.consumed().unwrap(),
            admission_v1(tiny()).unwrap().work
        );
    }
}
