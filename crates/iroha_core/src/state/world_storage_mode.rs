//! Delegate World storage phases to each original MV mode and refund scope.
//!
//! This module owns no map or publication algorithm. The existing World field
//! phase machine retains these exact MV slots, prepared owners and cleanup.

use super::*;
use concread::bptree::{ClonePlanning, Prepaid, Untracked};
use mv::{PublicationPreparationError, storage::AdmittedStorageError};

type Refusal = PublicationPreparationError<AdmittedStorageError>;

pub(super) fn widen_untracked(error: PublicationPreparationError<Infallible>) -> Refusal {
    match error {
        PublicationPreparationError::Busy(wait) => PublicationPreparationError::Busy(wait),
        PublicationPreparationError::Poisoned => PublicationPreparationError::Poisoned,
        PublicationPreparationError::Changed => PublicationPreparationError::Changed,
        PublicationPreparationError::Admission(impossible) => match impossible {},
    }
}

/// Closed mode selection for the same original current/undo publication owners.
/// Scoped associated owners cannot outlive the caller's pool-refund boundary.
pub(super) trait WorldStorageMode<K: Key, V: Value>:
    StorageMode<K, V> + Send + Sync + 'static
{
    type Slot<'a>;
    type Prepared<'a>;
    type Published<'a>;
    type Aborted<'a>;

    fn publication_slot<'a>(
        original: mv::storage::Detached<K, V, (), Self>,
        target: &'a Storage<K, V, Self>,
        scope: Option<&'a AllocationScope<'a>>,
    ) -> Self::Slot<'a>;
    fn try_prepare(slot: &mut Self::Slot<'_>) -> Result<(), Refusal>;
    fn release_writers(slot: &mut Self::Slot<'_>);
    fn recover_original(slot: &mut Self::Slot<'_>) -> mv::storage::Detached<K, V, (), Self>;
    fn into_prepared<'a>(slot: Self::Slot<'a>) -> Self::Prepared<'a>;
    fn abort<'a>(
        prepared: Self::Prepared<'a>,
    ) -> (mv::storage::Detached<K, V, (), Self>, Self::Aborted<'a>);
    fn publish<'a>(prepared: Self::Prepared<'a>) -> Self::Published<'a>;
}

impl<K: Key, V: Value> WorldStorageMode<K, V> for Untracked {
    type Slot<'a> = mv::storage::DetachedPublicationSlot<'a, K, V, (), ()>;
    type Prepared<'a> = mv::storage::PreparedPublication<'a, K, V, (), ()>;
    type Published<'a> = mv::storage::PublishedPublication<K, V, (), ()>;
    type Aborted<'a> = mv::PublicationCleanup<()>;

    fn publication_slot<'a>(
        original: mv::storage::Detached<K, V, ()>,
        target: &'a Storage<K, V>,
        _scope: Option<&'a AllocationScope<'a>>,
    ) -> Self::Slot<'a> {
        original.publication_slot(target)
    }
    fn try_prepare(slot: &mut Self::Slot<'_>) -> Result<(), Refusal> {
        slot.try_prepare(|_, _| Ok::<_, Infallible>(()))
            .map_err(widen_untracked)
    }
    fn release_writers(slot: &mut Self::Slot<'_>) {
        slot.release_writers();
    }
    fn recover_original(slot: &mut Self::Slot<'_>) -> mv::storage::Detached<K, V, ()> {
        slot.recover_original()
    }
    fn into_prepared<'a>(slot: Self::Slot<'a>) -> Self::Prepared<'a> {
        slot.into_prepared()
    }
    fn abort<'a>(
        prepared: Self::Prepared<'a>,
    ) -> (mv::storage::Detached<K, V, ()>, Self::Aborted<'a>) {
        prepared.abort()
    }
    fn publish<'a>(prepared: Self::Prepared<'a>) -> Self::Published<'a> {
        prepared.publish()
    }
}

/// Missing or foreign scope retains the original journal in an inert shell.
/// The aggregate sees the typed refusal during its normal preparation pass.
/// No writer is acquired and no payload is destroyed while shells are assembled.
pub(super) enum PrepaidSlot<'a, K: Key, V: Value, P>
where
    P: mv::storage::AdmittedStoragePolicy + ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    Original(mv::storage::AdmittedDetachedPublicationSlot<'a, 'a, K, V, (), P>),
    Refused {
        journal: Option<mv::storage::Detached<K, V, (), Prepaid<P>>>,
        cause: Option<Refusal>,
    },
}

impl<K: Key, V: Value, P> WorldStorageMode<K, V> for Prepaid<P>
where
    P: mv::storage::AdmittedStoragePolicy
        + ClonePlanning<K, V>
        + ClonePlanning<K, Option<V>>
        + Send
        + Sync
        + 'static,
{
    type Slot<'a> = PrepaidSlot<'a, K, V, P>;
    type Prepared<'a> = mv::storage::AdmittedPreparedPublication<'a, 'a, K, V, (), P>;
    type Published<'a> = mv::storage::AdmittedPublishedPublication<'a, K, V, (), P>;
    type Aborted<'a> = mv::storage::AdmittedAbortedPublication<'a>;

    fn publication_slot<'a>(
        original: mv::storage::Detached<K, V, (), Self>,
        target: &'a Storage<K, V, Self>,
        scope: Option<&'a AllocationScope<'a>>,
    ) -> Self::Slot<'a> {
        let result = match scope {
            Some(scope) => original.try_publication_slot(scope, target),
            None => Err((
                original,
                PublicationPreparationError::Admission(AdmittedStorageError::ScopeIdentity),
            )),
        };
        match result {
            Ok(slot) => PrepaidSlot::Original(slot),
            Err((journal, cause)) => PrepaidSlot::Refused {
                journal: Some(journal),
                cause: Some(cause),
            },
        }
    }
    fn try_prepare(slot: &mut Self::Slot<'_>) -> Result<(), Refusal> {
        match slot {
            PrepaidSlot::Original(slot) => slot.try_prepare(),
            PrepaidSlot::Refused { cause, .. } => Err(cause
                .take()
                .expect("original field preparation is one-shot")),
        }
    }
    fn release_writers(slot: &mut Self::Slot<'_>) {
        if let PrepaidSlot::Original(slot) = slot {
            slot.release_writers();
        }
    }
    fn recover_original(slot: &mut Self::Slot<'_>) -> mv::storage::Detached<K, V, (), Self> {
        match slot {
            PrepaidSlot::Original(slot) => slot.recover_original(),
            PrepaidSlot::Refused { journal, .. } => journal.take().expect("original refused field"),
        }
    }
    fn into_prepared<'a>(slot: Self::Slot<'a>) -> Self::Prepared<'a> {
        let PrepaidSlot::Original(slot) = slot else {
            unreachable!("refused field cannot complete preparation")
        };
        slot.into_prepared()
    }
    fn abort<'a>(
        prepared: Self::Prepared<'a>,
    ) -> (mv::storage::Detached<K, V, (), Self>, Self::Aborted<'a>) {
        prepared.abort()
    }
    fn publish<'a>(prepared: Self::Prepared<'a>) -> Self::Published<'a> {
        prepared.publish()
    }
}

#[cfg(test)]
#[path = "world_storage_mode_tests.rs"]
mod tests;
