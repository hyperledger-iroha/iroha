//! Retained diagnostic trace and execution behavior controls.
use ivm_abi::error::{VmDeferral, VmTrap};
use std::any::Any;

use crate::common;
use common::assemble_zk;
use ivm::{IVM, IVMHost, TraceMode, VMError, encoding, instruction};
#[test]
fn test_register_events_logged() {
    // Program: store 0x11 at heap, load it back
    let store = encoding::wide::encode_store(instruction::wide::memory::STORE64, 1, 2, 0);
    let load = encoding::wide::encode_load(instruction::wide::memory::LOAD64, 3, 1, 0);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&store.to_le_bytes());
    bytes.extend_from_slice(&load.to_le_bytes());
    bytes.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let prog = assemble_zk(&bytes, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.set_register(1, ivm::Memory::HEAP_START);
    vm.set_register(2, 0x11);
    vm.load_program(&prog).unwrap();
    vm.set_zk_trace_enabled(true);
    vm.run().unwrap();
    let snapshot = common::diagnostic_snapshot(&vm);
    assert_ne!(snapshot.register_event_count(), 0);
    for event in snapshot.register_events() {
        assert!(!event.path.is_empty());
        assert_ne!(*event.root, [0u8; 32]);
    }
    // Step log should match cycle count
    let steps = vm.step_log();
    assert_eq!(steps.len() as u64, vm.get_cycle_count());
}

#[test]
fn retained_snapshot_is_independent_of_callback_register_reads() {
    struct SnapshotHost {
        invocation_snapshot: ivm::zk::DiagnosticTraceSnapshot,
        calls: usize,
    }
    impl IVMHost for SnapshotHost {
        fn prepare_syscall(&self, _: u32, vm: ivm::host::SyscallQuote<'_>) -> Result<u64, VMError> {
            let _ = vm.public_register(1)?;
            let snapshot = &self.invocation_snapshot;
            let content_digest = |snapshot: &ivm::zk::DiagnosticTraceSnapshot| -> [u8; 32] {
                use sha2::Digest;
                let mut digest = sha2::Sha256::new();
                digest.update(snapshot.register_event_count().to_le_bytes());
                for event in snapshot.register_events() {
                    digest.update([u8::from(event.written)]);
                    digest.update(event.index.to_le_bytes());
                    digest.update(event.value.to_le_bytes());
                    digest.update([u8::from(event.tag)]);
                    digest.update(event.path.len().to_le_bytes());
                    for sibling in event.path {
                        digest.update(sibling);
                    }
                    digest.update(event.root);
                }
                digest.finalize().into()
            };
            let expected = content_digest(snapshot);
            for index in 1..=64 {
                let _ = vm.public_register((index % 8) + 1)?;
            }
            assert!(
                content_digest(snapshot) == expected,
                "register-log snapshots must not alias the active logger"
            );
            Ok(0)
        }
        fn syscall(
            &mut self,
            _: u32,
            context: &mut dyn ivm::host::SyscallContext,
        ) -> Result<u64, VMError> {
            self.calls += 1;
            context.write_register(6, 0xC0FFEE)?;
            Ok(0)
        }
        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }
    let words = [
        encoding::wide::encode_scall(ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS),
        encoding::wide::encode_halt(),
    ];
    let code: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&assemble_zk(&code, 8)).unwrap();
    vm.set_zk_trace_enabled(true);
    let mut host = SnapshotHost {
        invocation_snapshot: common::diagnostic_snapshot(&vm),
        calls: 0,
    };
    vm.run_with_host(&mut host).unwrap();
    assert_eq!(host.calls, 1);
    assert_eq!(vm.register(6), 0xC0FFEE);
    assert_eq!(host.invocation_snapshot.register_event_count(), 0);
    assert!(
        common::diagnostic_snapshot(&vm)
            .register_events()
            .any(|e| e.written && e.index == 6 && e.value == 0xC0FFEE)
    );
}

#[test]
fn original_host_allocation_refusal_survives_both_metering_modes() {
    struct RefusingHost {
        refusal: Option<ivm::error::AllocationRefusal>,
    }
    impl IVMHost for RefusingHost {
        fn prepare_syscall(&self, _: u32, _: ivm::host::SyscallQuote<'_>) -> Result<u64, VMError> {
            Ok(0)
        }
        fn syscall(
            &mut self,
            _: u32,
            vm: &mut dyn ivm::host::SyscallContext,
        ) -> Result<u64, VMError> {
            vm.write_register(6, 0)?;
            Err(VMError::Deferred(VmDeferral::AllocationDeferred(
                self.refusal.take().expect("first host call"),
            )))
        }
        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }
    for syscall in [
        ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS,
        ivm::syscalls::SYSCALL_QUANTITY_ADD,
    ] {
        let code = [
            encoding::wide::encode_scall(syscall),
            encoding::wide::encode_halt(),
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&assemble_zk(&code, 8)).unwrap();
        vm.set_zk_trace_enabled(true);
        let pool = iroha_allocation::AllocationBudget::new(8);
        let _active = pool.try_reserve_bytes(8).unwrap();
        let original = pool.try_reserve_bytes(1).unwrap_err();
        let mut host = RefusingHost {
            refusal: Some(original.clone()),
        };
        let error = vm.run_with_host(&mut host).unwrap_err();
        assert!(host.refusal.is_none());
        assert!(
            matches!(error, VMError::Deferred(VmDeferral::AllocationDeferred(ref owner)) if owner == &original),
            "syscall {syscall}: {error:?}"
        );
        assert!(vm.last_diagnostic().is_none());
        assert_eq!(vm.register(6), 0);
        let snapshot = common::diagnostic_snapshot(&vm);
        assert!(
            !snapshot
                .register_events()
                .any(|event| event.index == 6 && event.written)
        );
    }
}

#[test]
fn panicking_host_callbacks_scrub_detached_proof_state() {
    #[derive(Clone, Copy, Debug)]
    enum PanicPhase {
        AllowsSyscall,
        Prepare,
        Syscall,
    }

    struct PanickingHost(PanicPhase);

    impl IVMHost for PanickingHost {
        fn allows_syscall(&self, policy: ivm::SyscallPolicy, number: u32) -> bool {
            if matches!(self.0, PanicPhase::AllowsSyscall) {
                panic!("intentional allows_syscall panic");
            }
            ivm::syscalls::is_syscall_allowed(policy, number)
        }

        fn prepare_syscall(
            &self,
            _number: u32,
            vm: ivm::host::SyscallQuote<'_>,
        ) -> Result<u64, VMError> {
            if matches!(self.0, PanicPhase::Prepare) {
                assert_eq!(vm.public_register(5)?, 1);
                panic!("intentional prepare panic");
            }
            Ok(0)
        }

        fn syscall(
            &mut self,
            _number: u32,
            vm: &mut dyn ivm::host::SyscallContext,
        ) -> Result<u64, VMError> {
            assert_eq!(vm.public_register(5)?, 1);
            panic!("intentional syscall panic");
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    let syscall = ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS;
    let mut code = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 5, 0, 1)
        .to_le_bytes()
        .to_vec();
    code.extend_from_slice(&encoding::wide::encode_scall(syscall).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let program = assemble_zk(&code, 8);

    for phase in [
        PanicPhase::AllowsSyscall,
        PanicPhase::Prepare,
        PanicPhase::Syscall,
    ] {
        let mut vm = IVM::new(u64::MAX);
        vm.load_program(&program).expect("load ZK syscall program");
        vm.set_zk_trace_enabled(true);
        vm.set_trace_mode(TraceMode::DeltaRegisters);
        let mut host = PanickingHost(phase);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = vm.run_with_host(&mut host);
        }));
        assert!(panic.is_err(), "{phase:?} panic must propagate");
        assert!(
            common::diagnostic_snapshot(&vm).register_event_count() == 0,
            "{phase:?}"
        );
        assert!(
            common::diagnostic_snapshot(&vm).states().is_empty(),
            "{phase:?}"
        );
        assert!(vm.step_log().is_empty(), "{phase:?}");
        assert!(vm.memory_log().is_empty(), "{phase:?}");
        assert!(vm.trace_pcs().is_empty(), "{phase:?}");
        assert!(vm.delta_register_trace().is_empty(), "{phase:?}");

        // A stale detachment marker must not suppress out-of-run cleanup.
        vm.set_zk_trace_enabled(false);
        assert!(
            common::diagnostic_snapshot(&vm).register_event_count() == 0,
            "{phase:?}"
        );
    }
}

#[test]
fn nested_untraced_vm_masks_the_outer_register_logger() {
    struct NestedHost {
        policy_probe_vm: std::cell::RefCell<IVM>,
    }

    impl IVMHost for NestedHost {
        fn prepare_syscall(
            &self,
            _number: u32,
            _vm: ivm::host::SyscallQuote<'_>,
        ) -> Result<u64, VMError> {
            Ok(0)
        }

        fn syscall(
            &mut self,
            _number: u32,
            vm: &mut dyn ivm::host::SyscallContext,
        ) -> Result<u64, VMError> {
            let sentinel = 0x5A_u64;
            let foreign_secret = 0xA11C_E5EC_12E7_u64;
            let code = encoding::wide::encode_ri(
                instruction::wide::arithmetic::ADDI,
                7,
                0,
                sentinel as i16,
            )
            .to_le_bytes()
            .to_vec();
            let mut nested = IVM::new(u64::MAX);
            nested.load_code(&code)?;
            nested.set_register(8, foreign_secret);
            assert!(matches!(
                nested.run(),
                Err(VMError::Trap(VmTrap::MissingHalt))
            ));
            vm.write_register(9, 0xC011_AB1E)?;
            Ok(0)
        }

        fn allows_syscall(&self, policy: ivm::SyscallPolicy, number: u32) -> bool {
            self.policy_probe_vm
                .borrow_mut()
                .set_register(10, 0x00A1_1005_CA11_AB1E);
            ivm::syscalls::is_syscall_allowed(policy, number)
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    let syscall = ivm::syscalls::SYSCALL_SYSVAR_BLOCK_TIME_MS;
    let mut code = encoding::wide::encode_ri(instruction::wide::arithmetic::ADDI, 5, 0, 1)
        .to_le_bytes()
        .to_vec();
    code.extend_from_slice(&encoding::wide::encode_scall(syscall).to_le_bytes());
    code.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    let program = assemble_zk(&code, 8);
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("load outer ZK program");
    vm.set_zk_trace_enabled(true);
    let mut host = NestedHost {
        policy_probe_vm: std::cell::RefCell::new(IVM::new(u64::MAX)),
    };
    vm.run_with_host(&mut host)
        .expect("nested untraced execution succeeds");

    assert!(
        !common::diagnostic_snapshot(&vm)
            .register_events()
            .any(|event| event.written && event.index == 7 && event.value == 0x5A)
    );
    assert!(
        !common::diagnostic_snapshot(&vm)
            .register_events()
            .any(|event| event.written
                && event.index == 10
                && event.value == 0x00A1_1005_CA11_AB1E)
    );
    assert!(
        !common::diagnostic_snapshot(&vm)
            .register_events()
            .any(|event| event.written && event.index == 8 && event.value == 0xA11C_E5EC_12E7)
    );
    assert!(
        common::diagnostic_snapshot(&vm)
            .register_events()
            .any(|event| event.written && event.index == 9 && event.value == 0xC011_AB1E)
    );
    assert_eq!(
        vm.step_log().len() as u64,
        vm.get_cycle_count(),
        "the active invocation retains every completed step"
    );

    vm.set_zk_trace_enabled(false);
    assert!(matches!(vm.run(), Err(VMError::Trap(VmTrap::MissingHalt))));
    assert!(
        common::diagnostic_snapshot(&vm).register_event_count() == 0,
        "an untraced invocation must clear the preceding register log"
    );
    assert!(common::diagnostic_snapshot(&vm).states().is_empty());
    assert!(vm.step_log().is_empty());
}
