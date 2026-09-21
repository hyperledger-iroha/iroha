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
    /// Original displaced bookkeeping retained by a borrowed checkpoint.
    #[doc(hidden)]
    type RetainedBuffer<T: Copy>;
    /// Construct an empty checkpoint retention slot without allocating.
    #[doc(hidden)]
    fn empty_retained_buffer<T: Copy>() -> Self::RetainedBuffer<T>;
    /// Retain the first displaced allocation, dropping any later replacement.
    #[doc(hidden)]
    fn retain_original_buffer<T: Copy>(
        saved: &mut Self::RetainedBuffer<T>,
        original: Self::Buffer<T>,
    );
    /// Move the original allocation out before restoring checkpoint custody.
    #[doc(hidden)]
    fn take_original_buffer<T: Copy>(
        saved: &mut Self::RetainedBuffer<T>,
    ) -> Option<Self::Buffer<T>>;
    /// Transfer an ancestor's original slot without dropping either allocation.
    #[doc(hidden)]
    fn inherit_original_buffer<T: Copy>(
        parent: &mut Self::RetainedBuffer<T>,
        child: &mut Self::RetainedBuffer<T>,
    );
    /// Move-only input accepted under the original writer lock.
    type Input<T: Copy>;
    /// Consume the original provider and both original bookkeeping buffers.
    fn into_parts<T: Copy>(input: Self::Input<T>) -> (Self, Self::Buffer<T>, Self::Buffer<T>);
    /// Check that no closed edit still owns unused admission at a checkpoint.
    #[doc(hidden)]
    fn assert_funding_idle(&self);
    /// Drop only unused admission after restoring all original node owners.
    #[doc(hidden)]
    fn drop_unused_funding(&mut self);
}

impl sealed::Sealed for Untracked {}
impl MapMode for Untracked {
    type Buffer<T: Copy> = Vec<T>;
    // Untracked edits grow their original Vec in place and keep its capacity
    // on abort. No displaced allocation exists to retain in a checkpoint.
    type RetainedBuffer<T: Copy> = ();

    fn empty_retained_buffer<T: Copy>() {}

    fn retain_original_buffer<T: Copy>((): &mut (), original: Vec<T>) {
        drop(original);
    }

    fn take_original_buffer<T: Copy>((): &mut ()) -> Option<Vec<T>> {
        None
    }

    fn inherit_original_buffer<T: Copy>((): &mut (), (): &mut ()) {}
    type Input<T: Copy> = ();

    fn into_parts<T: Copy>((): ()) -> (Self, Self::Buffer<T>, Self::Buffer<T>) {
        (Untracked, Vec::with_capacity(16), Vec::with_capacity(16))
    }

    fn assert_funding_idle(&self) {}
    fn drop_unused_funding(&mut self) {}
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
    type RetainedBuffer<T: Copy> = Option<Self::Buffer<T>>;

    fn empty_retained_buffer<T: Copy>() -> Self::RetainedBuffer<T> {
        None
    }

    fn retain_original_buffer<T: Copy>(
        saved: &mut Self::RetainedBuffer<T>,
        original: Self::Buffer<T>,
    ) {
        if saved.is_none() {
            *saved = Some(original);
        } else {
            drop(original);
        }
    }

    fn take_original_buffer<T: Copy>(
        saved: &mut Self::RetainedBuffer<T>,
    ) -> Option<Self::Buffer<T>> {
        saved.take()
    }

    fn inherit_original_buffer<T: Copy>(
        parent: &mut Self::RetainedBuffer<T>,
        child: &mut Self::RetainedBuffer<T>,
    ) {
        if parent.is_none() {
            *parent = child.take();
        }
    }
    type Input<T: Copy> = (Self, Self::Buffer<T>, Self::Buffer<T>);

    fn into_parts<T: Copy>(input: Self::Input<T>) -> Self::Input<T> {
        input
    }

    fn assert_funding_idle(&self) {
        assert!(self.0.is_none(), "checkpoint requires sealed edit funding");
    }

    fn drop_unused_funding(&mut self) {
        drop(self.0.take());
    }
}
