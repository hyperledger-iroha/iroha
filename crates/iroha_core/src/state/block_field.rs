//! Keep one original executing field armed through attached publication.
//!
//! The aggregate inventory owns this field before preparation starts. Execution
//! can borrow only the original Block; publication requires a private transition
//! of this owner after the aggregate has accepted the complete result.

use mv::{BlockPublication, BlockRetirement, Key, Value};
use norito::json;
use std::{
    borrow::Borrow,
    ops::{Deref, DerefMut, RangeBounds},
};

mod sealed {
    pub trait Sealed {}
    impl<V: mv::Value, C: Send + Sync + 'static> Sealed for mv::cell::Block<'_, V, C> {}
    impl<K: mv::Key, V: mv::Value, M: mv::storage::StorageMode<K, V>> Sealed
        for mv::storage::Block<'_, K, V, M>
    {
    }
}

/// An original MV Block whose consuming transfer creates publication authority.
/// Implementations perform an inert move, without allocation or validation.
pub trait OriginalPublicationBlock: BlockRetirement + sealed::Sealed {
    /// Exact attached publication owner for this original Block.
    type Publication: BlockPublication;
    /// Consume execution authority without releasing any physical writer.
    fn into_publication(self) -> Self::Publication;
}

impl<'a, V: Value, C: Send + Sync + 'static> OriginalPublicationBlock
    for mv::cell::Block<'a, V, C>
{
    type Publication = mv::cell::BlockPublicationSlot<'a, V, C>;
    fn into_publication(self) -> Self::Publication {
        self.publication_slot()
    }
}

impl<'a, K: Key, V: Value, M: mv::storage::StorageMode<K, V>> OriginalPublicationBlock
    for mv::storage::Block<'a, K, V, M>
{
    type Publication = mv::storage::BlockPublicationSlot<'a, K, V, M>;
    fn into_publication(self) -> Self::Publication {
        self.publication_slot()
    }
}

/// Exact Cell field types keep aggregate lifetimes covariant. An associated
/// publication projection in the aggregate field type would make them invariant.
pub type CellField<'a, V> =
    BlockField<mv::cell::Block<'a, V>, mv::cell::BlockPublicationSlot<'a, V>>;

/// Exact Storage field types retain both original map publications in one phase.
pub type StorageField<'a, K, V> =
    BlockField<mv::storage::Block<'a, K, V>, mv::storage::BlockPublicationSlot<'a, K, V>>;

enum Phase<B, P> {
    Executing(B),
    Publishing(P),
}

/// One field in the original executing inventory, also retaining its retirement.
/// Borrowed execution exposes no publication method or publication slot.
pub struct BlockField<B, P = <B as OriginalPublicationBlock>::Publication>
where
    B: OriginalPublicationBlock<Publication = P>,
    P: BlockPublication,
{
    phase: Option<Phase<B, P>>,
    released: bool,
}

impl<B: OriginalPublicationBlock> BlockField<B> {
    pub(crate) fn new(block: B) -> Self {
        Self {
            phase: Some(Phase::Executing(block)),
            released: false,
        }
    }

    /// Transfer only an untouched executing owner to the aggregate capture path.
    pub(crate) fn into_executing(mut self) -> B {
        assert!(!self.released, "field was terminally released");
        assert!(
            matches!(self.phase, Some(Phase::Executing(_))),
            "field already entered publication"
        );
        match self.phase.take().expect("original field") {
            Phase::Executing(block) => block,
            Phase::Publishing(_) => unreachable!("checked original execution phase"),
        }
    }

    pub(crate) fn prepare_publication(&mut self) {
        assert!(!self.released, "field was terminally released");
        assert!(
            matches!(self.phase, Some(Phase::Executing(_))),
            "field publication is one-shot"
        );
        let Some(Phase::Executing(block)) = self.phase.take() else {
            unreachable!("checked original execution phase")
        };
        // Only an inert original-owner move occurs outside caller custody.
        self.phase = Some(Phase::Publishing(block.into_publication()));
        let Some(Phase::Publishing(slot)) = self.phase.as_mut() else {
            unreachable!("original publication phase")
        };
        slot.prepare_publication();
    }

    pub(crate) fn publish_prepared(&mut self) {
        assert!(!self.released, "field was terminally released");
        let Some(Phase::Publishing(slot)) = self.phase.as_mut() else {
            panic!("original field was not prepared")
        };
        slot.publish_prepared();
    }
}

impl<B: OriginalPublicationBlock> Deref for BlockField<B> {
    type Target = B;
    fn deref(&self) -> &B {
        assert!(!self.released, "field was terminally released");
        match self.phase.as_ref() {
            Some(Phase::Executing(block)) => block,
            _ => panic!("publication field has no execution authority"),
        }
    }
}
impl<B: OriginalPublicationBlock> DerefMut for BlockField<B> {
    fn deref_mut(&mut self) -> &mut B {
        assert!(!self.released, "field was terminally released");
        match self.phase.as_mut() {
            Some(Phase::Executing(block)) => block,
            _ => panic!("publication field has no execution authority"),
        }
    }
}
impl<B, P> BlockRetirement for BlockField<B, P>
where
    B: OriginalPublicationBlock<Publication = P>,
    P: BlockPublication,
{
    fn release_writers(&mut self) {
        self.released = true;
        match self.phase.as_mut() {
            Some(Phase::Executing(block)) => block.release_writers(),
            Some(Phase::Publishing(slot)) => slot.release_writers(),
            None => {}
        }
    }
}
impl<B, P> Drop for BlockField<B, P>
where
    B: OriginalPublicationBlock<Publication = P>,
    P: BlockPublication,
{
    fn drop(&mut self) {
        self.release_writers();
    }
}
impl<B: OriginalPublicationBlock + json::JsonSerialize> json::JsonSerialize for BlockField<B> {
    fn json_serialize(&self, out: &mut String) {
        self.deref().json_serialize(out);
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.deref().json_serialize_to(out)
    }
}
impl<K: Key, V: Value, B: OriginalPublicationBlock + mv::storage::StorageReadOnly<K, V>>
    mv::storage::StorageReadOnly<K, V> for BlockField<B>
{
    type Iter<'a>
        = B::Iter<'a>
    where
        Self: 'a;
    type RangeIter<'a>
        = B::RangeIter<'a>
    where
        Self: 'a;
    fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.deref().get(key)
    }
    fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        self.deref().get_key_value(key)
    }
    fn iter(&self) -> Self::Iter<'_> {
        self.deref().iter()
    }
    fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> Self::RangeIter<'_>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.deref().range(bounds)
    }
    fn first_key_value(&self) -> Option<(&K, &V)> {
        self.deref().first_key_value()
    }
    fn last_key_value(&self) -> Option<(&K, &V)> {
        self.deref().last_key_value()
    }
    fn len(&self) -> usize {
        self.deref().len()
    }
}

#[cfg(test)]
#[path = "block_field_tests.rs"]
mod tests;

/// One aggregate's exclusive permission to prepare and publish its original fields.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AggregatePublication {
    Executing,
    Preparing,
    Prepared,
    Publishing,
    Published,
    Released,
}
impl AggregatePublication {
    pub(crate) fn assert_executing(&self) {
        assert_eq!(*self, Self::Executing, "aggregate no longer executes");
    }
    pub(crate) fn begin_preparation(&mut self) {
        self.assert_executing();
        *self = Self::Preparing;
    }
    pub(crate) fn finish_preparation(&mut self) {
        assert_eq!(*self, Self::Preparing);
        *self = Self::Prepared;
    }
    pub(crate) fn begin_publication(&mut self) {
        assert_eq!(
            *self,
            Self::Prepared,
            "all original fields must prepare before any publishes"
        );
        *self = Self::Publishing;
    }
    pub(crate) fn finish_publication(&mut self) {
        assert_eq!(*self, Self::Publishing);
        *self = Self::Published;
    }
    pub(crate) fn release(&mut self) {
        *self = Self::Released;
    }
}
