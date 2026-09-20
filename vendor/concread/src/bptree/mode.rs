//! Sealed storage modes for the existing B+tree cursor engine.

use super::{FixedTrackingBuffer, NodeCloning, NodeFunding, TrackingBuffer, Untracked};
use std::alloc::Layout;

mod sealed {
    pub trait Sealed {}
}

/// Selects the original node and bookkeeping owners of a map.
///
/// The sealed implementations are explicit [`Untracked`] storage and
/// [`Prepaid<P>`]. A prepaid map has no unrestricted mutation constructor.
/// Generic bookkeeping types keep private node pointers out of this interface.
pub trait MapMode: NodeFunding + sealed::Sealed + Sized {
    /// Original bookkeeping allocation used by this mode.
    type Buffer<T: Copy>: TrackingBuffer<T, Charge = Self::Charge>;
    /// Move-only input accepted under the original writer lock.
    type Input<T: Copy>;
    /// Consume the original provider and both original bookkeeping buffers.
    fn into_parts<T: Copy>(input: Self::Input<T>) -> (Self, Self::Buffer<T>, Self::Buffer<T>);
}

impl sealed::Sealed for Untracked {}
impl MapMode for Untracked {
    type Buffer<T: Copy> = Vec<T>;
    type Input<T: Copy> = ();

    fn into_parts<T: Copy>((): ()) -> (Self, Self::Buffer<T>, Self::Buffer<T>) {
        (Untracked, Vec::with_capacity(16), Vec::with_capacity(16))
    }
}

/// A map mode whose closed edits consume one original prepaid provider.
///
/// Node and payload charges remain in their original allocation owners; the
/// provider cannot be extracted from an active or detached map writer. Only
/// closed admitted operations construct this mode's fixed bookkeeping buffers.
pub struct Prepaid<P>(pub(crate) Option<P>);

impl<P: NodeFunding> sealed::Sealed for Prepaid<P> {}
impl<P: NodeFunding> NodeFunding for Prepaid<P> {
    type Charge = P::Charge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.0
            .as_mut()
            .expect("original active prepaid operation")
            .take_node_charge(layout)
    }
}
impl<K, V, P: NodeCloning<K, V>> NodeCloning<K, V> for Prepaid<P> {
    fn clone_key(&mut self, key: &K) -> K {
        self.0
            .as_mut()
            .expect("original active prepaid operation")
            .clone_key(key)
    }
    fn clone_value(&mut self, value: &V) -> V {
        self.0
            .as_mut()
            .expect("original active prepaid operation")
            .clone_value(value)
    }
}
impl<P: NodeFunding> MapMode for Prepaid<P> {
    type Buffer<T: Copy> = FixedTrackingBuffer<T, P::Charge>;
    type Input<T: Copy> = (Self, Self::Buffer<T>, Self::Buffer<T>);

    fn into_parts<T: Copy>(input: Self::Input<T>) -> Self::Input<T> {
        input
    }
}
