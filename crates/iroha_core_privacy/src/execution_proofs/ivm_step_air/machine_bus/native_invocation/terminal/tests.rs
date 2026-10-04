//! Original native padding, mandatory suffix, funding and degree adversaries.

use super::super::{Source, SourceError, instructions, returning};
use super::*;
use crate::execution_proofs::ivm_step_air::machine_bus::private_dispatch;
use ivm::{
    PreparedContract, VMError, encoding::wide as enc, execution_memory::ExecutionMemoryLease,
    execution_packets::CaptureError, instruction::wide,
};
use packet::{AFTER, BEFORE, ENABLED};

fn artifact(cycles: u64, additions: usize) -> PreparedContract {
    private_dispatch::tests::contract(
        &vec![enc::encode_ri(wide::arithmetic::ADDI, 4, 4, 1); additions],
        cycles,
        ivm::ivm_mode::ZK,
    )
}
fn capture(
    contract: PreparedContract,
    gas: u64,
    budget: &AllocationBudget,
) -> Result<NativeInvocation, CaptureError> {
    let mut lease =
        ExecutionMemoryLease::reserve(budget, NativeInvocation::allocation_plan().unwrap())
            .unwrap();
    NativeInvocation::run_public_leaf_root(contract, "main", gas, &mut lease, budget)
}
fn native(cycles: u64, additions: usize, gas: u64) -> (NativeInvocation, AllocationBudget) {
    let budget = AllocationBudget::new(128 * 1024 * 1024);
    (
        capture(artifact(cycles, additions), gas, &budget).unwrap(),
        budget,
    )
}
fn plan(native: &NativeInvocation) -> root::Plan {
    root::Plan::derive(
        native.artifact(),
        native.entrypoint_index(),
        native.initial_gas(),
    )
    .unwrap()
}
fn residues(terminal: &Terminal, fixed: &Fixed) -> Vec<F> {
    let mut out = Vec::new();
    append(
        &mut out,
        terminal.cycle_limit,
        &terminal.row.as_slice()[0].0,
        fixed,
    );
    assert_eq!(out.len(), CONSTRAINTS);
    out
}
fn accepts(terminal: &Terminal, fixed: &Fixed) -> bool {
    residues(terminal, fixed).iter().all(|x| *x == F::ZERO)
}
fn put_word(fields: &mut Fields, at: usize, word: u64) {
    for limb in 0..4 {
        fields.0[at + limb] = F((word >> (16 * limb)) & 0xffff);
    }
}

#[test]
fn zero_one_maximum_padding_and_full_step_limit_match_original_native_outputs() {
    assert_eq!(padding_first(), 10409);
    assert_eq!(PACKET_SLOTS - padding_first() - 2, 5973);
    assert_eq!(Terminal::BYTES, 96);
    for (limit, additions, gas) in [
        (4, 0, 25),
        (5, 0, 26),
        (64, 0, 85),
        (64, 60, 85),
        (64, 3, u64::MAX),
    ] {
        let (native, budget) = native(limit, additions, gas);
        let before = budget.reserved_bytes();
        let terminal = Terminal::new(&native, &plan(&native), &budget).unwrap();
        assert_eq!(budget.reserved_bytes(), before + Terminal::BYTES);
        let fixed = Fixed::new(&native);
        assert!(accepts(&terminal, &fixed));
        let retired = additions as u64 + 4;
        let expected_gas = gas - (25 + additions as u64) - (limit - retired);
        let mut final_gas = 0;
        for limb in 0..4 {
            // A derived packet expression, not an expected-output coefficient.
            let value = fixed.validated_gas.0[AFTER + limb]
                .add(fixed.padding_gas.0[AFTER + limb])
                .sub(fixed.padding_gas.0[BEFORE + limb]);
            assert!(value.0 <= 0xffff);
            final_gas |= value.0 << (16 * limb);
        }
        assert_eq!(final_gas, expected_gas);
        // Native summaries are comparison evidence only; no production bank reads them.
        assert_eq!(native.remaining_gas(), final_gas);
        assert_eq!(native.cycles(), limit);
        assert_eq!(native.instructions() as u64, retired);
        assert_eq!(fixed.padding_gas.0[ENABLED], F(u64::from(retired < limit)));
        let mut seen = 0;
        terminal
            .evaluate(&native, &mut Scratch::new(), |row, values| {
                if matches!(row, Row::Unused(_)) {
                    seen += 1;
                }
                assert!(values.iter().all(|x| *x == F::ZERO), "{row:?}");
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(seen, 5973);
        drop(terminal);
        assert_eq!(budget.reserved_bytes(), before);
        drop(native);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn one_short_padding_gas_refuses_without_publishing_original_output() {
    for (limit, additions, gas) in [(4, 0, 24), (5, 0, 25), (64, 0, 84), (64, 60, 84)] {
        let budget = AllocationBudget::new(128 * 1024 * 1024);
        let occupied = budget.try_reserve_bytes(19).unwrap();
        assert!(matches!(
            capture(artifact(limit, additions), gas, &budget),
            Err(CaptureError::Execution(VMError::OutOfGas))
        ));
        assert_eq!(budget.reserved_bytes(), 19);
        drop(occupied);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn every_padding_packet_and_private_workspace_field_is_bound_in_both_modes() {
    for limit in [4, 64] {
        let (native, budget) = native(limit, 0, 1000);
        let mut terminal = Terminal::new(&native, &plan(&native), &budget).unwrap();
        let fixed = Fixed::new(&native);
        assert!(accepts(&terminal, &fixed));
        for field in 0..WIDTH {
            let original = terminal.row.as_slice()[0].0[field];
            terminal.row.as_mut_slice()[0].0[field] = original.add(F::ONE);
            assert!(
                !accepts(&terminal, &fixed),
                "limit {limit}, witness {field}"
            );
            terminal.row.as_mut_slice()[0].0[field] = original;
        }
        for cycles in [false, true] {
            for field in 0..packet::WIDTH {
                let mut changed = Fixed::new(&native);
                let target = if cycles {
                    &mut changed.padding_cycles
                } else {
                    &mut changed.padding_gas
                };
                target.0[field] = target.0[field].add(F::ONE);
                assert!(
                    !accepts(&terminal, &changed),
                    "limit {limit}, cycle {cycles}, field {field}"
                );
            }
        }
        for limb in 0..8 {
            let mut changed = Fixed::new(&native);
            changed.retired_cycles.0[AFTER + limb] =
                changed.retired_cycles.0[AFTER + limb].add(F::ONE);
            assert!(!accepts(&terminal, &changed));
        }
        if limit > 4 {
            for limb in 0..4 {
                let mut changed = Fixed::new(&native);
                changed.validated_gas.0[AFTER + limb] =
                    changed.validated_gas.0[AFTER + limb].add(F::ONE);
                assert!(!accepts(&terminal, &changed));
            }
        }
    }
}

#[test]
fn erased_invented_wrong_tariff_and_underflow_padding_fail_equations() {
    let (native, budget) = native(5, 0, 1000);
    let mut terminal = Terminal::new(&native, &plan(&native), &budget).unwrap();
    let mut fixed = Fixed::new(&native);
    fixed.padding_gas.clear();
    fixed.padding_cycles.clear();
    assert!(
        !accepts(&terminal, &fixed),
        "required padding cannot be erased"
    );
    let mut fixed = Fixed::new(&native);
    // Even freshly recomputed private work cannot choose another NOP tariff.
    let gas = word(&fixed.padding_gas, AFTER);
    put_word(&mut fixed.padding_gas, AFTER, gas - 1);
    fill(&mut terminal.row.as_mut_slice()[0].0, 5, &fixed);
    assert!(!accepts(&terminal, &fixed));
    let mut fixed = Fixed::new(&native);
    put_word(&mut fixed.padding_cycles, AFTER, 4);
    assert!(!accepts(&terminal, &fixed));
    let mut fixed = Fixed::new(&native);
    put_word(&mut fixed.validated_gas, AFTER, 0);
    put_word(&mut fixed.padding_gas, BEFORE, 0);
    put_word(&mut fixed.padding_gas, AFTER, u64::MAX);
    fill(&mut terminal.row.as_mut_slice()[0].0, 5, &fixed);
    assert_eq!(terminal.row.as_slice()[0].0[BORROWS + 3], F::ONE);
    assert!(
        !accepts(&terminal, &fixed),
        "wrapped gas subtraction is forbidden"
    );
    // Reuse fully formed headers but make a coherent zero-size padding write.
    let mut fixed = Fixed::new(&native);
    let gas = word(&fixed.validated_gas, AFTER);
    put_word(&mut fixed.padding_gas, AFTER, gas);
    put_word(&mut fixed.padding_cycles, AFTER, 4);
    terminal.cycle_limit = 4;
    fill(&mut terminal.row.as_mut_slice()[0].0, 4, &fixed);
    assert!(
        !accepts(&terminal, &fixed),
        "zero delta requires absent packets"
    );
}

#[test]
fn every_unused_suffix_slot_rejects_an_invented_state_access() {
    for clock in padding_first() + 2..PACKET_SLOTS {
        for field in 0..packet::WIDTH {
            let mut fields = [F::ZERO; packet::WIDTH];
            fields[field] = F::ONE;
            let mut residues = Vec::new();
            append_unused(&mut residues, &fields);
            assert_eq!(residues.len(), packet::WIDTH);
            assert!(
                residues.iter().any(|x| *x != F::ZERO),
                "clock {clock}, field {field}"
            );
        }
    }
}

#[test]
fn exact_terminal_funding_source_refusal_and_consumer_unwind_release_original_pool() {
    let (native, budget) = native(64, 0, 1000);
    let before = budget.reserved_bytes();
    budget.set_limit_bytes(before + Terminal::BYTES - 1);
    assert!(Terminal::new(&native, &plan(&native), &budget).is_err());
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(before + Terminal::BYTES);
    let terminal = Terminal::new(&native, &plan(&native), &budget).unwrap();
    let mut scratch = Scratch::new();
    assert_eq!(
        terminal.evaluate(&native, &mut scratch, |_, _| Err(7)),
        Err(7)
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = terminal.evaluate(&native, &mut scratch, |_, _| -> Result<(), ()> {
                panic!("terminal consumer");
            });
        }))
        .is_err()
    );
    drop(terminal);
    assert_eq!(budget.reserved_bytes(), before);
    // Refusal at the final owner releases native, instruction and return backing.
    budget.set_limit_bytes(
        before + instructions::Instructions::BYTES + returning::Returning::BYTES + Terminal::BYTES
            - 1,
    );
    assert!(matches!(
        Source::new(native, &budget),
        Err(SourceError::Terminal(_))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    let mut witness = Witness([F::ONE; WIDTH]);
    witness.clear();
    assert!(witness.0.iter().all(|x| *x == F::ZERO));
}

#[test]
fn terminal_stream_uses_bounded_stack_and_degree_two_for_all_original_fields() {
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    let degree = measured_maximum_affine_degree_v1(
        [0x7d; 32],
        [WIDTH + 4 * packet::WIDTH, 0, 0, 0, 0],
        8,
        2,
        |row, _, _, _, _| {
            let port = |i| {
                Fields(
                    row[WIDTH + i * packet::WIDTH..WIDTH + (i + 1) * packet::WIDTH]
                        .try_into()
                        .unwrap(),
                )
            };
            let fixed = Fixed {
                retired_cycles: port(0),
                validated_gas: port(1),
                padding_gas: port(2),
                padding_cycles: port(3),
            };
            let mut residues = Vec::new();
            append(&mut residues, 64, row[..WIDTH].try_into().unwrap(), &fixed);
            Ok::<_, core::convert::Infallible>(residues)
        },
    );
    assert_eq!(degree, 2);
    let (native, budget) = native(64, 0, 1000);
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let terminal = Terminal::new(&native, &plan(&native), &budget).unwrap();
            terminal
                .evaluate(&native, &mut Scratch::new(), |_, residues| {
                    assert!(residues.iter().all(|x| *x == F::ZERO));
                    Ok::<_, ()>(())
                })
                .unwrap();
            drop(terminal);
            drop(native);
            assert_eq!(budget.reserved_bytes(), 0);
        })
        .unwrap()
        .join()
        .unwrap();
}
