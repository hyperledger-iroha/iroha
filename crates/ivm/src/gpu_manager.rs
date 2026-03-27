#[cfg(feature = "cuda")]
use std::cell::Cell;
#[cfg(feature = "cuda")]
use std::collections::HashMap;
#[cfg(feature = "cuda")]
use std::sync::{Arc, OnceLock, atomic::AtomicU64};

#[cfg(feature = "cuda")]
use cust::context::CurrentContext;
#[cfg(feature = "cuda")]
use cust::error::CudaError;
#[cfg(feature = "cuda")]
use cust::init;
#[cfg(feature = "cuda")]
use cust::prelude::*;
#[cfg(feature = "cuda")]
use parking_lot::{Mutex, RwLock};

#[cfg(feature = "cuda")]
#[derive(Debug)]
pub struct GpuContext {
    pub device: Device,
    pub context: Context,
    modules: RwLock<HashMap<&'static str, Arc<Module>>>,
    u64_buffers: RwLock<HashMap<&'static str, DeviceBuffer<u64>>>,
    stream: Mutex<Stream>,
}

#[cfg(feature = "cuda")]
impl GpuContext {
    fn new(device: Device) -> cust::error::CudaResult<Self> {
        let context = Context::new(device)?;
        // WSL/driver-only hosts can expose a working primary context while rejecting
        // `MAP_HOST` on `cuDevicePrimaryCtxSetFlags_v2` with `CUDA_ERROR_INVALID_VALUE`.
        // None of the current IVM CUDA paths require pinned host mappings, so keep
        // the context usable by retrying with the scheduler flag only.
        let preferred_flags = ContextFlags::SCHED_AUTO | ContextFlags::MAP_HOST;
        match context.set_flags(preferred_flags) {
            Ok(()) => {}
            Err(err) => {
                if let Some(fallback_flags) = map_host_flag_fallback(err) {
                    context.set_flags(fallback_flags)?;
                } else {
                    return Err(err);
                }
            }
        }
        CurrentContext::set_current(&context)?;
        let stream = Stream::new(StreamFlags::DEFAULT, None)?;
        Ok(GpuContext {
            device,
            context,
            modules: RwLock::new(HashMap::new()),
            u64_buffers: RwLock::new(HashMap::new()),
            stream: Mutex::new(stream),
        })
    }

    pub fn with_stream<F, T>(&self, func: F) -> Option<T>
    where
        F: FnOnce(&Stream) -> Option<T>,
    {
        CurrentContext::set_current(&self.context).ok()?;
        let stream = self.stream.lock();
        func(&stream)
    }

    pub fn cached_module(&self, key: &'static str, ptx: &'static str) -> Option<Arc<Module>> {
        CurrentContext::set_current(&self.context).ok()?;
        if let Some(module) = self.modules.read().get(key).cloned() {
            return Some(module);
        }
        let module = Arc::new(Module::from_ptx(ptx, &[]).ok()?);
        let mut modules = self.modules.write();
        Some(
            modules
                .entry(key)
                .or_insert_with(|| Arc::clone(&module))
                .clone(),
        )
    }

    pub fn with_cached_u64_buffer<T>(
        &self,
        key: &'static str,
        host_data: &[u64],
        func: impl FnOnce(&DeviceBuffer<u64>) -> Option<T>,
    ) -> Option<T> {
        CurrentContext::set_current(&self.context).ok()?;
        if !self.u64_buffers.read().contains_key(key) {
            let buffer = DeviceBuffer::from_slice(host_data).ok()?;
            let mut buffers = self.u64_buffers.write();
            buffers.entry(key).or_insert(buffer);
        }
        let buffers = self.u64_buffers.read();
        let buffer = buffers.get(key)?;
        func(buffer)
    }
}

#[cfg(feature = "cuda")]
fn map_host_flag_fallback(err: CudaError) -> Option<ContextFlags> {
    match err {
        CudaError::InvalidValue | CudaError::PrimaryContextActive => Some(ContextFlags::SCHED_AUTO),
        _ => None,
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
thread_local! {
    static GPU_TASK_OVERRIDE: Cell<Option<u64>> = const { Cell::new(None) };
}

#[cfg(feature = "cuda")]
fn manager_cache() -> &'static RwLock<GpuManagerCache> {
    GPU_MANAGER.get_or_init(|| RwLock::new(GpuManagerCache::default()))
}

#[cfg(feature = "cuda")]
fn cuda_manager_enabled() -> bool {
    !crate::cuda::cuda_disabled()
}

#[cfg(feature = "cuda")]
fn resolve_task_id(task_id: u64) -> u64 {
    if task_id != 0 {
        return task_id;
    }
    GPU_TASK_OVERRIDE.with(|slot| slot.get().unwrap_or(0))
}

#[cfg(feature = "cuda")]
pub(crate) fn with_task_scope<T>(task_id: u64, func: impl FnOnce() -> T) -> T {
    struct ResetGuard(Option<u64>);

    impl Drop for ResetGuard {
        fn drop(&mut self) {
            GPU_TASK_OVERRIDE.with(|slot| slot.set(self.0));
        }
    }

    let previous = GPU_TASK_OVERRIDE.with(|slot| {
        let old = slot.get();
        slot.set(Some(task_id));
        old
    });
    let _reset = ResetGuard(previous);
    func()
}

#[cfg(feature = "cuda")]
fn clear_cached_manager(version: u64) {
    let cache = manager_cache();
    let mut guard = cache.write();
    guard.manager = None;
    guard.version = version;
}

#[cfg(feature = "cuda")]
impl GpuManager {
    /// Initialize and create contexts for all detected GPUs.
    pub fn init() -> Option<Self> {
        if !cuda_manager_enabled() {
            return None;
        }
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
        (resolve_task_id(task_id) as usize) % self.gpus.len()
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
        if !cuda_manager_enabled() {
            clear_cached_manager(CONFIG_VERSION.load(std::sync::atomic::Ordering::SeqCst));
            return None;
        }
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

    pub(crate) fn invalidate_cache() {
        let new_version = CONFIG_VERSION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        clear_cached_manager(new_version);
    }

    /// Set a configuration cap for the number of GPUs (0 = auto/no cap).
    pub fn set_max_gpus(limit: Option<usize>) {
        let v = limit.unwrap_or(0);
        CONFIG_MAX_GPUS.store(v, std::sync::atomic::Ordering::SeqCst);
        Self::invalidate_cache();
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

#[cfg(all(test, feature = "cuda"))]
pub(crate) fn test_resolve_task_id(task_id: u64) -> u64 {
    resolve_task_id(task_id)
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
    use std::sync::Arc;

    use super::*;

    static ADD_PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/add.ptx"));

    #[test]
    fn map_host_flag_fallback_drops_map_host_for_known_primary_context_errors() {
        assert_eq!(
            super::map_host_flag_fallback(CudaError::InvalidValue),
            Some(ContextFlags::SCHED_AUTO)
        );
        assert_eq!(
            super::map_host_flag_fallback(CudaError::PrimaryContextActive),
            Some(ContextFlags::SCHED_AUTO)
        );
        assert_eq!(
            super::map_host_flag_fallback(CudaError::InvalidDevice),
            None
        );
    }

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

    #[test]
    fn task_scope_overrides_default_task_zero_only() {
        assert_eq!(super::test_resolve_task_id(0), 0);
        assert_eq!(super::test_resolve_task_id(17), 17);

        super::with_task_scope(42, || {
            assert_eq!(super::test_resolve_task_id(0), 42);
            assert_eq!(super::test_resolve_task_id(17), 17);
        });

        assert_eq!(super::test_resolve_task_id(0), 0);
    }

    #[test]
    fn cached_module_reuses_loaded_handle_for_same_key() {
        let Some(manager) = GpuManager::shared() else {
            eprintln!("CUDA unavailable; skipping module cache regression");
            return;
        };
        let reused = manager.with_gpu_for_task(0, |gpu| {
            let first = gpu.cached_module("add-test", ADD_PTX)?;
            let second = gpu.cached_module("add-test", ADD_PTX)?;
            Some(Arc::ptr_eq(&first, &second))
        });
        assert_eq!(
            reused.flatten(),
            Some(true),
            "GpuContext should reuse a cached module handle for repeated loads of the same PTX key",
        );
    }

    #[test]
    fn with_stream_reuses_cached_stream_for_same_gpu() {
        let Some(manager) = GpuManager::shared() else {
            eprintln!("CUDA unavailable; skipping stream cache regression");
            return;
        };
        let reused = manager.with_gpu_for_task(0, |gpu| {
            let first = gpu.with_stream(|stream| Some(stream as *const Stream as usize))?;
            let second = gpu.with_stream(|stream| Some(stream as *const Stream as usize))?;
            Some(first == second)
        });
        assert_eq!(
            reused.flatten(),
            Some(true),
            "GpuContext should reuse the same cached stream across repeated helper calls",
        );
    }

    #[test]
    fn cached_u64_buffer_reuses_device_allocation_for_same_key() {
        let Some(manager) = GpuManager::shared() else {
            eprintln!("CUDA unavailable; skipping device buffer cache regression");
            return;
        };
        let reused = manager.with_gpu_for_task(0, |gpu| {
            let data = [1u64, 2, 3, 4];
            let first = gpu.with_cached_u64_buffer("u64-buffer-test", &data, |buffer| {
                Some(buffer.as_device_ptr().as_raw())
            })?;
            let second = gpu.with_cached_u64_buffer("u64-buffer-test", &data, |buffer| {
                Some(buffer.as_device_ptr().as_raw())
            })?;
            Some(first == second)
        });
        assert_eq!(
            reused.flatten(),
            Some(true),
            "GpuContext should reuse the same cached device allocation for repeated constant buffers",
        );
    }
}
