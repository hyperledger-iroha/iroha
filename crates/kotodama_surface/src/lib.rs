//! Kotodama V1 source-surface registry shared by the compiler and contract
//! admission.
//!
//! This crate owns the canonical builtin registry, the generated V1 source
//! policy tables and the reserved-name predicates. It is kept as a leaf (it
//! depends only on `ivm_abi`) so that compiler edits never rebuild the VM or the
//! node, while admission and tooling still agree with the compiler on which
//! source names are reserved.

pub mod builtins;
pub mod source_policy;
