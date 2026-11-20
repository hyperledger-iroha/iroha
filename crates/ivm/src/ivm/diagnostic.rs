//! Allocation-free semantic trap snapshots borrowing their original metadata owner.
//!
//! The returned VMError remains the only error owner. Rendering at Core/CLI/HTTP
//! boundaries is separate output work and is not funded by this snapshot.

use super::IVM;
use crate::error::{
    VMError, VmBudgetSnapshot, VmExecutionContext, VmExecutionDiagnostic, VmSourceLocation,
    VmTrapKind,
};
use crate::error::{VmDeferral, VmErrorRef, VmTrap};
use iroha_data_model::executor::fault::{
    IvmFaultPositionV1, IvmFaultSiteV1, IvmFaultV1, IvmInvocationSelectorV1,
};

use iroha_data_model::ivm_c10::IvmFaultC10V1;

/// Inline trap-time values. The source index resolves only against this VM's debug owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TrapSnapshot {
    pub(super) fault: Option<IvmFaultV1>,
    pub(super) c10_fault: Option<IvmFaultC10V1>,
    pub(super) propagated: bool,
    pub(super) trap_kind: VmTrapKind,
    pub(super) pc: u64,
    pub(super) source_index: Option<usize>,
    pub(super) budget: VmBudgetSnapshot,
    pub(super) entrypoint_pc: Option<u64>,
    pub(super) opcode: Option<u8>,
    pub(super) syscall: Option<u32>,
    pub(super) predecoded_loaded: bool,
    pub(super) predecoded_hit: Option<bool>,
}

impl IVM {
    /// Borrow the last semantic trap's context without formatting or allocating.
    ///
    /// Success, local refusal and a new run/load/reset clear this context. Supply
    /// the original returned error separately when rendering a semantic failure.
    pub fn last_diagnostic(&self) -> Option<VmExecutionDiagnostic<'_>> {
        let trap = self.last_diagnostic?;
        let source = trap.source_index.and_then(|index| {
            let entry = self.contract_debug.as_ref()?.source_map.get(index)?;
            Some(VmSourceLocation {
                function: Some(entry.function_name.as_str()),
                path: entry.source.source_path.as_deref(),
                line: Some(entry.source.line),
                column: Some(entry.source.column),
            })
        });
        Some(VmExecutionDiagnostic {
            fault: trap.fault,
            c10_fault: trap.c10_fault,
            trap_kind: trap.trap_kind,
            pc: trap.pc,
            source,
            budget: trap.budget,
            context: VmExecutionContext {
                entrypoint_pc: trap.entrypoint_pc,
                current_function: source.and_then(|source| source.function),
                opcode: trap.opcode,
                syscall: trap.syscall,
                predecoded_loaded: trap.predecoded_loaded,
                predecoded_hit: trap.predecoded_hit,
            },
        })
    }

    pub(super) fn capture_trap(&mut self, error: &VMError) {
        self.capture_trap_at(
            error,
            IvmFaultPositionV1::Execute {
                pc_offset: self.pc.saturating_sub(self.program_prefix_len),
            },
        );
    }

    pub(super) fn capture_trap_at(&mut self, error: &VMError, position: IvmFaultPositionV1) {
        let inherited = self.last_diagnostic.filter(|snapshot| snapshot.propagated);
        self.last_diagnostic = None;
        if error.execution_deferral().is_some() {
            return;
        }
        // Read diagnostics outside the guest register transcript.
        let _mask = crate::zk::RegLoggerGuard::mask();
        let fault = inherited
            .and_then(|snapshot| snapshot.fault)
            .or_else(|| self.execution_fault(error, position));
        let c10_fault = inherited
            .and_then(|snapshot| snapshot.c10_fault)
            .or_else(|| self.c10_execution_fault(error, position));
        let site = c10_fault
            .map(|fault| fault.site)
            .or_else(|| fault.map(|fault| fault.site));
        let relative_pc = match site {
            Some(site) if site.code_hash.as_ref() != &self.code_hash => None,
            Some(IvmFaultSiteV1 {
                position: IvmFaultPositionV1::Execute { pc_offset },
                ..
            }) => Some(pc_offset),
            Some(_) => None,
            None => self.pc.checked_sub(self.program_prefix_len),
        };
        let source_index = relative_pc.and_then(|pc| {
            self.contract_debug
                .as_ref()?
                .source_map
                .iter()
                .position(|entry| pc >= entry.pc_start && pc < entry.pc_end)
        });
        let stack_top = self.memory.stack_top();
        let stack_bytes_used = stack_top.saturating_sub(self.registers.get(31));
        let predecoded_loaded = self.prepared.is_some();
        self.last_diagnostic = Some(TrapSnapshot {
            fault,
            c10_fault,
            propagated: false,
            trap_kind: Self::classify_trap(error),
            pc: self.pc,
            source_index,
            budget: VmBudgetSnapshot {
                gas_limit: self.gas_limit,
                gas_remaining: self.gas_remaining,
                gas_used: self.gas_limit.saturating_sub(self.gas_remaining),
                cycles: self.cycles,
                max_cycles: self.max_cycles,
                stack_limit_bytes: self.memory.stack_limit(),
                stack_bytes_used,
            },
            entrypoint_pc: self.entrypoint_pc,
            // TODO: the diagnostic context carries the C10 opcode byte; a baseline rejection
            // reports its low 16 bits, which have no slot here.
            opcode: match error.as_unmetered() {
                VmErrorRef::Trap(VmTrap::InvalidOpcode(op)) => u8::try_from(*op).ok(),
                _ => None,
            },
            syscall: match error.as_unmetered() {
                VmErrorRef::Trap(VmTrap::UnknownSyscall(syscall))
                | VmErrorRef::Trap(VmTrap::NotImplemented { syscall }) => Some(*syscall),
                _ => None,
            },
            predecoded_loaded,
            predecoded_hit: Some(predecoded_loaded && self.prepared_contains_pc(self.pc)),
        });
    }

    /// Record a deterministic failure at a trusted host boundary outside the run loop.
    ///
    /// The selected entrypoint must already be installed. Local execution refusals
    /// clear the snapshot and never acquire a consensus fault value.
    pub fn record_boundary_fault(&mut self, error: &VMError, position: IvmFaultPositionV1) {
        self.capture_trap_at(error, position);
    }

    /// Return the bounded deterministic fault without copying diagnostic strings.
    ///
    /// The exact child origin already captured by a nested call wins over this
    /// VM's call-site coordinates. Application rejections and local refusals do
    /// not have a fault value. `position` applies only without a trap snapshot.
    pub fn execution_fault(
        &self,
        error: &VMError,
        position: IvmFaultPositionV1,
    ) -> Option<IvmFaultV1> {
        // The engine that ran this VM's image owns the projection of its failures.
        let kind = super::engine::fault_kind(self.engine(), error)?;
        if let Some(fault) = self.last_diagnostic.and_then(|snapshot| snapshot.fault) {
            return Some(fault);
        }
        Some(IvmFaultV1 {
            kind,
            site: self.fault_site(position)?,
        })
    }

    /// Return the selected C10 fault with the original nested invocation site.
    /// Local resource deferrals and declared contract rejections have no fault.
    pub fn c10_execution_fault(
        &self,
        error: &VMError,
        position: IvmFaultPositionV1,
    ) -> Option<IvmFaultC10V1> {
        if self.profile_id() != ivm_abi::IvmProfileId::C10_ABI_V1 {
            return None;
        }
        let kind = ivm_abi::profile::c10_wire_fault_kind(error)?;
        if let Some(fault) = self.last_diagnostic.and_then(|snapshot| snapshot.c10_fault) {
            return Some(fault);
        }
        Some(IvmFaultC10V1 {
            kind,
            site: self.fault_site(position)?,
        })
    }

    fn fault_site(&self, position: IvmFaultPositionV1) -> Option<IvmFaultSiteV1> {
        let selector = match self.contract_interface() {
            None => IvmInvocationSelectorV1::Generic,
            Some(interface) => {
                let pc = self.entrypoint_pc?.checked_sub(self.program_prefix_len)?;
                let index = interface
                    .entrypoints
                    .iter()
                    .position(|entry| entry.entry_pc == pc)?;
                IvmInvocationSelectorV1::Entrypoint(u32::try_from(index).ok()?)
            }
        };
        Some(IvmFaultSiteV1 {
            code_hash: iroha_crypto::Hash::prehashed(self.code_hash),
            selector,
            position,
        })
    }

    /// Retain a child's authenticated origin before its VM is released.
    ///
    /// Only the fixed fault value crosses this boundary. Child source indices,
    /// source text and allocation owners never become parent diagnostic owners.
    pub fn inherit_execution_fault(
        &mut self,
        child: &IVM,
        error: &VMError,
        position: IvmFaultPositionV1,
    ) {
        let fault = child.execution_fault(error, position);
        let c10_fault = child.c10_execution_fault(error, position);
        if fault.is_none() && c10_fault.is_none() {
            return;
        }
        self.capture_trap(error);
        if let Some(snapshot) = self.last_diagnostic.as_mut() {
            snapshot.fault = fault;
            snapshot.c10_fault = c10_fault;
            snapshot.propagated = true;
            snapshot.source_index = None;
        }
    }

    pub(crate) fn classify_trap(err: &VMError) -> VmTrapKind {
        match err.as_unmetered() {
            VmErrorRef::Trap(VmTrap::OutOfGas)
            | VmErrorRef::Trap(VmTrap::SyscallOutOfGas { .. }) => VmTrapKind::OutOfGas,
            VmErrorRef::Trap(VmTrap::OutOfMemory) => VmTrapKind::OutOfMemory,
            VmErrorRef::Trap(VmTrap::MemoryAccessViolation { .. })
            | VmErrorRef::Trap(VmTrap::MisalignedAccess { .. })
            | VmErrorRef::Trap(VmTrap::MemoryOutOfBounds) => VmTrapKind::MemoryFault,
            VmErrorRef::Trap(VmTrap::DecodeError) => VmTrapKind::DecodeError,
            VmErrorRef::Trap(VmTrap::InvalidOpcode(_)) => VmTrapKind::InvalidOpcode,
            VmErrorRef::Trap(VmTrap::UnknownSyscall(_)) => VmTrapKind::UnknownSyscall,
            VmErrorRef::Deferred(VmDeferral::HostUnavailable)
            | VmErrorRef::Trap(VmTrap::NotImplemented { .. }) => VmTrapKind::NotImplemented,
            VmErrorRef::Deferred(VmDeferral::SyscallGasQuoteExceeded { .. }) => {
                VmTrapKind::SyscallGasQuoteExceeded
            }
            VmErrorRef::Deferred(VmDeferral::SyscallMeteringModeMismatch { .. }) => {
                VmTrapKind::SyscallMeteringModeMismatch
            }
            VmErrorRef::Trap(VmTrap::GasCostOverflow) => VmTrapKind::GasCostOverflow,
            VmErrorRef::Trap(VmTrap::NumericFault(_))
            | VmErrorRef::Trap(VmTrap::RegisterIntegerOverflow) => VmTrapKind::NumericFault,
            VmErrorRef::Trap(VmTrap::PointerAbiFault(_)) => VmTrapKind::PointerAbiFault,
            VmErrorRef::Trap(VmTrap::AssertionFailed) => VmTrapKind::AssertionFailed,
            VmErrorRef::Trap(VmTrap::ContractAbort { .. }) => VmTrapKind::ContractAbort,
            VmErrorRef::Trap(VmTrap::ExceededMaxCycles) => VmTrapKind::ExceededMaxCycles,
            VmErrorRef::Trap(VmTrap::InvalidMetadata) => VmTrapKind::InvalidMetadata,
            VmErrorRef::Trap(VmTrap::UnsupportedProgramVersion { .. }) => {
                VmTrapKind::UnsupportedProgramVersion
            }
            VmErrorRef::Trap(VmTrap::UnsupportedProgramFeatureBits { .. }) => {
                VmTrapKind::UnsupportedProgramFeatureBits
            }
            VmErrorRef::Trap(VmTrap::UnsupportedProgramAbiVersion { .. }) => {
                VmTrapKind::UnsupportedProgramAbiVersion
            }
            VmErrorRef::Trap(VmTrap::MissingMaxCycles) => VmTrapKind::MissingMaxCycles,
            VmErrorRef::Trap(VmTrap::MaxCyclesWithoutZk { .. }) => VmTrapKind::MaxCyclesWithoutZk,
            VmErrorRef::Trap(VmTrap::ProgramVectorLengthTooLarge { .. }) => {
                VmTrapKind::ProgramVectorLengthTooLarge
            }
            VmErrorRef::Trap(VmTrap::InvalidVectorLength { .. }) => VmTrapKind::InvalidVectorLength,
            VmErrorRef::Trap(VmTrap::VectorExtensionDisabled)
            | VmErrorRef::Trap(VmTrap::ZkExtensionDisabled) => VmTrapKind::PermissionDenied,
            VmErrorRef::Trap(VmTrap::ArtifactAbiHashMismatch { .. }) => {
                VmTrapKind::ArtifactAbiHashMismatch
            }
            VmErrorRef::Trap(VmTrap::GenericSyscallNotAllowed { .. }) => {
                VmTrapKind::GenericSyscallNotAllowed
            }
            VmErrorRef::Trap(VmTrap::MissingHalt) => VmTrapKind::MissingHalt,
            VmErrorRef::Trap(VmTrap::NullifierAlreadyUsed)
            | VmErrorRef::Trap(VmTrap::PermissionDenied) => VmTrapKind::PermissionDenied,
            VmErrorRef::Trap(VmTrap::ReentrantCall) => VmTrapKind::ReentrantCall,
            VmErrorRef::Trap(VmTrap::CallDepthExceeded) => VmTrapKind::CallDepthExceeded,
            VmErrorRef::Trap(VmTrap::ControlFlowViolation) => VmTrapKind::ControlFlowViolation,
            VmErrorRef::Trap(VmTrap::PrivacyViolation) => VmTrapKind::PrivacyViolation,
            VmErrorRef::Trap(VmTrap::RegisterOutOfBounds) => VmTrapKind::RegisterOutOfBounds,
            VmErrorRef::Trap(VmTrap::NoritoInvalid) => VmTrapKind::NoritoInvalid,
            VmErrorRef::Trap(VmTrap::AbiTypeNotAllowed { .. }) => VmTrapKind::AbiTypeNotAllowed,
            VmErrorRef::Trap(VmTrap::HostOutputBudgetExceeded { .. }) => {
                VmTrapKind::HostOutputBudgetExceeded
            }
            VmErrorRef::Trap(VmTrap::AmxBudgetExceeded { .. }) => VmTrapKind::AmxBudgetExceeded,
            VmErrorRef::Deferred(VmDeferral::ExecutionDeferred(_))
            | VmErrorRef::Deferred(VmDeferral::AllocationDeferred(_))
            | VmErrorRef::Deferred(VmDeferral::AllocatorDeferred(_))
            | VmErrorRef::Deferred(VmDeferral::DecodeDeferred(_))
            | VmErrorRef::Deferred(VmDeferral::VerificationDeferred(_)) => VmTrapKind::Other,
            VmErrorRef::Trap(VmTrap::Metered { .. }) => {
                unreachable!("as_unmetered peels metered wrappers")
            }
        }
    }
}
#[cfg(test)]
mod tests;
