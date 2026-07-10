//! Canonical ABI records used to decode public contract arguments.
//!
//! Public tooling may accept JSON for ergonomics, but contract wrappers bind
//! that boundary value to this Norito schema and decode the complete argument
//! record in one host call. The decoded words are returned in declaration
//! order and are either canonical scalar bits or validated pointer-ABI addresses.

use iroha_crypto::Hash;
use norito::{Decode, Encode};

/// Domain separator for hashes binding public argument records to exact schemas.
pub const ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"KOTODAMA_ENTRYPOINT_ARGUMENT_SCHEMA_V1\0";

/// Hash an exact encoded V1 argument schema with its dedicated domain separator.
#[must_use]
pub fn entrypoint_argument_schema_hash_v1(schema_payload: &[u8]) -> [u8; 32] {
    let mut material =
        Vec::with_capacity(ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1.len() + schema_payload.len());
    material.extend_from_slice(ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1);
    material.extend_from_slice(schema_payload);
    Hash::new(&material).into()
}

/// Maximum number of public arguments carried by the ABI v1 register window.
pub const MAX_ENTRYPOINT_ARGUMENTS: usize = 13;
/// Maximum flattened words returned by one V1 argument-record decode.
pub const MAX_ENTRYPOINT_ARGUMENT_WORDS: usize = 256;
/// Maximum recursive type nodes in one V1 argument schema.
pub const MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES: usize = 256;
/// Maximum recursive aggregate depth in one V1 argument schema.
pub const MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH: usize = 256;
/// Maximum encoded compiler schema accepted by the V1 argument decoder.
pub const MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES: usize = 64 * 1024;
/// Maximum canonical Norito argument record accepted by one public V1 invocation.
pub const MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES: usize =
    iroha_data_model::transaction::executable::MAX_CONTRACT_ARGUMENT_RECORD_BYTES;

/// Byte offset of the first naturally aligned word in a decoded argument table.
///
/// Pointer-ABI envelopes have a seven-byte header; the result `Blob` therefore
/// reserves one payload byte so every following `u64` starts at an eight-byte
/// aligned address.
pub const DECODED_ARGUMENT_TABLE_OFFSET: i16 = 8;

/// Width of one decoded argument word in the returned table.
pub const DECODED_ARGUMENT_WORD_BYTES: i16 = 8;

/// Public argument representation requested by a contract wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum EntrypointArgumentKindV1 {
    /// Signed 64-bit integer returned directly in the table word.
    Int,
    /// Canonical unsigned-decimal JSON string returned as a scale-zero `Numeric` pointer.
    U128,
    /// Boolean scalar returned as exactly zero or one in the table word.
    Bool,
    /// UTF-8 string returned as a raw `Blob` pointer.
    String,
    /// Canonical `Numeric` returned as a `NoritoBytes` pointer.
    Numeric,
    /// Nested JSON value returned as a `Json` pointer.
    Json,
    /// Validated [`iroha_data_model::name::Name`] pointer.
    Name,
    /// Validated universal account identifier pointer.
    AccountId,
    /// Validated asset-definition identifier pointer.
    AssetDefinitionId,
    /// Validated full asset identifier pointer.
    AssetId,
    /// Validated domain identifier pointer.
    DomainId,
    /// Validated NFT identifier pointer.
    NftId,
    /// Validated dataspace identifier pointer.
    DataSpaceId,
    /// Hex-decoded bytes returned as a `Blob` pointer.
    Blob,
}

impl EntrypointArgumentKindV1 {
    /// Return whether the value occupies a validated pointer word.
    #[must_use]
    pub const fn is_pointer(self) -> bool {
        !matches!(self, Self::Int | Self::Bool)
    }
}

/// One preorder node in a public argument type schema.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum EntrypointArgumentTypeNodeV1 {
    /// Named product represented by an exact JSON object.
    Struct {
        /// Source type name, included in the schema identity.
        name: String,
        /// Ordered source field names. Child nodes immediately follow.
        fields: Vec<String>,
    },
    /// Positional product represented by an exact JSON array.
    Tuple {
        /// Number of child nodes which immediately follow.
        arity: u16,
    },
    /// Tagged optional payload represented by `{"some": value}` or `{"none": true}`.
    Option,
    /// Tagged result payload represented by `{"ok": value}` or `{"err": value}`.
    Result,
    /// Scalar or pointer leaf consuming one output word.
    Leaf(EntrypointArgumentKindV1),
}

impl EntrypointArgumentTypeNodeV1 {
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

/// Flat, compiler-emitted schema for one public parameter type.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EntrypointArgumentTypeV1 {
    /// Preorder aggregate layout.
    pub nodes: Vec<EntrypointArgumentTypeNodeV1>,
}

impl EntrypointArgumentTypeV1 {
    /// Validate tree shape, identifiers, depth, and the flattened word bound.
    #[must_use]
    pub fn validate(&self) -> bool {
        if self.nodes.is_empty() || self.nodes.len() > MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES {
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
                EntrypointArgumentTypeNodeV1::Struct { name, fields } => {
                    if name.is_empty()
                        || fields.iter().any(String::is_empty)
                        || fields
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != fields.len()
                    {
                        return false;
                    }
                }
                EntrypointArgumentTypeNodeV1::Option
                | EntrypointArgumentTypeNodeV1::Result
                | EntrypointArgumentTypeNodeV1::Leaf(_) => {
                    words = words.saturating_add(1);
                }
                EntrypointArgumentTypeNodeV1::Tuple { .. } => {}
            }
            if words > MAX_ENTRYPOINT_ARGUMENT_WORDS {
                return false;
            }
            let children = node.child_count();
            if children != 0 {
                if pending.len() >= MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
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

    /// Return the fixed number of ABI words emitted for this type.
    #[must_use]
    pub fn word_count(&self) -> Option<usize> {
        self.validate().then(|| {
            self.nodes
                .iter()
                .filter(|node| {
                    matches!(
                        node,
                        EntrypointArgumentTypeNodeV1::Option
                            | EntrypointArgumentTypeNodeV1::Result
                            | EntrypointArgumentTypeNodeV1::Leaf(_)
                    )
                })
                .count()
        })
    }

    /// Return flattened ABI word roles in deterministic preorder.
    pub fn word_kinds(&self) -> Option<Vec<EntrypointArgumentWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut words = Vec::new();
        for node in &self.nodes {
            match node {
                EntrypointArgumentTypeNodeV1::Option | EntrypointArgumentTypeNodeV1::Result => {
                    words.push(EntrypointArgumentWordKindV1::Tag)
                }
                EntrypointArgumentTypeNodeV1::Leaf(kind) => {
                    words.push(EntrypointArgumentWordKindV1::Leaf(*kind))
                }
                EntrypointArgumentTypeNodeV1::Struct { .. }
                | EntrypointArgumentTypeNodeV1::Tuple { .. } => {}
            }
        }
        Some(words)
    }

    /// Render the canonical public Kotodama type name represented by this schema.
    ///
    /// Artifact verification compares this value with the independently embedded
    /// manifest parameter type, preventing a malicious CNTR section from changing
    /// how signed argument bytes are interpreted while retaining the same field name.
    pub fn canonical_type_name(&self) -> Option<String> {
        if !self.validate() {
            return None;
        }

        fn render(nodes: &[EntrypointArgumentTypeNodeV1], index: &mut usize) -> Option<String> {
            let node = nodes.get(*index)?;
            *index = index.checked_add(1)?;
            Some(match node {
                EntrypointArgumentTypeNodeV1::Struct { name, fields } => {
                    for _ in fields {
                        render(nodes, index)?;
                    }
                    format!("struct {name}")
                }
                EntrypointArgumentTypeNodeV1::Tuple { arity } => {
                    let mut items = Vec::with_capacity(usize::from(*arity));
                    for _ in 0..*arity {
                        items.push(render(nodes, index)?);
                    }
                    format!("({})", items.join(", "))
                }
                EntrypointArgumentTypeNodeV1::Option => {
                    format!("Option<{}>", render(nodes, index)?)
                }
                EntrypointArgumentTypeNodeV1::Result => {
                    let ok = render(nodes, index)?;
                    let err = render(nodes, index)?;
                    format!("Result<{ok}, {err}>")
                }
                EntrypointArgumentTypeNodeV1::Leaf(kind) => match kind {
                    EntrypointArgumentKindV1::Int => "i64".to_owned(),
                    EntrypointArgumentKindV1::U128 => "u128".to_owned(),
                    EntrypointArgumentKindV1::Bool => "bool".to_owned(),
                    EntrypointArgumentKindV1::String => "string".to_owned(),
                    EntrypointArgumentKindV1::Numeric => "Amount".to_owned(),
                    EntrypointArgumentKindV1::Json => "Json".to_owned(),
                    EntrypointArgumentKindV1::Name => "Name".to_owned(),
                    EntrypointArgumentKindV1::AccountId => "AccountId".to_owned(),
                    EntrypointArgumentKindV1::AssetDefinitionId => "AssetDefinitionId".to_owned(),
                    EntrypointArgumentKindV1::AssetId => "AssetId".to_owned(),
                    EntrypointArgumentKindV1::DomainId => "DomainId".to_owned(),
                    EntrypointArgumentKindV1::NftId => "NftId".to_owned(),
                    EntrypointArgumentKindV1::DataSpaceId => "DataSpaceId".to_owned(),
                    EntrypointArgumentKindV1::Blob => "bytes".to_owned(),
                },
            })
        }

        let mut index = 0;
        let name = render(&self.nodes, &mut index)?;
        (index == self.nodes.len()).then_some(name)
    }
}

/// Flattened word role derived from a validated public argument schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntrypointArgumentWordKindV1 {
    /// Option/Result discriminant restricted to zero or one.
    Tag,
    /// Scalar or pointer leaf.
    Leaf(EntrypointArgumentKindV1),
}

/// Canonical wire atom in a public argument record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum EntrypointArgumentAtomV1 {
    /// Option/Result tag.
    Tag(bool),
    /// Signed 64-bit scalar.
    Int(i64),
    /// Boolean scalar.
    Bool(bool),
    /// Complete pointer-ABI TLV envelope for a typed leaf.
    Pointer(Vec<u8>),
    /// Inactive pointer payload in an Option/Result branch.
    Null,
}

/// Schema-bound canonical Norito payload supplied to a public entrypoint wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EntrypointArgumentRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Flattened atoms in declaration and schema preorder.
    pub atoms: Vec<EntrypointArgumentAtomV1>,
}

/// One named field in a public entrypoint argument record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EntrypointArgumentFieldV1 {
    /// Source-level parameter name used as the boundary object key.
    pub name: String,
    /// Canonical output representation expected by the compiled implementation.
    pub ty: EntrypointArgumentTypeV1,
}

/// Compiler-emitted schema for one public entrypoint invocation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct EntrypointArgumentSchemaV1 {
    /// Fields in source declaration and ABI register order.
    pub fields: Vec<EntrypointArgumentFieldV1>,
}

impl EntrypointArgumentSchemaV1 {
    /// Validate the first-release register-window bound.
    pub fn validate(&self) -> bool {
        if self.fields.is_empty() || self.fields.len() > MAX_ENTRYPOINT_ARGUMENTS {
            return false;
        }
        let mut names = std::collections::BTreeSet::new();
        self.fields.iter().all(|field| {
            !field.name.is_empty() && names.insert(field.name.as_str()) && field.ty.validate()
        }) && self.word_count_unchecked() <= MAX_ENTRYPOINT_ARGUMENT_WORDS
    }

    /// Return the total fixed-width table word count for this schema.
    #[must_use]
    pub fn word_count(&self) -> Option<usize> {
        if !self.validate() {
            return None;
        }
        Some(self.word_count_unchecked())
    }

    fn word_count_unchecked(&self) -> usize {
        self.fields
            .iter()
            .map(|field| field.ty.word_count().unwrap_or(usize::MAX))
            .fold(0_usize, usize::saturating_add)
    }

    /// Return flattened ABI word roles across fields in declaration order.
    pub fn word_kinds(&self) -> Option<Vec<EntrypointArgumentWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut words = Vec::with_capacity(self.word_count_unchecked());
        for field in &self.fields {
            words.extend(field.ty.word_kinds()?);
        }
        Some(words)
    }

    /// Validate atom variants and canonical inactive Option/Result payloads.
    #[must_use]
    pub fn validate_atoms(&self, atoms: &[EntrypointArgumentAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }

        fn visit(
            nodes: &[EntrypointArgumentTypeNodeV1],
            atoms: &[EntrypointArgumentAtomV1],
            node_index: &mut usize,
            atom_index: &mut usize,
            active: bool,
        ) -> bool {
            let Some(node) = nodes.get(*node_index) else {
                return false;
            };
            *node_index = node_index.saturating_add(1);
            match node {
                EntrypointArgumentTypeNodeV1::Struct { fields, .. } => fields
                    .iter()
                    .all(|_| visit(nodes, atoms, node_index, atom_index, active)),
                EntrypointArgumentTypeNodeV1::Tuple { arity } => {
                    (0..*arity).all(|_| visit(nodes, atoms, node_index, atom_index, active))
                }
                EntrypointArgumentTypeNodeV1::Option => {
                    let Some(EntrypointArgumentAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    if !active && *tag {
                        return false;
                    }
                    visit(nodes, atoms, node_index, atom_index, active && *tag)
                }
                EntrypointArgumentTypeNodeV1::Result => {
                    let Some(EntrypointArgumentAtomV1::Tag(tag)) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    if !active && *tag {
                        return false;
                    }
                    visit(nodes, atoms, node_index, atom_index, active && *tag)
                        && visit(nodes, atoms, node_index, atom_index, active && !*tag)
                }
                EntrypointArgumentTypeNodeV1::Leaf(kind) => {
                    let Some(atom) = atoms.get(*atom_index) else {
                        return false;
                    };
                    *atom_index = atom_index.saturating_add(1);
                    match (kind, atom, active) {
                        (EntrypointArgumentKindV1::Int, EntrypointArgumentAtomV1::Int(_), true)
                        | (
                            EntrypointArgumentKindV1::Int,
                            EntrypointArgumentAtomV1::Int(0),
                            false,
                        )
                        | (
                            EntrypointArgumentKindV1::Bool,
                            EntrypointArgumentAtomV1::Bool(_),
                            true,
                        )
                        | (
                            EntrypointArgumentKindV1::Bool,
                            EntrypointArgumentAtomV1::Bool(false),
                            false,
                        ) => true,
                        (kind, EntrypointArgumentAtomV1::Pointer(_), true) if kind.is_pointer() => {
                            true
                        }
                        (kind, EntrypointArgumentAtomV1::Null, false) if kind.is_pointer() => true,
                        _ => false,
                    }
                }
            }
        }

        let mut atom_index = 0;
        for field in &self.fields {
            let mut node_index = 0;
            if !visit(
                &field.ty.nodes,
                atoms,
                &mut node_index,
                &mut atom_index,
                true,
            ) || node_index != field.ty.nodes.len()
            {
                return false;
            }
        }
        atom_index == atoms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(kind: EntrypointArgumentKindV1) -> EntrypointArgumentTypeV1 {
        EntrypointArgumentTypeV1 {
            nodes: vec![EntrypointArgumentTypeNodeV1::Leaf(kind)],
        }
    }

    #[test]
    fn schema_roundtrips_with_stable_field_order() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "ready".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::Bool),
                },
                EntrypointArgumentFieldV1 {
                    name: "memo".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::String),
                },
                EntrypointArgumentFieldV1 {
                    name: "nonce".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::U128),
                },
                EntrypointArgumentFieldV1 {
                    name: "amount".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::Numeric),
                },
                EntrypointArgumentFieldV1 {
                    name: "owner".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::AccountId),
                },
                EntrypointArgumentFieldV1 {
                    name: "asset".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::AssetId),
                },
                EntrypointArgumentFieldV1 {
                    name: "domain".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::DomainId),
                },
                EntrypointArgumentFieldV1 {
                    name: "dataspace".to_owned(),
                    ty: leaf(EntrypointArgumentKindV1::DataSpaceId),
                },
            ],
        };
        let encoded = norito::to_bytes(&schema).expect("encode argument schema");
        let decoded: EntrypointArgumentSchemaV1 =
            norito::decode_from_bytes(&encoded).expect("decode argument schema");
        assert_eq!(decoded, schema);
        assert!(decoded.validate());

        let schema_bytes = norito::to_bytes(&decoded).expect("encode schema hash fixture");
        assert_ne!(
            entrypoint_argument_schema_hash_v1(&schema_bytes),
            *Hash::new(&schema_bytes).as_ref()
        );
    }

    #[test]
    fn schema_rejects_empty_and_oversized_records() {
        assert!(!EntrypointArgumentSchemaV1 { fields: Vec::new() }.validate());
        let fields = (0..=MAX_ENTRYPOINT_ARGUMENTS)
            .map(|index| EntrypointArgumentFieldV1 {
                name: format!("p{index}"),
                ty: leaf(EntrypointArgumentKindV1::Int),
            })
            .collect();
        assert!(!EntrypointArgumentSchemaV1 { fields }.validate());

        let duplicate = EntrypointArgumentSchemaV1 {
            fields: vec![
                EntrypointArgumentFieldV1 {
                    name: "same".into(),
                    ty: leaf(EntrypointArgumentKindV1::Int),
                },
                EntrypointArgumentFieldV1 {
                    name: "same".into(),
                    ty: leaf(EntrypointArgumentKindV1::Bool),
                },
            ],
        };
        assert!(!duplicate.validate());
    }

    #[test]
    fn recursive_schema_counts_words_and_rejects_duplicate_nested_fields() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "request".into(),
                ty: EntrypointArgumentTypeV1 {
                    nodes: vec![
                        EntrypointArgumentTypeNodeV1::Struct {
                            name: "Request".into(),
                            fields: vec!["pair".into(), "result".into()],
                        },
                        EntrypointArgumentTypeNodeV1::Tuple { arity: 2 },
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Bool),
                        EntrypointArgumentTypeNodeV1::Result,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::String),
                        EntrypointArgumentTypeNodeV1::Option,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Name),
                    ],
                },
            }],
        };
        assert!(schema.validate());
        assert_eq!(schema.word_count(), Some(6));
        assert_eq!(
            schema.fields[0].ty.canonical_type_name().as_deref(),
            Some("struct Request")
        );

        let duplicate = EntrypointArgumentTypeV1 {
            nodes: vec![
                EntrypointArgumentTypeNodeV1::Struct {
                    name: "Bad".into(),
                    fields: vec!["same".into(), "same".into()],
                },
                EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
                EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Bool),
            ],
        };
        assert_eq!(duplicate.word_count(), None);
    }

    #[test]
    fn record_atoms_require_canonical_inactive_payloads() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "outcome".into(),
                ty: EntrypointArgumentTypeV1 {
                    nodes: vec![
                        EntrypointArgumentTypeNodeV1::Result,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::String),
                        EntrypointArgumentTypeNodeV1::Option,
                        EntrypointArgumentTypeNodeV1::Leaf(EntrypointArgumentKindV1::Int),
                    ],
                },
            }],
        };
        let canonical = vec![
            EntrypointArgumentAtomV1::Tag(false),
            EntrypointArgumentAtomV1::Null,
            EntrypointArgumentAtomV1::Tag(true),
            EntrypointArgumentAtomV1::Int(7),
        ];
        assert!(schema.validate_atoms(&canonical));
        let mut hidden = canonical;
        hidden[1] = EntrypointArgumentAtomV1::Pointer(vec![1]);
        assert!(!schema.validate_atoms(&hidden));
    }
}
