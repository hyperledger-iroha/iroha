//! Exact node allocation custody shared by every B+tree allocation path.

use std::alloc::Layout;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

use crossbeam_utils::CachePadded;

use crate::internals::lincowcell::Untracked;

/// Supplies the original prepaid owner for one actual allocation layout.
///
/// A funded operation must reserve its complete demand before mutation. Taking
/// a node charge only splits that owner; it must not acquire fresh pool credit.
/// Nested key/value allocations require a separate admitted cloning policy.
pub(crate) trait NodeFunding {
    type Charge;

    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge;
}

impl NodeFunding for Untracked {
    type Charge = Untracked;

    fn take_node_charge(&mut self, _layout: Layout) -> Untracked {
        Untracked
    }
}

/// Charge storage inside the original concrete leaf or branch allocation.
pub(super) trait NodeAllocation {
    type Charge;

    fn charge(&mut self) -> &mut ManuallyDrop<Self::Charge>;
}

/// Owns a node through construction, including user clone/comparison unwind.
/// The same owner is reconstructed when the original raw node is reclaimed.
pub(super) struct OwnedNodeAllocation<N: NodeAllocation> {
    node: Option<Box<CachePadded<N>>>,
}

impl<N: NodeAllocation> OwnedNodeAllocation<N> {
    pub(super) fn new(node: N) -> Self {
        Self {
            node: Some(Box::new(CachePadded::new(node))),
        }
    }

    pub(super) fn as_ptr(&self) -> *const N {
        &***self.node.as_ref().expect("original node allocation")
    }

    pub(super) fn into_raw(mut self) -> *mut N {
        Box::into_raw(self.node.take().expect("original node allocation")).cast()
    }

    /// `node` must be the live original pointer returned by `into_raw`, owned
    /// exclusively by this reclamation path and using the exact same `N`.
    pub(super) unsafe fn from_raw(node: *mut N) -> Self {
        Self {
            node: Some(unsafe { Box::from_raw(node.cast::<CachePadded<N>>()) }),
        }
    }
}

impl<N: NodeAllocation> Deref for OwnedNodeAllocation<N> {
    type Target = N;

    fn deref(&self) -> &N {
        self.node.as_ref().expect("original node allocation")
    }
}

impl<N: NodeAllocation> DerefMut for OwnedNodeAllocation<N> {
    fn deref_mut(&mut self) -> &mut N {
        self.node.as_mut().expect("original node allocation")
    }
}

impl<N: NodeAllocation> Drop for OwnedNodeAllocation<N> {
    fn drop(&mut self) {
        let Some(mut node) = self.node.take() else {
            return;
        };
        // Extract without releasing capacity. If a payload destructor panics,
        // conservatively retain its charge: reclamation did not finish normally.
        let charge = ManuallyDrop::new(unsafe { ManuallyDrop::take(node.charge()) });
        drop(node);
        // Box destruction includes deallocation of this exact padded layout.
        drop(ManuallyDrop::into_inner(charge));
    }
}
