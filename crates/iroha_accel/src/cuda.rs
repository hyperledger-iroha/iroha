//! One persistent CUDA registry with finite metadata and opaque-owner counts.
//!
//! Explicit allocations are prepaid. Driver-private context/module allocations
//! cannot be measured by this API and are bounded by owner counts, not described
//! as charged bytes. The process has one fixed bootstrap control allocation; all
//! variable registry backing and shared records use the metadata budget.

use cust::{CudaFlags, init, sys};
use iroha_allocation::{AllocationReservation, ChargedBuffer, ChargedShared};
use parking_lot::Mutex;
use std::{
    alloc::Layout,
    ffi::c_void,
    mem::ManuallyDrop,
    ptr,
    sync::OnceLock,
    time::{Duration, Instant},
};

use crate::RegistryLimits;
use crate::{
    artifact::PtxArtifact, identity::DeviceIdentity, resources::DeviceHealth, slots::Slot,
};

mod capabilities;
use capabilities::{Capabilities, DeviceCapacity};

mod completion;
mod work;
pub use work::{CudaBuffer, CudaWork, WorkRequest};

/// Explicit requested allocation and owner-count observations, excluding opaque
/// driver-internal memory and fixed process bootstrap control metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaUsage {
    /// Current and peak host payload bytes, including charged escaping outputs.
    pub host_bytes: [usize; 2],
    /// Current and peak requested pinned payload bytes.
    pub pinned_bytes: [usize; 2],
    /// Current and peak requested device payload bytes.
    pub device_bytes: [usize; 2],
    /// Current and peak prepared/submitted/uncertain complete work counts.
    pub in_flight: [usize; 2],
    /// Current and peak variable Rust metadata bytes.
    pub metadata_bytes: [usize; 2],
    /// Current and peak opaque native module owner counts.
    pub modules: [usize; 2],
    /// Current and peak opaque native nonblocking stream owner counts.
    pub streams: [usize; 2],
    /// Lifetime-observed physical records, including failed and quarantined records.
    pub observed_devices: usize,
}

/// Local operational refusal; the caller may recompute on a qualified fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CudaFailure {
    /// No driver or device is currently available.
    Unavailable,
    /// A complete finite request could not be funded before allocation.
    Capacity,
    /// Physical quarantine forbids new work and publication.
    Quarantined,
    /// A nonblocking owner lock is currently busy.
    Busy,
    /// A request is malformed, overflows, or does not belong to this work owner.
    InvalidRequest,
    /// Native Driver API failure, preserving its exact numeric result.
    Driver(i32),
    /// The exact stream did not complete within the supplied finite timeout.
    Timeout,
}

pub(super) fn checked(result: sys::CUresult) -> Result<(), CudaFailure> {
    if result == sys::CUresult::CUDA_SUCCESS {
        Ok(())
    } else {
        Err(CudaFailure::Driver(result as i32))
    }
}

struct Inventory {
    records: ChargedBuffer<ChargedShared<Record>>,
    last_attempt: Option<Instant>,
}

/// The process-owned physical registry. Consumers cannot construct another one.
///
/// Reloads alter only admission ceilings. UUID records, context health, original
/// pools and outstanding counts survive opt-out, cap shrink, and later opt-in.
/// Its original registry capacity cannot grow at reload; a larger request must
/// wait for a reviewed process restart instead of creating a second registry.
pub struct CudaProcess {
    inventory: Mutex<Inventory>,
    owner: &'static crate::ProcessResources,
    limits: Mutex<RegistryLimits>,
    capacity: usize,
}

static PROCESS: OnceLock<CudaProcess> = OnceLock::new();
static INSTALL: Mutex<()> = Mutex::new(());

impl CudaProcess {
    /// Install the one registry, or update admission on that same owner.
    /// Required finite inputs have no implicit unlimited or free-memory default.
    ///
    /// # Errors
    ///
    /// Returns [`CudaFailure::InvalidRequest`] when a reload would grow the
    /// original record capacity or the record layout is not representable, and
    /// [`CudaFailure::Capacity`] when the metadata budget cannot fund the fixed
    /// record backing.
    pub fn install(limits: RegistryLimits) -> Result<&'static Self, CudaFailure> {
        let _installation = INSTALL.lock();
        if let Some(process) = PROCESS.get() {
            process.reconfigure(limits)?;
            return Ok(process);
        }
        let owner = crate::ProcessResources::get()
            .unwrap_or_else(|| crate::ProcessResources::get_or_initialize(limits));
        let layout = Layout::array::<ChargedShared<Record>>(limits.devices)
            .map_err(|_| CudaFailure::InvalidRequest)?;
        let mut reservation = owner
            .metadata
            .try_reserve(layout)
            .map_err(|_| CudaFailure::Capacity)?;
        let records = ChargedBuffer::from_reservation(limits.devices, &mut reservation)
            .map_err(|_| CudaFailure::Capacity)?;
        let process = Self {
            inventory: Mutex::new(Inventory {
                records,
                last_attempt: None,
            }),
            owner,
            limits: Mutex::new(limits),
            capacity: limits.devices,
        };
        // Serialized installation owns this value until it is placed in the static.
        if PROCESS.set(process).is_err() {
            return Err(CudaFailure::InvalidRequest);
        }
        PROCESS.get().ok_or(CudaFailure::InvalidRequest)
    }

    /// Borrow the installed process without initializing a driver or another owner.
    pub fn get() -> Option<&'static Self> {
        PROCESS.get()
    }

    /// Apply ceilings to the original pools. No driver reset or cache replacement.
    ///
    /// # Errors
    ///
    /// Returns [`CudaFailure::InvalidRequest`] when `limits.devices` exceeds the
    /// original record capacity; the current limits are then left unchanged.
    pub fn reconfigure(&self, limits: RegistryLimits) -> Result<(), CudaFailure> {
        if limits.devices > self.capacity {
            return Err(CudaFailure::InvalidRequest);
        }
        let mut previous = self.limits.lock();
        self.owner.configure(limits);
        *previous = limits;
        Ok(())
    }

    /// Probe ordinals independently without allocating an unbounded device list.
    /// A transient absent driver is retried after thirty seconds; quarantined UUID
    /// records are never replaced by a new handle to the same primary context.
    ///
    /// # Errors
    ///
    /// Returns [`CudaFailure::Busy`] while another caller holds the inventory,
    /// [`CudaFailure::Unavailable`] when the driver cannot be initialized,
    /// [`CudaFailure::Driver`] when the device count or driver version query
    /// fails, and [`CudaFailure::InvalidRequest`] when the driver reports a
    /// negative device count. A failing ordinal is skipped instead.
    pub fn discover(&self) -> Result<usize, CudaFailure> {
        let mut inventory = self.inventory.try_lock().ok_or(CudaFailure::Busy)?;
        let now = Instant::now();
        if inventory
            .last_attempt
            .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_secs(30))
        {
            let limit = self.limits.lock().devices;
            return Ok(eligible_record_count(&inventory, limit));
        }
        inventory.last_attempt = Some(now);
        init(CudaFlags::empty()).map_err(|_| CudaFailure::Unavailable)?;
        let mut count = 0;
        let mut version = 0;
        // SAFETY: the runtime-loaded driver writes only these initialized scalars.
        unsafe {
            checked(sys::cuDeviceGetCount(&raw mut count))?;
            checked(sys::cuDriverGetVersion(&raw mut version))?;
        }
        let limits = *self.limits.lock();
        let count = u32::try_from(count).map_err(|_| CudaFailure::InvalidRequest)?;
        for ordinal in 0..count.min(limits.discovery_ordinals) {
            let ordinal = i32::try_from(ordinal).map_err(|_| CudaFailure::InvalidRequest)?;
            let Some((raw_device, identity)) = discover_identity(ordinal, version) else {
                continue;
            };
            if let Some(record) = inventory
                .records
                .as_slice()
                .iter()
                .find(|record| record.primary.identity.same_physical_device(identity))
            {
                if record.primary.identity != identity {
                    record.primary.quarantine(true);
                }
                continue;
            }
            if inventory.records.as_slice().len() >= limits.devices {
                break;
            }
            let Ok(capabilities) = Capabilities::discover(raw_device) else {
                continue;
            };
            // Metadata is admitted before retaining the primary context. Module
            // cache backing is fixed per record and admitted at the same boundary.
            let primary_layout = ChargedShared::<Primary>::allocation_layout();
            let record_layout = ChargedShared::<Record>::allocation_layout();
            let cache_layout = match Layout::array::<ChargedShared<ModuleOwner>>(limits.modules) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(bytes) = primary_layout
                .size()
                .checked_add(record_layout.size())
                .and_then(|n| n.checked_add(cache_layout.size()))
            else {
                continue;
            };
            let Ok(mut reserve) = self.owner.metadata.try_reserve_bytes(bytes) else {
                continue;
            };
            let Ok(cache) = ChargedBuffer::from_reservation(limits.modules, &mut reserve) else {
                continue;
            };
            let empty = Primary {
                identity,
                device: raw_device,
                capacity: DeviceCapacity::new(capabilities.total_bytes),
                capabilities,
                native: Mutex::new(ptr::null_mut()),
                health: DeviceHealth::default(),
                gate: Mutex::new(()),
            };
            let Ok(primary) = ChargedShared::from_reservation(empty, &mut reserve) else {
                continue;
            };
            // The guarded record is inserted before any native handle is retained.
            // Failed initialization leaves its UUID and quarantine in the registry.
            let Ok(record) = ChargedShared::from_reservation(
                Record {
                    primary,
                    modules: Mutex::new(cache),
                },
                &mut reserve,
            ) else {
                continue;
            };
            inventory.records.push_reserved(record.clone());
            record.primary.initialize();
        }
        Ok(eligible_record_count(&inventory, limits.devices))
    }

    /// Borrow an already discovered eligible record without waiting on other work.
    pub fn device(&self, index: usize) -> Option<CudaDevice<'_>> {
        let inventory = self.inventory.try_lock()?;
        let limits = self.limits.try_lock()?;
        if index >= limits.devices {
            return None;
        }
        let record = inventory.records.as_slice().get(index)?;
        record.primary.health.usable().then(|| CudaDevice {
            record: record.clone(),
            process: self,
            index,
        })
    }

    /// Original fixed registry backing capacity, preserved across reloads.
    pub fn record_capacity(&self) -> usize {
        self.capacity
    }

    /// Admit exact consumer-owned policy metadata from the shared original pool.
    /// The consumer must bind it to charged allocation owners before construction.
    /// No allocation, wait, driver operation, or additional pool is created here.
    ///
    /// # Errors
    ///
    /// Returns [`CudaFailure::Capacity`] when the metadata budget cannot fund
    /// `layout`.
    pub fn reserve_consumer_metadata(
        &self,
        layout: Layout,
    ) -> Result<AllocationReservation, CudaFailure> {
        self.owner
            .metadata
            .try_reserve(layout)
            .map_err(|_| CudaFailure::Capacity)
    }

    /// Currently eligible records under the current cap and persistent health.
    /// This query never constructs contexts or replaces the original owner.
    pub fn usable_device_count(&self) -> usize {
        let inventory = self.inventory.lock();
        let limit = self.limits.lock().devices;
        eligible_record_count(&inventory, limit)
    }

    /// Observe original allocation pools and cardinality owners without refunds.
    pub fn usage(&self) -> CudaUsage {
        let work = self.owner.resources.usage();
        CudaUsage {
            host_bytes: [work.reserved.host_bytes, work.peak.host_bytes],
            pinned_bytes: [work.reserved.pinned_bytes, work.peak.pinned_bytes],
            device_bytes: [work.reserved.device_bytes, work.peak.device_bytes],
            in_flight: [work.in_flight, work.peak_in_flight],
            metadata_bytes: [
                self.owner.metadata.reserved_bytes(),
                self.owner.metadata.peak_reserved_bytes(),
            ],
            modules: [self.owner.modules.used(), self.owner.modules.peak()],
            streams: [self.owner.streams.used(), self.owner.streams.peak()],
            observed_devices: self.record_count(),
        }
    }

    /// Number of lifetime-observed records, including failures and quarantine.
    pub fn record_count(&self) -> usize {
        self.inventory.lock().records.as_slice().len()
    }

    /// Nonblocking count snapshot for admission; contention selects local fallback.
    ///
    /// # Errors
    ///
    /// Returns [`CudaFailure::Busy`] while another caller holds the inventory.
    pub fn try_record_count(&self) -> Result<usize, CudaFailure> {
        Ok(self
            .inventory
            .try_lock()
            .ok_or(CudaFailure::Busy)?
            .records
            .as_slice()
            .len())
    }
}

fn eligible_record_count(inventory: &Inventory, limit: usize) -> usize {
    inventory
        .records
        .as_slice()
        .iter()
        .take(limit)
        .filter(|record| record.primary.health.usable())
        .count()
}

fn discover_identity(ordinal: i32, driver_version: i32) -> Option<(sys::CUdevice, DeviceIdentity)> {
    let mut device = 0;
    let mut uuid = sys::CUuuid::default();
    // SAFETY: initialized scalar/UUID out-parameters have the exact driver layout.
    unsafe {
        checked(sys::cuDeviceGet(&raw mut device, ordinal)).ok()?;
        checked(sys::cuDeviceGetUuid_v2(&raw mut uuid, device)).ok()?;
    }
    Some((
        device,
        DeviceIdentity {
            // `c_char` signedness differs by target; keep each UUID byte's exact bits.
            uuid: uuid.bytes.map(|byte| u8::from_ne_bytes(byte.to_ne_bytes())),
            driver_version,
        },
    ))
}

struct Record {
    primary: ChargedShared<Primary>,
    modules: Mutex<ChargedBuffer<ChargedShared<ModuleOwner>>>,
}

pub(super) struct Primary {
    identity: DeviceIdentity,
    device: sys::CUdevice,
    capabilities: Capabilities,
    capacity: DeviceCapacity,
    native: Mutex<sys::CUcontext>,
    health: DeviceHealth,
    gate: Mutex<()>,
}
// SAFETY: handles are opaque driver objects. All use binds the origin context;
// native creation/release and publication are serialized by this owner's gate.
unsafe impl Send for Primary {}
unsafe impl Sync for Primary {}

impl Primary {
    fn initialize(&self) {
        let _gate = self.gate.lock();
        let mut context = self.native.lock();
        // SAFETY: installed guarded primary owner retains this exact handle even
        // on an error which writes an output; no implicit lossy Context drop runs.
        let result = unsafe { sys::cuDevicePrimaryCtxRetain(&raw mut *context, self.device) };
        if checked(result).is_err() || context.is_null() {
            self.health.quarantine(true);
        }
        // No mutable primary-context flags are set: another library may already
        // own the primary context. Nonblocking stream semantics are explicit below.
    }

    pub(super) fn quarantine(&self, uncertain: bool) {
        let _gate = self.gate.lock();
        self.health.quarantine(uncertain);
    }

    pub(super) fn bound<T>(
        &self,
        body: impl FnOnce() -> Result<T, CudaFailure>,
    ) -> Result<T, CudaFailure> {
        struct Driver;
        impl crate::context_binding::ContextDriver for Driver {
            type Handle = sys::CUcontext;
            type Error = CudaFailure;
            fn current(&self) -> Result<sys::CUcontext, CudaFailure> {
                let mut previous = ptr::null_mut();
                // SAFETY: initialized exact-layout synchronous out-parameter.
                unsafe {
                    checked(sys::cuCtxGetCurrent(&raw mut previous))?;
                }
                Ok(previous)
            }
            fn set_current(&self, handle: sys::CUcontext) -> Result<(), CudaFailure> {
                // SAFETY: handle is this guarded origin or a captured thread context.
                unsafe { checked(sys::cuCtxSetCurrent(handle)) }
            }
        }
        let target = *self.native.lock();
        if target.is_null() {
            return Err(CudaFailure::Quarantined);
        }
        crate::context_binding::with_context(&Driver, target, || self.health.quarantine(true), body)
    }
}

impl Drop for Primary {
    fn drop(&mut self) {
        // In production registry records are process-lived. This checked path
        // covers constructor rollback/test owners and never resets a primary.
        if self.native.get_mut().is_null() || self.health.uncertain() {
            return;
        }
        let released = unsafe { sys::cuDevicePrimaryCtxRelease_v2(self.device) };
        if checked(released).is_err() {
            self.health.quarantine(true);
        }
    }
}

/// A borrowed view of the same process-owned physical device.
#[derive(Clone)]
pub struct CudaDevice<'a> {
    record: ChargedShared<Record>,
    process: &'a CudaProcess,
    index: usize,
}

impl CudaDevice<'_> {
    /// Stable driver identity used by consumer qualification records.
    pub fn identity(&self) -> DeviceIdentity {
        self.record.primary.identity
    }
    /// Whether physical quarantine currently permits an attempt.
    pub fn usable(&self) -> bool {
        self.record.primary.health.usable()
    }
    /// Quarantine this physical identity without replacing its primary context.
    pub fn quarantine(&self, uncertain: bool) {
        self.record.primary.quarantine(uncertain);
    }

    fn module(&self, artifact: PtxArtifact) -> Result<ChargedShared<ModuleOwner>, CudaFailure> {
        let limits = *self.process.limits.try_lock().ok_or(CudaFailure::Busy)?;
        if artifact.bytes().to_bytes_with_nul().len() > limits.artifact_bytes {
            return Err(CudaFailure::InvalidRequest);
        }
        let mut cache = self.record.modules.try_lock().ok_or(CudaFailure::Busy)?;
        if let Some(module) = cache
            .as_slice()
            .iter()
            .find(|module| module.artifact == artifact)
        {
            return Ok(module.clone());
        }
        if cache.as_slice().len() == cache.capacity() {
            return Err(CudaFailure::Capacity);
        }
        let slot = self
            .process
            .owner
            .modules
            .try_acquire()
            .ok_or(CudaFailure::Capacity)?;
        let mut reservation = self
            .process
            .owner
            .metadata
            .try_reserve(ChargedShared::<ModuleOwner>::allocation_layout())
            .map_err(|_| CudaFailure::Capacity)?;
        let owner = ModuleOwner {
            artifact,
            handle: Mutex::new(ptr::null_mut()),
            primary: ManuallyDrop::new(self.record.primary.clone()),
            slot: ManuallyDrop::new(slot),
        };
        let module = ChargedShared::from_reservation(owner, &mut reservation)
            .map_err(|_| CudaFailure::Capacity)?;
        let _gate = self
            .record
            .primary
            .gate
            .try_lock()
            .ok_or(CudaFailure::Busy)?;
        if !self.usable() {
            return Err(CudaFailure::Quarantined);
        }
        self.record.primary.bound(|| {
            let mut handle = module.handle.lock();
            // SAFETY: immutable, admitted, NUL-terminated PTX remains alive for
            // the complete driver load; successful handle custody is installed.
            checked(unsafe {
                sys::cuModuleLoadData(&raw mut *handle, artifact.bytes().as_ptr().cast::<c_void>())
            })
        })?;
        cache.push_reserved(module.clone());
        Ok(module)
    }
}

pub(super) struct ModuleOwner {
    artifact: PtxArtifact,
    handle: Mutex<sys::CUmodule>,
    primary: ManuallyDrop<ChargedShared<Primary>>,
    slot: ManuallyDrop<Slot>,
}
unsafe impl Send for ModuleOwner {}
unsafe impl Sync for ModuleOwner {}

impl Drop for ModuleOwner {
    fn drop(&mut self) {
        let handle = *self.handle.get_mut();
        if handle.is_null() {
            // Pre-native admission rollback cannot wait behind another attempt.
            // SAFETY: no module was created and no native child can refer to it.
            unsafe {
                ManuallyDrop::drop(&mut self.slot);
                ManuallyDrop::drop(&mut self.primary);
            }
            return;
        }
        let gate = self.primary.gate.lock();
        if self.primary.health.uncertain() {
            return;
        }
        if !handle.is_null()
            && self
                .primary
                .bound(|| checked(unsafe { sys::cuModuleUnload(handle) }))
                .is_err()
        {
            self.primary.health.quarantine(true);
            return;
        }
        *self.handle.get_mut() = ptr::null_mut();
        drop(gate);
        // SAFETY: native child is gone before its parent and original count refund.
        unsafe {
            ManuallyDrop::drop(&mut self.slot);
            ManuallyDrop::drop(&mut self.primary);
        }
    }
}

#[cfg(test)]
#[path = "cuda/limits_tests.rs"]
mod limits_tests;
