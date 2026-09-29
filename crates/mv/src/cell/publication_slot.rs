//! Original Cell publication authority retained through aggregate preparation.

use super::*;

#[expect(
    clippy::large_enum_variant,
    reason = "publication phases retain original writers and retirement inline without allocating while sibling writers are held"
)]
enum Phase<'a, V: Value, C: Send + Sync + 'static> {
    Attached(Block<'a, V, C>),
    Frozen(DetachedPublicationSlot<'a, V, (), (), C>),
    Published {
        _retirement: PublishedPublication<V, (), (), C>,
    },
    Empty,
}

/// Caller-owned original Cell and its mutually exclusive publication phases.
/// Only an owned Block or its original frozen journal creates this slot.
/// Construction performs no physical acquisition, validation or allocation.
#[must_use = "prepare and publish, or jointly release the original slot"]
pub struct BlockPublicationSlot<'a, V: Value, C: Send + Sync + 'static = Untracked> {
    phase: Phase<'a, V, C>,
}

impl<'a, V: Value, C: Send + Sync + 'static> Block<'a, V, C> {
    /// Move the original Block into its caller's aggregate publication owner.
    /// Install every sibling slot before invoking preparation on any slot.
    pub fn publication_slot(self) -> BlockPublicationSlot<'a, V, C> {
        BlockPublicationSlot {
            phase: Phase::Attached(self),
        }
    }
}

impl<'a, V: Value, C: Send + Sync + 'static> BlockPublicationSlot<'a, V, C> {
    /// Install the exact original frozen pair in the existing publication slot.
    /// The first preparation checks its actual source and predecessor; equal
    /// values in another Cell cannot substitute for that authority.
    pub fn from_frozen(original: Detached<V, (), C>, target: &'a Cell<V, C>) -> Self {
        Self {
            phase: Phase::Frozen(original.publication_slot(target)),
        }
    }
}

impl<V: Value, C: Send + Sync + 'static> crate::FrozenBlockPublication
    for BlockPublicationSlot<'_, V, C>
{
    type Frozen = Detached<V, (), C>;

    fn try_prepare_frozen(
        &mut self,
    ) -> Result<(), PublicationPreparationError<core::convert::Infallible>> {
        let Phase::Frozen(slot) = &mut self.phase else {
            panic!("original frozen publication slot required");
        };
        slot.try_prepare(|_, _| Ok(()))
    }

    fn recover_frozen(&mut self) -> Self::Frozen {
        let Phase::Frozen(slot) = &mut self.phase else {
            panic!("original frozen publication slot required");
        };
        slot.recover_original()
    }
}

impl<V: Value, C: Send + Sync + 'static> crate::BlockPublication
    for BlockPublicationSlot<'_, V, C>
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
            matches!(&self.phase, Phase::Frozen(slot) if slot.is_prepared()),
            "complete original frozen preparation required"
        );
        let Phase::Frozen(slot) = std::mem::replace(&mut self.phase, Phase::Empty) else {
            panic!("complete original publication slot required");
        };
        // The existing native publisher only transfers already-prepared owners.
        // Retirement remains inline; no destructor or callback is run here.
        self.phase = Phase::Published {
            _retirement: slot.into_prepared().publish(),
        };
    }
}

impl<V: Value, C: Send + Sync + 'static> crate::BlockRetirement for BlockPublicationSlot<'_, V, C> {
    fn release_writers(&mut self) {
        match &mut self.phase {
            Phase::Attached(block) => crate::BlockRetirement::release_writers(block),
            Phase::Frozen(slot) => slot.release_writers(),
            Phase::Published { .. } | Phase::Empty => {}
        }
    }
}
