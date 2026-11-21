//! Shared ABI definitions for the Iroha Virtual Machine.
//!
//! This crate hosts the canonical opcode tables, metadata layout, pointer-ABI helpers, syscall
//! numbering, and related error types used by both the VM and the Kotodama compiler.
pub mod access_hints;
pub mod arguments;
pub mod axt;
pub mod baseline;
pub mod call;
pub mod codec;
pub mod contract_call;
pub mod core_query;
pub mod dev_env;
pub mod encoding;
pub mod entrypoint;
pub mod error;
pub mod error_types;
pub mod host_payload;
pub mod instruction;
pub mod json;
pub mod list;
pub mod metadata;
pub mod numeric;
pub mod numeric_tlv;
pub mod pointer_abi;
pub mod private_input;
pub mod profile;
pub mod state_cursor;
pub mod state_value;
pub mod sum;
pub mod syscalls;
pub mod upgrade;
pub use error::{HostOutputResource, Perm, VMError, VmDeferral, VmErrorRef, VmTrap};
pub use profile::{HeaderLayout, IvmEngine, IvmProfileId};
/// Named host policy authenticated by the fixed program header's exact ABI hash.
/// Every policy uses first-release ABI version 1. [`IvmProfileId::of_policy`] binds each policy
/// to the engine that executes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyscallPolicy {
    /// ABI surface for version 1 programs.
    AbiV1,
    /// ABI V1 with authenticated contract calls to the runtime obligation kernel.
    /// The baseline profile and its descriptor remain unchanged.
    AbiV1RuntimeLifecycle,
    /// Descriptor-selected carrier instructions with invocation-owned indexed storage access.
    /// This closed surface is never an ordinary raw-program or deployed-contract interface.
    AbiV1GenericCarrier,
}

#[cfg(test)]
mod captured_identity_tests;

#[cfg(test)]
mod enum_tests;
