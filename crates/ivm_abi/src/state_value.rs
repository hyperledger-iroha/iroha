//! Canonical schemas and records for aggregate Kotodama durable values.
//!
//! Aggregate state is stored under one durable key.  The compiler emits a
//! preorder schema, while the host converts the VM's flattened word table into
//! a canonical Norito record bound to that schema.

use iroha_crypto::Hash;
use norito::{Decode, Encode};

/// Domain separator for hashes binding stored records to exact state schemas.
pub const STATE_VALUE_SCHEMA_HASH_DOMAIN_V1: &[u8] = b"KOTODAMA_STATE_VALUE_SCHEMA_V1\0";

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
    /// Canonical Kotodama signed 4096-bit integer pointer.
    Int,
    /// Canonical exact bounded decimal pointer.
    Decimal,
    /// Canonical nominal non-negative quantity pointer.
    Quantity,
    /// Boolean scalar restricted to zero or one in the VM word table.
    Bool,
    /// UTF-8 source string carried in a Blob pointer.
    String,
    /// Canonical JSON pointer.
    Json,
    /// Source-level `bytes`, represented by a Blob pointer in the ABI.
    Bytes,
    /// Universal account identifier.
    AccountId,
    /// Asset-definition identifier.
    AssetDefinitionId,
    /// Asset identifier.
    AssetId,
    /// Domain identifier.
    DomainId,
    /// NFT identifier.
    NftId,
    /// Validated Iroha name.
    Name,
    /// Dataspace identifier.
    DataSpaceId,
    /// AXT descriptor.
    AxtDescriptor,
    /// AXT asset handle.
    AssetHandle,
    /// AXT proof blob.
    ProofBlob,
    /// Soracloud host request envelope.
    SoracloudRequest,
    /// Soracloud host response envelope.
    SoracloudResponse,
}

impl StateValueKindV1 {
    /// Return whether the value occupies a pointer word rather than an inline scalar.
    #[must_use]
    pub const fn is_pointer(self) -> bool {
        !matches!(self, Self::Bool)
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
    Struct {
        /// Source type name, included in schema identity.
        name: String,
        /// Ordered source field names.
        fields: Vec<String>,
    },
    /// Positional product type. Children immediately follow in index order.
    Tuple {
        /// Number of tuple children.
        arity: u16,
    },
    /// Optional value carried by one active-only compiler-owned sum handle.
    Option,
    /// Result value carried by one active-only compiler-owned sum handle.
    Result,
    /// Bounded contiguous list represented by one schema-bound sequence pointer.
    List {
        /// Exact recursive element schema.
        element: Box<StateValueSchemaV1>,
        /// Compile-time capacity in the inclusive range 1 through 64.
        capacity: u8,
    },
    /// Scalar or pointer leaf consuming one VM word.
    Leaf(StateValueKindV1),
}

/// Compiler-owned schema for one aggregate durable-state type.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
    Tag(bool),
    /// Boolean value.
    Bool(bool),
    /// Complete validated pointer-ABI TLV envelope.
    Pointer(Vec<u8>),
    /// Canonical bounded sequence; each item is one active-only element atom stream.
    List(Vec<Vec<StateValueAtomV1>>),
}

/// Canonical Norito value stored under one aggregate durable-state key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StateValueRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Active-only atoms in schema preorder; sum tags select exactly one payload.
    pub atoms: Vec<StateValueAtomV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_and_record_roundtrip_deterministically() {
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
