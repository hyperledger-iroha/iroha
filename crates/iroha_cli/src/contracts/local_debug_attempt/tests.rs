//! Local refusal identity and ordinary contract-debug outcome regressions.

use ivm::error::VmErrorRef;
use ivm_abi::error::{VmDeferral, VmTrap};
use std::any::Any;

use iroha_core::smartcontracts::ivm::{
    cache::PreparedContractCache, return_value::EntrypointReturnDecodeError,
};
use ivm::{
    TraceMode,
    encoding::wide::{encode_halt, encode_scall},
    error::ExecutionDeferral,
};

use super::*;

fn assert_debug_error(actual: &VMError, expected: &VMError) {
    assert_eq!(actual.metered_gas(), expected.metered_gas());
    match (actual.as_unmetered(), expected.as_unmetered()) {
        (
            VmErrorRef::Deferred(VmDeferral::AllocationDeferred(a)),
            VmErrorRef::Deferred(VmDeferral::AllocationDeferred(e)),
        ) => assert_eq!(a, e),
        (
            VmErrorRef::Deferred(VmDeferral::ExecutionDeferred(a)),
            VmErrorRef::Deferred(VmDeferral::ExecutionDeferred(e)),
        ) => assert_eq!(a, e),
        (
            VmErrorRef::Trap(VmTrap::UnknownSyscall(a)),
            VmErrorRef::Trap(VmTrap::UnknownSyscall(e)),
        ) => assert_eq!(a, e),
        (VmErrorRef::Trap(VmTrap::OutOfGas), VmErrorRef::Trap(VmTrap::OutOfGas))
        | (
            VmErrorRef::Trap(VmTrap::PrivacyViolation),
            VmErrorRef::Trap(VmTrap::PrivacyViolation),
        ) => {}
        (actual, expected) => {
            panic!("debug fixture error differs: {actual:?}; expected {expected:?}")
        }
    }
}

fn vm() -> IVM {
    let mut vm = IVM::try_new(100).unwrap();
    let words = [encode_scall(ivm::syscalls::SYSCALL_ABORT), encode_halt()];
    let code: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    vm.load_code(&code).unwrap();
    vm.set_zk_mode(false).unwrap();
    vm
}

struct Host {
    error: Option<VMError>,
    calls: usize,
}
impl IVMHost for Host {
    fn prepare_syscall(&self, _: u32, _: ivm::host::SyscallQuote<'_>) -> Result<u64, VMError> {
        Ok(0)
    }
    fn syscall(&mut self, _: u32, _: &mut dyn ivm::host::SyscallContext) -> Result<u64, VMError> {
        self.calls += 1;
        self.error.take().map_or(Ok(0), Err)
    }
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[test]
fn trace_owner_deferral_cannot_reach_completed_report() {
    let mut vm = vm();
    vm.set_trace_mode(TraceMode::DeltaRegisters);
    let mut host = Host {
        error: Some(VMError::Deferred(VmDeferral::ExecutionDeferred(
            ExecutionDeferral::TraceOwnerUnavailable,
        ))),
        calls: 0,
    };
    let mut report_built = false;
    let report = (|| -> eyre::Result<()> {
        let _outcome = run(&mut vm, &mut host)?;
        report_built = true;
        Ok(())
    })()
    .unwrap_err();
    assert_eq!(host.calls, 1);
    assert!(!report_built);
    assert!(match (
        &report.downcast_ref::<VMError>(),
        &Some(&VMError::Deferred(VmDeferral::ExecutionDeferred(
            ExecutionDeferral::TraceOwnerUnavailable
        )))
    ) {
        (
            Some(VMError::Deferred(VmDeferral::ExecutionDeferred(actual_field_0))),
            Some(VMError::Deferred(VmDeferral::ExecutionDeferred(expected_field_0))),
        ) => actual_field_0 == expected_field_0,
        _ => false,
    });
}

#[test]
fn nested_metered_local_errors_retain_their_original_type_without_gas() {
    for reason in [
        ExecutionDeferral::LocalInvariantViolation,
        ExecutionDeferral::TraceOwnerUnavailable,
        ExecutionDeferral::ActiveMemoryCapacity,
        ExecutionDeferral::AllocationUnavailable,
        ExecutionDeferral::VerifierArtifactsUnavailable,
        ExecutionDeferral::CanonicalHistoryUnavailable,
        ExecutionDeferral::CanonicalHistoryCapacity,
    ] {
        let error = VMError::metered(
            31,
            VMError::metered(17, VMError::Deferred(VmDeferral::ExecutionDeferred(reason))),
        );

        let report = completed(Err(error)).unwrap_err();
        let retained = report.downcast_ref::<VMError>().unwrap();
        assert!(
            matches!(retained, VMError::Deferred(VmDeferral::ExecutionDeferred(actual)) if *actual == reason)
        );
        assert_eq!(retained.execution_deferral(), Some(reason));
        assert_eq!(retained.metered_gas(), None);
    }
}

#[test]
fn allocation_refusal_retains_the_original_pool_release_observation() {
    let cache = iroha_core::smartcontracts::ivm::cache::PreparedContractCache::with_capacity(0);
    let original = cache.execution_budget();
    let baseline = original.reserved_bytes();
    let retained = original
        .try_reserve_bytes(original.limit_bytes() - baseline)
        .unwrap();
    let refusal = original.try_reserve_bytes(1).unwrap_err();
    let expected = VMError::Deferred(VmDeferral::AllocationDeferred(refusal.clone()));
    let mut host = Host {
        error: Some(VMError::Deferred(VmDeferral::AllocationDeferred(refusal))),
        calls: 0,
    };
    let mut vm = vm();
    let report = run(&mut vm, &mut host).unwrap_err();
    assert_eq!(host.calls, 1);
    assert_debug_error(report.downcast_ref::<VMError>().unwrap(), &expected);
    assert_eq!(original.reserved_bytes(), original.limit_bytes());
    drop(retained);
    assert_eq!(original.reserved_bytes(), baseline);
    assert_debug_error(report.downcast_ref::<VMError>().unwrap(), &expected);
}

#[test]
fn ordinary_success_and_semantic_faults_remain_completed_outcomes() {
    for fixture in 0..3 {
        let error = match fixture {
            0 => None,
            1 => Some(VMError::Trap(VmTrap::OutOfGas)),
            2 => Some(VMError::Trap(VmTrap::PrivacyViolation)),
            _ => unreachable!(),
        };
        let mut vm = vm();
        let mut host = Host { error, calls: 0 };
        assert!(matches!(
            (fixture, run(&mut vm, &mut host).unwrap()),
            (0, Ok(()))
                | (1, Err(VMError::Trap(VmTrap::OutOfGas)))
                | (2, Err(VMError::Trap(VmTrap::PrivacyViolation)))
        ));
        assert_eq!(host.calls, 1);
    }
    let semantic = VMError::metered(13, VMError::Trap(VmTrap::OutOfGas));
    assert!(matches!(completed(Err(semantic)).unwrap(),
        Err(VMError::Trap(VmTrap::Metered { gas: 13, trap })) if matches!(*trap, VmTrap::OutOfGas)));
}

fn completed_return() -> (IVM, PreparedContractCache, EntrypointValueTypeV1) {
    let cache = PreparedContractCache::with_capacity(0);
    let mut vm = IVM::try_new_with_memory_budget(100_000, cache.execution_budget()).unwrap();
    let program = kotodama_lang::compiler::Compiler::new()
        .compile_source(
            "seiyaku DebugReturnAttempt { view fn main() authorize(anyone) -> bool { true } }",
        )
        .unwrap();
    vm.load_program_for_profile(
        &program,
        ivm::IvmProfileId::of_policy(ivm::SyscallPolicy::AbiV1),
    )
    .unwrap();
    vm.select_entrypoint("main").unwrap();
    let schema = vm
        .contract_interface()
        .unwrap()
        .entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .unwrap()
        .return_schema
        .clone()
        .unwrap();
    vm.run().unwrap();
    (vm, cache, schema)
}

#[test]
fn actual_return_refusal_keeps_typed_original_owner_and_retries() {
    let (vm, cache, schema) = completed_return();
    let original = cache.execution_budget();
    let original_limit = original.limit_bytes();
    assert_eq!(
        decode_return(&vm, &schema, "return value").unwrap(),
        norito::json::Value::Bool(true)
    );
    let gas = vm.remaining_gas();
    vm.memory.clear_tracking();
    original.set_limit_bytes(original.reserved_bytes());
    let expected = (0..65_536)
        .find_map(|_| match vm.memory.load_u64(vm.register(10)) {
            Ok(_) => None,
            Err(error @ VMError::Deferred(VmDeferral::AllocationDeferred(_))) => Some(error),
            Err(error) => panic!("unexpected tracked return-read failure: {error:?}"),
        })
        .expect("tiny fixture exhausts its original read backing");
    let retained = original.reserved_bytes();
    for context in [
        "failed to decode contract debug view return value",
        "failed to decode contract debug call return value",
    ] {
        let report = decode_return(&vm, &schema, context).unwrap_err();
        let Some(EntrypointReturnDecodeError::ExecutionDeferred { word_index, reason }) =
            report.downcast_ref::<EntrypointReturnDecodeError>()
        else {
            panic!("return refusal must retain its typed owner");
        };
        assert_eq!(*word_index, 0);
        assert_debug_error(reason, &expected);
        assert_eq!(reason.metered_gas(), None);
        assert_eq!(report.to_string(), context);
        assert_eq!(vm.remaining_gas(), gas);
        assert_eq!(vm.call_result_word_count().unwrap(), 1);
        assert_eq!(original.reserved_bytes(), retained);
    }
    original.set_limit_bytes(original_limit);
    assert_eq!(
        decode_return(&vm, &schema, "return value").unwrap(),
        norito::json::Value::Bool(true)
    );
    assert_eq!(vm.remaining_gas(), gas);
    drop(vm);
    assert_eq!(original.reserved_bytes(), 0);
}

#[test]
fn actual_semantic_return_error_keeps_type_and_full_display_chain() {
    let (mut vm, cache, schema) = completed_return();
    vm.store_u64(vm.register(10), 2).unwrap();
    let gas = vm.remaining_gas();
    let context = "failed to decode contract debug call return value";
    let report = decode_return(&vm, &schema, context).unwrap_err();
    let typed = report
        .downcast_ref::<EntrypointReturnDecodeError>()
        .unwrap();
    assert!(matches!(
        typed,
        EntrypointReturnDecodeError::NonCanonicalBit {
            word_index: 0,
            role: "bool",
            value: 2,
        }
    ));
    assert_eq!(format!("{report:#}"), format!("{context}: {typed}"));
    assert_eq!(vm.remaining_gas(), gas);
    drop(vm);
    assert_eq!(cache.execution_budget().reserved_bytes(), 0);
}

#[test]
fn borrowed_trap_rendering_uses_exact_original_error_and_success_has_no_trap() {
    let mut vm = vm();
    let source_hash = iroha_crypto::Hash::prehashed(vm.code_hash());
    let original = VMError::Trap(VmTrap::UnknownSyscall(0x7fff));
    let mut host = Host {
        error: Some(original),
        calls: 0,
    };
    let error = run(&mut vm, &mut host).unwrap().unwrap_err();
    assert!(matches!(
        &error,
        VMError::Trap(VmTrap::UnknownSyscall(0x7fff))
    ));
    assert_eq!(host.calls, 1);
    assert!(
        host.error.is_none(),
        "the host transferred its original error"
    );
    let diagnostic =
        super::super::map_local_vm_diagnostic(vm.last_diagnostic().unwrap(), &error, &source_hash);
    assert_eq!(diagnostic.message, error.to_string());
    assert_eq!(diagnostic.syscall, Some(0x7fff));
    let (vm, _, _) = completed_return();
    assert!(
        vm.last_diagnostic().is_none(),
        "post-success return errors have no VM trap"
    );
}
