//! Original Unit return packets, staged gas and fixed complete-scan adversaries.

use super::super::super::private_dispatch;
use super::*;

fn native() -> (NativeInvocation, AllocationBudget) {
    let artifact = private_dispatch::tests::contract(
        &[ivm::encoding::wide::encode_ri(
            ivm::instruction::wide::arithmetic::ADDI,
            4,
            0,
            -1,
        )],
        64,
        ivm::ivm_mode::ZK,
    );
    let budget = AllocationBudget::new(128 * 1024 * 1024);
    let mut parent = ivm::execution_memory::ExecutionMemoryLease::reserve(
        &budget,
        NativeInvocation::allocation_plan().unwrap(),
    )
    .unwrap();
    let native =
        NativeInvocation::run_unit_root(artifact, "main", 10_000, &mut parent, &budget).unwrap();
    (native, budget)
}
fn scan_residues(returning: &Returning, fixed: &Fixed, cell: &Cell, offset: usize) -> Vec<F> {
    let first = first();
    let schedule = return_copyback::schedules(
        0,
        [46, 47, 20, 21].map(|offset| first + offset),
        first + 100,
    )
    .unwrap()
    .nth(offset)
    .unwrap();
    let mut residues = Vec::new();
    return_copyback::append_residues(
        &mut residues,
        schedule,
        &returning.rows.as_slice()[offset].0,
        fixed.ports(cell),
    );
    residues
}
fn validation_residues(returning: &Returning, fixed: &Fixed) -> Vec<F> {
    let mut residues = Vec::new();
    returning.validation.append_residues(
        &mut residues,
        first(),
        [&fixed.gas[0], &fixed.gas[1]],
        &fixed.memory,
    );
    residues
}

#[test]
fn native_unit_return_uses_every_original_scan_and_typed_slot() {
    let (native, budget) = native();
    let before = budget.reserved_bytes();
    let returning = Returning::new(&native, &budget).unwrap();
    assert_eq!(Returning::BYTES, 9_308_384);
    assert_eq!(budget.reserved_bytes() - before, Returning::BYTES);
    assert_eq!(returning.callable.entry_pc(), &[F::ZERO; 4]);
    assert_eq!(returning.callable.argument_words(), F::ZERO);
    assert_eq!(returning.callable.result_words(), F::ONE);
    let mut seen = [false; return_copyback::CELLS];
    let mut phases = [false; 2];
    let mut gaps = 0;
    returning
        .evaluate(&native, &mut Scratch::new(), |row, residues| {
            assert!(residues.iter().all(|value| *value == F::ZERO), "{row:?}");
            match row {
                Row::Operands => phases[0] = true,
                Row::Validation => phases[1] = true,
                Row::Scan(offset) => seen[offset] = true,
                Row::Gap(_) => gaps += 1,
            }
            Ok::<_, ()>(())
        })
        .unwrap();
    assert!(phases.into_iter().all(|seen| seen));
    assert!(seen.into_iter().all(|seen| seen));
    assert_eq!(gaps, 72);
    for offset in [22, 23, 24] {
        assert_eq!(original(&native, offset).0[packet::ENABLED], F::ONE);
    }
    drop(returning);
    assert_eq!(budget.reserved_bytes(), before);
    drop(native);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn exact_scan_funding_refusal_and_consumer_unwind_preserve_original_owners() {
    let (native, original) = native();
    let before = original.reserved_bytes();
    let short = AllocationBudget::new(Returning::BYTES - 1);
    assert!(matches!(
        Returning::new(&native, &short),
        Err(Error::Allocation(_))
    ));
    assert_eq!(short.reserved_bytes(), 0);
    assert_eq!(original.reserved_bytes(), before);
    let exact = AllocationBudget::new(Returning::BYTES);
    let returning = Returning::new(&native, &exact).unwrap();
    assert_eq!(
        returning.evaluate(&native, &mut Scratch::new(), |row, _| {
            if row == Row::Scan(3) { Err(7) } else { Ok(()) }
        }),
        Err(7)
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = returning.evaluate(&native, &mut Scratch::new(), |row, _| {
                assert_ne!(row, Row::Scan(3), "consumer panic");
                Ok::<_, ()>(())
            });
        }))
        .is_err()
    );
    assert_eq!(exact.reserved_bytes(), Returning::BYTES);
    let mut scratch = Witness([F::ONE; return_copyback::WIDTH]);
    scratch.clear();
    assert!(scratch.0.iter().all(|value| *value == F::ZERO));
    drop(returning);
    assert_eq!(exact.reserved_bytes(), 0);
    assert_eq!(original.reserved_bytes(), before);
    drop(native);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn source_return_refusal_drops_native_and_instruction_owners_without_reclassification() {
    let (native, original) = native();
    let budget = AllocationBudget::new(
        super::super::instructions::Instructions::BYTES + Returning::BYTES - 1,
    );
    assert!(matches!(
        super::super::Source::new(native, &budget),
        Err(super::super::SourceError::Return(Error::Allocation(_)))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn all_scan_workspace_fields_and_original_masks_remain_constrained() {
    let (native, budget) = native();
    let mut returning = Returning::new(&native, &budget).unwrap();
    let fixed = Fixed::new(&native);
    for offset in [0, 1, return_copyback::CELLS - 1] {
        let cell = Cell::new(&native, offset);
        assert!(
            scan_residues(&returning, &fixed, &cell, offset)
                .iter()
                .all(|value| *value == F::ZERO)
        );
        for field in 0..return_copyback::WIDTH {
            let original = returning.rows.as_slice()[offset].0[field];
            returning.rows.as_mut_slice()[offset].0[field] = original.add(F::ONE);
            assert!(
                scan_residues(&returning, &fixed, &cell, offset)
                    .iter()
                    .any(|value| *value != F::ZERO),
                "offset {offset}, field {field}"
            );
            returning.rows.as_mut_slice()[offset].0[field] = original;
        }
        for which in [false, true] {
            for field in 0..packet::WIDTH {
                let mut changed = Cell::new(&native, offset);
                let port = if which {
                    &mut changed.copyback
                } else {
                    &mut changed.child
                };
                port.0[field] = port.0[field].add(F::ONE);
                assert!(
                    scan_residues(&returning, &fixed, &changed, offset)
                        .iter()
                        .any(|value| *value != F::ZERO),
                    "offset {offset}, parent {which}, field {field}"
                );
            }
        }
    }
}

#[test]
fn original_unit_read_and_each_staged_debit_reject_all_single_field_mutations() {
    let (native, budget) = native();
    let returning = Returning::new(&native, &budget).unwrap();
    let fixed = Fixed::new(&native);
    assert!(
        validation_residues(&returning, &fixed)
            .iter()
            .all(|value| *value == F::ZERO)
    );
    for port in 0..3 {
        for field in 0..packet::WIDTH {
            let mut fixed = Fixed::new(&native);
            let changed = if port < 2 {
                &mut fixed.gas[port]
            } else {
                &mut fixed.memory
            };
            changed.0[field] = changed.0[field].add(F::ONE);
            assert!(
                validation_residues(&returning, &fixed)
                    .iter()
                    .any(|value| *value != F::ZERO),
                "port {port}, field {field}"
            );
        }
    }
    // A coherent different tariff is still rejected; native gas is a witness,
    // while NODE/WORD are coefficients of the admitted Unit schema semantics.
    let mut fixed = Fixed::new(&native);
    fixed.gas[1].0[packet::AFTER] = fixed.gas[1].0[packet::AFTER].sub(F::ONE);
    assert!(
        validation_residues(&returning, &fixed)
            .iter()
            .any(|value| *value != F::ZERO)
    );
}

#[test]
fn complete_native_return_uses_bounded_stack() {
    let (native, budget) = native();
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let returning = Returning::new(&native, &budget).unwrap();
            returning
                .evaluate(&native, &mut Scratch::new(), |row, residues| {
                    assert!(residues.iter().all(|value| *value == F::ZERO), "{row:?}");
                    Ok::<_, ()>(())
                })
                .unwrap();
            drop(returning);
            drop(native);
            assert_eq!(budget.reserved_bytes(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
