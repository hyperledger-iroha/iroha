//! Original prepaid tree physical custody, retained by the membership writer.
use super::{history::*, *};
use concread::bptree::{
    BptreeMapAbandonment, BptreeMapCommitRetirement, BptreeMapCommitSlot, BptreeMapOwnedAcquisition,
};
use concread::release::{DeferredRelease, ReleaseGuard};

type Acquired<'a> = ReleaseGuard<'a, BptreeMapOwnedAcquisition<'a, Key, Value, Mode>>;
type Preparing<'a> = ReleaseGuard<'a, BptreeMapCommitSlot<'a, Key, Value, Mode>>;
type Abandoned = BptreeMapAbandonment<Key, Value, Mode>;
type Retired = BptreeMapCommitRetirement<Key, Value, Mode>;
enum Phase<'a> {
    Original(Work),
    Acquired(Acquired<'a>),
    Preparing(Preparing<'a>),
    Abandoned(Abandoned),
    Published(Retired),
}

/// Native payload cleanup and actual notices, all after physical release.
pub(super) struct Cleanup {
    _pending: Option<Pending>,
    _abandoned: Option<Abandoned>,
    _retired: Option<Retired>,
    _reader: Option<DeferredRelease>,
    _writer: Option<DeferredRelease>,
    _loan: Option<DeferredRelease>,
}

pub(super) struct Slot<'a> {
    target: &'a TransactionsStorage,
    pending: Option<Pending>,
    phase: Option<Phase<'a>>,
    prepared: bool,
    reader_release: Option<DeferredRelease>,
    writer_release: Option<DeferredRelease>,
    loan_release: Option<DeferredRelease>,
}
impl<'a> Slot<'a> {
    pub(super) fn new(target: &'a TransactionsStorage, mut pending: Pending) -> Self {
        let work = pending
            .work
            .take()
            .expect("original complete private successor");
        Self {
            target,
            pending: Some(pending),
            phase: Some(Phase::Original(work)),
            prepared: false,
            reader_release: None,
            writer_release: None,
            loan_release: None,
        }
    }
    pub(super) fn next_sequence(&self) -> u64 {
        self.pending
            .as_ref()
            .expect("original pending metadata")
            .next_sequence
    }
    pub(super) fn next_identity(&mut self) -> Identity {
        self.pending
            .as_mut()
            .expect("original pending metadata")
            .next_identity
            .take()
            .expect("prepaid next identity")
    }
    pub(super) fn prepare(&mut self) -> Result<(), MembershipAdmissionError> {
        assert!(!self.prepared, "original history preparation is one-shot");
        if matches!(self.phase, Some(Phase::Original(_))) {
            let Some(Phase::Original(work)) = self.phase.take() else {
                unreachable!()
            };
            let wait = self.target.released.observe();
            let acquired = match self.target.blocks.try_acquire_owned_retained(work) {
                Ok(acquired) => self.target.released.guard(acquired),
                Err((work, error)) => {
                    self.phase = Some(Phase::Original(work));
                    return Err(physical_error(error, wait));
                }
            };
            self.phase = Some(Phase::Acquired(acquired));
        }
        if matches!(self.phase, Some(Phase::Acquired(_))) {
            let Some(Phase::Acquired(acquired)) = self.phase.take() else {
                unreachable!()
            };
            let wait = self.target.released.observe();
            let writer = match acquired.try_map_preserving_release(|a| a.validate()) {
                Ok(writer) => writer,
                Err((acquired, error)) => {
                    self.phase = Some(Phase::Acquired(acquired));
                    return Err(physical_error(error, wait));
                }
            };
            self.phase = Some(Phase::Preparing(
                writer.map_preserving_release(|w| w.commit_slot()),
            ));
        }
        let wait = self.target.blocks.observe_reader_release();
        let Some(Phase::Preparing(slot)) = self.phase.as_mut() else {
            unreachable!()
        };
        slot.try_prepare()
            .map_err(|error| physical_error(error, wait))?;
        self.prepared = true;
        Ok(())
    }
    pub(super) fn is_prepared(&self) -> bool {
        self.prepared
    }
    pub(super) fn publish(&mut self) {
        assert!(self.prepared, "original prepared membership history");
        let Some(Phase::Preparing(slot)) = self.phase.take() else {
            unreachable!()
        };
        let (retired, writer) =
            slot.release_deferred(|slot| slot.into_prepared().publish().release());
        self.writer_release = Some(writer);
        self.phase = Some(Phase::Published(retired));
        self.prepared = false;
    }
    /// Release every physical tree owner while retaining all payloads/notices.
    pub(super) fn release(&mut self) {
        self.prepared = false;
        self.phase = match self.phase.take() {
            Some(Phase::Acquired(acquired)) => {
                let (work, writer) = acquired.release_deferred(|a| a.abort());
                self.writer_release = Some(writer);
                Some(Phase::Original(work))
            }
            Some(Phase::Preparing(slot)) => {
                let ((work, reader), writer) = slot.release_deferred(|slot| {
                    let (writer, reader) = slot.abort_retaining();
                    (writer.abort_retaining(), reader)
                });
                self.reader_release = reader;
                self.writer_release = Some(writer);
                Some(Phase::Abandoned(work))
            }
            phase => phase,
        };
    }
    /// Detach the same original cursor after normal admitted preparation.
    pub(super) fn recover(&mut self) -> Pending {
        self.prepared = false;
        let work = match self.phase.take().expect("original tree custody") {
            Phase::Original(work) => work,
            Phase::Acquired(acquired) => {
                let (work, writer) = acquired.release_deferred(|a| a.abort());
                self.writer_release = Some(writer);
                work
            }
            Phase::Preparing(slot) => {
                let ((work, reader), writer) = slot.release_deferred(|slot| {
                    let (writer, reader) = slot.abort_retaining();
                    (writer.detach(), reader)
                });
                self.reader_release = reader;
                self.writer_release = Some(writer);
                work
            }
            _ => panic!("terminal history is not retry authority"),
        };
        let mut pending = self.pending.take().expect("original pending metadata");
        pending.work = Some(work);
        pending
    }
    /// A detached semantic journal owns its cursor independently; it releases
    /// only the preparation-slot loan, retaining the actual notice in cleanup.
    pub(super) fn recover_detached(&mut self) -> Pending {
        let mut pending = self.recover();
        self.loan_release = pending.release_loan();
        pending
    }
    pub(super) fn cleanup(&mut self) -> Cleanup {
        self.release();
        let mut pending = self.pending.take();
        let mut abandoned = None;
        let mut retired = None;
        match self.phase.take() {
            Some(Phase::Original(work)) => {
                pending.as_mut().expect("original metadata").work = Some(work)
            }
            Some(Phase::Abandoned(work)) => abandoned = Some(work),
            Some(Phase::Published(work)) => retired = Some(work),
            None => {}
            _ => unreachable!("all physical history owners released"),
        }
        Cleanup {
            _pending: pending,
            _abandoned: abandoned,
            _retired: retired,
            _reader: self.reader_release.take(),
            _writer: self.writer_release.take(),
            _loan: self.loan_release.take(),
        }
    }
}
impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.release();
    }
}
