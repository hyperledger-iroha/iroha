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
/// Byte offset of the first aligned word in a decoded state-value table.
pub const DECODED_STATE_VALUE_TABLE_OFFSET: i16 = 8;
/// Width of one decoded state-value word.
pub const DECODED_STATE_VALUE_WORD_BYTES: i16 = 8;

/// Canonical representation of one scalar leaf in a durable aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StateValueKindV1 {
    /// Signed 64-bit integer.
    Int,
    /// Unsigned 128-bit numeric value.
    U128,
    /// Amount numeric value.
    Amount,
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
        !matches!(self, Self::Int | Self::Bool)
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
    /// Optional value. One tag word precedes the child's flattened words.
    Option,
    /// Result value. One tag word precedes the ok and error child words.
    Result,
    /// Scalar or pointer leaf consuming one VM word.
    Leaf(StateValueKindV1),
}

impl StateValueNodeV1 {
    fn child_count(&self) -> usize {
        match self {
            Self::Struct { fields, .. } => fields.len(),
            Self::Tuple { arity } => usize::from(*arity),
            Self::Option => 1,
            Self::Result => 2,
            Self::Leaf(_) => 0,
        }
    }
}

/// Compiler-owned schema for one aggregate durable-state type.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StateValueSchemaV1 {
    /// Preorder aggregate layout.
    pub nodes: Vec<StateValueNodeV1>,
}

impl StateValueSchemaV1 {
    /// Validate tree shape and V1 node/word/depth limits.
    #[must_use]
    pub fn validate(&self) -> bool {
        if self.nodes.is_empty() || self.nodes.len() > MAX_STATE_VALUE_NODES {
            return false;
        }
        let mut pending = vec![1_usize];
        let mut words = 0_usize;
        for node in &self.nodes {
            while pending.last() == Some(&0) {
                pending.pop();
            }
            let Some(remaining) = pending.last_mut() else {
                return false;
            };
            *remaining -= 1;
            match node {
                StateValueNodeV1::Struct { name, fields } => {
                    if name.is_empty()
                        || fields.iter().any(|field| field.is_empty())
                        || fields
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != fields.len()
                    {
                        return false;
                    }
                }
                StateValueNodeV1::Option | StateValueNodeV1::Result => {
                    words = words.saturating_add(1);
                }
                StateValueNodeV1::Leaf(_) => words = words.saturating_add(1),
                StateValueNodeV1::Tuple { .. } => {}
            }
            if words > MAX_STATE_VALUE_WORDS {
                return false;
            }
            let children = node.child_count();
            if children != 0 {
                if pending.len() >= MAX_STATE_VALUE_NODES {
                    return false;
                }
                pending.push(children);
            }
        }
        while pending.last() == Some(&0) {
            pending.pop();
        }
        pending.is_empty()
    }

    /// Return the flattened VM word kinds in deterministic preorder.
    pub fn word_kinds(&self) -> Option<Vec<StateValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut words = Vec::new();
        for node in &self.nodes {
            match node {
                StateValueNodeV1::Option | StateValueNodeV1::Result => {
                    words.push(StateValueWordKindV1::Tag)
                }
                StateValueNodeV1::Leaf(kind) => words.push(StateValueWordKindV1::Leaf(*kind)),
                StateValueNodeV1::Struct { .. } | StateValueNodeV1::Tuple { .. } => {}
            }
        }
        Some(words)
    }

    /// Validate the atom variants and the canonical active/inactive shape of a record.
    ///
    /// `Option` and `Result` reserve words for every payload so the compiler can use a
    /// fixed-width table.  Inactive payloads must therefore be the unique all-zero/null
    /// representation; otherwise the record could smuggle data through a value which is
    /// semantically absent.  Conversely, pointer leaves in an active payload may never be
    /// null.  The VM performs pointer-type and payload validation after this structural
    /// check.
    #[must_use]
    pub fn validate_atoms(&self, atoms: &[StateValueAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }

        fn visit(
            nodes: &[StateValueNodeV1],
            atoms: &[StateValueAtomV1],
            node_index: &mut usize,
            atom_index: &mut usize,
            active: bool,
        ) -> bool {
            let Some(node) = nodes.get(*node_index) else {
                return false;
            };
            *node_index = node_index.saturating_add(1);

            match node {
                StateValueNodeV1::Struct { fields, .. } => {
                    for _ in fields {
                        if !visit(nodes, atoms, node_index, atom_index, active) {
                            return false;
                        }
                    }
                    true
                }
                StateValueNodeV1::Tuple { arity } => {
                    for _ in 0..*arity {
                        if !visit(nodes, atoms, node_index, atom_index, active) {
                            return false;
                        }
                    }
                    true
                }
                StateValueNodeV1::Option => {
                    let Some(StateValueAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    if !active && *tag {
                        return false;
                    }
                    visit(nodes, atoms, node_index, atom_index, active && *tag)
                }
                StateValueNodeV1::Result => {
                    let Some(StateValueAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    if !active && *tag {
                        return false;
                    }
                    visit(nodes, atoms, node_index, atom_index, active && *tag)
                        && visit(nodes, atoms, node_index, atom_index, active && !*tag)
                }
                StateValueNodeV1::Leaf(kind) => {
                    let Some(atom) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    match (kind, atom, active) {
                        (StateValueKindV1::Int, StateValueAtomV1::Int(_), true)
                        | (StateValueKindV1::Int, StateValueAtomV1::Int(0), false)
                        | (StateValueKindV1::Bool, StateValueAtomV1::Bool(_), true)
                        | (StateValueKindV1::Bool, StateValueAtomV1::Bool(false), false) => true,
                        (kind, StateValueAtomV1::Pointer(_), true) if kind.is_pointer() => true,
                        (kind, StateValueAtomV1::Null, false) if kind.is_pointer() => true,
                        _ => false,
                    }
                }
            }
        }

        let mut node_index = 0;
        let mut atom_index = 0;
        visit(&self.nodes, atoms, &mut node_index, &mut atom_index, true)
            && node_index == self.nodes.len()
            && atom_index == atoms.len()
    }

    /// Canonicalize inactive sum payloads while validating active atom variants.
    ///
    /// Source constructors carry a typed placeholder for V1 inference. That
    /// placeholder is not part of the semantic value and must not affect its
    /// durable bytes. The encoder uses this method to replace every inactive
    /// scalar/pointer/tag with its unique zero/null form. Active null pointers
    /// and atom-kind mismatches remain errors.
    pub fn canonicalize_atoms(&self, atoms: &mut [StateValueAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }

        fn visit(
            nodes: &[StateValueNodeV1],
            atoms: &mut [StateValueAtomV1],
            node_index: &mut usize,
            atom_index: &mut usize,
            active: bool,
        ) -> bool {
            let Some(node) = nodes.get(*node_index) else {
                return false;
            };
            *node_index = node_index.saturating_add(1);
            match node {
                StateValueNodeV1::Struct { fields, .. } => {
                    for _ in fields {
                        if !visit(nodes, atoms, node_index, atom_index, active) {
                            return false;
                        }
                    }
                    true
                }
                StateValueNodeV1::Tuple { arity } => {
                    for _ in 0..*arity {
                        if !visit(nodes, atoms, node_index, atom_index, active) {
                            return false;
                        }
                    }
                    true
                }
                StateValueNodeV1::Option => {
                    let Some(StateValueAtomV1::Tag(tag)) = atoms.get_mut(*atom_index) else {
                        return false;
                    };
                    let child_active = active && *tag;
                    if !active {
                        *tag = false;
                    }
                    *atom_index = atom_index.saturating_add(1);
                    visit(nodes, atoms, node_index, atom_index, child_active)
                }
                StateValueNodeV1::Result => {
                    let Some(StateValueAtomV1::Tag(tag)) = atoms.get_mut(*atom_index) else {
                        return false;
                    };
                    let ok_active = active && *tag;
                    let err_active = active && !*tag;
                    if !active {
                        *tag = false;
                    }
                    *atom_index = atom_index.saturating_add(1);
                    visit(nodes, atoms, node_index, atom_index, ok_active)
                        && visit(nodes, atoms, node_index, atom_index, err_active)
                }
                StateValueNodeV1::Leaf(kind) => {
                    let Some(atom) = atoms.get_mut(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    match (kind, atom, active) {
                        (StateValueKindV1::Int, StateValueAtomV1::Int(_), true)
                        | (StateValueKindV1::Bool, StateValueAtomV1::Bool(_), true) => true,
                        (StateValueKindV1::Int, atom @ StateValueAtomV1::Int(_), false) => {
                            *atom = StateValueAtomV1::Int(0);
                            true
                        }
                        (StateValueKindV1::Bool, atom @ StateValueAtomV1::Bool(_), false) => {
                            *atom = StateValueAtomV1::Bool(false);
                            true
                        }
                        (kind, StateValueAtomV1::Pointer(_), true) if kind.is_pointer() => true,
                        (
                            kind,
                            atom @ (StateValueAtomV1::Pointer(_) | StateValueAtomV1::Null),
                            false,
                        ) if kind.is_pointer() => {
                            *atom = StateValueAtomV1::Null;
                            true
                        }
                        _ => false,
                    }
                }
            }
        }

        let mut node_index = 0;
        let mut atom_index = 0;
        visit(&self.nodes, atoms, &mut node_index, &mut atom_index, true)
            && node_index == self.nodes.len()
            && atom_index == atoms.len()
    }
}

/// Flattened word role derived from a validated schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateValueWordKindV1 {
    /// Option/Result discriminant restricted to zero or one.
    Tag,
    /// Scalar or pointer leaf.
    Leaf(StateValueKindV1),
}

/// Canonical stored representation of one flattened aggregate word.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum StateValueAtomV1 {
    /// Option/Result tag.
    Tag(bool),
    /// Signed integer bits.
    Int(i64),
    /// Boolean value.
    Bool(bool),
    /// Complete validated pointer-ABI TLV envelope.
    Pointer(Vec<u8>),
    /// Null pointer placeholder, used by inactive sum branches.
    Null,
}

/// Canonical Norito value stored under one aggregate durable-state key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct StateValueRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Flattened atoms matching the schema's word order.
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
            atoms: vec![StateValueAtomV1::Int(9), StateValueAtomV1::Bool(true)],
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
    fn sum_records_require_canonical_inactive_branches() {
        let option = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Option,
                StateValueNodeV1::Leaf(StateValueKindV1::Int),
            ],
        };
        assert!(option.validate_atoms(&[StateValueAtomV1::Tag(false), StateValueAtomV1::Int(0),]));
        assert!(option.validate_atoms(&[StateValueAtomV1::Tag(true), StateValueAtomV1::Int(9),]));
        assert!(!option.validate_atoms(&[StateValueAtomV1::Tag(false), StateValueAtomV1::Int(9),]));

        let result = StateValueSchemaV1 {
            nodes: vec![
                StateValueNodeV1::Result,
                StateValueNodeV1::Leaf(StateValueKindV1::String),
                StateValueNodeV1::Leaf(StateValueKindV1::Bool),
            ],
        };
        assert!(result.validate_atoms(&[
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Null,
            StateValueAtomV1::Bool(true),
        ]));
        assert!(!result.validate_atoms(&[
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Pointer(vec![1]),
            StateValueAtomV1::Bool(true),
        ]));
        assert!(!result.validate_atoms(&[
            StateValueAtomV1::Tag(true),
            StateValueAtomV1::Null,
            StateValueAtomV1::Bool(false),
        ]));

        let mut noncanonical = vec![
            StateValueAtomV1::Tag(false),
            StateValueAtomV1::Pointer(vec![1]),
            StateValueAtomV1::Bool(true),
        ];
        assert!(result.canonicalize_atoms(&mut noncanonical));
        assert_eq!(
            noncanonical,
            vec![
                StateValueAtomV1::Tag(false),
                StateValueAtomV1::Null,
                StateValueAtomV1::Bool(true),
            ]
        );
        assert!(result.validate_atoms(&noncanonical));
    }
}
