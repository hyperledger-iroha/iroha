//! Mutable suspension is granted only by the original executing journal.
//! Ordinary Frozen/Detached owners never enter this state machine.

use super::*;
use mv::{BlockResumption as _, BlockSuspension as _};

impl<B: OriginalPublicationBlock> BlockField<B> {
    pub(crate) fn begin_suspension(&mut self) {
        assert!(!self.released);
        let Some(Phase::Executing(block)) = self.phase.take() else {
            panic!("original executing field required");
        };
        self.phase = Some(Phase::Suspending(block.into_suspension()));
    }
    pub(crate) fn finish_suspension(&mut self) {
        let Some(Phase::Suspending(slot)) = self.phase.as_mut() else {
            panic!("original suspension slot required");
        };
        slot.suspend();
        let Some(Phase::Suspending(slot)) = self.phase.take() else {
            unreachable!()
        };
        let (original, cleanup) = slot.into_suspended();
        self.phase = Some(Phase::Suspended(original));
        self.suspension_cleanup = cleanup;
    }
    fn install_resumption(&mut self, install: impl FnOnce(B::Suspended) -> B::Resumption) {
        assert!(!self.released && self.resumption_cleanup.is_none());
        let Some(Phase::Suspended(original)) = self.phase.take() else {
            panic!("original suspended field required");
        };
        self.phase = Some(Phase::Resuming(install(original)));
    }
    pub(crate) fn try_resume(
        &mut self,
    ) -> Result<(), mv::PublicationPreparationError<core::convert::Infallible>> {
        let Some(Phase::Resuming(slot)) = self.phase.as_mut() else {
            panic!("original resumption slot required");
        };
        slot.try_resume()
    }
    /// Call only after every sibling has successfully reacquired its original pair.
    pub(crate) fn finish_resumption(&mut self) {
        let Some(Phase::Resuming(slot)) = self.phase.as_mut() else {
            panic!("prepared original resumption slot required");
        };
        assert!(slot.is_resumed());
        let original = slot.take_block();
        let Some(Phase::Resuming(slot)) = self.phase.take() else {
            unreachable!()
        };
        self.phase = Some(Phase::Executing(original));
        self.resumption_cleanup = Some(slot);
    }
    pub(crate) fn recover_resumption(&mut self) {
        let original = match self.phase.as_mut() {
            Some(Phase::Suspended(_)) => return,
            Some(Phase::Resuming(slot)) => slot.recover_suspended(),
            _ => panic!("original suspended or resuming field required"),
        };
        let Some(Phase::Resuming(slot)) = self.phase.take() else {
            unreachable!()
        };
        self.phase = Some(Phase::Suspended(original));
        self.resumption_cleanup = Some(slot);
    }
    /// No writer or enclosing publication fence may remain when notices retire.
    pub(crate) fn retire_suspension_cleanup(&mut self) {
        assert!(matches!(
            self.phase,
            Some(Phase::Suspended(_) | Phase::Frozen(_) | Phase::Reading(_))
        ));
        drop(self.resumption_cleanup.take());
        drop(std::mem::take(&mut self.suspension_cleanup));
    }
}

impl<'a, V: mv::CellValue, C: Send + Sync + 'static> CellField<'a, V, C> {
    pub(crate) fn install_suspended_resumption(&mut self, target: &'a mv::cell::Cell<V, C>) {
        self.install_resumption(|original| original.resumption_slot(target));
    }
}
impl<'a, K: Key, V: Value, M: mv::storage::StorageMode<K, V>> StorageField<'a, K, V, M> {
    pub(crate) fn install_suspended_resumption(
        &mut self,
        target: &'a mv::storage::Storage<K, V, M>,
    ) {
        self.install_resumption(|original| original.resumption_slot(target));
    }
}
