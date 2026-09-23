//! Prepaid historical membership and original resumable pre-World preparation.
use super::*;
use concread::bptree::{
    AllocationDemand, BptreeMap, BptreeMapOwned, BptreeMapReadTxn, ClonePlanning,
    MapAdmissionError, NodeCloning, NodeFunding, OwnedWriteError, PlanningError, Prepaid,
};
use concread::shared::{Reserved, Shared};
use mv::allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation, ChargedBuffer,
    ChargedBufferError, ChargedBufferFromChargeError,
};

#[derive(Debug, Default)]
pub(super) struct IdentityState {
    loaned: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    retirement_observer: std::sync::OnceLock<Arc<()>>,
}
#[cfg(test)]
impl IdentityState {
    /// Observe this actual payload's retirement without retaining the charged owner.
    /// The marker is allocated only when a destruction-observer test requests it.
    pub(super) fn observe_retirement_for_tests(&self) -> std::sync::Weak<()> {
        Arc::downgrade(self.retirement_observer.get_or_init(|| Arc::new(())))
    }
}
pub(super) type Identity = Shared<IdentityState, AllocationCharge>;
pub(super) type Mode = Prepaid<Policy>;
pub(super) type Map = BptreeMap<Key, Value, Mode>;
pub(super) type Work = BptreeMapOwned<Key, Value, Mode>;
pub(super) type Reader<'a> = BptreeMapReadTxn<'a, Key, Value, Mode>;

/// Local resource refusal, never a deterministic transaction or block verdict.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MembershipAdmissionError {
    /// The original membership or history lock is owned by another operation.
    #[error("transaction membership is busy")]
    Busy(concread::release::ReleaseWait),
    /// The original configured pool refused this allocation.
    #[error("transaction membership capacity: {0}")]
    Capacity(AllocationRefusal),
    /// Checked native allocation planning failed.
    #[error("transaction membership planning failed: {0:?}")]
    Planning(PlanningError),
    /// A failed mutation poisoned the original native generation.
    #[error("transaction membership history is poisoned")]
    Poisoned,
    /// A different committed membership generation replaced this predecessor.
    #[error("transaction membership predecessor changed")]
    Changed(concread::release::ReleaseWait),
    /// The allocator refused an already prepaid membership allocation.
    #[error("transaction membership allocator refused {requested_bytes} bytes")]
    Allocator {
        /// Actual requested layout size.
        requested_bytes: usize,
    },
}
impl MembershipAdmissionError {
    /// Borrow only the original resource release that can permit a retry.
    pub fn release_wait(&self) -> Option<&concread::release::ReleaseWait> {
        match self {
            Self::Busy(wait) | Self::Changed(wait) => Some(wait),
            Self::Capacity(AllocationRefusal::Capacity { release, .. }) => Some(release),
            _ => None,
        }
    }
}
impl From<ChargedBufferError> for MembershipAdmissionError {
    fn from(error: ChargedBufferError) -> Self {
        match error {
            ChargedBufferError::Admission(e) => Self::Capacity(e),
            ChargedBufferError::Allocator { requested_bytes } => {
                Self::Allocator { requested_bytes }
            }
        }
    }
}
pub(super) fn physical_error(
    error: OwnedWriteError,
    wait: concread::release::ReleaseWait,
) -> MembershipAdmissionError {
    match error {
        OwnedWriteError::Busy => MembershipAdmissionError::Busy(wait),
        OwnedWriteError::Changed => MembershipAdmissionError::Changed(wait),
        OwnedWriteError::Poisoned => MembershipAdmissionError::Poisoned,
    }
}
pub(super) fn edit_error(
    error: MapAdmissionError<AllocationRefusal>,
    wait: concread::release::ReleaseWait,
) -> MembershipAdmissionError {
    match error {
        MapAdmissionError::Busy => MembershipAdmissionError::Busy(wait),
        MapAdmissionError::Changed => MembershipAdmissionError::Changed(wait),
        MapAdmissionError::Poisoned => MembershipAdmissionError::Poisoned,
        MapAdmissionError::Planning(e) => MembershipAdmissionError::Planning(e),
        MapAdmissionError::Refused(e) => MembershipAdmissionError::Capacity(e),
    }
}

pub(super) struct Policy(AllocationReservation);
impl NodeFunding for Policy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: std::alloc::Layout) -> AllocationCharge {
        self.0
            .try_split(layout)
            .expect("complete native membership plan")
    }
}
impl NodeCloning<Key, Value> for Policy {
    fn clone_key(&mut self, key: &Key) -> Key {
        *key
    }
    fn clone_value(&mut self, value: &Value) -> Value {
        *value
    }
}
impl ClonePlanning<Key, Value> for Policy {
    fn plan_key(_: &Key, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
    fn plan_value(_: &Value, _: &mut AllocationDemand) -> Result<(), PlanningError> {
        Ok(())
    }
}
pub(super) fn admit(
    budget: &AllocationBudget,
    demand: AllocationDemand,
) -> Result<Policy, AllocationRefusal> {
    budget.try_reserve_bytes(demand.bytes()).map(Policy)
}
#[cfg(any(test, feature = "iroha-core-tests"))]
pub(super) fn new_identity(
    budget: &AllocationBudget,
) -> Result<Identity, MembershipAdmissionError> {
    let layout = Identity::layout();
    let mut reservation = budget
        .try_reserve(layout)
        .map_err(MembershipAdmissionError::Capacity)?;
    Ok(Identity::new(
        IdentityState::default(),
        reservation
            .try_split(layout)
            .expect("exact identity layout"),
    ))
}

/// One original preparation. A capacity retry resumes this same cursor/batch;
/// it cannot rebuild already completed work against a newly sampled cut.
pub(crate) struct Pending {
    pub(super) predecessor: Identity,
    pub(super) latest: Option<Tip>,
    pub(super) replacement: bool,
    pub(super) next_sequence: u64,
    batch: Option<ChargedBuffer<Key>>,
    next: usize,
    pub(super) work: Option<Work>,
    pub(super) next_identity: Option<Identity>,
    pub(super) baseline: Option<concread::bptree::BptreeMapRetainedPredecessor<Key, Value, Mode>>,
    leased: bool,
    // Last: clear the original loan and retire payloads before its actual wake.
    lease_release: Option<concread::release::DeferredRelease>,
    pub(super) attachment_releases: concread::release::DeferredReleaseBatch,
}
impl Pending {
    fn new(
        predecessor: Identity,
        latest: Option<Tip>,
        replacement: bool,
        next_sequence: u64,
        attachment_releases: concread::release::DeferredReleaseBatch,
    ) -> Self {
        Self {
            predecessor,
            latest,
            replacement,
            next_sequence,
            batch: None,
            next: 0,
            work: None,
            next_identity: None,
            baseline: None,
            leased: false,
            lease_release: None,
            attachment_releases,
        }
    }
    pub(super) fn release_loan(&mut self) -> Option<concread::release::DeferredRelease> {
        if self.leased {
            self.predecessor.loaned.store(false, Ordering::Release);
            self.leased = false;
        }
        self.lease_release.take()
    }
    fn advance(&mut self, storage: &TransactionsStorage) -> Result<(), MembershipAdmissionError> {
        if self.work.is_none() {
            let len = if self.replacement {
                0
            } else {
                self.latest.as_ref().map_or(0, |b| b.transactions.len())
            };
            let batch_layout = std::alloc::Layout::array::<Key>(len).map_err(|_| {
                MembershipAdmissionError::Capacity(AllocationRefusal::DemandOverflow)
            })?;
            let identity_layout = Identity::layout();
            let wait = storage.released.observe();
            let writer = storage
                .blocks
                .try_write_admitted_with_footprint(|existing, additional| {
                    let required = existing
                        .bytes()
                        .checked_add(additional.bytes())
                        .and_then(|n| n.checked_add(batch_layout.size()))
                        .and_then(|n| n.checked_add(identity_layout.size().checked_mul(2)?))
                        .ok_or(MembershipAdmissionError::Capacity(
                            AllocationRefusal::DemandOverflow,
                        ))?;
                    if required > storage.budget.limit_bytes() {
                        return Err(MembershipAdmissionError::Capacity(
                            AllocationRefusal::ExceedsLimit {
                                requested_bytes: required,
                                limit_bytes: storage.budget.limit_bytes(),
                            },
                        ));
                    }
                    // The callback precedes every cursor shell allocation. Fund
                    // its exact demand with the missing original batch and next
                    // identity in one pool acquisition before sorting or building
                    // any of them. Already retained siblings remain owned by this
                    // Pending across a later private-tree insertion refusal.
                    let initial_bytes = additional
                        .bytes()
                        .checked_add(if self.batch.is_none() {
                            batch_layout.size()
                        } else {
                            0
                        })
                        .and_then(|n| {
                            n.checked_add(if self.next_identity.is_none() {
                                identity_layout.size()
                            } else {
                                0
                            })
                        })
                        .ok_or(MembershipAdmissionError::Capacity(
                            AllocationRefusal::DemandOverflow,
                        ))?;
                    let mut reservation = storage
                        .budget
                        .try_reserve_bytes(initial_bytes)
                        .map_err(MembershipAdmissionError::Capacity)?;
                    if self.next_identity.is_none() {
                        let charge = reservation
                            .try_split(identity_layout)
                            .expect("complete original identity layout was admitted");
                        let shell = Reserved::<IdentityState, AllocationCharge>::try_new(charge)
                            .map_err(|(_charge, error)| MembershipAdmissionError::Allocator {
                                requested_bytes: error.layout().size(),
                            })?;
                        self.next_identity = Some(shell.initialize(IdentityState::default()));
                    }
                    if self.batch.is_none() {
                        let charge = reservation
                            .try_split(batch_layout)
                            .expect("complete original batch layout was admitted");
                        let mut batch = ChargedBuffer::try_from_charge(len, charge).map_err(
                            |(_charge, error)| match error {
                                ChargedBufferFromChargeError::Allocator { layout } => {
                                    MembershipAdmissionError::Allocator {
                                        requested_bytes: layout.size(),
                                    }
                                }
                                ChargedBufferFromChargeError::DemandOverflow
                                | ChargedBufferFromChargeError::LayoutMismatch { .. } => {
                                    unreachable!("checked original batch layout and charge")
                                }
                            },
                        )?;
                        if !self.replacement
                            && let Some(latest) = &self.latest
                        {
                            for key in &latest.transactions {
                                batch
                                    .append(std::slice::from_ref(key))
                                    .expect("exact preceding tip count");
                            }
                        }
                        batch.as_mut_slice().sort_unstable();
                        self.batch = Some(batch);
                    }
                    debug_assert_eq!(reservation.remaining_bytes(), additional.bytes());
                    Ok(Policy(reservation))
                })
                .map_err(|error| match error {
                    MapAdmissionError::Busy => MembershipAdmissionError::Busy(wait.clone()),
                    MapAdmissionError::Poisoned => MembershipAdmissionError::Poisoned,
                    MapAdmissionError::Changed => MembershipAdmissionError::Changed(wait.clone()),
                    MapAdmissionError::Planning(e) => MembershipAdmissionError::Planning(e),
                    MapAdmissionError::Refused(e) => e,
                })?;
            self.baseline = Some(writer.predecessor().retain());
            self.work = Some(writer.detach());
        }
        let batch = self.batch.as_ref().expect("original funded batch");
        let work = self.work.as_mut().expect("original successor");
        while self.next < batch.as_slice().len() {
            let key = batch.as_slice()[self.next];
            let height = self
                .latest
                .as_ref()
                .expect("nonempty promotion has a tip")
                .height;
            let wait = storage.released.observe();
            let result = work.try_insert_admitted(key, height, |d| admit(&storage.budget, d));
            if let Err((_, error)) = result {
                // Distinguish a permanently impossible original successor from
                // credits retained by other readers. This floor is diagnostic,
                // not an authorization and is computed only after refusal.
                let error = edit_error(error, wait);
                if let MembershipAdmissionError::Capacity(AllocationRefusal::Capacity {
                    requested_bytes,
                    ..
                }) = error
                {
                    let floor = work
                        .required_allocation_floor()
                        .map_err(MembershipAdmissionError::Planning)?
                        .bytes();
                    let batch_bytes = std::alloc::Layout::array::<Key>(batch.capacity())
                        .map_err(|_| {
                            MembershipAdmissionError::Capacity(AllocationRefusal::DemandOverflow)
                        })?
                        .size();
                    let required = floor
                        .checked_add(batch_bytes)
                        .and_then(|n| n.checked_add(Identity::layout().size() * 2))
                        .and_then(|n| n.checked_add(requested_bytes))
                        .ok_or(MembershipAdmissionError::Capacity(
                            AllocationRefusal::DemandOverflow,
                        ))?;
                    if required > storage.budget.limit_bytes() {
                        return Err(MembershipAdmissionError::Capacity(
                            AllocationRefusal::ExceedsLimit {
                                requested_bytes: required,
                                limit_bytes: storage.budget.limit_bytes(),
                            },
                        ));
                    }
                }
                return Err(error);
            }
            self.next += 1;
        }
        Ok(())
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        // The actual release remains a last field, after the flag and all payloads.
        if self.leased {
            self.predecessor.loaned.store(false, Ordering::Release);
        }
    }
}

impl TransactionsStorage {
    /// Construct the sole historical representation from an explicit original pool.
    pub fn try_new(budget: AllocationBudget) -> Result<Self, MembershipAdmissionError> {
        budget.with_deferred_refund_notifications(|_| {
            let mut identity_charge = None;
            let identity_layout = Identity::layout();
            let blocks = Map::try_new_with_node_custody(|demand| {
                let bytes = demand
                    .bytes()
                    .checked_add(identity_layout.size())
                    .ok_or(AllocationRefusal::DemandOverflow)?;
                let mut reservation = budget.try_reserve_bytes(bytes)?;
                identity_charge = Some(
                    reservation
                        .try_split(identity_layout)
                        .expect("exact initial identity"),
                );
                Ok::<_, AllocationRefusal>(Policy(reservation))
            })
            .map_err(MembershipAdmissionError::Capacity)?;
            let identity = Identity::new(
                IdentityState::default(),
                identity_charge.expect("initial identity admitted with native root"),
            );
            Ok(Self {
                latest_block: TipStore::default(),
                blocks,
                write_lock: Mutex::new(identity),
                released: Default::default(),
                budget: budget.clone(),
                pending: Mutex::new(None),
                publication_sequence: AtomicU64::new(0),
            })
        })
    }
    /// Return the unchanged admission owner after a later physical attachment
    /// refusal. The caller has already released World and all sibling writers.
    pub(crate) fn retain_preparation(&self, mut original: Pending) {
        self.budget.with_deferred_refund_notifications(|_| {
            // This method is called only after the enclosing aggregate releases
            // all physical fences. No other accepted preparation may be displaced.
            let guard = self.released.guard(self.write_lock.lock());
            let mut pending = self.pending.lock();
            let current = Identity::ptr_eq(&original.predecessor, &guard);
            if current {
                assert!(
                    original.leased && pending.is_none(),
                    "exclusive original preparation loan"
                );
            }
            let notice = original.release_loan();
            let attachments = std::mem::replace(
                &mut original.attachment_releases,
                self.released.deferred_batch(),
            );
            let stale = if current {
                *pending = Some(original);
                None
            } else {
                Some(original)
            };
            drop(pending);
            drop(guard);
            drop(stale);
            drop(attachments);
            drop(notice);
        });
    }
    /// Complete the original previous-tip batch and private generation before
    /// acquiring World or any State effect/publication fence. The returned owner
    /// holds no physical writer; attaching it still checks its exact predecessor.
    pub(crate) fn prepare_next_block(
        &self,
        replacement: bool,
    ) -> Result<Pending, MembershipAdmissionError> {
        self.budget.with_deferred_refund_notifications(|_| {
            // This custody is declared before either lock so actual retained
            // notices also retire after both locks on unwind, not only success.
            let mut retired = None;
            let wait = self.released.observe();
            let guard = self
                .write_lock
                .try_lock()
                .ok_or_else(|| MembershipAdmissionError::Busy(wait.clone()))?;
            let guard = self.released.guard(guard);
            if guard.loaned.load(Ordering::Acquire) {
                let identity = guard.clone();
                drop(guard);
                // Observe after this read-only acquisition releases; its own
                // release must not turn an outstanding loan into a busy loop.
                let after_release = self.released.observe();
                return Err(MembershipAdmissionError::Busy(
                    if identity.loaned.load(Ordering::Acquire) {
                        after_release
                    } else {
                        wait
                    },
                ));
            }
            let next_sequence = self
                .publication_sequence
                .load(Ordering::Relaxed)
                .checked_add(2)
                .ok_or(MembershipAdmissionError::Planning(PlanningError::Overflow))?;
            let mut pending = self.pending.lock();
            if pending.as_ref().is_some_and(|p| {
                !Identity::ptr_eq(&p.predecessor, &guard) || p.replacement != replacement
            }) {
                retired = pending.take();
            }
            let original = pending.get_or_insert_with(|| {
                Pending::new(
                    guard.clone(),
                    self.latest_block.load_full(),
                    replacement,
                    next_sequence,
                    self.released.deferred_batch(),
                )
            });
            #[cfg(test)]
            tests::panic_before_advance();
            let result = original.advance(self);
            let mut ready = result
                .is_ok()
                .then(|| pending.take().expect("completed original preparation"));
            if let Some(ready) = ready.as_mut() {
                ready.predecessor.loaned.store(true, Ordering::Release);
                ready.leased = true;
            }
            drop(pending);
            if let Some(ready) = ready.as_mut() {
                ready.lease_release = Some(guard.release_deferred(drop).1);
            } else {
                drop(guard);
            }
            drop(retired);
            result?;
            Ok(ready.expect("successful original preparation"))
        })
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
