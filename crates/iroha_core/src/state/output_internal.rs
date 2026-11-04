//! One rollback and retention boundary for actual Pipeline and Time invocations.
//! Failure policy uses the real rejection before bounded diagnostic projection.

use super::*;
use crate::smartcontracts::isi::triggers::{
    TRIGGER_ENABLED_METADATA_KEY, progress,
    set::{
        SetReadOnly,
        invocation_identity::{pipeline_trigger_use_v1, time_trigger_use_v1},
    },
    specialized::{LoadedActionTrait, TimeTriggerRetryState},
};
use crate::state::StateReadOnly;
use crate::state::contract_event_journal::DrainedContractEvents;
use iroha_data_model::{
    ValidationFail,
    block::execution_output::{
        INTERNAL_REJECTION_DIAGNOSTIC_OMITTED, InvocationCompletionV1, PipelineExecutionOutputV1,
        PipelineInvocationV1, TimeExecutionOutputV1, TimeInvocationV1, TriggerFailureRootV1,
        TriggerUseV1,
    },
    events::{
        EventBox,
        trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
    },
    transaction::{
        TransactionResult,
        error::{TransactionLimitError, TransactionRejectionReason},
        signed::ExecutionStep,
    },
};
use iroha_model_base::name::Name;
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;

pub(super) enum InternalInvocation {
    Pipeline(PipelineInvocationV1),
    Time(TimeInvocationV1),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum InternalOutputDisposition {
    Applied,
    OutputLimit,
    Rejected,
}

impl InternalInvocation {
    fn trigger(&self) -> &TriggerUseV1 {
        match self {
            Self::Pipeline(i) => &i.trigger,
            Self::Time(i) => &i.trigger,
        }
    }

    fn call(&self, proposal: HashOf<iroha_data_model::block::BlockHeader>) -> Result<Hash, String> {
        match self {
            Self::Pipeline(i) => i.execution_call_hash(proposal),
            Self::Time(i) => i.execution_call_hash(proposal),
        }
    }

    fn terminal(&self) -> ExecutionOutputV1 {
        match self {
            Self::Pipeline(i) => ExecutionOutputV1::pipeline_output_limit_rejection(i.clone()),
            Self::Time(i) => ExecutionOutputV1::time_output_limit_rejection(i.clone()),
        }
    }

    fn row(
        &self,
        result: TransactionResult,
        failure_root: Option<TriggerFailureRootV1>,
        completions: Vec<InvocationCompletionV1>,
    ) -> ExecutionOutputV1 {
        match self {
            Self::Pipeline(i) => ExecutionOutputV1::Pipeline(PipelineExecutionOutputV1 {
                invocation: i.clone(),
                result,
                failure_root,
                completions,
            }),
            Self::Time(i) => ExecutionOutputV1::Time(TimeExecutionOutputV1 {
                invocation: i.clone(),
                result,
                failure_root,
                completions,
            }),
        }
    }

    fn authenticate(
        &self,
        state: &mut StateBlock<'_>,
        height: u64,
    ) -> Result<(), ExecutionAttemptError<String>> {
        let id = &self.trigger().trigger_id;
        let actual = if state.lifecycle_output_enabled()? {
            let kind = match self {
                Self::Pipeline(_) => {
                    iroha_data_model::block::execution_output::TriggerLifecycleStateKindV1::Pipeline
                }
                Self::Time(_) => {
                    iroha_data_model::block::execution_output::TriggerLifecycleStateKindV1::Time
                }
            };
            let source = lifecycle::internal_source(state, id, height, kind)?;
            if !source.available() {
                return Err("internal policy source is unavailable".into());
            }
            source.identity().clone()
        } else {
            match self {
                Self::Pipeline(_) => pipeline_trigger_use_v1(&state.world.triggers, id, height)?,
                Self::Time(_) => time_trigger_use_v1(&state.world.triggers, id, height)?,
            }
        };
        if &actual != self.trigger() {
            return Err("internal action changed its invocation owner".into());
        }
        Ok(())
    }

    // The existing V1 caller has already fitted its rejected row. Preserve that
    // order while making the separately owned policy overlay disposable.
    fn apply_failure_policy(
        &self,
        state: &mut StateBlock<'_>,
        height: u64,
        now: u64,
    ) -> Result<(), ExecutionAttemptError<String>> {
        if let Some(policy) = self.prepare_failure_policy(state, height, now)? {
            policy.apply()?;
        }
        Ok(())
    }

    // Output-limit rollback retains only the independently owned native attempt debit.
    fn prepare_progress_policy<'block, 'state>(
        &self,
        state: &'block mut StateBlock<'state>,
        height: u64,
    ) -> Result<Option<OutputTransaction<'block, 'state>>, ExecutionAttemptError<String>> {
        if !matches!(self, Self::Time(_)) {
            return Ok(None);
        }
        self.authenticate(state, height)?;
        let id = &self.trigger().trigger_id;
        if state
            .world
            .triggers
            .time_triggers()
            .get(id)
            .is_none_or(|action| progress::remaining(action.metadata()) == 0)
        {
            return Ok(None);
        }
        let enabled = state.lifecycle_output_enabled()?;
        let maximum = state.callback_output_byte_limit();
        let mut policy = OutputTransaction::new(state)?;
        let tx = policy
            .transaction
            .as_mut()
            .ok_or("missing progress attempt transaction")?;
        lifecycle::arm(tx, enabled, maximum)?;
        tx.with_trigger_lifecycle_policy(Some(0),
            iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::ConsumeTimeProgress,
            |tx| { tx.world.triggers.consume_time_progress(id); });
        Ok(Some(policy))
    }

    fn consume_progress_attempt(
        &self,
        state: &mut StateBlock<'_>,
        height: u64,
    ) -> Result<(), ExecutionAttemptError<String>> {
        if let Some(policy) = self.prepare_progress_policy(state, height)? {
            policy.apply()?;
        }
        Ok(())
    }

    // Prepare only after the failed business overlay has been dropped. Persistent
    // action identity authenticates this separate quarantine/retry transaction.
    // Drop rolls the policy and witness back; preparation never commits them.
    // The successor producer drains and fits this policy's complete trace before
    // apply; predecessor replay retains its original V1 settlement ordering.
    fn prepare_failure_policy<'block, 'state>(
        &self,
        state: &'block mut StateBlock<'state>,
        height: u64,
        now: u64,
    ) -> Result<Option<OutputTransaction<'block, 'state>>, ExecutionAttemptError<String>> {
        self.prepare_failure_policy_with(state, height, now, |_| {})
    }

    // The committed services selection arms production policy capture before
    // its first mutation. This additional private hook only supplies component
    // test admission; ordinary callers supply the no-op closure above.
    fn prepare_failure_policy_with<'block, 'state>(
        &self,
        state: &'block mut StateBlock<'state>,
        height: u64,
        now: u64,
        prepare: impl FnOnce(&mut StateTransaction<'_, '_>),
    ) -> Result<Option<OutputTransaction<'block, 'state>>, ExecutionAttemptError<String>> {
        self.prepare_failure_policy_from_source(state, height, now, None, prepare)
    }

    // A production source was captured immediately before this invocation under
    // the same exclusive producer. Business failure has dropped the complete
    // overlay, so its prior identity remains authoritative without rehashing
    // immutable executables. Standalone callers must authenticate afresh above.
    fn prepare_failure_policy_from_source<'block, 'state>(
        &self,
        state: &'block mut StateBlock<'state>,
        height: u64,
        now: u64,
        source: Option<&lifecycle::InternalSource>,
        prepare: impl FnOnce(&mut StateTransaction<'_, '_>),
    ) -> Result<Option<OutputTransaction<'block, 'state>>, ExecutionAttemptError<String>> {
        if let Some(source) = source {
            if !source.available() || source.identity() != self.trigger() {
                return Err("failure policy changed its original bounded invocation source".into());
            }
        } else {
            self.authenticate(state, height)?;
        }
        let enabled_capture = state.lifecycle_output_enabled()?;
        let maximum = state.callback_output_byte_limit();
        let id = &self.trigger().trigger_id;
        match self {
            Self::Pipeline(_) => {
                let enabled: Name = TRIGGER_ENABLED_METADATA_KEY
                    .parse()
                    .map_err(|_| "invalid trigger enabled metadata key")?;
                let mut policy = OutputTransaction::new(state)?;
                let tx = policy
                    .transaction
                    .as_mut()
                    .ok_or("missing quarantine transaction")?;
                lifecycle::arm(tx, enabled_capture, maximum)?;
                prepare(tx);
                tx.world.triggers.set_lifecycle_origin(iroha_data_model::block::execution_output::TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy: iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::QuarantinePipeline,
                });
                if tx
                    .world
                    .triggers
                    .inspect_by_id_mut(id, |action| {
                        action
                            .metadata_mut()
                            .insert(enabled.clone(), Json::from(false));
                    })
                    .is_none()
                {
                    return Err("quarantine action disappeared".into());
                }
                Ok(Some(policy))
            }
            Self::Time(_) => {
                let action = state
                    .world
                    .triggers
                    .time_triggers()
                    .get(id)
                    .ok_or("retry action disappeared")?;
                let retry_policy = action.retry_policy;
                if retry_policy.is_none() && progress::remaining(action.metadata()) == 0 {
                    return Ok(None);
                }
                let used = action
                    .retry_state
                    .map_or(0, |s| s.retries_used)
                    .saturating_add(1);
                let mut policy = OutputTransaction::new(state)?;
                let tx = policy
                    .transaction
                    .as_mut()
                    .ok_or("missing retry transaction")?;
                lifecycle::arm(tx, enabled_capture, maximum)?;
                prepare(tx);
                let Some(retry_policy) = retry_policy else {
                    tx.with_trigger_lifecycle_policy(Some(0),
                        iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::ConsumeTimeProgress,
                        |tx| { tx.world.triggers.consume_time_progress(id); });
                    return Ok(Some(policy));
                };
                tx.world.triggers.set_lifecycle_origin(iroha_data_model::block::execution_output::TriggerLifecycleOriginV1::InvocationPolicy {
                    callback_index: 0,
                    policy: if used > retry_policy.max_retries.get() {
                        iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::ExhaustTimeRetry
                    } else { iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::AdvanceTimeRetry },
                });
                if used > retry_policy.max_retries.get() {
                    if !tx.world.triggers.remove(id) {
                        return Err("retry action disappeared".into());
                    }
                    crate::smartcontracts::isi::triggers::isi::remove_trigger_associated_permissions(tx, id);
                } else if !tx.world.triggers.advance_time_retry(
                    id,
                    TimeTriggerRetryState {
                        retries_used: used,
                        next_retry_at_ms: now.saturating_add(retry_policy.retry_after_ms.get()),
                    },
                ) {
                    return Err("retry action disappeared".into());
                }
                Ok(Some(policy))
            }
        }
    }
}

impl ExecutionOutputProducer<'_, '_, '_> {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "Applying consumes the authenticated invocation owner exactly once"
    )]
    pub(super) fn try_apply_internal(
        &mut self,
        invocation: InternalInvocation,
        event: EventBox,
        action: &impl LoadedActionTrait,
        mut source: Option<lifecycle::InternalSource>,
    ) -> Result<InternalOutputDisposition, ExecutionAttemptError<String>> {
        let height = self.source.header().height().get();
        let now = u64::try_from(self.source.header().creation_time().as_millis())
            .map_err(|_| "internal timestamp exceeds u64")?;
        if self.lifecycle {
            if source
                .as_ref()
                .is_none_or(|source| source.identity() != invocation.trigger())
            {
                return Err(
                    "selected internal invocation lacks its original bounded source".into(),
                );
            }
        } else {
            if source.is_some() {
                return Err("predecessor internal invocation carries a selected source".into());
            }
            invocation.authenticate(self.state, height)?;
        }
        let call = invocation.call(self.source.hash())?;
        let id = &invocation.trigger().trigger_id;
        if self.rows.len() >= self.rows.capacity() {
            return Err("internal row storage exhausted".into());
        }
        let maximum = self.state.callback_output_byte_limit();
        let reservation = self
            .budget
            .as_mut()
            .ok_or("output budget already consumed")?
            .begin(lifecycle::empty(invocation.terminal(), self.lifecycle))?;
        self.state.retain_fastpq_source_invocation(call)?;
        let mut attempt = OutputTransaction::new(self.state)?;
        let tx = attempt
            .transaction
            .as_mut()
            .ok_or("missing internal transaction")?;
        if let Some(source) = source.as_mut() {
            source.install(tx)?;
        }
        tx.tx_call_hash = Some(call);
        tx.current_tx_hash = None;
        tx.current_entrypoint_index = None;
        let root_dataspace = capture_internal_root_dataspace(tx)?;
        let original_source = OwnedExecutionSource::new(call, None, root_dataspace);
        tx.bind_original_fastpq_invocation_source(original_source)?;
        if source.as_ref().is_some_and(|source| !source.available()) {
            let actual = lifecycle::empty(invocation.terminal(), true);
            actual.validate_structure(height, &self.source)?;
            let row = match reservation.finish(actual)? {
                ReservedExecutionOutput::Accepted(row)
                | ReservedExecutionOutput::OutputLimit(row) => row,
            };
            drop(attempt);
            append_completions(&mut self.state.world.external_event_buf, call, &row)?;
            self.retain_completed_internal_source(original_source)?;
            self.rows.push(row);
            return Ok(InternalOutputDisposition::OutputLimit);
        }
        // Event routing is authenticated event data, not authority to route the
        // callback's writes. The immutable root owns this invocation's namespace.
        let generation = tx.world.triggers.registration_generation(id);
        let nft = match &invocation {
            InternalInvocation::Pipeline(_) => None,
            InternalInvocation::Time(i) => Some(StateBlock::time_trigger_nft_seq_base(
                height,
                usize::try_from(i.schedule_index).map_err(|_| "Time index exceeds host width")?,
            )),
        };
        let root = tx.execute_trigger(id, action.authority(), action.executable(), event, 0, nft);
        let mut execution = match root {
            Err(reason) => Err((reason, None)),
            Ok(step) => {
                // Pipeline consumes its root repeat before chained callbacks;
                // Time consumes it only after the complete invocation succeeds.
                if matches!(invocation, InternalInvocation::Pipeline(_))
                    && tx.world.triggers.registration_generation(id) == generation
                {
                    tx.decrease_trigger_repeats_and_cleanup(id, Some(0));
                }
                tx.execute_data_triggers_dfs_from(action.authority(), 1)
                    .map(|_| ())
                    .map_err(|reason| (reason, Some(step)))
            }
        };
        // Native fee settlement shares the internal invocation's disposable overlay.
        execution = execution
            .and_then(|()| crate::retail_fee::finalize(tx).map_err(|reason| (reason, None)));
        if height != 1 {
            // Genesis callbacks belong to the authenticated complete genesis result.
            // Every later invocation must be coherent before publishing its overlay.
            execution = execution.and_then(|()| {
                crate::resource_coherence::validate_final_policy(&tx.world, height).map_err(
                    |error| {
                        let rejection = match error {
                            ExecutionAttemptError::Rejected(error) => {
                                error.into_transaction_rejection()
                            }
                            ExecutionAttemptError::Deferred(original) => {
                                tx.world.record_execution_deferral(original);
                                TransactionRejectionReason::Validation(
                                    ValidationFail::InternalError(
                                        "local private-settlement policy read did not complete"
                                            .into(),
                                    ),
                                )
                            }
                        };
                        (rejection, None)
                    },
                )
            });
        }
        if tx.fastpq_source_quota.intrinsic_rejected()? {
            execution = Err((
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(
                    crate::fastpq::source_reservation::admission::SOURCE_INTRINSIC_REJECTION.into(),
                )),
                None,
            ));
        }
        tx.ensure_execution_ready()
            .map_err(ExecutionAttemptError::Deferred)?;
        if tx.tx_call_hash != Some(call)
            || tx.current_tx_hash.is_some()
            || tx.current_entrypoint_index.is_some()
            || tx.current_dataspace_id != Some(root_dataspace)
            || tx.world.current_dataspace_id != Some(root_dataspace)
        {
            return Err("internal callback changed its execution owner".into());
        }
        tx.require_original_fastpq_invocation_source(original_source)?;
        let work = CompletedOutputWork::capture(tx);
        // Resource refusal is local even if the business body also failed.
        let _ = lifecycle::output_limit(tx)?;
        if let Err((reason, root)) = execution {
            // Refused/incomplete journal custody is a local carrier failure, even
            // when execute_trigger reports an InternalError. Never quarantine it.
            tx.callback_journal.discard_rejected(call)?;
            tx.contract_event_journal.discard_rejected(call)?;
            let actual = rejected_row(&invocation, action.executable(), root, reason, maximum)?;
            actual.validate_structure(height, &self.source)?;
            drop(attempt);
            work.account(self.state);
            let (row, disposition) = if self.lifecycle {
                // Business has rolled back. Prepare the independently owned policy
                // and its full trace before retaining any applying output.
                let mut policy = invocation.prepare_failure_policy_from_source(
                    self.state,
                    height,
                    now,
                    source.as_ref(),
                    |_| {},
                )?;
                if let Some(policy) = policy.as_mut()
                    && lifecycle::output_limit(
                        policy
                            .transaction
                            .as_ref()
                            .ok_or("policy State owner absent")?,
                    )?
                {
                    return Err("native policy exceeded its fixed reserved capture envelope".into());
                }
                let mut custody;
                let actual = if let Some(policy) = policy.as_mut() {
                    let (row, ledger) = lifecycle::take(
                        policy
                            .transaction
                            .as_mut()
                            .ok_or("policy State owner absent")?,
                        actual,
                    )?;
                    custody = ledger;
                    row
                } else {
                    custody = None;
                    lifecycle::empty(actual, true)
                };
                actual.validate_structure(height, &self.source)?;
                // Its phase reservation already covers one complete compact
                // policy Update. Shorten diagnostics only; never lose policy.
                let row = reservation.finish_internal_rejection(actual)?;
                if custody.is_some() {
                    let destination = self
                        .lifecycle_custody
                        .as_mut()
                        .ok_or("policy custody owner absent")?;
                    if destination.as_slice().len() == destination.capacity() {
                        return Err("policy custody row capacity exhausted".into());
                    }
                    destination.push_reserved(custody.take().expect("checked custody presence"));
                }
                if let Some(policy) = policy {
                    policy.apply()?;
                }
                (row, InternalOutputDisposition::Rejected)
            } else {
                let row = reservation.finish_internal_rejection(actual)?;
                invocation.apply_failure_policy(self.state, height, now)?;
                (row, InternalOutputDisposition::Rejected)
            };
            append_completions(&mut self.state.world.external_event_buf, call, &row)?;
            self.retain_completed_internal_source(original_source)?;
            self.rows.push(row);
            return Ok(disposition);
        }
        if matches!(invocation, InternalInvocation::Time(_))
            && tx.world.triggers.registration_generation(id) == generation
        {
            tx.with_trigger_lifecycle_policy(Some(0),
                iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::ConsumeTimeProgress,
                |tx| { tx.world.triggers.consume_time_progress(id); });
            tx.with_trigger_lifecycle_policy(
                Some(0),
                iroha_data_model::block::execution_output::TriggerLifecyclePolicyV1::ClearTimeRetry,
                |tx| {
                    let _ = tx.world.triggers.set_time_trigger_retry_state(id, None);
                },
            );
            tx.decrease_trigger_repeats_and_cleanup(id, Some(0));
        }
        if tx
            .world
            .external_event_buf
            .iter()
            .any(|event| matches!(event, EventBox::TriggerCompleted(_)))
        {
            return Err("internal completions must come from the callback journal".into());
        }
        let mut receipts = core::mem::take(&mut tx.pending_batch_transfer_outcomes);
        let owned = receipts
            .remove(&HashOf::from_untyped_unchecked(call))
            .unwrap_or_default();
        if !receipts.is_empty() {
            return Err("internal receipts belong to a foreign call".into());
        }
        let callback_capture = tx.callback_journal.take(call)?;
        let event_capture = if matches!(callback_capture, DrainedCallbacks::OutputLimit) {
            tx.contract_event_journal.discard_rejected(call)?;
            DrainedContractEvents::OutputLimit
        } else {
            let budget = tx.execution_budget();
            tx.contract_event_journal.take(call, &budget)?
        };
        let (actual, overflow) = match (callback_capture, event_capture) {
            (
                DrainedCallbacks::Complete { steps, completions },
                DrainedContractEvents::Complete(events),
            ) => {
                let mut result = TransactionResult::new(Ok(steps));
                result.set_batch_transfer_outcomes(owned);
                result.set_contract_events(events);
                (invocation.row(result, None, completions), false)
            }
            _ => (invocation.terminal(), true),
        };
        let overflow = overflow || lifecycle::output_limit(tx)?;
        let unwrapped = actual;
        let mut custody;
        let actual;
        (actual, custody) = if overflow {
            (
                lifecycle::empty(invocation.terminal(), self.lifecycle),
                None,
            )
        } else {
            lifecycle::take(tx, unwrapped)?
        };
        actual.validate_structure(height, &self.source)?;
        if self.lifecycle
            && matches!(invocation, InternalInvocation::Time(_))
            && (overflow || !reservation.fits(&actual)?)
        {
            drop(attempt);
            drop(custody);
            let mut policy = invocation.prepare_progress_policy(self.state, height)?;
            let terminal = invocation.terminal();
            let (terminal, mut policy_custody) = if let Some(policy) = policy.as_mut() {
                let tx = policy
                    .transaction
                    .as_mut()
                    .ok_or("progress policy owner absent")?;
                if lifecycle::output_limit(tx)? {
                    return Err(
                        "native progress policy exceeded its reserved capture envelope".into(),
                    );
                }
                lifecycle::take(tx, terminal)?
            } else {
                (lifecycle::empty(terminal, true), None)
            };
            terminal.validate_structure(height, &self.source)?;
            let row = reservation.finish_output_limit_policy(terminal)?;
            if policy_custody.is_some() {
                let destination = self
                    .lifecycle_custody
                    .as_mut()
                    .ok_or("progress custody owner absent")?;
                if destination.as_slice().len() == destination.capacity() {
                    return Err("progress custody row capacity exhausted".into());
                }
                destination.push_reserved(policy_custody.take().expect("checked custody presence"));
            }
            if let Some(owned_policy) = policy.take() {
                owned_policy.apply()?;
            }
            drop(policy);
            append_completions(&mut self.state.world.external_event_buf, call, &row)?;
            work.account(self.state);
            self.retain_completed_internal_source(original_source)?;
            self.rows.push(row);
            return Ok(InternalOutputDisposition::OutputLimit);
        }
        let (row, apply) = match reservation.finish(actual)? {
            ReservedExecutionOutput::Accepted(row) => (row, !overflow),
            ReservedExecutionOutput::OutputLimit(row) => (row, false),
        };
        if apply {
            let tx = attempt
                .transaction
                .as_mut()
                .ok_or("missing internal transaction")?;
            append_completions(&mut tx.world.external_event_buf, call, &row)?;
            if custody.is_some() {
                let destination = self
                    .lifecycle_custody
                    .as_mut()
                    .ok_or("internal custody owner absent")?;
                if destination.as_slice().len() == destination.capacity() {
                    return Err("internal custody row capacity exhausted".into());
                }
                destination.push_reserved(custody.take().expect("checked custody presence"));
            }
            attempt.apply()?;
        } else {
            drop(attempt);
            drop(custody);
            invocation.consume_progress_attempt(self.state, height)?;
            append_completions(&mut self.state.world.external_event_buf, call, &row)?;
        }
        work.account(self.state);
        self.retain_completed_internal_source(original_source)?;
        self.rows.push(row);
        Ok(if apply {
            InternalOutputDisposition::Applied
        } else {
            InternalOutputDisposition::OutputLimit
        })
    }
}

fn append_completions(
    events: &mut Vec<EventBox>,
    call: Hash,
    row: &ExecutionOutputV1,
) -> Result<(), ExecutionAttemptError<String>> {
    events.try_reserve(row.completions().len()).map_err(|_| {
        ExecutionAttemptError::Deferred(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
    })?;
    for completion in row.completions() {
        events.push(
            TriggerCompletedEvent::new(
                completion.trigger_id.clone(),
                HashOf::from_untyped_unchecked(call),
                completion.callback_index,
                completion.outcome.clone(),
            )
            .into(),
        );
    }
    Ok(())
}

/// Size diagnostic copies before allocating them. These child payload sizes are
/// lower bounds, so exceeding the row ceiling is sufficient to omit the root.
fn rejected_row(
    invocation: &InternalInvocation,
    executable: &crate::smartcontracts::isi::triggers::set::ExecutableRef,
    returned: Option<ExecutionStep>,
    reason: TransactionRejectionReason,
    maximum: u64,
) -> Result<ExecutionOutputV1, ExecutionAttemptError<String>> {
    use crate::smartcontracts::isi::triggers::set::ExecutableRef;
    let limit = usize::try_from(maximum).map_err(|_| "diagnostic ceiling exceeds host width")?;
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let root_fits = if let Some(step) = returned.as_ref() {
        norito::core::encoded_payload_len(step).map_err(|e| e.to_string())? <= limit
    } else {
        match executable {
            ExecutableRef::Instructions(instructions) => {
                norito::core::encoded_payload_len(instructions).map_err(|e| e.to_string())? <= limit
            }
            ExecutableRef::Batch(items) => {
                let mut bytes = 0_usize;
                for item in items {
                    if let iroha_data_model::transaction::ExecutableBatchItem::Instruction(
                        instruction,
                    ) = item
                    {
                        let size = norito::core::encoded_payload_len(instruction)
                            .map_err(|e| e.to_string())?;
                        bytes = bytes.saturating_add(size);
                        if bytes > limit {
                            break;
                        }
                    }
                }
                bytes <= limit
            }
            ExecutableRef::Ivm(_) | ExecutableRef::ContractCall(_) => true,
        }
    };
    let diagnostic = if root_fits {
        bounded_diagnostic(&reason, limit)?
    } else {
        None
    };
    let (reason, root, diagnostic) = diagnostic.map_or_else(
        || {
            (
                TransactionRejectionReason::LimitCheck(TransactionLimitError {
                    reason: INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.to_owned(),
                }),
                TriggerFailureRootV1::OmittedAfterRejection,
                INTERNAL_REJECTION_DIAGNOSTIC_OMITTED.to_owned(),
            )
        },
        |diagnostic| {
            let root = returned.map_or_else(
                || {
                    TriggerFailureRootV1::DeclaredInstructionProjection(
                        StateTransaction::execution_step_from_executable(executable),
                    )
                },
                TriggerFailureRootV1::ReturnedBeforeRollback,
            );
            (reason, root, diagnostic)
        },
    );
    Ok(invocation.row(
        TransactionResult::new(Err(reason)),
        Some(root),
        vec![InvocationCompletionV1 {
            callback_index: 0,
            trigger_id: invocation.trigger().trigger_id.clone(),
            outcome: TriggerCompletedOutcome::Failure(diagnostic),
        }],
    ))
}

fn bounded_diagnostic(
    reason: &impl std::fmt::Display,
    maximum: usize,
) -> Result<Option<String>, ExecutionAttemptError<String>> {
    use std::fmt::Write;
    struct Writer {
        text: String,
        maximum: usize,
        overflow: bool,
        refused: bool,
    }
    impl Write for Writer {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if text.len() > self.maximum.saturating_sub(self.text.len()) {
                self.overflow = true;
                return Err(std::fmt::Error);
            }
            if self.text.try_reserve_exact(text.len()).is_err() {
                self.refused = true;
                return Err(std::fmt::Error);
            }
            self.text.push_str(text);
            Ok(())
        }
    }
    let mut writer = Writer {
        text: String::new(),
        maximum,
        overflow: false,
        refused: false,
    };
    let result = write!(&mut writer, "{reason}");
    if writer.refused {
        return Err(ExecutionAttemptError::Deferred(
            ivm::error::ExecutionDeferral::AllocationUnavailable.into(),
        ));
    }
    if result.is_err() && !writer.overflow {
        return Err("host cannot retain internal failure diagnostic".into());
    }
    Ok(if writer.overflow {
        None
    } else {
        Some(writer.text)
    })
}

#[cfg(test)]
mod tests {
    use super::bounded_diagnostic;

    #[test]
    fn diagnostic_exact_utf8_boundary_and_overflow_are_distinct_from_formatter_refusal() {
        let text = "é界";
        assert_eq!(
            bounded_diagnostic(&text, text.len()).unwrap().as_deref(),
            Some(text)
        );
        assert!(bounded_diagnostic(&text, text.len() - 1).unwrap().is_none());
        struct Refused;
        impl std::fmt::Display for Refused {
            fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        assert!(bounded_diagnostic(&Refused, 1024).is_err());
    }
}

/// Capture the immutable root's namespace for a fresh internal invocation.
/// The triggering event's route does not authorize the callback's own effects.
fn capture_internal_root_dataspace(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<iroha_model_base::topology::DataSpaceId, String> {
    if tx.current_dataspace_id.is_some() || tx.world.current_dataspace_id.is_some() {
        return Err("internal invocation already has a captured dataspace".into());
    }
    let scope =
        crate::executor::root_scope::execution_root_scope(tx).map_err(|error| error.to_string())?;
    let dataspace = scope.dataspace_id();
    tx.current_dataspace_id = Some(dataspace);
    tx.world.current_dataspace_id = Some(dataspace);
    Ok(dataspace)
}

#[cfg(test)]
#[path = "output_internal_root_scope_tests.rs"]
mod root_scope_tests;

#[cfg(test)]
#[path = "output_time_policy_tests.rs"]
mod time_policy_tests;
