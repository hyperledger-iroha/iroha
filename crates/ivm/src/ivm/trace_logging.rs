//! Original-funded runtime and completed-cycle observations admitted before effects.
//!
//! Local trace-owner mismatches defer execution without becoming transaction
//! rejections. Existing ZK host-isolation errors keep PrivacyViolation precedence.
//! The run owner begins before root preparation, records the initial observation
//! before fetch, and admits each next observation before the preceding instruction
//! changes registers. It finishes on every result, including the unwind branch.

#[cfg(test)]
use super::zk;
use super::{IVM, IvmEngine, TraceMode, VMError, completed_instruction_cycles};
use crate::error::VmDeferral;
#[cfg(test)]
use iroha_allocation::AllocationBudget;
use iroha_allocation::{AllocationRefusal, AllocationScope};

mod change_counts;

/// Trace shape selected once when the VM admits this invocation.
///
/// The interpreter owns the VM for the lifetime of the run. A syscall borrows
/// only its sealed execution context and cannot change the program, trace
/// policy or allocation pool. Trace storage retains its own allocation owner.
pub(super) struct InvocationTrace {
    mode: TraceMode,
    engine: IvmEngine,
    cycles: bool,
}

/// Borrowed preparation custody; refusal or unwind cancels only unused spans.
///
/// No storage is refunded here. Original allocation scopes and their mutable
/// log borrows finish before this guard retires; cancellation only clears the
/// inline pending quota. The enclosing invocation owns the panic boundary.
struct TracePreparation<'vm> {
    vm: &'vm mut IVM,
    armed: bool,
}
impl Drop for TracePreparation<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.vm.discard_unobserved_trace();
        }
    }
}

#[cfg(test)]
pub(super) struct TraceStorageCopies {
    pub(super) cycles: zk::DeltaTraceLog,
    pub(super) runtime: zk::DeltaTraceLog,
    pub(super) pcs: zk::PcTraceLog,
}

impl IVM {
    fn with_trace_scope<T>(
        &mut self,
        operation: impl FnOnce(&mut Self, Option<&AllocationScope<'_>>) -> Result<T, VMError>,
    ) -> Result<T, VMError> {
        // Clone the original budget handle, never its limits or an independent
        // pool. All replacements and partial copies retire inside this scope;
        // callbacks occur only after mutable log borrows have ended.
        match self.memory.allocation_budget().cloned() {
            Some(original) => {
                original.with_deferred_refund_notifications(|scope| operation(self, Some(scope)))
            }
            None => operation(self, None),
        }
    }

    /// Check lifecycle custody before any reset or program-load effects.
    pub(super) fn check_trace_storage_owner(&self) -> Result<(), VMError> {
        match self.memory.allocation_budget() {
            Some(original) => original
                .with_deferred_refund_notifications(|scope| self.validate_trace_scope(Some(scope))),
            None => self.validate_trace_scope(None),
        }
    }

    fn validate_trace_scope(&self, scope: Option<&AllocationScope<'_>>) -> Result<(), VMError> {
        // Check all three before reset/copy/preparation mutates any one owner.
        self.trace_log.validate_scope(scope)?;
        self.delta_trace.validate_scope(scope)?;
        self.pc_trace.validate_scope(scope)
    }

    /// Admit the initial pre-fetch record before root arguments or host effects.
    ///
    /// The caller has scrubbed the preceding invocation. Admit storage before
    /// root arguments, guest state, gas or cycles change.
    pub(super) fn begin_trace_invocation(&mut self) -> Result<InvocationTrace, VMError> {
        let invocation = InvocationTrace {
            mode: self.trace_mode,
            engine: self.engine(),
            cycles: self.zk_trace_collection_enabled() && self.max_cycles != 0,
        };
        if invocation.cycles && invocation.engine == IvmEngine::Baseline {
            // Completed-cycle proof traces are C10 rows; the baseline producer is not
            // reconstructed (`run_baseline` refuses it first), so never record C10 rows here.
            return Err(VMError::Deferred(VmDeferral::ExecutionDeferred(
                crate::error::ExecutionDeferral::TraceOwnerUnavailable,
            )));
        }
        if invocation.cycles {
            // Every completed cycle records the memory root. Allocate its Merkle
            // storage now, before root arguments or guest effects, so no step
            // meets the first allocation.
            self.memory.ensure_merkle_storage()?;
        }
        self.with_trace_scope(|vm, scope| {
            vm.validate_trace_scope(scope)?;
            match invocation.mode {
                TraceMode::Off => Ok(()),
                TraceMode::PcOnly => vm.pc_trace.prepare(1, scope),
                TraceMode::DeltaRegisters => match invocation.engine {
                    IvmEngine::C10 => {
                        vm.delta_trace
                            .prepare_batch(1, crate::parallel::REGISTER_COUNT, 0, scope)
                    }
                    IvmEngine::Baseline => vm.delta_trace.prepare_baseline_batch(
                        1,
                        crate::registers::BASELINE_REGISTER_COUNT,
                        0,
                        scope,
                    ),
                },
            }
        })?;
        Ok(invocation)
    }

    /// Publish the already-admitted initial or preceding-instruction observation.
    #[inline]
    pub(super) fn record_trace_prefetch(&mut self, invocation: &InvocationTrace) {
        match invocation.mode {
            TraceMode::Off => {}
            TraceMode::PcOnly => self.pc_trace.record_reserved(self.pc),
            TraceMode::DeltaRegisters => match invocation.engine {
                IvmEngine::C10 => self.delta_trace.record_reserved(
                    self.pc,
                    self.registers.snapshot(),
                    self.registers.snapshot_tags(),
                ),
                IvmEngine::Baseline => self.delta_trace.record_baseline_reserved(
                    self.pc,
                    &self.registers.baseline_snapshot(),
                    &self.registers.baseline_snapshot_tags(),
                ),
            },
        }
    }

    /// Admit completed cycles and the next pre-fetch row, then all later preflight.
    ///
    /// `tail_preflight` admits register rows, syscall shells, shared cycles and
    /// native capture before the caller debits opcode gas or performs effects.
    /// A refused/panicking tail cancels unobserved credit without losing earlier
    /// observations. Nested host callbacks do not own or finish this batch.
    pub(super) fn prepare_trace_instruction<T>(
        &mut self,
        invocation: &InvocationTrace,
        instruction: u32,
        tail_preflight: impl FnOnce(&mut Self) -> Result<T, VMError>,
    ) -> Result<T, VMError> {
        if !invocation.cycles && invocation.mode == TraceMode::Off {
            // Ordinary execution has no trace backing to admit or cancel. Do
            // not clone an allocation owner, enter a refund scope, or classify
            // destinations on this hot path. Tail errors (including the
            // earlier ZK privacy check) and panics propagate unchanged.
            return tail_preflight(self);
        }
        let changes = if invocation.cycles || invocation.mode == TraceMode::DeltaRegisters {
            match invocation.engine {
                IvmEngine::C10 => change_counts::instruction(instruction)?,
                // The baseline refuses proof-trace cycle collection (`run_baseline`), so only
                // its delta observations reach here.
                IvmEngine::Baseline => {
                    change_counts::baseline::instruction(instruction, self.baseline.vector_length)?
                }
            }
        } else {
            0
        };
        let cycles = if invocation.cycles {
            completed_instruction_cycles(crate::instruction::wide::opcode(instruction))
        } else {
            0
        };
        let mut preparation = TracePreparation {
            vm: self,
            armed: true,
        };
        preparation.vm.with_trace_scope(|vm, scope| {
            vm.validate_trace_scope(scope)?;
            if invocation.cycles {
                let rows = usize::try_from(cycles).map_err(|_| overflow())?;
                vm.step_log.prepare_cycles(cycles)?;
                vm.trace_log.prepare_batch(rows, changes, 0, scope)?;
            }
            match invocation.mode {
                TraceMode::Off => {}
                TraceMode::PcOnly => vm.pc_trace.prepare(1, scope)?,
                TraceMode::DeltaRegisters => match invocation.engine {
                    IvmEngine::C10 => vm.delta_trace.prepare_batch(1, changes, 0, scope)?,
                    IvmEngine::Baseline => {
                        vm.delta_trace
                            .prepare_baseline_batch(1, changes, 0, scope)?;
                    }
                },
            }
            Ok(())
        })?;
        let output = tail_preflight(preparation.vm)?;
        preparation.armed = false;
        Ok(output)
    }

    /// Admit payable padding only after the final instruction's cycles flushed.
    ///
    /// The caller keeps the existing unpayable-padding OOG path, without calling
    /// this method or attempting to allocate. Padding never consumes the unused
    /// next-pre-fetch runtime record reserved by the terminal instruction.
    pub(super) fn prepare_trace_padding(
        &mut self,
        invocation: &InvocationTrace,
        additional: u64,
    ) -> Result<(), VMError> {
        if !invocation.cycles || additional == 0 {
            return Ok(());
        }
        let mut preparation = TracePreparation {
            vm: self,
            armed: true,
        };
        preparation.vm.with_trace_scope(|vm, scope| {
            vm.validate_trace_scope(scope)?;
            let rows = usize::try_from(additional).map_err(|_| overflow())?;
            vm.step_log.prepare_cycles(additional)?;
            vm.trace_log.prepare_batch(rows, 0, 0, scope)
        })?;
        preparation.armed = false;
        Ok(())
    }

    /// Flush only completed, pre-admitted cycles after delayed native finish.
    ///
    /// The untraced return stays inline. Publishing funded cycles remains out of line.
    #[inline]
    pub(super) fn publish_trace_cycles(
        &mut self,
        invocation: &InvocationTrace,
        last_logged_cycle: &mut u64,
    ) -> Result<(), VMError> {
        if !invocation.cycles {
            return Ok(());
        }
        self.publish_admitted_cycles(last_logged_cycle)
    }

    fn publish_admitted_cycles(&mut self, last_logged_cycle: &mut u64) -> Result<(), VMError> {
        while *last_logged_cycle < self.cycles {
            // The invocation allocated the Merkle storage at its start; this
            // only hashes leaves written since. Read it before either record so
            // a cycle is never half published.
            let memory_root = self.memory.root()?;
            self.trace_log.record_reserved(
                self.pc,
                self.registers.snapshot(),
                self.registers.snapshot_tags(),
            );
            self.step_log
                .record_reserved(self.pc, self.registers.merkle_root(), memory_root);
            *last_logged_cycle += 1;
        }
        Ok(())
    }

    /// Finish on success, refusal or unwind without erasing initialized history.
    pub(super) fn finish_trace_invocation(&mut self, _invocation: InvocationTrace) {
        self.discard_unobserved_trace();
    }

    fn discard_unobserved_trace(&mut self) {
        self.trace_log.discard_unobserved();
        self.delta_trace.discard_unobserved();
    }

    /// Retire all delta/PC backing while keeping each original allocation owner.
    pub(super) fn reset_trace_storage(&mut self) -> Result<(), VMError> {
        self.with_trace_scope(|vm, scope| {
            vm.validate_trace_scope(scope)?;
            vm.trace_log.reset(scope)?;
            vm.delta_trace.reset(scope)?;
            vm.pc_trace.reset(scope)
        })
    }

    /// Independently fund all capacities and pending credit inside snapshot scope.
    ///
    /// Snapshot aggregate byte accounting must exclude these three owners. The
    /// caller holds the original deferred-refund scope outside its register-log
    /// mutex, so partial copies retire before any release callback is delivered.
    #[cfg(test)]
    pub(super) fn try_clone_trace_storage(
        &self,
        scope: Option<&AllocationScope<'_>>,
    ) -> Result<TraceStorageCopies, VMError> {
        self.validate_trace_scope(scope)?;
        Ok(TraceStorageCopies {
            cycles: self.trace_log.try_clone_allocation(scope)?,
            runtime: self.delta_trace.try_clone_allocation(scope)?,
            pcs: self.pc_trace.try_clone_allocation(scope)?,
        })
    }
}

fn overflow() -> VMError {
    VMError::Deferred(VmDeferral::AllocationDeferred(
        AllocationRefusal::DemandOverflow,
    ))
}

#[cfg(test)]
mod baseline_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod cycle_tests;
