//! Aggregate content, pre-analysis admission and unwind cleanup.

use super::*;
use crate::{analysis::SyscallUsage, encoding::wide as enc};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

fn decoded(words: &[u32]) -> Vec<DecodedOp> {
    words
        .iter()
        .enumerate()
        .map(|(index, inst)| DecodedOp {
            pc: index as u64 * 4,
            inst: *inst,
        })
        .collect()
}

#[test]
fn exact_funded_histogram_preserves_register_memory_and_extended_syscall_analysis() {
    let ops = decoded(&[
        enc::encode_rr(wide::arithmetic::ADD, 4, 2, 3),
        enc::encode_load(wide::memory::LOAD64, 5, 6, 0),
        enc::encode_store(wide::memory::STORE64, 7, 5, 0),
        enc::encode_sys(wide::system::SCALL, 17),
        enc::encode_syscallx(0x00ff_ffff),
        enc::encode_sys(wide::system::SCALL, 17),
        enc::encode_halt(),
    ]);
    let budget = AllocationBudget::new(4096);
    let funded = analyze(
        ProgramMetadata::default(),
        || ops.iter().copied(),
        Some(&budget),
    )
    .unwrap();
    let local = analyze(ProgramMetadata::default(), || ops.iter().copied(), None).unwrap();
    assert_eq!(funded.registers, local.registers);
    assert_eq!(funded.memory, local.memory);
    assert_eq!(funded.syscalls, local.syscalls);
    assert_eq!(funded.instruction_count, 7);
    assert_eq!((funded.memory.load64, funded.memory.store64), (1, 1));
    for register in [2, 3, 5, 6, 7] {
        assert_eq!(funded.registers.reads[register], 1);
    }
    assert_eq!(
        (funded.registers.writes[4], funded.registers.writes[5]),
        (1, 1)
    );
    assert_eq!(
        &*funded.syscalls,
        &[
            SyscallUsage {
                number: 17,
                count: 2
            },
            SyscallUsage {
                number: 0x00ff_ffff,
                count: 1
            }
        ]
    );
    let charged = budget.reserved_bytes();
    let borrower = funded.clone();
    drop(funded);
    assert_eq!(budget.reserved_bytes(), charged);
    drop(borrower);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_pool_refuses_before_second_pass_and_reclaims_on_visit_unwind() {
    let ops = decoded(&[enc::encode_syscallx(300), enc::encode_halt()]);
    let budget = AllocationBudget::new(0);
    let passes = Cell::new(0);
    let result = analyze(
        ProgramMetadata::default(),
        || {
            passes.set(passes.get() + 1);
            ops.iter().copied()
        },
        Some(&budget),
    );
    assert!(matches!(result, Err(VMError::AllocationDeferred(_))));
    assert_eq!(
        passes.get(),
        1,
        "only allocation-free counting precedes admission"
    );
    assert_eq!(budget.peak_reserved_bytes(), 0);
    budget.set_limit_bytes(4096);
    passes.set(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = analyze(
                ProgramMetadata::default(),
                || {
                    passes.set(passes.get() + 1);
                    let pass = passes.get();
                    ops.iter().copied().inspect(move |_| {
                        if pass == 2 {
                            panic!("visit interrupted after scratch admission");
                        }
                    })
                },
                Some(&budget),
            );
        }))
        .is_err()
    );
    assert_eq!(budget.peak_reserved_bytes(), std::mem::size_of::<u32>());
    assert_eq!(budget.reserved_bytes(), 0);
}
