//! The frozen baseline engine ([`crate::IvmEngine::Baseline`]) behind the one [`IVM`] facade.
//!
//! This is the interpreter of the pre-C10 anchor `77600a4840`, reconstructed so that artifacts
//! admitted under the installed baseline selection keep their exact meaning: the 8-bit-operand
//! instruction set over 256 registers, vector, parallel, direct-crypto and ZK-helper opcodes,
//! 128-bit memory operations, `AssertionFailed` arithmetic and call-integrity traps, header-derived
//! cycle limits, ZK padding to the header bound, shared nested cycle allowances
//! ([`VmCycleBudget`]) and the original gas schedule ([`crate::gas::baseline`]). The run loop is
//! the anchor's `run_with_host_ref` with only these mechanical changes: opcodes and field
//! extractors come from [`ivm_abi::baseline::instruction`], baseline-only state lives in
//! [`BaselineMachine`], and call-frame helpers shared with C10 have their C10 fault spelling mapped
//! back ([`baseline_call_fault`]). Nothing here may be optimized until differential evidence over
//! `crates/ivm/tests/fixtures/baseline_v1` shows identical bytes, results, faults, gas, events and
//! commitments.

use super::*;
use ivm_abi::baseline::{encoding::wide as bencoding, instruction::wide as bwide};

mod field;

/// Upper bound on the logical vector length.
const LOGICAL_VECTOR_MAX: usize = ivm_abi::baseline::VECTOR_LENGTH_MAX as usize;
/// First general register that holds vector register data.
const VECTOR_BASE: usize = 32;

fn setvl_length(raw: usize) -> Result<usize, VMError> {
    let vl = if raw == 0 { 1 } else { raw };
    if vl > LOGICAL_VECTOR_MAX {
        return Err(VMError::InvalidVectorLength { vector_length: vl });
    }
    Ok(vl)
}
fn validate_vadd64_length(vl: usize) -> Result<(), VMError> {
    if vl == 0 || !vl.is_multiple_of(2) {
        return Err(VMError::InvalidVectorLength { vector_length: vl });
    }
    Ok(())
}
fn checked_div_i64(num: i64, denom: i64) -> Result<i64, VMError> {
    num.checked_div(denom).ok_or(VMError::AssertionFailed)
}
fn checked_rem_i64(num: i64, denom: i64) -> Result<i64, VMError> {
    num.checked_rem(denom).ok_or(VMError::AssertionFailed)
}
fn checked_abs_i64(value: i64) -> Result<i64, VMError> {
    value.checked_abs().ok_or(VMError::AssertionFailed)
}
fn div_ceil_i64(num: i64, denom: i64) -> Result<i64, VMError> {
    let q = checked_div_i64(num, denom)?;
    let r = checked_rem_i64(num, denom)?;
    if r == 0 {
        Ok(q)
    } else if (r > 0 && denom > 0) || (r < 0 && denom < 0) {
        q.checked_add(1).ok_or(VMError::AssertionFailed)
    } else {
        Ok(q)
    }
}
/// Reservation cost matching the completed-cycle increments of the baseline dispatch table.
fn baseline_completed_instruction_cycles(opcode: u8) -> u64 {
    match opcode {
        bwide::arithmetic::ISQRT => 6,
        bwide::arithmetic::DIV_CEIL | bwide::arithmetic::GCD => 12,
        bwide::arithmetic::MEAN => 3,
        _ => 1,
    }
}
/// The baseline spelling of a call-integrity failure raised by call-frame helpers shared with
/// C10.
///
/// The baseline reported every call/return-integrity and call-depth failure of a protected
/// contract as `AssertionFailed`; C10 renamed them `ControlFlowViolation` and
/// `CallDepthExceeded` in those helpers. Callers apply this only to results of those helpers,
/// never to host results, where `CallDepthExceeded` keeps its own meaning.
pub(super) fn baseline_call_fault(error: VMError) -> VMError {
    match error {
        VMError::ControlFlowViolation | VMError::CallDepthExceeded => VMError::AssertionFailed,
        other => other,
    }
}

/// A constraint recorded by a baseline ZK helper while proof traces are collected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaselineConstraint {
    /// Register `reg` must be zero at `cycle`.
    Zero {
        /// Register index.
        reg: usize,
        /// Cycle of the assertion.
        cycle: u64,
    },
    /// Registers must be equal at `cycle`.
    Eq {
        /// First register index.
        reg1: usize,
        /// Second register index.
        reg2: usize,
        /// Cycle of the assertion.
        cycle: u64,
    },
    /// Register `reg` must fit in `bits` bits at `cycle`.
    Range {
        /// Register index.
        reg: usize,
        /// Bit width.
        bits: u8,
        /// Cycle of the assertion.
        cycle: u64,
    },
}

/// The anchor's constraint digest bound into execution summaries.
pub(super) fn hash_constraints(constraints: &[BaselineConstraint]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ivm-summary:constraints:v1");
    hasher.update((constraints.len() as u64).to_le_bytes());
    for constraint in constraints {
        match *constraint {
            BaselineConstraint::Zero { reg, cycle } => {
                hasher.update([0]);
                hasher.update((reg as u64).to_le_bytes());
                hasher.update(cycle.to_le_bytes());
            }
            BaselineConstraint::Eq { reg1, reg2, cycle } => {
                hasher.update([1]);
                hasher.update((reg1 as u64).to_le_bytes());
                hasher.update((reg2 as u64).to_le_bytes());
                hasher.update(cycle.to_le_bytes());
            }
            BaselineConstraint::Range { reg, bits, cycle } => {
                hasher.update([2]);
                hasher.update((reg as u64).to_le_bytes());
                hasher.update([bits]);
                hasher.update(cycle.to_le_bytes());
            }
        }
    }
    IVM::finish_digest(hasher)
}

/// Machine state only the baseline engine reads; inert while a C10 image is installed.
#[derive(Debug, Clone)]
pub(super) struct BaselineMachine {
    /// Whether the header enabled the vector extension.
    pub(super) vector_enabled: bool,
    /// Current logical vector length.
    pub(super) vector_length: usize,
    /// Logical vector length the header selected, restored by every reset.
    pub(super) header_vector_length: u8,
    /// Constraints recorded by ZK helpers while proof traces are collected.
    pub(super) constraints: Vec<BaselineConstraint>,
    /// Scoped shared cycle allowance; runtime resets cannot replenish it.
    pub(super) active_cycle_budget: Option<Arc<SharedVmCycleBudget>>,
}

impl Default for BaselineMachine {
    fn default() -> Self {
        Self {
            vector_enabled: false,
            vector_length: ivm_abi::baseline::DEFAULT_VECTOR_LENGTH,
            header_vector_length: 0,
            constraints: Vec::new(),
            active_cycle_budget: None,
        }
    }
}

impl BaselineMachine {
    /// State of a freshly installed image whose header declared `vector_length` and `mode`.
    pub(super) fn install(&mut self, profile: IvmProfileId, mode: u8, vector_length: u8) {
        self.constraints.clear();
        match profile.engine() {
            IvmEngine::Baseline => {
                self.vector_enabled = mode & ivm_abi::baseline::mode::VECTOR != 0;
                self.header_vector_length = vector_length;
                self.vector_length = ivm_abi::baseline::initial_vector_length(vector_length);
            }
            IvmEngine::C10 => {
                self.vector_enabled = false;
                self.header_vector_length = 0;
                self.vector_length = ivm_abi::baseline::DEFAULT_VECTOR_LENGTH;
            }
        }
    }

    /// Restore the per-invocation state a runtime reset restores.
    pub(super) fn reset(&mut self) {
        self.constraints.clear();
        self.vector_length = ivm_abi::baseline::initial_vector_length(self.header_vector_length);
    }
}

/// One finite allowance of completed architectural VM cycles shared by a source's runs.
///
/// This is a non-cloneable owner, not source authentication or a host-work budget.
/// Each scoped run consumes the same allowance, including nested VMs and ZK padding.
/// Failed instructions release their pending reservation; previously completed cycles
/// remain consumed. Dropping this owner closes every retained runtime reference.
///
/// Only the baseline engine draws from an allowance; a C10 image is bounded by gas alone.
#[derive(Debug)]
pub struct VmCycleBudget {
    shared: Arc<SharedVmCycleBudget>,
}
#[derive(Debug)]
pub(super) struct SharedVmCycleBudget {
    limit: u64,
    state: parking_lot::Mutex<VmCycleBudgetState>,
}
#[derive(Debug, Default)]
struct VmCycleBudgetState {
    consumed: u64,
    reserved: u64,
    refused: bool,
    closed: bool,
}
impl VmCycleBudget {
    /// Create an explicit finite allowance. The caller owns source/policy authentication.
    pub fn new(limit: std::num::NonZeroU64) -> Self {
        Self {
            shared: Arc::new(SharedVmCycleBudget {
                limit: limit.get(),
                state: parking_lot::Mutex::new(VmCycleBudgetState::default()),
            }),
        }
    }
    /// The immutable finite allowance established by the owner.
    pub fn limit(&self) -> u64 {
        self.shared.limit
    }
    /// Completed architectural cycles, including completed nested execution and ZK padding.
    pub fn consumed(&self) -> u64 {
        self.shared.state.lock().consumed
    }
    /// Cycles available for another dispatch, excluding active parent reservations.
    pub fn remaining(&self) -> u64 {
        let state = self.shared.state.lock();
        self.shared.limit - state.consumed - state.reserved
    }
    /// Whether the owner is open and has no local runtime-ownership fault.
    ///
    /// This is independent of deterministic cycle exhaustion.
    pub fn is_open(&self) -> bool {
        !self.shared.state.lock().closed
    }
    /// Whether any dispatch or padding reservation was refused for lack of cycles.
    ///
    /// Exact-fit successful execution has zero remaining cycles without exhaustion.
    pub fn exhausted(&self) -> bool {
        self.shared.state.lock().refused
    }
}
impl Drop for VmCycleBudget {
    fn drop(&mut self) {
        self.shared.state.lock().closed = true;
    }
}
impl SharedVmCycleBudget {
    fn ensure_healthy(&self) -> Result<(), VMError> {
        let state = self.state.lock();
        if state.closed {
            Err(VMError::HostUnavailable)
        } else if state.refused {
            Err(VMError::ExceededMaxCycles)
        } else {
            Ok(())
        }
    }
    fn reserve(&self, cycles: u64) -> Result<VmCycleReservation<'_>, VMError> {
        let mut state = self.state.lock();
        if state.closed {
            return Err(VMError::HostUnavailable);
        }
        if state.refused || cycles > self.limit - state.consumed - state.reserved {
            state.refused = true;
            return Err(VMError::ExceededMaxCycles);
        }
        state.reserved += cycles;
        Ok(VmCycleReservation {
            shared: self,
            cycles,
            pending: true,
        })
    }
}
struct VmCycleReservation<'a> {
    shared: &'a SharedVmCycleBudget,
    cycles: u64,
    pending: bool,
}
impl VmCycleReservation<'_> {
    fn complete(mut self, actual: Option<u64>) -> Result<(), VMError> {
        let mut state = self.shared.state.lock();
        state.reserved -= self.cycles;
        self.pending = false;
        if actual != Some(self.cycles) {
            // A lifecycle mutation or a new opcode with an unreviewed cycle cost
            // cannot turn an inaccurate reservation into accepted execution.
            state.closed = true;
            return Err(VMError::HostUnavailable);
        }
        state.consumed += self.cycles;
        Ok(())
    }
}
impl Drop for VmCycleReservation<'_> {
    fn drop(&mut self) {
        if self.pending {
            self.shared.state.lock().reserved -= self.cycles;
        }
    }
}
impl IVM {
    /// Current logical vector length of a baseline image.
    #[must_use]
    pub fn vector_length(&self) -> usize {
        self.baseline.vector_length
    }

    fn baseline_match_tags(&self, rs1: usize, rs2: usize) -> Result<Option<bool>, VMError> {
        if self.zk_mode {
            let tag_a = self.registers.tag(rs1);
            let tag_b = self.registers.tag(rs2);
            if tag_a != tag_b {
                return Err(VMError::PrivacyViolation);
            }
            Ok(Some(tag_a))
        } else {
            Ok(None)
        }
    }

    /// Fetch the next instruction priced by the baseline schedule.
    ///
    /// The prepared stream of a baseline image was validated and priced by the baseline engine.
    /// An unprepared fetch decodes the raw word with no reserved-bit rule, as the anchor did.
    fn baseline_fetch_instruction(&mut self) -> Result<FetchedOp, VMError> {
        if let Some(op) = self
            .prepared
            .as_ref()
            .and_then(|prepared| prepared.op_at(self.pc))
        {
            let fetched = op.fetched();
            self.memory
                .diagnostic_record_prepared_fetch(self.pc, fetched.inst)?;
            return Ok(fetched);
        }
        if self.prepared_required {
            return Err(VMError::MemoryAccessViolation {
                addr: self.pc as u32,
                perm: Perm::EXECUTE,
            });
        }
        let inst = decoder::decode(&self.memory, self.pc)?;
        Ok(FetchedOp {
            inst,
            wide_op: bwide::opcode(inst),
            base_gas: crate::gas::baseline::cost_of(inst),
        })
    }

    /// Charge the byte-linear portion of a direct signature opcode before
    /// checksum hashing or cryptographic verification.
    ///
    /// All operands are inspected even after a malformed public pointer so a
    /// later private operand still fails with `PrivacyViolation`. Aliased
    /// operands are charged independently because validation scans each role.
    fn baseline_preflight_signature_opcode_payloads(
        &mut self,
        pointers: [u64; 3],
    ) -> Result<bool, VMError> {
        let mut payload_lengths = [0_u64; 3];
        let mut malformed = false;
        for (index, pointer) in pointers.into_iter().enumerate() {
            match self.inspect_owned_public_tlv_header(pointer) {
                Ok((length, version)) => {
                    if version == 1 {
                        payload_lengths[index] = length;
                    } else {
                        // Version rejection is fixed-header work. It precedes
                        // checksum hashing, so it must not receive the
                        // byte-linear surcharge.
                        malformed = true;
                    }
                }
                Err(VMError::PrivacyViolation) => return Err(VMError::PrivacyViolation),
                Err(error) if error.execution_deferral().is_some() => return Err(error),
                Err(_) => malformed = true,
            }
        }
        if malformed {
            return Ok(false);
        }
        let extra_gas = crate::gas::baseline::signature_opcode_extra_gas(
            payload_lengths[0],
            payload_lengths[1],
            payload_lengths[2],
        )
        .ok_or(VMError::GasCostOverflow)?;
        self.debit_gas(extra_gas)?;
        Ok(true)
    }
    /// Validate a public cryptographic operand with a boolean result for malformed TLVs.
    /// Privacy violations and operational refusals never become verification failures.
    fn baseline_validate_public_crypto_tlv(
        &self,
        ptr: u64,
    ) -> Result<Option<crate::pointer_abi::Tlv<'_>>, VMError> {
        match self.validate_tlv(ptr) {
            Ok(tlv) => Ok(Some(tlv)),
            Err(VMError::PrivacyViolation) => Err(VMError::PrivacyViolation),
            Err(error) if error.execution_deferral().is_some() => Err(error),
            Err(_) => Ok(None),
        }
    }
    /// Run with the caller's single finite allowance for completed architectural cycles.
    ///
    /// Reusing a VM or running another Batch segment does not replenish `budget`.
    /// Core owns the signed-source binding and must use ordinary `run_with_host`
    /// for separately executed trigger callbacks. Gas and per-program cycle checks
    /// remain independent. A refused shared reservation returns `ExceededMaxCycles`.
    ///
    /// A shared cycle allowance belongs to the baseline engine; a C10 image refuses it locally
    /// with [`VMError::HostUnavailable`], since gas is its only execution budget.
    pub fn run_with_host_and_cycle_budget(
        &mut self,
        host: &mut dyn IVMHost,
        budget: &VmCycleBudget,
    ) -> Result<(), VMError> {
        self.run_with_host_and_shared_cycles(host, Arc::clone(&budget.shared))
    }
    /// Run a nested VM with the active parent's exact cycle allowance, if present.
    ///
    /// Hosts must use this for nested contract dispatch rather than creating a new
    /// allowance or deriving one from a remaining-cycle scalar.
    pub fn run_with_host_and_parent_cycle_budget(
        &mut self,
        host: &mut dyn IVMHost,
        parent: &IVM,
    ) -> Result<(), VMError> {
        match &parent.baseline.active_cycle_budget {
            Some(shared) => self.run_with_host_and_shared_cycles(host, Arc::clone(shared)),
            None => self.run_with_host(host),
        }
    }
    fn run_with_host_and_shared_cycles(
        &mut self,
        host: &mut dyn IVMHost,
        shared: Arc<SharedVmCycleBudget>,
    ) -> Result<(), VMError> {
        self.last_diagnostic = None;
        if self.engine() != IvmEngine::Baseline {
            return Err(VMError::HostUnavailable);
        }
        if self
            .baseline
            .active_cycle_budget
            .as_ref()
            .is_some_and(|active| !Arc::ptr_eq(active, &shared))
        {
            // Rebinding a runtime retained by another active source is a local
            // host ownership error, not deterministic capacity exhaustion.
            shared.state.lock().closed = true;
            if let Some(active) = &self.baseline.active_cycle_budget {
                active.state.lock().closed = true;
            }
            return Err(VMError::HostUnavailable);
        }
        shared.ensure_healthy()?;
        let previous = self.baseline.active_cycle_budget.replace(shared);
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| self.run_with_host(host)));
        self.baseline.active_cycle_budget = previous;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    #[inline]
    pub(super) fn run_baseline(
        &mut self,
        host: &mut dyn IVMHost,
        mut recorder: Option<&mut DiagnosticStepRecorder>,
    ) -> Result<(), VMError> {
        self.last_diagnostic = None;
        self.ensure_complete_program_image()?;
        // TODO: the baseline's proof-trace producers (register authentication logs over the
        // 256-register file, diagnostic step records and native execution
        // packets) are not reconstructed. They are refused locally instead of recording C10-shaped
        // observations of a baseline execution; consensus execution never requests them.
        if (self.zk_mode && self.zk_trace_enabled)
            || zk::scoped_reg_logger_enabled() == Some(true)
            || self.native_packets.is_some()
            || recorder.is_some()
        {
            return Err(VMError::ExecutionDeferred(
                crate::error::ExecutionDeferral::TraceOwnerUnavailable,
            ));
        }
        // Keep a local reference through host callbacks/lifecycle changes. This
        // owner never comes from program bytes or a warmed runtime template.
        let cycle_budget = self.baseline.active_cycle_budget.clone();
        if let Some(budget) = &cycle_budget {
            budget.ensure_healthy()?;
        }
        // Admit the next logger before changing guest state or clearing the
        // preceding invocation. Invalid detached custody is severed without allocation.
        self.prepare_invocation_register_log()?;
        // Select this invocation's policy from its configuration, never from
        // an outer VM's thread-local scope.
        let invocation_log = if self.zk_mode && self.zk_trace_enabled {
            self.reg_log.clone()
        } else {
            None
        };
        // The installed scope fixes this policy for every step; host callbacks
        // only mask it. An untraced run therefore never needs register rows.
        let traced = invocation_log.is_some();
        let _reg_logger_guard = zk::RegLoggerGuard::install(invocation_log);
        // Proof-facing accessors describe the most recent invocation. Clear
        // prior artifacts even when this run has tracing disabled.
        self.clear_zk_trace_logs();
        self.baseline.constraints.clear();
        if !self.zk_mode && (self.registers.has_private() || !self.private_memory_bytes.is_empty())
        {
            return Err(VMError::ExecutionDeferred(
                crate::error::ExecutionDeferral::LocalInvariantViolation,
            ));
        }
        let invocation_trace = self.begin_trace_invocation()?;
        let mut previous_registers = None;
        self.last_diagnostic = None;
        self.halted = false;
        self.constraint_failed = false;
        self.contract_abort_error = None;
        self.cycles = 0;
        // A run is one invocation. Never retain protected return state after a
        // prior trap or across a pooled-runtime reuse boundary.
        self.contract_return_stack.clear();
        self.contract_outer_return_pc = None;
        self.memory.call_frames.clear();
        let mut pending_step: Option<PendingDiagnosticStep> = None;
        let mut fault_position =
            iroha_data_model::executor::fault::IvmFaultPositionV1::Initialization;
        let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _pointer_policy_guard =
                PointerPolicyGuard::install(self.syscall_policy(), self.abi_version());
            self.begin_root_call(host).map_err(baseline_call_fault)?;
            let mut last_logged_cycle = 0;
            let mut pending_cycles: Option<(u64, VmCycleReservation<'_>)> = None;
            let mut pending_register_batch = None;
            // Baseline fetch, decode and execute.
            loop {
                fault_position = iroha_data_model::executor::fault::IvmFaultPositionV1::Execute {
                    pc_offset: self.pc.saturating_sub(self.program_prefix_len),
                };
                if let Some((before, reservation)) = pending_cycles.take() {
                    reservation.complete(self.cycles.checked_sub(before))?;
                }
                if let Some(step) = pending_step.take() {
                    recorder
                        .as_deref_mut()
                        .expect("diagnostic step has recorder")
                        .finish_step(
                            step,
                            self.diagnostic_step_state(),
                            DiagnosticStepOutcome::Completed,
                        )?;
                }
                self.native_finish_step();
                if let Some(batch) = pending_register_batch.take() {
                    drop(batch);
                }
                self.publish_trace_cycles(&invocation_trace, &mut last_logged_cycle)?;
                // Stop the loop if HALT was executed. When a cycle limit is set we
                // continue executing even after a failed assertion so the trace
                // length is independent of witness values.
                if unlikely(self.halted || (!self.zk_mode && self.constraint_failed)) {
                    break;
                }
                if unlikely(self.pc == self.memory.code_len()) {
                    return Err(VMError::MissingHalt);
                }
                if unlikely(self.max_cycles != 0 && self.cycles >= self.max_cycles) {
                    return Err(VMError::ExceededMaxCycles);
                }
                self.record_baseline_prefetch(&invocation_trace, &mut previous_registers)?;
                if let Some(recorder) = recorder.as_deref_mut() {
                    pending_step = Some(recorder.begin_step(self.diagnostic_step_state())?);
                    self.memory
                        .diagnostic_step_ordinal(recorder.records().len() as u64);
                }
                let fetched = self.baseline_fetch_instruction()?;
                let instr = fetched.inst;
                if let Some(step) = pending_step.as_mut() {
                    step.fetched(instr);
                }
                let length = WIDE_INSTRUCTION_LEN;
                let wide_op = fetched.wide_op;
                if !bwide::is_valid_opcode(wide_op) {
                    return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
                }
                // Determine the gas cost for this operation and deduct it.
                // Scale vector op costs by the current logical vector length.
                let Some(cost) = crate::gas::baseline::cost_from_parts(
                    fetched.base_gas,
                    wide_op,
                    self.baseline.vector_length,
                ) else {
                    return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
                };
                if let Some(step) = pending_step.as_mut() {
                    step.priced(cost);
                }
                if unlikely(self.gas_remaining < cost) {
                    return Err(VMError::OutOfGas);
                }
                let prepared_host_log =
                    self.prepare_trace_instruction(&invocation_trace, instr, |vm| {
                        // Every admitted opcode has a finite row bound, so an
                        // untraced run skips only an inactive batch here.
                        if traced {
                            pending_register_batch =
                                Some(vm.prepare_instruction_register_events(instr)?);
                        }
                        // Shell, shared-cycle and native capture demand all precede
                        // base gas and execute under the same observation quota.
                        let prepared_host_log =
                            matches!(wide_op, bwide::system::SCALL | bwide::system::SYSTEM)
                                .then(|| vm.prepare_host_register_log())
                                .transpose()?;
                        if let Some(budget) = &cycle_budget {
                            let reservation =
                                budget.reserve(baseline_completed_instruction_cycles(wide_op))?;
                            pending_cycles = Some((vm.cycles, reservation));
                        }
                        vm.native_preflight_step(instr, cost)?;
                        Ok(prepared_host_log)
                    })?;
                self.gas_remaining -= cost;
                // Execute the instruction
                let opcode = instr & 0x7F;
                // Dispatch via the canonical wide (8-bit opcode) instruction table. Classic
                // encodings are no longer executable; unknown opcodes trap with `InvalidOpcode`.
                match wide_op {
                    bwide::system::GETGAS => {
                        let rd = bwide::rd(instr);
                        self.registers.set(rd, self.gas_remaining);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::system::SCALL => {
                        let imm8 = bwide::imm8(instr) as u8 as u32;
                        if !host_allows_syscall_masked(host, self.syscall_policy(), imm8) {
                            return Err(VMError::UnknownSyscall(imm8));
                        }
                        self.execute_syscall_with_register_log(
                            host,
                            imm8,
                            prepared_host_log.as_ref(),
                        )?;
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::system::SYSTEM => {
                        let number = bencoding::decode_syscallx(instr);
                        if !host_allows_syscall_masked(host, self.syscall_policy(), number) {
                            return Err(VMError::UnknownSyscall(number));
                        }
                        self.execute_syscall_with_register_log(
                            host,
                            number,
                            prepared_host_log.as_ref(),
                        )?;
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ADD => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self
                            .registers
                            .get(rs1)
                            .wrapping_add(self.registers.get(rs2));
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SUB => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self
                            .registers
                            .get(rs1)
                            .wrapping_sub(self.registers.get(rs2));
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::AND => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self.registers.get(rs1) & self.registers.get(rs2);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::OR => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self.registers.get(rs1) | self.registers.get(rs2);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::XOR => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self.registers.get(rs1) ^ self.registers.get(rs2);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SLL => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self.registers.get(rs1) << (self.registers.get(rs2) & 0x3f);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SRL => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self.registers.get(rs1) >> (self.registers.get(rs2) & 0x3f);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SRA => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = ((self.registers.get(rs1) as i64)
                            >> (self.registers.get(rs2) & 0x3f))
                            as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::MUL => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let val = self
                            .registers
                            .get(rs1)
                            .wrapping_mul(self.registers.get(rs2));
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::MULH => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64 as i128;
                        let b = self.registers.get(rs2) as i64 as i128;
                        let val = ((a * b) >> 64) as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::MULHU => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let prod = (self.registers.get(rs1) as u128)
                            .wrapping_mul(self.registers.get(rs2) as u128);
                        let val = (prod >> 64) as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::MULHSU => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64 as i128;
                        let b = self.registers.get(rs2) as u64 as i128;
                        let val = ((a * b) >> 64) as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::DIV => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let num = self.registers.get(rs1) as i64;
                        let denom = self.registers.get(rs2) as i64;
                        let val = checked_div_i64(num, denom)?;
                        self.registers.set(rd, val as u64);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::DIVU => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let denom = self.registers.get(rs2);
                        if denom == 0 {
                            return Err(VMError::AssertionFailed);
                        }
                        let val = self.registers.get(rs1) / denom;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::REM => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let num = self.registers.get(rs1) as i64;
                        let denom = self.registers.get(rs2) as i64;
                        let val = checked_rem_i64(num, denom)?;
                        self.registers.set(rd, val as u64);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::REMU => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let denom = self.registers.get(rs2);
                        if denom == 0 {
                            return Err(VMError::AssertionFailed);
                        }
                        let val = self.registers.get(rs1) % denom;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SLT => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let lhs = self.registers.get(rs1) as i64;
                        let rhs = self.registers.get(rs2) as i64;
                        self.registers.set(rd, u64::from(lhs < rhs));
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SLTU => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let lhs = self.registers.get(rs1);
                        let rhs = self.registers.get(rs2);
                        self.registers.set(rd, u64::from(lhs < rhs));
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SEQ => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let lhs = self.registers.get(rs1);
                        let rhs = self.registers.get(rs2);
                        self.registers.set(rd, u64::from(lhs == rhs));
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::SNE => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let lhs = self.registers.get(rs1);
                        let rhs = self.registers.get(rs2);
                        self.registers.set(rd, u64::from(lhs != rhs));
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::CMOV => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let cond = bwide::rs2(instr);
                        if self.zk_mode && self.registers.tag(cond) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if self.registers.get(cond) != 0 {
                            let val = self.registers.get(src);
                            self.registers.set(rd, val);
                            let tag = self.zk_unary_tag(src);
                            self.zk_apply_tag(rd, tag);
                        }
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::NEG => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = self.registers.get(src).wrapping_neg();
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::NOT => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = !self.registers.get(src);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ROTL => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let amt = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(src, amt)?;
                        let sh = (self.registers.get(amt) & 0x3f) as u32;
                        let val = self.registers.get(src).rotate_left(sh);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ROTR => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let amt = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(src, amt)?;
                        let sh = (self.registers.get(amt) & 0x3f) as u32;
                        let val = self.registers.get(src).rotate_right(sh);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ROTL_IMM => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let sh = (bwide::imm8(instr) as u8 as u32) & 0x3f;
                        let val = self.registers.get(src).rotate_left(sh);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(WIDE_INSTRUCTION_LEN);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ROTR_IMM => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let sh = (bwide::imm8(instr) as u8 as u32) & 0x3f;
                        let val = self.registers.get(src).rotate_right(sh);
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::POPCNT => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = self.registers.get(src).count_ones() as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::CLZ => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = self.registers.get(src).leading_zeros() as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::CTZ => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = self.registers.get(src).trailing_zeros() as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ISQRT => {
                        let rd = bwide::rd(instr);
                        let src = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(src);
                        let val = self.registers.get(src);
                        let root = isqrt_u64(val);
                        self.registers.set(rd, root);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 6;
                        continue;
                    }
                    bwide::arithmetic::MIN => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64;
                        let b = self.registers.get(rs2) as i64;
                        self.registers
                            .set(rd, if a < b { a as u64 } else { b as u64 });
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::MAX => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64;
                        let b = self.registers.get(rs2) as i64;
                        self.registers
                            .set(rd, if a > b { a as u64 } else { b as u64 });
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ABS => {
                        let rd = bwide::rd(instr);
                        let rs = bwide::rs1(instr);
                        self.zk_require_public_trap_operands(&[rs])?;
                        let tag = self.zk_unary_tag(rs);
                        let v = self.registers.get(rs) as i64;
                        let abs = checked_abs_i64(v)?;
                        self.registers.set(rd, abs as u64);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::DIV_CEIL => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let num = self.registers.get(rs1) as i64;
                        let denom = self.registers.get(rs2) as i64;
                        let val = div_ceil_i64(num, denom)?;
                        self.registers.set(rd, val as u64);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 12;
                        continue;
                    }
                    bwide::arithmetic::GCD => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64;
                        let b = self.registers.get(rs2) as i64;
                        let g = gcd_i64(a, b);
                        self.registers.set(rd, g);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 12;
                        continue;
                    }
                    bwide::arithmetic::MEAN => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let a = self.registers.get(rs1) as i64;
                        let b = self.registers.get(rs2) as i64;
                        let sum = (a as i128) + (b as i128);
                        let avg = (sum / 2) as i64;
                        self.registers.set(rd, avg as u64);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 3;
                        continue;
                    }
                    bwide::arithmetic::ADDI => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(rs1);
                        let imm = i64::from(i16::from(bwide::imm8(instr)));
                        let val = (self.registers.get(rs1) as i64).wrapping_add(imm) as u64;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ANDI => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(rs1);
                        let imm = bwide::imm8(instr) as i64 as u64;
                        let val = self.registers.get(rs1) & imm;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::ORI => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(rs1);
                        let imm = bwide::imm8(instr) as i64 as u64;
                        let val = self.registers.get(rs1) | imm;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::XORI => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let tag = self.zk_unary_tag(rs1);
                        let imm = bwide::imm8(instr) as i64 as u64;
                        let val = self.registers.get(rs1) ^ imm;
                        self.registers.set(rd, val);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::arithmetic::CMOVI => {
                        let rd = bwide::rd(instr);
                        let cond = bwide::rs1(instr);
                        let imm = i64::from(bwide::imm8(instr)) as u64;
                        if self.zk_mode && self.registers.tag(cond) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if self.registers.get(cond) != 0 {
                            self.registers.set(rd, imm);
                            if self.zk_mode {
                                self.registers.set_tag(rd, false);
                            }
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::LDLIT => {
                        let rd = bwide::rd(instr);
                        let index = bwide::literal_index(instr);
                        let DecodedLiteral::Pointer(pointer) = self
                            .literal_table
                            .entries()
                            .get(index)
                            .copied()
                            .ok_or(VMError::InvalidMetadata)?
                        else {
                            return Err(VMError::InvalidMetadata);
                        };
                        self.registers.set(rd, pointer);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::LDI64 => {
                        let rd = bwide::rd(instr);
                        let index = bwide::literal_index(instr);
                        let DecodedLiteral::I64(value) = self
                            .literal_table
                            .entries()
                            .get(index)
                            .copied()
                            .ok_or(VMError::InvalidMetadata)?
                        else {
                            return Err(VMError::InvalidMetadata);
                        };
                        self.registers.set(rd, value);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::LOAD64 => {
                        let rd = bwide::rd(instr);
                        let base = bwide::rs1(instr);
                        let imm = i64::from(i16::from(bwide::imm8(instr)));
                        if self.zk_mode && self.registers.tag(base) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                        let value = self.memory.load_u64(addr)?;
                        let tag = self.memory_load_privacy_tag(addr, 8)?;
                        self.memory.diagnostic_classify_last_access(
                            addr,
                            8,
                            DiagnosticMemoryAccessKind::Read,
                            tag,
                        );
                        self.registers.set(rd, value);
                        if self.zk_mode {
                            self.registers.set_tag(rd, tag);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::LOAD128 => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let rd_lo = bwide::rd(instr);
                        let base = bwide::rs1(instr);
                        let rd_hi = bwide::rs2(instr);
                        if self.zk_mode && self.registers.tag(base) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if rd_lo >= 256 || rd_hi >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let addr = self.registers.get(base);
                        if !addr.is_multiple_of(16) {
                            return Err(VMError::MisalignedAccess { addr: addr as u32 });
                        }
                        let value = self.memory.load_u128(addr)?;
                        let tag = self.memory_load_privacy_tag(addr, 16)?;
                        self.memory.diagnostic_classify_last_access(
                            addr,
                            16,
                            DiagnosticMemoryAccessKind::Read,
                            tag,
                        );
                        self.registers.set(rd_lo, value as u64);
                        self.registers.set(rd_hi, (value >> 64) as u64);
                        if self.zk_mode {
                            self.registers.set_tag(rd_lo, tag);
                            self.registers.set_tag(rd_hi, tag);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::STORE64 => {
                        let base = bwide::rd(instr);
                        let rs = bwide::rs1(instr);
                        let imm = i64::from(i16::from(bwide::imm8(instr)));
                        if self.zk_mode && self.registers.tag(base) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let addr = (self.registers.get(base) as i64).wrapping_add(imm) as u64;
                        let value = self.registers.get(rs);
                        let tag = self.zk_mode && self.registers.tag(rs);
                        self.validate_memory_store_privacy(addr, 8, tag)?;
                        self.preflight_memory_store_privacy(addr, 8, tag)?;
                        self.memory.store_u64(addr, value)?;
                        self.record_memory_store_privacy(addr, 8, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::memory::STORE128 => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let base = bwide::rd(instr);
                        let rs_lo = bwide::rs1(instr);
                        let rs_hi = bwide::rs2(instr);
                        if self.zk_mode && self.registers.tag(base) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if rs_lo >= 256 || rs_hi >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let addr = self.registers.get(base);
                        if !addr.is_multiple_of(16) {
                            return Err(VMError::MisalignedAccess { addr: addr as u32 });
                        }
                        let lo = self.registers.get(rs_lo);
                        let hi = self.registers.get(rs_hi);
                        let tag = self.baseline_match_tags(rs_lo, rs_hi)?.unwrap_or(false);
                        self.validate_memory_store_privacy(addr, 16, tag)?;
                        self.preflight_memory_store_privacy(addr, 16, tag)?;
                        let value = ((hi as u128) << 64) | lo as u128;
                        self.memory.store_u128(addr, value)?;
                        self.record_memory_store_privacy(addr, 16, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::HALT => {
                        if self.strict_return_integrity {
                            return Err(VMError::AssertionFailed);
                        }
                        self.halted = true;
                        self.cycles += 1;
                        self.pc = self.pc.wrapping_add(length);
                        // Commit completed HALT work at the loop boundary before
                        // trace flushing, which may unwind while allocating.
                        continue;
                    }
                    bwide::crypto::SETVL => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        self.baseline.vector_length = setvl_length(bwide::rs2(instr))?;
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::VADD32 | bwide::crypto::VADD64 => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = VECTOR_BASE + bwide::rd(instr) * stride;
                        let rs = VECTOR_BASE + bwide::rs1(instr) * stride;
                        let rt = VECTOR_BASE + bwide::rs2(instr) * stride;
                        if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        if wide_op == bwide::crypto::VADD64 {
                            validate_vadd64_length(vl)?;
                            // Process full 128-bit chunks through the vector helpers when possible.
                            let mut lane = 0usize;
                            while lane + 4 <= vl {
                                let mut a = [0u32; 4];
                                let mut b = [0u32; 4];
                                for offset in 0..4 {
                                    let idx = lane + offset;
                                    if self.zk_mode {
                                        let a_tag = self.registers.tag(rs + idx);
                                        let b_tag = self.registers.tag(rt + idx);
                                        if a_tag != b_tag {
                                            return Err(VMError::PrivacyViolation);
                                        }
                                        self.registers.set_tag(rd + idx, a_tag);
                                    }
                                    a[offset] = self.registers.get(rs + idx) as u32;
                                    b[offset] = self.registers.get(rt + idx) as u32;
                                }
                                // Enforce pair tag alignment for zk mode (lo/hi halves must share a tag).
                                if self.zk_mode {
                                    for pair in 0..2 {
                                        let lo = lane + pair * 2;
                                        let hi = lo + 1;
                                        let tag_lo = self.registers.tag(rd + lo);
                                        let tag_hi = self.registers.tag(rd + hi);
                                        if tag_lo != tag_hi {
                                            return Err(VMError::PrivacyViolation);
                                        }
                                    }
                                }
                                let sum = vector::vadd64(a, b);
                                for (offset, value) in sum.iter().enumerate() {
                                    let idx = lane + offset;
                                    self.registers.set(rd + idx, u64::from(*value));
                                }
                                lane += 4;
                            }
                            // Handle any remaining pairs with the scalar fallback.
                            let pairs = (vl - lane) / 2;
                            for pair in 0..pairs {
                                let lo_idx = lane + pair * 2;
                                let hi_idx = lo_idx + 1;
                                let a_lo = self.registers.get(rs + lo_idx) & 0xffff_ffff;
                                let a_hi = self.registers.get(rs + hi_idx) & 0xffff_ffff;
                                let b_lo = self.registers.get(rt + lo_idx) & 0xffff_ffff;
                                let b_hi = self.registers.get(rt + hi_idx) & 0xffff_ffff;
                                if self.zk_mode {
                                    let a_lo_tag = self.registers.tag(rs + lo_idx);
                                    let a_hi_tag = self.registers.tag(rs + hi_idx);
                                    let b_lo_tag = self.registers.tag(rt + lo_idx);
                                    let b_hi_tag = self.registers.tag(rt + hi_idx);
                                    if a_lo_tag != a_hi_tag
                                        || b_lo_tag != b_hi_tag
                                        || a_lo_tag != b_lo_tag
                                    {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + lo_idx, a_lo_tag);
                                    self.registers.set_tag(rd + hi_idx, a_lo_tag);
                                }
                                let a = (a_hi << 32) | a_lo;
                                let b = (b_hi << 32) | b_lo;
                                let sum = a.wrapping_add(b);
                                self.registers.set(rd + lo_idx, sum & 0xffff_ffff);
                                self.registers.set(rd + hi_idx, sum >> 32);
                            }
                        } else {
                            let mut lane = 0usize;
                            while lane + 4 <= vl {
                                let mut a = [0u32; 4];
                                let mut b = [0u32; 4];
                                for offset in 0..4 {
                                    let idx = lane + offset;
                                    if self.zk_mode {
                                        let taga = self.registers.tag(rs + idx);
                                        let tagb = self.registers.tag(rt + idx);
                                        if taga != tagb {
                                            return Err(VMError::PrivacyViolation);
                                        }
                                        self.registers.set_tag(rd + idx, taga);
                                    }
                                    a[offset] = self.registers.get(rs + idx) as u32;
                                    b[offset] = self.registers.get(rt + idx) as u32;
                                }
                                let sum = vector::vadd32(a, b);
                                for (offset, value) in sum.iter().enumerate() {
                                    let idx = lane + offset;
                                    self.registers.set(rd + idx, u64::from(*value));
                                }
                                lane += 4;
                            }
                            for idx in lane..vl {
                                let a = self.registers.get(rs + idx) as u32;
                                let b = self.registers.get(rt + idx) as u32;
                                if self.zk_mode {
                                    let taga = self.registers.tag(rs + idx);
                                    let tagb = self.registers.tag(rt + idx);
                                    if taga != tagb {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, taga);
                                }
                                self.registers.set(rd + idx, u64::from(a.wrapping_add(b)));
                            }
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BEQ => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs);
                        let b = self.registers.get(rt);
                        let taken = a == b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BNE => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs);
                        let b = self.registers.get(rt);
                        let taken = a != b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BLT => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs) as i64;
                        let b = self.registers.get(rt) as i64;
                        let taken = a < b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BGE => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs) as i64;
                        let b = self.registers.get(rt) as i64;
                        let taken = a >= b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BLTU => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs);
                        let b = self.registers.get(rt);
                        let taken = a < b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::BGEU => {
                        let rs = bwide::rd(instr);
                        let rt = bwide::rs1(instr);
                        let offset = i64::from(bwide::imm8(instr));
                        if self.zk_mode && (self.registers.tag(rs) || self.registers.tag(rt)) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs);
                        let b = self.registers.get(rt);
                        let taken = a >= b;
                        if taken {
                            let byte_off = offset as i64 * 4;
                            self.pc = ((self.pc as i64) + byte_off) as u64;
                        } else {
                            self.pc = self.pc.wrapping_add(length);
                        }
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::JR => {
                        let rs = bwide::rd(instr);
                        if self.zk_mode && self.registers.tag(rs) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if self.strict_return_integrity {
                            return Err(VMError::AssertionFailed);
                        }
                        self.pc = self.registers.get(rs);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::JALR => {
                        let rd = bwide::rd(instr);
                        let rs = bwide::rs1(instr);
                        if self.zk_mode && self.registers.tag(rs) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let imm = i64::from(bwide::imm8(instr));
                        if self.strict_return_integrity && !(rd == 0 && rs == 1 && imm == 0) {
                            return Err(VMError::AssertionFailed);
                        }
                        let raw_target = self.registers.get(rs).wrapping_add_signed(imm);
                        let target =
                            ((raw_target.wrapping_sub(self.pc_alignment)) & !3) | self.pc_alignment;
                        if self.strict_return_integrity {
                            if let Some(expected) = self.contract_return_stack.last().copied() {
                                if target != expected {
                                    return Err(VMError::AssertionFailed);
                                }
                            } else {
                                if self.contract_outer_return_pc != Some(target) {
                                    return Err(VMError::AssertionFailed);
                                }
                                if target != self.memory.code_len() {
                                    return Err(VMError::AssertionFailed);
                                }
                            }
                            self.finish_call().map_err(baseline_call_fault)?;
                            self.contract_return_stack.pop();
                            if target == self.memory.code_len() {
                                // Contract hosts use end-of-code as the trusted
                                // outer-invocation return sentinel.
                                self.halted = true;
                            }
                        }
                        let return_pc = self.pc.wrapping_add(length);
                        self.registers.set(rd, return_pc);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = target;
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::JAL => {
                        let rd = bwide::rd(instr);
                        let imm = bwide::imm16(instr) as i64;
                        let return_pc = self.pc.wrapping_add(length);
                        if self.strict_return_integrity {
                            if rd != 0 && rd != 1 {
                                return Err(VMError::AssertionFailed);
                            }
                            if rd == 1 {
                                self.begin_child_call(((self.pc as i64) + (imm * 4)) as u64)
                                    .map_err(baseline_call_fault)?;
                                self.push_contract_return(return_pc);
                            }
                        }
                        self.registers.set(rd, return_pc);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::JMP => {
                        let imm = i64::from(bwide::imm24(instr));
                        self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                        self.cycles += 1;
                        continue;
                    }
                    bwide::control::JALS => {
                        let imm = i64::from(bwide::imm24(instr));
                        let return_pc = self.pc.wrapping_add(length);
                        self.begin_child_call(((self.pc as i64) + (imm * 4)) as u64)
                            .map_err(baseline_call_fault)?;
                        self.push_contract_return(return_pc);
                        self.registers.set(1, return_pc);
                        if self.zk_mode {
                            self.registers.set_tag(1, false);
                        }
                        self.pc = ((self.pc as i64) + (imm * 4)) as u64;
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::VAND => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = VECTOR_BASE + bwide::rd(instr) * stride;
                        let rs = VECTOR_BASE + bwide::rs1(instr) * stride;
                        let rt = VECTOR_BASE + bwide::rs2(instr) * stride;
                        if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            let mut b = [0u32; 4];
                            for offset in 0..4 {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let taga = self.registers.tag(rs + idx);
                                    let tagb = self.registers.tag(rt + idx);
                                    if taga != tagb {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, taga);
                                }
                                a[offset] = self.registers.get(rs + idx) as u32;
                                b[offset] = self.registers.get(rt + idx) as u32;
                            }
                            let out = vector::vand(a, b);
                            for (offset, value) in out.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }
                        for idx in lane..vl {
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            let a = self.registers.get(rs + idx) as u32;
                            let b = self.registers.get(rt + idx) as u32;
                            self.registers.set(rd + idx, u64::from(a & b));
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::VXOR => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = VECTOR_BASE + bwide::rd(instr) * stride;
                        let rs = VECTOR_BASE + bwide::rs1(instr) * stride;
                        let rt = VECTOR_BASE + bwide::rs2(instr) * stride;
                        if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            let mut b = [0u32; 4];
                            for offset in 0..4 {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let taga = self.registers.tag(rs + idx);
                                    let tagb = self.registers.tag(rt + idx);
                                    if taga != tagb {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, taga);
                                }
                                a[offset] = self.registers.get(rs + idx) as u32;
                                b[offset] = self.registers.get(rt + idx) as u32;
                            }
                            let out = vector::vxor(a, b);
                            for (offset, value) in out.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }
                        for idx in lane..vl {
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            let a = self.registers.get(rs + idx) as u32;
                            let b = self.registers.get(rt + idx) as u32;
                            self.registers.set(rd + idx, u64::from(a ^ b));
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::VOR => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = VECTOR_BASE + bwide::rd(instr) * stride;
                        let rs = VECTOR_BASE + bwide::rs1(instr) * stride;
                        let rt = VECTOR_BASE + bwide::rs2(instr) * stride;
                        if rd + vl > 256 || rs + vl > 256 || rt + vl > 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            let mut b = [0u32; 4];
                            for offset in 0..4 {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let taga = self.registers.tag(rs + idx);
                                    let tagb = self.registers.tag(rt + idx);
                                    if taga != tagb {
                                        return Err(VMError::PrivacyViolation);
                                    }
                                    self.registers.set_tag(rd + idx, taga);
                                }
                                a[offset] = self.registers.get(rs + idx) as u32;
                                b[offset] = self.registers.get(rt + idx) as u32;
                            }
                            let out = vector::vor(a, b);
                            for (offset, value) in out.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }
                        for idx in lane..vl {
                            if self.zk_mode {
                                let taga = self.registers.tag(rs + idx);
                                let tagb = self.registers.tag(rt + idx);
                                if taga != tagb {
                                    return Err(VMError::PrivacyViolation);
                                }
                                self.registers.set_tag(rd + idx, taga);
                            }
                            let a = self.registers.get(rs + idx) as u32;
                            let b = self.registers.get(rt + idx) as u32;
                            self.registers.set(rd + idx, u64::from(a | b));
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::VROT32 => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = VECTOR_BASE + bwide::rd(instr) * stride;
                        let rs = VECTOR_BASE + bwide::rs1(instr) * stride;
                        if rd + vl > 256 || rs + vl > 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let shift = (bwide::imm8(instr) as u8 & 0x1F) as u32;
                        let mut lane = 0usize;
                        while lane + 4 <= vl {
                            let mut a = [0u32; 4];
                            for (offset, value) in a.iter_mut().enumerate() {
                                let idx = lane + offset;
                                if self.zk_mode {
                                    let src_tag = self.registers.tag(rs + idx);
                                    self.registers.set_tag(rd + idx, src_tag);
                                }
                                *value = self.registers.get(rs + idx) as u32;
                            }
                            let rotated = vector::vrot32(a, shift);
                            for (offset, value) in rotated.iter().enumerate() {
                                let idx = lane + offset;
                                self.registers.set(rd + idx, u64::from(*value));
                            }
                            lane += 4;
                        }
                        for idx in lane..vl {
                            if self.zk_mode {
                                let src_tag = self.registers.tag(rs + idx);
                                self.registers.set_tag(rd + idx, src_tag);
                            }
                            let value = self.registers.get(rs + idx) as u32;
                            let rotated = value.rotate_left(shift) as u64;
                            self.registers.set(rd + idx, rotated);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::SHA256BLOCK => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let vl = self.baseline.vector_length.max(1);
                        let stride = vl;
                        let rd = bwide::rd(instr);
                        let ptr_reg = bwide::rs1(instr);
                        if self.zk_mode && self.registers.tag(ptr_reg) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let base = VECTOR_BASE + rd * stride;
                        let second = base + stride;
                        if second + stride > 256 || ptr_reg >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let state_lanes = stride.min(4);
                        if self.zk_mode
                            && (0..state_lanes).any(|lane| {
                                self.registers.tag(base + lane) || self.registers.tag(second + lane)
                            })
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        let addr = self.registers.get(ptr_reg);
                        self.ensure_public_memory(addr, 64)?;
                        let mut block = [0u8; 64];
                        self.memory.load_bytes(addr, &mut block)?;
                        if self.zk_trace_collection_enabled() {
                            for (i, b) in block.iter().enumerate() {
                                let (root, path) =
                                    self.memory.merkle_root_and_path(addr + i as u64)?;
                                self.mem_log.record(MemEvent::Load {
                                    addr: addr + i as u64,
                                    value: *b as u128,
                                    size: 1,
                                    path,
                                    root,
                                });
                            }
                        }
                        let mut state = [0u32; 8];
                        let first_lanes = state_lanes;
                        for (lane, slot) in state.iter_mut().take(first_lanes).enumerate() {
                            *slot = self.registers.get(base + lane) as u32;
                        }
                        let second_lanes = state_lanes;
                        for (lane, slot) in state.iter_mut().skip(4).take(second_lanes).enumerate()
                        {
                            *slot = self.registers.get(second + lane) as u32;
                        }
                        vector::sha256_compress(&mut state, &block);
                        for (lane, value) in state.iter().take(first_lanes).enumerate() {
                            self.registers.set(base + lane, u64::from(*value));
                        }
                        for (lane, value) in state.iter().skip(4).take(second_lanes).enumerate() {
                            self.registers.set(second + lane, u64::from(*value));
                        }
                        if self.zk_mode {
                            for lane in 0..state_lanes {
                                self.registers.set_tag(base + lane, false);
                                self.registers.set_tag(second + lane, false);
                            }
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::SHA3BLOCK => {
                        let rd = bwide::rd(instr);
                        let rs_state = bwide::rs1(instr);
                        let rs_block = bwide::rs2(instr);
                        if self.zk_mode
                            && (self.registers.tag(rd)
                                || self.registers.tag(rs_state)
                                || self.registers.tag(rs_block))
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        if rd >= 256 || rs_state >= 256 || rs_block >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let state_ptr = self.registers.get(rs_state);
                        let block_ptr = self.registers.get(rs_block);
                        let out_ptr = self.registers.get(rd);
                        self.ensure_public_memory(state_ptr, (25 * 8) as u64)?;
                        self.ensure_public_memory(block_ptr, 136)?;
                        let mut state = [0u64; 25];
                        let mut state_bytes = [0u8; 25 * 8];
                        self.memory.load_bytes(state_ptr, &mut state_bytes)?;
                        for i in 0..25 {
                            let mut lane = [0u8; 8];
                            lane.copy_from_slice(&state_bytes[i * 8..(i + 1) * 8]);
                            state[i] = u64::from_le_bytes(lane);
                        }
                        let mut block = [0u8; 136];
                        self.memory.load_bytes(block_ptr, &mut block)?;
                        crate::sha3::sha3_absorb_block(&mut state, &block);
                        let mut out_bytes = [0u8; 25 * 8];
                        for i in 0..25 {
                            out_bytes[i * 8..(i + 1) * 8].copy_from_slice(&state[i].to_le_bytes());
                        }
                        self.preflight_memory_store_privacy(
                            out_ptr,
                            out_bytes.len() as u64,
                            false,
                        )?;
                        self.memory.store_bytes(out_ptr, &out_bytes)?;
                        self.record_memory_store_privacy(out_ptr, out_bytes.len() as u64, false);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::AESENC => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs_state = bwide::rs1(instr);
                        let rs_key = bwide::rs2(instr);
                        if rd + 1 >= 256 || rs_state + 1 >= 256 || rs_key + 1 >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        if self.zk_mode {
                            let state_tag_lo = self.registers.tag(rs_state);
                            let state_tag_hi = self.registers.tag(rs_state + 1);
                            let key_tag_lo = self.registers.tag(rs_key);
                            let key_tag_hi = self.registers.tag(rs_key + 1);
                            if state_tag_lo != state_tag_hi
                                || key_tag_lo != key_tag_hi
                                || state_tag_lo != key_tag_lo
                            {
                                return Err(VMError::PrivacyViolation);
                            }
                            self.registers.set_tag(rd, state_tag_lo);
                            self.registers.set_tag(rd + 1, state_tag_lo);
                        }
                        let mut state = [0u8; 16];
                        let s_lo = self.registers.get(rs_state).to_le_bytes();
                        let s_hi = self.registers.get(rs_state + 1).to_le_bytes();
                        state[..8].copy_from_slice(&s_lo);
                        state[8..].copy_from_slice(&s_hi);
                        let mut rk = [0u8; 16];
                        let k_lo = self.registers.get(rs_key).to_le_bytes();
                        let k_hi = self.registers.get(rs_key + 1).to_le_bytes();
                        rk[..8].copy_from_slice(&k_lo);
                        rk[8..].copy_from_slice(&k_hi);
                        let out = crate::aes::aesenc(state, rk);
                        let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                        let hi = u64::from_le_bytes(out[8..].try_into().unwrap());
                        self.registers.set(rd, lo);
                        self.registers.set(rd + 1, hi);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::AESDEC => {
                        if unlikely(!self.baseline.vector_enabled) {
                            return Err(VMError::VectorExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs_state = bwide::rs1(instr);
                        let rs_key = bwide::rs2(instr);
                        if rd + 1 >= 256 || rs_state + 1 >= 256 || rs_key + 1 >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        if self.zk_mode {
                            let state_tag_lo = self.registers.tag(rs_state);
                            let state_tag_hi = self.registers.tag(rs_state + 1);
                            let key_tag_lo = self.registers.tag(rs_key);
                            let key_tag_hi = self.registers.tag(rs_key + 1);
                            if state_tag_lo != state_tag_hi
                                || key_tag_lo != key_tag_hi
                                || state_tag_lo != key_tag_lo
                            {
                                return Err(VMError::PrivacyViolation);
                            }
                            self.registers.set_tag(rd, state_tag_lo);
                            self.registers.set_tag(rd + 1, state_tag_lo);
                        }
                        let mut state = [0u8; 16];
                        let s_lo = self.registers.get(rs_state).to_le_bytes();
                        let s_hi = self.registers.get(rs_state + 1).to_le_bytes();
                        state[..8].copy_from_slice(&s_lo);
                        state[8..].copy_from_slice(&s_hi);
                        let mut rk = [0u8; 16];
                        let k_lo = self.registers.get(rs_key).to_le_bytes();
                        let k_hi = self.registers.get(rs_key + 1).to_le_bytes();
                        rk[..8].copy_from_slice(&k_lo);
                        rk[8..].copy_from_slice(&k_hi);
                        let out = crate::aes::aesdec(state, rk);
                        let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                        let hi = u64::from_le_bytes(out[8..].try_into().unwrap());
                        self.registers.set(rd, lo);
                        self.registers.set(rd + 1, hi);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::BLAKE2S => {
                        use iroha_crypto::blake2::{Blake2s256, Digest as _};
                        let rd = bwide::rd(instr);
                        let rs = bwide::rs1(instr);
                        if self.zk_mode && self.registers.tag(rs) {
                            return Err(VMError::PrivacyViolation);
                        }
                        if rd + 1 >= 256 || rs >= 256 {
                            return Err(VMError::RegisterOutOfBounds);
                        }
                        let ptr = self.registers.get(rs);
                        self.ensure_public_memory(ptr, 64)?;
                        let mut input = [0u8; 64];
                        self.memory.load_bytes(ptr, &mut input)?;
                        let mut hasher = Blake2s256::new();
                        hasher.update(input);
                        let digest = hasher.finalize();
                        let mut out = [0u8; 32];
                        out.copy_from_slice(&digest);
                        let lo = u64::from_le_bytes(out[..8].try_into().unwrap());
                        let hi = u64::from_le_bytes(out[8..16].try_into().unwrap());
                        self.registers.set(rd, lo);
                        self.registers.set(rd + 1, hi);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                            self.registers.set_tag(rd + 1, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::POSEIDON2 => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        if self.zk_mode && (self.registers.tag(rs1) || self.registers.tag(rs2)) {
                            // This legacy scalar gadget returns only 64 bits
                            // and is never a Secret<T> declassification path.
                            return Err(VMError::PrivacyViolation);
                        }
                        let a = self.registers.get(rs1);
                        let b = self.registers.get(rs2);
                        let res = crate::poseidon::poseidon2(a, b);
                        self.registers.set(rd, res);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::POSEIDON6 => {
                        let Some((rd, rs_base)) = bencoding::decode_poseidon6(instr) else {
                            return Err(VMError::DecodeError);
                        };
                        let rd = usize::from(rd);
                        let rs_base = usize::from(rs_base);
                        if self.zk_mode {
                            for index in 0..bwide::crypto::POSEIDON6_INPUTS {
                                if self.registers.tag(rs_base + index) {
                                    // This legacy scalar gadget returns only
                                    // 64 bits and cannot consume Secret<T>.
                                    return Err(VMError::PrivacyViolation);
                                }
                            }
                        }
                        let vals = std::array::from_fn(|index| {
                            debug_assert!(index < bwide::crypto::POSEIDON6_INPUTS);
                            self.registers.get(rs_base + index)
                        });
                        let res = crate::poseidon::poseidon6(vals);
                        self.registers.set(rd, res);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::ED25519BATCHVERIFY => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        if self.zk_mode && self.registers.tag(rs1) {
                            return Err(VMError::PrivacyViolation);
                        }
                        let mut fail_index = 0u64;
                        let request_ptr = self.registers.get(rs1);
                        let payload_len = match self.inspect_owned_public_tlv_header(request_ptr) {
                            Ok((payload_len, _)) => Some(payload_len),
                            Err(VMError::PrivacyViolation) => {
                                return Err(VMError::PrivacyViolation);
                            }
                            Err(error) if error.execution_deferral().is_some() => {
                                return Err(error);
                            }
                            Err(_) => None,
                        };
                        let ok = if let Some(payload_len) = payload_len {
                            if payload_len
                                > u64::try_from(
                                    crate::gas::baseline::ED25519_BATCH_MAX_PAYLOAD_BYTES,
                                )
                                .expect("batch payload limit always fits u64")
                            {
                                false
                            } else {
                                self.debit_gas(crate::gas::baseline::ed25519_batch_extra_gas(
                                    payload_len,
                                    0,
                                ))?;
                                let request = self
                                    .baseline_validate_public_crypto_tlv(request_ptr)?
                                    .and_then(|tlv_req| {
                                        (tlv_req.type_id_raw()
                                            == crate::pointer_abi::PointerType::NoritoBytes as u16)
                                            .then(|| {
                                                norito::decode_from_bytes_with_limits::<
                                                    crate::signature::Ed25519BatchRequest,
                                                >(
                                                    tlv_req.payload,
                                                    crate::gas::baseline::ED25519_BATCH_DECODE_LIMITS,
                                                )
                                                .ok()
                                            })
                                            .flatten()
                                    });
                                if let Some(request) = request {
                                    if request.entries.is_empty() {
                                        false
                                    } else if request.entries.len()
                                        > crate::gas::baseline::ED25519_BATCH_MAX_ENTRIES
                                    {
                                        fail_index = u64::try_from(request.entries.len())
                                            .map_err(|_| VMError::GasCostOverflow)?;
                                        false
                                    } else {
                                        let entry_count = u64::try_from(request.entries.len())
                                            .map_err(|_| VMError::GasCostOverflow)?;
                                        self.debit_gas(
                                            crate::gas::baseline::ed25519_batch_extra_gas(
                                                0,
                                                entry_count,
                                            ),
                                        )?;
                                        match crate::signature::verify_ed25519_batch(
                                            &request,
                                            crate::gas::baseline::ED25519_BATCH_MAX_ENTRIES,
                                        ) {
                                            Ok(()) => true,
                                            Err(err) => {
                                                fail_index = match err {
                                                    crate::signature::Ed25519BatchError::Empty => 0,
                                                    crate::signature::Ed25519BatchError::TooMany {
                                                        actual,
                                                        ..
                                                    } => u64::try_from(actual)
                                                        .map_err(|_| VMError::GasCostOverflow)?,
                                                    crate::signature::Ed25519BatchError::InvalidEntry {
                                                        index,
                                                    }
                                                    | crate::signature::Ed25519BatchError::SignatureFailed {
                                                        index,
                                                    } => u64::try_from(index)
                                                        .map_err(|_| VMError::GasCostOverflow)?,
                                                };
                                                false
                                            }
                                        }
                                    }
                                } else {
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        self.registers.set(rd, if ok { 1 } else { 0 });
                        self.registers.set(rs2, fail_index);
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                            self.registers.set_tag(rs2, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::ED25519VERIFY => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        if self.zk_mode
                            && (self.registers.tag(rd)
                                || self.registers.tag(rs1)
                                || self.registers.tag(rs2))
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        let msg_ptr = self.registers.get(rs1);
                        let sig_ptr = self.registers.get(rs2);
                        let pk_ptr = self.registers.get(rd);
                        let operands_ready =
                            self.baseline_preflight_signature_opcode_payloads([
                                msg_ptr, sig_ptr, pk_ptr,
                            ])?;
                        let ok = if operands_ready
                            && let (Some(tlv_msg), Some(tlv_sig), Some(tlv_pk)) = (
                                self.baseline_validate_public_crypto_tlv(msg_ptr)?,
                                self.baseline_validate_public_crypto_tlv(sig_ptr)?,
                                self.baseline_validate_public_crypto_tlv(pk_ptr)?,
                            ) {
                            let types_ok = tlv_msg.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_sig.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_pk.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16;
                            types_ok
                                && crate::signature::verify_signature(
                                    crate::signature::SignatureScheme::Ed25519,
                                    tlv_msg.payload,
                                    tlv_sig.payload,
                                    tlv_pk.payload,
                                )
                        } else {
                            false
                        };
                        self.registers.set(rd, if ok { 1 } else { 0 });
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::ECDSAVERIFY => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        if self.zk_mode
                            && (self.registers.tag(rd)
                                || self.registers.tag(rs1)
                                || self.registers.tag(rs2))
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        let msg_ptr = self.registers.get(rs1);
                        let sig_ptr = self.registers.get(rs2);
                        let pk_ptr = self.registers.get(rd);
                        let operands_ready =
                            self.baseline_preflight_signature_opcode_payloads([
                                msg_ptr, sig_ptr, pk_ptr,
                            ])?;
                        let ok = if operands_ready
                            && let (Some(tlv_msg), Some(tlv_sig), Some(tlv_pk)) = (
                                self.baseline_validate_public_crypto_tlv(msg_ptr)?,
                                self.baseline_validate_public_crypto_tlv(sig_ptr)?,
                                self.baseline_validate_public_crypto_tlv(pk_ptr)?,
                            ) {
                            let types_ok = tlv_msg.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_sig.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_pk.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16;
                            types_ok
                                && crate::signature::verify_signature(
                                    crate::signature::SignatureScheme::Secp256k1,
                                    tlv_msg.payload,
                                    tlv_sig.payload,
                                    tlv_pk.payload,
                                )
                        } else {
                            false
                        };
                        self.registers.set(rd, if ok { 1 } else { 0 });
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::DILITHIUMVERIFY => {
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        if self.zk_mode
                            && (self.registers.tag(rd)
                                || self.registers.tag(rs1)
                                || self.registers.tag(rs2))
                        {
                            return Err(VMError::PrivacyViolation);
                        }
                        let msg_ptr = self.registers.get(rs1);
                        let sig_ptr = self.registers.get(rs2);
                        let pk_ptr = self.registers.get(rd);
                        let operands_ready =
                            self.baseline_preflight_signature_opcode_payloads([
                                msg_ptr, sig_ptr, pk_ptr,
                            ])?;
                        let ok = if operands_ready
                            && let (Some(tlv_msg), Some(tlv_sig), Some(tlv_pk)) = (
                                self.baseline_validate_public_crypto_tlv(msg_ptr)?,
                                self.baseline_validate_public_crypto_tlv(sig_ptr)?,
                                self.baseline_validate_public_crypto_tlv(pk_ptr)?,
                            ) {
                            let types_ok = tlv_msg.type_id_raw()
                                == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_sig.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16
                                && tlv_pk.type_id_raw()
                                    == crate::pointer_abi::PointerType::Blob as u16;
                            types_ok
                                && crate::signature::verify_signature(
                                    crate::signature::SignatureScheme::MlDsa,
                                    tlv_msg.payload,
                                    tlv_sig.payload,
                                    tlv_pk.payload,
                                )
                        } else {
                            false
                        };
                        self.registers.set(rd, if ok { 1 } else { 0 });
                        if self.zk_mode {
                            self.registers.set_tag(rd, false);
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::ASSERT => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rs = bwide::rs1(instr);
                        self.zk_require_public_trap_operands(&[rs])?;
                        if self.zk_trace_collection_enabled() {
                            self.baseline.constraints.push(BaselineConstraint::Zero {
                                reg: rs,
                                cycle: self.cycles,
                            });
                        }
                        let value = self.registers.get(rs);
                        if value != 0 {
                            self.constraint_failed = true;
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::ASSERT_EQ => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        self.zk_require_public_trap_operands(&[rs1, rs2])?;
                        if self.zk_trace_collection_enabled() {
                            self.baseline.constraints.push(BaselineConstraint::Eq {
                                reg1: rs1,
                                reg2: rs2,
                                cycle: self.cycles,
                            });
                        }
                        let v1 = self.registers.get(rs1);
                        let v2 = self.registers.get(rs2);
                        if v1 != v2 {
                            self.constraint_failed = true;
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::FADD => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let v1 = self.registers.get(rs1);
                        let v2 = self.registers.get(rs2);
                        let res = field::add(v1, v2);
                        self.registers.set(rd, res);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::FSUB => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let v1 = self.registers.get(rs1);
                        let v2 = self.registers.get(rs2);
                        let res = field::sub(v1, v2);
                        self.registers.set(rd, res);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::FMUL => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs1 = bwide::rs1(instr);
                        let rs2 = bwide::rs2(instr);
                        let tag = self.baseline_match_tags(rs1, rs2)?;
                        let v1 = self.registers.get(rs1);
                        let v2 = self.registers.get(rs2);
                        let res = field::mul(v1, v2);
                        self.registers.set(rd, res);
                        self.zk_apply_tag(rd, tag);
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::FINV => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rd = bwide::rd(instr);
                        let rs = bwide::rs1(instr);
                        self.zk_require_public_trap_operands(&[rs])?;
                        let tag = self.registers.tag(rs);
                        let value = self.registers.get(rs);
                        match field::inv(value) {
                            Some(inv) => {
                                self.registers.set(rd, inv);
                                self.zk_apply_tag(rd, Some(tag));
                            }
                            None => {
                                self.constraint_failed = true;
                            }
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::zk::ASSERT_RANGE => {
                        if unlikely(!self.zk_mode) {
                            return Err(VMError::ZkExtensionDisabled);
                        }
                        let rs = bwide::rs1(instr);
                        let imm = bwide::imm8(instr) as u8;
                        self.zk_require_public_trap_operands(&[rs])?;
                        if self.zk_trace_collection_enabled() {
                            self.baseline.constraints.push(BaselineConstraint::Range {
                                reg: rs,
                                bits: imm,
                                cycle: self.cycles,
                            });
                        }
                        if imm <= 64 {
                            let mask = if imm == 64 {
                                u64::MAX
                            } else {
                                (1u64 << imm) - 1
                            };
                            let value = self.registers.get(rs);
                            if value & !mask != 0 {
                                self.constraint_failed = true;
                            }
                        } else {
                            self.constraint_failed = true;
                        }
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    bwide::crypto::PARBEGIN | bwide::crypto::PAREND => {
                        // Parallel sections are markers; no state change required.
                        self.pc = self.pc.wrapping_add(length);
                        self.cycles += 1;
                        continue;
                    }
                    _ => {
                        if crate::dev_env::debug_invalid_enabled() {
                            eprintln!(
                                "[invalid wide opcode] pc=0x{pc:08x} instr=0x{ins:08x} op_lo=0x{op_lo:02x} op_hi=0x{op_hi:02x}",
                                pc = self.pc,
                                ins = instr,
                                op_lo = opcode,
                                op_hi = wide_op,
                            );
                        }
                        return Err(VMError::InvalidOpcode((instr & 0xFFFF) as u16));
                    }
                }
            }
            if let Some((before, reservation)) = pending_cycles.take() {
                reservation.complete(self.cycles.checked_sub(before))?;
            }
            if let Some(step) = pending_step.take() {
                recorder
                    .as_deref_mut()
                    .expect("diagnostic step has recorder")
                    .finish_step(
                        step,
                        self.diagnostic_step_state(),
                        DiagnosticStepOutcome::Completed,
                    )?;
            }
            // If we exit the loop early, pad the trace so that prover and verifier
            // observe exactly `max_cycles` steps when zero‑knowledge mode is enabled.
            if self.zk_mode && self.max_cycles != 0 && self.cycles < self.max_cycles {
                // Append dummy cycles (treated as NOPs) until the target length is
                // reached. Each padded cycle still costs one unit of gas.
                let remaining = self.max_cycles - self.cycles;
                // Preserve the existing OOG outcome when padding cannot be paid.
                // Otherwise admit its complete root backing before any padding
                // cycles, shared allowance or gas are consumed.
                if self.gas_remaining >= remaining {
                    self.prepare_trace_padding(&invocation_trace, remaining)?;
                }
                let native_before_padding = (self.gas_remaining, self.cycles);
                if let Some(budget) = &cycle_budget {
                    // Padding contributes to architectural cycles even when the
                    // existing following gas check rejects the padded execution.
                    budget.reserve(remaining)?.complete(Some(remaining))?;
                }
                self.cycles = self.max_cycles;
                // Padding instructions still consume gas like NOPs (cost 1 each)
                if self.gas_remaining < remaining {
                    return Err(VMError::OutOfGas);
                } else {
                    self.gas_remaining -= remaining;
                }
                self.native_padding_completed(native_before_padding.0, native_before_padding.1);
                self.publish_trace_cycles(&invocation_trace, &mut last_logged_cycle)?;
            }
            self.commit_memory_after_run_if_needed();
            if let Some(error) = self.contract_abort_error.clone() {
                Err(error.into_vm_error())
            } else if self.constraint_failed {
                Err(VMError::AssertionFailed)
            } else {
                if self.strict_return_integrity {
                    self.call_result_word_count().map_err(baseline_call_fault)?;
                }
                Ok(())
            }
        })) {
            Ok(result) => result,
            Err(payload) => {
                self.finish_trace_invocation(invocation_trace);
                // Policy probes such as `allows_syscall` run before syscall
                // callback isolation. If any host hook panics, discard the
                // partial invocation proof before preserving the original
                // unwind behavior for the caller.
                self.abort_host_register_log_isolation(None);
                self.memory.call_frames.clear();
                self.captured_root_tables = None;
                std::panic::resume_unwind(payload);
            }
        };
        self.finish_trace_invocation(invocation_trace);
        let result = result.and_then(|()| {
            // A host may swallow a nested error and return from its last syscall.
            // Never accept such a parent after a shared reservation was refused.
            cycle_budget
                .as_ref()
                .map_or(Ok(()), |budget| budget.ensure_healthy())
        });
        if let Some(recorder) = recorder {
            if let Some(step) = pending_step.take() {
                let trap = result
                    .as_ref()
                    .err()
                    .map(Self::classify_trap)
                    .unwrap_or(VmTrapKind::Other);
                recorder.finish_step(
                    step,
                    self.diagnostic_step_state(),
                    DiagnosticStepOutcome::Trapped(trap),
                )?;
            }
            let state = self.diagnostic_step_state();
            let last_instruction_cycles = recorder
                .records()
                .last()
                .map_or(0, |step| step.after.cycles);
            recorder.finish_run(DiagnosticRunEnd {
                state,
                padding_cycles: if result.is_ok() {
                    state.cycles.saturating_sub(last_instruction_cycles)
                } else {
                    0
                },
                outcome: result.as_ref().map(|_| ()).map_err(Self::classify_trap),
            });
        }
        if let Err(err) = &result {
            self.memory.call_frames.clear();
            self.captured_root_tables = None;
            // Inline semantic context masks its register read; local refusals
            // retain only the returned error and never publish a diagnostic.
            self.capture_trap_at(err, fault_position);
        }
        result
    }

    /// Count trace and log events priced by the baseline execution-summary syscall.
    pub(crate) fn baseline_execution_summary_event_count(&self) -> u64 {
        self.execution_summary_event_count()
            .saturating_add(u64::try_from(self.baseline.constraints.len()).unwrap_or(u64::MAX))
    }

    /// Execution summary in the baseline layout: the C10 summary plus the constraint digest,
    /// count and failure flag the baseline bound.
    ///
    /// # Errors
    /// Defers locally when the memory root's Merkle storage cannot be allocated.
    pub fn baseline_execution_summary(
        &mut self,
    ) -> Result<crate::execution_summary::BaselineExecutionSummary, VMError> {
        let summary = self.execution_summary()?;
        Ok(crate::execution_summary::BaselineExecutionSummary {
            version: summary.version,
            code_hash: summary.code_hash,
            final_register_root: *self.registers.baseline_merkle_root().as_ref(),
            final_memory_root: summary.final_memory_root,
            output_hash: summary.output_hash,
            pc_trace_hash: summary.pc_trace_hash,
            delta_trace_hash: summary.delta_trace_hash,
            register_trace_hash: summary.register_trace_hash,
            constraint_hash: hash_constraints(&self.baseline.constraints),
            memory_log_hash: summary.memory_log_hash,
            register_log_hash: summary.register_log_hash,
            step_log_hash: summary.step_log_hash,
            cycles: summary.cycles,
            max_cycles: summary.max_cycles,
            gas_used: summary.gas_used,
            gas_remaining: summary.gas_remaining,
            pc_trace_len: summary.pc_trace_len,
            delta_trace_len: summary.delta_trace_len,
            register_trace_len: summary.register_trace_len,
            constraint_len: u64::try_from(self.baseline.constraints.len()).unwrap_or(u64::MAX),
            memory_log_len: summary.memory_log_len,
            register_log_len: summary.register_log_len,
            step_log_len: summary.step_log_len,
            zk_mode: summary.zk_mode,
            halted: summary.halted,
            constraint_failed: self.constraint_failed,
        })
    }

    /// Constraints recorded by the last baseline invocation.
    #[must_use]
    pub fn baseline_constraints(&self) -> &[BaselineConstraint] {
        &self.baseline.constraints
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_deltas_trace_all_baseline_registers_with_unchanged_execution() {
        use ivm_abi::baseline::{
            encode_header, encoding::wide, instruction::wide::arithmetic::ADDI,
        };
        let mut artifact = encode_header(&ProgramMetadata::default(), 0);
        for word in [
            wide::encode_ri(ADDI, 200, 0, 42),
            wide::encode_ri(ADDI, 255, 200, 1),
            wide::encode_halt(),
        ] {
            artifact.extend_from_slice(&word.to_le_bytes());
        }
        let run = |mode| {
            let mut vm = IVM::new(1000);
            vm.load_program_for_profile(&artifact, IvmProfileId::BASELINE_ABI_V1)
                .unwrap();
            vm.set_trace_mode(mode);
            vm.run().unwrap();
            vm
        };
        let plain = run(TraceMode::Off);
        let traced = run(TraceMode::DeltaRegisters);
        assert_eq!(traced.register(255), 43);
        assert_eq!(
            plain.registers.baseline_snapshot(),
            traced.registers.baseline_snapshot()
        );
        assert_eq!(
            (plain.pc, plain.cycles, plain.remaining_gas()),
            (traced.pc, traced.cycles, traced.remaining_gas())
        );
        let trace = traced.delta_register_trace();
        assert_eq!(trace.len(), 3);
        assert_eq!(
            trace.entry(0).unwrap().changes.len(),
            crate::registers::BASELINE_REGISTER_COUNT
        );
        assert_eq!(trace.entry(1).unwrap().changes, &[(200, 42, false)]);
        assert_eq!(trace.entry(2).unwrap().changes, &[(255, 43, false)]);
    }

    #[test]
    fn baseline_arithmetic_traps_and_vector_bounds_are_the_anchor_rules() {
        assert!(matches!(
            checked_div_i64(1, 0),
            Err(VMError::AssertionFailed)
        ));
        assert!(matches!(
            checked_div_i64(i64::MIN, -1),
            Err(VMError::AssertionFailed)
        ));
        assert!(matches!(
            checked_rem_i64(1, 0),
            Err(VMError::AssertionFailed)
        ));
        assert!(matches!(
            checked_abs_i64(i64::MIN),
            Err(VMError::AssertionFailed)
        ));
        assert_eq!(div_ceil_i64(7, 2).unwrap(), 4);
        assert_eq!(div_ceil_i64(-7, 2).unwrap(), -3);
        assert_eq!(setvl_length(0).unwrap(), 1);
        assert_eq!(setvl_length(64).unwrap(), 64);
        assert!(matches!(
            setvl_length(65),
            Err(VMError::InvalidVectorLength { vector_length: 65 })
        ));
        assert!(validate_vadd64_length(4).is_ok());
        assert!(validate_vadd64_length(3).is_err());
        assert_eq!(
            baseline_completed_instruction_cycles(bwide::arithmetic::ISQRT),
            6
        );
        assert_eq!(
            baseline_completed_instruction_cycles(bwide::arithmetic::GCD),
            12
        );
        assert_eq!(
            baseline_completed_instruction_cycles(bwide::arithmetic::ADD),
            1
        );
        assert_eq!(crate::registers::BASELINE_REGISTER_COUNT, 256);
    }

    #[test]
    fn shared_call_helper_faults_take_their_baseline_spelling() {
        assert!(matches!(
            baseline_call_fault(VMError::ControlFlowViolation),
            VMError::AssertionFailed
        ));
        assert!(matches!(
            baseline_call_fault(VMError::CallDepthExceeded),
            VMError::AssertionFailed
        ));
        assert!(matches!(
            baseline_call_fault(VMError::OutOfGas),
            VMError::OutOfGas
        ));
    }

    #[test]
    fn constraint_digest_and_machine_reset_follow_the_anchor() {
        let empty = hash_constraints(&[]);
        let one = hash_constraints(&[BaselineConstraint::Zero { reg: 3, cycle: 1 }]);
        assert_ne!(empty, one);
        let mut machine = BaselineMachine::default();
        assert_eq!(
            machine.vector_length,
            ivm_abi::baseline::DEFAULT_VECTOR_LENGTH
        );
        machine.install(
            IvmProfileId::BASELINE_ABI_V1,
            ivm_abi::baseline::mode::VECTOR,
            8,
        );
        assert!(machine.vector_enabled);
        assert_eq!(machine.vector_length, 8);
        machine.vector_length = 2;
        machine
            .constraints
            .push(BaselineConstraint::Zero { reg: 1, cycle: 0 });
        machine.reset();
        assert_eq!(machine.vector_length, 8);
        assert!(machine.constraints.is_empty());
        machine.install(IvmProfileId::C10_ABI_V1, 0xff, 9);
        assert!(!machine.vector_enabled);
        assert_eq!(
            machine.vector_length,
            ivm_abi::baseline::DEFAULT_VECTOR_LENGTH
        );
    }

    #[test]
    fn cycle_budget_reserves_completes_and_refuses_exhaustion() {
        let budget = VmCycleBudget::new(std::num::NonZeroU64::new(3).unwrap());
        assert_eq!(budget.limit(), 3);
        {
            let reservation = budget.shared.reserve(2).unwrap();
            assert_eq!(budget.remaining(), 1);
            reservation.complete(Some(2)).unwrap();
        }
        assert_eq!(budget.consumed(), 2);
        assert!(matches!(
            budget.shared.reserve(2),
            Err(VMError::ExceededMaxCycles)
        ));
        assert!(budget.exhausted());
        assert!(budget.is_open());
        let mut vm = IVM::new(10);
        vm.load_program(&{
            let mut program = ProgramMetadata::default().encode();
            program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
            program
        })
        .unwrap();
        let fresh = VmCycleBudget::new(std::num::NonZeroU64::new(3).unwrap());
        assert!(matches!(
            vm.run_with_host_and_cycle_budget(&mut crate::host::DefaultHost::new(), &fresh),
            Err(VMError::HostUnavailable)
        ));
    }
}
