//! Public full-schedule admission, real arithmetic seams and explicit long runs.
//!
//! The ignored tests exercise the actual full layer/full sixteen-layer executor.
//! N128/q26 is an unqualified arithmetic fixture, never a selected class profile.
//! `BigInt` oracle values are synthetic test data, not production custody owners.

use super::*;
use crate::ram_lfe::{
    HiddenRamFheInstruction as Op, HiddenRamFheProgram, RamLfeClassV1, RamLfeReferenceInputV1,
    clearing::cleared_cells, ram_lfe_reference_execute_v1, ram_lfe_reference_normalize_v1,
};
use iroha_fhe::bgv::{
    BgvCandidate, BgvParameters, Ciphertext, LocalBootstrapSetupBudget, SecretKey, fixtures,
};
use num_bigint::{BigInt, Sign};
use std::{cell::Cell, time::Instant};
use zeroize::Zeroizing;

thread_local! {
    static FAULT: Cell<(&'static str, usize, bool)> = const { Cell::new(("", usize::MAX, false)) };
    static MATCHES: Cell<usize> = const { Cell::new(0) };
    static RETIRED_AT_FAULT: Cell<usize> = const { Cell::new(0) };
    static CLEARED_AT_FAULT: Cell<usize> = const { Cell::new(0) };

    static PUBLIC_PREPARATIONS: Cell<usize> = const { Cell::new(0) };
    static ACTUAL: Cell<RamLfeBgvNonlinearOperationsV1> = const { Cell::new(RamLfeBgvNonlinearOperationsV1 {
        bootstrap_calls: 0, routing_calls: 0, galois_applications: 0, plaintext_products: 0,
        ciphertext_additions: 0, plaintext_additions: 0, ciphertext_products: 0,
        modulus_switches: 0,
    }) };
}
fn update(change: impl FnOnce(&mut RamLfeBgvNonlinearOperationsV1)) {
    ACTUAL.with(|counter| {
        let mut value = counter.get();
        change(&mut value);
        counter.set(value);
    });
}
pub(super) fn observe_route<const I: usize, const O: usize>(plan: &SlotLinearPlan<I, O>) {
    // Shared-routing tests separately instrument its real arithmetic calls.
    update(|x| {
        x.routing_calls += 1;
        x.galois_applications += plan.galois_applications();
        x.plaintext_products += plan.plaintext_products();
        x.ciphertext_additions += plan.plaintext_products();
    });
}
pub(super) fn observe_plain_add() {
    update(|x| x.plaintext_additions += 1);
}
pub(super) fn observe_switch() {
    update(|x| x.modulus_switches += 1);
}
pub(super) fn observe_product() {
    update(|x| x.ciphertext_products += 1);
}
pub(super) fn observe_add() {
    update(|x| x.ciphertext_additions += 1);
}
pub(super) fn checkpoint(stage: &'static str) -> Result<(), RamLfeBgvNonlinearErrorV1> {
    let (selected, at, unwind) = FAULT.with(Cell::get);
    if stage == selected {
        let index = MATCHES.with(|counter| {
            let next = counter.get() + 1;
            counter.set(next);
            next
        });
        if index == at {
            RETIRED_AT_FAULT.with(|counter| counter.set(fixtures::cleared_residue_words()));
            CLEARED_AT_FAULT.with(|counter| counter.set(cleared_cells()));
            assert!(!unwind, "bounded ownership unwind control");
            return Err(RamLfeBgvNonlinearErrorV1::Allocation);
        }
    }
    Ok(())
}

pub(in crate::ram_lfe) fn observe_public_preparation() {
    PUBLIC_PREPARATIONS.with(|count| count.set(count.get() + 1));
}

pub(super) fn observe_refresh() {
    update(|x| x.bootstrap_calls += 1);
}
pub(super) fn actual_counts() -> RamLfeBgvNonlinearOperationsV1 {
    ACTUAL.with(Cell::get)
}
pub(super) fn reset_counts() {
    ACTUAL.with(|counter| counter.set(RamLfeBgvNonlinearOperationsV1::default()));
    PUBLIC_PREPARATIONS.with(|counter| counter.set(0));
    FAULT.with(|counter| counter.set(("", usize::MAX, false)));
    MATCHES.with(|counter| counter.set(0));
    RETIRED_AT_FAULT.with(|counter| counter.set(0));
    CLEARED_AT_FAULT.with(|counter| counter.set(0));
}

fn fixture_profile() -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(7, 257, 1, (50, 26), (60, 2), 2).unwrap()).unwrap()
}
fn fixture_schedule() -> RamLfeBgvNonlinearScheduleV1 {
    let levels = [
        (25, 24),
        (24, 23),
        (23, 21),
        (21, 20),
        (20, 18),
        (18, 17),
        (17, 15),
        (15, 14),
        (14, 12),
        (12, 11),
        (11, 9),
        (9, 8),
        (8, 6),
        (6, 5),
        (5, 3),
        (3, 2),
    ];
    RamLfeBgvNonlinearScheduleV1 {
        input_level: 25,
        refresh: None,
        layers: RamLfeBgvNonlinearLayersV1::Bounded(Box::new(levels.map(
            |(routing_level, multiplication_level)| RamLfeBgvNonlinearLayerChoiceV1 {
                routing_level,
                multiplication_level,
                routing_scratch_bytes: 64 << 20,
            },
        ))),
        projection_level: 1,
        output_level: 0,
        projection_scratch_bytes: 64 << 20,
    }
}
fn fixture_budget() -> RamLfeBgvNonlinearBudgetV1 {
    RamLfeBgvNonlinearBudgetV1 {
        maximum_refresh_plans: 0,
        preparation_payload_bytes: 64 << 20,
        preparation_metadata_bytes: 64 << 20,
        canonical_arithmetic_units: 20_000_000_000,
        scalar_word_operations: 8_000_000_000,
        matrix_payload_bytes: RamLfeBgvNonlinearPlanV1::matrix_payload_bytes(
            RamLfeBgvNonlinearShapeV1::Bounded,
        ),
        layout_metadata_bytes: RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(
            RamLfeBgvNonlinearShapeV1::Bounded,
        )
        .unwrap(),
        preparation_scratch_bytes: RamLfeBgvNonlinearShapeV1::Bounded.preparation_scratch_bytes(),
        owned_residue_bytes: 64 << 20,
    }
}
struct Keys {
    secret: SecretKey,
    local: LocalBootstrapKey,
}
fn keys(profile: &BgvProfile, elements: &[u32]) -> Keys {
    let (secret, local) = LocalBootstrapKey::generate(
        profile,
        elements,
        LocalBootstrapSetupBudget {
            retained_residue_bytes: 16 << 20,
            returned_secret_bytes: 1 << 20,
            plaintext_bytes: 4096,
        },
    )
    .unwrap();
    Keys { secret, local }
}
fn encrypt<'key>(
    profile: &BgvProfile,
    keys: &'key Keys,
    values: &[u64],
) -> LocalScalarCiphertext<'key> {
    let encoding = SlotEncoding::new(profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let plaintext = encoding.encode(profile, values).unwrap();
    keys.local
        .encrypt_scalar(profile, &encoding, &plaintext)
        .unwrap()
}

fn integer(words: &[u64]) -> BigInt {
    words
        .iter()
        .rev()
        .fold(BigInt::from(0), |x, &word| (x << 64) + word)
}
fn bound(shape: &CiphertextShape) -> BigInt {
    integer(shape.bound().words_le())
}
fn switch_bound(profile: &BgvProfile, mut value: BigInt, from: usize, to: usize) -> BigInt {
    for level in (to + 1..=from).rev() {
        let q = BigInt::from(profile.parameters().main_primes[level]);
        value = (value + 257_u32 * ((&q - 1_u32) / 2_u32) * (profile.degree() + 1)) / q;
    }
    value
}
fn route_bounds<const O: usize>(
    profile: &BgvProfile,
    route: &SlotLinearPlan<21, O>,
    state: &[BigInt; 20],
    base: &BigInt,
) -> [BigInt; O] {
    fn fold(
        leaves: &[Option<BigInt>],
        bit: usize,
        mask: usize,
        depth: usize,
        k: &BigInt,
    ) -> Option<BigInt> {
        if bit == depth {
            return leaves[mask].clone();
        }
        let left = fold(leaves, bit + 1, mask, depth, k);
        let right = fold(leaves, bit + 1, mask | (1 << bit), depth, k);
        match (left, right) {
            (None, None) => None,
            (Some(left), None) => Some(left),
            (left, Some(right)) => Some(left.unwrap_or_else(|| BigInt::from(0)) + right + k),
        }
    }
    // Independently enumerate every public support edge and classify its odd
    // action. Do not use the production activity mask or its generator index.
    let mut active = vec![false; O * 21 * 128];
    for row in 0..O * 128 {
        let denominator = 2 * (row % 128) + 1;
        let inverse = (1..256)
            .step_by(2)
            .find(|x| x * denominator % 256 == 1)
            .unwrap();
        for segment in route.layout().row_segments(row).unwrap() {
            for at in 0..segment.count() {
                let column = segment.start() + at * segment.stride();
                if column == 21 * 128 {
                    continue;
                }
                let odd = (2 * (column % 128) + 1) * inverse % 256;
                active[((row / 128 * 21 + column / 128) * 128) + (odd - 1) / 2] = true;
            }
        }
    }
    let babies = route.baby_count();
    let baby_bits = babies.ilog2() as usize;
    let giant_bits = 7 - baby_bits;
    let generators = [5_usize, 25, 113, 225, 193, 129, 255];
    let element = |bits: &[usize], mask: usize| {
        bits.iter().enumerate().fold(1, |acc, (bit, g)| {
            if mask >> bit & 1 == 0 {
                acc
            } else {
                acc * g % 256
            }
        })
    };
    let k = integer(
        profile
            .key_switch_noise(route.output_shapes()[0].level())
            .unwrap()
            .words_le(),
    );
    std::array::from_fn(|output| {
        let leaves = (0..128 / babies)
            .map(|giant| {
                let mut sum = None;
                for input in 0..21 {
                    for baby in 0..babies {
                        let odd = element(&generators[..baby_bits], baby)
                            * element(&generators[baby_bits..], giant)
                            % 256;
                        if active[(output * 21 + input) * 128 + (odd - 1) / 2] {
                            let source = if input == 20 { base } else { &state[input] };
                            let product = 16_384_u32 * (source + baby.count_ones() * &k);
                            sum = Some(sum.unwrap_or_else(|| BigInt::from(0)) + product);
                        }
                    }
                }
                sum
            })
            .collect::<Vec<_>>();
        128_u32 + fold(&leaves, 0, 0, giant_bits, &k).unwrap_or_else(|| BigInt::from(0))
    })
}

#[test]
fn complete_public_plan_matches_independent_integer_transfers_and_counts() {
    let profile = fixture_profile();
    let schedule = fixture_schedule();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &schedule,
        fixture_budget(),
    )
    .unwrap();
    let mut state: [BigInt; 20] = std::array::from_fn(|_| BigInt::from(128));
    let mut base = integer(profile.fresh_noise().words_le()) + 128_u32;
    let mut entry = profile.top_level();
    assert_eq!(bound(&plan.bounded().augmented), base);
    for stage in &plan.bounded().layers {
        for value in &mut state {
            *value = switch_bound(&profile, value.clone(), entry, stage.choice.routing_level);
        }
        base = switch_bound(&profile, base, entry, stage.choice.routing_level);
        let routed = route_bounds(&profile, &stage.routing, &state, &base);
        for (shape, exact) in stage.routing.output_shapes().iter().zip(&routed) {
            assert_eq!(&bound(shape), exact);
        }
        assert_eq!(stage.routing.baby_count(), 16);
        let operands = routed.map(|x| {
            switch_bound(
                &profile,
                x,
                stage.choice.routing_level,
                stage.choice.multiplication_level,
            )
        });
        let key_noise = integer(
            profile
                .key_switch_noise(stage.choice.multiplication_level)
                .unwrap()
                .words_le(),
        );
        for bank in 0..20 {
            state[bank] = profile.degree() * &operands[bank] * &operands[20 + bank]
                + &key_noise
                + &operands[40 + bank];
            assert_eq!(bound(&stage.products.state_outputs[bank]), state[bank]);
            assert_eq!(
                stage.products.state_outputs[bank].level(),
                stage.products.base_output.level()
            );
        }
        base = switch_bound(
            &profile,
            base,
            stage.choice.routing_level,
            stage.choice.multiplication_level,
        );
        assert_eq!(bound(&stage.products.base_output), base);
        entry = stage.choice.multiplication_level;
    }
    for value in &mut state {
        *value = switch_bound(&profile, value.clone(), entry, schedule.projection_level);
    }
    base = switch_bound(&profile, base, entry, schedule.projection_level);
    let [routed] = route_bounds(&profile, &plan.bounded().projection, &state, &base);
    assert_eq!(bound(&plan.bounded().projection.output_shapes()[0]), routed);
    let expected = switch_bound(
        &profile,
        routed,
        schedule.projection_level,
        schedule.output_level,
    );
    assert_eq!(bound(plan.output_shape()), expected);
    assert_eq!(plan.output_shape().level(), 0);
    assert_eq!(plan.bounded().projection.baby_count(), 2);
    assert_eq!(
        *plan.operations(),
        RamLfeBgvNonlinearOperationsV1 {
            bootstrap_calls: 0,
            routing_calls: 17,
            galois_applications: 11_844,
            plaintext_products: 1_100_920,
            ciphertext_additions: 1_101_240,
            plaintext_additions: 1,
            ciphertext_products: 320,
            modulus_switches: 1_425,
        }
    );
    assert_eq!(
        plan.owned_residue_bytes(),
        (21 + 60 + 342) * 2 * 128 * 26 * 8
    );
    assert_eq!(plan.galois_elements().len(), 7);
    assert_eq!(
        plan.bounded().layers[0].routing.preparation_scratch_bytes(),
        10_756
    );
    assert_eq!(
        plan.bounded().layers[0].routing.coefficient_bytes(),
        1_177_080
    );
    assert_eq!(plan.bounded().projection.coefficient_bytes(), 90_112);
    assert_eq!(
        RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(RamLfeBgvNonlinearShapeV1::Bounded)
            .unwrap(),
        plan.bounded().layers[0].routing.layout_metadata_bytes()
            + plan.bounded().projection.layout_metadata_bytes()
    );
    for stage in &plan.bounded().layers {
        assert_eq!(
            stage.routing.layout(),
            plan.bounded().layers[0].routing.layout()
        );
    }
    assert_eq!(
        RamLfeBgvNonlinearPlanV1::matrix_payload_bytes(RamLfeBgvNonlinearShapeV1::Bounded),
        18_923_392
    );
    let tight = RamLfeBgvNonlinearBudgetV1 {
        owned_residue_bytes: plan.owned_residue_bytes(),
        ..fixture_budget()
    };
    let exact_plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &schedule,
        tight,
    )
    .unwrap();
    // The plan now owns non-Clone bootstrap programs. Compare every ordinary
    // field instead of requiring a public equality/duplication API for them.
    assert_eq!(exact_plan.bounded().identity, plan.bounded().identity);
    assert_eq!(exact_plan.bounded().source, plan.bounded().source);
    assert_eq!(exact_plan.bounded().augmented, plan.bounded().augmented);
    assert_eq!(
        exact_plan.bounded().layers.len(),
        plan.bounded().layers.len()
    );
    for (left, right) in exact_plan
        .bounded()
        .layers
        .iter()
        .zip(&plan.bounded().layers)
    {
        assert_eq!(left.stage, right.stage);
        assert_eq!(left.choice, right.choice);
        assert_eq!(left.routing, right.routing);
        assert_eq!(left.products, right.products);
        assert_eq!(left.after_refresh, right.after_refresh);
        assert_eq!(left.repetitions, right.repetitions);
        assert!(
            matches!((&left.coefficients,&right.coefficients),(Coefficients::Private(a),Coefficients::Private(b)) if a==b)
        );
    }
    assert_eq!(exact_plan.bounded().projection, plan.bounded().projection);
    assert_eq!(
        exact_plan.bounded().projection_level,
        plan.bounded().projection_level
    );
    assert_eq!(
        exact_plan.bounded().output_level,
        plan.bounded().output_level
    );
    assert_eq!(exact_plan.bounded().output, plan.bounded().output);
    assert_eq!(exact_plan.bounded().galois, plan.bounded().galois);
    assert!(exact_plan.bounded().refresh.is_none() && plan.bounded().refresh.is_none());
    assert_eq!(exact_plan.bounded().preparation, plan.bounded().preparation);
    assert_eq!(exact_plan.bounded().work, plan.bounded().work);
    assert_eq!(exact_plan.bounded().operations, plan.bounded().operations);
    assert_eq!(
        exact_plan.bounded().owned_residue_bytes,
        plan.bounded().owned_residue_bytes
    );
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &schedule,
            RamLfeBgvNonlinearBudgetV1 {
                owned_residue_bytes: tight.owned_residue_bytes - 1,
                ..tight
            }
        ),
        Err(RamLfeBgvNonlinearErrorV1::ResidueBudget { .. })
    ));
}

#[test]
fn full_size_public_plan_retains_heterogeneous_state_bounds_without_widening() {
    // Unqualified arithmetic-only q26 proposal. No ciphertext allocation/native
    // execution or selected-profile change. Unlike N128, this vector retains
    // significant bank-to-bank differences after public modulus switching.
    let profile =
        BgvProfile::new(BgvParameters::derive(16, 257, 1, (50, 26), (60, 2), 2).unwrap()).unwrap();
    let mut schedule = fixture_schedule();
    for choice in schedule.layers.bounded_mut() {
        choice.routing_scratch_bytes = (21 + 7 + 3)
            * usize::try_from(
                CostModel::new(&profile)
                    .ciphertext_residue_bytes(choice.routing_level, 2)
                    .unwrap(),
            )
            .unwrap();
    }
    schedule.projection_level = 2;
    schedule.output_level = 2;
    schedule.projection_scratch_bytes = (21 + 7 + 3)
        * usize::try_from(
            CostModel::new(&profile)
                .ciphertext_residue_bytes(2, 2)
                .unwrap(),
        )
        .unwrap();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &schedule,
        RamLfeBgvNonlinearBudgetV1 {
            owned_residue_bytes: 8_usize << 30,
            canonical_arithmetic_units: 1_u64 << 60,
            scalar_word_operations: 1_u64 << 60,
            ..fixture_budget()
        },
    )
    .unwrap();
    assert_eq!(
        bound(plan.output_shape()).to_string(),
        "97694392958005410072338193844114391"
    );
    let mut state: [BigInt; 20] = std::array::from_fn(|_| BigInt::from(128));
    let mut base = integer(profile.fresh_noise().words_le()) + 128_u32;
    let mut entry = profile.top_level();
    for (index, stage) in plan.bounded().layers.iter().enumerate() {
        assert_eq!(stage.routing.baby_count(), 1);
        assert_eq!(stage.routing.galois_applications(), 7616);
        assert_eq!(stage.routing.plaintext_products(), 68643);
        for value in &mut state {
            *value = switch_bound(&profile, value.clone(), entry, stage.choice.routing_level);
        }
        base = switch_bound(&profile, base, entry, stage.choice.routing_level);
        let expected = route_bounds(&profile, &stage.routing, &state, &base);
        for (actual, expected) in stage.routing.output_shapes().iter().zip(&expected) {
            assert_eq!(&bound(actual), expected);
        }
        let operands = expected.map(|x| {
            switch_bound(
                &profile,
                x,
                stage.choice.routing_level,
                stage.choice.multiplication_level,
            )
        });
        let k = integer(
            profile
                .key_switch_noise(stage.choice.multiplication_level)
                .unwrap()
                .words_le(),
        );
        for bank in 0..20 {
            state[bank] = profile.degree() * &operands[bank] * &operands[20 + bank]
                + &k
                + &operands[40 + bank];
            assert_eq!(bound(&stage.products.state_outputs[bank]), state[bank]);
        }
        if index % 2 == 1 {
            assert_eq!(
                stage
                    .products
                    .state_outputs
                    .iter()
                    .map(CiphertextShape::bound_bits)
                    .min(),
                Some(84)
            );
            assert_eq!(
                stage
                    .products
                    .state_outputs
                    .iter()
                    .map(CiphertextShape::bound_bits)
                    .max(),
                Some(93)
            );
        }
        base = switch_bound(
            &profile,
            base,
            stage.choice.routing_level,
            stage.choice.multiplication_level,
        );
        entry = stage.choice.multiplication_level;
    }
}

#[test]
fn public_levels_budgets_and_known_current_profile_refuse_honestly() {
    let profile = fixture_profile();
    let mut schedule = fixture_schedule();
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &schedule,
            RamLfeBgvNonlinearBudgetV1 {
                matrix_payload_bytes: 0,
                ..fixture_budget()
            }
        ),
        Err(RamLfeBgvNonlinearErrorV1::MatrixBudget { .. })
    ));
    for small in [
        RamLfeBgvNonlinearBudgetV1 {
            layout_metadata_bytes: RamLfeBgvNonlinearPlanV1::layout_metadata_bytes(
                RamLfeBgvNonlinearShapeV1::Bounded,
            )
            .unwrap()
                - 1,
            ..fixture_budget()
        },
        RamLfeBgvNonlinearBudgetV1 {
            preparation_scratch_bytes: RamLfeBgvNonlinearShapeV1::Bounded
                .preparation_scratch_bytes()
                - 1,
            ..fixture_budget()
        },
    ] {
        assert!(matches!(
            RamLfeBgvNonlinearPlanV1::new(
                &profile,
                profile.plaintext_precision(1).unwrap(),
                &schedule,
                small
            ),
            Err(RamLfeBgvNonlinearErrorV1::Routing(
                SlotLinearError::LayoutBudget { .. } | SlotLinearError::PreparationBudget { .. }
            ))
        ));
    }
    schedule.layers.bounded_mut()[0].multiplication_level = 26;
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &schedule,
            fixture_budget()
        ),
        Err(RamLfeBgvNonlinearErrorV1::Bgv(
            BgvError::LevelMismatch { .. }
        ))
    ));
    schedule = fixture_schedule();
    schedule.layers.bounded_mut()[0].routing_scratch_bytes = 1;
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &schedule,
            fixture_budget()
        ),
        Err(RamLfeBgvNonlinearErrorV1::Routing(
            SlotLinearError::ScratchBudget { .. }
        ))
    ));
    schedule = fixture_schedule();
    schedule.output_level = 2;
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(1).unwrap(),
            &schedule,
            fixture_budget()
        ),
        Err(RamLfeBgvNonlinearErrorV1::Bgv(
            BgvError::LevelMismatch { .. }
        ))
    ));
    let current = BgvProfile::candidate(BgvCandidate::BoundedF257).unwrap();
    assert_eq!((current.degree(), current.top_level()), (65536, 21));
    let mut current_schedule = fixture_schedule();
    current_schedule.input_level = current.top_level();
    for stage in current_schedule.layers.bounded_mut() {
        stage.routing_level = stage.routing_level.min(current.top_level());
        stage.multiplication_level = stage.multiplication_level.min(current.top_level());
        stage.routing_scratch_bytes = 8_usize << 30;
    }
    current_schedule.projection_scratch_bytes = 8_usize << 30;
    let error = RamLfeBgvNonlinearPlanV1::new(
        &current,
        current.plaintext_precision(1).unwrap(),
        &current_schedule,
        RamLfeBgvNonlinearBudgetV1 {
            owned_residue_bytes: 16_usize << 30,
            ..fixture_budget()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        RamLfeBgvNonlinearErrorV1::Bgv(BgvError::NoiseBudgetExceeded { .. })
            | RamLfeBgvNonlinearErrorV1::Routing(SlotLinearError::Bgv(
                BgvError::NoiseBudgetExceeded { .. }
            ))
    ));
    // This supplied complete schedule fails; the separate feasibility search
    // is broader evidence, not a blanket impossibility theorem about BGV.
}

#[test]
fn real_input_packing_uses_current_lanes_and_all_key_preflight() {
    let profile = fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &fixture_schedule(),
        fixture_budget(),
    )
    .unwrap();
    let keys = keys(&profile, plan.galois_elements());
    let values = std::array::from_fn::<_, 128, _>(|i| ((i * 19 + 256) % 257) as u64);
    let cipher = encrypt(&profile, &keys, &values);
    let missing = self::keys(&profile, &[]);
    let missing_source = encrypt(&profile, &missing, &values);
    let doubled = keys
        .local
        .add_scalar(
            &profile,
            encrypt(&profile, &keys, &values),
            encrypt(&profile, &keys, &values),
        )
        .unwrap();
    let other =
        BgvProfile::new(BgvParameters::derive(7, 257, 1, (40, 4), (41, 2), 2).unwrap()).unwrap();
    let other_keys = self::keys(&other, &[]);
    let foreign_same_profile = self::keys(&profile, plan.galois_elements());
    let before = cleared_cells();
    reset_counts();
    assert_eq!(
        plan.bounded()
            .check_inputs(&profile, &missing_source, &missing.local)
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::Bootstrap(BootstrapError::Bgv(BgvError::WrongSwitchingKey))
    );
    assert_eq!(
        plan.bounded()
            .check_inputs(&profile, &doubled, &keys.local)
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::InputShape
    );
    // The closed origin check is stronger than detached evaluation-key profile
    // equality: both foreign masters and same-profile foreign owners refuse.
    for foreign in [&other_keys.local, &foreign_same_profile.local] {
        assert_eq!(
            plan.bounded()
                .check_inputs(&profile, &cipher, foreign)
                .unwrap_err(),
            RamLfeBgvNonlinearErrorV1::Bootstrap(BootstrapError::Origin)
        );
    }
    assert_eq!(
        plan.bounded()
            .check_inputs(&other, &cipher, &keys.local)
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::Bgv(BgvError::ProfileMismatch)
    );
    assert_eq!(cleared_cells(), before);
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    let input = RamLfeReferenceInputV1::from_bytes(&std::array::from_fn::<_, 63, _>(|i| {
        u8::try_from((i * 19 + 255) % 256).unwrap()
    }))
    .unwrap();
    let values: [u64; 128] = std::array::from_fn(|i| {
        if i < 64 {
            u64::from(input.slots()[i])
        } else {
            0
        }
    });
    let mut cipher = fixture_cipher(&profile, &keys, &input);
    let input_bytes = cipher.ciphertext().to_bytes(&profile).unwrap();
    let mut first = None;
    for seed in [7_u16, 211, 7] {
        let lanes = std::array::from_fn::<_, 32, _>(|i| (seed + u16::try_from(i).unwrap()) % 257);
        let memory = RamLfeInitialMemoryV1::from_lanes(&lanes).unwrap();
        cipher
            .preflight(
                &profile,
                &keys.local,
                &plan.bounded().source,
                plan.galois_elements(),
            )
            .unwrap();
        let (original, banks) =
            initialize_state(&profile, plan.bounded(), &memory, cipher, &keys.local).unwrap();
        cipher = original;
        assert_eq!(cipher.ciphertext().to_bytes(&profile).unwrap(), input_bytes);
        assert_eq!(banks.len(), 21);
        for bank in &banks[..20] {
            assert_eq!(
                bank.shape(),
                profile
                    .plan_trivial(profile.plaintext_precision(1).unwrap(), 25)
                    .unwrap()
            );
        }
        let opened = keys
            .secret
            .decrypt(&profile, banks[20].ciphertext())
            .unwrap();
        let actual = encoding.decode(&opened).unwrap();
        assert_eq!(&actual[..64], &values[..64]);
        assert_eq!(&actual[64..96], &lanes.map(u64::from));
        assert_eq!(actual[96], 1);
        assert_eq!(&actual[97..], &[0; 31]);
        let encoded = banks[20].ciphertext().to_bytes(&profile).unwrap();
        if seed == 7 {
            if let Some(old) = &first {
                assert_eq!(&encoded, old);
            } else {
                first = Some(encoded);
            }
        }
    }
}

#[test]
fn real_product_seam_moves_all_operands_and_switches_the_base() {
    for master_exponent in 1..=3 {
        check_product_seam(master_exponent);
    }
}

fn check_product_seam(master_exponent: u32) {
    let profile = BgvProfile::new(
        BgvParameters::derive(7, 257, master_exponent, (40, 4), (41, 2), 2).unwrap(),
    )
    .unwrap();
    let precision = profile.plaintext_precision(1).unwrap();
    assert_eq!(precision.modulus(), 257);
    let source = profile.plan_fresh(precision).unwrap();
    assert_eq!(
        bound(&source),
        BigInt::from(128)
            + BigInt::from(profile.master_plaintext_modulus())
                * 21_u32
                * (2 * profile.degree() + 1)
    );
    let keys = keys(&profile, &[]);
    let raw: Vec<[u64; 128]> = (0..60)
        .map(|bank| std::array::from_fn(|slot| ((bank * 31 + slot * 7 + 256) % 257) as u64))
        .collect();
    let operands = raw
        .iter()
        .map(|values| encrypt(&profile, &keys, values))
        .collect();
    let base_values = std::array::from_fn::<_, 128, _>(|slot| (slot % 257) as u64);
    let base = encrypt(&profile, &keys, &base_values);
    let lowered = profile
        .plan_switch_to_level(
            &profile
                .plan_fresh(profile.plaintext_precision(1).unwrap())
                .unwrap(),
            2,
        )
        .unwrap();
    let product_plan: ProductPlan<20, 20> = ProductPlan {
        level: 2,
        updates: std::array::from_fn(|i| i),
        state_outputs: std::array::from_fn(|_| {
            profile
                .plan_add(
                    &profile.plan_multiply(&lowered, &lowered).unwrap(),
                    &lowered,
                )
                .unwrap()
        }),
        base_output: lowered,
    };
    reset_counts();
    let mut originals: [Option<LocalScalarCiphertext<'_>>; 21] = std::array::from_fn(|_| None);
    originals[20] = Some(base);
    let state = finish_layer(&profile, &product_plan, operands, originals, &keys.local).unwrap();
    assert_eq!(state.len(), 21);
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    for bank in 0..20 {
        let expected = std::array::from_fn::<_, 128, _>(|slot| {
            (raw[bank][slot] * raw[20 + bank][slot] + raw[40 + bank][slot]) % 257
        });
        let plain = encoding.encode(&profile, &expected).unwrap();
        assert_eq!(
            encoding
                .decode(
                    &keys
                        .secret
                        .decrypt(&profile, state[bank].ciphertext())
                        .unwrap()
                )
                .unwrap()
                .as_slice(),
            expected.as_slice()
        );
        assert_physical(&profile, &keys, state[bank].ciphertext(), &plain);
        assert_eq!(state[bank].shape(), product_plan.state_outputs[bank]);
    }
    assert_eq!(state[20].shape(), product_plan.base_output);
    assert_eq!(
        encoding
            .decode(
                &keys
                    .secret
                    .decrypt(&profile, state[20].ciphertext())
                    .unwrap()
            )
            .unwrap()
            .as_slice(),
        base_values.as_slice()
    );
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1 {
            ciphertext_products: 20,
            ciphertext_additions: 20,
            modulus_switches: 61,
            ..RamLfeBgvNonlinearOperationsV1::default()
        }
    );
    let unchanged = encrypt(&profile, &keys, &base_values);
    let before = unchanged.ciphertext().to_bytes(&profile).unwrap();
    reset_counts();
    let unchanged = lower(&profile, &keys.local, unchanged, 3).unwrap();
    assert_eq!(unchanged.ciphertext().to_bytes(&profile).unwrap(), before);
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    assert!(lower(&profile, &keys.local, unchanged, 4).is_err());
}

fn fixture_program() -> HiddenRamFheProgram {
    let mut instructions = vec![Op::LoadConst(0, 256)];
    for slot in 0..60_u16 {
        instructions.extend([
            Op::LoadInput(1, slot),
            Op::MulPlain(1, 1, (u64::from(slot) * 37 + 256) % 257),
            Op::Add(0, 0, 1),
        ]);
    }
    instructions.extend([
        Op::LoadState(3, 31),
        Op::LoadState(1, 0),
        Op::SelectEqZero(2, 1, 0, 3),
    ]);
    instructions.extend(std::iter::repeat_n(Op::Mul(2, 2, 3), 6));
    instructions.extend([Op::StoreState(0, 2), Op::LoadState(2, 0)]);
    instructions.extend(std::iter::repeat_n(Op::Output(2), 64));
    assert_eq!(instructions.len(), 256);
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for instruction in instructions {
        builder.push(instruction).unwrap();
    }
    builder.finish().unwrap()
}
fn fixture_input() -> RamLfeReferenceInputV1 {
    RamLfeReferenceInputV1::from_bytes(&std::array::from_fn::<_, 63, _>(|i| {
        u8::try_from((i * 43 + 7) % 256).unwrap()
    }))
    .unwrap()
}
fn fixture_memory(condition: u16) -> RamLfeInitialMemoryV1 {
    let mut values =
        std::array::from_fn::<_, 32, _>(|i| u16::try_from((i * 19 + 197) % 257).unwrap());
    values[0] = condition;
    RamLfeInitialMemoryV1::from_lanes(&values).unwrap()
}
fn fixture_cipher<'key>(
    profile: &BgvProfile,
    keys: &'key Keys,
    input: &RamLfeReferenceInputV1,
) -> RamLfeBgvPackedInputV1<'key> {
    RamLfeBgvPackedInputV1::encrypt(
        profile,
        profile.plaintext_precision(1).unwrap(),
        profile.top_level(),
        &keys.local,
        input,
    )
    .unwrap()
}

/// Independent centered CRT and plaintext check; no secret-derived value prints.
fn assert_physical(profile: &BgvProfile, keys: &Keys, cipher: &Ciphertext, expected: &Plaintext) {
    let primes = &profile.parameters().main_primes[..=cipher.level()];
    let modulus = primes.iter().fold(BigInt::from(1), |q, &p| q * p);
    let half = (&modulus - 1_u32) / 2_u32;
    let weights: Vec<BigInt> = primes
        .iter()
        .map(|&p| {
            let partial = &modulus / p;
            let inverse = partial.modpow(&BigInt::from(p - 2), &BigInt::from(p));
            partial * inverse
        })
        .collect();
    let words =
        Zeroizing::new(fixtures::inner_product_words(profile, &keys.secret, cipher).unwrap());
    let declared = integer(cipher.noise_bound().as_natural().words_le());
    let t = BigInt::from(257);
    assert!(declared <= half);
    for coefficient in 0..profile.degree() {
        let canonical = weights
            .iter()
            .enumerate()
            .fold(BigInt::from(0), |sum, (limb, w)| {
                sum + w * words[limb * profile.degree() + coefficient]
            })
            % &modulus;
        let centered = if canonical > half {
            canonical - &modulus
        } else {
            canonical
        };
        let magnitude = if centered.sign() == Sign::Minus {
            -&centered
        } else {
            centered.clone()
        };
        assert!(
            magnitude <= declared,
            "physical noise exceeds public transfer bound"
        );
        assert_eq!(
            ((&centered % &t) + &t) % &t,
            BigInt::from(expected.coefficients()[coefficient])
        );
    }
}

#[test]
#[ignore = "actual full fixed21→60 layer at N128/q26; timing must be observed before full16 run"]
fn unqualified_n128_full_first_layer_real_executor_timing_and_physical_noise() {
    let started = Instant::now();
    let profile = fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &fixture_schedule(),
        fixture_budget(),
    )
    .unwrap();
    let program = fixture_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let compiler = RamLfeBgvNonlinearCompilerV1::from_normal_form(
        &normal,
        RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
    )
    .unwrap();
    let map = compiler
        .prepare_layer(
            RamLfeBgvNonlinearStageV1::BoundedLayer(1),
            &plan.bounded().layers[0].routing,
        )
        .unwrap();
    let map = PreparedLayer {
        stage: RamLfeBgvNonlinearStageV1::BoundedLayer(1),
        map,
    };
    let keys = keys(&profile, plan.galois_elements());
    let input = fixture_input();
    let memory = fixture_memory(3);
    let cipher = fixture_cipher(&profile, &keys, &input);
    cipher
        .preflight(
            &profile,
            &keys.local,
            &plan.bounded().source,
            plan.galois_elements(),
        )
        .unwrap();
    let (original, state) =
        initialize_state(&profile, plan.bounded(), &memory, cipher, &keys.local).unwrap();
    drop(original);
    reset_counts();
    let execution = Instant::now();
    let result = execute_layer(
        &profile,
        &plan.bounded().layers[0],
        &map,
        state,
        &keys.local,
    )
    .unwrap();
    let elapsed = execution.elapsed().as_millis();
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    for (bank, value) in result[..21].iter().enumerate() {
        let mut expected = [0_u64; 128];
        if bank == 0 {
            expected[0] = 9;
        }
        if bank == 20 {
            for (to, &from) in expected[..64].iter_mut().zip(input.slots()) {
                *to = u64::from(from);
            }
            for (to, &from) in expected[64..96].iter_mut().zip(memory.lanes()) {
                *to = u64::from(from);
            }
            expected[96] = 1;
        }
        assert_eq!(
            encoding
                .decode(&keys.secret.decrypt(&profile, value.ciphertext()).unwrap())
                .unwrap()
                .as_slice(),
            expected.as_slice()
        );
        assert_physical(
            &profile,
            &keys,
            value.ciphertext(),
            &encoding.encode(&profile, &expected).unwrap(),
        );
    }
    let route = &plan.bounded().layers[0].routing;
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1 {
            routing_calls: 1,
            galois_applications: route.galois_applications(),
            plaintext_products: route.plaintext_products(),
            ciphertext_additions: route.plaintext_products() + 20,
            ciphertext_products: 20,
            modulus_switches: 61,
            ..RamLfeBgvNonlinearOperationsV1::default()
        }
    );
    eprintln!(
        "bounded_unqualified_first_layer n=128 main=26 layer=1 route_level=25 product_level=24 execute_ms={} total_ms={}",
        elapsed,
        started.elapsed().as_millis()
    );
}

#[test]
#[ignore = "complete authoritative 16-layer encrypted arithmetic, 18.9MB prepared matrices; potentially long run"]
fn unqualified_n128_complete_sixteen_layers_match_reference_and_physical_noise() {
    let started = Instant::now();
    let profile = fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &fixture_schedule(),
        fixture_budget(),
    )
    .unwrap();
    let program = fixture_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    assert_eq!(normal.report().peak_rank(), 16);
    let prepare = Instant::now();
    let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan).unwrap();
    let prepare_ms = prepare.elapsed().as_millis();
    drop(normal);
    let keys = keys(&profile, plan.galois_elements());
    let input = fixture_input();
    let memory = fixture_memory(0);
    let cipher = fixture_cipher(&profile, &keys, &input);
    let missing = self::keys(&profile, &[]);
    let missing_cipher = fixture_cipher(&profile, &missing, &input);
    let before = cleared_cells();
    reset_counts();
    assert_eq!(
        prepared
            .evaluate(&profile, &plan, &memory, missing_cipher, &missing.local)
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::Bootstrap(BootstrapError::Bgv(BgvError::WrongSwitchingKey))
    );
    assert_eq!(cleared_cells(), before);
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1::default()
    );
    let execution = Instant::now();
    let output = prepared
        .evaluate(&profile, &plan, &memory, cipher, &keys.local)
        .unwrap();
    let execution_ms = execution.elapsed().as_millis();
    assert_eq!(ACTUAL.with(Cell::get), *plan.operations());
    assert_eq!(output.shape(), *plan.output_shape());
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Bounded, &program, &memory, &input).unwrap();
    let mut scalar = 256_u64;
    for slot in 0..60 {
        scalar = (scalar + u64::from(input.slots()[slot]) * ((slot as u64 * 37 + 256) % 257)) % 257;
    }
    for _ in 0..6 {
        scalar = scalar * u64::from(memory.lanes()[31]) % 257;
    }
    assert!(
        reference
            .output()
            .scalars()
            .iter()
            .all(|&x| u64::from(x) == scalar)
    );
    let mut expected = [0_u64; 128];
    expected[..64].fill(scalar);
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    assert_eq!(
        encoding
            .decode(&keys.secret.decrypt(&profile, output.ciphertext()).unwrap())
            .unwrap()
            .as_slice(),
        expected.as_slice()
    );
    assert_physical(
        &profile,
        &keys,
        output.ciphertext(),
        &encoding.encode(&profile, &expected).unwrap(),
    );
    eprintln!(
        "bounded_unqualified_full n=128 main=26 layers=16 prepare_ms={} execute_ms={} total_ms={}",
        prepare_ms,
        execution_ms,
        started.elapsed().as_millis()
    );
}

#[test]
fn explicit_bounded_message_precision_refuses_other_contexts_before_preparation() {
    let profile =
        BgvProfile::new(BgvParameters::derive(7, 257, 3, (50, 26), (60, 2), 2).unwrap()).unwrap();
    let foreign = fixture_profile();
    let before = cleared_cells();
    reset_counts();
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            profile.plaintext_precision(2).unwrap(),
            &fixture_schedule(),
            fixture_budget()
        ),
        Err(RamLfeBgvNonlinearErrorV1::UnsupportedProfile)
    ));
    assert!(matches!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            foreign.plaintext_precision(1).unwrap(),
            &fixture_schedule(),
            fixture_budget()
        ),
        Err(RamLfeBgvNonlinearErrorV1::Bgv(BgvError::ProfileMismatch))
    ));
    assert_eq!(cleared_cells(), before);
    assert_eq!(
        ACTUAL.with(Cell::get),
        RamLfeBgvNonlinearOperationsV1::default()
    );
}

pub(super) fn refresh_fixture_profile() -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(7, 257, 3, (60, 20), (60, 2), 2).unwrap()).unwrap()
}

pub(super) fn refresh_fixture_schedule() -> RamLfeBgvNonlinearScheduleV1 {
    use iroha_fhe::{
        bgv::{BootstrapBudget, CoefficientSlotBudget, UpperDigitsBudget},
        prime_power::{PrimePowerContext, UpperDigitsBudget as ProgramBudget, UpperDigitsPlan},
    };
    let context = PrimePowerContext::new(257, 3).unwrap();
    let required = UpperDigitsPlan::required(context, 1).unwrap();
    let program = UpperDigitsPlan::new(
        context,
        1,
        ProgramBudget {
            coefficient_bytes: required.coefficient_bytes,
            preparation_scratch_bytes: required.preparation_scratch_bytes,
            program_bytes: required.program_bytes,
            preparation_operations: required.preparation_operations,
            live_values: required.live_values,
        },
    )
    .unwrap();
    let transform = CoefficientSlotBudget {
        routing: SlotLinearBudget {
            coefficient_bytes: 128 * 129 * 4,
            layout_metadata_bytes: 16 << 10,
            preparation_scratch_bytes: 129 * 4,
            output_residue_bytes: 1 << 20,
            scratch_residue_bytes: 2 << 20,
        },
        owned_residue_bytes: 8 << 20,
        trace_plaintext_bytes: 1024,
    };
    let bootstrap = BootstrapBudget {
        decomposition: transform,
        recomposition: transform,
        upper_digits: UpperDigitsBudget {
            compiler_steps: 1_000_000,
            metadata_bytes: 16 << 20,
            owned_residue_bytes: 8 << 20,
            plaintext_bytes: 1024,
            backend_scratch_bytes: 8 << 20,
            canonical_arithmetic_units: 1_u64 << 40,
            scalar_word_operations: 1_u64 << 36,
        },
        retained_preparation_bytes: 32 << 20,
        input_owned_bytes: 1 << 20,
        input_plaintext_bytes: 2048,
        input_arithmetic_units: 1_u64 << 32,
        input_addition_words: 1_u64 << 24,
        stream_owned_residue_bytes: 696_320,
        stream_metadata_bytes: 1 << 20,
        canonical_arithmetic_units: 22_275_456,
        scalar_word_operations: 1_821_952,
    };
    RamLfeBgvNonlinearScheduleV1 {
        input_level: 19,
        refresh: Some(RamLfeBgvNonlinearRefreshChoiceV1 {
            source_level: 0,
            program,
            levels: vec![
                18, 17, 16, 15, 14, 13, 12, 11, 11, 10, 9, 8, 7, 6, 5, 4, 4, 4,
            ],
            budget: bootstrap,
        }),
        layers: RamLfeBgvNonlinearLayersV1::Bounded(Box::new(
            [RamLfeBgvNonlinearLayerChoiceV1 {
                routing_level: 4,
                multiplication_level: 1,
                routing_scratch_bytes: 317_440,
            }; 16],
        )),
        projection_level: 4,
        output_level: 0,
        projection_scratch_bytes: 317_440,
    }
}

fn refresh_fixture_budget() -> RamLfeBgvNonlinearBudgetV1 {
    RamLfeBgvNonlinearBudgetV1 {
        maximum_refresh_plans: 21,
        preparation_payload_bytes: 512 << 20,
        preparation_metadata_bytes: 64 << 20,
        canonical_arithmetic_units: 16_638_609_472,
        scalar_word_operations: 2_059_780_864,
        owned_residue_bytes: 1_146_880,
        ..fixture_budget()
    }
}

#[test]
fn complete_public_refresh_plan_reuses_only_exact_shapes_and_counts_all_calls() {
    let profile = refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = refresh_fixture_schedule();
    reset_counts();
    let plan =
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, refresh_fixture_budget())
            .unwrap();
    assert_eq!(plan.prepared_refresh_plans(), 21);
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 1);
    assert_eq!(plan.operations().bootstrap_calls, 357);
    assert_eq!(plan.operations().routing_calls, 17);
    assert_eq!(plan.operations().ciphertext_products, 320);
    assert_eq!(plan.operations().modulus_switches, 3667);
    assert_eq!(plan.canonical_arithmetic_units(), 16_638_609_472);
    assert_eq!(plan.scalar_word_operations(), 2_059_780_864);
    assert_eq!(plan.owned_residue_bytes(), 1_146_880);
    assert_eq!(bound(plan.output_shape()), BigInt::from(1_094_861_248_u64));
    assert_eq!(plan.output_shape().level(), 0);
    assert_eq!(
        plan.preparation_requirement().retained_payload_bytes,
        36_238_176
    );
    assert_eq!(
        plan.preparation_requirement().peak_payload_bytes,
        36_838_178
    );
    let refresh = plan.bounded().refresh.as_ref().unwrap();
    assert_eq!(plan.bounded().initial_refresh, [Some(0); 21]);
    let expected_indices: [Option<usize>; 21] =
        std::array::from_fn(|i| Some(if i == 20 { 0 } else { i + 1 }));
    assert!(
        plan.bounded()
            .layers
            .iter()
            .all(|stage| stage.after_refresh == expected_indices)
    );
    let expected = [
        1_094_861_248_u64,
        1_099_353_311,
        1_104_930_541,
        1_112_728_767,
        1_122_376_463,
        1_134_209_518,
        1_147_575_492,
        1_164_281_213,
        1_183_117_129,
        1_203_107_384,
        1_225_136_675,
        1_248_657_548,
        1_276_855_059,
        1_306_910_781,
        1_337_061_479,
        1_368_923_709,
        1_402_599_481,
        1_442_652_077,
        1_484_109_720,
        1_524_238_746,
        1_556_621_555,
    ];
    for (at, public) in refresh.plans.iter().enumerate() {
        assert_eq!(
            public.message_space(),
            iroha_fhe::bgv::BootstrapMessageSpace::ScalarSlots
        );
        assert_eq!(bound(public.input_shape()), BigInt::from(expected[at]));
        assert_eq!(public.input_shape().plaintext_precision(), precision);
        assert_eq!(public.input_shape().level(), 0);
        for earlier in &refresh.plans[..at] {
            assert_ne!(public.input_shape(), earlier.input_shape());
        }
        assert_eq!(public.output_shape(), refresh.plans[0].output_shape());
    }
    reset_counts();
    let error = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        precision,
        &schedule,
        RamLfeBgvNonlinearBudgetV1 {
            maximum_refresh_plans: 20,
            ..refresh_fixture_budget()
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        RamLfeBgvNonlinearErrorV1::RefreshPlanBudget {
            required: 21,
            budget: 20
        }
    );
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 1);
    assert_eq!(actual_counts(), RamLfeBgvNonlinearOperationsV1::default());
    reset_counts();
}

#[test]
fn complete_preparation_and_work_caps_accept_exact_and_refuse_one_under() {
    let profile = fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = fixture_schedule();
    let plan =
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, fixture_budget()).unwrap();
    assert_eq!(plan.canonical_arithmetic_units(), 14_481_008_704);
    assert_eq!(plan.scalar_word_operations(), 4_342_931_968);
    assert_eq!(plan.prepared_refresh_plans(), 0);
    assert_eq!(plan.operations().bootstrap_calls, 0);
    let exact = RamLfeBgvNonlinearBudgetV1 {
        preparation_payload_bytes: plan.preparation_requirement().peak_payload_bytes,
        preparation_metadata_bytes: plan.preparation_requirement().peak_metadata_bytes,
        canonical_arithmetic_units: plan.canonical_arithmetic_units(),
        scalar_word_operations: plan.scalar_word_operations(),
        ..fixture_budget()
    };
    RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, exact).unwrap();
    let before = cleared_cells();
    for budget in [
        RamLfeBgvNonlinearBudgetV1 {
            preparation_payload_bytes: exact.preparation_payload_bytes - 1,
            ..exact
        },
        RamLfeBgvNonlinearBudgetV1 {
            preparation_metadata_bytes: exact.preparation_metadata_bytes - 1,
            ..exact
        },
        RamLfeBgvNonlinearBudgetV1 {
            canonical_arithmetic_units: exact.canonical_arithmetic_units - 1,
            ..exact
        },
        RamLfeBgvNonlinearBudgetV1 {
            scalar_word_operations: exact.scalar_word_operations - 1,
            ..exact
        },
    ] {
        reset_counts();
        assert!(matches!(
            RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, budget),
            Err(RamLfeBgvNonlinearErrorV1::PreparationBudget { .. }
                | RamLfeBgvNonlinearErrorV1::WorkBudget { .. })
        ));
        assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
        assert_eq!(cleared_cells(), before);
        assert_eq!(actual_counts(), RamLfeBgvNonlinearOperationsV1::default());
    }
    assert_eq!(
        RamLfeBgvNonlinearPlanV1::new(
            &profile,
            precision,
            &schedule,
            RamLfeBgvNonlinearBudgetV1 {
                maximum_refresh_plans: 1,
                ..exact
            }
        )
        .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::RefreshSchedule
    );
    reset_counts();
}

#[test]
fn refreshed_preparation_and_work_caps_accept_exact_and_refuse_one_under() {
    let profile = refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = refresh_fixture_schedule();
    let plan =
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, refresh_fixture_budget())
            .unwrap();
    let exact = RamLfeBgvNonlinearBudgetV1 {
        preparation_payload_bytes: plan.preparation_requirement().peak_payload_bytes,
        preparation_metadata_bytes: plan.preparation_requirement().peak_metadata_bytes,
        ..refresh_fixture_budget()
    };
    let accepted = RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, exact).unwrap();
    assert_eq!(accepted.prepared_refresh_plans(), 21);
    assert_eq!(accepted.operations().bootstrap_calls, 357);
    let before = cleared_cells();
    for (budget, public_preparations, expected) in [
        (
            RamLfeBgvNonlinearBudgetV1 {
                preparation_payload_bytes: exact.preparation_payload_bytes - 1,
                ..exact
            },
            0,
            RamLfeBgvNonlinearErrorV1::PreparationBudget {
                resource: "complete preparation payload",
                required: exact.preparation_payload_bytes,
                budget: exact.preparation_payload_bytes - 1,
            },
        ),
        (
            RamLfeBgvNonlinearBudgetV1 {
                preparation_metadata_bytes: exact.preparation_metadata_bytes - 1,
                ..exact
            },
            0,
            RamLfeBgvNonlinearErrorV1::PreparationBudget {
                resource: "complete preparation metadata",
                required: exact.preparation_metadata_bytes,
                budget: exact.preparation_metadata_bytes - 1,
            },
        ),
        (
            RamLfeBgvNonlinearBudgetV1 {
                canonical_arithmetic_units: exact.canonical_arithmetic_units - 1,
                ..exact
            },
            1,
            RamLfeBgvNonlinearErrorV1::WorkBudget {
                resource: "canonical arithmetic units",
                required: exact.canonical_arithmetic_units,
                budget: exact.canonical_arithmetic_units - 1,
            },
        ),
        (
            RamLfeBgvNonlinearBudgetV1 {
                scalar_word_operations: exact.scalar_word_operations - 1,
                ..exact
            },
            1,
            RamLfeBgvNonlinearErrorV1::WorkBudget {
                resource: "scalar word operations",
                required: exact.scalar_word_operations,
                budget: exact.scalar_word_operations - 1,
            },
        ),
    ] {
        reset_counts();
        assert_eq!(
            RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, budget).unwrap_err(),
            expected
        );
        // Work admission follows bounded public planning but precedes every
        // private compiler/map callback and all ciphertext execution.
        assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), public_preparations);
        assert_eq!(cleared_cells(), before);
        assert_eq!(actual_counts(), RamLfeBgvNonlinearOperationsV1::default());
    }
    reset_counts();
}

#[test]
fn exact_refresh_shapes_remain_distinct_and_checked_totals_refuse_overflow() {
    let profile = refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let schedule = refresh_fixture_schedule();
    let choice = schedule.refresh.as_ref().unwrap();
    let source = profile
        .plan_switch_to_level(&profile.plan_fresh(precision).unwrap(), 0)
        .unwrap();
    let encoding = SlotEncoding::new(&profile, precision).unwrap();
    let changed = profile
        .plan_add_plain(&source, encoding.vector_bound())
        .unwrap();
    assert_ne!(source, changed);
    let mut sources: [CiphertextShape; 21] = std::array::from_fn(|_| source.clone());
    sources[13] = changed.clone();
    let mut plans = RefreshPlans::new(0, 2).unwrap();
    let mut work = Work::default();
    let mut switches = 0;
    let mut peak = 0;
    reset_counts();
    let indices = plans
        .group(
            &profile,
            choice,
            RefreshSelection {
                sources: &mut sources,
                selected: &[true; 21],
            },
            &mut work,
            &mut switches,
            &mut peak,
        )
        .unwrap();
    assert_eq!(indices, std::array::from_fn(|i| Some(usize::from(i == 13))));
    assert_eq!(plans.plans.len(), 2);
    assert_eq!(plans.plans[0].input_shape(), &source);
    assert_eq!(plans.plans[1].input_shape(), &changed);
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 1);
    assert!(
        sources
            .iter()
            .all(|shape| shape == plans.plans[0].output_shape())
    );
    assert_eq!(switches, 0);
    assert_eq!(work.arithmetic, 21 * 22_275_456);
    assert_eq!(work.words, 21 * 1_821_952);
    assert!(peak > 0);
    for mut work in [
        Work {
            arithmetic: u64::MAX,
            words: 0,
        },
        Work {
            arithmetic: 0,
            words: u64::MAX,
        },
    ] {
        assert_eq!(work.add(1, 1), Err(RamLfeBgvNonlinearErrorV1::Geometry));
    }
    let mut overflowing = schedule.clone();
    overflowing
        .refresh
        .as_mut()
        .unwrap()
        .budget
        .decomposition
        .routing
        .coefficient_bytes = usize::MAX;
    reset_counts();
    assert_eq!(
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &overflowing, refresh_fixture_budget())
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::Geometry
    );
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
    assert_eq!(actual_counts(), RamLfeBgvNonlinearOperationsV1::default());
    reset_counts();
}

#[test]
fn refresh_public_program_and_count_refusals_precede_preparation() {
    let profile = refresh_fixture_profile();
    let precision = profile.plaintext_precision(1).unwrap();
    let original = refresh_fixture_schedule();
    let geometry = original.refresh.as_ref().unwrap().program.geometry();
    let required = geometry.coefficient_bytes + geometry.program_bytes;
    for ceiling in [0, required - 1] {
        let mut schedule = original.clone();
        schedule
            .refresh
            .as_mut()
            .unwrap()
            .budget
            .upper_digits
            .metadata_bytes = ceiling;
        reset_counts();
        assert!(matches!(
            RamLfeBgvNonlinearPlanV1::new(&profile, precision, &schedule, refresh_fixture_budget()),
            Err(RamLfeBgvNonlinearErrorV1::PreparationBudget {
                resource: "bootstrap public program payload",
                ..
            })
        ));
        assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
    }
    for maximum in [0, 358, usize::MAX] {
        reset_counts();
        assert_eq!(
            RamLfeBgvNonlinearPlanV1::new(
                &profile,
                precision,
                &original,
                RamLfeBgvNonlinearBudgetV1 {
                    maximum_refresh_plans: maximum,
                    ..refresh_fixture_budget()
                }
            )
            .unwrap_err(),
            RamLfeBgvNonlinearErrorV1::RefreshSchedule
        );
        assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
    }
    let mut oversized = original.clone();
    oversized.refresh.as_mut().unwrap().levels = vec![0; geometry.polynomial_evaluations * 257 + 1];
    assert_eq!(
        RamLfeBgvNonlinearPlanV1::new(&profile, precision, &oversized, refresh_fixture_budget())
            .unwrap_err(),
        RamLfeBgvNonlinearErrorV1::RefreshSchedule
    );
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
    reset_counts();
}

#[test]
#[ignore = "complete fixed16-layer physical-refresh path,357genuine bootstraps; parent schedules the diagnostic"]
fn unqualified_n128_master_e3_full_refreshed_maximum_tape_matches_reference() {
    let started = Instant::now();
    let profile = refresh_fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &refresh_fixture_schedule(),
        refresh_fixture_budget(),
    )
    .unwrap();
    let program = fixture_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    assert_eq!(normal.report().peak_rank(), 16);
    let prepared = RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan).unwrap();
    let keys = keys(&profile, plan.galois_elements());
    let input = fixture_input();
    let memory = fixture_memory(0);
    let cipher = fixture_cipher(&profile, &keys, &input);
    reset_counts();
    let output = prepared
        .evaluate(&profile, &plan, &memory, cipher, &keys.local)
        .unwrap();
    assert_eq!(actual_counts(), *plan.operations());
    assert_eq!(actual_counts().bootstrap_calls, 357);
    assert_eq!(output.shape(), *plan.output_shape());
    let reference =
        ram_lfe_reference_execute_v1(RamLfeClassV1::Bounded, &program, &memory, &input).unwrap();
    let mut scalar = 256_u64;
    for slot in 0..60 {
        scalar = (scalar + u64::from(input.slots()[slot]) * ((slot as u64 * 37 + 256) % 257)) % 257;
    }
    for _ in 0..6 {
        scalar = scalar * u64::from(memory.lanes()[31]) % 257;
    }
    assert_eq!(
        reference.output().scalars(),
        &[u16::try_from(scalar).unwrap(); 64]
    );
    let mut expected = [0_u64; 128];
    expected[..64].fill(scalar);
    let encoding = SlotEncoding::new(&profile, profile.plaintext_precision(1).unwrap()).unwrap();
    assert_eq!(
        encoding
            .decode(&keys.secret.decrypt(&profile, output.ciphertext()).unwrap())
            .unwrap()
            .as_slice(),
        &expected
    );
    assert_physical(
        &profile,
        &keys,
        output.ciphertext(),
        &encoding.encode(&profile, &expected).unwrap(),
    );
    eprintln!(
        "bounded_refreshed_unqualified n=128 main=20 layers=16 calls=357 total_ms={}",
        started.elapsed().as_millis()
    );
}

#[test]
fn private_preparation_failure_clears_compiler_after_earlier_maps() {
    let profile = fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &fixture_schedule(),
        fixture_budget(),
    )
    .unwrap();
    let mut affine_program = HiddenRamFheProgram::builder().unwrap();
    affine_program.push(Op::LoadInput(0, 0)).unwrap();
    affine_program.push(Op::Output(0)).unwrap();
    let affine =
        ram_lfe_reference_normalize_v1(RamLfeClassV1::Affine, &affine_program.finish().unwrap())
            .unwrap();
    reset_counts();
    let before = cleared_cells();
    assert!(matches!(
        RamLfeBgvNonlinearProgramV1::from_normal_form(&affine, &plan),
        Err(RamLfeBgvNonlinearErrorV1::Compile(
            RamLfeBgvNonlinearCompileErrorV1::ClassMismatch
        ))
    ));
    assert_eq!(cleared_cells(), before);
    assert_eq!(MATCHES.with(Cell::get), 0);
    assert_eq!(PUBLIC_PREPARATIONS.with(Cell::get), 0);
    assert_eq!(actual_counts(), RamLfeBgvNonlinearOperationsV1::default());
    let program = fixture_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    for (stage, at) in [
        ("private layer prepared", 1),
        ("private layer prepared", 8),
        ("private layer prepared", 16),
        ("private projection prepared", 1),
    ] {
        for unwind in [false, true] {
            reset_counts();
            FAULT.with(|fault| fault.set((stage, at, unwind)));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                RamLfeBgvNonlinearProgramV1::from_normal_form(&normal, &plan)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RamLfeBgvNonlinearErrorV1::Allocation)
                ));
            }
            assert_eq!(MATCHES.with(Cell::get), at);
            // This counter directly observes the crypto compiler's clearing.
            // Map coefficient retirement uses its existing lower owner tests;
            // no claim of an allocator-memory observation is made here.
            assert_eq!(
                cleared_cells() - CLEARED_AT_FAULT.with(Cell::get),
                RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes() / size_of::<u16>()
            );
        }
    }
    reset_counts();
}

#[test]
fn closed_layer_failures_retire_every_input_and_partial_output() {
    let profile = fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &fixture_schedule(),
        fixture_budget(),
    )
    .unwrap();
    let program = fixture_program();
    let normal = ram_lfe_reference_normalize_v1(RamLfeClassV1::Bounded, &program).unwrap();
    let compiler = RamLfeBgvNonlinearCompilerV1::from_normal_form(
        &normal,
        RamLfeBgvNonlinearShapeV1::Bounded.private_payload_bytes(),
    )
    .unwrap();
    let map = compiler
        .prepare_layer(
            RamLfeBgvNonlinearStageV1::BoundedLayer(1),
            &plan.bounded().layers[0].routing,
        )
        .unwrap();
    let map = PreparedLayer {
        stage: RamLfeBgvNonlinearStageV1::BoundedLayer(1),
        map,
    };
    let keys = keys(&profile, plan.galois_elements());
    let input = fixture_input();
    let memory = fixture_memory(0);
    for (stage, at, owners, level) in [("layer routed", 1, 81, 25), ("layer product", 2, 58, 24)] {
        for unwind in [false, true] {
            reset_counts();
            let source = fixture_cipher(&profile, &keys, &input);
            let (original, state) =
                initialize_state(&profile, plan.bounded(), &memory, source, &keys.local).unwrap();
            drop(original);
            FAULT.with(|fault| fault.set((stage, at, unwind)));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute_layer(
                    &profile,
                    &plan.bounded().layers[0],
                    &map,
                    state,
                    &keys.local,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(RamLfeBgvNonlinearErrorV1::Allocation)
                ));
            }
            assert_eq!(MATCHES.with(Cell::get), at);
            assert_eq!(
                fixtures::cleared_residue_words() - RETIRED_AT_FAULT.with(Cell::get),
                owners * 2 * profile.degree() * (level + 1)
            );
        }
    }
    reset_counts();
}

#[test]
fn refresh_failure_retires_pending_sources_and_completed_outputs() {
    let profile = refresh_fixture_profile();
    let plan = RamLfeBgvNonlinearPlanV1::new(
        &profile,
        profile.plaintext_precision(1).unwrap(),
        &refresh_fixture_schedule(),
        refresh_fixture_budget(),
    )
    .unwrap();
    let refresh = plan.bounded().refresh.as_ref().unwrap();
    let keys = keys(&profile, plan.galois_elements());
    let input = fixture_input();
    let memory = fixture_memory(0);
    for unwind in [false, true] {
        reset_counts();
        let source = fixture_cipher(&profile, &keys, &input);
        let (original, state) =
            initialize_state(&profile, plan.bounded(), &memory, source, &keys.local).unwrap();
        drop(original);
        FAULT.with(|fault| fault.set(("refreshed bank", 2, unwind)));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            refresh_banks(
                &profile,
                &keys.local,
                refresh,
                &plan.bounded().initial_refresh,
                state,
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RamLfeBgvNonlinearErrorV1::Allocation)
            ));
        }
        assert_eq!(MATCHES.with(Cell::get), 2);
        assert_eq!(actual_counts().bootstrap_calls, 2);
        // Nineteen pending source0 owners and two completed output4 owners.
        let expected = 19 * 2 * profile.degree() + 2 * 2 * profile.degree() * 5;
        assert_eq!(
            fixtures::cleared_residue_words() - RETIRED_AT_FAULT.with(Cell::get),
            expected
        );
    }
    reset_counts();
}

pub(super) fn public_preparations() -> usize {
    PUBLIC_PREPARATIONS.with(Cell::get)
}

pub(super) fn arm_fault(stage: &'static str, at: usize, unwind: bool) {
    FAULT.with(|fault| fault.set((stage, at, unwind)));
}
pub(super) fn retired_at_fault() -> usize {
    RETIRED_AT_FAULT.with(Cell::get)
}
