//! Delete only through the original paired physical writers or checkpoints.

use super::*;
use crate::internals::bptree::cursor::CheckpointBuffers;

fn remove_borrowed<K, V, P, E>(
    current: &mut CursorWrite<K, V, Prepaid<P>>,
    current_parent: Option<&mut CheckpointBuffers<K, V, Prepaid<P>>>,
    undo: &mut CursorWrite<K, Option<V>, Prepaid<P>>,
    undo_parent: Option<&mut CheckpointBuffers<K, Option<V>, Prepaid<P>>>,
    key: K,
    admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
) -> Result<Option<V>, (K, PairRemoveError<E>)>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    let mut pair = BorrowedPair {
        current,
        undo,
        resolved: false,
    };
    let plan = match plan_remove_pair::<K, V, P>(pair.current, pair.undo, &key) {
        Ok(plan) => plan,
        Err(error) => {
            pair.resolved = true;
            return Err((key, PairRemoveError::Planning(error)));
        }
    };
    let provider = match admit(plan.demand, &key) {
        Ok(provider) => provider,
        Err(error) => {
            pair.resolved = true;
            return Err((key, PairRemoveError::Refused(error)));
        }
    };
    let previous = {
        let mut current = CursorCheckpoint::new(&mut *pair.current, current_parent)
            .expect("same held current generation preflighted before removal");
        let mut undo = if plan.undo.is_some() {
            Some(
                CursorCheckpoint::new(&mut *pair.undo, undo_parent)
                    .expect("same held undo generation preflighted before removal"),
            )
        } else {
            None
        };
        let previous = execute_remove_pair(&mut current, undo.as_mut(), plan, key, provider);
        current.apply();
        if let Some(undo) = undo {
            undo.apply();
        }
        previous
    };
    pair.resolved = true;
    Ok(previous)
}

impl<K, V, P> BptreeMapWriteTxn<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    /// Remove current and retain its first preimage under both original writers.
    ///
    /// One allocation-free plan covers the canonical path/sibling deletion,
    /// concrete tracking growth and a missing undo insertion. The callback sees
    /// the complete checked demand and original query before checkpoints/copies.
    /// Absence still records a missing undo as None; existing None/Some skips
    /// every undo edit and generation advance. No separate provider is acquired.
    ///
    /// Typed refusal returns the original key and preserves both cursors. Success
    /// consumes the query under the armed pair before cleanup/sealing and keeps
    /// both successors private. Every unwind invalidates both original cursors,
    /// including caught callback, payload, query destructor and apply failures.
    /// The caller must keep both physical writers inside the original common
    /// budget's synchronous refund-deferral scope and supply its intended roles.
    /// This does not fund transaction touches or arbitrary mutable payloads.
    pub fn try_remove_with_undo_admitted<E>(
        &mut self,
        undo: &mut BptreeMapWriteTxn<'_, K, Option<V>, Prepaid<P>>,
        key: K,
        admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
    ) -> Result<Option<V>, (K, PairRemoveError<E>)> {
        remove_borrowed(
            self.inner.as_mut(),
            None,
            undo.inner.as_mut(),
            None,
            key,
            admit,
        )
    }
}

impl<K, V, P> BptreeMapCheckpoint<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    /// Remove through the exact original current/undo parent rollback owners.
    ///
    /// The one complete admission precedes both child checkpoints and all copies.
    /// Children retain the matching parents' displaced tracking buffers. Success
    /// remains private; abort restores original roots, generations and buffers
    /// without allocation or credit. The owned key drops while guards are armed.
    /// A prefailed parent or any caught panic invalidates both original cursors;
    /// abandon both. Keep their full lifetime in the original refund-deferral
    /// scope. This provides neither a detached owner nor a mutable value escape.
    pub fn try_remove_with_undo_admitted<E>(
        &mut self,
        undo: &mut BptreeMapCheckpoint<'_, K, Option<V>, Prepaid<P>>,
        key: K,
        admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
    ) -> Result<Option<V>, (K, PairRemoveError<E>)> {
        let (current, current_parent) = self.inner.joined_edit_parts();
        let (undo, undo_parent) = undo.inner.joined_edit_parts();
        remove_borrowed(
            current,
            Some(current_parent),
            undo,
            Some(undo_parent),
            key,
            admit,
        )
    }
}
