//! KOTODAMA high-level language support.
//!
//! Bytecode Target
//! ---------------
//! Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`). It does
//! not target RISC‑V as a standalone architecture. The compiler now emits IVM's
//! native wide (8-bit opcode) encodings exclusively. Earlier RISC‑V–like
//! encodings (e.g., classic `0x33/0x13` ALU forms) are rejected by the VM
//! loader and interpreter; they now exist only in regression tests that verify
//! the trap behaviour. Observable behavior and outputs are defined by IVM.
//!
//! This module provides the building blocks for a compiler that translates
//! Kotodama source programs into IVM bytecode.

pub mod analysis;
pub mod ast;
pub mod builtins;
pub mod compiler;
mod doc_consistency;
pub mod i18n;
pub mod ir;
pub mod lexer;
pub mod lint;
pub mod parser;
pub mod policy;
pub mod regalloc;
pub mod semantic;
pub mod wide;

pub use ivm_abi::{
    Perm, SyscallPolicy, VMError, axt, dev_env, encoding, instruction, metadata, pointer_abi,
    syscalls,
};
