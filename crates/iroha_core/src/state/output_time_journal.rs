//! Disposable, State-owned staging of actual Time retry and instruction mutations.
//!
//! This component has no apply, transaction-extraction or lifecycle-output API.
//! Its fixed record backing and captured source bytes retain their real pool
//! charges. They do not fund the State/storage mutation graph, authenticate a
//! callable, or admit complete State-derived terminal/aggregate output ceilings.
//! Before/after captures also have separate finite codec budgets; they are not
//! a complete pre-mutation funding reservation. Any local refusal drops the
//! entire owned overlay, including a refusal while capturing the after-image.
//! Instruction clones, guard lookups, mutation graphs, permission cleanup and event
//! backing are not funded by these captured-evidence charges. Calling Execute
//! preserves its existing guards/events/cleanup but does not authenticate the
//! enclosing executor/callback scope. The legacy all-table `decrease_repeats` sweep
//! is not intercepted. Register preserves its original deferred-byte policy; a
//! captured Action does not become canonical/schema-authorized by being recorded.
//! Production journal/output-V2 activation remains outside.

use crate::smartcontracts::Execute;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_data_model::{
    isi::error::InstructionExecutionError,
    prelude::{AccountId, Burn, EventFilterBox, Mint, Register, Trigger, Unregister},
    trigger::TriggerId,
};
use norito::core::EncodeWorkLimits;

use super::OutputTransaction;
use crate::smartcontracts::isi::triggers::{
    set::{ApplyingTimeSourceError, ApplyingTimeSourceWork, CapturedApplyingTimeSource},
    specialized::TimeTriggerRetryState,
};

/// A local staging refusal; the returned error never retains a commit capability.
#[derive(Debug, thiserror::Error)]
pub(super) enum TimeRetryJournalError {
    #[error("Time retry journal record backing refused")]
    RecordBacking(#[source] ChargedBufferError),
    #[error("Time retry journal original source capture refused")]
    Source(#[source] ApplyingTimeSourceError),
    #[error("Time retry journal fixed record capacity is exhausted")]
    RecordCapacity,
    #[error("Time retry journal encounter ordinal overflowed")]
    EncounterOverflow,
    #[error("Time retry journal lexical depth limit is exceeded")]
    DepthLimit,
    #[error("Time retry journal lexical scope aborted before completion")]
    ScopeAborted,
    #[error("Time retry journal lexical scope is not balanced")]
    UnbalancedScope,
    #[error("Time retry journal has no original transaction owner")]
    MissingTransaction,
    #[error("Time retry journal original Time source is absent")]
    MissingTimeSource,
    #[error("Time retry journal admitted mutation target disappeared")]
    MutationTargetDisappeared,
    #[error("Time journal original instruction execution refused")]
    Execution(#[source] InstructionExecutionError),
    #[error("Time journal registration input is outside the typed Time scope")]
    NonTimeRegistration,
    #[error("Time journal unregister left its original typed Time row present")]
    RemovalTargetSurvived,
    #[error("Time retry journal backing does not belong to its original pool")]
    ForeignPool,
}

/// Fixed actual operation associated with the captured original transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TimeJournalMutation {
    RetryState,
    Register,
    Unregister,
    MintRepetitions(u32),
    BurnRepetitions(u32),
}

// Only the three fixed entrypoints below can construct an instruction operation.
// There is no caller-provided mutation closure or supplied after-state.
#[derive(Clone, Copy)]
enum TimeInstructionMutation {
    Unregister,
    Mint(u32),
    Burn(u32),
}

impl TimeInstructionMutation {
    fn record_kind(self) -> TimeJournalMutation {
        match self {
            Self::Unregister => TimeJournalMutation::Unregister,
            Self::Mint(amount) => TimeJournalMutation::MintRepetitions(amount),
            Self::Burn(amount) => TimeJournalMutation::BurnRepetitions(amount),
        }
    }
}

/// Original observations around one actual fixed operation, in encounter order.
/// This is captured source evidence, not a validated lifecycle effect.
pub(super) struct OwnedTimeRetryTransition {
    ordinal: u64,
    depth: u32,
    kind: TimeJournalMutation,
    before: Option<CapturedApplyingTimeSource>,
    after: Option<CapturedApplyingTimeSource>,
}

impl OwnedTimeRetryTransition {
    pub(super) fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub(super) fn depth(&self) -> u32 {
        self.depth
    }

    /// `None` records only actual pre-operation absence in the typed Time table.
    /// Register still owns the separate global-ID duplicate decision.
    pub(super) fn before(&self) -> Option<&CapturedApplyingTimeSource> {
        self.before.as_ref()
    }

    /// `None` is an actual post-operation absence in the typed Time table.
    /// It is not global trigger absence or proof of an intermediate zero image.
    pub(super) fn after(&self) -> Option<&CapturedApplyingTimeSource> {
        self.after.as_ref()
    }

    pub(super) fn kind(&self) -> TimeJournalMutation {
        self.kind
    }
}

/// Owns the disposable overlay and all originals until either success or refusal.
/// Fields are private; callers cannot append snapshots or borrow mutable State.
pub(super) struct ApplyingTimeRetryJournal<'block, 'state, 'pool> {
    // Declaration order drops the original rollback owner before its evidence.
    output: OutputTransaction<'block, 'state>,
    records: ChargedBuffer<OwnedTimeRetryTransition>,
    pool: &'pool AllocationBudget,
    source_work: ApplyingTimeSourceWork,
    encode_limits: EncodeWorkLimits,
    next_ordinal: u64,
    depth: u32,
    max_depth: u32,
    // Facades return only a marker; the first original error stays with this
    // owner even if a nested caller ignores its marker or catches an unwind.
    first_error: Option<TimeRetryJournalError>,
}

impl<'block, 'state, 'pool> ApplyingTimeRetryJournal<'block, 'state, 'pool> {
    /// Reserve all fixed row slots before accepting any retry operation.
    /// Neither this cap nor the depth cap is a consensus/output admission proof.
    pub(super) fn new(
        output: OutputTransaction<'block, 'state>,
        pool: &'pool AllocationBudget,
        max_records: usize,
        max_depth: u32,
        source_work: u64,
        encode_limits: EncodeWorkLimits,
    ) -> Result<Self, TimeRetryJournalError> {
        if output.transaction.is_none() {
            drop(output);
            return Err(TimeRetryJournalError::MissingTransaction);
        }
        let records = match ChargedBuffer::new(max_records, pool) {
            Ok(records) => records,
            Err(original) => {
                drop(output);
                return Err(TimeRetryJournalError::RecordBacking(original));
            }
        };
        Ok(Self {
            output,
            records,
            pool,
            source_work: ApplyingTimeSourceWork::new(source_work),
            encode_limits,
            next_ordinal: 0,
            depth: 0,
            max_depth,
            first_error: None,
        })
    }

    /// Invoke only the genuine retry-state mutator, retaining its actual sources.
    /// Every failure consumes this owner; no surrounding caller can commit it.
    pub(super) fn set_retry_state(
        mut self,
        id: &TriggerId,
        retry: Option<TimeTriggerRetryState>,
    ) -> Result<Self, TimeRetryJournalError> {
        if let Some(original) = self.first_error.take() {
            return Err(self.refuse(original));
        }
        match self.set_retry_state_inner(id, retry) {
            Ok(()) => Ok(self),
            Err(original) => Err(self.refuse(original)),
        }
    }

    fn set_retry_state_inner(
        &mut self,
        id: &TriggerId,
        retry: Option<TimeTriggerRetryState>,
    ) -> Result<(), TimeRetryJournalError> {
        if self.depth > self.max_depth {
            return Err(TimeRetryJournalError::DepthLimit);
        }
        if self.records.as_slice().len() >= self.records.capacity() {
            return Err(TimeRetryJournalError::RecordCapacity);
        }
        if !self.records.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        let ordinal = self.next_ordinal;
        let next_ordinal = ordinal
            .checked_add(1)
            .ok_or(TimeRetryJournalError::EncounterOverflow)?;
        let before = self
            .output
            .transaction
            .as_ref()
            .ok_or(TimeRetryJournalError::MissingTransaction)?
            .world
            .triggers
            .capture_applying_time_source(id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?
            .ok_or(TimeRetryJournalError::MissingTimeSource)?;
        if !before.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        // The record slot and encounter are admitted before the actual mutator.
        // The before owner remains live during both mutation and after capture.
        self.next_ordinal = next_ordinal;
        let changed = self
            .output
            .transaction
            .as_mut()
            .ok_or(TimeRetryJournalError::MissingTransaction)?
            .world
            .triggers
            .set_time_trigger_retry_state(id, retry);
        if !changed {
            return Err(TimeRetryJournalError::MutationTargetDisappeared);
        }
        let after = self
            .output
            .transaction
            .as_ref()
            .ok_or(TimeRetryJournalError::MissingTransaction)?
            .world
            .triggers
            .capture_applying_time_source(id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?
            .ok_or(TimeRetryJournalError::MutationTargetDisappeared)?;
        if !after.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        // No clone, growth or caller-supplied observation is used here.
        self.records.push_reserved(OwnedTimeRetryTransition {
            ordinal,
            depth: self.depth,
            kind: TimeJournalMutation::RetryState,
            before: Some(before),
            after: Some(after),
        });
        Ok(())
    }

    /// Consume the genuine Register instruction without replacing its authority,
    /// global-ID duplicate checks, guards, metadata or deferred-executable policy.
    /// Only the typed Time before-image may be absent. Actual post-State supplies
    /// the after-image; neither the input Action nor absence is synthesized as it.
    pub(super) fn register_time(
        mut self,
        authority: &AccountId,
        instruction: Register<Trigger>,
    ) -> Result<Self, TimeRetryJournalError> {
        if let Some(original) = self.first_error.take() {
            return Err(self.refuse(original));
        }
        match self.register_time_inner(authority, instruction) {
            Ok(()) => Ok(self),
            Err(original) => Err(self.refuse(original)),
        }
    }

    fn register_time_inner(
        &mut self,
        authority: &AccountId,
        instruction: Register<Trigger>,
    ) -> Result<(), TimeRetryJournalError> {
        if self.depth > self.max_depth {
            return Err(TimeRetryJournalError::DepthLimit);
        }
        if self.records.as_slice().len() >= self.records.capacity() {
            return Err(TimeRetryJournalError::RecordCapacity);
        }
        if !self.records.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        let ordinal = self.next_ordinal;
        let next_ordinal = ordinal
            .checked_add(1)
            .ok_or(TimeRetryJournalError::EncounterOverflow)?;
        if !matches!(
            instruction.object().action().filter(),
            EventFilterBox::Time(_)
        ) {
            return Err(TimeRetryJournalError::NonTimeRegistration);
        }
        // The original instruction remains owned until actual Execute consumes it.
        // This temporary ID clone and Register's own cloned/decoded State graph
        // are not funded by the record/source backing charges.
        let id = instruction.object().id().clone();
        let transaction = self
            .output
            .transaction
            .as_ref()
            .ok_or(TimeRetryJournalError::MissingTransaction)?;
        let before = transaction
            .world
            .triggers
            .capture_applying_time_source(&id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?;
        if before
            .as_ref()
            .is_some_and(|source| !source.belongs_to(self.pool))
        {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        // Do not turn a missing typed row into global-ID absence, or reject a
        // present row as a fabricated duplicate. The genuine entrypoint decides.
        self.next_ordinal = next_ordinal;
        let transaction = self
            .output
            .transaction
            .as_mut()
            .ok_or(TimeRetryJournalError::MissingTransaction)?;
        instruction
            .execute(authority, transaction)
            .map_err(TimeRetryJournalError::Execution)?;
        let after = transaction
            .world
            .triggers
            .capture_applying_time_source(&id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?
            .ok_or(TimeRetryJournalError::MutationTargetDisappeared)?;
        if !after.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        transaction
            .world
            .triggers
            .require_applying_time_route(&id, &mut self.source_work)
            .map_err(TimeRetryJournalError::Source)?;
        self.records.push_reserved(OwnedTimeRetryTransition {
            ordinal,
            depth: self.depth,
            kind: TimeJournalMutation::Register,
            before,
            after: Some(after),
        });
        Ok(())
    }

    /// Execute the genuine unregister entrypoint inside this disposable owner.
    pub(super) fn unregister_time(
        self,
        authority: &AccountId,
        id: &TriggerId,
    ) -> Result<Self, TimeRetryJournalError> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Unregister)
    }

    pub(super) fn mint_time_repetitions(
        self,
        authority: &AccountId,
        id: &TriggerId,
        amount: u32,
    ) -> Result<Self, TimeRetryJournalError> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Mint(amount))
    }

    /// The genuine Burn entrypoint also removes its target when repetitions reach
    /// zero; its existing permission cleanup and event emission run unchanged.
    pub(super) fn burn_time_repetitions(
        self,
        authority: &AccountId,
        id: &TriggerId,
        amount: u32,
    ) -> Result<Self, TimeRetryJournalError> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Burn(amount))
    }

    fn execute_time_instruction(
        mut self,
        authority: &AccountId,
        id: &TriggerId,
        operation: TimeInstructionMutation,
    ) -> Result<Self, TimeRetryJournalError> {
        if let Some(original) = self.first_error.take() {
            return Err(self.refuse(original));
        }
        match self.execute_time_instruction_inner(authority, id, operation) {
            Ok(()) => Ok(self),
            Err(original) => Err(self.refuse(original)),
        }
    }

    fn execute_time_instruction_inner(
        &mut self,
        authority: &AccountId,
        id: &TriggerId,
        operation: TimeInstructionMutation,
    ) -> Result<(), TimeRetryJournalError> {
        if self.depth > self.max_depth {
            return Err(TimeRetryJournalError::DepthLimit);
        }
        if self.records.as_slice().len() >= self.records.capacity() {
            return Err(TimeRetryJournalError::RecordCapacity);
        }
        if !self.records.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        let ordinal = self.next_ordinal;
        let next_ordinal = ordinal
            .checked_add(1)
            .ok_or(TimeRetryJournalError::EncounterOverflow)?;
        let transaction = self
            .output
            .transaction
            .as_ref()
            .ok_or(TimeRetryJournalError::MissingTransaction)?;
        let before = transaction
            .world
            .triggers
            .capture_applying_time_source(id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?
            .ok_or(TimeRetryJournalError::MissingTimeSource)?;
        if !before.belongs_to(self.pool) {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        // Generic Set mutators dispatch through ids; a typed row alone is not
        // proof that that separate original routing entry still names Time.
        transaction
            .world
            .triggers
            .require_applying_time_route(id, &mut self.source_work)
            .map_err(TimeRetryJournalError::Source)?;
        self.next_ordinal = next_ordinal;
        let transaction = self
            .output
            .transaction
            .as_mut()
            .ok_or(TimeRetryJournalError::MissingTransaction)?;
        match operation {
            TimeInstructionMutation::Unregister => {
                Unregister::trigger(id.clone()).execute(authority, transaction)
            }
            TimeInstructionMutation::Mint(amount) => {
                Mint::trigger_repetitions(amount, id.clone()).execute(authority, transaction)
            }
            TimeInstructionMutation::Burn(amount) => {
                Burn::trigger_repetitions(amount, id.clone()).execute(authority, transaction)
            }
        }
        .map_err(TimeRetryJournalError::Execution)?;
        let after = transaction
            .world
            .triggers
            .capture_applying_time_source(id, &mut self.source_work, self.pool, self.encode_limits)
            .map_err(TimeRetryJournalError::Source)?;
        if after
            .as_ref()
            .is_some_and(|source| !source.belongs_to(self.pool))
        {
            return Err(TimeRetryJournalError::ForeignPool);
        }
        match (operation, after.is_some()) {
            (TimeInstructionMutation::Unregister, true) => {
                return Err(TimeRetryJournalError::RemovalTargetSurvived);
            }
            (TimeInstructionMutation::Mint(_), false) => {
                return Err(TimeRetryJournalError::MutationTargetDisappeared);
            }
            _ => {}
        }
        self.records.push_reserved(OwnedTimeRetryTransition {
            ordinal,
            depth: self.depth,
            kind: operation.record_kind(),
            before: Some(before),
            after,
        });
        Ok(())
    }

    /// A lexical staging scope, not an independent State savepoint/callback proof.
    /// The closure borrows a facade and cannot stash, replace or return this owner.
    pub(super) fn with_scope(
        mut self,
        operation: impl FnOnce(
            &mut TimeRetryScope<'_, 'block, 'state, 'pool>,
        ) -> Result<(), TimeRetryScopeRefused>,
    ) -> Result<Self, TimeRetryJournalError> {
        let refused = self.run_scope(operation).is_err();
        if let Some(original) = self.first_error.take() {
            return Err(self.refuse(original));
        }
        if refused {
            return Err(self.refuse(TimeRetryJournalError::ScopeAborted));
        }
        Ok(self)
    }

    fn remember_refusal(&mut self, original: TimeRetryJournalError) {
        if self.first_error.is_none() {
            self.first_error = Some(original);
        }
    }

    fn run_scope(
        &mut self,
        operation: impl FnOnce(
            &mut TimeRetryScope<'_, 'block, 'state, 'pool>,
        ) -> Result<(), TimeRetryScopeRefused>,
    ) -> Result<(), TimeRetryScopeRefused> {
        if self.first_error.is_some() {
            return Err(TimeRetryScopeRefused);
        }
        let Some(depth) = self.depth.checked_add(1) else {
            self.remember_refusal(TimeRetryJournalError::DepthLimit);
            return Err(TimeRetryScopeRefused);
        };
        if depth > self.max_depth {
            self.remember_refusal(TimeRetryJournalError::DepthLimit);
            return Err(TimeRetryScopeRefused);
        }
        let previous_depth = self.depth;
        self.depth = depth;
        let mut scope = TimeRetryScope {
            journal: self,
            previous_depth,
            completed: false,
        };
        let result = operation(&mut scope);
        if result.is_err() {
            scope
                .journal
                .remember_refusal(TimeRetryJournalError::ScopeAborted);
        }
        scope.completed = true;
        let refused = scope.journal.first_error.is_some();
        drop(scope);
        if refused {
            Err(TimeRetryScopeRefused)
        } else {
            Ok(())
        }
    }

    /// Keep the original overlay and captured originals together, still disposable.
    /// The finished owner intentionally cannot apply or release its transaction.
    pub(super) fn finish(
        self,
    ) -> Result<StagedTimeRetryEvidence<'block, 'state>, TimeRetryJournalError> {
        let mut owner = self;
        if let Some(original) = owner.first_error.take() {
            return Err(owner.refuse(original));
        }
        if owner.depth != 0 {
            return Err(owner.refuse(TimeRetryJournalError::UnbalancedScope));
        }
        Ok(StagedTimeRetryEvidence {
            _output: owner.output,
            records: owner.records,
        })
    }

    fn refuse(self, original: TimeRetryJournalError) -> TimeRetryJournalError {
        // Drop completes before the error reaches a caller which could continue
        // with its StateBlock. The original error contains no transaction owner.
        drop(self);
        original
    }
}

/// Copyable local signal; the journal retains the original refusal itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TimeRetryScopeRefused;

/// Borrowed lexical facade. No API yields its underlying owner or mutable State.
pub(super) struct TimeRetryScope<'scope, 'block, 'state, 'pool> {
    journal: &'scope mut ApplyingTimeRetryJournal<'block, 'state, 'pool>,
    previous_depth: u32,
    completed: bool,
}

impl<'block, 'state, 'pool> TimeRetryScope<'_, 'block, 'state, 'pool> {
    pub(super) fn set_retry_state(
        &mut self,
        id: &TriggerId,
        retry: Option<TimeTriggerRetryState>,
    ) -> Result<(), TimeRetryScopeRefused> {
        if self.journal.first_error.is_some() {
            return Err(TimeRetryScopeRefused);
        }
        if let Err(original) = self.journal.set_retry_state_inner(id, retry) {
            self.journal.remember_refusal(original);
            return Err(TimeRetryScopeRefused);
        }
        Ok(())
    }

    pub(super) fn register_time(
        &mut self,
        authority: &AccountId,
        instruction: Register<Trigger>,
    ) -> Result<(), TimeRetryScopeRefused> {
        if self.journal.first_error.is_some() {
            return Err(TimeRetryScopeRefused);
        }
        if let Err(original) = self.journal.register_time_inner(authority, instruction) {
            self.journal.remember_refusal(original);
            return Err(TimeRetryScopeRefused);
        }
        Ok(())
    }

    pub(super) fn unregister_time(
        &mut self,
        authority: &AccountId,
        id: &TriggerId,
    ) -> Result<(), TimeRetryScopeRefused> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Unregister)
    }

    pub(super) fn mint_time_repetitions(
        &mut self,
        authority: &AccountId,
        id: &TriggerId,
        amount: u32,
    ) -> Result<(), TimeRetryScopeRefused> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Mint(amount))
    }

    pub(super) fn burn_time_repetitions(
        &mut self,
        authority: &AccountId,
        id: &TriggerId,
        amount: u32,
    ) -> Result<(), TimeRetryScopeRefused> {
        self.execute_time_instruction(authority, id, TimeInstructionMutation::Burn(amount))
    }

    fn execute_time_instruction(
        &mut self,
        authority: &AccountId,
        id: &TriggerId,
        operation: TimeInstructionMutation,
    ) -> Result<(), TimeRetryScopeRefused> {
        if self.journal.first_error.is_some() {
            return Err(TimeRetryScopeRefused);
        }
        if let Err(original) = self
            .journal
            .execute_time_instruction_inner(authority, id, operation)
        {
            self.journal.remember_refusal(original);
            return Err(TimeRetryScopeRefused);
        }
        Ok(())
    }

    pub(super) fn with_scope(
        &mut self,
        operation: impl FnOnce(
            &mut TimeRetryScope<'_, 'block, 'state, 'pool>,
        ) -> Result<(), TimeRetryScopeRefused>,
    ) -> Result<(), TimeRetryScopeRefused> {
        self.journal.run_scope(operation)
    }
}

impl Drop for TimeRetryScope<'_, '_, '_, '_> {
    fn drop(&mut self) {
        if !self.completed {
            self.journal
                .remember_refusal(TimeRetryJournalError::ScopeAborted);
        }
        self.journal.depth = self.previous_depth;
    }
}

/// Read-only captured evidence retaining its uncommitted rollback owner.
/// Dropping this owner rolls back; reading records never authorizes publication.
pub(super) struct StagedTimeRetryEvidence<'block, 'state> {
    _output: OutputTransaction<'block, 'state>,
    records: ChargedBuffer<OwnedTimeRetryTransition>,
}

impl StagedTimeRetryEvidence<'_, '_> {
    pub(super) fn records(&self) -> &[OwnedTimeRetryTransition] {
        self.records.as_slice()
    }

    pub(super) fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.records.belongs_to(pool)
            && self.records.as_slice().iter().all(|record| {
                record
                    .before
                    .as_ref()
                    .is_none_or(|before| before.belongs_to(pool))
                    && record
                        .after
                        .as_ref()
                        .is_none_or(|after| after.belongs_to(pool))
            })
    }
}

#[cfg(test)]
#[path = "output_time_journal_tests.rs"]
mod tests;
