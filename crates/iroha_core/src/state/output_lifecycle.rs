//! Selection-owned lifecycle capture and original allocation transfer to output rows.

use super::*;
use crate::smartcontracts::isi::triggers::set::lifecycle_journal::{
    LifecycleChange, LifecycleJournal, LifecycleJournalFailure, LifecycleJournalLimits,
    LifecycleSnapshot,
};
use crate::state::StateReadOnly;
use iroha_allocation::{AllocationCharge, ChargedBuffer, ChargedBufferError};
use iroha_data_model::block::execution_output::*;

pub(super) type Ledger = ChargedBuffer<AllocationCharge>;

/// Original bounded source, kept under the producer's exclusive StateBlock owner.
/// Its journal moves into the business overlay before any callback; on rejection
/// the original identity remains authoritative after that entire overlay drops.
pub(super) struct InternalSource {
    identity: TriggerUseV1,
    journal: Option<LifecycleJournal>,
    available: bool,
}

impl InternalSource {
    pub(super) fn identity(&self) -> &TriggerUseV1 {
        &self.identity
    }
    pub(super) fn available(&self) -> bool {
        self.available
    }

    pub(super) fn install(
        &mut self,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), ExecutionAttemptError<String>> {
        if self.available && self.journal.is_none() {
            return Err("bounded internal source journal already consumed".into());
        }
        if let Some(journal) = self.journal.take() {
            tx.world
                .triggers
                .adopt_lifecycle_source_journal(journal)
                .map_err(|_| {
                    ExecutionAttemptError::Deferred(
                        ivm::ExecutionDeferral::LocalInvariantViolation.into(),
                    )
                })?;
        }
        Ok(())
    }
}

/// Capture one source without applying its temporary overlay. The caller must
/// immediately consume this capsule under the same exclusive producer owner;
/// it is neither serializable nor a caller-supplied invocation assertion.
pub(super) fn internal_source(
    state: &mut StateBlock<'_>,
    id: &iroha_data_model::trigger::TriggerId,
    height: u64,
    kind: TriggerLifecycleStateKindV1,
) -> Result<InternalSource, ExecutionAttemptError<String>> {
    let pool = state.execution_budget();
    let maximum = state.callback_output_byte_limit();
    let mut source = state.world.triggers.transaction();
    source
        .enable_selected_lifecycle_journal(pool, limits(maximum), height)
        .map_err(|_| {
            ExecutionAttemptError::Deferred(ivm::ExecutionDeferral::AllocationUnavailable.into())
        })?;
    let snapshot = source.lifecycle_source_snapshot(id);
    match source.lifecycle_failure() {
        Some(LifecycleJournalFailure::Allocation(_)) => {
            return Err(ExecutionAttemptError::Deferred(
                ivm::ExecutionDeferral::AllocationUnavailable.into(),
            ));
        }
        Some(LifecycleJournalFailure::Refused(_)) => {
            return Err(ExecutionAttemptError::Deferred(
                ivm::ExecutionDeferral::LocalInvariantViolation.into(),
            ));
        }
        _ => {}
    }
    let identity = snapshot.as_ref().and_then(|snapshot| {
        let registered_at_height = snapshot.registered_at_height?;
        let action_hash = snapshot.action_hash?;
        (snapshot.kind == kind && registered_at_height < height).then(|| TriggerUseV1 {
            trigger_id: id.clone(),
            registered_at_height,
            action_hash,
        })
    });
    drop(snapshot);
    if let Some(identity) = identity {
        let journal = source.take_lifecycle_journal().map_err(|_| {
            ExecutionAttemptError::Deferred(ivm::ExecutionDeferral::LocalInvariantViolation.into())
        })?;
        Ok(InternalSource {
            identity,
            journal,
            available: true,
        })
    } else {
        // This identity only names a deterministic rejected attempt; it never
        // proves an Invoke, business result, policy mutation or selected Action.
        Ok(InternalSource {
            identity: TriggerUseV1 {
                trigger_id: id.clone(),
                registered_at_height: 0,
                action_hash: Hash::prehashed([0xff; Hash::LENGTH]),
            },
            journal: None,
            available: false,
        })
    }
}

pub(super) fn allocation(error: ChargedBufferError) -> ExecutionAttemptError<String> {
    ExecutionAttemptError::Deferred(match error {
        ChargedBufferError::Admission(reason) => reason.into(),
        ChargedBufferError::Allocator { .. } => {
            ivm::ExecutionDeferral::AllocationUnavailable.into()
        }
    })
}

/// Fixed successor-service work ceilings, further restricted by agreed row bytes.
/// These constants belong to the immutable lifecycle service's reference closure.
pub(super) fn limits(maximum: u64) -> LifecycleJournalLimits {
    LifecycleJournalLimits::selected(maximum)
}

pub(super) fn arm(
    tx: &mut StateTransaction<'_, '_>,
    enabled: bool,
    maximum: u64,
) -> Result<(), ExecutionAttemptError<String>> {
    if enabled {
        let budget = tx.execution_budget();
        let applying_height = tx.block_height();
        if tx
            .world
            .triggers
            .enable_selected_lifecycle_journal(budget, limits(maximum), applying_height)
            .is_err()
        {
            tx.world
                .record_execution_deferral(ivm::ExecutionDeferral::AllocationUnavailable);
            return Err(ExecutionAttemptError::Deferred(
                ivm::ExecutionDeferral::AllocationUnavailable.into(),
            ));
        }
    }
    Ok(())
}

/// Local refusal must win over any deterministic business/limit disposition.
pub(super) fn output_limit(
    tx: &StateTransaction<'_, '_>,
) -> Result<bool, ExecutionAttemptError<String>> {
    match tx.world.triggers.lifecycle_failure() {
        None => Ok(false),
        Some(LifecycleJournalFailure::OutputLimit(_)) => Ok(true),
        Some(LifecycleJournalFailure::Allocation(_)) => Err(ExecutionAttemptError::Deferred(
            ivm::ExecutionDeferral::AllocationUnavailable.into(),
        )),
        Some(LifecycleJournalFailure::Refused(_)) => Err(ExecutionAttemptError::Deferred(
            ivm::ExecutionDeferral::LocalInvariantViolation.into(),
        )),
    }
}

pub(super) fn cover(row: ExecutionOutputV1, trace: TriggerLifecycleEffectsV1) -> ExecutionOutputV1 {
    match row {
        ExecutionOutputV1::Network(execution) => {
            ExecutionOutputV1::NetworkWithLifecycle(NetworkExecutionOutputV2 {
                execution,
                trigger_lifecycle: trace,
            })
        }
        ExecutionOutputV1::Pipeline(execution) => {
            ExecutionOutputV1::PipelineWithLifecycle(PipelineExecutionOutputV2 {
                execution,
                trigger_lifecycle: trace,
            })
        }
        ExecutionOutputV1::Time(execution) => {
            ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                execution,
                trigger_lifecycle: trace,
            })
        }
        _ => unreachable!("capture wraps the original unwrapped execution exactly once"),
    }
}

pub(super) fn empty(row: ExecutionOutputV1, enabled: bool) -> ExecutionOutputV1 {
    if enabled {
        cover(
            row,
            TriggerLifecycleEffectsV1::Complete {
                effects: Vec::new(),
            },
        )
    } else {
        row
    }
}

/// No effect graph, Action bytes or identifier backing is cloned at this boundary.
/// The caller retains the ledger until the returned row dies or enters BlockResult.
#[allow(unsafe_code)]
pub(super) fn take(
    tx: &mut StateTransaction<'_, '_>,
    row: ExecutionOutputV1,
) -> Result<(ExecutionOutputV1, Option<Ledger>), ExecutionAttemptError<String>> {
    let result = take_inner(tx, row);
    if result.is_err() {
        tx.world
            .triggers
            .refuse_lifecycle("lifecycle output custody transfer refused");
    }
    result
}

#[allow(unsafe_code)]
fn take_inner(
    tx: &mut StateTransaction<'_, '_>,
    row: ExecutionOutputV1,
) -> Result<(ExecutionOutputV1, Option<Ledger>), ExecutionAttemptError<String>> {
    let Some(mut journal) = tx.world.triggers.take_lifecycle_journal().map_err(|_| {
        ExecutionAttemptError::Deferred(ivm::ExecutionDeferral::LocalInvariantViolation.into())
    })?
    else {
        return Ok((row, None));
    };
    let count = journal.effects.as_slice().len();
    let mut charges = ChargedBuffer::new(count * 3 + 1, &journal.pool).map_err(allocation)?;
    let mut effects = ChargedBuffer::new(count, &journal.pool).map_err(allocation)?;
    for effect in journal.effects.drain_all() {
        let change = match effect.change {
            LifecycleChange::Insert(after) => TriggerLifecycleChangeV1::Insert {
                after: snapshot(after, &mut charges),
            },
            LifecycleChange::Update(before, after) => TriggerLifecycleChangeV1::Update {
                before: snapshot(before, &mut charges),
                after: snapshot(after, &mut charges),
            },
            LifecycleChange::Remove(before) => TriggerLifecycleChangeV1::Remove {
                before: snapshot(before, &mut charges),
            },
            LifecycleChange::Invoke(state, callback_index) => TriggerLifecycleChangeV1::Invoke {
                state: snapshot(state, &mut charges),
                callback_index,
            },
        };
        if let Some(charge) = effect.trigger_id._charge {
            charges.push_reserved(charge);
        }
        effects.push_reserved(TriggerLifecycleEffectV1 {
            ordinal: effect.ordinal,
            trigger_id: effect.trigger_id.id,
            origin: effect.origin,
            change,
        });
    }
    // SAFETY: unchanged fixed backing moves into the row; its ledger is returned
    // alongside it and all consumers preserve allocation-before-charge disposal.
    let (effects, charge) = unsafe { effects.into_allocation_parts() };
    charges.push_reserved(charge);
    Ok((
        cover(row, TriggerLifecycleEffectsV1::Complete { effects }),
        Some(charges),
    ))
}

#[allow(unsafe_code)]
fn snapshot(source: LifecycleSnapshot, charges: &mut Ledger) -> TriggerLifecycleStateV1 {
    // SAFETY: bytes move unchanged to the canonical field and the original
    // charge joins its enclosing row's already prepaid ledger without allocation.
    let (bytes, charge) = unsafe { source.action.into_allocation_parts() };
    charges.push_reserved(charge);
    TriggerLifecycleStateV1 {
        kind: source.kind,
        registered_at_height: source.registered_at_height,
        action_hash: source.action_hash,
        retry_state: source.retry_state,
        action: TriggerLifecycleActionFrameV1 {
            codec: source.codec,
            bytes,
        },
    }
}

/// Network rejection economics have no trigger callbacks or lifecycle authority.
/// Guard that native invariant at the storage seams before applying the fragment.
pub(super) fn require_empty_native_policy(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), ExecutionAttemptError<String>> {
    let result = (|| {
        if output_limit(tx)? {
            return Err("native Network policy exceeded lifecycle capacity".into());
        }
        let journal = tx.world.triggers.take_lifecycle_journal().map_err(|_| {
            ExecutionAttemptError::Deferred(ivm::ExecutionDeferral::LocalInvariantViolation.into())
        })?;
        if journal
            .as_ref()
            .is_some_and(|journal| !journal.effects.as_slice().is_empty())
        {
            return Err("native Network rejection policy changed trigger storage".into());
        }
        Ok(())
    })();
    if result.is_err() {
        tx.world
            .triggers
            .refuse_lifecycle("native policy lifecycle coverage refused");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::time_test_support as support;
    use super::*;
    use crate::smartcontracts::isi::triggers::set::invocation_identity::time_trigger_use_v1;

    #[test]
    fn lifecycle_internal_source_moves_its_bounded_identity_into_the_root_invoke() {
        let (state, id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        let predecessor = time_trigger_use_v1(&block.world.triggers, &id, 2).unwrap();
        let mut source =
            internal_source(&mut block, &id, 2, TriggerLifecycleStateKindV1::Time).unwrap();
        assert!(source.available());
        assert_eq!(
            source.identity().registered_at_height,
            predecessor.registered_at_height
        );
        assert_ne!(source.identity().action_hash, predecessor.action_hash);
        let identity = source.identity().clone();
        let mut tx = block.transaction();
        source.install(&mut tx).unwrap();
        tx.world.triggers.lifecycle_invoke(&id, 0);
        let effects = tx.world.triggers.lifecycle_effects();
        assert_eq!(effects.len(), 1);
        let LifecycleChange::Invoke(snapshot, 0) = &effects[0].change else {
            panic!("root Invoke")
        };
        assert_eq!(snapshot.action_hash, Some(identity.action_hash));
        assert_eq!(
            snapshot.registered_at_height,
            Some(identity.registered_at_height)
        );
        assert!(
            source.journal.is_none(),
            "original cache custody moved exactly once"
        );
    }

    #[test]
    fn lifecycle_internal_source_unavailable_has_only_a_deterministic_rejection_identity() {
        let (state, id) = support::state_fixture(None, true);
        let mut block = state.block(support::header());
        {
            let mut tx = block.transaction();
            tx.world
                .triggers
                .inspect_by_id_mut(&id, |action| {
                    action.metadata_mut().remove("__registered_block_height");
                })
                .unwrap();
            tx.apply();
        }
        let source =
            internal_source(&mut block, &id, 2, TriggerLifecycleStateKindV1::Time).unwrap();
        assert!(!source.available());
        assert!(source.journal.is_none());
        assert_eq!(source.identity().registered_at_height, 0);
        assert_eq!(
            source.identity().action_hash,
            Hash::prehashed([0xff; Hash::LENGTH])
        );
        assert_eq!(source.identity().trigger_id, id);
    }
}
