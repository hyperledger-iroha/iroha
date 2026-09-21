//! Real canonical T256 commitments and actual retained resource custody.
use super::*;
use crate::generalized_bulletproof::{
    ProofSuite, SecretMultiexpBuilder, secret_u15_msm_v1::test_controls_v1 as controls,
};

fn original_commitment_v1(values: &[u16], rho: &Scalar) -> SecretPoint<Point> {
    let basis = Suite::generators().reduce(values.len()).unwrap();
    let mut builder = SecretMultiexpBuilder::<Suite>::new(values.len() + 1).unwrap();
    for (value, generator) in values.iter().zip(basis.g_bold) {
        builder
            .push(&Scalar::from_u64(u64::from(*value)), generator)
            .unwrap();
    }
    builder.push(rho, &basis.h).unwrap();
    builder.evaluate().unwrap()
}

#[test]
fn u15_canonical_kernel_matches_original_full_width_commitments_and_fixed_work() {
    assert!(!Suite::ALLOW_PARALLEL_PROVER_WORKSPACE_V1);
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    let mut high = [0u8; 32];
    high[0] = 0x40;
    high[31] = 7;
    let high_rho = Scalar::from_be_bytes_exact(high).unwrap();
    let expected_work = controls::WorkV1 {
        table_adds: 0,
        windows: 256,
        doubles: 1024,
        selects: 1_048_576,
        window_adds: 65_536,
        folds: 64,
        rho_terms: 1,
        combines: 1,
    };
    for pattern in 0..3 {
        let values: Vec<u16> = (0..16_384)
            .map(|i| match pattern {
                0 => 0,
                1 => 32767,
                _ => ((i * 8191 + i / 256) % 32768) as u16,
            })
            .collect();
        let expected = original_commitment_v1(&values, &high_rho);
        controls::reset_v1();
        let actual = table
            .commitment_v1(&budget, values.len(), |i| Ok(values[i]), &high_rho)
            .unwrap();
        assert!(expected.equals(actual.expose_ref_v1()));
        assert_eq!(controls::work_v1(), expected_work);
        assert_eq!(controls::clear_v1(), (16_384, true));
        actual.require_original_budget_v1(&budget).unwrap();
    }
    assert_eq!(
        budget.live_bytes().unwrap(),
        RnsNativeU15MsmTableV1::retained_bytes_v1() as u64
    );
}

#[test]
fn u15_kernel_binds_boundary_coordinates_and_full_width_rho() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    let rho = Scalar::from_u64(7);
    let base = table
        .commitment_v1(&budget, 16_384, |_| Ok(0), &rho)
        .unwrap();
    let generators = Suite::generators().reduce(16_384).unwrap();
    for changed in [0, 255, 256, 257, 16_383] {
        let altered = table
            .commitment_v1(&budget, 16_384, |i| Ok(u16::from(i == changed)), &rho)
            .unwrap();
        assert_eq!(
            *altered.expose_ref_v1() - *base.expose_ref_v1(),
            generators.g_bold[changed]
        );
    }
    let mut high = [0u8; 32];
    high[0] = 0x40;
    high[31] = 7;
    let changed_rho = Scalar::from_be_bytes_exact(high).unwrap();
    let high_result = table
        .commitment_v1(&budget, 16_384, |_| Ok(0), &changed_rho)
        .unwrap();
    let expected = original_commitment_v1(&vec![0; 16_384], &changed_rho);
    assert!(expected.equals(high_result.expose_ref_v1()));
    assert_ne!(high_result.expose_ref_v1(), base.expose_ref_v1());
}

#[test]
fn u15_real_table_capacity_refuses_before_allocation_then_retries_after_actual_drop() {
    let retained = RnsNativeU15MsmTableV1::retained_bytes_v1() as u64;
    let scratch = CanonicalU15PublicTableV1::<Suite>::construction_scratch_bytes_v1() as u64;
    let mut one_short =
        RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(retained + scratch - 1);
    controls::reset_v1();
    assert!(matches!(
        RnsNativeU15MsmTableV1::new_v1(&mut one_short),
        Err(RnsNativeU15MsmErrorV1::Capacity)
    ));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(one_short.live_bytes().unwrap(), 0);
    let mut budget =
        RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(retained + scratch);
    let first = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    assert_eq!(budget.live_bytes().unwrap(), retained);
    controls::reset_v1();
    assert!(matches!(
        RnsNativeU15MsmTableV1::new_v1(&mut budget),
        Err(RnsNativeU15MsmErrorV1::Capacity)
    ));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(budget.live_bytes().unwrap(), retained);
    drop(first);
    assert_eq!(budget.live_bytes().unwrap(), 0);
    let retry = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    retry.require_original_budget_v1(&budget).unwrap();
    drop(retry);
    assert_eq!(budget.live_bytes().unwrap(), 0);
    assert_eq!(budget.peak_bytes().unwrap(), retained + scratch);
    // This slice reserves new named memory; it does not assign hash work units
    // to T256 arithmetic or claim a complete work ledger.
    assert_eq!(budget.consumed().unwrap(), 0);
}

#[test]
fn u15_table_and_digit_allocation_failures_refund_real_reservations() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    controls::reset_v1();
    controls::fail_table_v1();
    assert!(matches!(
        RnsNativeU15MsmTableV1::new_v1(&mut budget),
        Err(RnsNativeU15MsmErrorV1::Allocation)
    ));
    assert_eq!(budget.live_bytes().unwrap(), 0);
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    controls::reset_v1();
    controls::fail_digits_v1();
    assert!(matches!(
        table.commitment_v1(
            &budget,
            16_384,
            |_| panic!("allocation failure must precede digit read"),
            &Scalar::from_u64(9)
        ),
        Err(RnsNativeU15MsmErrorV1::Allocation)
    ));
    assert_eq!(
        budget.live_bytes().unwrap(),
        RnsNativeU15MsmTableV1::retained_bytes_v1() as u64
    );
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = table.commitment_v1(
            &budget,
            16_384,
            |i| {
                assert_ne!(i, 257, "injected source unwind");
                Ok(17)
            },
            &Scalar::from_u64(9),
        );
    }));
    assert!(unwound.is_err());
    assert_eq!(controls::clear_v1(), (257, true));
    // Preserve the exact malformed-prefix control retired from the unsupported
    // retained-source forwarding route; this is the actual arithmetic owner.
    assert!(matches!(
        table.commitment_v1(
            &budget,
            16_384,
            |i| Ok(if i == 256 { 32768 } else { 1 }),
            &Scalar::from_u64(7)
        ),
        Err(RnsNativeU15MsmErrorV1::Source)
    ));
    assert_eq!(controls::clear_v1(), (256, true));
    assert_eq!(
        budget.live_bytes().unwrap(),
        RnsNativeU15MsmTableV1::retained_bytes_v1() as u64
    );
}

#[test]
fn u15_original_ledger_identity_and_escaped_result_keep_exact_custody() {
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let unrelated = RnsNativeProofResourceBudgetV1::default();
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    controls::reset_v1();
    assert!(matches!(
        table.commitment_v1(
            &unrelated,
            16_384,
            |_| panic!("unrelated ledger must reject before source read"),
            &Scalar::from_u64(3)
        ),
        Err(RnsNativeU15MsmErrorV1::Source)
    ));
    assert_eq!(controls::allocations_v1(), 0);
    let point = table
        .commitment_v1(
            &budget,
            16_384,
            |i| Ok((i % 16) as u16),
            &Scalar::from_u64(3),
        )
        .unwrap();
    assert_eq!(
        point.require_original_budget_v1(&unrelated),
        Err(RnsNativeU15MsmErrorV1::Source)
    );
    drop(table);
    assert_eq!(
        budget.live_bytes().unwrap(),
        core::mem::size_of::<RnsNativeU15CommitmentV1>() as u64
    );
    point.require_original_budget_v1(&budget).unwrap();
    drop(point);
    assert_eq!(budget.live_bytes().unwrap(), 0);
}

#[test]
fn u15_evaluation_capacity_and_malformed_values_cannot_read_or_escape() {
    let retained = RnsNativeU15MsmTableV1::retained_bytes_v1() as u64;
    let construction = CanonicalU15PublicTableV1::<Suite>::construction_scratch_bytes_v1() as u64;
    let mut tight =
        RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(retained + construction);
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut tight).unwrap();
    controls::reset_v1();
    assert!(matches!(
        table.commitment_v1(
            &tight,
            16_384,
            |_| panic!("capacity refusal must precede source read"),
            &Scalar::from_u64(3)
        ),
        Err(RnsNativeU15MsmErrorV1::Capacity)
    ));
    assert_eq!(controls::allocations_v1(), 0);
    assert_eq!(tight.live_bytes().unwrap(), retained);
    drop(table);
    let mut budget = RnsNativeProofResourceBudgetV1::default();
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut budget).unwrap();
    for bad in [32768, u16::MAX] {
        assert!(matches!(
            table.commitment_v1(
                &budget,
                16_384,
                |i| Ok(if i == 256 { bad } else { 1 }),
                &Scalar::from_u64(3)
            ),
            Err(RnsNativeU15MsmErrorV1::Source)
        ));
        assert_eq!(controls::clear_v1(), (256, true));
        assert_eq!(budget.live_bytes().unwrap(), retained);
    }
}

#[test]
fn u15_pre_admitted_evaluation_rejects_foreign_ledger_and_refunds_only_its_owner() {
    let mut original = RnsNativeProofResourceBudgetV1::default();
    let other = RnsNativeProofResourceBudgetV1::default();
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut original).unwrap();
    let before = original.live_bytes().unwrap();
    let admission = table.admit_evaluation_v1(&original).unwrap();
    assert!(original.live_bytes().unwrap() > before);
    assert!(matches!(
        table.commitment_with_admission_v1(
            &other,
            admission,
            16_384,
            |_| panic!("foreign evaluation must not read digits"),
            &Scalar::from_u64(1)
        ),
        Err(RnsNativeU15MsmErrorV1::Source)
    ));
    assert_eq!(original.live_bytes().unwrap(), before);
    assert_eq!(other.live_bytes().unwrap(), 0);
    let admission = table.admit_evaluation_v1(&original).unwrap();
    drop(table);
    assert!(original.live_bytes().unwrap() > 0);
    drop(admission);
    assert_eq!(original.live_bytes().unwrap(), 0);
}

#[test]
fn u15_pre_admitted_evaluation_has_no_second_charge_before_actual_allocation() {
    let retained = RnsNativeU15MsmTableV1::retained_bytes_v1() as u64;
    let evaluation = (core::mem::size_of::<RnsNativeU15CommitmentV1>()
        + CanonicalU15PublicTableV1::<Suite>::evaluation_scratch_bytes_v1())
        as u64;
    let mut original =
        RnsNativeProofResourceBudgetV1::with_test_workspace_limit_v1(retained + evaluation);
    let mut table = RnsNativeU15MsmTableV1::new_v1(&mut original).unwrap();
    let admission = table.admit_evaluation_v1(&original).unwrap();
    assert_eq!(original.live_bytes().unwrap(), retained + evaluation);
    let peak = original.peak_bytes().unwrap();
    controls::reset_v1();
    controls::fail_digits_v1();
    assert!(matches!(
        table.commitment_with_admission_v1(
            &original,
            admission,
            16_384,
            |_| panic!("actual digit allocation fails before a read"),
            &Scalar::from_u64(1)
        ),
        Err(RnsNativeU15MsmErrorV1::Allocation)
    ));
    assert_eq!(controls::allocations_v1(), 1);
    assert_eq!(original.peak_bytes().unwrap(), peak);
    assert_eq!(original.live_bytes().unwrap(), retained);
}
