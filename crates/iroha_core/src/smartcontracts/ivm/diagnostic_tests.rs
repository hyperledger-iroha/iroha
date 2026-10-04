//! Lazy local-refusal mapping and exact completed semantic diagnostic rendering.

use super::*;
use crate::{
    execution_attempt::{ExecutionAttemptError, expect_completed_rejection},
    test_allocations::allocations_during,
};
use iroha_allocation::AllocationBudget;
use ivm::{
    VMError, VmBudgetSnapshot, VmExecutionContext, VmExecutionDiagnostic, VmSourceLocation,
    VmTrapKind, encoding::wide::encode_ri, instruction::wide::arithmetic,
};

#[test]
fn direct_local_mapper_keeps_original_nested_owner_without_physical_allocation() {
    let original = AllocationBudget::new(128 * 1024 * 1024);
    let vm = ivm::IVM::try_new_with_memory_budget(100, &original).unwrap();
    original.set_limit_bytes(original.reserved_bytes());
    let capacity = original.try_reserve_bytes(1).unwrap_err();
    for source in [
        VMError::AllocationDeferred(capacity),
        VMError::ExecutionDeferred(ivm::error::ExecutionDeferral::TraceOwnerUnavailable),
    ] {
        let expected = source.clone();
        let error = VMError::Metered {
            gas: 31,
            source: Box::new(VMError::Metered {
                gas: 17,
                source: Box::new(source),
            }),
        };
        let mut attempt = None;
        let allocations = allocations_during(|| {
            attempt = Some(map_vm_error_with_context_to_validation(&vm, error));
        });
        assert_eq!(allocations, 0);
        let ExecutionAttemptError::Deferred(owner) = attempt.unwrap() else {
            panic!("a local refusal cannot become a completed validation diagnostic");
        };
        assert_eq!(owner.into_vm_error(), expected);
        assert!(vm.last_diagnostic().is_none());
    }
    drop(vm);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn actual_semantic_trap_retains_exact_error_and_context_display() {
    let mut vm = ivm::IVM::try_new(0).unwrap();
    vm.load_code(&encode_ri(arithmetic::ADDI, 10, 0, 1).to_le_bytes())
        .unwrap();
    let error = vm.run().unwrap_err();
    assert_eq!(error, VMError::OutOfGas);
    let expected = format!("{error} at pc=0x0");
    let outcome = map_vm_error_with_context_to_validation(&vm, error);
    assert_eq!(
        expect_completed_rejection(outcome),
        ValidationFail::NotPermitted(expected)
    );
}

#[test]
fn borrowed_source_display_preserves_all_semantic_context_fields() {
    let diagnostic = VmExecutionDiagnostic {
        trap_kind: VmTrapKind::UnknownSyscall,
        pc: 0x24,
        source: Some(VmSourceLocation {
            function: Some("transfer"),
            path: Some("contracts/wallet.ko"),
            line: Some(12),
            column: Some(4),
        }),
        budget: VmBudgetSnapshot {
            gas_limit: 100,
            gas_remaining: 41,
            gas_used: 59,
            cycles: 7,
            max_cycles: 64,
            stack_limit_bytes: 4096,
            stack_bytes_used: 16,
        },
        context: VmExecutionContext {
            entrypoint_pc: Some(8),
            current_function: Some("transfer"),
            opcode: Some(0x71),
            syscall: Some(0x7fff),
            predecoded_loaded: true,
            predecoded_hit: Some(true),
        },
    };
    let error = VMError::Metered {
        gas: 9,
        source: Box::new(VMError::UnknownSyscall(0x7fff)),
    };
    assert_eq!(
        format_vm_diagnostic(diagnostic, &error),
        format!(
            "{error} at pc=0x24 fn=transfer src=contracts/wallet.ko:12:4 opcode=0x71 syscall=0x7fff"
        ),
    );
}

#[test]
fn declared_rejection_moves_original_backing_without_physical_allocation() {
    let vm = ivm::IVM::try_new(100).unwrap();
    for depth in 0..=2 {
        for presentation in [None, Some("authenticated explanation")] {
            let contract: Box<str> = "LiquidityPolicy".into();
            let mut name = String::with_capacity(80);
            name.push_str("BelowMinimum");
            let mut error_type = String::with_capacity(120);
            error_type.push_str("LiquidityPolicy::LiquidityError");
            let message: Option<Box<str>> = presentation.map(Into::into);
            let contract_backing = contract.as_ptr();
            let name_backing = (name.as_ptr(), name.capacity());
            let type_backing = (error_type.as_ptr(), error_type.capacity());
            let message_backing = message.as_deref().map(str::as_ptr);
            let mut error = VMError::ContractAbort {
                contract,
                name,
                error_type,
                schema_hash: [7; 32],
                code: 18,
                message,
            };
            for gas in 0..depth {
                error = VMError::Metered {
                    gas: gas + 13,
                    source: Box::new(error),
                };
            }
            let mut outcome = None;
            let allocations = allocations_during(|| {
                outcome = Some(map_vm_error_with_context_to_validation(&vm, error));
            });
            assert_eq!(
                allocations, 0,
                "depth={depth}, presentation={presentation:?}"
            );
            let ValidationFail::ContractRejected(rejection) =
                expect_completed_rejection(outcome.unwrap())
            else {
                panic!("declared rejection must retain its authenticated structure");
            };
            assert_eq!(rejection.contract.as_ptr(), contract_backing);
            assert_eq!(
                (rejection.name.as_ptr(), rejection.name.capacity()),
                name_backing
            );
            assert_eq!(
                (
                    rejection.error_type.as_ptr(),
                    rejection.error_type.capacity()
                ),
                type_backing,
            );
            assert_eq!(
                rejection.message.as_deref().map(str::as_ptr),
                message_backing
            );
            assert_eq!(&*rejection.contract, "LiquidityPolicy");
            assert_eq!(rejection.name, "BelowMinimum");
            assert_eq!(rejection.error_type, "LiquidityPolicy::LiquidityError");
            assert_eq!(rejection.schema_hash, [7; 32]);
            assert_eq!(rejection.code, 18);
            assert_eq!(rejection.message.as_deref(), presentation);
        }
    }
}

#[test]
fn metered_non_declared_mapper_retains_both_display_routes() {
    for with_context in [false, true] {
        let mut vm = ivm::IVM::try_new(0).unwrap();
        let source = if with_context {
            vm.load_code(&encode_ri(arithmetic::ADDI, 10, 0, 1).to_le_bytes())
                .unwrap();
            vm.run().unwrap_err()
        } else {
            VMError::OutOfGas
        };
        let error = VMError::Metered {
            gas: 31,
            source: Box::new(VMError::Metered {
                gas: 17,
                source: Box::new(source),
            }),
        };
        let expected = if with_context {
            format!("{error} at pc=0x0")
        } else {
            error.to_string()
        };
        assert_eq!(
            expect_completed_rejection(map_vm_error_with_context_to_validation(&vm, error)),
            ValidationFail::NotPermitted(expected),
        );
    }
}
