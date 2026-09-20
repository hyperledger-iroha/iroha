//! Stack-owned rollback of an original private cursor and its tracking storage.

use super::CursorWrite;
use crate::bptree::{NodeCloning, Prepaid};
use crate::internals::bptree::{
    node::{Node, TXID_MASK, TXID_SHF},
    tracking::{FixedTrackingBuffer, TrackingBuffer},
};
use std::{fmt::Debug, mem};

type Buffer<K, V, P> = FixedTrackingBuffer<
    *mut Node<K, V, <P as crate::bptree::NodeFunding>::Charge>,
    <P as crate::bptree::NodeFunding>::Charge,
>;

/// Original allocations displaced during the first growth in this checkpoint.
pub(crate) struct CheckpointBuffers<K, V, P: NodeCloning<K, V>> {
    first: Option<Buffer<K, V, P>>,
    last: Option<Buffer<K, V, P>>,
}

impl<K, V, P: NodeCloning<K, V>> CheckpointBuffers<K, V, P> {
    pub(super) fn retain_first(&mut self, old: Buffer<K, V, P>) {
        if self.first.is_none() {
            self.first = Some(old);
        } else {
            drop(old);
        }
    }

    pub(super) fn retain_last(&mut self, old: Buffer<K, V, P>) {
        if self.last.is_none() {
            self.last = Some(old);
        } else {
            drop(old);
        }
    }

    // Move both ancestor originals before any excess charge may unwind.
    fn absorb(&mut self, mut child: Self) -> Self {
        if self.first.is_none() {
            self.first = child.first.take();
        }
        if self.last.is_none() {
            self.last = child.last.take();
        }
        child
    }
}

struct Saved<K, V, P: NodeCloning<K, V>> {
    root: *mut Node<K, V, P::Charge>,
    txid: u64,
    length: usize,
    first_cut: usize,
    last_cut: usize,
    buffers: CheckpointBuffers<K, V, P>,
}

/// Exclusive, move-only checkpoint; exclusive reborrows enforce LIFO resolution.
pub(crate) struct CursorCheckpoint<'a, K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> {
    cursor: &'a mut CursorWrite<K, V, Prepaid<P>>,
    parent: Option<&'a mut CheckpointBuffers<K, V, P>>,
    saved: Option<Saved<K, V, P>>,
}

impl<'a, K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> CursorCheckpoint<'a, K, V, P> {
    pub(super) fn new(
        cursor: &'a mut CursorWrite<K, V, Prepaid<P>>,
        parent: Option<&'a mut CheckpointBuffers<K, V, P>>,
    ) -> Option<Self> {
        cursor.assert_operable();
        assert!(
            cursor.funding.0.is_none(),
            "checkpoint requires sealed edit funding"
        );
        let next = cursor
            .txid
            .checked_add(1)
            .filter(|n| *n < (TXID_MASK >> TXID_SHF))?;
        let saved = Saved {
            root: cursor.root,
            txid: cursor.txid,
            length: cursor.length,
            first_cut: cursor.first_seen.as_slice().len(),
            last_cut: cursor
                .last_seen
                .as_ref()
                .expect("original retirement buffer")
                .as_slice()
                .len(),
            buffers: CheckpointBuffers {
                first: None,
                last: None,
            },
        };
        cursor.txid = next;
        Some(Self {
            cursor,
            parent,
            saved: Some(saved),
        })
    }

    pub(crate) fn as_ref(&self) -> &CursorWrite<K, V, Prepaid<P>> {
        self.cursor.assert_operable();
        self.cursor
    }

    pub(crate) fn edit_parts(
        &mut self,
    ) -> (
        &mut CursorWrite<K, V, Prepaid<P>>,
        &mut CheckpointBuffers<K, V, P>,
    ) {
        self.cursor.assert_operable();
        (
            self.cursor,
            &mut self.saved.as_mut().expect("live checkpoint").buffers,
        )
    }

    pub(crate) fn checkpoint(&mut self) -> Option<CursorCheckpoint<'_, K, V, P>> {
        CursorCheckpoint::new(
            self.cursor,
            Some(&mut self.saved.as_mut().expect("live checkpoint").buffers),
        )
    }

    pub(crate) fn apply(mut self) {
        self.cursor.assert_operable();
        let saved = self.saved.take().expect("live checkpoint");
        self.cursor.edit_failed = true;
        let excess = if let Some(parent) = self.parent.take() {
            parent.absorb(saved.buffers)
        } else {
            saved.buffers
        };
        // No rollback may run after transferring the saved owners to the parent.
        // A refund panic leaves the complete private cursor in a failed state.
        drop(excess);
        self.cursor.edit_failed = false;
    }
}

enum DrainBuffer<'a, K, V, P: NodeCloning<K, V>> {
    Borrowed(&'a mut Buffer<K, V, P>),
    Owned(Buffer<K, V, P>),
}

struct SuffixDrain<'a, K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> {
    buffer: DrainBuffer<'a, K, V, P>,
    cut: usize,
}

impl<K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> SuffixDrain<'_, K, V, P> {
    fn drain(&mut self) {
        let buffer = match &mut self.buffer {
            DrainBuffer::Borrowed(buffer) => &mut **buffer,
            DrainBuffer::Owned(buffer) => buffer,
        };
        while buffer.as_slice().len() > self.cut {
            // Remove custody before invoking any payload or charge destructor.
            // Node::free is nonrecursive; shared parent children stay untouched.
            let node = buffer.pop().expect("initialized child suffix");
            Node::free(node);
        }
    }
}

impl<K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> Drop for SuffixDrain<'_, K, V, P> {
    fn drop(&mut self) {
        self.drain();
    }
}

impl<K: Clone + Ord + Debug, V: Clone, P: NodeCloning<K, V>> Drop
    for CursorCheckpoint<'_, K, V, P>
{
    fn drop(&mut self) {
        let Some(mut saved) = self.saved.take() else {
            return;
        };
        let was_failed = self.cursor.edit_failed;
        self.cursor.edit_failed = true;
        // Restore all parent metadata and retired-pointer bookkeeping before
        // arbitrary cleanup. Published or parent-private targets are not freed
        // through last_seen: only child first_seen allocations belong to abort.
        self.cursor.root = saved.root;
        self.cursor.txid = saved.txid;
        self.cursor.length = saved.length;
        let retired = self
            .cursor
            .last_seen
            .as_mut()
            .expect("original retirement buffer");
        let retired_replacement = saved
            .buffers
            .last
            .take()
            .map(|original| mem::replace(retired, original));
        retired.truncate(saved.last_cut);
        let buffer: DrainBuffer<'_, K, V, P> = if let Some(original) = saved.buffers.first.take() {
            let child = mem::replace(&mut self.cursor.first_seen, original);
            self.cursor.first_seen.truncate(saved.first_cut);
            DrainBuffer::Owned(child)
        } else {
            DrainBuffer::Borrowed(&mut self.cursor.first_seen)
        };
        let mut drain = SuffixDrain {
            buffer,
            cut: saved.first_cut,
        };
        drain.drain();
        drop(drain);
        drop(retired_replacement);
        drop(self.cursor.funding.0.take());
        // Caught edit panics remain failed even if rollback itself succeeded.
        // Successful abort restores the exact parent owners without admission.
        self.cursor.edit_failed = was_failed;
    }
}

#[cfg(all(test, not(feature = "dhat-heap"), not(miri)))]
#[path = "checkpoint_tests.rs"]
mod tests;
