//! Canonical schemas and records for aggregate Kotodama durable values.
//!
//! Aggregate state is stored under one durable key.  The compiler emits a
//! preorder schema, while the host converts the VM's flattened word table into
//! a canonical Norito record bound to that schema.

use iroha_crypto::Hash;
use norito::{Decode, Encode};

use crate::pointer_abi::PointerType;

/// Domain separator for hashes binding stored records to exact state schemas.
pub const STATE_VALUE_SCHEMA_HASH_DOMAIN_V1: &[u8] = b"KOTODAMA_STATE_VALUE_SCHEMA_V1\0";
/// Nominal Norito schema name for compiler-emitted durable-value schemas.
pub const STATE_VALUE_SCHEMA_NAME_V1: &str = "iroha.kotodama.StateValueSchemaV1";
/// Nominal Norito schema name for canonical durable-value records.
pub const STATE_VALUE_RECORD_NAME_V1: &str = "iroha.kotodama.StateValueRecordV1";

/// Hash an exact encoded V1 schema with its dedicated domain separator.
#[must_use]
pub fn state_value_schema_hash_v1(schema_payload: &[u8]) -> [u8; 32] {
    let mut material =
        Vec::with_capacity(STATE_VALUE_SCHEMA_HASH_DOMAIN_V1.len() + schema_payload.len());
    material.extend_from_slice(STATE_VALUE_SCHEMA_HASH_DOMAIN_V1);
    material.extend_from_slice(schema_payload);
    Hash::new(&material).into()
}

/// Maximum schema nodes accepted by the V1 aggregate-state codec.
pub const MAX_STATE_VALUE_NODES: usize = 256;
/// Maximum flattened VM words accepted by the V1 aggregate-state codec.
pub const MAX_STATE_VALUE_WORDS: usize = 256;
/// Maximum encoded schema size accepted by the V1 aggregate-state codec.
pub const MAX_STATE_VALUE_SCHEMA_BYTES: usize = 64 * 1024;
/// Maximum encoded durable aggregate value accepted by the V1 codec.
pub const MAX_STATE_VALUE_RECORD_BYTES: usize = 1024 * 1024;
/// Minimum capacity accepted for a durable `List<T, N>`.
pub const MIN_STATE_VALUE_LIST_CAPACITY_V1: u8 = 1;
/// Maximum capacity accepted for a durable `List<T, N>`.
pub const MAX_STATE_VALUE_LIST_CAPACITY_V1: u8 = 64;
/// Byte offset of the first aligned word in a decoded state-value table.
pub const DECODED_STATE_VALUE_TABLE_OFFSET: i16 = 8;
/// Width of one decoded state-value word.
pub const DECODED_STATE_VALUE_WORD_BYTES: i16 = 8;

/// Canonical representation of one scalar leaf in a durable aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StateValueKindV1 {
    /// Canonical Kotodama signed 512-bit integer pointer.
    #[codec(index = 0)]
    Int,
    /// Canonical exact bounded decimal pointer.
    #[codec(index = 1)]
    Decimal,
    /// Canonical nominal non-negative quantity pointer.
    #[codec(index = 2)]
    Quantity,
    /// Boolean scalar restricted to zero or one in the VM word table.
    #[codec(index = 3)]
    Bool,
    /// UTF-8 source string carried in a Blob pointer.
    #[codec(index = 4)]
    String,
    /// Canonical JSON pointer.
    #[codec(index = 5)]
    Json,
    /// Source-level `bytes`, represented by a Blob pointer in the ABI.
    #[codec(index = 6)]
    Bytes,
    /// Universal account identifier.
    #[codec(index = 7)]
    AccountId,
    /// Asset-definition identifier.
    #[codec(index = 8)]
    AssetDefinitionId,
    /// Asset identifier.
    #[codec(index = 9)]
    AssetId,
    /// Domain identifier.
    #[codec(index = 10)]
    DomainId,
    /// NFT identifier.
    #[codec(index = 11)]
    NftId,
    /// Validated Iroha name.
    #[codec(index = 12)]
    Name,
    /// Dataspace identifier.
    #[codec(index = 13)]
    DataSpaceId,
    /// AXT descriptor.
    #[codec(index = 14)]
    AxtDescriptor,
    /// AXT asset handle.
    #[codec(index = 15)]
    AssetHandle,
    /// AXT proof blob.
    #[codec(index = 16)]
    ProofBlob,
    /// Soracloud host request envelope.
    #[codec(index = 17)]
    SoracloudRequest,
    /// Soracloud host response envelope.
    #[codec(index = 18)]
    SoracloudResponse,
}

impl StateValueKindV1 {
    /// Return the stable Norito enum discriminant used by ABI V1.
    #[must_use]
    pub const fn tag(self) -> u32 {
        match self {
            Self::Int => 0,
            Self::Decimal => 1,
            Self::Quantity => 2,
            Self::Bool => 3,
            Self::String => 4,
            Self::Json => 5,
            Self::Bytes => 6,
            Self::AccountId => 7,
            Self::AssetDefinitionId => 8,
            Self::AssetId => 9,
            Self::DomainId => 10,
            Self::NftId => 11,
            Self::Name => 12,
            Self::DataSpaceId => 13,
            Self::AxtDescriptor => 14,
            Self::AssetHandle => 15,
            Self::ProofBlob => 16,
            Self::SoracloudRequest => 17,
            Self::SoracloudResponse => 18,
        }
    }

    /// Return the canonical persisted pointer-ABI type for this leaf, or `None`
    /// for inline booleans.
    ///
    /// Storage-boundary encoders may accept additional transient carriers. In
    /// particular, source-level `bytes` accepts `NoritoBytes` but canonicalizes
    /// the stored atom to the `Blob` type returned here.
    #[must_use]
    pub const fn pointer_type(self) -> Option<PointerType> {
        Some(match self {
            Self::Bool => return None,
            Self::Int => PointerType::Int,
            Self::Decimal => PointerType::Decimal,
            Self::Quantity => PointerType::Quantity,
            Self::String | Self::Bytes => PointerType::Blob,
            Self::Json => PointerType::Json,
            Self::AccountId => PointerType::AccountId,
            Self::AssetDefinitionId => PointerType::AssetDefinitionId,
            Self::AssetId => PointerType::AssetId,
            Self::DomainId => PointerType::DomainId,
            Self::NftId => PointerType::NftId,
            Self::Name => PointerType::Name,
            Self::DataSpaceId => PointerType::DataSpaceId,
            Self::AxtDescriptor => PointerType::AxtDescriptor,
            Self::AssetHandle => PointerType::AssetHandle,
            Self::ProofBlob => PointerType::ProofBlob,
            Self::SoracloudRequest => PointerType::SoracloudRequest,
            Self::SoracloudResponse => PointerType::SoracloudResponse,
        })
    }

    /// Return whether the value occupies a pointer word rather than an inline scalar.
    #[must_use]
    pub const fn is_pointer(self) -> bool {
        self.pointer_type().is_some()
    }

    /// Return whether this leaf is a non-copyable resource handle.
    #[must_use]
    pub const fn is_resource_handle(self) -> bool {
        matches!(self, Self::AssetHandle)
    }
}

/// One preorder node in a compiler-emitted durable-value schema.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StateValueNodeV1 {
    /// Named product type. Children immediately follow in field order.
    #[codec(index = 0)]
    Struct {
        /// Source type name, included in schema identity.
        name: String,
        /// Ordered source field names.
        fields: Vec<String>,
    },
    /// Positional product type. Children immediately follow in index order.
    #[codec(index = 1)]
    Tuple {
        /// Number of tuple children.
        arity: u16,
    },
    /// Optional value carried by one active-only compiler-owned sum handle.
    #[codec(index = 2)]
    Option,
    /// Result value carried by one active-only compiler-owned sum handle.
    #[codec(index = 3)]
    Result,
    /// Bounded contiguous list represented by one schema-bound sequence pointer.
    #[codec(index = 4)]
    List {
        /// Exact recursive element schema.
        element: Box<StateValueSchemaV1>,
        /// Compile-time capacity in the inclusive range 1 through 64.
        capacity: u8,
    },
    /// Scalar or pointer leaf consuming one VM word.
    #[codec(index = 5)]
    Leaf(StateValueKindV1),
}

impl StateValueNodeV1 {
    /// Stable Norito discriminant for [`Self::Struct`].
    pub const STRUCT_TAG: u32 = 0;
    /// Stable Norito discriminant for [`Self::Tuple`].
    pub const TUPLE_TAG: u32 = 1;
    /// Stable Norito discriminant for [`Self::Option`].
    pub const OPTION_TAG: u32 = 2;
    /// Stable Norito discriminant for [`Self::Result`].
    pub const RESULT_TAG: u32 = 3;
    /// Stable Norito discriminant for [`Self::List`].
    pub const LIST_TAG: u32 = 4;
    /// Stable Norito discriminant for [`Self::Leaf`].
    pub const LEAF_TAG: u32 = 5;

    /// Return this node's stable Norito enum discriminant.
    #[must_use]
    pub const fn tag(&self) -> u32 {
        match self {
            Self::Struct { .. } => Self::STRUCT_TAG,
            Self::Tuple { .. } => Self::TUPLE_TAG,
            Self::Option => Self::OPTION_TAG,
            Self::Result => Self::RESULT_TAG,
            Self::List { .. } => Self::LIST_TAG,
            Self::Leaf(_) => Self::LEAF_TAG,
        }
    }
}

/// Compiler-owned schema for one aggregate durable-state type.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kotodama.StateValueSchemaV1")]
pub struct StateValueSchemaV1 {
    /// Preorder aggregate layout.
    pub nodes: Vec<StateValueNodeV1>,
}

impl StateValueSchemaV1 {
    fn analyze(&self) -> Option<StateValueAnalysisV1> {
        fn analyze_node(
            nodes: &[StateValueNodeV1],
            index: &mut usize,
            depth: usize,
        ) -> Option<StateValueAnalysisV1> {
            if depth > MAX_STATE_VALUE_NODES {
                return None;
            }
            let node = nodes.get(*index)?;
            *index = index.checked_add(1)?;
            let mut analysis = StateValueAnalysisV1 {
                node_count: 1,
                max_words: 0,
                depth,
                contains_resource_handle: false,
            };
            let mut merge_child = |child: StateValueAnalysisV1| -> Option<()> {
                analysis.node_count = analysis.node_count.checked_add(child.node_count)?;
                analysis.max_words = analysis.max_words.checked_add(child.max_words)?;
                analysis.depth = analysis.depth.max(child.depth);
                analysis.contains_resource_handle |= child.contains_resource_handle;
                Some(())
            };
            match node {
                StateValueNodeV1::Struct { name, fields } => {
                    if name.is_empty()
                        || fields.is_empty()
                        || fields.iter().any(|field| field.is_empty())
                        || fields
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != fields.len()
                    {
                        return None;
                    }
                    for _ in fields {
                        let child = analyze_node(nodes, index, depth.checked_add(1)?)?;
                        merge_child(child)?;
                    }
                }
                StateValueNodeV1::Tuple { arity } => {
                    if *arity < 2 {
                        return None;
                    }
                    for _ in 0..*arity {
                        let child = analyze_node(nodes, index, depth.checked_add(1)?)?;
                        merge_child(child)?;
                    }
                }
                StateValueNodeV1::Option => {
                    let child = analyze_node(nodes, index, depth.checked_add(1)?)?;
                    analysis.node_count = analysis.node_count.checked_add(child.node_count)?;
                    analysis.max_words = 1;
                    analysis.depth = analysis.depth.max(child.depth);
                    analysis.contains_resource_handle = child.contains_resource_handle;
                }
                StateValueNodeV1::Result => {
                    let ok = analyze_node(nodes, index, depth.checked_add(1)?)?;
                    let err = analyze_node(nodes, index, depth.checked_add(1)?)?;
                    analysis.node_count = analysis
                        .node_count
                        .checked_add(ok.node_count)?
                        .checked_add(err.node_count)?;
                    analysis.max_words = 1;
                    analysis.depth = analysis.depth.max(ok.depth).max(err.depth);
                    analysis.contains_resource_handle =
                        ok.contains_resource_handle || err.contains_resource_handle;
                }
                StateValueNodeV1::List { element, capacity } => {
                    if !(MIN_STATE_VALUE_LIST_CAPACITY_V1..=MAX_STATE_VALUE_LIST_CAPACITY_V1)
                        .contains(capacity)
                    {
                        return None;
                    }
                    let nested = element.analyze_at_depth(depth.checked_add(1)?)?;
                    if nested.contains_resource_handle {
                        return None;
                    }
                    analysis.node_count = analysis.node_count.checked_add(nested.node_count)?;
                    analysis.max_words = 1;
                    analysis.depth = analysis.depth.max(nested.depth);
                }
                StateValueNodeV1::Leaf(kind) => {
                    analysis.max_words = 1;
                    analysis.contains_resource_handle = kind.is_resource_handle();
                }
            }
            (analysis.node_count <= MAX_STATE_VALUE_NODES
                && analysis.max_words <= MAX_STATE_VALUE_WORDS)
                .then_some(analysis)
        }

        let mut index = 0;
        let analysis = analyze_node(&self.nodes, &mut index, 1)?;
        (index == self.nodes.len()).then_some(analysis)
    }

    fn analyze_at_depth(&self, depth: usize) -> Option<StateValueAnalysisV1> {
        let analysis = self.analyze()?;
        let adjusted_depth = depth.checked_add(analysis.depth.checked_sub(1)?)?;
        (adjusted_depth <= MAX_STATE_VALUE_NODES).then_some(StateValueAnalysisV1 {
            depth: adjusted_depth,
            ..analysis
        })
    }

    /// Validate tree shape, active-width bounds, and recursive list constraints.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.analyze().is_some()
    }

    /// Return the fixed VM words needed by a value of this type.
    ///
    /// Every `Option`, `Result`, and `List` consumes one compiler-owned handle.
    #[must_use]
    pub fn word_count(&self) -> Option<usize> {
        self.analyze().map(|analysis| analysis.max_words)
    }

    /// Return the flattened VM word kinds in deterministic preorder.
    pub fn word_kinds(&self) -> Option<Vec<StateValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut node_index = 0;
        let words = max_state_value_word_kinds(&self.nodes, &mut node_index)?;
        (node_index == self.nodes.len()).then_some(words)
    }

    /// Validate an active-only atom stream against this exact schema.
    #[must_use]
    pub fn validate_atoms(&self, atoms: &[StateValueAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }
        let mut node_index = 0;
        let mut atom_index = 0;
        walk_state_value_atoms(&self.nodes, atoms, &mut node_index, &mut atom_index, None)
            && node_index == self.nodes.len()
            && atom_index == atoms.len()
    }

    /// Return actual flattened VM word roles selected by this value.
    pub fn word_kinds_for_atoms(
        &self,
        atoms: &[StateValueAtomV1],
    ) -> Option<Vec<StateValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut node_index = 0;
        let mut atom_index = 0;
        let mut kinds = Vec::new();
        if !walk_state_value_atoms(
            &self.nodes,
            atoms,
            &mut node_index,
            &mut atom_index,
            Some(&mut kinds),
        ) || node_index != self.nodes.len()
            || atom_index != atoms.len()
            || kinds.len() > MAX_STATE_VALUE_WORDS
        {
            return None;
        }
        Some(kinds)
    }
}

#[derive(Clone, Copy)]
struct StateValueAnalysisV1 {
    node_count: usize,
    max_words: usize,
    depth: usize,
    contains_resource_handle: bool,
}

fn skip_state_value_node(nodes: &[StateValueNodeV1], node_index: &mut usize) -> bool {
    let Some(node) = nodes.get(*node_index) else {
        return false;
    };
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => fields
            .iter()
            .all(|_| skip_state_value_node(nodes, node_index)),
        StateValueNodeV1::Tuple { arity } => {
            (0..*arity).all(|_| skip_state_value_node(nodes, node_index))
        }
        StateValueNodeV1::Option => skip_state_value_node(nodes, node_index),
        StateValueNodeV1::Result => {
            skip_state_value_node(nodes, node_index) && skip_state_value_node(nodes, node_index)
        }
        StateValueNodeV1::List { .. } | StateValueNodeV1::Leaf(_) => true,
    }
}

fn max_state_value_word_kinds(
    nodes: &[StateValueNodeV1],
    node_index: &mut usize,
) -> Option<Vec<StateValueWordKindV1>> {
    let node = nodes.get(*node_index)?;
    *node_index = node_index.checked_add(1)?;
    let mut words = Vec::new();
    match node {
        StateValueNodeV1::Struct { fields, .. } => {
            for _ in fields {
                words.extend(max_state_value_word_kinds(nodes, node_index)?);
            }
        }
        StateValueNodeV1::Tuple { arity } => {
            for _ in 0..*arity {
                words.extend(max_state_value_word_kinds(nodes, node_index)?);
            }
        }
        StateValueNodeV1::Option => {
            words.push(StateValueWordKindV1::Sum);
            max_state_value_word_kinds(nodes, node_index)?;
        }
        StateValueNodeV1::Result => {
            words.push(StateValueWordKindV1::Sum);
            max_state_value_word_kinds(nodes, node_index)?;
            max_state_value_word_kinds(nodes, node_index)?;
        }
        StateValueNodeV1::List { .. } => words.push(StateValueWordKindV1::List),
        StateValueNodeV1::Leaf(kind) => words.push(StateValueWordKindV1::Leaf(*kind)),
    }
    Some(words)
}

fn walk_state_value_atoms(
    nodes: &[StateValueNodeV1],
    atoms: &[StateValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
    mut kinds: Option<&mut Vec<StateValueWordKindV1>>,
) -> bool {
    let Some(node) = nodes.get(*node_index) else {
        return false;
    };
    *node_index = node_index.saturating_add(1);
    match node {
        StateValueNodeV1::Struct { fields, .. } => fields.iter().all(|_| {
            walk_state_value_atoms(nodes, atoms, node_index, atom_index, kinds.as_deref_mut())
        }),
        StateValueNodeV1::Tuple { arity } => (0..*arity).all(|_| {
            walk_state_value_atoms(nodes, atoms, node_index, atom_index, kinds.as_deref_mut())
        }),
        StateValueNodeV1::Option => {
            let Some(StateValueAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                return false;
            };
            *atom_index = atom_index.saturating_add(1);
            if let Some(kinds) = kinds.as_deref_mut() {
                kinds.push(StateValueWordKindV1::Sum);
            }
            if *tag {
                walk_state_value_atoms(nodes, atoms, node_index, atom_index, None)
            } else {
                skip_state_value_node(nodes, node_index)
            }
        }
        StateValueNodeV1::Result => {
            let Some(StateValueAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                return false;
            };
            *atom_index = atom_index.saturating_add(1);
            if let Some(kinds) = kinds.as_deref_mut() {
                kinds.push(StateValueWordKindV1::Sum);
            }
            if *tag {
                walk_state_value_atoms(nodes, atoms, node_index, atom_index, None)
                    && skip_state_value_node(nodes, node_index)
            } else {
                skip_state_value_node(nodes, node_index)
                    && walk_state_value_atoms(nodes, atoms, node_index, atom_index, None)
            }
        }
        StateValueNodeV1::List { element, capacity } => {
            let Some(StateValueAtomV1::List(items)) = atoms.get(*atom_index) else {
                return false;
            };
            *atom_index = atom_index.saturating_add(1);
            if items.len() > usize::from(*capacity)
                || items.iter().any(|item| !element.validate_atoms(item))
            {
                return false;
            }
            if let Some(kinds) = kinds {
                kinds.push(StateValueWordKindV1::List);
            }
            true
        }
        StateValueNodeV1::Leaf(kind) => {
            let Some(atom) = atoms.get(*atom_index) else {
                return false;
            };
            *atom_index = atom_index.saturating_add(1);
            let valid = matches!(
                (kind, atom),
                (StateValueKindV1::Bool, StateValueAtomV1::Bool(_))
            ) || (kind.is_pointer() && matches!(atom, StateValueAtomV1::Pointer(_)));
            if valid && let Some(kinds) = kinds {
                kinds.push(StateValueWordKindV1::Leaf(*kind));
            }
            valid
        }
    }
}

/// Flattened word role derived from a validated schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateValueWordKindV1 {
    /// One active-only compiler-owned Option/Result handle.
    Sum,
    /// One schema-bound canonical list-sequence pointer.
    List,
    /// Scalar or pointer leaf.
    Leaf(StateValueKindV1),
}

/// Canonical stored representation of one flattened aggregate word.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StateValueAtomV1 {
    /// Option/Result tag.
    #[codec(index = 0)]
    Tag(bool),
    /// Boolean value.
    #[codec(index = 1)]
    Bool(bool),
    /// Complete validated pointer-ABI TLV envelope.
    #[codec(index = 2)]
    Pointer(Vec<u8>),
    /// Canonical bounded sequence; each item is one active-only element atom stream.
    #[codec(index = 3)]
    List(Vec<Vec<StateValueAtomV1>>),
}

impl StateValueAtomV1 {
    /// Stable Norito discriminant for [`Self::Tag`].
    pub const TAG_TAG: u32 = 0;
    /// Stable Norito discriminant for [`Self::Bool`].
    pub const BOOL_TAG: u32 = 1;
    /// Stable Norito discriminant for [`Self::Pointer`].
    pub const POINTER_TAG: u32 = 2;
    /// Stable Norito discriminant for [`Self::List`].
    pub const LIST_TAG: u32 = 3;

    /// Return this atom's stable Norito enum discriminant.
    #[must_use]
    pub const fn tag(&self) -> u32 {
        match self {
            Self::Tag(_) => Self::TAG_TAG,
            Self::Bool(_) => Self::BOOL_TAG,
            Self::Pointer(_) => Self::POINTER_TAG,
            Self::List(_) => Self::LIST_TAG,
        }
    }
}

/// Canonical Norito value stored under one aggregate durable-state key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kotodama.StateValueRecordV1")]
pub struct StateValueRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Active-only atoms in schema preorder; sum tags select exactly one payload.
    pub atoms: Vec<StateValueAtomV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_norito_discriminant<T: norito::codec::Encode>(value: &T, expected: u32) {
        let encoded = norito::codec::Encode::encode(value);
        assert!(encoded.len() >= 4, "enum encoding must contain a u32 tag");
        assert_eq!(
            u32::from_le_bytes(encoded[..4].try_into().expect("four-byte tag")),
            expected
        );
    }

    #[test]
    fn durable_state_enum_tags_match_the_pinned_wire_discriminants() {
        let kinds = [
            StateValueKindV1::Int,
            StateValueKindV1::Decimal,
            StateValueKindV1::Quantity,
            StateValueKindV1::Bool,
            StateValueKindV1::String,
            StateValueKindV1::Json,
            StateValueKindV1::Bytes,
            StateValueKindV1::AccountId,
            StateValueKindV1::AssetDefinitionId,
            StateValueKindV1::AssetId,
            StateValueKindV1::DomainId,
            StateValueKindV1::NftId,
            StateValueKindV1::Name,
            StateValueKindV1::DataSpaceId,
            StateValueKindV1::AxtDescriptor,
            StateValueKindV1::AssetHandle,
            StateValueKindV1::ProofBlob,
            StateValueKindV1::SoracloudRequest,
            StateValueKindV1::SoracloudResponse,
        ];
        let pointer_types = [
            Some(PointerType::Int),
            Some(PointerType::Decimal),
            Some(PointerType::Quantity),
            None,
            Some(PointerType::Blob),
            Some(PointerType::Json),
            Some(PointerType::Blob),
            Some(PointerType::AccountId),
            Some(PointerType::AssetDefinitionId),
            Some(PointerType::AssetId),
            Some(PointerType::DomainId),
            Some(PointerType::NftId),
            Some(PointerType::Name),
            Some(PointerType::DataSpaceId),
            Some(PointerType::AxtDescriptor),
            Some(PointerType::AssetHandle),
            Some(PointerType::ProofBlob),
            Some(PointerType::SoracloudRequest),
            Some(PointerType::SoracloudResponse),
        ];
        for (expected, (kind, pointer_type)) in kinds.into_iter().zip(pointer_types).enumerate() {
            assert_eq!(kind.tag(), u32::try_from(expected).expect("kind tag"));
            assert_eq!(kind.pointer_type(), pointer_type);
            assert_norito_discriminant(&kind, kind.tag());
        }

        let int_schema = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
        };
        let nodes = [
            StateValueNodeV1::Struct {
                name: "S".into(),
                fields: vec!["field".into()],
            },
            StateValueNodeV1::Tuple { arity: 2 },
            StateValueNodeV1::Option,
            StateValueNodeV1::Result,
            StateValueNodeV1::List {
                element: Box::new(int_schema),
                capacity: 1,
            },
            StateValueNodeV1::Leaf(StateValueKindV1::Int),
        ];
        for (expected, node) in nodes.into_iter().enumerate() {
            assert_eq!(node.tag(), u32::try_from(expected).expect("node tag"));
            assert_norito_discriminant(&node, node.tag());
        }

        let atoms = [
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Bool(false),
            StateValueAtomV1::Pointer(Vec::new()),
            StateValueAtomV1::List(Vec::new()),
        ];
        for (expected, atom) in atoms.into_iter().enumerate() {
            assert_eq!(atom.tag(), u32::try_from(expected).expect("atom tag"));
            assert_norito_discriminant(&atom, atom.tag());
        }
    }

    #[test]
    fn schema_and_record_roundtrip_deterministically() {
        assert_eq!(
            <StateValueSchemaV1 as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(STATE_VALUE_SCHEMA_NAME_V1)
        );
        assert_eq!(
            <StateValueRecordV1 as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(STATE_VALUE_RECORD_NAME_V1)
        );
        let schema = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Struct {
                    name: "Pair".into(),
                    fields: vec!["count".into(), "ready".into()],
                },
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        };
        assert!(schema.validate());
        assert_eq!(schema.word_kinds().expect("words").len(), 2);
        let first = norito::to_bytes(&schema).expect("encode schema");
        let second = norito::to_bytes(&schema).expect("encode schema again");
        assert_eq!(first, second);
        assert_eq!(
            norito::decode_from_bytes::<StateValueSchemaV1>(&first).expect("decode schema"),
            schema
        );

        let record = StateValueRecordV1 {
            schema_hash: [7; 32],
            atoms: vec![
                StateValueAtomV1::Pointer(vec![1]),
                StateValueAtomV1::Bool(true),
            ],
        };
        let encoded = norito::to_bytes(&record).expect("encode record");
        assert_eq!(
            norito::decode_from_bytes::<StateValueRecordV1>(&encoded).expect("decode record"),
            record
        );
        assert_eq!(
            state_value_schema_hash_v1(&first),
            state_value_schema_hash_v1(&second)
        );
        assert_ne!(
            state_value_schema_hash_v1(&first),
            *Hash::new(&first).as_ref()
        );
    }

    #[test]
    fn malformed_schema_shape_is_rejected() {
        assert!(
            !StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::Tuple { arity: 2 }]
            }
            .validate()
        );
        assert!(
            !StateValueSchemaV1 {
                nodes: vec![
                    StateValueNodeV1::Leaf(StateValueKindV1::Int),
                    StateValueNodeV1::Leaf(StateValueKindV1::Bool),
                ]
            }
            .validate()
        );
    }

    #[test]
    fn sum_records_carry_only_the_active_payload() {
        let option = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        assert_eq!(option.word_count(), Some(1));
        assert_eq!(option.word_kinds(), Some(vec![StateValueWordKindV1::Sum]));
        assert!(option.validate_atoms(&[StateValueAtomV1::Tag(false)]));
        assert_eq!(
            option.word_kinds_for_atoms(&[StateValueAtomV1::Tag(false)]),
            Some(vec![StateValueWordKindV1::Sum])
        );
        assert!(option.validate_atoms(&[
            StateValueAtomV1::Tag(true),
            StateValueAtomV1::Pointer(vec![1]),
        ]));
        assert!(!option.validate_atoms(&[
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Pointer(vec![1]),
        ]));

        let result = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Result,
                StateValueNodeV1::Leaf(StateValueKindV1::String),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        };
        assert!(
            result.validate_atoms(&[StateValueAtomV1::Tag(false), StateValueAtomV1::Bool(true),])
        );
        assert!(!result.validate_atoms(&[
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Pointer(vec![1]),
            StateValueAtomV1::Bool(true),
        ]));
        assert!(result.validate_atoms(&[
            StateValueAtomV1::Tag(true),
            StateValueAtomV1::Pointer(vec![1]),
        ]));
    }

    #[test]
    fn nested_quantity_lists_roundtrip_and_reject_invalid_shapes() {
        let quantity = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Quantity)],
        };
        let inner = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(quantity),
                capacity: 2,
            }],
        };
        let nested = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(inner),
                capacity: 3,
            }],
        };
        assert!(nested.validate());
        assert_eq!(nested.word_count(), Some(1));
        let atoms = vec![StateValueAtomV1::List(vec![vec![StateValueAtomV1::List(
            vec![vec![StateValueAtomV1::Pointer(vec![1])]],
        )]])];
        assert!(nested.validate_atoms(&atoms));

        let record = StateValueRecordV1 {
            schema_hash: [9; 32],
            atoms,
        };
        let encoded = norito::to_bytes(&record).expect("encode nested list record");
        assert_eq!(
            norito::decode_from_bytes::<StateValueRecordV1>(&encoded)
                .expect("decode nested list record"),
            record
        );

        let overflow = vec![StateValueAtomV1::List(vec![
            vec![StateValueAtomV1::List(Vec::new())],
            vec![StateValueAtomV1::List(Vec::new())],
            vec![StateValueAtomV1::List(Vec::new())],
            vec![StateValueAtomV1::List(Vec::new())],
        ])];
        assert!(!nested.validate_atoms(&overflow));

        for capacity in [0, 65] {
            let invalid = StateValueSchemaV1 {
                nodes: vec![StateValueNodeV1::List {
                    element: Box::new(StateValueSchemaV1 {
                        nodes: vec![StateValueNodeV1::Leaf(StateValueKindV1::Int)],
                    }),
                    capacity,
                }],
            };
            assert!(!invalid.validate());
        }
    }

    #[test]
    fn lists_reject_resource_handles_recursively() {
        let resource = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::AssetHandle),
            ],
        };
        assert!(
            resource.validate(),
            "resource values remain valid outside lists"
        );
        let list = StateValueSchemaV1 {
            nodes: vec![StateValueNodeV1::List {
                element: Box::new(resource),
                capacity: 1,
            }],
        };
        assert!(!list.validate());
    }
}
