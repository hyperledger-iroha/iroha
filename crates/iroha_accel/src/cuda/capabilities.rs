//! Immutable public device bounds and aggregate requested-byte permission.
//!
//! This permission bounds attempts by device capacity. Physical allocation bytes
//! are charged only in the process resource envelope, never twice here.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{CudaFailure, checked};
use cust::sys;

#[derive(Clone, Copy, Debug)]
pub(super) struct Capabilities {
    pub(super) total_bytes: usize,
    max_threads: u32,
    max_block: [u32; 3],
    max_grid: [u32; 3],
    max_shared_bytes: u32,
}

impl Capabilities {
    pub(super) fn discover(device: sys::CUdevice) -> Result<Self, CudaFailure> {
        use sys::CUdevice_attribute::*;
        fn attribute(
            device: sys::CUdevice,
            kind: sys::CUdevice_attribute,
        ) -> Result<u32, CudaFailure> {
            let mut value = 0;
            // SAFETY: the initialized scalar has the driver attribute's exact type.
            checked(unsafe { sys::cuDeviceGetAttribute(&raw mut value, kind, device) })?;
            u32::try_from(value).map_err(|_| CudaFailure::InvalidRequest)
        }
        let mut total_bytes = 0;
        // SAFETY: initialized usize is the total-memory API's exact out-parameter.
        checked(unsafe { sys::cuDeviceTotalMem_v2(&raw mut total_bytes, device) })?;
        let capabilities = Self {
            total_bytes,
            max_threads: attribute(device, CU_DEVICE_ATTRIBUTE_MAX_THREADS_PER_BLOCK)?,
            max_block: [
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_X)?,
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Y)?,
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_BLOCK_DIM_Z)?,
            ],
            max_grid: [
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_X)?,
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Y)?,
                attribute(device, CU_DEVICE_ATTRIBUTE_MAX_GRID_DIM_Z)?,
            ],
            max_shared_bytes: attribute(device, CU_DEVICE_ATTRIBUTE_MAX_SHARED_MEMORY_PER_BLOCK)?,
        };
        if capabilities.total_bytes == 0
            || capabilities.max_threads == 0
            || capabilities.max_block.contains(&0)
            || capabilities.max_grid.contains(&0)
        {
            return Err(CudaFailure::InvalidRequest);
        }
        Ok(capabilities)
    }

    #[cfg(test)]
    pub(super) fn test_device() -> Self {
        Self {
            total_bytes: 1024,
            max_threads: 256,
            max_block: [256, 16, 8],
            max_grid: [4096, 32, 32],
            max_shared_bytes: 1024,
        }
    }

    pub(super) fn permits_launch(
        &self,
        grid: [u32; 3],
        block: [u32; 3],
        shared_bytes: u32,
    ) -> bool {
        if grid.contains(&0) || block.contains(&0) || shared_bytes > self.max_shared_bytes {
            return false;
        }
        if grid
            .into_iter()
            .zip(self.max_grid)
            .any(|(value, cap)| value > cap)
            || block
                .into_iter()
                .zip(self.max_block)
                .any(|(value, cap)| value > cap)
        {
            return false;
        }
        block
            .into_iter()
            .try_fold(1u32, u32::checked_mul)
            .is_some_and(|threads| threads <= self.max_threads)
    }
}

/// Requested-byte permission, independent of (and never reported as) residency.
/// Immutable physical capacity requires no config lock or second allocation pool.
pub(super) struct DeviceCapacity {
    limit: usize,
    used: AtomicUsize,
}

impl DeviceCapacity {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            used: AtomicUsize::new(0),
        }
    }

    pub(super) fn try_acquire(&self, bytes: usize) -> bool {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|&next| next <= self.limit)
            })
            .is_ok()
    }

    pub(super) fn release(&self, bytes: usize) {
        self.used.fetch_sub(bytes, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Capabilities {
        Capabilities {
            total_bytes: 1024,
            max_threads: 256,
            max_block: [256, 16, 8],
            max_grid: [4096, 32, 32],
            max_shared_bytes: 1024,
        }
    }

    #[test]
    fn launch_checks_every_dimension_product_and_shared_memory() {
        let caps = device();
        assert!(caps.permits_launch([4096, 32, 32], [16, 16, 1], 1024));
        assert!(!caps.permits_launch([4097, 1, 1], [256, 1, 1], 0));
        assert!(!caps.permits_launch([1, 33, 1], [256, 1, 1], 0));
        assert!(!caps.permits_launch([1, 1, 33], [256, 1, 1], 0));
        assert!(!caps.permits_launch([1; 3], [257, 1, 1], 0));
        assert!(!caps.permits_launch([1; 3], [1, 17, 1], 0));
        assert!(!caps.permits_launch([1; 3], [1, 1, 9], 0));
        assert!(!caps.permits_launch([1; 3], [16, 16, 2], 0));
        assert!(!caps.permits_launch([1; 3], [256, 1, 1], 1025));
        for dimension in 0..3 {
            let mut grid = [1; 3];
            grid[dimension] = 0;
            let mut block = [1; 3];
            block[dimension] = 0;
            assert!(!caps.permits_launch(grid, [1; 3], 0));
            assert!(!caps.permits_launch([1; 3], block, 0));
        }
        let huge = Capabilities {
            max_threads: u32::MAX,
            max_block: [u32::MAX; 3],
            ..caps
        };
        assert!(!huge.permits_launch([1; 3], [u32::MAX; 3], 0));
    }

    #[test]
    fn concurrent_attempts_share_one_physical_capacity_permission() {
        let capacity = DeviceCapacity::new(1024);
        assert!(capacity.try_acquire(768));
        assert!(capacity.try_acquire(256));
        assert!(!capacity.try_acquire(1));
        assert!(!capacity.try_acquire(usize::MAX));
        capacity.release(768);
        assert!(capacity.try_acquire(768));
        capacity.release(768);
        capacity.release(256);
        assert!(capacity.try_acquire(1024));
        capacity.release(1024);
        assert_eq!(capacity.used.load(Ordering::Acquire), 0);
    }
}
