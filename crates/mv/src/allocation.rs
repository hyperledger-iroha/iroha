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
    cell::Cell,
    fmt, ptr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{ReleaseNotification, ReleaseWait};

/// Original-budget owners around the existing charged map engine.
pub mod map;

mod buffer;

pub use buffer::{ChargedBuffer, ChargedBufferError};

thread_local! {
    // Scope records live on this thread's stack; registration allocates nothing.
    static REFUND_SCOPES: Cell<*const RefundScope> = const { Cell::new(ptr::null()) };
}

struct RefundScope {
    pool: *const Pool,
    previous: *const RefundScope,
    pending: Cell<bool>,
}

// This borrow keeps the stack record at its registered address. Neither the
// record nor this guard escapes the synchronous closure API or crosses threads.
struct EnteredRefundScope<'scope> {
    scope: &'scope RefundScope,
    pool: &'scope Pool,
}

impl Drop for EnteredRefundScope<'_> {
    fn drop(&mut self) {
        REFUND_SCOPES.with(|head| {
            debug_assert_eq!(head.get(), ptr::from_ref(self.scope));
            head.set(self.scope.previous);
        });
        // Unlink and end the TLS access before invoking any user callback. A
        // matching outer scope receives this wake; other threads are unaffected.
        if self.scope.pending.get() {
            self.pool.notify_refund();
        }
    }
}

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
        // Credits become reusable immediately, even if this thread must defer
        // notification until its physical writer guards have been released.
        let previous = self.reserved.fetch_sub(bytes, Ordering::AcqRel);
        debug_assert!(previous >= bytes, "allocation custody cannot refund twice");
        self.notify_refund();
    }

    fn notify_refund(&self) {
        let deferred = REFUND_SCOPES.with(|head| {
            let mut current = head.get();
            while !current.is_null() {
                // SAFETY: only this thread accesses its TLS chain. Each record
                // is borrowed at a stable stack address by EnteredRefundScope,
                // whose destructor unlinks it before that borrow or pool ends.
                let scope = unsafe { &*current };
                if ptr::eq(scope.pool, self) {
                    scope.pending.set(true);
                    return true;
                }
                current = scope.previous;
            }
            false
        });
        if !deferred {
            // No TLS access or pool lock remains while Waker::wake can reenter.
            drop(self.released.guard(()));
        }
    }
}

/// Borrowed proof that one original pool defers refunds on this thread.
///
/// Only the budget's synchronous callback creates this token. Physical owners
/// which borrow it cannot escape the callback or move to another thread.
/// Detached owners may leave after their physical guards have been released.
///
/// ```compile_fail
/// let budget = mv::allocation::AllocationBudget::new(1024);
/// let escaped = budget.with_deferred_refund_notifications(|scope| scope);
/// drop(escaped);
/// ```
///
/// The scope cannot be used from a different thread:
/// ```compile_fail
/// let budget = mv::allocation::AllocationBudget::new(1024);
/// budget.with_deferred_refund_notifications(|scope| {
///     std::thread::scope(|threads| {
///         threads.spawn(move || { std::hint::black_box(scope); });
///     });
/// });
/// ```
pub struct AllocationScope<'scope> {
    budget: &'scope AllocationBudget,
    // Refund deferral is thread-local. Even a scoped thread cannot borrow this
    // token to acquire writers whose refunds would notify on another thread.
    _thread: std::marker::PhantomData<*mut ()>,
}

impl AllocationScope<'_> {
    pub(crate) fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        Arc::ptr_eq(&self.budget.pool, &budget.pool)
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
    // Equality of actual retained pool owners, never a caller-supplied digest
    // or the address of a movable AllocationBudget handle.
    fn same_pool(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pool, &other.pool)
    }

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

    /// Defer this thread's refund notifications through a synchronous operation.
    ///
    /// Freed allocation credits become available immediately. Only wakes for
    /// this exact pool wait until the closure returns or unwinds. Nested scopes
    /// for the same pool coalesce; other pools and other threads notify normally.
    /// Entering, recording refunds and leaving the scope allocate no storage.
    /// User waker callbacks can still allocate or panic when notification runs.
    ///
    /// Acquire and release every physical guard inside the closure. Do not keep
    /// an enclosing guard held or return one from the closure: the budget cannot
    /// infer lock ownership. Detached allocation owners may escape after their
    /// physical guards have been released. The callback receives a borrowed,
    /// thread-bound token for APIs that enforce this lifetime in their physical
    /// owner types. This does not make an async future execute inside the scope.
    pub fn with_deferred_refund_notifications<R>(
        &self,
        operation: impl for<'scope> FnOnce(&'scope AllocationScope<'scope>) -> R,
    ) -> R {
        let scope = RefundScope {
            pool: Arc::as_ptr(&self.pool),
            previous: REFUND_SCOPES.with(Cell::get),
            pending: Cell::new(false),
        };
        let entered = EnteredRefundScope {
            scope: &scope,
            pool: &self.pool,
        };
        REFUND_SCOPES.with(|head| head.set(ptr::from_ref(&scope)));
        let capability = AllocationScope {
            budget: self,
            _thread: std::marker::PhantomData,
        };
        let output = operation(&capability);
        drop(entered);
        output
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
        self.try_reserve_bytes(bytes)
    }

    /// Prepay an already checked sum of concrete requested allocation layouts.
    ///
    /// Use this when allocation-free planning has accumulated a complete demand
    /// without retaining a collection of layouts. The sum may exceed the maximum
    /// size of one allocation; it is not represented by a fabricated aggregate
    /// `Layout`. Each actual allocation still splits its exact layout from the
    /// returned original reservation. This method does not infer nested storage
    /// or validate a caller's payload-cloning policy.
    pub fn try_reserve_bytes(
        &self,
        bytes: usize,
    ) -> Result<AllocationReservation, AllocationRefusal> {
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

    /// Whether this original reservation belongs to the exact budget pool.
    /// Equal limits or available-byte observations never establish this identity.
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        Arc::ptr_eq(&self.pool, &budget.pool)
    }

    /// Move part of one already prepaid sum into a second move-only reservation.
    ///
    /// No pool CAS, allocation, refund or notification occurs. Both remainders
    /// retain the same original pool and together own exactly the previous sum.
    /// A refused partition leaves the original owner unchanged.
    pub fn try_partition_bytes(&mut self, bytes: usize) -> Result<Self, InsufficientReservation> {
        if bytes > self.remaining {
            return Err(InsufficientReservation {
                requested_bytes: bytes,
                remaining_bytes: self.remaining,
            });
        }
        self.remaining -= bytes;
        Ok(Self {
            pool: Arc::clone(&self.pool),
            remaining: bytes,
        })
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

/// An allocation or component partition exceeds this owner's prepaid remainder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InsufficientReservation {
    /// Requested allocation layout size or checked component layout sum.
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

#[cfg(test)]
pub(crate) use tests::without_allocations;
