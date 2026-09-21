//! Closed reset under the original held prepaid writer and checkpoint owners.

use super::*;
use crate::internals::bptree::cursor::{CheckpointBuffers, CursorCheckpoint};

// A caught panic can retain the physical writer. This guard outlives its child
// checkpoint, including both unused-provider and saved-buffer destructors.
struct Reset<'a, K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> {
    cursor: &'a mut CursorWrite<K, V, Prepaid<P>>,
    resolved: bool,
}
impl<K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> Drop for Reset<'_, K, V, P> {
    fn drop(&mut self) {
        if !self.resolved {
            self.cursor.poison_joined_edit();
        }
    }
}

fn clear_admitted<K, V, P, E>(
    cursor: &mut CursorWrite<K, V, Prepaid<P>>,
    parent: Option<&mut CheckpointBuffers<K, V, Prepaid<P>>>,
    admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
) -> Result<(), MapAdmissionError<E>>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: NodeCloning<K, V>,
{
    let mut reset = Reset {
        cursor,
        resolved: false,
    };
    let plan = match plan_clear(reset.cursor) {
        Ok(plan) => plan,
        Err(error) => {
            reset.resolved = true;
            return Err(MapAdmissionError::Planning(error));
        }
    };
    let mut provider = match admit(plan.demand) {
        Ok(provider) => provider,
        Err(error) => {
            reset.resolved = true;
            return Err(MapAdmissionError::Refused(error));
        }
    };
    {
        let mut checkpoint = CursorCheckpoint::new(&mut *reset.cursor, parent)
            .expect("same held reset generation preflighted before admission");
        let (cursor, saved) = checkpoint.edit_parts();
        let first = allocate_tracking::<K, V, P>(plan.first, &mut provider);
        let last = allocate_tracking::<K, V, P>(plan.last, &mut provider);
        cursor.begin_admitted_edit();
        cursor.resume_admitted_funding(provider, first, last, Some(saved));
        cursor.try_clear().expect("complete original clear plan");
        cursor.finish_admitted_funding();
        checkpoint.apply();
    }
    reset.resolved = true;
    Ok(())
}

impl<K, V, P> BptreeMapWriteTxn<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: NodeCloning<K, V>,
{
    /// Reset this private tree through one complete finite admission.
    ///
    /// The allocation-free plan includes a new empty root and every required
    /// original bookkeeping replacement. No payload is cloned. Existing nodes
    /// remain charged in their original retirement chain until old readers are
    /// released after publication; success itself publishes nothing. Even an
    /// empty root follows the same checked replacement and one callback.
    ///
    /// Typed refusal leaves the original cursor unchanged. Any unwind makes it
    /// unusable, including caught callback, provider or apply-cleanup panics;
    /// abandon the writer. Keep its entire physical lifetime inside the original
    /// budget's synchronous refund-notification deferral scope. This does not
    /// admit arbitrary removal, MV touch storage or complete carrier execution.
    pub fn try_clear_admitted<E>(
        &mut self,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<(), MapAdmissionError<E>> {
        clear_admitted(self.inner.as_mut(), None, admit)
    }
}

impl<K, V, P> BptreeMapCheckpoint<'_, K, V, Prepaid<P>>
where
    K: Clone + Ord + Debug + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
    P: NodeCloning<K, V>,
{
    /// Reset while retaining this exact parent's tree and tracking allocations.
    ///
    /// A temporary child transfers displaced original buffers to this parent;
    /// abort restores their pointers, prefixes, root, generation and entries
    /// without new allocation or credit. Applying retains the empty private
    /// successor and its original retirement obligations. Refusal changes no
    /// owner. Any unwind invalidates the original cursor: drop this checkpoint
    /// and abandon its writer. The whole physical writer lifetime must remain
    /// inside the original budget's synchronous refund-deferral scope.
    pub fn try_clear_admitted<E>(
        &mut self,
        admit: impl FnOnce(AllocationDemand) -> Result<P, E>,
    ) -> Result<(), MapAdmissionError<E>> {
        let (cursor, parent) = self.inner.joined_edit_parts();
        clear_admitted(cursor, Some(parent), admit)
    }
}
