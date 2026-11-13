//! KOTODAMA high-level language support.
//!
//! Bytecode Target
//! ---------------
//! Kotodama compiles to Iroha Virtual Machine (IVM) bytecode (`.to`). It does not target RISC‑V as
//! a standalone architecture. The compiler emits only the canonical IVM encoding: one 32-bit word
//! per instruction with an 8-bit opcode, 32 registers (`r0` reads as zero), signed 14-bit
//! immediates, displacements and branch word offsets, a signed 19-bit `JAL` and 24-bit
//! `JMP`/`JALS` word offset, and one `SCALL` opcode carrying a 24-bit syscall number. Encoders
//! reject out-of-range operands instead of truncating them. Observable behavior and outputs are
//! defined by IVM.
//!
//! This module provides the building blocks for a compiler that translates
//! Kotodama source programs into IVM bytecode.
//!
//! Multi-file projects use [`linker::SourceLinkRequest`]: a deployable root, explicit companion
//! sources, and optional locked packages. [`session::CompilerSession::build_source_bundle`]
//! compiles that closed inventory without filesystem access. [`compiler::Compiler::compile_file`]
//! loads the declared include/import closure within the entry file's parent directory; the build
//! driver and CLI also accept an explicit source root. Declaration includes share their owner's
//! scope, while imported modules expose only declarations marked `export`.
mod abi_schema;
pub mod ast;
mod call_abi;
mod checked_arithmetic;
pub mod compiler;
pub mod diagnostic;
mod doc_consistency;
pub mod driver;
pub mod editor;
pub mod formatter;
pub mod glossary;
pub mod i18n;
pub mod ir;
pub mod lexer;
pub mod linker;
pub mod lint;
pub mod parser;
mod pointer_literals;
pub mod policy;
pub mod regalloc;
pub mod resolved;
mod result_use;
mod secret;
pub mod semantic;
mod semantic_diagnostics;
pub mod session;
pub mod signature_render;
pub mod source;
pub mod spanned_ast;
mod ssa;
pub mod syntax;
pub mod testing;
pub use ivm_abi::{
    Perm, SyscallPolicy, VMError, axt, dev_env, encoding, instruction, metadata, pointer_abi,
    syscalls,
};
