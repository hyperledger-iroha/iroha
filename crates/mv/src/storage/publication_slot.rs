//! Original Storage publication authority retained through aggregate preparation.

use super::*;
use crate::allocation::OwnedAllocationScope;
use concread::bptree::{ClonePlanning, Prepaid};

#[expect(
    clippy::large_enum_variant,
    reason = "publication phases retain original journals, retirement and refund scope inline without an unadmitted allocation"
)]
enum Phase<'a, K: Key, V: Value, M: StorageMode<K, V>> {
    Attached(Block<'a, K, V, M>),
    Frozen {
        slot: DetachedPublicationSlotInner<'a, K, V, (), (), M>,
        // Original scope outlives all private owners and actual release notices.
        scope: M::AcquisitionCustody,
    },
    Published {
        _retirement: PublishedPublication<K, V, (), (), M>,
        _scope: M::AcquisitionCustody,
    },
    Empty,
}

/// Caller-owned original map Block and its exclusive publication phases.
/// Consuming construction prevents a borrowed prepaid operation callback from
/// publishing before its enclosing HRTB operation accepts the complete result.
#[must_use = "prepare and publish, or jointly release the original slot"]
pub struct BlockPublicationSlot<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    phase: Phase<'a, K, V, M>,
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> Block<'a, K, V, M> {
    /// Move the exact original Block into an inert caller-owned publication slot.
    /// No writer is released, cloned or reacquired during this transition.
    pub fn publication_slot(self) -> BlockPublicationSlot<'a, K, V, M> {
        BlockPublicationSlot {
            phase: Phase::Attached(self),
        }
    }
}

impl<'a, K: Key, V: Value> BlockPublicationSlot<'a, K, V> {
    /// Install the exact original untracked map pair before reacquisition.
    /// Prepaid storage cannot use this unscoped constructor.
    pub fn from_frozen(original: Detached<K, V, ()>, target: &'a Storage<K, V>) -> Self {
        Self {
            phase: Phase::Frozen {
                slot: DetachedPublicationSlotInner::new(original, target),
                scope: (),
            },
        }
    }
}

/// A foreign refund-scope refusal returning the exact original journal.
type FrozenScopeRefusal<K, V, P> = (Detached<K, V, (), Prepaid<P>>, AdmittedStorageError);

impl<'a, K: Key, V: Value, P> BlockPublicationSlot<'a, K, V, Prepaid<P>>
where
    P: AdmittedStoragePolicy + ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    /// Install the exact prepaid map pair under its original refund scope.
    /// A foreign scope returns unchanged custody before touching any writer.
    #[expect(
        clippy::result_large_err,
        reason = "refusal preserves the original inline journal without allocating"
    )]
    pub fn try_from_frozen_owned(
        original: Detached<K, V, (), Prepaid<P>>,
        target: &'a Storage<K, V, Prepaid<P>>,
        scope: &OwnedAllocationScope,
    ) -> Result<Self, FrozenScopeRefusal<K, V, P>> {
        if !scope.belongs_to(target.allocation_budget()) {
            return Err((original, AdmittedStorageError::ScopeIdentity));
        }
        Ok(Self {
            phase: Phase::Frozen {
                slot: DetachedPublicationSlotInner::new(original, target),
                scope: AdmittedAcquisitionCustody {
                    _owned: Some(scope.clone()),
                    _thread: std::marker::PhantomData,
                },
            },
        })
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> crate::FrozenBlockPublication
    for BlockPublicationSlot<'_, K, V, M>
{
    type Frozen = Detached<K, V, (), M>;

    fn try_prepare_frozen(
        &mut self,
    ) -> Result<(), PublicationPreparationError<core::convert::Infallible>> {
        let Phase::Frozen { slot, .. } = &mut self.phase else {
            panic!("original frozen publication slot required");
        };
        slot.try_prepare(|_, _| Ok(()))
    }

    fn recover_frozen(&mut self) -> Self::Frozen {
        let Phase::Frozen { slot, .. } = &mut self.phase else {
            panic!("original frozen publication slot required");
        };
        slot.recover_original()
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> crate::BlockPublication
    for BlockPublicationSlot<'_, K, V, M>
{
    fn prepare_publication(&mut self) {
        let Phase::Attached(block) = &mut self.phase else {
            panic!("frozen publication requires explicit fallible preparation");
        };
        block.prepare_attached_publication();
    }

    fn publish_prepared(&mut self) {
        if let Phase::Attached(block) = &mut self.phase {
            block.publish_attached_prepared();
            return;
        }
        assert!(
            matches!(&self.phase, Phase::Frozen { slot, .. } if slot.is_prepared()),
            "complete original frozen preparation required"
        );
        let Phase::Frozen { slot, scope } = std::mem::replace(&mut self.phase, Phase::Empty) else {
            panic!("complete original publication slot required");
        };
        self.phase = Phase::Published {
            _retirement: slot.into_prepared().publish(),
            _scope: scope,
        };
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> crate::BlockRetirement
    for BlockPublicationSlot<'_, K, V, M>
{
    fn release_writers(&mut self) {
        match &mut self.phase {
            Phase::Attached(block) => crate::BlockRetirement::release_writers(block),
            Phase::Frozen { slot, .. } => slot.release_writers(),
            Phase::Published { .. } | Phase::Empty => {}
        }
    }
}
