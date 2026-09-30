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
use crate::{VMError, host::IVMHost, ivm::IVM};
use std::any::Any;
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
    fn checkpoint(&self) -> Option<Box<dyn Any + Send>> {
        self.inner.checkpoint()
    }
    fn restore(&mut self, snapshot: &dyn Any) -> Result<(), VMError> {
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
