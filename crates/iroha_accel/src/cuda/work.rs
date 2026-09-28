//! Typed stable CUDA buffers and allocation-free uncertain-work retention.
//!
//! Driver pointers target independently stable native or pinned allocations.
//! No asynchronous command ever points at the movable Rust ownership records.
//! Every child retains the original work owner, so forgotten or uncertain children
//! also retain its in-flight count, stream, module, context, and original pools.

use cust::{
    memory::{DeviceCopy, DevicePointer},
    sys,
};
use mv::allocation::{AllocationCharge, ChargedBuffer, ChargedShared};
use parking_lot::Mutex;
use std::{
    alloc::Layout,
    ffi::{CStr, c_void},
    marker::PhantomData,
    mem::{ManuallyDrop, MaybeUninit, size_of},
    ptr,
    time::{Duration, Instant},
};

use super::{CudaDevice, CudaFailure, ModuleOwner, Primary, checked};
use crate::{
    artifact::PtxArtifact,
    custody::Phase,
    output::HostOutput,
    resources::{BufferRequest, ResourceReservation},
    slots::Slot,
};

/// Complete explicit demand for one operation, reserved before native allocation.
///
/// Include every temporary pinned/device allocation and every escaping host output.
/// The owner adds its exact shared Rust control layout to `host_bytes` itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkRequest {
    /// Output and ordinary host backing requested by this operation.
    pub host_bytes: usize,
    /// All pinned-host backing requested by this operation.
    pub pinned_bytes: usize,
    /// All device backing requested by this operation.
    pub device_bytes: usize,
}

struct WorkState {
    stream: sys::CUstream,
    phase: Phase,
    reservation: ManuallyDrop<ResourceReservation>,
}

struct WorkInner {
    state: Mutex<WorkState>,
    primary: ManuallyDrop<ChargedShared<Primary>>,
    modules: ManuallyDrop<ChargedBuffer<ChargedShared<ModuleOwner>>>,
    stream_slot: ManuallyDrop<Slot>,
    device_permit: ManuallyDrop<DevicePermit>,
}

struct DevicePermit {
    primary: ChargedShared<Primary>,
    bytes: usize,
}

impl DevicePermit {
    fn acquire(primary: &ChargedShared<Primary>, bytes: usize) -> Result<Self, CudaFailure> {
        if !primary.capacity.try_acquire(bytes) {
            return Err(CudaFailure::Capacity);
        }
        Ok(Self {
            primary: primary.clone(),
            bytes,
        })
    }
}

impl Drop for DevicePermit {
    fn drop(&mut self) {
        self.primary.capacity.release(self.bytes);
    }
}
// SAFETY: native access binds the origin context and is serialized by the primary
// publication/enqueue gate followed by the work mutex. Buffers are stable native
// allocations; handles never authorize unsynchronized access through this API.
unsafe impl Send for WorkInner {}
unsafe impl Sync for WorkInner {}

/// One admitted stream/module aggregate. Construction never waits for capacity.
///
/// Dropping pending work quarantines its physical context and retains every native
/// parent and original reservation. An ordinary host output escapes only after
/// exact-stream completion and publication serialized against quarantine.
pub struct CudaWork {
    inner: ChargedShared<WorkInner>,
}

fn initialize_pinned<T: Copy>(
    destination: &mut [MaybeUninit<T>],
    mut generate: impl FnMut(usize) -> T,
) {
    for (index, element) in destination.iter_mut().enumerate() {
        element.write(generate(index));
    }
}

impl CudaDevice<'_> {
    /// Admit the complete request and construct one nonblocking stream.
    pub fn prepare(
        &self,
        artifacts: &[PtxArtifact],
        request: WorkRequest,
    ) -> Result<CudaWork, CudaFailure> {
        if artifacts.is_empty() {
            return Err(CudaFailure::InvalidRequest);
        }
        // Keep eligibility stable through complete resource admission. A retained
        // device view cannot create fresh work after its index is capped away.
        let eligibility = self.process.limits.try_lock().ok_or(CudaFailure::Busy)?;
        if self.index >= eligibility.devices {
            return Err(CudaFailure::Capacity);
        }
        if !self.usable() {
            return Err(CudaFailure::Quarantined);
        }
        let device_permit = DevicePermit::acquire(&self.record.primary, request.device_bytes)?;
        let module_backing = Layout::array::<ChargedShared<ModuleOwner>>(artifacts.len())
            .map_err(|_| CudaFailure::InvalidRequest)?;
        let host_bytes = request
            .host_bytes
            .checked_add(ChargedShared::<WorkInner>::allocation_layout().size())
            .and_then(|bytes| bytes.checked_add(module_backing.size()))
            .ok_or(CudaFailure::InvalidRequest)?;
        let mut reservation = self
            .process
            .owner
            .resources
            .try_reserve(BufferRequest {
                host_bytes,
                pinned_bytes: request.pinned_bytes,
                device_bytes: request.device_bytes,
            })
            .map_err(|_| CudaFailure::Capacity)?;
        let stream_slot = self
            .process
            .owner
            .streams
            .try_acquire()
            .ok_or(CudaFailure::Capacity)?;
        drop(eligibility);
        // Partition control custody before moving the remaining complete request
        // into the guarded native aggregate. No second pool acquisition occurs.
        let mut control = reservation
            .host
            .try_partition_bytes(ChargedShared::<WorkInner>::allocation_layout().size())
            .map_err(|_| CudaFailure::Capacity)?;
        let mut modules = ChargedBuffer::from_reservation(artifacts.len(), &mut reservation.host)
            .map_err(|_| CudaFailure::Capacity)?;
        for &artifact in artifacts {
            modules.push_reserved(self.module(artifact)?);
        }
        let empty = WorkInner {
            state: Mutex::new(WorkState {
                stream: ptr::null_mut(),
                phase: Phase::Prepared,
                reservation: ManuallyDrop::new(reservation),
            }),
            primary: ManuallyDrop::new(self.record.primary.clone()),
            modules: ManuallyDrop::new(modules),
            stream_slot: ManuallyDrop::new(stream_slot),
            device_permit: ManuallyDrop::new(device_permit),
        };
        let inner = ChargedShared::from_reservation(empty, &mut control)
            .map_err(|_| CudaFailure::Capacity)?;
        {
            let _gate = inner.primary.gate.try_lock().ok_or(CudaFailure::Busy)?;
            if !inner.primary.health.usable() {
                return Err(CudaFailure::Quarantined);
            }
            inner.primary.bound(|| {
                let mut state = inner.state.lock();
                // SAFETY: installed guard owns the out-parameter, including a
                // non-null handle returned with an error. No commands are queued.
                checked(unsafe {
                    sys::cuStreamCreate(
                        &mut state.stream,
                        sys::CUstream_flags::CU_STREAM_NON_BLOCKING as u32,
                    )
                })
            })?;
        }
        Ok(CudaWork { inner })
    }
}

impl CudaWork {
    /// Allocate a typed native/pinned pair from this original complete request.
    /// Both allocations are synchronous; ownership is installed before the next
    /// fallible call. Inputs are copied separately before asynchronous enqueue.
    pub fn buffer<T: DeviceCopy + Copy>(
        &self,
        len: usize,
    ) -> Result<CudaBuffer<'_, T>, CudaFailure> {
        if size_of::<T>() == 0 {
            return Err(CudaFailure::InvalidRequest);
        }
        let layout = Layout::array::<T>(len).map_err(|_| CudaFailure::InvalidRequest)?;
        let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
        if state.phase == Phase::Pending {
            return Err(CudaFailure::Busy);
        }
        let device_charge = state
            .reservation
            .device
            .try_split(layout)
            .map_err(|_| CudaFailure::Capacity)?;
        let pinned_charge = state
            .reservation
            .pinned
            .try_split(layout)
            .map_err(|_| CudaFailure::Capacity)?;
        drop(state);
        let mut buffer = CudaBuffer {
            device: 0,
            pinned: ptr::null_mut(),
            len,
            parent: ManuallyDrop::new(self.inner.clone()),
            device_charge: ManuallyDrop::new(device_charge),
            pinned_charge: ManuallyDrop::new(pinned_charge),
            lifetime: PhantomData,
        };
        {
            let _gate = self
                .inner
                .primary
                .gate
                .try_lock()
                .ok_or(CudaFailure::Busy)?;
            if !self.inner.primary.health.usable() {
                return Err(CudaFailure::Quarantined);
            }
            self.inner.primary.bound(|| {
                if layout.size() != 0 {
                    checked(unsafe { sys::cuMemAlloc_v2(&mut buffer.device, layout.size()) })?;
                    let mut pinned = ptr::null_mut();
                    let allocated = unsafe { sys::cuMemHostAlloc(&mut pinned, layout.size(), 0) };
                    // Install custody before inspecting the driver's return code.
                    buffer.pinned = pinned.cast::<T>();
                    checked(allocated)?;
                    if buffer.pinned.is_null() || (buffer.pinned as usize) % layout.align() != 0 {
                        return Err(CudaFailure::InvalidRequest);
                    }
                }
                Ok(())
            })?;
        }
        Ok(buffer)
    }

    /// Copy original caller input into stable pinned custody, then enqueue upload.
    /// Caller input is never mutated or referenced by the asynchronous command.
    pub fn upload<T: DeviceCopy + Copy>(
        &self,
        buffer: &mut CudaBuffer<'_, T>,
        input: &[T],
    ) -> Result<(), CudaFailure> {
        if input.len() != buffer.len {
            return Err(CudaFailure::InvalidRequest);
        }
        self.upload_generated(buffer, |index| input[index])
    }

    /// Initialize reserved pinned storage by index, then enqueue its upload.
    /// This avoids an intermediate host allocation for canonical packing or
    /// scalar conversion. The generator runs synchronously; a panic enqueues no
    /// native work, and neither caller storage nor a movable temporary is a DMA target.
    pub fn upload_generated<T: DeviceCopy + Copy>(
        &self,
        buffer: &mut CudaBuffer<'_, T>,
        generate: impl FnMut(usize) -> T,
    ) -> Result<(), CudaFailure> {
        self.same_owner(buffer)?;
        let _gate = self
            .inner
            .primary
            .gate
            .try_lock()
            .ok_or(CudaFailure::Busy)?;
        self.ensure_usable()?;
        let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
        if state.phase == Phase::Pending {
            return Err(CudaFailure::Busy);
        }
        if buffer.is_empty() {
            return Ok(());
        }
        // SAFETY: the work exclusively owns `len` aligned pinned elements and no
        // command is pending. MaybeUninit permits partially initialized backing
        // if the synchronous generator unwinds before the enqueue below.
        let pinned = unsafe {
            std::slice::from_raw_parts_mut(buffer.pinned.cast::<MaybeUninit<T>>(), buffer.len)
        };
        initialize_pinned(pinned, generate);
        if !state.phase.begin(self.inner.primary.health.usable()) {
            return Err(CudaFailure::Quarantined);
        }
        let result = self.inner.primary.bound(|| {
            checked(unsafe {
                sys::cuMemcpyHtoDAsync_v2(
                    buffer.device,
                    buffer.pinned.cast::<c_void>(),
                    buffer.len * size_of::<T>(),
                    state.stream,
                )
            })
        });
        if result.is_err() {
            self.inner.primary.health.quarantine(true);
        }
        result
    }

    /// Launch an admitted consumer kernel on this exact stream and module.
    ///
    /// # Safety
    /// Arguments must exactly match the admitted symbol's ABI and checked public
    /// geometry. Every device pointer must refer to a live buffer from this work;
    /// kernel writes must stay within those allocations. Scalar argument pointers
    /// are copied synchronously by the driver, and may not themselves become DMA
    /// targets. The consumer retains arithmetic/geometry and artifact qualification.
    pub unsafe fn launch(
        &self,
        artifact: PtxArtifact,
        symbol: &CStr,
        grid: [u32; 3],
        block: [u32; 3],
        shared_bytes: u32,
        arguments: &mut [*mut c_void],
    ) -> Result<(), CudaFailure> {
        if arguments.len() > 32
            || !self
                .inner
                .primary
                .capabilities
                .permits_launch(grid, block, shared_bytes)
        {
            return Err(CudaFailure::InvalidRequest);
        }
        let _gate = self
            .inner
            .primary
            .gate
            .try_lock()
            .ok_or(CudaFailure::Busy)?;
        self.ensure_usable()?;
        let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
        let module = self
            .inner
            .modules
            .as_slice()
            .iter()
            .find(|module| module.artifact == artifact)
            .ok_or(CudaFailure::InvalidRequest)?;
        let module = *module.handle.lock();
        self.inner.primary.bound(|| {
            let mut function = ptr::null_mut();
            checked(unsafe { sys::cuModuleGetFunction(&mut function, module, symbol.as_ptr()) })?;
            // Mark before even a failed enqueue: a driver error is not a witness
            // that every pointer is no longer in use by this exact stream.
            if !state.phase.begin(self.inner.primary.health.usable()) {
                return Err(CudaFailure::Quarantined);
            }
            let result = checked(unsafe {
                sys::cuLaunchKernel(
                    function,
                    grid[0],
                    grid[1],
                    grid[2],
                    block[0],
                    block[1],
                    block[2],
                    shared_bytes,
                    state.stream,
                    arguments.as_mut_ptr(),
                    ptr::null_mut(),
                )
            });
            if result.is_err() {
                self.inner.primary.health.quarantine(true);
            }
            result
        })
    }

    /// Observe successful completion of all preceding commands on this stream.
    /// A timeout or driver failure permanently quarantines this physical owner.
    pub fn wait(&self) -> Result<(), CudaFailure> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            {
                let _gate = self
                    .inner
                    .primary
                    .gate
                    .try_lock()
                    .ok_or(CudaFailure::Busy)?;
                self.ensure_usable()?;
                let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
                if state.phase != Phase::Pending {
                    return Ok(());
                }
                let queried = self
                    .inner
                    .primary
                    .bound(|| Ok(unsafe { sys::cuStreamQuery(state.stream) }));
                if super::completion::observe(
                    queried,
                    &mut state.phase,
                    &self.inner.primary.health,
                    Instant::now() >= deadline,
                )? {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// Download into stable private staging, then publish charged initialized host
    /// output only after exact-stream completion and a final physical health check.
    ///
    /// # Safety
    /// Every downloaded element must have been initialized to a valid T by a
    /// completed upload or admitted kernel. Uninitialized device output and invalid
    /// Rust bit patterns are forbidden even if the stream completed successfully.
    pub unsafe fn download<T: DeviceCopy + Copy + Default>(
        &self,
        buffer: &mut CudaBuffer<'_, T>,
    ) -> Result<HostOutput<T>, CudaFailure> {
        // SAFETY: the caller promises initialization of every buffer element.
        unsafe { self.download_prefix(buffer, buffer.len) }
    }

    /// Download only the initialized prefix of a larger device buffer, retaining
    /// the exact output charge until the caller publishes or drops it.
    ///
    /// # Safety
    /// Every element in `0..initialized_len` must contain a valid initialized T
    /// from an admitted kernel or completed upload. The remaining device bytes
    /// are never read, copied, or exposed as Rust values.
    pub unsafe fn download_prefix<T: DeviceCopy + Copy + Default>(
        &self,
        buffer: &mut CudaBuffer<'_, T>,
        initialized_len: usize,
    ) -> Result<HostOutput<T>, CudaFailure> {
        if initialized_len > buffer.len {
            return Err(CudaFailure::InvalidRequest);
        }
        self.same_owner(buffer)?;
        self.wait()?;
        let mut output = {
            let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
            HostOutput::from_reservation(initialized_len, &mut state.reservation.host)
                .map_err(|_| CudaFailure::Capacity)?
        };
        {
            let _gate = self
                .inner
                .primary
                .gate
                .try_lock()
                .ok_or(CudaFailure::Busy)?;
            self.ensure_usable()?;
            let mut state = self.inner.state.try_lock().ok_or(CudaFailure::Busy)?;
            if initialized_len != 0 {
                if !state.phase.begin(self.inner.primary.health.usable()) {
                    return Err(CudaFailure::Quarantined);
                }
                let result = self.inner.primary.bound(|| {
                    checked(unsafe {
                        sys::cuMemcpyDtoHAsync_v2(
                            buffer.pinned.cast::<c_void>(),
                            buffer.device,
                            initialized_len * size_of::<T>(),
                            state.stream,
                        )
                    })
                });
                if result.is_err() {
                    self.inner.primary.health.quarantine(true);
                }
                result?;
            }
        }
        self.wait()?;
        let _publication = self
            .inner
            .primary
            .gate
            .try_lock()
            .ok_or(CudaFailure::Busy)?;
        self.ensure_usable()?;
        if !self
            .inner
            .state
            .lock()
            .phase
            .may_publish(self.inner.primary.health.usable())
        {
            return Err(CudaFailure::Busy);
        }
        if initialized_len != 0 {
            // SAFETY: successful exact-stream completion precedes this typed read.
            // T's bit validity after the kernel is part of unsafe launch's contract.
            unsafe {
                ptr::copy_nonoverlapping(
                    buffer.pinned,
                    output.as_mut_slice().as_mut_ptr(),
                    initialized_len,
                );
            }
        }
        Ok(output)
    }

    fn ensure_usable(&self) -> Result<(), CudaFailure> {
        if self.inner.primary.health.usable() {
            Ok(())
        } else {
            Err(CudaFailure::Quarantined)
        }
    }
    fn same_owner<T: DeviceCopy + Copy>(
        &self,
        buffer: &CudaBuffer<'_, T>,
    ) -> Result<(), CudaFailure> {
        if ptr::eq::<WorkInner>(&*self.inner, &**buffer.parent) {
            Ok(())
        } else {
            Err(CudaFailure::InvalidRequest)
        }
    }
}

impl Drop for CudaWork {
    fn drop(&mut self) {
        // Safe Rust cannot enqueue through this work after its exclusive drop
        // begins. An unsubmitted or completed owner has no pending command to
        // quarantine; avoid acquiring the native gate during ordinary rollback.
        if self.inner.state.lock().phase != Phase::Pending {
            return;
        }
        let _gate = self.inner.primary.gate.lock();
        if self.inner.state.lock().phase == Phase::Pending {
            self.inner.primary.health.quarantine(true);
        }
    }
}

/// Stable typed device/pinned storage, bound to one admitted work owner.
/// No safe API detaches either native allocation or its original byte charge.
pub struct CudaBuffer<'work, T: DeviceCopy + Copy> {
    device: sys::CUdeviceptr,
    pinned: *mut T,
    len: usize,
    parent: ManuallyDrop<ChargedShared<WorkInner>>,
    device_charge: ManuallyDrop<AllocationCharge>,
    pinned_charge: ManuallyDrop<AllocationCharge>,
    lifetime: PhantomData<&'work CudaWork>,
}

impl<T: DeviceCopy + Copy> CudaBuffer<'_, T> {
    /// Initialized geometry count; this does not expose host or device contents.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Whether no device/pinned storage was allocated for this buffer.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Pointer for the admitted kernel ABI. Dereferencing/enqueue remains unsafe.
    pub fn device_pointer(&self) -> DevicePointer<T> {
        DevicePointer::from_raw(self.device)
    }
}

impl<T: DeviceCopy + Copy> Drop for CudaBuffer<'_, T> {
    fn drop(&mut self) {
        if self.device == 0 && self.pinned.is_null() {
            // Busy admission happened before native allocation. No physical gate
            // or driver call is needed to return this original Rust-only custody.
            unsafe {
                ManuallyDrop::drop(&mut self.device_charge);
                ManuallyDrop::drop(&mut self.pinned_charge);
                ManuallyDrop::drop(&mut self.parent);
            }
            return;
        }
        let _gate = self.parent.primary.gate.lock();
        if self.parent.state.lock().phase == Phase::Pending {
            self.parent.primary.health.quarantine(true);
        }
        if self.parent.primary.health.uncertain() {
            return;
        }
        let result = self.parent.primary.bound(|| {
            if self.device != 0 {
                checked(unsafe { sys::cuMemFree_v2(self.device) })?;
                self.device = 0;
            }
            if !self.pinned.is_null() {
                checked(unsafe { sys::cuMemFreeHost(self.pinned.cast::<c_void>()) })?;
                self.pinned = ptr::null_mut();
            }
            Ok(())
        });
        if result.is_err() {
            self.parent.primary.health.quarantine(true);
            return;
        }
        drop(_gate);
        // SAFETY: checked physical cleanup precedes byte refunds and parent release.
        unsafe {
            ManuallyDrop::drop(&mut self.device_charge);
            ManuallyDrop::drop(&mut self.pinned_charge);
            ManuallyDrop::drop(&mut self.parent);
        }
    }
}

impl Drop for WorkInner {
    fn drop(&mut self) {
        let state = self.state.get_mut();
        if state.stream.is_null() && state.phase != Phase::Pending {
            // No stream could enqueue work. Module shares remain owned by their
            // process cache, so admission rollback does not wait on a native gate.
            unsafe {
                ManuallyDrop::drop(&mut self.modules);
                ManuallyDrop::drop(&mut self.stream_slot);
                ManuallyDrop::drop(&mut self.device_permit);
                ManuallyDrop::drop(&mut self.primary);
                ManuallyDrop::drop(&mut state.reservation);
            }
            return;
        }
        let _gate = self.primary.gate.lock();
        let state = self.state.get_mut();
        if state.phase == Phase::Pending {
            self.primary.health.quarantine(true);
        }
        if self.primary.health.uncertain() {
            return;
        }
        if !state.stream.is_null()
            && self
                .primary
                .bound(|| checked(unsafe { sys::cuStreamDestroy_v2(state.stream) }))
                .is_err()
        {
            self.primary.health.quarantine(true);
            return;
        }
        state.stream = ptr::null_mut();
        drop(_gate);
        // SAFETY: stream is gone and every typed child has released its parent share.
        // Native/module/context parents are released before original work refunds.
        unsafe {
            ManuallyDrop::drop(&mut self.modules);
            ManuallyDrop::drop(&mut self.stream_slot);
            ManuallyDrop::drop(&mut self.device_permit);
            ManuallyDrop::drop(&mut self.primary);
            ManuallyDrop::drop(&mut state.reservation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_pinned_input_initializes_exact_order_without_intermediate_storage() {
        let mut slots = [MaybeUninit::uninit(); 5];
        let mut visited = 0;
        initialize_pinned(&mut slots, |index| {
            assert_eq!(index, visited);
            visited += 1;
            (index as u64).wrapping_mul(u64::MAX)
        });
        assert_eq!(visited, 5);
        // SAFETY: every element was initialized by the completed generator above.
        let actual = slots.map(|slot| unsafe { slot.assume_init() });
        assert_eq!(
            actual,
            [0, u64::MAX, u64::MAX - 1, u64::MAX - 2, u64::MAX - 3]
        );
        initialize_pinned::<u64>(&mut [], |_| panic!("empty upload must not generate input"));
    }

    #[test]
    fn generated_input_unwind_stops_before_uninitialized_values_can_publish() {
        let mut slots = [MaybeUninit::<u64>::uninit(); 4];
        let mut visited = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize_pinned(&mut slots, |index| {
                visited += 1;
                assert!(index != 2, "injected generator failure");
                index as u64
            });
        }));
        assert!(result.is_err());
        assert_eq!(visited, 3);
        // Only the initialized prefix is readable after a generator unwinds.
        assert_eq!(unsafe { slots[0].assume_init() }, 0);
        assert_eq!(unsafe { slots[1].assume_init() }, 1);
    }

    #[test]
    fn uncreated_work_and_buffer_rollback_never_take_a_busy_native_gate() {
        let process = super::super::limits_tests::test_registry();
        let device = process.device(0).unwrap();
        let primary = device.record.primary.clone();
        let mut reserve = process.owner.metadata.try_reserve_bytes(4096).unwrap();
        let modules = ChargedBuffer::from_reservation(0, &mut reserve).unwrap();
        let request = BufferRequest {
            host_bytes: 0,
            pinned_bytes: 8,
            device_bytes: 8,
        };
        let mut resources = process.owner.resources.try_reserve(request).unwrap();
        let charge_layout = Layout::array::<u64>(1).unwrap();
        let device_charge = resources.device.try_split(charge_layout).unwrap();
        let pinned_charge = resources.pinned.try_split(charge_layout).unwrap();
        let inner = ChargedShared::from_reservation(
            WorkInner {
                state: Mutex::new(WorkState {
                    stream: ptr::null_mut(),
                    phase: Phase::Prepared,
                    reservation: ManuallyDrop::new(resources),
                }),
                primary: ManuallyDrop::new(primary.clone()),
                modules: ManuallyDrop::new(modules),
                stream_slot: ManuallyDrop::new(process.owner.streams.try_acquire().unwrap()),
                device_permit: ManuallyDrop::new(DevicePermit::acquire(&primary, 8).unwrap()),
            },
            &mut reserve,
        )
        .unwrap_or_else(|_| panic!("test work reservation"));
        let mut buffer = CudaBuffer::<u64> {
            device: 0,
            pinned: ptr::null_mut(),
            len: 1,
            parent: ManuallyDrop::new(inner.clone()),
            device_charge: ManuallyDrop::new(device_charge),
            pinned_charge: ManuallyDrop::new(pinned_charge),
            lifetime: PhantomData,
        };
        let _busy = primary.gate.lock();
        let work = CudaWork {
            inner: inner.clone(),
        };
        // Rejected bounds must return before native access, even with null
        // unallocated pointers and a deliberately busy physical gate.
        assert!(matches!(
            unsafe { work.download_prefix(&mut buffer, 2) },
            Err(CudaFailure::InvalidRequest)
        ));
        assert!(matches!(
            work.upload(&mut buffer, &[1, 2]),
            Err(CudaFailure::InvalidRequest)
        ));
        drop(work);
        drop(buffer);
        assert_eq!(process.owner.usage().device_bytes[0], 0);
        assert_eq!(process.owner.usage().pinned_bytes[0], 0);
        drop(inner);
        assert_eq!(process.owner.usage().streams[0], 0);
        assert_eq!(process.owner.usage().in_flight[0], 0);
        assert!(primary.capacity.try_acquire(1024));
        primary.capacity.release(1024);
    }
}
