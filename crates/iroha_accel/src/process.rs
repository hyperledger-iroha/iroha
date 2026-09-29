//! One process resource envelope shared by physical backend registries.
//!
//! These pools own acceleration attempt storage only. They do not recreate a
//! State execution budget or fund caller destinations. Metal integration must
//! charge unified backing once and account its applicable ceilings separately.

use std::{
    alloc::Layout,
    sync::{Arc, OnceLock},
};

use mv::allocation::AllocationBudget;
use parking_lot::Mutex;

use crate::{RegistryLimits, resources::ResourcePools, slots::Slots};

/// Process-lived acceleration capacity, shared across backend registries.
/// Physical registry construction remains backend-specific; allocation budgets
/// and opaque native owner counts are never replaced during configuration reload.
pub struct ProcessResources {
    pub(crate) metadata: AllocationBudget,
    pub(crate) modules: Arc<Slots>,
    pub(crate) streams: Arc<Slots>,
    pub(crate) resources: Arc<ResourcePools>,
}

/// Original process command and stream-cardinality credit.
/// No bytes are inferred from opaque driver bookkeeping. This move-only permit
/// belongs to the actual native command lifetime, including uncertain retention.
pub struct NativeCommandPermit {
    _work: crate::resources::InFlightPermit,
    _stream: crate::slots::Slot,
}

static OWNER: OnceLock<ProcessResources> = OnceLock::new();
static INSTALL: Mutex<()> = Mutex::new(());

/// Requested allocation and opaque-owner counts in the original process envelope.
/// Driver-private allocations and allocator/page overhead are not measured bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProcessUsage {
    /// Current and peak ordinary host backing bytes.
    pub host_bytes: [usize; 2],
    /// Current and peak pinned host backing bytes.
    pub pinned_bytes: [usize; 2],
    /// Current and peak requested device backing bytes.
    pub device_bytes: [usize; 2],
    /// Current and peak unified backing bytes, already included in both host
    /// and device ceiling usage. Count this physical allocation only once.
    pub unified_bytes: [usize; 2],
    /// Current and peak complete attempted-work owners.
    pub in_flight: [usize; 2],
    /// Current and peak variable control backing bytes.
    pub metadata_bytes: [usize; 2],
    /// Current and peak opaque native module owner counts.
    pub modules: [usize; 2],
    /// Current and peak opaque native stream owner counts.
    pub streams: [usize; 2],
}

impl ProcessResources {
    fn new(limits: RegistryLimits) -> Self {
        Self {
            metadata: AllocationBudget::new(limits.metadata_bytes),
            modules: Slots::new(limits.modules),
            streams: Slots::new(limits.streams),
            resources: ResourcePools::new(limits.work),
        }
    }

    /// Install the configured finite process envelope or update its original pools.
    /// This does not load a driver, discover devices, or create a State budget.
    pub fn install(limits: RegistryLimits) -> &'static Self {
        let _install = INSTALL.lock();
        let owner = OWNER.get_or_init(|| Self::new(limits));
        owner.configure(limits);
        owner
    }

    /// Initialize the finite process envelope only if no owner exists yet.
    /// A concurrent or previously applied file policy is never overwritten by
    /// this lazy startup path. No backend or driver is initialized.
    pub fn get_or_initialize(limits: RegistryLimits) -> &'static Self {
        let _install = INSTALL.lock();
        OWNER.get_or_init(|| Self::new(limits))
    }

    /// Reserve and initialize an escaping host result from the original process
    /// envelope, without needing a native driver or an in-flight device attempt.
    /// CPU fallback and foreign-runtime copying retain this same allocation owner.
    ///
    /// # Errors
    ///
    /// Returns [`HostOutputError::InvalidLayout`](crate::HostOutputError::InvalidLayout)
    /// when `len` values of `T` do not form a valid layout,
    /// [`HostOutputError::Capacity`](crate::HostOutputError::Capacity) when the
    /// original host pool cannot fund them, and
    /// [`HostOutputError::Allocation`](crate::HostOutputError::Allocation) when
    /// the allocator refuses the already reserved backing storage.
    pub fn try_host_output<T: Copy + Default>(
        &self,
        len: usize,
    ) -> Result<crate::HostOutput<T>, crate::HostOutputError> {
        let layout = Layout::array::<T>(len).map_err(|_| crate::HostOutputError::InvalidLayout)?;
        let mut reservation = self.resources.try_reserve_host(layout.size())?;
        crate::HostOutput::from_reservation(len, &mut reservation).map_err(|error| match error {
            mv::allocation::PrepaidBufferError::Reservation(_) => crate::HostOutputError::Capacity,
            mv::allocation::PrepaidBufferError::Allocation(_) => crate::HostOutputError::Allocation,
        })
    }

    /// Allocate one page-aligned backing region from this original process owner.
    /// The same physical allocation is admitted against both applicable host and
    /// device ceilings. Alignment and rounded capacity are checked before admission.
    ///
    /// # Errors
    ///
    /// Returns the [`UnifiedBufferError`](crate::UnifiedBufferError) from
    /// [`UnifiedBuffer`](crate::UnifiedBuffer) allocation: an invalid layout, an
    /// exhausted host or device ceiling, or an allocator refusal.
    pub fn try_unified_buffer(
        &self,
        len: usize,
        alignment: usize,
    ) -> Result<crate::UnifiedBuffer, crate::UnifiedBufferError> {
        crate::UnifiedBuffer::allocate(&self.resources, len, alignment)
    }

    /// Admit one opaque command/encoder aggregate before any native construction.
    /// Retain this permit until native completion and reclamation; uncertain work
    /// must retain the permit along with the native command and all its buffers.
    pub fn try_native_command(&self) -> Option<NativeCommandPermit> {
        let work = crate::resources::InFlightPermit::acquire(&self.resources).ok()?;
        let stream = self.streams.try_acquire()?;
        Some(NativeCommandPermit {
            _work: work,
            _stream: stream,
        })
    }

    /// Reserve exact consumer registry metadata from the original process envelope.
    /// The caller must attach this reservation to its actual allocation owner.
    pub fn try_consumer_metadata(
        &self,
        layout: Layout,
    ) -> Option<mv::allocation::AllocationReservation> {
        self.metadata.try_reserve(layout).ok()
    }

    /// Borrow existing process capacity without allocating or changing admission.
    pub fn get() -> Option<&'static Self> {
        OWNER.get()
    }

    /// Observe original charges without creating or initializing any backend.
    pub fn usage(&self) -> ProcessUsage {
        let work = self.resources.usage();
        ProcessUsage {
            host_bytes: [work.reserved.host_bytes, work.peak.host_bytes],
            pinned_bytes: [work.reserved.pinned_bytes, work.peak.pinned_bytes],
            device_bytes: [work.reserved.device_bytes, work.peak.device_bytes],
            in_flight: [work.in_flight, work.peak_in_flight],
            unified_bytes: [
                self.resources
                    .unified_bytes
                    .load(std::sync::atomic::Ordering::Acquire),
                self.resources
                    .peak_unified_bytes
                    .load(std::sync::atomic::Ordering::Acquire),
            ],
            metadata_bytes: [
                self.metadata.reserved_bytes(),
                self.metadata.peak_reserved_bytes(),
            ],
            modules: [self.modules.used(), self.modules.peak()],
            streams: [self.streams.used(), self.streams.peak()],
        }
    }

    pub(crate) fn configure(&self, limits: RegistryLimits) {
        self.metadata.set_limit_bytes(limits.metadata_bytes);
        self.modules.set_limit(limits.modules);
        self.streams.set_limit(limits.streams);
        self.resources.set_limits(limits.work);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GpuResourceLimits, resources::BufferRequest};

    #[test]
    fn consumer_metadata_uses_original_pool_and_survives_limit_shrink() {
        let mut limits = RegistryLimits::STANDARD;
        limits.metadata_bytes = 64;
        let owner = ProcessResources::new(limits);
        let credit = owner
            .try_consumer_metadata(Layout::array::<u64>(8).unwrap())
            .expect("exact metadata credit");
        assert_eq!(owner.usage().metadata_bytes[0], 64);
        limits.metadata_bytes = 0;
        owner.configure(limits);
        assert!(owner.try_consumer_metadata(Layout::new::<u8>()).is_none());
        assert_eq!(owner.usage().metadata_bytes[0], 64);
        drop(credit);
        assert_eq!(owner.usage().metadata_bytes[0], 0);
    }

    #[test]
    fn unified_backing_and_native_commands_share_the_existing_process_envelope() {
        let mut limits = RegistryLimits::STANDARD;
        limits.work.host_bytes = 8192;
        limits.work.device_bytes = 8192;
        limits.work.in_flight = 1;
        limits.streams = 1;
        let owner = ProcessResources::new(limits);
        let backing = owner.try_unified_buffer(23, 4096).unwrap();
        let command = owner.try_native_command().unwrap();
        assert_eq!(owner.usage().unified_bytes, [4096, 4096]);
        assert_eq!(owner.usage().host_bytes[0], 4096);
        assert_eq!(owner.usage().device_bytes[0], 4096);
        assert_eq!(owner.usage().in_flight, [1, 1]);
        assert_eq!(owner.usage().streams, [1, 1]);
        assert!(owner.try_native_command().is_none());
        limits.work.host_bytes = 0;
        limits.work.device_bytes = 0;
        limits.work.in_flight = 0;
        limits.streams = 0;
        owner.configure(limits);
        assert_eq!(owner.usage().unified_bytes[0], 4096);
        assert!(owner.try_native_command().is_none());
        drop(backing);
        assert_eq!(owner.usage().unified_bytes[0], 0);
        assert_eq!(owner.usage().in_flight[0], 1);
        drop(command);
        assert_eq!(owner.usage().in_flight[0], 0);
        assert_eq!(owner.usage().streams[0], 0);
        limits.work.in_flight = 1;
        owner.configure(limits);
        assert!(owner.try_native_command().is_none());
        assert_eq!(
            owner.usage().in_flight[0],
            0,
            "partial stream refusal returns in-flight credit"
        );
    }

    #[test]
    fn cpu_output_borrows_original_host_pool_and_survives_policy_shrink() {
        let mut limits = RegistryLimits::STANDARD;
        limits.work.host_bytes = 32;
        limits.work.in_flight = 0;
        let owner = ProcessResources::new(limits);
        let mut output = owner.try_host_output::<u64>(4).unwrap();
        output.copy_from_slice(&[11, 22, 33, 44]);
        assert_eq!(owner.usage().host_bytes, [32, 32]);
        assert_eq!(owner.usage().device_bytes[0], 0);
        assert_eq!(owner.usage().in_flight[0], 0);
        assert_eq!(
            owner.try_host_output::<u64>(1).unwrap_err(),
            crate::HostOutputError::Capacity
        );
        limits.work.host_bytes = 0;
        owner.configure(limits);
        assert_eq!(output.as_slice(), [11, 22, 33, 44]);
        assert_eq!(owner.usage().host_bytes[0], 32);
        drop(output);
        assert_eq!(owner.usage().host_bytes[0], 0);
        assert!(owner.try_host_output::<u64>(0).unwrap().is_empty());
        assert_eq!(
            owner.try_host_output::<u64>(usize::MAX).unwrap_err(),
            crate::HostOutputError::InvalidLayout
        );
    }

    #[test]
    fn process_reload_keeps_original_attempt_credit_and_owner_counts() {
        let mut limits = RegistryLimits::STANDARD;
        limits.work = GpuResourceLimits {
            host_bytes: 8,
            pinned_bytes: 16,
            device_bytes: 24,
            in_flight: 1,
        };
        let owner = ProcessResources::new(limits);
        let request = BufferRequest {
            host_bytes: 8,
            pinned_bytes: 16,
            device_bytes: 24,
        };
        let lease = owner.resources.try_reserve(request).unwrap();
        let stream = owner.streams.try_acquire().unwrap();
        let module = owner.modules.try_acquire().unwrap();
        limits.work = GpuResourceLimits {
            host_bytes: 0,
            pinned_bytes: 0,
            device_bytes: 0,
            in_flight: 0,
        };
        limits.streams = 0;
        limits.modules = 0;
        owner.configure(limits);
        assert_eq!(owner.resources.usage().reserved, request);
        assert_eq!(owner.usage().host_bytes, [8, 8]);
        assert_eq!(owner.usage().device_bytes, [24, 24]);
        assert_eq!(owner.resources.usage().in_flight, 1);
        assert!(owner.resources.try_reserve(request).is_err());
        assert!(owner.streams.try_acquire().is_none());
        assert!(owner.modules.try_acquire().is_none());
        assert_eq!(owner.streams.used(), 1);
        assert_eq!(owner.modules.used(), 1);
        drop((lease, stream, module));
        assert_eq!(owner.resources.usage().reserved, BufferRequest::default());
        assert_eq!(owner.resources.usage().in_flight, 0);
        assert_eq!(owner.streams.used(), 0);
        assert_eq!(owner.modules.used(), 0);
        assert_eq!(owner.usage().in_flight, [0, 1]);
    }
}
