//! Finite custody of explicitly requested GPU payload storage and submitted work.
//!
//! Driver context and module internals are opaque: these pools account for
//! explicit host, pinned-host and device buffers, not total driver residency.
//! Process/context/module owners require separate finite cardinality bounds.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use iroha_allocation::AllocationBudget;
use iroha_allocation::AllocationReservation;
use parking_lot::RwLock;
#[cfg(any(feature = "cuda", test))]
use std::sync::atomic::AtomicBool;

use crate::limits::GpuResourceLimits;

/// Checked complete storage request established before allocating an aggregate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[allow(
    clippy::struct_field_names,
    reason = "the `*_bytes` names deliberately mirror the public GpuResourceLimits, \
              WorkRequest and ProcessUsage byte fields these values flow between"
)]
pub struct BufferRequest {
    pub(crate) host_bytes: usize,
    pub(crate) pinned_bytes: usize,
    pub(crate) device_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceRefusal {
    InFlight,
    #[cfg(any(feature = "cuda", test))]
    Host,
    #[cfg(any(feature = "cuda", test))]
    Pinned,
    #[cfg(any(feature = "cuda", test))]
    Device,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceUsage {
    pub(crate) reserved: BufferRequest,
    pub(crate) peak: BufferRequest,
    pub(crate) in_flight: usize,
    pub(crate) peak_in_flight: usize,
}

/// One original process budget: reload mutates its limits, never its identity.
#[derive(Debug)]
pub struct ResourcePools {
    pub(crate) host: AllocationBudget,
    pinned: AllocationBudget,
    pub(crate) device: AllocationBudget,
    in_flight_limit: RwLock<usize>,
    in_flight: AtomicUsize,
    peak_in_flight: AtomicUsize,
    pub(crate) unified_bytes: AtomicUsize,
    pub(crate) peak_unified_bytes: AtomicUsize,
}

impl ResourcePools {
    pub(crate) fn new(limits: GpuResourceLimits) -> Arc<Self> {
        Arc::new(Self {
            host: AllocationBudget::new(limits.host_bytes),
            pinned: AllocationBudget::new(limits.pinned_bytes),
            device: AllocationBudget::new(limits.device_bytes),
            in_flight_limit: RwLock::new(limits.in_flight),
            in_flight: AtomicUsize::new(0),
            peak_in_flight: AtomicUsize::new(0),
            unified_bytes: AtomicUsize::new(0),
            peak_unified_bytes: AtomicUsize::new(0),
        })
    }

    /// Reconfigure only admission; outstanding resources retain original charges.
    /// Call without physical driver/cache locks held. Admission does not wait for
    /// another execution to release capacity, including a parent execution.
    pub(crate) fn set_limits(&self, limits: GpuResourceLimits) {
        self.host.set_limit_bytes(limits.host_bytes);
        self.pinned.set_limit_bytes(limits.pinned_bytes);
        self.device.set_limit_bytes(limits.device_bytes);
        *self.in_flight_limit.write() = limits.in_flight;
    }

    #[cfg(any(feature = "cuda", test))]
    pub(crate) fn try_reserve(
        self: &Arc<Self>,
        request: BufferRequest,
    ) -> Result<ResourceReservation, ResourceRefusal> {
        let in_flight = InFlightPermit::acquire(self)?;
        // Partial admission rolls back without allocating payload storage. Every
        // pool is separate; the count permit never masquerades as a byte charge.
        let host = self
            .host
            .try_reserve_bytes(request.host_bytes)
            .map_err(|_| ResourceRefusal::Host)?;
        let pinned = self
            .pinned
            .try_reserve_bytes(request.pinned_bytes)
            .map_err(|_| ResourceRefusal::Pinned)?;
        let device = self
            .device
            .try_reserve_bytes(request.device_bytes)
            .map_err(|_| ResourceRefusal::Device)?;
        Ok(ResourceReservation {
            host,
            pinned,
            device,
            _in_flight: in_flight,
        })
    }

    pub(crate) fn try_reserve_host(
        &self,
        bytes: usize,
    ) -> Result<AllocationReservation, crate::HostOutputError> {
        self.host
            .try_reserve_bytes(bytes)
            .map_err(|_| crate::HostOutputError::Capacity)
    }

    pub(crate) fn usage(&self) -> ResourceUsage {
        ResourceUsage {
            reserved: BufferRequest {
                host_bytes: self.host.reserved_bytes(),
                pinned_bytes: self.pinned.reserved_bytes(),
                device_bytes: self.device.reserved_bytes(),
            },
            peak: BufferRequest {
                host_bytes: self.host.peak_reserved_bytes(),
                pinned_bytes: self.pinned.peak_reserved_bytes(),
                device_bytes: self.device.peak_reserved_bytes(),
            },
            in_flight: self.in_flight.load(Ordering::Acquire),
            peak_in_flight: self.peak_in_flight.load(Ordering::Acquire),
        }
    }
}

pub struct InFlightPermit {
    pools: Arc<ResourcePools>,
}

impl InFlightPermit {
    pub(crate) fn acquire(pools: &Arc<ResourcePools>) -> Result<Self, ResourceRefusal> {
        let limit = pools
            .in_flight_limit
            .try_read()
            .ok_or(ResourceRefusal::InFlight)?;
        let previous = pools
            .in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                (used < *limit).then(|| used + 1)
            })
            .map_err(|_| ResourceRefusal::InFlight)?;
        pools
            .peak_in_flight
            .fetch_max(previous + 1, Ordering::Relaxed);
        Ok(Self {
            pools: Arc::clone(pools),
        })
    }
}

impl Drop for InFlightPermit {
    fn drop(&mut self) {
        self.pools.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Prepaid original-pool storage; held through the actual allocation lifetime.
/// The reservation covers the whole checked aggregate, including temporary
/// construction buffers, before any allocation or growth can begin.
#[cfg(any(feature = "cuda", test))]
pub struct ResourceReservation {
    pub(crate) host: AllocationReservation,
    pub(crate) pinned: AllocationReservation,
    pub(crate) device: AllocationReservation,
    // The permit's destructor releases the original aggregate work count.
    _in_flight: InFlightPermit,
}

#[cfg(any(feature = "cuda", test))]
impl ResourceReservation {
    #[cfg(test)]
    pub(crate) fn request(&self) -> BufferRequest {
        BufferRequest {
            host_bytes: self.host.remaining_bytes(),
            pinned_bytes: self.pinned.remaining_bytes(),
            device_bytes: self.device.remaining_bytes(),
        }
    }

    #[cfg(test)]
    #[allow(
        clippy::used_underscore_binding,
        reason = "`_in_flight` is held only for its Drop in production builds; this \
                  test-only accessor is its sole reader"
    )]
    pub(crate) fn owner(&self) -> &Arc<ResourcePools> {
        &self._in_flight.pools
    }
}

/// Physical failure state for exactly one context generation; not kernel policy.
#[derive(Debug, Default)]
#[cfg(any(feature = "cuda", test))]
pub struct DeviceHealth {
    quarantined: AtomicBool,
    uncertain: AtomicBool,
}

#[cfg(any(feature = "cuda", test))]
impl DeviceHealth {
    pub(crate) fn usable(&self) -> bool {
        !self.quarantined.load(Ordering::Acquire)
    }

    pub(crate) fn quarantine(&self, uncertain: bool) {
        // Set uncertainty first so a simultaneous destructor cannot observe the
        // quarantined state while missing the stronger retention requirement.
        if uncertain {
            self.uncertain.store(true, Ordering::Release);
        }
        self.quarantined.store(true, Ordering::Release);
    }

    pub(crate) fn uncertain(&self) -> bool {
        self.uncertain.load(Ordering::Acquire)
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
