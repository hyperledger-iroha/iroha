//! One aligned physical backing allocation admitted against applicable ceilings.

use crate::resources::ResourcePools;
use mv::allocation::AllocationCharge;
use std::{
    alloc::{Layout, alloc_zeroed, dealloc},
    ptr::NonNull,
    sync::{Arc, atomic::Ordering},
};

/// Local refusal before a shared native allocation can be published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnifiedBufferError {
    /// Length, alignment or rounded extent cannot form a valid allocation.
    InvalidLayout,
    /// The original host ceiling cannot fund this physical backing region.
    HostCapacity,
    /// The original device ceiling cannot admit access to this same region.
    DeviceCapacity,
    /// The allocator refused already admitted backing storage.
    Allocation,
}

impl std::fmt::Display for UnifiedBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidLayout => "shared native buffer layout is not representable",
            Self::HostCapacity => "shared native buffer exceeds host capacity",
            Self::DeviceCapacity => "shared native buffer exceeds device capacity",
            Self::Allocation => "shared native backing allocation failed",
        })
    }
}
impl std::error::Error for UnifiedBufferError {}

/// A zero-initialized, aligned physical allocation with original-pool custody.
///
/// The two ceiling credits describe the same allocation; they are not two
/// physical allocations. Native wrappers must outlive all device access before
/// dropping this owner, or retain it permanently if completion is uncertain.
/// There is no conversion into an uncharged vector or a detached credit.
pub struct UnifiedBuffer {
    pointer: NonNull<u8>,
    layout: Layout,
    len: usize,
    _host_ceiling: AllocationCharge,
    _device_ceiling: AllocationCharge,
    pools: Arc<ResourcePools>,
}

impl UnifiedBuffer {
    pub(crate) fn allocate(
        pools: &Arc<ResourcePools>,
        len: usize,
        alignment: usize,
    ) -> Result<Self, UnifiedBufferError> {
        let layout = Layout::from_size_align(len, alignment)
            .map_err(|_| UnifiedBufferError::InvalidLayout)?
            .pad_to_align();
        let mut host = pools
            .host
            .try_reserve_bytes(layout.size())
            .map_err(|_| UnifiedBufferError::HostCapacity)?;
        let mut device = pools
            .device
            .try_reserve_bytes(layout.size())
            .map_err(|_| UnifiedBufferError::DeviceCapacity)?;
        let host_ceiling = host
            .try_split(layout)
            .map_err(|_| UnifiedBufferError::HostCapacity)?;
        let device_ceiling = device
            .try_split(layout)
            .map_err(|_| UnifiedBufferError::DeviceCapacity)?;
        let pointer = if layout.size() == 0 {
            // No access is permitted for this zero-length allocation. The
            // nonzero validated alignment provides an aligned dangling pointer.
            NonNull::new(layout.align() as *mut u8).ok_or(UnifiedBufferError::InvalidLayout)?
        } else {
            // SAFETY: Layout checked the nonzero size and power-of-two alignment;
            // both applicable original ceilings are funded before allocation.
            NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or(UnifiedBufferError::Allocation)?
        };
        let old = pools
            .unified_bytes
            .fetch_add(layout.size(), Ordering::AcqRel);
        pools
            .peak_unified_bytes
            .fetch_max(old + layout.size(), Ordering::Relaxed);
        Ok(Self {
            pointer,
            layout,
            len,
            _host_ceiling: host_ceiling,
            _device_ceiling: device_ceiling,
            pools: Arc::clone(pools),
        })
    }

    /// Checked full backing extent for capability checks before any allocation.
    pub fn required_capacity(len: usize, alignment: usize) -> Result<usize, UnifiedBufferError> {
        Layout::from_size_align(len, alignment)
            .map(|layout| layout.pad_to_align().size())
            .map_err(|_| UnifiedBufferError::InvalidLayout)
    }

    /// Logical initialized byte length, excluding alignment padding.
    pub fn len(&self) -> usize {
        self.len
    }
    /// Whether there are no logical bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Full aligned physical extent admitted before allocation.
    pub fn capacity(&self) -> usize {
        self.layout.size()
    }
    /// Allocation alignment used by the actual backing allocator.
    pub fn alignment(&self) -> usize {
        self.layout.align()
    }
    /// Stable raw address for a native no-copy wrapper.
    ///
    /// Dereferencing the address is unsafe. Consumers must ensure that the
    /// backing owner remains live and that CPU/device access is synchronized.
    pub fn as_ptr(&self) -> NonNull<u8> {
        self.pointer
    }
    /// Initialize logical bytes before transferring access to a native wrapper.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: this owner uniquely controls an initialized allocation. The
        // safe mutable borrow prevents simultaneous safe CPU access.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len) }
    }
}

impl Drop for UnifiedBuffer {
    fn drop(&mut self) {
        if self.layout.size() != 0 {
            // SAFETY: this exact allocator/layout created the unique backing.
            // Native wrapper safety requires completion before owner destruction.
            unsafe { dealloc(self.pointer.as_ptr(), self.layout) };
        }
        self.pools
            .unified_bytes
            .fetch_sub(self.layout.size(), Ordering::AcqRel);
        // Both ceiling credits drop only after the physical deallocation.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GpuResourceLimits;

    fn pools(bytes: usize) -> Arc<ResourcePools> {
        ResourcePools::new(GpuResourceLimits {
            host_bytes: bytes,
            pinned_bytes: 0,
            device_bytes: bytes,
            in_flight: 1,
        })
    }
    #[test]
    fn one_aligned_allocation_keeps_both_original_ceilings_until_deallocation() {
        let pools = pools(8192);
        let mut value = UnifiedBuffer::allocate(&pools, 33, 4096).unwrap();
        assert_eq!(value.len(), 33);
        assert!(!value.is_empty());
        assert_eq!(value.capacity(), 4096);
        assert_eq!(UnifiedBuffer::required_capacity(33, 4096), Ok(4096));
        assert_eq!(value.alignment(), 4096);
        assert_eq!(value.as_ptr().as_ptr() as usize % 4096, 0);
        assert!(value.as_mut_slice().iter().all(|&byte| byte == 0));
        value.as_mut_slice().copy_from_slice(&[91; 33]);
        assert_eq!(pools.usage().reserved.host_bytes, 4096);
        assert_eq!(pools.usage().reserved.device_bytes, 4096);
        assert_eq!(pools.unified_bytes.load(Ordering::Acquire), 4096);
        pools.set_limits(GpuResourceLimits {
            host_bytes: 0,
            pinned_bytes: 0,
            device_bytes: 0,
            in_flight: 0,
        });
        assert!(matches!(
            UnifiedBuffer::allocate(&pools, 1, 4096),
            Err(UnifiedBufferError::HostCapacity)
        ));
        assert_eq!(value.as_mut_slice(), [91; 33]);
        drop(value);
        assert_eq!(pools.usage().reserved.host_bytes, 0);
        assert_eq!(pools.usage().reserved.device_bytes, 0);
        assert_eq!(pools.unified_bytes.load(Ordering::Acquire), 0);
        assert_eq!(pools.peak_unified_bytes.load(Ordering::Acquire), 4096);
    }
    #[test]
    fn malformed_geometry_and_partial_admission_leave_no_live_credits() {
        let pools = pools(4096);
        for (len, align) in [(1, 0), (1, 3), (usize::MAX, 4096)] {
            assert_eq!(
                UnifiedBuffer::required_capacity(len, align),
                Err(UnifiedBufferError::InvalidLayout)
            );
            assert!(matches!(
                UnifiedBuffer::allocate(&pools, len, align),
                Err(UnifiedBufferError::InvalidLayout)
            ));
        }
        pools.set_limits(GpuResourceLimits {
            host_bytes: 4096,
            pinned_bytes: 0,
            device_bytes: 0,
            in_flight: 0,
        });
        assert!(matches!(
            UnifiedBuffer::allocate(&pools, 1, 4096),
            Err(UnifiedBufferError::DeviceCapacity)
        ));
        assert_eq!(pools.usage().reserved.host_bytes, 0);
        assert_eq!(pools.usage().reserved.device_bytes, 0);
        assert_eq!(pools.unified_bytes.load(Ordering::Acquire), 0);
        let mut empty = UnifiedBuffer::allocate(&pools, 0, 4096).unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.capacity(), 0);
        assert!(empty.as_mut_slice().is_empty());
    }
    #[test]
    fn unwind_reclaims_the_actual_backing_before_refunding_ceilings() {
        let pools = pools(4096);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _buffer = UnifiedBuffer::allocate(&pools, 8, 4096).unwrap();
            panic!("injected caller unwind before native construction");
        }));
        assert!(result.is_err());
        assert_eq!(pools.usage().reserved.host_bytes, 0);
        assert_eq!(pools.usage().reserved.device_bytes, 0);
        assert_eq!(pools.unified_bytes.load(Ordering::Acquire), 0);
    }
}
