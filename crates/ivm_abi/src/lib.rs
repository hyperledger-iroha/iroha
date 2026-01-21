//! Shared ABI definitions for the Iroha Virtual Machine.
//!
//! This crate hosts the canonical opcode tables, metadata layout, pointer-ABI
//! helpers, syscall numbering, and related error types used by both the VM and
//! the Kotodama compiler.

pub mod axt;
pub mod dev_env;
pub mod encoding;
pub mod error;
pub mod instruction;
pub mod metadata;
pub mod pointer_abi;
pub mod syscalls;

pub use error::{Perm, VMError};

/// Syscall policy determined by `ProgramMetadata.abi_version`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyscallPolicy {
    /// ABI surface for version 1 programs.
    AbiV1,
    /// Explicit ABI experiments keyed by header-version.
    Experimental(u8),
}
