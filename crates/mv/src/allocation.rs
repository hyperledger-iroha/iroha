//! Finite requested-allocation credits with release-driven retry.
//!
//! Enumerate real allocation layouts before constructing their objects, reserve
//! their combined demand once, then split the prepaid owner into charges held
//! by those allocations through actual deallocation. A charge is not a lifetime
//! wrapper: its consumer must bind it to the real allocation owner, such as the
//! charged Concread EBR cell. Returning a charge at publication is too early.
//!
//! This accounts requested layout bytes, not RSS. Nested payloads, allocator and
//! collector bookkeeping, and this budget's fixed control storage are not
//! inferred. Their complete admission remains the integrating caller's job.

use std::{
    alloc::Layout,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{ReleaseNotification, ReleaseWait};

struct Pool {
    limit: usize,
    reserved: AtomicUsize,
    released: ReleaseNotification,
}

impl Pool {
    fn refund(&self, bytes: usize) {
        if bytes == 0 {
            return;
        }
        // Signal only after credits become available. Observations acquired
        // before a failed reservation cannot lose a concurrent refund.
        let signal = self.released.guard(());
        let previous = self.reserved.fetch_sub(bytes, Ordering::AcqRel);
        debug_assert!(previous >= bytes, "allocation custody cannot refund twice");
        drop(signal);
    }
}

/// One immutable finite allocation limit shared by its outstanding owners.
///
/// Clones refer to the same pool. Dropping the budget handle does not invalidate
/// outstanding reservations or allocation charges. Zero allows only zero-byte
/// layouts; it never means unlimited.
#[derive(Clone)]
pub struct AllocationBudget {
    pool: Arc<Pool>,
}

impl AllocationBudget {
    /// Construct a pool with an explicit finite requested-byte limit.
    pub fn new(limit_bytes: usize) -> Self {
        Self {
            pool: Arc::new(Pool {
                limit: limit_bytes,
                reserved: AtomicUsize::new(0),
                released: ReleaseNotification::default(),
            }),
        }
    }

    /// Return the immutable policy limit in requested allocation bytes.
    pub fn limit_bytes(&self) -> usize {
        self.pool.limit
    }

    /// Observe credits currently held by prepaid or allocated owners.
    /// This snapshot is diagnostic and grants no allocation permission.
    pub fn reserved_bytes(&self) -> usize {
        self.pool.reserved.load(Ordering::Acquire)
    }

    /// Prepay one exact allocation layout without allocating its payload.
    pub fn try_reserve(&self, layout: Layout) -> Result<AllocationReservation, AllocationRefusal> {
        self.try_reserve_layouts([layout])
    }

    /// Prepay all supplied layouts atomically before any payload construction.
    ///
    /// This method retains no collection of layouts. Their checked sum must fit
    /// the finite policy limit. Refusal changes no credits; a temporary capacity
    /// refusal includes the original pool's pre-probe release observation.
    pub fn try_reserve_layouts(
        &self,
        layouts: impl IntoIterator<Item = Layout>,
    ) -> Result<AllocationReservation, AllocationRefusal> {
        let bytes = layouts.into_iter().try_fold(0_usize, |total, layout| {
            total
                .checked_add(layout.size())
                .ok_or(AllocationRefusal::DemandOverflow)
        })?;
        if bytes > self.pool.limit {
            return Err(AllocationRefusal::ExceedsLimit {
                requested_bytes: bytes,
                limit_bytes: self.pool.limit,
            });
        }
        let release = self.pool.released.observe();
        let mut reserved = self.pool.reserved.load(Ordering::Acquire);
        loop {
            // Subtraction is safe because every credit acquisition checks this
            // same immutable limit and only original owners can refund credits.
            if bytes > self.pool.limit - reserved {
                return Err(AllocationRefusal::Capacity {
                    requested_bytes: bytes,
                    reserved_bytes: reserved,
                    limit_bytes: self.pool.limit,
                    release,
                });
            }
            match self.pool.reserved.compare_exchange_weak(
                reserved,
                reserved + bytes,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(AllocationReservation {
                        pool: Arc::clone(&self.pool),
                        remaining: bytes,
                    });
                }
                Err(current) => reserved = current,
            }
        }
    }
}

impl fmt::Debug for AllocationBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllocationBudget")
            .field("limit_bytes", &self.limit_bytes())
            .field("reserved_bytes", &self.reserved_bytes())
            .finish()
    }
}

/// Local resource refusal, independent of proposal validity or publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AllocationRefusal {
    /// The requested layout sum is not representable; no credits were changed.
    DemandOverflow,
    /// This demand cannot fit even after every outstanding allocation is freed.
    ExceedsLimit {
        /// Checked requested layout sum.
        requested_bytes: usize,
        /// Immutable pool limit.
        limit_bytes: usize,
    },
    /// Other outstanding owners temporarily occupy the required capacity.
    Capacity {
        /// Checked requested layout sum.
        requested_bytes: usize,
        /// Diagnostic occupied credits at the refused probe.
        reserved_bytes: usize,
        /// Immutable pool limit.
        limit_bytes: usize,
        /// Retry hint tied to this exact pool; it grants no future reservation.
        release: ReleaseWait,
    },
}

impl fmt::Display for AllocationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DemandOverflow => f.write_str("allocation layout demand overflows"),
            Self::ExceedsLimit {
                requested_bytes,
                limit_bytes,
            } => write!(
                f,
                "allocation demand {requested_bytes} exceeds finite limit {limit_bytes}"
            ),
            Self::Capacity {
                requested_bytes,
                reserved_bytes,
                limit_bytes,
                ..
            } => write!(
                f,
                "allocation demand {requested_bytes} unavailable: {reserved_bytes}/{limit_bytes} reserved"
            ),
        }
    }
}

impl std::error::Error for AllocationRefusal {}

/// Prepaid aggregate demand, split before constructing its actual allocations.
///
/// Dropping this owner refunds only its unused remainder. Charges already moved
/// into allocations remain reserved until those allocation owners release them.
#[must_use = "retain prepaid credits until split into actual allocation owners"]
pub struct AllocationReservation {
    pool: Arc<Pool>,
    remaining: usize,
}

impl AllocationReservation {
    /// Return prepaid bytes not yet moved into an allocation charge.
    pub fn remaining_bytes(&self) -> usize {
        self.remaining
    }

    /// Move one exact layout's credits into an independent allocation owner.
    /// No pool acquisition or payload allocation occurs here. Refusal preserves
    /// the complete original reservation for a corrected split or abandonment.
    pub fn try_split(
        &mut self,
        layout: Layout,
    ) -> Result<AllocationCharge, InsufficientReservation> {
        let bytes = layout.size();
        if bytes > self.remaining {
            return Err(InsufficientReservation {
                requested_bytes: bytes,
                remaining_bytes: self.remaining,
            });
        }
        self.remaining -= bytes;
        Ok(AllocationCharge {
            pool: Arc::clone(&self.pool),
            layout,
        })
    }
}

impl fmt::Debug for AllocationReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllocationReservation")
            .field("remaining_bytes", &self.remaining)
            .finish_non_exhaustive()
    }
}

impl Drop for AllocationReservation {
    fn drop(&mut self) {
        self.pool.refund(self.remaining);
    }
}

/// An attempted allocation exceeds this owner's remaining prepaid demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsufficientReservation {
    /// Exact requested layout size.
    pub requested_bytes: usize,
    /// Original prepaid remainder, unchanged by refusal.
    pub remaining_bytes: usize,
}

impl fmt::Display for InsufficientReservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "allocation needs {} bytes but only {} prepaid bytes remain",
            self.requested_bytes, self.remaining_bytes
        )
    }
}

impl std::error::Error for InsufficientReservation {}

/// Move-only credits belonging to one concrete requested allocation layout.
///
/// Bind this owner to the allocation's actual deallocator. The charged EBR
/// implementation does so across writer abort, commit and deferred reclamation.
/// This value deliberately cannot be cloned or detached into a refundable count.
#[must_use = "move this charge into its actual allocation owner"]
pub struct AllocationCharge {
    pool: Arc<Pool>,
    layout: Layout,
}

impl AllocationCharge {
    /// Return the exact layout whose requested bytes remain prepaid.
    pub fn layout(&self) -> Layout {
        self.layout
    }
}

impl fmt::Debug for AllocationCharge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllocationCharge")
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

impl Drop for AllocationCharge {
    fn drop(&mut self) {
        self.pool.refund(self.layout.size());
    }
}

#[cfg(test)]
#[path = "allocation_tests.rs"]
mod tests;
