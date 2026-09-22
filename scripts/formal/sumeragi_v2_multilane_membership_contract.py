"""Bind membership admission and consuming publication to their actual owners.

Used by the multilane release checker; no environment configuration is required.
These structural obligations do not qualify the unfinished State/Apply publisher.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any, Callable

from sumeragi_v2_multilane_geometry_evidence_contract import _code


STATE = "crates/iroha_core/src/state.rs"
STORAGE = "crates/iroha_core/src/state/storage_transactions.rs"
CAPTURE = "crates/iroha_core/src/state/storage_transactions/block/capture.rs"
CAPTURE_TESTS = "crates/iroha_core/src/state/storage_transactions/block/capture_tests.rs"
MODELS = (
    "SumeragiV2NativeApplicationEvidence",
    "SumeragiV2AutonomousReservationCarrier",
)
MEMBERSHIP_BINDINGS = (
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'TransactionsStorage', ('write_lock: Mutex<Arc<()>>', 'released: concread::release::ReleaseNotification')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'TransactionsStorage::block_impl', ('let guard = self.released.guard(self.write_lock.lock());', '_guard: block::MembershipWriter::new(guard)')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'TransactionsBlock', ("_guard: MembershipWriter<'storage>", "latest_block_ref: &'storage ArcSwapOption<BlockInfo>", "blocks_ref: &'storage DashMap<Key, Value>")),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'PreparedTransactionsBlock', ("    pub(crate) struct PreparedTransactionsBlock<'storage> {\n        block: TransactionsBlock<'storage>,\n        publication: MembershipPublication,\n        next_identity: Arc<()>,\n        // Fixed original-owner metadata. During publication the admitted action\n        // stays in place, and every displaced allocation stays in this caller.\n        retired_tip: Option<Arc<BlockInfo>>,\n        publication_started: bool,\n        published: bool,\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'enum', 'MembershipPublication', ('Repeated', 'Replace', 'Advance', 'previous: Option<Arc<BlockInfo>>')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'TransactionsBlock::prepare_commit', ('let mut capture = self.capture_slot();', 'capture.try_prepare()?;', 'Ok(capture.into_prepared())')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'TransactionsBlock::admit_publication', ('self._guard.identity();', 'self.latest_block_ref.load_full()', 'TransactionsBlockError::MissingInsertBlock', 'previous_block.height == current_block.height', 'previous_block.transactions == current_block.transactions', 'MembershipPublication::Repeated', 'usize::from(!self.revert)', '.checked_add(addition)', 'TransactionsBlockError::HeightOverflow', 'expected_current_height != current_height', 'TransactionsBlockError::HeightMismatch', 'MembershipPublication::Replace', 'MembershipPublication::Advance', 'previous: previous_block')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::as_block', ("&TransactionsBlock<'storage>", '&self.block')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::publish', ('        pub(crate) fn publish(mut self) -> TransactionsPublicationRetirement {\n            self.publish_in_place();\n            self.into_retirement()\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'TransactionsPublicationRetirement', ('    pub(crate) struct TransactionsPublicationRetirement {\n        _tip: Option<Arc<BlockInfo>>,\n        _staged: Option<Arc<BlockInfo>>,\n        _identity: Arc<()>,\n        _publication: MembershipPublication,\n        _release: concread::release::DeferredRelease,\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'PublishedTransactions', ('    pub(crate) struct PublishedTransactions<Installation> {\n        _retirement: TransactionsPublicationRetirement,\n        _installation: Installation,\n        _preflight_release: Option<concread::release::DeferredRelease>,\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedDetachedTransactionsBlock::publish', ('        pub(crate) fn publish(self) -> PublishedTransactions<Installation> {\n            let Self {\n                prepared,\n                installation,\n                preflight_release,\n            } = self;\n            PublishedTransactions {\n                _retirement: prepared.publish(),\n                _installation: installation,\n                _preflight_release: preflight_release,\n            }\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'DetachedTransactionsBlock', ('predecessor_identity: Arc<()>', 'predecessor: Option<Arc<BlockInfo>>', 'current: Arc<BlockInfo>', 'revert: bool', 'publication: MembershipPublication', 'next_identity: Arc<()>')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::detach', ('fn detach(self)', 'self.detach_retaining().0')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::detach_retaining', ('        fn detach_retaining(\n            self,\n        ) -> (\n            DetachedTransactionsBlock,\n            concread::release::DeferredRelease,\n        ) {\n            self.assert_unpublished();\n            let Self {\n                block,\n                publication,\n                next_identity,\n                retired_tip: _,\n                publication_started: _,\n                published: _,\n            } = self;\n            let detached = DetachedTransactionsBlock {\n                predecessor_identity: Arc::clone(block._guard.identity()),\n                predecessor: block.latest_block_ref.load_full(),\n                current: Arc::clone(block.current_block.as_ref().expect("admitted membership")),\n                revert: block.revert,\n                publication,\n                next_identity,\n            };\n            let TransactionsBlock {\n                _guard,\n                current_block,\n                ..\n            } = block;\n            let release = _guard.into_release();\n            // The detached journal owns the original staged allocation.\n            drop(current_block);\n            (detached, release)\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'DetachedTransactionsBlock::observe_predecessor', ('storage.write_lock.try_lock()', 'MembershipPredecessorStatus::Busy', 'let guard = storage.released.guard(guard);', 'Arc::ptr_eq(&guard, &self.predecessor_identity)', 'MembershipPredecessorStatus::Current', 'MembershipPredecessorStatus::Changed')),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'DetachedTransactionsBlock::try_prepare_publication', ("        pub(crate) fn try_prepare_publication<'storage, Installation, E>(\n            self,\n            storage: &'storage TransactionsStorage,\n            admit: impl FnOnce(&Self, &TransactionsStorage) -> Result<Installation, E>,\n        ) -> Result<\n            PreparedDetachedTransactionsBlock<'storage, Installation>,\n            (\n                Self,\n                mv::PublicationPreparationError<E>,\n                AbortedTransactions<Installation>,\n            ),\n        > {\n            let mut slot = self.publication_slot(storage);\n            match slot.try_prepare(admit) {\n                Ok(()) => Ok(slot.into_prepared()),\n                Err(error) => {\n                    let original = slot.recover_original();\n                    Err((original, error, slot.into_cleanup()))\n                }\n            }\n        }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'enum', 'MembershipWriterPhase', ("enum MembershipWriterPhase<'storage> {\n    Attached(OriginalMembershipGuard<'storage>),\n    Released(concread::release::DeferredRelease),\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'struct', 'MembershipWriter', ("pub(in crate::state::storage_transactions) struct MembershipWriter<'storage> {\n    phase: Option<MembershipWriterPhase<'storage>>,\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'enum', 'MembershipCapturePhase', ("pub(super) enum MembershipCapturePhase<'storage> {\n    Empty,\n    Attached(TransactionsBlock<'storage>),\n    Prepared(PreparedTransactionsBlock<'storage>),\n    Captured(DetachedTransactionsBlock),\n    Published(PreparedTransactionsBlock<'storage>),\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'struct', 'TransactionsCaptureSlot', ("pub(crate) struct TransactionsCaptureSlot<'storage> {\n    pub(super) phase: MembershipCapturePhase<'storage>,\n    attempted: bool,\n    released: bool,\n    // Last: the original staged payloads precede the successful capture's\n    // deferred notification. All physical siblings must already be free.\n    cleanup: Option<concread::release::DeferredRelease>,\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::new', ("    pub(in crate::state::storage_transactions) fn new(\n        guard: OriginalMembershipGuard<'storage>,\n    ) -> Self {\n        Self {\n            phase: Some(MembershipWriterPhase::Attached(guard)),\n        }\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::identity', ('    pub(in crate::state::storage_transactions) fn identity(&self) -> &Arc<()> {\n        match self.phase.as_ref() {\n            Some(MembershipWriterPhase::Attached(guard)) => guard,\n            _ => panic!("original membership writer was terminally released"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::identity_mut', ('    pub(super) fn identity_mut(&mut self) -> &mut Arc<()> {\n        match self.phase.as_mut() {\n            Some(MembershipWriterPhase::Attached(guard)) => guard,\n            _ => panic!("original membership writer was terminally released"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::release', ('    fn release(&mut self) {\n        match self.phase.take() {\n            Some(MembershipWriterPhase::Attached(guard)) => {\n                // Native unlock does not invoke the original callback.\n                let ((), release) = guard.release_deferred(drop);\n                self.phase = Some(MembershipWriterPhase::Released(release));\n            }\n            other => self.phase = other,\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::into_release', ('    pub(super) fn into_release(mut self) -> concread::release::DeferredRelease {\n        self.take_release()\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::try_prepare', ('    pub(crate) fn try_prepare(&mut self) -> Result<(), TransactionsBlockError> {\n        assert!(\n            !self.attempted && !self.released,\n            "membership capture is one-shot"\n        );\n        self.attempted = true;\n        let MembershipCapturePhase::Attached(block) = &self.phase else {\n            panic!("original attached membership capture");\n        };\n        let publication = block.admit_publication()?;\n        // Same existing identity allocation as standalone prepare_commit.\n        // No second allocation is introduced by capture or terminal release.\n        let next_identity = Arc::new(());\n        // All fallible work precedes extraction. Only original-owner moves\n        // occur until the prepared owner is stored back in the caller slot.\n        let MembershipCapturePhase::Attached(block) =\n            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)\n        else {\n            unreachable!("original checked membership block");\n        };\n        self.phase = MembershipCapturePhase::Prepared(PreparedTransactionsBlock::new(\n            block,\n            publication,\n            next_identity,\n        ));\n        Ok(())\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::try_capture', ('    pub(crate) fn try_capture(&mut self) -> Result<(), TransactionsBlockError> {\n        assert!(!self.released, "membership capture was terminally released");\n        if matches!(&self.phase, MembershipCapturePhase::Attached(_)) {\n            self.try_prepare()?;\n        }\n        // This assertion also excludes an explicitly released prepared block\n        // before taking it out of caller custody.\n        match &self.phase {\n            MembershipCapturePhase::Prepared(prepared) => {\n                prepared.assert_unpublished();\n                prepared.block._guard.identity();\n            }\n            _ => panic!("original prepared membership capture"),\n        }\n        let MembershipCapturePhase::Prepared(prepared) =\n            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)\n        else {\n            unreachable!("original checked membership preparation");\n        };\n        // Existing detach kernel has no callback, user Drop, semantic\n        // refusal or allocation after the prepared owner is extracted.\n        let (journal, release) = prepared.detach_retaining();\n        self.phase = MembershipCapturePhase::Captured(journal);\n        self.cleanup = Some(release);\n        Ok(())\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::release', ('    pub(crate) fn release(&mut self) {\n        self.released = true;\n        let block = match &mut self.phase {\n            MembershipCapturePhase::Attached(block) => Some(block),\n            MembershipCapturePhase::Prepared(prepared)\n            | MembershipCapturePhase::Published(prepared) => Some(&mut prepared.block),\n            MembershipCapturePhase::Captured(_) | MembershipCapturePhase::Empty => None,\n        };\n        if let Some(block) = block {\n            block.release_writers();\n            if self.cleanup.is_none() {\n                self.cleanup = Some(block._guard.take_release());\n            }\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::into_prepared', ('    pub(super) fn into_prepared(mut self) -> PreparedTransactionsBlock<\'storage> {\n        assert!(!self.released, "membership capture was terminally released");\n        match &self.phase {\n            MembershipCapturePhase::Prepared(prepared) => {\n                prepared.assert_unpublished();\n                prepared.block._guard.identity();\n            }\n            _ => panic!("original membership preparation did not complete"),\n        }\n        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {\n            MembershipCapturePhase::Prepared(prepared) => prepared,\n            _ => unreachable!("original checked membership preparation"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::into_detached', ('    pub(crate) fn into_detached(\n        mut self,\n    ) -> (\n        DetachedTransactionsBlock,\n        concread::release::DeferredRelease,\n    ) {\n        assert!(!self.released, "membership capture was terminally released");\n        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {\n            MembershipCapturePhase::Captured(journal) => (\n                journal,\n                self.cleanup\n                    .take()\n                    .expect("original membership capture release"),\n            ),\n            original => {\n                self.phase = original;\n                panic!("original membership capture did not complete");\n            }\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::drop', ('    fn drop(&mut self) {\n        self.release();\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlock::capture_slot', ("    pub(crate) fn capture_slot(self) -> TransactionsCaptureSlot<'storage> {\n        TransactionsCaptureSlot {\n            phase: MembershipCapturePhase::Attached(self),\n            attempted: false,\n            released: false,\n            cleanup: None,\n        }\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlock::release_writers', ('    pub(crate) fn release_writers(&mut self) {\n        self._guard.release();\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::new', ("        fn new(\n            block: TransactionsBlock<'storage>,\n            publication: MembershipPublication,\n            next_identity: Arc<()>,\n        ) -> Self {\n            Self {\n                block,\n                publication,\n                next_identity,\n                retired_tip: None,\n                publication_started: false,\n                published: false,\n            }\n        }",)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::assert_unpublished', ('        fn assert_unpublished(&self) {\n            assert!(\n                !self.publication_started && !self.published,\n                "membership publication was already attempted"\n            );\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::publish_in_place', ('        fn publish_in_place(&mut self) {\n            self.assert_unpublished();\n            self.block._guard.identity();\n            self.publication_started = true;\n            match &self.publication {\n                MembershipPublication::Repeated => {}\n                MembershipPublication::Replace { current } => {\n                    self.block\n                        .blocks_ref\n                        .retain(|_, height| *height < current.height);\n                    self.retired_tip = self.block.latest_block_ref.swap(Some(Arc::clone(current)));\n                }\n                MembershipPublication::Advance { previous, current } => {\n                    if let Some(previous) = previous {\n                        for &transaction in &previous.transactions {\n                            self.block.blocks_ref.insert(transaction, previous.height);\n                        }\n                    }\n                    self.retired_tip = self.block.latest_block_ref.swap(Some(Arc::clone(current)));\n                }\n            }\n            if !matches!(&self.publication, MembershipPublication::Repeated) {\n                // Install the exact pre-admitted identity and retain its exact\n                // predecessor in the same field. No identity is reconstructed.\n                std::mem::swap(self.block._guard.identity_mut(), &mut self.next_identity);\n            }\n            self.block.release_writers();\n            self.published = true;\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedTransactionsBlock::into_retirement', ('        fn into_retirement(self) -> TransactionsPublicationRetirement {\n            assert!(\n                self.published,\n                "original membership publication must complete"\n            );\n            let Self {\n                block,\n                publication,\n                next_identity,\n                retired_tip,\n                publication_started: _,\n                published: _,\n            } = self;\n            let TransactionsBlock {\n                current_block,\n                _guard,\n                ..\n            } = block;\n            let release = _guard.into_release();\n            TransactionsPublicationRetirement {\n                _tip: retired_tip,\n                _staged: current_block,\n                _identity: next_identity,\n                _publication: publication,\n                _release: release,\n            }\n        }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'MembershipWriter::take_release', ('    fn take_release(&mut self) -> concread::release::DeferredRelease {\n        self.release();\n        match self.phase.take() {\n            Some(MembershipWriterPhase::Released(release)) => release,\n            _ => unreachable!("original membership release custody"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::publish_prepared', ('    pub(crate) fn publish_prepared(&mut self) {\n        assert!(!self.released, "membership capture was terminally released");\n        let MembershipCapturePhase::Prepared(prepared) = &mut self.phase else {\n            panic!("original prepared membership publication");\n        };\n        prepared.publish_in_place();\n        self.cleanup = Some(prepared.block._guard.take_release());\n        // No fallible work occurs during this inert successful phase transfer.\n        let MembershipCapturePhase::Prepared(prepared) =\n            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)\n        else {\n            unreachable!("original completed membership publication");\n        };\n        self.phase = MembershipCapturePhase::Published(prepared);\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::executing', ('    fn executing(&self) -> &TransactionsBlock<\'storage> {\n        assert!(\n            !self.attempted && !self.released,\n            "membership execution authority ended"\n        );\n        match &self.phase {\n            MembershipCapturePhase::Attached(block) => block,\n            _ => panic!("original executing membership"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::executing_mut', ('    fn executing_mut(&mut self) -> &mut TransactionsBlock<\'storage> {\n        assert!(\n            !self.attempted && !self.released,\n            "membership execution authority ended"\n        );\n        match &mut self.phase {\n            MembershipCapturePhase::Attached(block) => block,\n            _ => panic!("original executing membership"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsCaptureSlot::into_executing', ('    fn into_executing(mut self) -> TransactionsBlock<\'storage> {\n        self.executing();\n        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {\n            MembershipCapturePhase::Attached(block) => block,\n            _ => unreachable!("checked original executing membership"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'struct', 'TransactionsBlockField', ("pub struct TransactionsBlockField<'storage> {\n    pub(super) slot: TransactionsCaptureSlot<'storage>,\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::new', ("    pub(crate) fn new(block: TransactionsBlock<'storage>) -> Self {\n        Self {\n            slot: block.capture_slot(),\n        }\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::try_prepare_publication', ('    pub(crate) fn try_prepare_publication(&mut self) -> Result<(), TransactionsBlockError> {\n        self.slot.try_prepare()\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::publish_prepared', ('    pub(crate) fn publish_prepared(&mut self) {\n        self.slot.publish_prepared();\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::into_executing', ("    pub(crate) fn into_executing(self) -> TransactionsBlock<'storage> {\n        self.slot.into_executing()\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::into_capture', ("    pub(crate) fn into_capture(self) -> TransactionsCaptureSlot<'storage> {\n        self.slot.executing();\n        self.slot\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::deref', ('    fn deref(&self) -> &Self::Target {\n        self.slot.executing()\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::deref_mut', ('    fn deref_mut(&mut self) -> &mut Self::Target {\n        self.slot.executing_mut()\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/capture.rs', 'method', 'TransactionsBlockField::get', ('    fn get<Q>(&self, key: &Q) -> Option<Value>\n    where\n        Key: Borrow<Q>,\n        Q: Hash + Eq + ?Sized,\n    {\n        self.slot.executing().get(key)\n    }',)),
)

DETACHED = 'crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs'
DETACHED_TESTS = 'crates/iroha_core/src/state/storage_transactions/block/detached_publication_tests.rs'
MEMBERSHIP_BINDINGS += (
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'enum', 'Phase', ("enum Phase<'storage> {\n    Original(DetachedTransactionsBlock),\n    Prepared(PreparedTransactionsBlock<'storage>),\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'struct', 'DetachedTransactionsPublicationSlot', ("pub(crate) struct DetachedTransactionsPublicationSlot<'storage, Installation> {\n    target: &'storage TransactionsStorage,\n    phase: Option<Phase<'storage>>,\n    writer: Option<MembershipWriter<'storage>>,\n    attempted: bool,\n    retryable: bool,\n    complete: bool,\n    released: bool,\n    installation: Option<Installation>,\n    preflight_release: Option<DeferredRelease>,\n    writer_release: Option<DeferredRelease>,\n}",)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsBlock::publication_slot', ("    pub(crate) fn publication_slot<Installation>(\n        self,\n        target: &TransactionsStorage,\n    ) -> DetachedTransactionsPublicationSlot<'_, Installation> {\n        DetachedTransactionsPublicationSlot {\n            target,\n            phase: Some(Phase::Original(self)),\n            writer: None,\n            attempted: false,\n            retryable: true,\n            complete: false,\n            released: false,\n            installation: None,\n            preflight_release: None,\n            writer_release: None,\n        }\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::original', ('    fn original(&self) -> &DetachedTransactionsBlock {\n        match self.phase.as_ref() {\n            Some(Phase::Original(original)) => original,\n            _ => panic!("original detached membership phase"),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::refuse', ('    fn refuse<E>(\n        &mut self,\n        error: PublicationPreparationError<E>,\n    ) -> Result<(), PublicationPreparationError<E>> {\n        self.retryable = true;\n        Err(error)\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::try_prepare', ('    pub(crate) fn try_prepare<E>(\n        &mut self,\n        admit: impl FnOnce(&DetachedTransactionsBlock, &TransactionsStorage) -> Result<Installation, E>,\n    ) -> Result<(), PublicationPreparationError<E>> {\n        assert!(\n            !self.attempted && !self.released,\n            "detached membership preparation is one-shot"\n        );\n        self.attempted = true;\n        // A caught admission panic never grants retry authority.\n        self.retryable = false;\n        let target = self.target;\n        let wait = target.released.observe();\n        let Some(guard) = target.write_lock.try_lock() else {\n            return self.refuse(PublicationPreparationError::after_failed_acquisition(wait));\n        };\n        self.writer = Some(MembershipWriter::new(target.released.guard(guard)));\n        let current = Arc::ptr_eq(\n            self.writer\n                .as_ref()\n                .expect("original observation writer")\n                .identity(),\n            &self.original().predecessor_identity,\n        );\n        // Unlock without notifying. Keep the actual event before admission can\n        // run arbitrary code, refuse, or unwind through the enclosing owner.\n        self.preflight_release = Some(\n            self.writer\n                .take()\n                .expect("original observation writer")\n                .into_release(),\n        );\n        if !current {\n            return self.refuse(PublicationPreparationError::Changed);\n        }\n        let installation = match admit(self.original(), target) {\n            Ok(installation) => installation,\n            Err(error) => return self.refuse(PublicationPreparationError::Admission(error)),\n        };\n        self.installation = Some(installation);\n        let wait = target.released.observe();\n        let Some(guard) = target.write_lock.try_lock() else {\n            return self.refuse(PublicationPreparationError::after_failed_acquisition(wait));\n        };\n        // Install the actual final acquisition before checking its predecessor.\n        self.writer = Some(MembershipWriter::new(target.released.guard(guard)));\n        if !Arc::ptr_eq(\n            self.writer\n                .as_ref()\n                .expect("original publication writer")\n                .identity(),\n            &self.original().predecessor_identity,\n        ) {\n            return self.refuse(PublicationPreparationError::Changed);\n        }\n        // All checks precede extraction. These moves reuse the admitted action,\n        // immutable payload and next identity; no new admission or allocation.\n        let Some(Phase::Original(original)) = self.phase.take() else {\n            unreachable!("checked original membership phase");\n        };\n        let DetachedTransactionsBlock {\n            predecessor_identity: _,\n            predecessor: _,\n            current,\n            revert,\n            publication,\n            next_identity,\n        } = original;\n        self.phase = Some(Phase::Prepared(PreparedTransactionsBlock::new(\n            TransactionsBlock {\n                latest_block_ref: &target.latest_block,\n                blocks_ref: &target.blocks,\n                _guard: self\n                    .writer\n                    .take()\n                    .expect("checked original publication writer"),\n                revert,\n                current_block: Some(current),\n            },\n            publication,\n            next_identity,\n        )));\n        self.complete = true;\n        self.retryable = true;\n        Ok(())\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::recover_original', ('    pub(crate) fn recover_original(&mut self) -> DetachedTransactionsBlock {\n        assert!(\n            self.retryable && !self.released,\n            "unwound or released membership is not retry authority"\n        );\n        if let Some(Phase::Prepared(prepared)) = self.phase.as_ref() {\n            prepared.assert_unpublished();\n        }\n        self.released = true;\n        self.complete = false;\n        if let Some(writer) = self.writer.take() {\n            self.writer_release = Some(writer.into_release());\n        }\n        match self.phase.take().expect("original membership journal") {\n            Phase::Original(original) => original,\n            Phase::Prepared(prepared) => {\n                let (original, release) = prepared.detach_retaining();\n                self.writer_release = Some(release);\n                original\n            }\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::release_writers', ('    pub(crate) fn release_writers(&mut self) {\n        self.released = true;\n        self.retryable = false;\n        self.complete = false;\n        if let Some(writer) = self.writer.take() {\n            self.writer_release = Some(writer.into_release());\n        }\n        if let Some(Phase::Prepared(prepared)) = self.phase.as_mut() {\n            prepared.block.release_writers();\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::into_prepared', ('    pub(crate) fn into_prepared(\n        mut self,\n    ) -> PreparedDetachedTransactionsBlock<\'storage, Installation> {\n        assert!(\n            self.complete && !self.released,\n            "complete original membership preparation"\n        );\n        let Some(Phase::Prepared(prepared)) = self.phase.take() else {\n            unreachable!("checked prepared membership phase");\n        };\n        self.released = true;\n        PreparedDetachedTransactionsBlock {\n            prepared,\n            installation: self.installation.take().expect("original installation"),\n            preflight_release: self.preflight_release.take(),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::into_cleanup', ('    pub(super) fn into_cleanup(mut self) -> AbortedTransactions<Installation> {\n        assert!(\n            self.released && self.phase.is_none(),\n            "original membership was recovered"\n        );\n        AbortedTransactions {\n            _installation: self.installation.take(),\n            _preflight_release: self.preflight_release.take(),\n            _release: self.writer_release.take(),\n        }\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions/block/detached_publication.rs', 'method', 'DetachedTransactionsPublicationSlot::drop', ('    fn drop(&mut self) {\n        self.release_writers();\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'PreparedDetachedTransactionsBlock', ("    pub(crate) struct PreparedDetachedTransactionsBlock<'storage, Installation> {\n        prepared: PreparedTransactionsBlock<'storage>,\n        installation: Installation,\n        preflight_release: Option<concread::release::DeferredRelease>,\n    }",)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'struct', 'AbortedTransactions', ('    pub(crate) struct AbortedTransactions<Installation> {\n        _installation: Option<Installation>,\n        _preflight_release: Option<concread::release::DeferredRelease>,\n        _release: Option<concread::release::DeferredRelease>,\n    }',)),
    ('crates/iroha_core/src/state/storage_transactions.rs', 'method', 'PreparedDetachedTransactionsBlock::abort', ('        pub(crate) fn abort(\n            self,\n        ) -> (DetachedTransactionsBlock, AbortedTransactions<Installation>) {\n            let Self {\n                prepared,\n                installation,\n                preflight_release,\n            } = self;\n            let (journal, release) = prepared.detach_retaining();\n            (\n                journal,\n                AbortedTransactions {\n                    _release: Some(release),\n                    _installation: Some(installation),\n                    _preflight_release: preflight_release,\n                },\n            )\n        }',)),
)

MEMBERSHIP_SOURCE_RELATIVES = (
    Path("scripts/formal/sumeragi_v2_multilane_membership_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_membership_contract_test.py"),
    Path(STATE), Path(STORAGE), Path(CAPTURE), Path(CAPTURE_TESTS),
    Path(DETACHED), Path(DETACHED_TESTS),
)


def validate_membership_contract(
    root: Path, models: Any, errors: list[str], rust_binding_item: Callable,
) -> None:
    """Require original writer ownership, exact admission and infallible consumption."""

    for name in MODELS:
        owners = [m for m in models if isinstance(m, dict) and m.get("module") == name]
        bindings = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
        for path, kind, symbol, tokens in MEMBERSHIP_BINDINGS:
            matches = [b for b in bindings if isinstance(b, dict)
                       and (b.get("path"), b.get("kind"), b.get("symbol")) == (path, kind, symbol)]
            if len(matches) != 1:
                errors.append(f"membership ledger {name} owner {symbol} must occur exactly once")
            elif tuple(matches[0].get("required_tokens", ())) != tokens:
                errors.append(f"membership reviewed tokens changed for {name}::{symbol}")

    items: dict[str, str] = {}
    raw_items: dict[str, str] = {}
    for path, kind, symbol, tokens in MEMBERSHIP_BINDINGS + (
        (STATE, "fn", "commit_inner", ()),
        (STATE, "struct", "StateBlockFields", ()),
    ):
        item = rust_binding_item(root, path, kind, symbol, "membership ownership", errors)
        if item is not None:
            raw_items[symbol] = item
            items[symbol] = _code(item)
            for token in tokens:
                if _code(token) not in items[symbol]:
                    errors.append(f"membership {symbol} is missing executable relation {token!r}")

    def require(symbol: str, relation: str) -> None:
        if symbol in items and _code(relation) not in items[symbol]:
            errors.append(f"membership {symbol} is missing executable relation {relation!r}")

    require("TransactionsBlock::prepare_commit", "fn prepare_commit(self,)")
    require("TransactionsBlock::prepare_commit",
            "let mut capture = self.capture_slot(); capture.try_prepare()?; Ok(capture.into_prepared())")
    require("TransactionsBlock::admit_publication",
            "if !self.revert && previous_block.as_ref().is_some_and(|previous_block| { previous_block.height == current_block.height && previous_block.transactions == current_block.transactions }) { return Ok(MembershipPublication::Repeated); }")
    require("TransactionsBlock::admit_publication",
            "if expected_current_height != current_height { return Err(TransactionsBlockError::HeightMismatch {")
    require("TransactionsBlock::admit_publication",
            "if self.revert { Ok(MembershipPublication::Replace { current: Arc::clone(current_block), }) } else { Ok(MembershipPublication::Advance { previous: previous_block, current: Arc::clone(current_block), }) }")
    require("PreparedTransactionsBlock::publish",
            "fn publish(mut self) -> TransactionsPublicationRetirement { self.publish_in_place(); self.into_retirement() }")
    require("TransactionsBlock::admit_publication",
            "self._guard.identity(); let previous_block = self.latest_block_ref.load_full();")
    require("PreparedTransactionsBlock::publish_in_place", "MembershipPublication::Repeated => {}")
    require("PreparedTransactionsBlock::publish_in_place",
            "self.assert_unpublished(); self.block._guard.identity(); self.publication_started = true; match &self.publication")
    require("PreparedTransactionsBlock::publish_in_place",
            "if !matches!(&self.publication, MembershipPublication::Repeated) { std::mem::swap(self.block._guard.identity_mut(), &mut self.next_identity); }")
    require("PreparedTransactionsBlock::publish_in_place",
            "self.block.release_writers(); self.published = true;")
    require("PreparedTransactionsBlock::detach", "self.detach_retaining().0")
    require("PreparedTransactionsBlock::detach_retaining", "let release = _guard.into_release();")
    require("DetachedTransactionsBlock::observe_predecessor",
            "let Some(guard) = storage.write_lock.try_lock() else { return MembershipPredecessorStatus::Busy; }; let guard = storage.released.guard(guard); if Arc::ptr_eq(&guard, &self.predecessor_identity)")
    require("DetachedTransactionsBlock::try_prepare_publication",
            "let mut slot = self.publication_slot(storage); match slot.try_prepare(admit)")
    require("DetachedTransactionsBlock::try_prepare_publication",
            "let original = slot.recover_original(); Err((original, error, slot.into_cleanup()))")
    require("PreparedTransactionsBlock::as_block",
            "fn as_block(&self) -> &TransactionsBlock<'storage> { &self.block }")

    prepared = raw_items.get("PreparedTransactionsBlock", "")
    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+\w+\s*:", prepared) or "&mut" in prepared:
        errors.append("membership prepared owner exposes mutable authority")
    detached = raw_items.get("DetachedTransactionsBlock", "")
    if re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+\w+\s*:", detached) or "&mut" in detached:
        errors.append("membership detached owner exposes mutable authority")
    if any(owner in detached for owner in ("MutexGuard", "ReleaseGuard", ": TransactionsBlock<")):
        errors.append("membership detached owner retains a physical writer")
    for forbidden in ("admit_publication(", "validate_commit(", "prepare_commit(",
                      "load_full(", "Err(", "Result<", "?", ".lock("):
        if any(forbidden in items.get(symbol, "") for symbol in (
            "PreparedTransactionsBlock::publish", "PreparedTransactionsBlock::publish_in_place",
        )):
            errors.append(f"membership publication repeats admission or can refuse: {forbidden}")

    # Both the inherent aggregate delegate and trait delegate must preserve the
    # same terminal release engine. The exact parser deliberately rejects an
    # ambiguous Type::method, so verify the two defining bodies explicitly.
    from check_sumeragi_v2_multilane_models import _extract_rust_binding_items
    capture_source = (root / CAPTURE).read_text(encoding="utf-8")
    delegates = _extract_rust_binding_items(
        capture_source, "method", "TransactionsBlockField::release_writers",
    )
    expected_delegates = (
        "pub(crate) fn release_writers(&mut self) { self.slot.release(); }",
        "fn release_writers(&mut self) { self.slot.release(); }",
    )
    if tuple(map(_code, delegates)) != tuple(map(_code, expected_delegates)):
        errors.append("membership terminal release delegates differ from original slot")
    for owner in ("TransactionsBlock", "TransactionsBlockField"):
        for method in ("publish_in_place", "publish_prepared"):
            definitions = _extract_rust_binding_items(
                capture_source + "\n" + (root / STORAGE).read_text(encoding="utf-8"),
                "method", f"{owner}::{method}",
            )
            if owner == "TransactionsBlock" and definitions:
                errors.append("membership executing block exposes borrowed publication")
            if owner == "TransactionsBlockField" and method == "publish_prepared":
                if len(definitions) != 1 or _code(definitions[0]) != _code(
                    "pub(crate) fn publish_prepared(&mut self) { self.slot.publish_prepared(); }"
                ):
                    errors.append("membership publication wrapper exposes external authority")

    # The actual attached State owner retains membership and every sibling
    # throughout refusal/unwind. No consumed membership temporary is recreated.
    require("StateBlockFields",
            "pub transactions: storage_transactions::TransactionsBlockField<'state>")
    commit = items.get("commit_inner", "")
    cursor = 0
    for relation in (
        "let mut publication_notice = self.state_ref.state_view_publication();",
        "let state_commit_authorization = state_commit_authorization;",
        "let mut world_effects = None;",
        "let mut commit_fence = self.state_ref.state_commit_lock.defer_notifications();",
        "let mut write_fence = self.state_write_lock.defer_notifications();",
        "let mut lifecycle_fence = self.state_ref.lane_lifecycle_lock.defer_notifications();",
        "hash_budget.with_deferred_refund_notifications(|_| {",
        "let mut this = self;",
        "} = this.fields.as_mut().expect(\"original executing State\");",
        "let _state_commit_lock = commit_fence.lock();",
        "let tx_validate_result = transactions.try_prepare_publication();",
        "tx_validate_result?;",
        "let autoscale_lifecycle_guard",
        "autoscale_retirement_queue_veto.as_mut()",
        "world_effects = Some(world_commit::PreparedWorldCommit::prepare_overlay(",
        "state_ref.apply_committed_autoscale_lane_geometry(",
        "block_hashes.try_prepare_publication().map_err(",
        "world.prepare_publication();",
        "canonical_runtime.prepare_publication();",
        "let _view_generation = publication_notice.begin();",
        "transactions.publish_prepared();",
        "canonical_runtime.publish_prepared();",
        "world.publish_prepared();",
        "block_hashes.publish_prepared();",
        "drop(_state_commit_lock);",
    ):
        normalized = _code(relation)
        position = commit.find(normalized, cursor)
        if position < 0 or commit.count(normalized) != 1:
            errors.append(f"membership State publication misses or reorders {relation!r}")
            break
        cursor = position + len(normalized)
    if "into_fields(" in commit or "transactions.prepare_commit(" in commit:
        errors.append("membership State publication consumes its original attached owner")
