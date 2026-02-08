//! Runtime façade for embedding the IVM safely.
//!
//! The goal of this module is to expose a compact interface that callers can
//! depend on instead of touching the sprawling `IVM` implementation directly.
//! Keeping the entry points narrow makes it easier to reason about security
//! policies (syscalls, pointer checks) and lets future optimisations – such as
//! alternate interpreters or JITs – slot in behind the same trait without
//! compromising determinism.
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

use std::{
    any::Any,
    sync::{Arc, Mutex},
};

pub use crate::ivm::{
    AccelerationPolicy, HardwareCapabilities, IvmBuilder, IvmConfig, IvmConfigBuilder,
};
use crate::{VMError, host::IVMHost, ivm::IVM, metadata::ProgramMetadata, syscalls};

/// Runtime operations exposed by the VM core.
pub trait VmEngine {
    /// Attach a host implementation. Hosts are responsible for syscall handling
    /// and should enforce any execution policy required by the deployment. The
    /// generic parameter ensures strongly typed hosts can be attached without
    /// forcing callers to allocate trait objects themselves.
    fn set_host<H: IVMHost + Send + 'static>(&mut self, host: H);

    /// Load a compiled program (`.to` bytecode) into the VM. Implementations
    /// must preserve the existing INPUT buffer so that hosts can preload TLVs
    /// deterministically prior to execution.
    fn load_program(&mut self, program: &[u8]) -> Result<(), VMError>;

    /// Execute the currently loaded program from the `pc`. Errors must surface
    /// deterministically and leave the VM in a halted state.
    fn run(&mut self) -> Result<(), VMError>;

    /// Access immutable program metadata as parsed from the bytecode header.
    fn program_metadata(&self) -> &ProgramMetadata;

    /// Convenience helper that sets a host, loads a program and immediately
    /// executes it. Useful for embedding scenarios where the host lifecycle is
    /// scoped to a single call.
    fn execute_with_host<H: IVMHost + Send + 'static>(
        &mut self,
        host: H,
        program: &[u8],
    ) -> Result<(), VMError> {
        self.set_host(host);
        self.load_program(program)?;
        self.run()
    }
}

impl VmEngine for IVM {
    fn set_host<H: IVMHost + Send + 'static>(&mut self, host: H) {
        IVM::set_host(self, host);
    }

    fn load_program(&mut self, program: &[u8]) -> Result<(), VMError> {
        IVM::load_program(self, program)
    }

    fn run(&mut self) -> Result<(), VMError> {
        IVM::run(self)
    }

    fn program_metadata(&self) -> &ProgramMetadata {
        IVM::metadata(self)
    }
}

/// Wrapper that enforces syscall policy before delegating to the underlying host.
///
/// Future iterations will extend this struct with pointer-ABI validation tables
/// so that hosts no longer need to duplicate TLV checks for well-known syscalls.
pub struct SyscallDispatcher<H> {
    inner: H,
}

impl<H> SyscallDispatcher<H> {
    /// Create a dispatcher around `host`.
    pub fn new(host: H) -> Self {
        Self { inner: host }
    }

    /// Access the wrapped host.
    pub fn inner(&self) -> &H {
        &self.inner
    }

    /// Access the wrapped host mutably.
    pub fn inner_mut(&mut self) -> &mut H {
        &mut self.inner
    }

    /// Consume the dispatcher and return the wrapped host.
    pub fn into_inner(self) -> H {
        self.inner
    }
}

impl<H: IVMHost> IVMHost for SyscallDispatcher<H> {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        if !syscalls::is_syscall_allowed(vm.syscall_policy(), number) {
            return Err(VMError::UnknownSyscall(number));
        }
        self.inner.syscall(number, vm)
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
    inner: Arc<Mutex<Option<Box<dyn IVMHost + Send>>>>,
}

impl SharedHost {
    fn new(inner: Arc<Mutex<Option<Box<dyn IVMHost + Send>>>>) -> Self {
        Self { inner }
    }
}

impl IVMHost for SharedHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let Some(host) = guard.as_mut() else {
            return Err(VMError::HostUnavailable);
        };
        host.syscall(number, vm)
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
    pub fn shared(host: Arc<Mutex<Option<Box<dyn IVMHost + Send>>>>) -> Self {
        Self::new(SharedHost::new(host))
    }
}
