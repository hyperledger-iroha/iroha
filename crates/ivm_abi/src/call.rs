//! Canonical V1 function calls through caller-owned word tables.
//!
//! These authenticated descriptors are shared by compiler emission, artifact admission, runtime
//! frame checks, and proof constraints. Syscall register conventions are independent of calls.

use norito::{Decode, Encode};

mod schema;
pub use schema::{CallNodeLayoutV1, CallSchemaSummaryV1, CallSchemaV1, CallTypeNodeV1};

/// Maximum nodes in one private callable schema, independently of public record limits.
pub const MAX_CALL_SCHEMA_NODES_V1: usize = 250_000;
/// Maximum nesting of a callable value schema.
pub const MAX_CALL_SCHEMA_DEPTH_V1: usize = 256;

/// Width and alignment of every call-table slot.
pub const CALL_WORD_BYTES_V1: usize = 8;
/// Maximum storage reserved for either call table.
pub const MAX_CALL_TABLE_BYTES_V1: usize = 64 * 1024;
/// Maximum flattened words in one argument or result table.
pub const MAX_CALL_WORDS_V1: usize = crate::entrypoint::MAX_ENTRYPOINT_ARGUMENT_WORDS;
/// Register carrying the argument table address, then the completed result table address.
pub const CALL_ARGUMENT_BASE_REGISTER_V1: usize = 10;
/// Register carrying the exact argument count, then the exact initialized result count.
pub const CALL_ARGUMENT_COUNT_REGISTER_V1: usize = 11;
/// Register carrying the caller-owned result table address on entry.
pub const CALL_RESULT_BASE_REGISTER_V1: usize = 12;
/// Register carrying the schema-derived exact result capacity on entry.
pub const CALL_RESULT_COUNT_REGISTER_V1: usize = 13;
/// Maximum admitted stack frame, bounded by the IVM stack region.
pub const MAX_CALL_FRAME_BYTES_V1: u32 = 4 * 1024 * 1024;

/// Authenticated callable root, fixed stack reservation, and complete value schemas.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "ivm_abi::call::EmbeddedCallableV1")]
pub struct EmbeddedCallableV1 {
    /// Instruction offset relative to the artifact executable stream.
    pub entry_pc: u64,
    /// Caller stack pointer minus the active frame's lowest address, aligned to 16 bytes.
    pub frame_bytes: u32,
    /// Exact parameter types, as a flat preorder forest in declaration order.
    pub arguments: CallSchemaV1,
    /// Exactly one complete result type, including Unit for a void function.
    pub results: CallSchemaV1,
}
impl EmbeddedCallableV1 {
    /// Check local layout bounds; admission additionally checks roots and public schemas.
    #[must_use]
    pub fn validate(&self) -> bool {
        if !self.entry_pc.is_multiple_of(4)
            || !self.frame_bytes.is_multiple_of(16)
            || self.frame_bytes > MAX_CALL_FRAME_BYTES_V1
        {
            return false;
        }
        let Some(arguments) = self.arguments.analyze() else {
            return false;
        };
        let Some(results) = self.results.analyze() else {
            return false;
        };
        arguments.word_count() <= MAX_CALL_WORDS_V1
            && results.root_count() == 1
            && (1..=MAX_CALL_WORDS_V1).contains(&results.word_count())
    }
    /// Derive the exact argument-table count from its authoritative schema.
    #[must_use]
    pub fn argument_word_count(&self) -> Option<usize> {
        self.arguments.word_count()
    }
    /// Derive the exact result-table capacity/count from its authoritative schema.
    #[must_use]
    pub fn result_word_count(&self) -> Option<usize> {
        self.results.word_count()
    }
}

#[cfg(test)]
mod tests;
