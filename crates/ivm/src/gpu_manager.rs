#[cfg(feature = "cuda")]
use std::sync::{Arc, OnceLock, atomic::AtomicU64};

#[cfg(feature = "cuda")]
use cust::context::CurrentContext;
#[cfg(feature = "cuda")]
use cust::init;
#[cfg(feature = "cuda")]
use cust::prelude::*;
#[cfg(feature = "cuda")]
use parking_lot::RwLock;

#[cfg(feature = "cuda")]
#[derive(Debug)]
pub struct GpuContext {
    pub device: Device,
    pub context: Context,
}

#[cfg(feature = "cuda")]
impl GpuContext {
    fn new(device: Device) -> cust::error::CudaResult<Self> {
        let context = Context::new(device)?;
        context.set_flags(ContextFlags::SCHED_AUTO | ContextFlags::MAP_HOST)?;
        Ok(GpuContext { device, context })
    }

    pub fn with_stream<F, T>(&self, func: F) -> Option<T>
    where
        F: FnOnce(&Stream) -> Option<T>,
    {
        CurrentContext::set_current(&self.context).ok()?;
        let stream = Stream::new(StreamFlags::DEFAULT, None).ok()?;
        func(&stream)
    }
}

#[cfg(feature = "cuda")]
#[derive(Debug)]
pub struct GpuManager {
    gpus: Vec<GpuContext>,
}

#[cfg(feature = "cuda")]
#[derive(Default)]
struct GpuManagerCache {
    manager: Option<Arc<GpuManager>>,
    version: u64,
}

#[cfg(feature = "cuda")]
#[derive(Clone, Debug)]
pub struct GpuManagerHandle {
    inner: Arc<GpuManager>,
}

#[cfg(feature = "cuda")]
impl std::ops::Deref for GpuManagerHandle {
    type Target = GpuManager;

    fn deref(&self) -> &GpuManager {
        &self.inner
    }
}

#[cfg(feature = "cuda")]
impl GpuManagerHandle {
    fn new(inner: Arc<GpuManager>) -> Self {
        Self { inner }
    }
}

#[cfg(feature = "cuda")]
static GPU_MANAGER: OnceLock<RwLock<GpuManagerCache>> = OnceLock::new();
#[cfg(feature = "cuda")]
static CONFIG_MAX_GPUS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(feature = "cuda")]
static CONFIG_VERSION: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "cuda")]
fn manager_cache() -> &'static RwLock<GpuManagerCache> {
    GPU_MANAGER.get_or_init(|| RwLock::new(GpuManagerCache::default()))
}

#[cfg(feature = "cuda")]
impl GpuManager {
    /// Initialize and create contexts for all detected GPUs.
    pub fn init() -> Option<Self> {
        init(CudaFlags::empty()).ok()?;
        let count = Device::num_devices().ok()?;
        if count == 0 {
            return None;
        }
        let cfg_max = CONFIG_MAX_GPUS.load(std::sync::atomic::Ordering::SeqCst);
        let max = if cfg_max > 0 { cfg_max as u32 } else { count };
        let limit = std::cmp::min(count, max);
        if limit == 0 {
            return None;
        }
        let mut gpus = Vec::with_capacity(limit as usize);
        for idx in 0..limit {
            let device = Device::get_device(idx).ok()?;
            let ctx = GpuContext::new(device).ok()?;
            gpus.push(ctx);
        }
        Some(Self { gpus })
    }

    /// Number of GPU devices initialized.
    pub fn device_count(&self) -> usize {
        self.gpus.len()
    }

    /// Map a task ID deterministically to a GPU index.
    pub fn gpu_for_task(&self, task_id: u64) -> usize {
        if self.gpus.is_empty() {
            return 0;
        }
        (task_id as usize) % self.gpus.len()
    }

    /// Execute a closure on the GPU assigned for the given task ID.
    pub fn with_gpu_for_task<F, T>(&self, task_id: u64, func: F) -> Option<T>
    where
        F: FnOnce(&GpuContext) -> T,
    {
        if self.gpus.is_empty() {
            return None;
        }
        let idx = self.gpu_for_task(task_id);
        self.gpus.get(idx).map(func)
    }

    /// Return a handle to the global GPU manager, initializing it on first use.
    pub fn shared() -> Option<GpuManagerHandle> {
        let target_version = CONFIG_VERSION.load(std::sync::atomic::Ordering::SeqCst);
        // Fast path: cached manager matches current configuration
        if let Some(handle) = {
            let cache = manager_cache();
            let guard = cache.read();
            if guard.version == target_version {
                guard
                    .manager
                    .as_ref()
                    .map(|mgr| GpuManagerHandle::new(Arc::clone(mgr)))
            } else {
                None
            }
        } {
            return Some(handle);
        }

        // Slow path: upgrade to write lock and (re)initialize if needed.
        let cache = manager_cache();
        let mut guard = cache.write();
        if guard.version != target_version {
            guard.manager = None;
            guard.version = target_version;
        }
        if guard.manager.is_none() {
            let manager = GpuManager::init()?;
            guard.manager = Some(Arc::new(manager));
        }
        guard
            .manager
            .as_ref()
            .map(|mgr| GpuManagerHandle::new(Arc::clone(mgr)))
    }

    /// Set a configuration cap for the number of GPUs (0 = auto/no cap).
    pub fn set_max_gpus(limit: Option<usize>) {
        let v = limit.unwrap_or(0);
        CONFIG_MAX_GPUS.store(v, std::sync::atomic::Ordering::SeqCst);
        let new_version = CONFIG_VERSION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let cache = manager_cache();
        let mut guard = cache.write();
        guard.manager = None;
        guard.version = new_version;
    }
}

#[cfg(all(test, feature = "cuda"))]
pub(crate) fn test_cache_version() -> u64 {
    manager_cache().read().version
}

#[cfg(all(test, feature = "cuda"))]
pub(crate) fn test_has_cached_manager() -> bool {
    manager_cache().read().manager.is_some()
}

#[cfg(all(test, feature = "cuda"))]
pub(crate) fn test_current_max_gpus() -> Option<usize> {
    match CONFIG_MAX_GPUS.load(std::sync::atomic::Ordering::SeqCst) {
        0 => None,
        v => Some(v),
    }
}

#[cfg(not(feature = "cuda"))]
#[derive(Debug)]
pub struct GpuManager;

#[cfg(not(feature = "cuda"))]
#[derive(Clone, Debug)]
pub struct GpuManagerHandle;

#[cfg(not(feature = "cuda"))]
impl GpuManager {
    pub fn init() -> Option<Self> {
        None
    }
    pub fn device_count(&self) -> usize {
        0
    }
    pub fn gpu_for_task(&self, _task_id: u64) -> usize {
        0
    }
    pub fn with_gpu_for_task<F, T>(&self, _task_id: u64, _func: F) -> Option<T>
    where
        F: FnOnce(&()) -> T,
    {
        None
    }

    pub fn shared() -> Option<GpuManagerHandle> {
        None
    }
    pub fn set_max_gpus(_limit: Option<usize>) {}
}

#[cfg(all(test, feature = "cuda"))]
mod tests {
    use super::*;

    #[test]
    fn set_max_gpus_invalidates_cached_manager() {
        let baseline_limit = super::test_current_max_gpus();
        // Touch the cache once so we can observe transitions even without GPUs.
        let _ = GpuManager::shared();
        let version_after_shared = super::test_cache_version();

        let new_limit = baseline_limit.unwrap_or(0).saturating_add(1);
        GpuManager::set_max_gpus(Some(new_limit));

        let version_after_set = super::test_cache_version();
        assert!(
            version_after_set > version_after_shared,
            "configuration change should advance cache version"
        );
        assert!(
            !super::test_has_cached_manager(),
            "cache must be cleared after configuration change"
        );

        GpuManager::set_max_gpus(baseline_limit);
        let version_after_restore = super::test_cache_version();
        assert!(
            version_after_restore > version_after_set,
            "restoring configuration should also update cache version"
        );
    }
}
