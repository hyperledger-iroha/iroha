//! Canonical V1 function calls through caller-owned word tables.
//!
//! These authenticated descriptors are shared by compiler emission, artifact admission, runtime
//! frame checks, and proof constraints. Syscall register conventions are independent of calls.

use norito::{Decode, Encode};

use crate::{entrypoint::EntrypointValueWordKindV1, pointer_abi::PointerType};

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

/// Exact runtime representation and privacy of one initialized table slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "ivm_abi::call::CallWordV1")]
pub enum CallWordV1 {
    /// The public scalar zero.
    Unit,
    /// A public scalar zero or one.
    Bool,
    /// A public enum-local error code; the nominal schema validates the exact variant.
    Error,
    /// A public canonical pointer-ABI envelope of this registered type ID.
    Pointer(u16),
    /// An active-only Option or Result heap handle.
    Sum,
    /// A bounded List heap handle.
    List,
    /// A canonical state cursor frame.
    StateCursor,
    /// A compiler-owned public durable-state root handle.
    StateRoot,
    /// A private numeric handle of this registered numeric pointer type ID.
    SecretNumeric(u16),
}
impl CallWordV1 {
    /// Whether the entire slot must carry the private memory tag.
    #[must_use]
    pub const fn is_private(self) -> bool {
        matches!(self, Self::SecretNumeric(_))
    }
    /// Whether this role names a canonical V1 representation.
    #[must_use]
    pub fn validate(self) -> bool {
        match self {
            Self::Pointer(id) => PointerType::from_u16(id).is_some(),
            Self::SecretNumeric(id) => matches!(
                PointerType::from_u16(id),
                Some(PointerType::Int | PointerType::Decimal | PointerType::Quantity)
            ),
            _ => true,
        }
    }
    /// Flatten a public boundary word into the corresponding call role.
    #[must_use]
    pub fn from_entrypoint_word(word: EntrypointValueWordKindV1) -> Self {
        use crate::entrypoint::EntrypointValueKindV1 as Leaf;
        match word {
            EntrypointValueWordKindV1::Unit => Self::Unit,
            EntrypointValueWordKindV1::Error => Self::Error,
            EntrypointValueWordKindV1::Sum => Self::Sum,
            EntrypointValueWordKindV1::List => Self::List,
            EntrypointValueWordKindV1::StateCursor(_) => Self::StateCursor,
            EntrypointValueWordKindV1::Leaf(Leaf::Bool) => Self::Bool,
            EntrypointValueWordKindV1::Leaf(leaf) => Self::Pointer(match leaf {
                Leaf::Int => PointerType::Int,
                Leaf::Decimal => PointerType::Decimal,
                Leaf::Quantity => PointerType::Quantity,
                Leaf::String | Leaf::Blob => PointerType::Blob,
                Leaf::Json => PointerType::Json,
                Leaf::Name => PointerType::Name,
                Leaf::AccountId => PointerType::AccountId,
                Leaf::AssetDefinitionId => PointerType::AssetDefinitionId,
                Leaf::AssetId => PointerType::AssetId,
                Leaf::DomainId => PointerType::DomainId,
                Leaf::NftId => PointerType::NftId,
                Leaf::DataSpaceId => PointerType::DataSpaceId,
                Leaf::Bool => unreachable!("boolean role handled above"),
            } as u16),
        }
    }
}

/// Authenticated callable root, fixed stack reservation, and exact table slot roles.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "ivm_abi::call::EmbeddedCallableV1")]
pub struct EmbeddedCallableV1 {
    /// Instruction offset relative to the artifact executable stream.
    pub entry_pc: u64,
    /// Caller stack pointer minus the active frame's lowest address, aligned to 16 bytes.
    pub frame_bytes: u32,
    /// Exact argument count and the required role of every initialized argument slot.
    pub argument_words: Vec<CallWordV1>,
    /// Exact result capacity/count and the required role of every initialized result slot.
    pub result_words: Vec<CallWordV1>,
}
impl EmbeddedCallableV1 {
    /// Check local layout bounds; admission additionally checks roots and public schemas.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.entry_pc.is_multiple_of(4)
            && self.frame_bytes.is_multiple_of(16)
            && self.frame_bytes <= MAX_CALL_FRAME_BYTES_V1
            && self.argument_words.len() <= MAX_CALL_WORDS_V1
            && !self.result_words.is_empty()
            && self.result_words.len() <= MAX_CALL_WORDS_V1
            && self
                .argument_words
                .iter()
                .chain(&self.result_words)
                .all(|word| word.validate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callable_layout_rejects_invalid_roles_and_unbounded_tables() {
        let mut descriptor = EmbeddedCallableV1 {
            entry_pc: 4,
            frame_bytes: 16,
            argument_words: vec![CallWordV1::Bool; MAX_CALL_WORDS_V1],
            result_words: vec![CallWordV1::Unit],
        };
        assert!(descriptor.validate());
        descriptor.argument_words.push(CallWordV1::Bool);
        assert!(!descriptor.validate());
        descriptor.argument_words = vec![CallWordV1::Pointer(u16::MAX)];
        assert!(!descriptor.validate());
        descriptor.argument_words = vec![CallWordV1::SecretNumeric(PointerType::Blob as u16)];
        assert!(!descriptor.validate());
        descriptor.argument_words.clear();
        descriptor.frame_bytes = 15;
        assert!(!descriptor.validate());
    }
    #[test]
    fn public_boundary_roles_preserve_privacy_and_representation() {
        assert_eq!(
            CallWordV1::from_entrypoint_word(EntrypointValueWordKindV1::Unit),
            CallWordV1::Unit
        );
        assert_eq!(
            CallWordV1::from_entrypoint_word(EntrypointValueWordKindV1::Sum),
            CallWordV1::Sum
        );
        assert_eq!(
            CallWordV1::from_entrypoint_word(EntrypointValueWordKindV1::Leaf(
                crate::entrypoint::EntrypointValueKindV1::Int
            )),
            CallWordV1::Pointer(PointerType::Int as u16)
        );
        assert!(!CallWordV1::Bool.is_private());
        assert!(CallWordV1::SecretNumeric(PointerType::Int as u16).is_private());
        assert!(CallWordV1::SecretNumeric(PointerType::Int as u16).validate());
        assert!(!CallWordV1::SecretNumeric(PointerType::Blob as u16).validate());
    }
}
