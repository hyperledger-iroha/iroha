//! Runtime façade for embedding the IVM safely.
//!
//! This module groups the VM construction types used by embedded hosts.
//!
//! Typical usage:
//! ```
//! use ivm::runtime::{IvmBuilder, IvmConfig};
//!
//! // Start from a configuration preset.
//! let config = IvmConfig::deterministic(1_000);
//! // Optionally tweak via the builder helpers.
//! let mut builder = IvmBuilder::with_config(config).suppress_startup_banner();
//! builder.set_gas_limit(2_000);
//! let (cfg, mut vm) = builder.build_with_config();
//! // `cfg` can be reused to spawn another VM later.
//! let mut vm2 = IvmBuilder::with_config(cfg)
//!     .suppress_startup_banner()
//!     .build();
//! // ... attach host, load programs, run, etc.
//! ```
pub use crate::ivm::{
    AccelerationPolicy, HardwareCapabilities, IvmBuilder, IvmConfig, IvmConfigBuilder,
};
pub use crate::stack_policy::IvmStackPolicy;
use crate::{VMError, host::IVMHost, ivm::IVM, syscalls};
use std::{
    any::Any,
    sync::{Arc, Mutex},
};
/// Wrapper that enforces syscall policy before delegating to the underlying host.
pub(crate) struct SyscallDispatcher<H> {
    inner: H,
}
impl<H> SyscallDispatcher<H> {
    /// Create a dispatcher around `host`.
    pub(crate) fn new(host: H) -> Self {
        Self { inner: host }
    }
}
impl<H: IVMHost> IVMHost for SyscallDispatcher<H> {
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError> {
        self.inner.prepare_syscall(number, vm)
    }
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if !self.allows_syscall(vm.syscall_policy(), number) {
            return Err(VMError::UnknownSyscall(number));
        }
        self.inner.syscall(number, vm)
    }
    fn allows_syscall(&self, policy: crate::SyscallPolicy, number: u32) -> bool {
        self.inner.allows_syscall(policy, number)
    }
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self.inner.as_any()
    }
    fn supports_concurrent_blocks(&self) -> bool {
        self.inner.supports_concurrent_blocks()
    }
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        self.inner.checkpoint()
    }
    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        self.inner.restore(snapshot)
    }
    fn begin_tx(&mut self, declared: &crate::parallel::StateAccessSet) -> Result<(), VMError> {
        self.inner.begin_tx(declared)
    }
    fn finish_tx(&mut self) -> Result<crate::host::AccessLog, VMError> {
        self.inner.finish_tx()
    }
    fn access_logging_supported(&self) -> bool {
        self.inner.access_logging_supported()
    }
}
/// Shared host wrapper used when cloning VMs across worker threads.
pub(crate) struct SharedHost {
    inner: Arc<Mutex<Option<Box<dyn IVMHost + Send + Sync>>>>,
}
impl SharedHost {
    fn new(inner: Arc<Mutex<Option<Box<dyn IVMHost + Send + Sync>>>>) -> Self {
        Self { inner }
    }
}
impl IVMHost for SharedHost {
    fn prepare_syscall(&self, number: u32, vm: &IVM) -> Result<u64, VMError> {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let Some(host) = guard.as_ref() else {
            return Err(VMError::HostUnavailable);
        };
        host.prepare_syscall(number, vm)
    }
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let Some(host) = guard.as_mut() else {
            return Err(VMError::HostUnavailable);
        };
        host.syscall(number, vm)
    }
    fn allows_syscall(&self, policy: crate::SyscallPolicy, number: u32) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard
            .as_ref()
            .map(|h| h.allows_syscall(policy, number))
            .unwrap_or_else(|| syscalls::is_syscall_allowed(policy, number))
    }
    fn as_any(&mut self) -> &mut dyn Any
    where
        Self: 'static,
    {
        self
    }
    fn supports_concurrent_blocks(&self) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard
            .as_ref()
            .map(|h| h.supports_concurrent_blocks())
            .unwrap_or(false)
    }
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.as_ref().and_then(|h| h.checkpoint())
    }
    fn restore(&mut self, snapshot: &dyn Any) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.as_mut().map(|h| h.restore(snapshot)).unwrap_or(false)
    }
    fn begin_tx(&mut self, declared: &crate::parallel::StateAccessSet) -> Result<(), VMError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard
            .as_mut()
            .map(|h| h.begin_tx(declared))
            .unwrap_or(Ok(()))
    }
    fn finish_tx(&mut self) -> Result<crate::host::AccessLog, VMError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard
            .as_mut()
            .map(|h| h.finish_tx())
            .unwrap_or_else(|| Ok(crate::host::AccessLog::default()))
    }
    fn access_logging_supported(&self) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard
            .as_ref()
            .map(|h| h.access_logging_supported())
            .unwrap_or(false)
    }
}
impl SyscallDispatcher<SharedHost> {
    /// Clone-safe dispatcher that forwards calls through a shared host.
    pub(crate) fn shared(host: Arc<Mutex<Option<Box<dyn IVMHost + Send + Sync>>>>) -> Self {
        Self::new(SharedHost::new(host))
    }
}
