//! Original Queue policy residence and physically funded shared transaction controls.
//!
//! The encoded-size expansion is a resident admission estimate, not a ledger for
//! nested decoded allocations. Only the actual ledger/control layouts below are
//! physically prepaid from the admitting State's original finite execution pool.
//! TODO: Carry the full canonical owned proof/decoder ledger through accepted,
//! block and gossip successors; this resident estimate does not fund their graph.

use super::*;
use iroha_allocation::shared::{Reserved, Shared};
use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedShared};

/// The budget a queue resident counts against (`specs/parliament_private_ballot_design.md`
/// §3.6): the ordinary queue's capacity and byte budget, or the anonymous-ballot partition's
/// own (`[queue.anonymous_ballots]`). It is a function of the resident's entrypoint, so its
/// admission and every exit name the same partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ResidentPartition {
    /// Every input but a signerless ballot.
    Ordinary,
    /// A signerless anonymous ballot.
    Ballot,
}

impl ResidentPartition {
    /// The partition of `tx`.
    pub(super) fn of(tx: &crate::tx::AcceptedTransaction<'_>) -> Self {
        if matches!(tx.entrypoint(), TransactionEntrypoint::AnonymousBallot(_)) {
            Self::Ballot
        } else {
            Self::Ordinary
        }
    }
}

/// The resident counters of one partition.
#[derive(Debug, Default)]
pub(super) struct ResidentCounters {
    /// Retained-byte estimates of its original owners, including detached last readers.
    pub(super) retained_bytes: AtomicU64,
    /// Its tracked residents.
    pub(super) active_count: AtomicUsize,
    /// Its admission-FIFO entries.
    pub(super) queued_count: AtomicUsize,
}

/// The original Queue pressure counters, retained by every original transaction owner.
///
/// Each resident is counted in exactly one partition, from its reservation to its last
/// refund. The coarse backpressure reads the ordinary counters only, so no claim, residence,
/// release, removal or last-reader refund of a ballot moves ordinary pressure, whatever its
/// interleaving with ordinary admission and publication.
pub(super) struct QueueResidentLedger {
    /// The ordinary queue's residents.
    pub(super) ordinary: ResidentCounters,
    /// The anonymous-ballot partition's residents. The partition's own admission budget is
    /// its pending index (`queue::anonymous_ballots`).
    pub(super) ballots: ResidentCounters,
    /// Latched original admission health, refreshed at the existing publication boundary.
    pub(super) faulted: AtomicBool,
    mutations: AtomicUsize,
    budget: AllocationBudget,
    capacity: NonZeroUsize,
    max_retained_bytes: NonZeroU64,
    backpressure: watch::Sender<BackpressureState>,
}

impl QueueResidentLedger {
    /// The counters of `partition`.
    pub(super) fn counters(&self, partition: ResidentPartition) -> &ResidentCounters {
        match partition {
            ResidentPartition::Ordinary => &self.ordinary,
            ResidentPartition::Ballot => &self.ballots,
        }
    }

    /// Tracked residents of both partitions.
    pub(super) fn active_count(&self) -> usize {
        self.ordinary
            .active_count
            .load(Ordering::Relaxed)
            .saturating_add(self.ballots.active_count.load(Ordering::Relaxed))
    }

    /// Retained-byte estimates of both partitions.
    pub(super) fn retained_bytes(&self) -> u64 {
        self.ordinary
            .retained_bytes
            .load(Ordering::Relaxed)
            .saturating_add(self.ballots.retained_bytes.load(Ordering::Relaxed))
    }

    fn backpressure_state(&self) -> BackpressureState {
        // The ballot partition has its own budget: only ordinary residents gate admission.
        let ordinary = &self.ordinary;
        let queued = ordinary.queued_count.load(Ordering::Relaxed);
        // Match QueuePressureSnapshot::into_backpressure: age remains in the
        // rich Queue snapshot/telemetry and does not gate coarse admission.
        let saturated = self.faulted.load(Ordering::Acquire)
            || ordinary.active_count.load(Ordering::Relaxed) >= self.capacity.get()
            || ordinary
                .retained_bytes
                .load(Ordering::Relaxed)
                .saturating_add(TX_RETAINED_OVERHEAD_BYTES)
                > self.max_retained_bytes.get();
        if saturated {
            BackpressureState::Saturated {
                queued,
                capacity: self.capacity,
            }
        } else {
            BackpressureState::Healthy {
                queued,
                capacity: self.capacity,
            }
        }
    }

    /// Sample the actual counters inside the original watch publication fence.
    pub(super) fn publish_backpressure(&self) -> BackpressureState {
        self.backpressure.send_if_modified(|current| {
            let state = self.backpressure_state();
            if *current == state {
                false
            } else {
                *current = state;
                true
            }
        });
        self.backpressure_state()
    }

    /// Defer watch wakes until all enclosing original Queue mutation guards release.
    pub(super) fn mutation(&self) -> ResidentMutation<'_> {
        self.mutations.fetch_add(1, Ordering::AcqRel);
        ResidentMutation { ledger: self }
    }

    fn refund_estimate(&self, partition: ResidentPartition, cost: u64) {
        let retained_bytes = &self.counters(partition).retained_bytes;
        let mut current = retained_bytes.load(Ordering::Relaxed);
        loop {
            match retained_bytes.compare_exchange_weak(
                current,
                current.saturating_sub(cost),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
        if self.mutations.load(Ordering::Acquire) == 0 {
            self.publish_backpressure();
        }
    }
}

/// Borrowed fence lifetime; it owns no graph and allocates no replacement control.
pub(super) struct ResidentMutation<'queue> {
    ledger: &'queue QueueResidentLedger,
}

impl Drop for ResidentMutation<'_> {
    fn drop(&mut self) {
        if self.ledger.mutations.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.ledger.publish_backpressure();
        }
    }
}

/// Custody of one exact shared shell and its original Queue resident estimate.
/// Shared destroys the checked input and frees its shell before dropping this charge.
pub(super) struct QueueResidentCharge {
    _physical: AllocationCharge,
    ledger: ChargedShared<QueueResidentLedger>,
    /// The partition charged with the estimate; the refund returns it to the same one.
    partition: ResidentPartition,
    estimated_bytes: u64,
}

impl Drop for QueueResidentCharge {
    fn drop(&mut self) {
        self.ledger
            .refund_estimate(self.partition, self.estimated_bytes);
    }
}

/// One exact original admission, physically funded inside its shared input shell.
pub(super) struct QueueAdmission {
    checked: CheckedTransaction<'static>,
    incarnation: u64,
}

/// The production Queue's sole shared accepted owner; cloning retains the original graph.
///
/// Proposal builders and gossip borrow these handles instead of copying transaction graphs;
/// a handle keeps the original resident charge until its last reader drops it. The private
/// module bounds it to the crate-visible re-export in `queue`.
pub struct QueuedTransaction {
    original: Shared<QueueAdmission, QueueResidentCharge>,
}

impl QueuedTransaction {
    /// Exact funded shell, including the immutable admission identity.
    pub(super) fn allocation_layout() -> std::alloc::Layout {
        Shared::<QueueAdmission, QueueResidentCharge>::layout()
    }

    /// The non-reusable identity assigned by the original Queue under its mutation fence.
    pub(super) fn incarnation(&self) -> u64 {
        self.original.incarnation
    }

    /// Reserve the concrete shell before moving or publishing the checked transaction, and
    /// charge its residence estimate to `partition`.
    pub(super) fn reserve(
        ledger: &ChargedShared<QueueResidentLedger>,
        encoded_len: usize,
        partition: ResidentPartition,
    ) -> Result<Reserved<QueueAdmission, QueueResidentCharge>, Error> {
        let layout = Self::allocation_layout();
        let mut reservation = ledger
            .budget
            .try_reserve(layout)
            .map_err(|original| Error::Deferred(original.into()))?;
        let physical = reservation
            .try_split(layout)
            .expect("exact admitted original Queue shell");
        let cost = Queue::retained_byte_cost(encoded_len);
        ledger
            .counters(partition)
            .retained_bytes
            .fetch_add(cost, Ordering::Relaxed);
        let charge = QueueResidentCharge {
            _physical: physical,
            ledger: ledger.clone(),
            partition,
            estimated_bytes: cost,
        };
        Reserved::try_new(charge).map_err(|(charge, _refusal)| {
            drop(charge);
            Error::Deferred(ivm::error::ExecutionDeferral::AllocationUnavailable.into())
        })
    }

    pub(super) fn initialize(
        shell: Reserved<QueueAdmission, QueueResidentCharge>,
        checked: CheckedTransaction<'static>,
        incarnation: u64,
    ) -> Self {
        Self {
            original: shell.initialize(QueueAdmission {
                checked,
                incarnation,
            }),
        }
    }

    /// Whether both handles share the exact same original admission.
    pub(crate) fn ptr_eq(first: &Self, second: &Self) -> bool {
        Shared::ptr_eq(&first.original, &second.original)
    }
}

impl Clone for QueuedTransaction {
    fn clone(&self) -> Self {
        Self {
            original: self.original.clone(),
        }
    }
}

impl std::ops::Deref for QueuedTransaction {
    type Target = CheckedTransaction<'static>;
    fn deref(&self) -> &Self::Target {
        &self.original.checked
    }
}

impl AsRef<CheckedTransaction<'static>> for QueuedTransaction {
    fn as_ref(&self) -> &CheckedTransaction<'static> {
        self
    }
}

/// Let selection and lease code take shared handles and owned copies alike.
impl std::borrow::Borrow<crate::tx::AcceptedTransaction<'static>> for QueuedTransaction {
    fn borrow(&self) -> &crate::tx::AcceptedTransaction<'static> {
        self.original.checked.as_accepted()
    }
}

impl Queue {
    /// Initialize only under the original admission mutation fence, from its State pool.
    pub(super) fn resident_ledger(
        &self,
        budget: &AllocationBudget,
    ) -> Result<&ChargedShared<QueueResidentLedger>, Error> {
        if let Some(ledger) = self.resident_accounting.get() {
            if !ledger.belongs_to(budget) {
                return Err(Error::AdmissionInvariant {
                    reason: "Queue resident custody belongs to a different original State pool"
                        .to_owned(),
                });
            }
            return Ok(ledger);
        }
        {
            // Startup can reserve before the first actual admission initializes custody.
            // Borrow the inline pool binding only; no charged owner drops under this lock.
            let binding = self.sumeragi_wake.lock();
            #[cfg(all(test, sumeragi_core_mutation = "HC192"))]
            let _ = &binding;
            #[cfg(not(all(test, sumeragi_core_mutation = "HC192")))]
            if binding
                .as_ref()
                .is_some_and(|original| !original.belongs_to(budget))
            {
                return Err(Error::AdmissionInvariant {
                    reason: "Queue resident custody belongs to a different original State pool"
                        .to_owned(),
                });
            }
        }
        let layout = ChargedShared::<QueueResidentLedger>::allocation_layout();
        let mut reservation = budget
            .try_reserve(layout)
            .map_err(|original| Error::Deferred(original.into()))?;
        let shell = ChargedShared::<QueueResidentLedger>::reserve_from(&mut reservation).map_err(
            |error| {
                Error::Deferred(match error {
                    iroha_allocation::PrepaidSharedError::Allocator { .. } => {
                        ivm::error::ExecutionDeferral::AllocationUnavailable.into()
                    }
                    iroha_allocation::PrepaidSharedError::Reservation(_) => {
                        ivm::error::ExecutionDeferral::LocalInvariantViolation.into()
                    }
                })
            },
        )?;
        #[cfg(test)]
        let retained_bytes = self.retained_bytes_before_admission.load(Ordering::Relaxed);
        #[cfg(not(test))]
        let retained_bytes = 0;
        let ledger = shell.initialize(QueueResidentLedger {
            ordinary: ResidentCounters {
                retained_bytes: AtomicU64::new(retained_bytes),
                ..ResidentCounters::default()
            },
            ballots: ResidentCounters::default(),
            faulted: AtomicBool::new(self.admission_faulted()),
            mutations: AtomicUsize::new(0),
            budget: budget.clone(),
            capacity: self.capacity,
            max_retained_bytes: self.max_retained_bytes,
            backpressure: self.backpressure_tx.clone(),
        });
        // The original Queue mutation fence serializes this one initialization.
        assert!(self.resident_accounting.set(ledger).is_ok());
        Ok(self
            .resident_accounting
            .get()
            .expect("published original Queue resident ledger"))
    }

    /// Inspect original custody under the actual Queue mutation fence, including cold startup.
    /// The locked callback must not reacquire that fence. The second callback runs after
    /// its original guard drops, but before mutation/watch and original-pool refunds.
    pub(super) fn with_resident_refunds<T, R>(
        &self,
        locked: impl FnOnce() -> T,
        after_unlock: impl FnOnce(T) -> R,
    ) -> R {
        #[cfg(not(all(test, sumeragi_core_mutation = "HC138")))]
        let guard = self.push_remove_lock.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC138"))]
        let guard: Option<parking_lot::MutexGuard<'_, ()>> = None;
        if let Some(ledger) = self.resident_accounting.get() {
            ledger.budget.with_deferred_refund_notifications(|_| {
                let _mutation = ledger.mutation();
                // Declare this owned original guard after the mutation owner so
                // unwind also releases the writer before any refund/watch callback.
                let guard = guard;
                let completed = locked();
                drop(guard);
                after_unlock(completed)
            })
        } else {
            // Admission cannot initialize the ledger while this cold writer is held.
            let completed = locked();
            drop(guard);
            after_unlock(completed)
        }
    }
}
