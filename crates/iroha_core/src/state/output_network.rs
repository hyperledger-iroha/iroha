//! Actual Network execution and rejection settlement under one retained budget.
//!
//! Routes and authenticated QueuePlan validation instants are frozen before any
//! source runs. The caller still owes complete proposal/finality, duplicate/replay,
//! control, host-memory and common-wire admission before production integration.
//! Ordinary carriers and constructor-owned native sources share this executor.
//! Native admission retains its exact QueuePlan instant and typed fitting prefix.

use super::*;
use crate::{
    queue::{
        RoutingDecision, evaluate_policy_plan_with_nexus_and_world_at_block_height,
        routing_plan_from_execution_context,
    },
    smartcontracts::ivm::cache::IvmCache,
    tx::{
        AcceptedTransaction, execution_rejection_from_admission_failure,
        rejected_transaction_gas_is_accountable,
    },
};
use iroha_data_model::{
    ValidationFail,
    events::{EventBox, trigger_completed::TriggerCompletedEvent},
    transaction::{
        TransactionAdmissionIntent, TransactionResult, error::TransactionRejectionReason,
    },
};
use std::{borrow::Cow, time::Duration};

struct FrozenNetworkSource<'source> {
    pub(super) routing: RoutingDecision,
    admission: Option<Result<AcceptedTransaction<'source>, TransactionRejectionReason>>,
    quarantine: QuarantineAdmission,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuarantineAdmission {
    Normal,
    Selected,
    Overflow,
}

struct FrozenQuarantinePolicy {
    max_sources: usize,
    max_cycles: u64,
}

pub(super) struct FrozenNetworkSources<'source> {
    sources: Vec<FrozenNetworkSource<'source>>,
    pub(super) order: Option<Vec<usize>>,
    quarantine_policy: FrozenQuarantinePolicy,
}

/// Actual admission identity captured before a transaction can remove its sealed
/// commitment or compress its result to a bounded output-limit rejection.
pub(super) struct ExecutedNetworkSource {
    pub(super) stateless_accepted: bool,
    pub(super) authenticated_signed_replay_alias: Option<Hash>,
}

impl<'source> ExecutionOutputProducer<'_, '_, 'source> {
    /// Resolve actual Network sources once, preserving immutable output positions.
    pub(super) fn execute_network_sources(
        &mut self,
        genesis: Option<&crate::block::AuthenticatedGenesisOutputSource>,
    ) -> Result<(), String> {
        let result = (|| {
            if self.failed
                || self.network_sources.is_some()
                || self.network_resolved.iter().any(|done| *done)
            {
                return Err("Network execution is repeated or has already started".into());
            }
            self.network_sources = Some(self.freeze_network_sources(genesis)?);
            let order = self
                .network_sources
                .as_mut()
                .and_then(|plan| plan.order.take())
                .ok_or("Network execution order lost its owner")?;
            let mut cache = IvmCache::with_prepared_contract_cache(
                self.state.pipeline.cache_size,
                self.state.pipeline_ivm_prepared_cache.clone(),
            );
            for index in order {
                self.execute_network_source(index, &mut cache)?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }

    fn freeze_network_sources(
        &self,
        genesis: Option<&crate::block::AuthenticatedGenesisOutputSource>,
    ) -> Result<FrozenNetworkSources<'source>, String> {
        let ExecutionSource::Ordinary(source) = &self.source else {
            return Err("ordinary admission cannot replace native preflight".into());
        };
        let source = *source;
        let genesis_account = match (source.header().is_genesis(), genesis) {
            (true, Some(genesis)) if self.state.block_hashes.is_empty() => {
                Some(genesis.account_for(source)?)
            }
            (false, None) => None,
            _ => return Err("Network source lacks its exact initial genesis admission".into()),
        };
        let height = source.header().height().get();
        let now = source.header().creation_time();
        let now_ms = u64::try_from(now.as_millis()).map_err(|_| "Network timestamp exceeds u64")?;
        let count = source.network_entrypoint_count();
        let context = match (
            source.header().execution_context_hash(),
            source.execution_context(),
        ) {
            (None, None) => None,
            (Some(expected), Some(context))
                if context.has_current_version()
                    && HashOf::new(context) == expected
                    && context.native_lane_decisions.is_none()
                    && context.merge_entry.is_none()
                    && context.external.len() == count =>
            {
                Some(context)
            }
            _ => return Err("Network source has an invalid execution context".into()),
        };
        let parameters = self.state.world.parameters.get();
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(count)
            .map_err(|_| "host cannot retain frozen Network sources")?;
        for (index, input) in source.network_entrypoints().enumerate() {
            let embedded = context.map(|context| &context.external[index]);
            if embedded.is_some_and(|context| context.entrypoint_hash != input.hash()) {
                return Err("Network route belongs to another source".into());
            }
            let routing = if let Some(context) = embedded {
                RoutingDecision::new(context.lane_id, context.dataspace_id)
            } else {
                let borrowed = AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(input));
                evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &self.state.nexus,
                    &borrowed,
                    &self.state.world,
                    now_ms,
                    height,
                )
                .map_err(|error| {
                    format!("Network route cannot be frozen at index {index}: {error}")
                })?
                .coordinator_route()
            };
            let validation_time =
                if input.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced {
                    if let Some(context) = embedded {
                        let plan = routing_plan_from_execution_context(context)
                            .map_err(|error| error.to_string())?;
                        self.state
                            .pending_queue_plan_binding_for_execution_at_block_start(
                                input, &plan, height,
                            )?
                            .map_or(now, |binding| {
                                Duration::from_millis(binding.enqueue_timestamp_ms)
                            })
                    } else {
                        now
                    }
                } else {
                    now
                };
            let admission = if let Some(account) = genesis_account {
                let TransactionEntrypoint::External(signed) = input else {
                    return Err("authenticated genesis contains a non-signed Network source".into());
                };
                AcceptedTransaction::validate_genesis_with_now(
                    signed,
                    parameters.sumeragi().max_clock_drift(),
                    account,
                    &self.state.crypto,
                    validation_time,
                )
                .map(|()| AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(input)))
                .map_err(execution_rejection_from_admission_failure)
            } else {
                AcceptedTransaction::accept_borrowed_entrypoint_at_time(
                    input,
                    &self.state.network_id,
                    parameters.sumeragi().max_clock_drift(),
                    parameters.transaction(),
                    &self.state.crypto,
                    validation_time,
                )
                .map_err(execution_rejection_from_admission_failure)
            };
            let admission = if genesis_account.is_none() {
                if let Some(signed) = signed_source(input) {
                    if signed.creation_time() >= now {
                        Err(TransactionRejectionReason::Validation(
                            ValidationFail::NotPermitted(format!(
                                "transaction creation time {} is not earlier than block creation time {}",
                                signed.creation_time().as_millis(),
                                now.as_millis(),
                            )),
                        ))
                    } else {
                        #[cfg(feature = "telemetry")]
                        let telemetry = Some(self.state.telemetry);
                        #[cfg(not(feature = "telemetry"))]
                        let telemetry = None;
                        crate::tx::enforce_fraud_policy(
                            &self.state.fraud_monitoring,
                            signed.metadata(),
                            telemetry,
                            &crate::tx::LaneAssignment {
                                lane_id: routing.lane_id,
                                dataspace_id: routing.dataspace_id,
                                dataspace_catalog: &self.state.nexus.dataspace_catalog,
                            },
                        )
                        .and(admission)
                    }
                } else {
                    admission
                }
            } else {
                admission
            };
            sources.push(FrozenNetworkSource {
                routing,
                admission: Some(admission),
                quarantine: QuarantineAdmission::Normal,
            });
        }
        self.freeze_network_order(sources)
    }

    /// Freeze native admission and its fitting prefix before the first economic
    /// attempt. The exact QueuePlan enqueue instant owns TTL; the applying
    /// carrier owns placement, policy and effects.
    pub(super) fn freeze_native_network_sources(
        &self,
    ) -> Result<FrozenNetworkSources<'source>, crate::state::MergeLedgerCommitError> {
        use crate::state::MergeLedgerCommitError;
        let invalid = MergeLedgerCommitError::ExecutionBatchInvalid;
        let ExecutionSource::Native { groups, .. } = &self.source else {
            return Err(invalid(
                "native admission requires its consumed preflight".into(),
            ));
        };
        let groups = *groups;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(groups.len())
            .map_err(|_| invalid("host cannot retain native Network admission".into()))?;
        let mut reserved_gas = 0u64;
        for (index, group) in groups.iter().enumerate() {
            let input = &group.body().payload().input;
            let routing = input.routing_plan().map_err(invalid)?.coordinator_route();
            let mut admission = self.state.accept_native_group_entrypoint(group)?;
            if let Some(signed) = signed_source(&input.entrypoint) {
                #[cfg(feature = "telemetry")]
                let telemetry = Some(self.state.telemetry);
                #[cfg(not(feature = "telemetry"))]
                let telemetry = None;
                admission = crate::tx::enforce_fraud_policy(
                    &self.state.fraud_monitoring,
                    signed.metadata(),
                    telemetry,
                    &crate::tx::LaneAssignment {
                        lane_id: routing.lane_id,
                        dataspace_id: routing.dataspace_id,
                        dataspace_catalog: &self.state.nexus.dataspace_catalog,
                    },
                )
                .and(admission);
            }
            if let Ok(transaction) = &admission {
                match crate::queue::Queue::compute_proposal_gas_cost(transaction) {
                    Ok(cost)
                        if crate::gas::gas_components_fit_block_limit(
                            self.state.gas_limit_per_block,
                            [cost],
                        ) =>
                    {
                        reserved_gas = reserved_gas
                            .checked_add(cost)
                            .filter(|next| {
                                crate::gas::gas_components_fit_block_limit(
                                    self.state.gas_limit_per_block,
                                    [self.state.gas_used_in_block, *next],
                                )
                            })
                            .ok_or(MergeLedgerCommitError::ExecutionBatchFull {
                                fitting_prefix: index,
                                gas_limit: self.state.gas_limit_per_block,
                                gas_used: self.state.gas_used_in_block,
                            })?;
                    }
                    result => {
                        let reason = match result {
                            Ok(cost) => format!(
                                "native input gas reservation {cost} exceeds current whole-block limit {}",
                                self.state.gas_limit_per_block,
                            ),
                            Err(error) => {
                                format!("native input has invalid proposal gas accounting: {error}")
                            }
                        };
                        admission = Err(TransactionRejectionReason::LimitCheck(
                            iroha_data_model::transaction::error::TransactionLimitError { reason },
                        ));
                    }
                }
            }
            sources.push(FrozenNetworkSource {
                routing,
                admission: Some(admission),
                quarantine: QuarantineAdmission::Normal,
            });
        }
        self.freeze_network_order(sources).map_err(invalid)
    }

    /// Both source kinds share frozen reveal order and no-refill quarantine
    /// ranking. Later State changes cannot change selection or reorder effects.
    fn freeze_network_order(
        &self,
        mut sources: Vec<FrozenNetworkSource<'source>>,
    ) -> Result<FrozenNetworkSources<'source>, String> {
        let count = self.source.network_entrypoint_count();
        if sources.len() != count {
            return Err("Network admission lost a source position".into());
        }
        let quarantine_policy = FrozenQuarantinePolicy {
            max_sources: self.state.pipeline.quarantine_max_txs_per_block,
            max_cycles: self.state.pipeline.quarantine_tx_max_cycles,
        };
        let mut order = Vec::new();
        let mut reveal_positions = Vec::new();
        let mut sorted_reveals = Vec::new();
        let mut quarantine_candidates = Vec::new();
        for vector in [&mut order, &mut reveal_positions] {
            vector
                .try_reserve_exact(count)
                .map_err(|_| "host cannot retain Network order")?;
        }
        sorted_reveals
            .try_reserve_exact(count)
            .map_err(|_| "host cannot retain frozen reveal keys")?;
        quarantine_candidates
            .try_reserve_exact(count)
            .map_err(|_| "host cannot retain quarantine admission ranking")?;
        for (index, frozen) in sources.iter_mut().enumerate() {
            let input = self
                .source
                .network_entrypoint_at(index)
                .ok_or("Network ordering lost a source")?;
            if frozen.admission.as_ref().is_some_and(Result::is_ok)
                && signed_source(input).is_some_and(crate::tx::is_quarantine_transaction)
            {
                quarantine_candidates.push((input.hash(), index));
                frozen.quarantine = QuarantineAdmission::Overflow;
            }
            order.push(index);
            if let TransactionEntrypoint::SealedReveal(reveal) = input {
                reveal_positions.push(index);
                sorted_reveals.push((
                    crate::tx::sealed_reveal_execution_key(self.state, reveal),
                    index,
                ));
            }
        }
        sorted_reveals.sort_unstable();
        for (slot, (_, source)) in reveal_positions.into_iter().zip(sorted_reveals) {
            order[slot] = source;
        }
        quarantine_candidates.sort_unstable();
        for (_, index) in quarantine_candidates
            .into_iter()
            .take(quarantine_policy.max_sources)
        {
            sources[index].quarantine = QuarantineAdmission::Selected;
        }
        Ok(FrozenNetworkSources {
            sources,
            order: Some(order),
            quarantine_policy,
        })
    }

    /// A later Pipeline phase consumes this frozen route, never reroutes against
    /// State changed by an earlier Network source.
    pub(super) fn network_route(&self, index: usize) -> Option<RoutingDecision> {
        self.network_sources
            .as_ref()?
            .sources
            .get(index)
            .map(|source| source.routing)
    }

    pub(super) fn execute_network_source(
        &mut self,
        index: usize,
        cache: &mut IvmCache,
    ) -> Result<ExecutedNetworkSource, String> {
        let policy = &self
            .network_sources
            .as_ref()
            .ok_or("Network execution has no frozen quarantine policy")?
            .quarantine_policy;
        if policy.max_sources != self.state.pipeline.quarantine_max_txs_per_block
            || policy.max_cycles != self.state.pipeline.quarantine_tx_max_cycles
        {
            return Err("Network quarantine policy changed after source admission".into());
        }
        let input_index = u32::try_from(index).map_err(|_| "Network position exceeds u32")?;
        if self.network_resolved.get(index) != Some(&false) {
            return Err("Network source is missing or already resolved".into());
        }
        let input = self
            .source
            .network_entrypoint_at(index)
            .ok_or("Network source disappeared")?;
        let frozen = self
            .network_sources
            .as_mut()
            .and_then(|plan| plan.sources.get_mut(index))
            .ok_or("Network admission has no frozen source")?;
        let routing = frozen.routing;
        let admitted = frozen
            .admission
            .take()
            .ok_or("Network admission was consumed twice")?;
        let quarantine = frozen.quarantine;
        let disposition = ExecutedNetworkSource {
            stateless_accepted: admitted.is_ok(),
            authenticated_signed_replay_alias: (admitted.is_ok() && self.source.is_native())
                .then(|| {
                    crate::tx::authenticated_signed_replay_alias(self.state, input).map(Hash::from)
                })
                .flatten(),
        };
        let reservation = self
            .budget
            .as_mut()
            .ok_or("output budget already consumed")?
            .begin(ExecutionOutputV1::network_output_limit_rejection(
                input_index,
            ))?;
        let gas_before = self.state.gas_used_in_block;
        let gas_limit = self.state.gas_limit_per_block;
        let mut attempt = OutputTransaction::new(self.state);
        let transaction = attempt
            .transaction
            .as_mut()
            .ok_or("Network attempt is absent")?;
        bind_source(transaction, input, input_index, routing);
        let mut result = match admitted {
            Ok(_) if quarantine == QuarantineAdmission::Overflow => {
                Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted("quarantine overflow".into()),
                ))
            }
            Ok(accepted) => StateBlock::execute_accepted_transaction_in_overlay(
                accepted,
                transaction,
                cache,
                Some(routing),
            ),
            Err(reason) => Err(reason),
        };
        require_source(transaction, input, input_index, routing)?;
        transaction.require_completed_execution_effect_owner()?;
        let effect_limit_rejection = transaction.execution_effect_limit_exceeded();
        let mut work = CompletedOutputWork::capture(transaction);
        let rejection_fee = transaction.take_execution_fee_settlement()?;
        let penalties = transaction.take_deferred_governance_ballot_penalties_v1();
        if result.is_ok() && !penalties.is_empty() {
            return Err("successful Network attempt retains rejection-only penalties".into());
        }
        let block_gas_rejection = result.is_ok()
            && !crate::gas::gas_components_fit_block_limit(gas_limit, [gas_before, work.gas]);
        if block_gas_rejection {
            let attempted = u128::from(gas_before) + u128::from(work.gas);
            result = Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted(format!(
                    "block gas limit exceeded: {attempted} > {gas_limit}"
                )),
            ));
        }
        if let Err(reason) = result {
            transaction
                .callback_journal
                .discard_rejected(Hash::from(input.execution_call_hash()))?;
            drop(attempt);
            // Confidential work survives every completed attempt. Actual gas/fee
            // eligibility is decided from the real rejection, never its bounded row.
            let used_gas = work.gas;
            work.gas = 0;
            work.account(self.state);
            let mut result = Err(reason);
            let mut penalty_committed = true;
            if !penalties.is_empty() && signed_source(input).is_none() {
                result = Err(TransactionRejectionReason::Validation(
                    ValidationFail::InternalError(
                        "deferred governance ballot penalty has no signed transaction".to_owned(),
                    ),
                ));
                penalty_committed = false;
            } else if !penalties.is_empty() {
                let mut penalty = OutputTransaction::new(self.state);
                let transaction = penalty
                    .transaction
                    .as_mut()
                    .ok_or("penalty attempt is absent")?;
                bind_source(transaction, input, input_index, routing);
                let applied = StateBlock::stage_rejected_governance_ballot_penalties_v1(
                    transaction,
                    &penalties,
                );
                match applied {
                    Ok(()) => {
                        require_rejection_fragment(transaction, input, input_index, routing)?;
                        penalty.apply();
                    }
                    Err(error) => {
                        drop(penalty);
                        result = Err(error);
                        penalty_committed = false;
                    }
                }
            }
            if !block_gas_rejection && penalty_committed {
                if rejected_transaction_gas_is_accountable(used_gas, &result) {
                    self.state.gas_used_in_block =
                        self.state.gas_used_in_block.saturating_add(used_gas);
                }
                let chargeable = !matches!(
                    &result,
                    Err(TransactionRejectionReason::Validation(
                        ValidationFail::InternalError(_)
                    ))
                ) && !effect_limit_rejection;
                if let Some(signed) = signed_source(input)
                    && chargeable
                    && let Some(basis) = rejection_fee
                {
                    let mut fee = OutputTransaction::new(self.state);
                    let transaction = fee.transaction.as_mut().ok_or("fee attempt is absent")?;
                    bind_source(transaction, input, input_index, routing);
                    let charged = match basis.settle(transaction, signed) {
                        Ok(charged) => Ok(charged),
                        Err(crate::executor::ExecutionFeeSettlementError::Owner(error)) => {
                            return Err(error);
                        }
                        Err(crate::executor::ExecutionFeeSettlementError::Charge(error)) => {
                            Err(TransactionRejectionReason::Validation(error))
                        }
                    };
                    match charged {
                        Ok(true) => {
                            require_rejection_fragment(transaction, input, input_index, routing)?;
                            fee.apply();
                        }
                        Ok(false) => drop(fee),
                        Err(error) => {
                            drop(fee);
                            result = Err(error);
                        }
                    }
                }
            }
            let actual = ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index,
                result: TransactionResult::new(result),
                completions: Vec::new(),
            });
            actual.validate_structure(self.source.header().height().get(), &self.source)?;
            let row = reservation.finish_network_rejection(actual)?;
            self.rows[index] = row;
            self.network_resolved[index] = true;
            return Ok(disposition);
        }
        // The legacy returned DFS vector is not the capture owner. The actual
        // journal also retains nested by-call steps omitted from that vector.
        if transaction
            .world
            .external_event_buf
            .iter()
            .any(|event| matches!(event, EventBox::TriggerCompleted(_)))
        {
            return Err("Network completion bypassed its actual callback journal".into());
        }
        let call = Hash::from(input.execution_call_hash());
        let mut receipts = core::mem::take(&mut transaction.pending_batch_transfer_outcomes);
        let owned = receipts
            .remove(&HashOf::from_untyped_unchecked(call))
            .unwrap_or_default();
        if !receipts.is_empty() {
            return Err("Network receipts belong to another execution call".into());
        }
        let (actual, journal_overflow) = match transaction.callback_journal.take(call)? {
            DrainedCallbacks::Complete { steps, completions } => {
                let mut result = TransactionResult::new(Ok(steps));
                result.set_batch_transfer_outcomes(owned);
                (
                    ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                        input_index,
                        result,
                        completions,
                    }),
                    false,
                )
            }
            DrainedCallbacks::OutputLimit => (
                ExecutionOutputV1::network_output_limit_rejection(input_index),
                true,
            ),
        };
        actual.validate_structure(self.source.header().height().get(), &self.source)?;
        let (row, apply) = match reservation.finish(actual)? {
            ReservedExecutionOutput::Accepted(row) => (row, !journal_overflow),
            ReservedExecutionOutput::OutputLimit(row) => (row, false),
        };
        if apply {
            let transaction = attempt
                .transaction
                .as_mut()
                .ok_or("Network attempt is absent")?;
            transaction
                .world
                .external_event_buf
                .try_reserve(row.completions().len())
                .map_err(|_| "host cannot retain Network completions")?;
            for completion in row.completions() {
                transaction.world.external_event_buf.push(
                    TriggerCompletedEvent::new(
                        completion.trigger_id.clone(),
                        HashOf::from_untyped_unchecked(call),
                        completion.callback_index,
                        completion.outcome.clone(),
                    )
                    .into(),
                );
            }
            attempt.apply();
        } else {
            drop(attempt);
        }
        work.account(self.state);
        self.rows[index] = row;
        self.network_resolved[index] = true;
        Ok(disposition)
    }
}

fn signed_source(
    input: &TransactionEntrypoint,
) -> Option<&iroha_data_model::transaction::SignedTransaction> {
    match input {
        TransactionEntrypoint::External(signed) => Some(signed),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
        TransactionEntrypoint::SealedCommitment(_) => None,
    }
}

fn bind_source(
    transaction: &mut StateTransaction<'_, '_>,
    input: &TransactionEntrypoint,
    index: u32,
    route: RoutingDecision,
) {
    transaction.current_entrypoint_index = Some(u64::from(index));
    transaction.current_network_entrypoint_hash = Some(input.hash());
    transaction.tx_call_hash = Some(Hash::from(input.execution_call_hash()));
    transaction.current_tx_hash = signed_source(input).map(|signed| signed.hash());
    transaction.current_lane_id = Some(route.lane_id);
    transaction.current_dataspace_id = Some(route.dataspace_id);
    transaction.world.current_dataspace_id = Some(route.dataspace_id);
}

fn require_source(
    transaction: &StateTransaction<'_, '_>,
    input: &TransactionEntrypoint,
    index: u32,
    route: RoutingDecision,
) -> Result<(), String> {
    if transaction.current_entrypoint_index != Some(u64::from(index))
        || transaction.current_network_entrypoint_hash != Some(input.hash())
        || transaction.tx_call_hash != Some(Hash::from(input.execution_call_hash()))
        || transaction.current_tx_hash != signed_source(input).map(|signed| signed.hash())
        || transaction.current_lane_id != Some(route.lane_id)
        || transaction.current_dataspace_id != Some(route.dataspace_id)
        || transaction.world.current_dataspace_id != Some(route.dataspace_id)
    {
        return Err("Network execution changed its source or frozen route owner".into());
    }
    Ok(())
}

fn require_rejection_fragment(
    transaction: &StateTransaction<'_, '_>,
    input: &TransactionEntrypoint,
    index: u32,
    route: RoutingDecision,
) -> Result<(), String> {
    require_source(transaction, input, index, route)?;
    if !transaction.callback_journal.allows_apply()
        || !transaction.pending_batch_transfer_outcomes.is_empty()
        || transaction
            .world
            .external_event_buf
            .iter()
            .any(|event| matches!(event, EventBox::TriggerCompleted(_)))
    {
        return Err("rejection settlement retained rejected business capture".into());
    }
    Ok(())
}

#[cfg(test)]
mod source_binding_tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        block::BlockHeader,
        isi::Log,
        transaction::{
            FeePaymentIntent, TransactionBuilder,
            signed::{SealedTransactionReveal, compute_sealed_transaction_commitment},
        },
    };
    use std::num::NonZeroU64;

    #[test]
    fn binding_keeps_outer_reveal_identity_distinct_from_inner_call() {
        let key = KeyPair::try_from_seed(vec![0x91; 32], Algorithm::Ed25519).unwrap();
        let authority = AccountId::new(key.public_key().clone());
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let signed = TransactionBuilder::new(
            state.network_id,
            authority,
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([Log::new(
            iroha_data_model::Level::INFO,
            "source identity".to_owned(),
        )])
        .sign(key.private_key());
        let external = TransactionEntrypoint::External(signed.clone());
        let salt = [0x92; 32];
        let commitment = compute_sealed_transaction_commitment(&state.network_id, &signed, salt, 9);
        let reveal = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            commitment, signed, salt,
        ));
        assert_ne!(
            Hash::from(reveal.hash()),
            Hash::from(reveal.execution_call_hash())
        );

        let mut block = state.block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            1,
            0,
        ));
        let mut transaction = block.transaction();
        let route = RoutingDecision::default();
        bind_source(&mut transaction, &external, 0, route);
        assert_eq!(
            transaction.current_network_entrypoint_hash,
            Some(external.hash())
        );
        require_source(&transaction, &external, 0, route).unwrap();
        transaction.current_network_entrypoint_hash = None;
        assert!(require_source(&transaction, &external, 0, route).is_err());

        bind_source(&mut transaction, &reveal, 1, route);
        assert_eq!(
            transaction.current_network_entrypoint_hash,
            Some(reveal.hash())
        );
        assert_eq!(
            transaction.tx_call_hash,
            Some(Hash::from(reveal.execution_call_hash()))
        );
        require_source(&transaction, &reveal, 1, route).unwrap();
    }
}
