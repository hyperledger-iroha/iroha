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
/// Wrapper that enforces syscall policy and retains the host's prepared argument owner.
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
    fn prepared_entrypoint_arguments(&self) -> Option<crate::PreparedArgumentRecord> {
        self.inner.prepared_entrypoint_arguments()
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PreparedArgumentRecord;
    use iroha_primitives::json::Json;
    use std::sync::Arc;

    struct PreparedHost(PreparedArgumentRecord);

    impl IVMHost for PreparedHost {
        fn prepared_entrypoint_arguments(&self) -> Option<PreparedArgumentRecord> {
            Some(self.0.clone())
        }

        fn prepare_syscall(&self, _number: u32, _vm: &IVM) -> Result<u64, VMError> {
            Err(VMError::PermissionDenied)
        }

        fn syscall(&mut self, _number: u32, _vm: &mut IVM) -> Result<u64, VMError> {
            panic!("prepared arguments must not request public input")
        }

        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn owned_host_retains_prepared_arguments_and_requires_exact_prepayment() {
        let (program, _) = crate::KotodamaCompiler::new()
            .compile_source_with_manifest(
                "seiyaku Prepared { view fn echo(bool ready) -> bool { return ready; } }",
            )
            .expect("compile parameterized view");
        let verified = crate::verify_contract_artifact(&program).expect("verify view");
        let schema = verified
            .contract_interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == "echo")
            .expect("echo entrypoint")
            .argument_schema
            .as_ref()
            .expect("argument schema");
        let canonical = crate::encode_argument_record_from_json(
            schema,
            &Json::from(norito::json!({"ready": true})),
        )
        .expect("encode arguments");
        crate::reset_argument_record_decode_count();
        let prepared =
            crate::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), 100_000)
                .expect("prepare arguments");

        let mut vm = IVM::new(100_000);
        vm.load_program(&program).expect("load view");
        vm.select_entrypoint("echo").expect("select view");
        prepared.precharge_vm(&mut vm).expect("prepay arguments");
        vm.set_host(PreparedHost(prepared.clone()));
        vm.run().expect("owned host retains preparation");
        assert_eq!(vm.call_result_word_count().unwrap(), 1);
        assert_eq!(vm.public_call_result_word(0).unwrap(), 1);
        #[cfg(debug_assertions)]
        assert_eq!(crate::argument_record_decode_count(), 1);

        let mut unpaid = IVM::new(100_000);
        unpaid.load_program(&program).unwrap();
        unpaid.select_entrypoint("echo").unwrap();
        unpaid.set_host(PreparedHost(prepared));
        assert_eq!(unpaid.run(), Err(VMError::DecodeError));
        assert!(unpaid.call_result_word_count().is_err());
        #[cfg(debug_assertions)]
        assert_eq!(crate::argument_record_decode_count(), 1);
    }
}
