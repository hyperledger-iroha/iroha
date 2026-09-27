//! Deterministic byte reservations for the single execution-output collection.
//!
//! A reservation plan accounts for every prospective output's terminal row
//! before any invocation runs. Unused callback slots can be released only by
//! the execution owner after actual eligibility checks. This arithmetic neither
//! authenticates eligibility nor reserves host allocations or non-output wire
//! metadata. Core must supply frozen policy, bound matching before allocation,
//! admit a feasible plan, and fit all associated effects before applying State.
//! The complete executed-wire limit is checked separately by the block setter.

use super::execution_output::ExecutionOutputV1;

#[path = "output_terminal_ceilings.rs"]
mod terminal_ceilings;
pub use terminal_ceilings::ExecutionOutputTerminalCeilings;

/// Explicit applying-policy bounds; there is no unlimited or default policy.
///
/// Row accounting is the sum of complete canonical row frames. It is not a
/// claim that their sum equals the nested block wire length. The independent
/// complete-wire ceiling includes inputs, framing and all execution metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOutputLimits {
    /// Maximum number of top-level output rows in one carrier.
    pub max_outputs: u32,
    /// Maximum canonical frame length of any individual output row.
    pub max_output_bytes: u64,
    /// Maximum sum of canonical output-row frame lengths.
    pub max_total_output_bytes: u64,
    /// Maximum complete canonical executed `SignedBlockWire` length.
    pub max_executed_wire_bytes: u64,
}

impl ExecutionOutputLimits {
    /// Check finite, ordered limits before using the policy.
    ///
    /// # Errors
    /// Rejects zero limits or individual/aggregate limits above their parent.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_outputs == 0
            || self.max_output_bytes == 0
            || self.max_total_output_bytes == 0
            || self.max_executed_wire_bytes == 0
            || self.max_output_bytes > self.max_total_output_bytes
            || self.max_total_output_bytes > self.max_executed_wire_bytes
        {
            return Err("execution output limits must be finite, nonzero and ordered".into());
        }
        Ok(())
    }

    /// Check exact row-frame costs without constructing encoded output buffers.
    ///
    /// This is an allocated-value check, not an ingress allocation guard or
    /// pre-execution capacity reservation. It does not validate row authority.
    ///
    /// # Errors
    /// Rejects invalid policy, too many rows, encoding errors or exceeded bounds.
    pub fn validate_outputs(&self, outputs: &[ExecutionOutputV1]) -> Result<(), String> {
        self.validate()?;
        if u64::try_from(outputs.len()).map_err(|_| "output count exceeds u64")?
            > u64::from(self.max_outputs)
        {
            return Err("execution output count exceeds applying policy".into());
        }
        let mut total = 0_u64;
        for output in outputs {
            let bytes = output_bytes(output)?;
            if bytes > self.max_output_bytes {
                return Err("execution output row exceeds applying byte policy".into());
            }
            total = total
                .checked_add(bytes)
                .ok_or("execution output byte sum overflows u64")?;
            if total > self.max_total_output_bytes {
                return Err("execution output aggregate exceeds applying byte policy".into());
            }
        }
        Ok(())
    }
}

/// Canonical top-level execution phases, in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionOutputPhase {
    /// Exact immutable network-input prefix.
    Network,
    /// Isolated callbacks for actual prefix events and `BlockApproved`.
    Pipeline,
    /// Actual scheduled Time occurrences.
    Time,
}

impl ExecutionOutputPhase {
    fn index(self) -> usize {
        match self {
            Self::Network => 0,
            Self::Pipeline => 1,
            Self::Time => 2,
        }
    }

    fn of(output: &ExecutionOutputV1) -> Self {
        match output {
            ExecutionOutputV1::Network(_) => Self::Network,
            ExecutionOutputV1::Pipeline(_) => Self::Pipeline,
            ExecutionOutputV1::Time(_) => Self::Time,
        }
    }
}

/// Upper bounds established by the execution owner before invocation work.
///
/// Core must prove both the candidate count and terminal-row byte ceiling from
/// the actual source, trigger registry and applying policy. A caller-provided
/// scalar does not establish those facts. This type performs no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOutputPhaseReservation {
    /// Prospective invocations in this phase, including later revalidation skips.
    pub count: u32,
    /// Maximum complete canonical frame length of one bounded terminal row.
    pub terminal_bytes_per_output: u64,
}

/// Conservative carrier envelope derived before network-input selection.
///
/// Every selected network input may produce a pipeline event, followed by one
/// `BlockApproved` event. Each event may select every eligible existing pipeline
/// registration. Time has a separate finite invocation ceiling. Nested callbacks
/// stay in their enclosing row and do not create additional top-level outputs.
/// Core must derive the registry and terminal-byte bounds from authenticated
/// applying State and must reject registration/policy changes that make even one
/// admitted input impossible to carry. This descriptor grants no such authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOutputEnvelope {
    /// Existing pipeline registrations that could become eligible within a block.
    /// Include disabled registrations if earlier work can enable them.
    pub pipeline_candidates_per_event: u32,
    /// Applying-policy maximum for the bounded Time matcher.
    pub max_time_invocations: u32,
    /// Canonical terminal-row frame ceilings in Network, Pipeline, Time order.
    /// The Network bound must cover every admitted source index encoding.
    pub terminal_bytes: [u64; 3],
}

impl ExecutionOutputEnvelope {
    /// Determine the largest network batch whose complete fallback plan fits.
    ///
    /// Zero means the caller cannot admit a network input under this envelope;
    /// it is not authority to retry an already accepted input forever. Registry
    /// admission and policy transitions must preserve a positive minimum, while
    /// candidate assembly selects no more than this count and all other limits.
    ///
    /// # Errors
    /// Rejects invalid policy, false zero/row ceilings and arithmetic overflow.
    pub fn maximum_network_inputs(&self, limits: &ExecutionOutputLimits) -> Result<u32, String> {
        limits.validate()?;
        let [network, pipeline, time] = self.terminal_bytes;
        if network == 0
            || network > limits.max_output_bytes
            || (self.pipeline_candidates_per_event == 0) != (pipeline == 0)
            || (self.max_time_invocations == 0) != (time == 0)
            || pipeline > limits.max_output_bytes
            || time > limits.max_output_bytes
        {
            return Err("execution output envelope has invalid terminal ceilings".into());
        }
        let pipeline_count = u64::from(self.pipeline_candidates_per_event);
        let time_count = u64::from(self.max_time_invocations);
        let base_count = pipeline_count
            .checked_add(time_count)
            .ok_or("execution output envelope count overflows u64")?;
        let per_input_count = pipeline_count + 1;
        let base_pipeline_bytes = pipeline_count
            .checked_mul(pipeline)
            .ok_or("pipeline terminal byte envelope overflows u64")?;
        let base_bytes = time_count
            .checked_mul(time)
            .and_then(|bytes| bytes.checked_add(base_pipeline_bytes))
            .ok_or("internal terminal byte envelope overflows u64")?;
        let per_input_bytes = base_pipeline_bytes
            .checked_add(network)
            .ok_or("per-input terminal byte envelope overflows u64")?;
        let Some(count_room) = u64::from(limits.max_outputs).checked_sub(base_count) else {
            return Ok(0);
        };
        let Some(byte_room) = limits.max_total_output_bytes.checked_sub(base_bytes) else {
            return Ok(0);
        };
        u32::try_from((count_room / per_input_count).min(byte_room / per_input_bytes))
            .map_err(|_| "admissible network count exceeds u32".into())
    }

    /// Build the complete fallback plan for one selected network batch.
    ///
    /// This reserves the conservative event product without allocating that
    /// product. Actual nonmatching/revalidated callbacks release unused slots.
    ///
    /// # Errors
    /// Rejects infeasible selection or a count that cannot be represented.
    pub fn reservations(
        &self,
        network_inputs: u32,
        limits: &ExecutionOutputLimits,
    ) -> Result<[ExecutionOutputPhaseReservation; 3], String> {
        if network_inputs > self.maximum_network_inputs(limits)? {
            return Err("network batch exceeds its complete terminal output envelope".into());
        }
        let pipeline = u64::from(network_inputs)
            .checked_add(1)
            .and_then(|events| events.checked_mul(u64::from(self.pipeline_candidates_per_event)))
            .and_then(|count| u32::try_from(count).ok())
            .ok_or("pipeline candidate count exceeds u32")?;
        let counts = [network_inputs, pipeline, self.max_time_invocations];
        let phases = std::array::from_fn(|index| ExecutionOutputPhaseReservation {
            count: counts[index],
            terminal_bytes_per_output: if counts[index] == 0 {
                0
            } else {
                self.terminal_bytes[index]
            },
        });
        // Also reject an infeasible internal-only base when the computed maximum
        // is zero. No ownership or allocation is retained by this validation.
        ExecutionOutputBudget::new(*limits, phases)?;
        Ok(phases)
    }
}

/// One transient capacity owner for a complete carrier output sequence.
///
/// The owner is deliberately non-Clone and nonserializable. A live reservation
/// mutably borrows it, so no second invocation can spend the same capacity.
/// Dropping an unfinished reservation poisons the owner; it cannot silently
/// release an obligation and allow a partial carrier to be sealed.
#[derive(Debug)]
#[expect(
    missing_copy_implementations,
    reason = "capacity reservations must have one owner; copying would permit double spending"
)]
pub struct ExecutionOutputBudget {
    limits: ExecutionOutputLimits,
    remaining: [ExecutionOutputPhaseReservation; 3],
    remaining_terminal_bytes: u64,
    committed_bytes: u64,
    committed_outputs: u32,
    phase: usize,
    in_flight: bool,
    abandoned: bool,
}

impl ExecutionOutputBudget {
    /// Reserve all possible terminal rows before executing the first invocation.
    ///
    /// The array is Network, Pipeline, Time. This preflight must run before
    /// source acceptance/selection becomes irreversible. It cannot repair an
    /// already accepted atomic group whose minimum output cannot fit a carrier.
    ///
    /// # Errors
    /// Rejects invalid limits, overflowing costs and infeasible terminal plans.
    pub fn new(
        limits: ExecutionOutputLimits,
        phases: [ExecutionOutputPhaseReservation; 3],
    ) -> Result<Self, String> {
        limits.validate()?;
        let mut count = 0_u32;
        let mut bytes = 0_u64;
        for phase in phases {
            if (phase.count == 0) != (phase.terminal_bytes_per_output == 0)
                || phase.terminal_bytes_per_output > limits.max_output_bytes
            {
                return Err("terminal reservation has invalid count or row bound".into());
            }
            count = count
                .checked_add(phase.count)
                .ok_or("terminal output count overflows u32")?;
            bytes = bytes
                .checked_add(
                    u64::from(phase.count)
                        .checked_mul(phase.terminal_bytes_per_output)
                        .ok_or("terminal reservation byte product overflows u64")?,
                )
                .ok_or("terminal reservation byte sum overflows u64")?;
        }
        if count > limits.max_outputs || bytes > limits.max_total_output_bytes {
            return Err("complete terminal output plan cannot fit applying policy".into());
        }
        Ok(Self {
            limits,
            remaining: phases,
            remaining_terminal_bytes: bytes,
            committed_bytes: 0,
            committed_outputs: 0,
            phase: 0,
            in_flight: false,
            abandoned: false,
        })
    }

    fn check_phase(&self, phase: ExecutionOutputPhase) -> Result<(), String> {
        if self.abandoned || self.in_flight {
            return Err("execution output reservation is unfinished or abandoned".into());
        }
        let index = phase.index();
        if index < self.phase || self.remaining[..index].iter().any(|p| p.count != 0) {
            return Err("execution output phase bypasses unresolved prior obligations".into());
        }
        Ok(())
    }

    /// Release callback slots proven uninvoked by the actual execution owner.
    ///
    /// Network inputs cannot be skipped. This does not independently prove a
    /// callback was ineligible; canonical replay must reproduce every skip.
    ///
    /// # Errors
    /// Rejects network skips, phase inversions and release beyond reserved count.
    pub fn skip_uninvoked(
        &mut self,
        phase: ExecutionOutputPhase,
        count: u32,
    ) -> Result<(), String> {
        self.check_phase(phase)?;
        if phase == ExecutionOutputPhase::Network || count > self.remaining[phase.index()].count {
            return Err("cannot skip a network input or an unreserved callback".into());
        }
        let slot = &mut self.remaining[phase.index()];
        let bytes = u64::from(count)
            .checked_mul(slot.terminal_bytes_per_output)
            .ok_or("skipped output byte product overflows u64")?;
        self.remaining_terminal_bytes = self
            .remaining_terminal_bytes
            .checked_sub(bytes)
            .ok_or("skipped output bytes exceed remaining reservation")?;
        slot.count -= count;
        self.phase = phase.index();
        Ok(())
    }

    /// Acquire one invocation's exact bounded terminal before executing its body.
    ///
    /// The terminal must use the canonical output-limit rejection constructor.
    /// The returned reservation owns it, so oversized success never requires
    /// constructing its failure after capacity is exhausted.
    ///
    /// # Errors
    /// Rejects a malformed terminal, absent slot, phase inversion or false bound.
    pub fn begin(
        &mut self,
        terminal: ExecutionOutputV1,
    ) -> Result<ExecutionOutputReservation<'_>, String> {
        let phase = ExecutionOutputPhase::of(&terminal);
        self.check_phase(phase)?;
        if !terminal.is_output_limit_rejection() {
            return Err("reservation requires an exact bounded output-limit terminal".into());
        }
        let slot = self.remaining[phase.index()];
        let bytes = output_bytes(&terminal)?;
        if slot.count == 0 || bytes > slot.terminal_bytes_per_output {
            return Err("actual terminal exceeds its pre-admitted reservation".into());
        }
        self.remaining_terminal_bytes = self
            .remaining_terminal_bytes
            .checked_sub(slot.terminal_bytes_per_output)
            .ok_or("terminal reservation accounting underflows")?;
        self.remaining[phase.index()].count -= 1;
        self.phase = phase.index();
        // Keep this fact on the owner as well as the linear guard. Even a
        // forgotten guard must not make its invocation disappear at sealing.
        self.in_flight = true;
        Ok(ExecutionOutputReservation {
            owner: self,
            terminal: Some(terminal),
            terminal_bytes: bytes,
            finished: false,
        })
    }

    /// Require every admitted slot to have a terminal or a proven callback skip.
    ///
    /// Returns the exact retained row count and canonical row-frame byte sum.
    /// This says nothing about non-output block metadata or actual State work.
    ///
    /// # Errors
    /// Rejects abandoned execution or remaining unowned terminal obligations.
    pub fn finish(self) -> Result<(u32, u64), String> {
        if self.abandoned
            || self.in_flight
            || self.remaining.iter().any(|phase| phase.count != 0)
            || self.remaining_terminal_bytes != 0
        {
            return Err("cannot seal unresolved execution output obligations".into());
        }
        Ok((self.committed_outputs, self.committed_bytes))
    }
}

/// One pre-body terminal reservation; dropping it invalidates its carrier owner.
#[derive(Debug)]
pub struct ExecutionOutputReservation<'a> {
    owner: &'a mut ExecutionOutputBudget,
    terminal: Option<ExecutionOutputV1>,
    terminal_bytes: u64,
    finished: bool,
}

/// Decision made before applying an invocation's transactional effects.
#[derive(Debug)]
pub enum ReservedExecutionOutput {
    /// The actual output fits while preserving every later terminal reservation.
    Accepted(ExecutionOutputV1),
    /// Roll back the attempted invocation and retain this pre-reserved failure.
    ///
    /// This is deterministic output-capacity exhaustion, not an action defect
    /// or host allocation failure. It must not automatically quarantine a
    /// healthy callback because earlier work consumed the shared surplus.
    OutputLimit(ExecutionOutputV1),
}

impl ExecutionOutputReservation<'_> {
    /// Retain an actual Network rejection without changing its economic disposition.
    ///
    /// The execution owner has already dropped the rejected business overlay and
    /// settled its independently determined penalty/fee fragments. Oversized error
    /// details use a distinct bounded diagnostic, never healthy `OutputLimit`.
    /// This projection must not be used to choose fees, misconduct or work charges.
    ///
    /// # Errors
    /// Rejects a non-Network or successful row, rejected business side channels,
    /// an origin substitution, encoding failure or accounting contradiction.
    pub fn finish_network_rejection(
        mut self,
        actual: ExecutionOutputV1,
    ) -> Result<ExecutionOutputV1, String> {
        use super::execution_output::NETWORK_REJECTION_DIAGNOSTIC_OMITTED;
        use crate::transaction::error::TransactionRejectionReason;

        let ExecutionOutputV1::Network(network) = &actual else {
            return Err("rejection settlement requires a Network output".into());
        };
        if network.result.is_ok()
            || !network.result.batch_transfer_outcomes().is_empty()
            || !network.completions.is_empty()
        {
            return Err("rejection settlement retains successful business output".into());
        }
        let terminal = self
            .terminal
            .as_mut()
            .ok_or("terminal ownership disappeared")?;
        if !same_origin(terminal, &actual) {
            return Err("rejection differs from its reserved Network source".into());
        }
        let ExecutionOutputV1::Network(network) = terminal else {
            return Err("rejection has no Network terminal".into());
        };
        let Err(TransactionRejectionReason::LimitCheck(error)) = &mut network.result.0 else {
            return Err("Network terminal lost its bounded diagnostic".into());
        };
        // Reuse the already allocated terminal string. The shorter diagnostic
        // has the same enum/layout and cannot need more capacity than its owner.
        if NETWORK_REJECTION_DIAGNOSTIC_OMITTED.len() > error.reason.len() {
            return Err("rejection diagnostic exceeds its reserved terminal".into());
        }
        error.reason.clear();
        error.reason.push_str(NETWORK_REJECTION_DIAGNOSTIC_OMITTED);
        self.terminal_bytes = output_bytes(terminal)?;
        match self.finish(actual)? {
            ReservedExecutionOutput::Accepted(row) | ReservedExecutionOutput::OutputLimit(row) => {
                Ok(row)
            }
        }
    }

    /// Retain a real Pipeline or Time rejection without changing its disposition.
    ///
    /// Core must decide rollback and quarantine/retry effects from the original
    /// typed error first. A fitting row is retained exactly. Otherwise the two
    /// already allocated terminal reason strings hold a shorter, distinct
    /// diagnostic, with no new reservation and no healthy-overflow classification.
    /// Structural checks do not authenticate the action, execution or effects.
    ///
    /// # Errors
    /// Rejects a successful/non-internal row, healthy-overflow terminal, malformed
    /// failure root/completion, applied receipts, substituted origin, encoding
    /// error or a fallback larger than its preowned terminal. Refusal abandons
    /// this reservation and prevents the budget from sealing.
    pub fn finish_internal_rejection(
        mut self,
        actual: ExecutionOutputV1,
    ) -> Result<ExecutionOutputV1, String> {
        use super::execution_output::{
            INTERNAL_REJECTION_DIAGNOSTIC_OMITTED, TriggerFailureRootV1,
        };
        use crate::{
            events::trigger_completed::TriggerCompletedOutcome,
            transaction::error::TransactionRejectionReason,
        };

        let (trigger, root, completions) = match &actual {
            ExecutionOutputV1::Pipeline(output) => (
                &output.invocation.trigger,
                &output.failure_root,
                &output.completions,
            ),
            ExecutionOutputV1::Time(output) => (
                &output.invocation.trigger,
                &output.failure_root,
                &output.completions,
            ),
            ExecutionOutputV1::Network(_) => {
                return Err("internal rejection requires a Pipeline or Time output".into());
            }
        };
        if actual.result().is_ok()
            || !actual.result().batch_transfer_outcomes().is_empty()
            || !matches!(
                root,
                Some(
                    TriggerFailureRootV1::DeclaredInstructionProjection(_)
                        | TriggerFailureRootV1::ReturnedBeforeRollback(_)
                        | TriggerFailureRootV1::OmittedAfterRejection
                )
            )
            || !matches!(completions.as_slice(), [completion]
                if completion.callback_index == 0
                    && completion.trigger_id == trigger.trigger_id
                    && matches!(completion.outcome, TriggerCompletedOutcome::Failure(_)))
            || (matches!(root, Some(TriggerFailureRootV1::OmittedAfterRejection))
                && !actual.is_internal_rejection_diagnostic_omitted())
        {
            return Err("internal rejection retains invalid business output or diagnostics".into());
        }
        let terminal = self
            .terminal
            .as_mut()
            .ok_or("terminal ownership disappeared")?;
        if !same_origin(terminal, &actual) {
            return Err("internal rejection differs from its reserved invocation".into());
        }
        if !terminal.is_output_limit_rejection() {
            return Err("internal rejection lost its preowned terminal".into());
        }
        let (result, root, completions) = match terminal {
            ExecutionOutputV1::Pipeline(output) => (
                &mut output.result,
                &mut output.failure_root,
                &mut output.completions,
            ),
            ExecutionOutputV1::Time(output) => (
                &mut output.result,
                &mut output.failure_root,
                &mut output.completions,
            ),
            ExecutionOutputV1::Network(_) => {
                return Err("internal rejection has no internal terminal".into());
            }
        };
        let Err(TransactionRejectionReason::LimitCheck(error)) = &mut result.0 else {
            return Err("internal terminal lost its bounded result".into());
        };
        let [completion] = completions.as_mut_slice() else {
            return Err("internal terminal lost its root completion".into());
        };
        let TriggerCompletedOutcome::Failure(reason) = &mut completion.outcome else {
            return Err("internal terminal lost its bounded completion".into());
        };
        if INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.len() > error.reason.len()
            || INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.len() > reason.len()
        {
            return Err("internal rejection diagnostic exceeds its preowned strings".into());
        }
        error.reason.clear();
        error.reason.push_str(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED);
        reason.clear();
        reason.push_str(INTERNAL_REJECTION_DIAGNOSTIC_OMITTED);
        *root = Some(TriggerFailureRootV1::OmittedAfterRejection);
        let bytes = output_bytes(terminal)?;
        if bytes > self.terminal_bytes {
            return Err("internal rejection exceeds its preowned terminal bytes".into());
        }
        self.terminal_bytes = bytes;
        match self.finish(actual)? {
            ReservedExecutionOutput::Accepted(row) | ReservedExecutionOutput::OutputLimit(row) => {
                Ok(row)
            }
        }
    }

    /// Resolve an attempted invocation before its effects become applied.
    ///
    /// An oversized actual row is replaced by the already-owned terminal. The
    /// caller must roll back State and every receipt/witness/completion side
    /// channel before publishing `OutputLimit`. Allocation/encoding failure is
    /// a local refusal; it is never converted into a ledger rejection here.
    ///
    /// # Errors
    /// Rejects an origin substitution, encoding error or arithmetic contradiction.
    pub fn finish(mut self, actual: ExecutionOutputV1) -> Result<ReservedExecutionOutput, String> {
        let terminal = self
            .terminal
            .as_ref()
            .ok_or("terminal ownership disappeared")?;
        if !same_origin(terminal, &actual) {
            return Err("execution output differs from its reserved invocation".into());
        }
        let bytes = output_bytes(&actual)?;
        let total = self
            .owner
            .committed_bytes
            .checked_add(bytes)
            .and_then(|sum| sum.checked_add(self.owner.remaining_terminal_bytes));
        let fits = bytes <= self.owner.limits.max_output_bytes
            && total.is_some_and(|sum| sum <= self.owner.limits.max_total_output_bytes);
        let (output, selected_bytes) = if fits {
            (ReservedExecutionOutput::Accepted(actual), bytes)
        } else {
            (
                ReservedExecutionOutput::OutputLimit(
                    self.terminal
                        .take()
                        .ok_or("terminal ownership disappeared")?,
                ),
                self.terminal_bytes,
            )
        };
        self.owner.committed_bytes = self
            .owner
            .committed_bytes
            .checked_add(selected_bytes)
            .ok_or("retained output byte sum overflows u64")?;
        self.owner.committed_outputs = self
            .owner
            .committed_outputs
            .checked_add(1)
            .ok_or("retained output count overflows u32")?;
        self.owner.in_flight = false;
        self.finished = true;
        Ok(output)
    }
}

impl Drop for ExecutionOutputReservation<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.owner.abandoned = true;
        }
    }
}

fn output_bytes(output: &ExecutionOutputV1) -> Result<u64, String> {
    u64::try_from(norito::canonical_frame_len(output).map_err(|error| error.to_string())?)
        .map_err(|_| "canonical output frame length exceeds u64".into())
}

fn same_origin(left: &ExecutionOutputV1, right: &ExecutionOutputV1) -> bool {
    match (left, right) {
        (ExecutionOutputV1::Network(left), ExecutionOutputV1::Network(right)) => {
            left.input_index == right.input_index
        }
        (ExecutionOutputV1::Pipeline(left), ExecutionOutputV1::Pipeline(right)) => {
            left.invocation == right.invocation
        }
        (ExecutionOutputV1::Time(left), ExecutionOutputV1::Time(right)) => {
            left.invocation == right.invocation
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "output_budget_tests.rs"]
mod tests;
