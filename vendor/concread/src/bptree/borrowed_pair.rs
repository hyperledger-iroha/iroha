//! Closed insertion while both original physical writer owners remain borrowed.

use super::*;
use crate::internals::bptree::cursor::CheckpointBuffers;

// This guard must precede, and therefore outlive, every nested checkpoint. A
// caller may catch an unwind while keeping both physical writers: lock poison
// alone cannot protect their partly applied or failed private generations.
struct BorrowedPair<'c, 'u, K, V, P>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    current: &'c mut CursorWrite<K, V, Prepaid<P>>,
    undo: &'u mut CursorWrite<K, Option<V>, Prepaid<P>>,
    resolved: bool,
}
impl<K, V, P> Drop for BorrowedPair<'_, '_, K, V, P>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    fn drop(&mut self) {
        if !self.resolved {
            self.current.poison_joined_edit();
            self.undo.poison_joined_edit();
        }
    }
}

fn insert_borrowed<K, V, P, E>(
    current: &mut CursorWrite<K, V, Prepaid<P>>,
    current_parent: Option<&mut CheckpointBuffers<K, V, Prepaid<P>>>,
    undo: &mut CursorWrite<K, Option<V>, Prepaid<P>>,
    undo_parent: Option<&mut CheckpointBuffers<K, Option<V>, Prepaid<P>>>,
    key: K,
    value: V,
    admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
) -> Result<Option<V>, ((K, V), PairInsertError<E>)>
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
    let plan = match plan_pair::<K, V, P>(pair.current, pair.undo, &key) {
        Ok(plan) => plan,
        Err(error) => {
            pair.resolved = true;
            return Err(((key, value), PairInsertError::Planning(error)));
        }
    };
    let provider = match admit(plan.demand, &key) {
        Ok(provider) => provider,
        Err(error) => {
            pair.resolved = true;
            return Err(((key, value), PairInsertError::Refused(error)));
        }
    };
    let previous = {
        let mut current = CursorCheckpoint::new(&mut *pair.current, current_parent)
            .expect("same held current generation preflighted before admission");
        let mut undo = if plan.undo.is_some() {
            Some(
                CursorCheckpoint::new(&mut *pair.undo, undo_parent)
                    .expect("same held undo generation preflighted before admission"),
            )
        } else {
            None
        };
        let previous = execute_pair(&mut current, undo.as_mut(), plan, key, value, provider);
        current.apply();
        if let Some(undo) = undo {
            undo.apply();
        }
        previous
    };
    // The optional checkpoint's destructor borrow has ended. Both applies and
    // every provider/buffer destructor succeeded before either cursor is usable.
    pair.resolved = true;
    Ok(previous)
}

impl<K, V, P> BptreeMapWriteTxn<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    /// Insert current and a missing first undo preimage under already-held writers.
    ///
    /// One callback receives the checked pair demand and a borrow of the original
    /// incoming key before any checkpoint or payload copy. The borrow lets an
    /// aggregate include its metadata in the same reservation without copying
    /// the key beforehand. Any metadata it prepares remains the caller's owner;
    /// this method itself funds only the pair and publishes no metadata.
    /// Typed planning/admission refusal returns the original key/value without
    /// changing either cursor. Existing undo None/Some is never checkpointed or
    /// rewritten. Success applies only to these private writers and publishes
    /// nothing; no detach, reacquisition, cursor shell or replacement lock exists.
    ///
    /// An unwind, including a planning/callback panic, makes both original
    /// cursors unusable even if caught while their physical guards remain held.
    /// Abandon both writers; do not publish one of them independently.
    ///
    /// The caller supplies the corresponding current/undo roles and common
    /// original budget. Its refund-notification deferral scope must enclose
    /// acquisition through final release of BOTH physical writers, not merely
    /// this method. This does not admit ordered touch storage or arbitrary edits.
    /// Only Planning and Refused variants of PairInsertError can be returned.
    pub fn try_insert_with_undo_admitted<E>(
        &mut self,
        undo: &mut BptreeMapWriteTxn<'_, K, Option<V>, Prepaid<P>>,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
    ) -> Result<Option<V>, ((K, V), PairInsertError<E>)> {
        insert_borrowed(
            self.inner.as_mut(),
            None,
            undo.inner.as_mut(),
            None,
            key,
            value,
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
    /// Insert through two original parent checkpoints under one joined admission.
    ///
    /// The callback borrows the original incoming key after complete pair
    /// preflight and before either child or copy, as in the writer operation.
    /// Each temporary child checkpoint retains its own parent's displaced
    /// buffers. Applying these children transfers custody to those exact parents;
    /// dropping either parent later restores its original tree without new credit.
    /// Refusal returns the original key/value and changes neither parent. A stored
    /// undo None/Some skips that child's generation advance and allocation.
    ///
    /// Any unwind marks both original cursors unusable, including failure during
    /// the second apply after the first succeeded. Drop both parent checkpoints
    /// and abandon both writers. The original common-budget refund-deferral scope
    /// must enclose their entire physical writer lifetime, as for the writer API.
    /// This is not admission for transaction touch-key storage or complete MV.
    /// Only Planning and Refused variants of PairInsertError can be returned.
    pub fn try_insert_with_undo_admitted<E>(
        &mut self,
        undo: &mut BptreeMapCheckpoint<'_, K, Option<V>, Prepaid<P>>,
        key: K,
        value: V,
        admit: impl FnOnce(AllocationDemand, &K) -> Result<P, E>,
    ) -> Result<Option<V>, ((K, V), PairInsertError<E>)> {
        let (current, current_parent) = self.inner.joined_edit_parts();
        let (undo, undo_parent) = undo.inner.joined_edit_parts();
        insert_borrowed(
            current,
            Some(current_parent),
            undo,
            Some(undo_parent),
            key,
            value,
            admit,
        )
    }
}
