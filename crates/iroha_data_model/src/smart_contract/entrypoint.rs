//! Exact recursive boundary schemas for Kotodama entrypoints.
//!
//! JSON is accepted only at client-facing boundaries. The compiler embeds these
//! schemas in the signed contract interface so hosts can bind argument records
//! and return registers to one canonical, recursively typed ABI description.
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::{Decode, Encode, NoritoDeserialize};
// BEGIN GENERATED: kotodama-v1-source-identifier-policy
/// Exact identifier spellings forbidden in every Kotodama V1 source position.
const KOTODAMA_V1_FORBIDDEN_SOURCE_IDENTIFIERS: &[&str] = &["Amount"];
// END GENERATED: kotodama-v1-source-identifier-policy
/// Domain separator for hashes binding public argument records to exact schemas.
pub const ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"KOTODAMA_ENTRYPOINT_ARGUMENT_SCHEMA_V1\0";
/// Domain separator for hashes binding return records to exact value schemas.
pub const ENTRYPOINT_RETURN_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"KOTODAMA_ENTRYPOINT_RETURN_SCHEMA_V1\0";
/// Hash an exact encoded V1 argument schema with its dedicated domain separator.
#[must_use]
pub fn entrypoint_argument_schema_hash_v1(schema_payload: &[u8]) -> [u8; 32] {
    let mut material =
        Vec::with_capacity(ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1.len() + schema_payload.len());
    material.extend_from_slice(ENTRYPOINT_ARGUMENT_SCHEMA_HASH_DOMAIN_V1);
    material.extend_from_slice(schema_payload);
    Hash::new(&material).into()
}
/// Hash an exact encoded V1 return schema with its dedicated domain separator.
#[must_use]
pub fn entrypoint_return_schema_hash_v1(schema_payload: &[u8]) -> [u8; 32] {
    let mut material =
        Vec::with_capacity(ENTRYPOINT_RETURN_SCHEMA_HASH_DOMAIN_V1.len() + schema_payload.len());
    material.extend_from_slice(ENTRYPOINT_RETURN_SCHEMA_HASH_DOMAIN_V1);
    material.extend_from_slice(schema_payload);
    Hash::new(&material).into()
}
/// Maximum number of public source parameters carried by ABI V1.
pub const MAX_ENTRYPOINT_ARGUMENTS: usize = 13;
/// Maximum flattened public argument words in the V1 `r10..r22` call window.
pub const MAX_ENTRYPOINT_ARGUMENT_WORDS: usize = 13;
/// Maximum words returned through the public V1 register window (`r10..r22`).
pub const MAX_ENTRYPOINT_RETURN_WORDS: usize = 13;
/// Maximum recursive type nodes in one V1 boundary schema.
pub const MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES: usize = 256;
/// Maximum recursive aggregate depth in one V1 boundary schema.
pub const MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH: usize = 256;
/// Minimum capacity accepted for a boundary `List<T, N>`.
pub const MIN_ENTRYPOINT_LIST_CAPACITY_V1: u8 = 1;
/// Maximum capacity accepted for a boundary `List<T, N>`.
pub const MAX_ENTRYPOINT_LIST_CAPACITY_V1: u8 = 64;
/// Maximum encoded compiler schema accepted by the V1 argument decoder.
pub const MAX_ENTRYPOINT_ARGUMENT_SCHEMA_BYTES: usize = 64 * 1024;
/// Maximum complete encoded payload envelope at one public V1 contract boundary.
pub const MAX_ENTRYPOINT_BOUNDARY_BYTES: usize =
    crate::transaction::executable::MAX_CONTRACT_ARGUMENT_RECORD_BYTES;
/// Maximum canonical Norito argument record accepted by one public V1 invocation.
pub const MAX_ENTRYPOINT_ARGUMENT_RECORD_BYTES: usize = MAX_ENTRYPOINT_BOUNDARY_BYTES;
/// Fixed V1 pointer-ABI envelope around a `NoritoBytes` return payload.
///
/// The layout is `type: u16`, `version: u8`, `length: u32`, and a 32-byte
/// payload hash, for seven header bytes plus [`Hash::LENGTH`].
pub const ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1: usize = 7 + Hash::LENGTH;
/// Maximum canonical Norito return record accepted from one public V1 invocation.
///
/// The complete `NoritoBytes` TLV published by `CALL_CONTRACT`, not merely its
/// payload, must fit the existing one-mebibyte contract-boundary allocation.
/// Return collectors enforce this payload limit before cloning pointed VM data
/// and check the exact framed Norito length before publishing the record.
pub const MAX_ENTRYPOINT_RETURN_RECORD_BYTES: usize =
    MAX_ENTRYPOINT_BOUNDARY_BYTES - ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1;
/// Byte offset of the first naturally aligned word in a decoded argument table.
///
/// Pointer-ABI envelopes have a seven-byte header; the result `Blob` therefore reserves one payload
/// byte so every following `u64` starts at an eight-byte aligned address.
pub const DECODED_ARGUMENT_TABLE_OFFSET: i16 = 8;
/// Width of one decoded argument word in the returned table.
pub const DECODED_ARGUMENT_WORD_BYTES: i16 = 8;
/// Leaf representation used at a public Kotodama boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum EntrypointValueKindV1 {
    /// Canonical signed 512-bit integer pointer.
    Int,
    /// Canonical exact decimal pointer.
    Decimal,
    /// Canonical non-negative nominal asset-quantity pointer.
    Quantity,
    /// Boolean scalar carried as exactly zero or one.
    Bool,
    /// UTF-8 string carried as a `Blob` pointer.
    String,
    /// Nested JSON value carried as a `Json` pointer.
    Json,
    /// Validated [`crate::name::Name`] pointer.
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
    /// Raw bytes carried as a `Blob` pointer.
    Blob,
}
impl EntrypointValueKindV1 {
    /// Return whether the value occupies a validated pointer word.
    #[must_use]
    pub const fn is_pointer(self) -> bool {
        !matches!(self, Self::Bool)
    }
}
/// Named product metadata carried by a [`EntrypointValueTypeNodeV1::Struct`] node.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct EntrypointStructTypeNodeV1 {
    /// Source type name, included in the schema identity.
    pub name: String,
    /// Ordered source field names. Child nodes immediately follow.
    pub fields: Vec<String>,
}
/// Metadata carried by an [`EntrypointValueTypeNodeV1::List`] node.
///
/// The exact element subtree immediately follows this node in the enclosing preorder tape. Keeping
/// every aggregate in one flat tape makes decoding, validation, cloning, comparison, and
/// destruction bounded by the explicit V1 node budget rather than the native call stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct EntrypointListTypeNodeV1 {
    /// Compile-time capacity in the inclusive range 1 through 64.
    pub capacity: u8,
}
/// One preorder node in an exact public boundary type.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(tag = "kind", content = "value", deny_unknown_fields)]
pub enum EntrypointValueTypeNodeV1 {
    /// Named product represented by an exact JSON object.
    Struct(EntrypointStructTypeNodeV1),
    /// Positional product represented by an exact JSON array.
    Tuple(
        /// Number of child nodes which immediately follow.
        u16,
    ),
    /// Tagged optional payload represented by one active-only compiler-owned sum handle.
    Option,
    /// Tagged result payload represented by one active-only compiler-owned sum handle.
    Result,
    /// Bounded contiguous list represented by one schema-bound sequence pointer.
    List(EntrypointListTypeNodeV1),
    /// Scalar or pointer leaf consuming one ABI word.
    Leaf(EntrypointValueKindV1),
}
/// Flat, compiler-emitted recursive value type schema.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct EntrypointValueTypeV1 {
    /// Preorder aggregate layout.
    pub nodes: Vec<EntrypointValueTypeNodeV1>,
}
/// Decode-only wire twin retaining the derive-generated V1 layout while the
/// public type validates the decoded schema before returning it.
#[repr(transparent)]
#[derive(Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
struct DecodedEntrypointValueTypeV1 {
    nodes: Vec<EntrypointValueTypeNodeV1>,
}
impl<'de> NoritoDeserialize<'de> for EntrypointValueTypeV1 {
    fn schema_hash() -> [u8; 16] {
        norito::core::type_name_schema_hash::<Self>()
    }
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .unwrap_or_else(|error| panic!("invalid V1 entrypoint value schema: {error}"))
    }
    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        // Both transparent types wrap the exact same `nodes` field, so their
        // archived layouts are identical. The private twin exists solely to
        // reuse the derive-generated wire decoder.
        let decoded =
            <DecodedEntrypointValueTypeV1 as NoritoDeserialize>::try_deserialize(archived.cast())?;
        let value = Self {
            nodes: decoded.nodes,
        };
        if !value.validate() {
            return Err(norito::Error::Message(
                "invalid V1 entrypoint value schema".to_owned(),
            ));
        }
        Ok(value)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for EntrypointValueTypeV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let decoded =
            <DecodedEntrypointValueTypeV1 as norito::json::JsonDeserialize>::json_deserialize(
                parser,
            )?;
        Self::from_decoded_json(decoded)
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let decoded =
            <DecodedEntrypointValueTypeV1 as norito::json::JsonDeserialize>::json_from_value(
                value,
            )?;
        Self::from_decoded_json(decoded)
    }
}
#[cfg(feature = "json")]
impl<'a> norito::json::FastFromJson<'a> for EntrypointValueTypeV1 {
    fn parse(
        walker: &mut norito::json::TapeWalker<'a>,
        _arena: &mut norito::json::Arena,
    ) -> Result<Self, norito::Error> {
        walker.ensure_document_depth()?;
        let input = walker.input();
        let mut parser = norito::json::Parser::new_at(input, walker.raw_pos());
        let value = <Self as norito::json::JsonDeserialize>::json_deserialize(&mut parser)
            .map_err(norito::Error::from)?;
        walker.sync_to_raw(parser.position());
        Ok(value)
    }
}
// These nominal names are compiler-reserved. Matching their complete subtree
// here keeps artifact admission from trusting a forged name with different
// fields or pointer kinds.
fn core_query_view_nodes_name(nodes: &[EntrypointValueTypeNodeV1]) -> Option<(&str, usize)> {
    use EntrypointValueKindV1 as Kind;
    use EntrypointValueTypeNodeV1 as Node;
    match nodes {
        [
            Node::Struct(view),
            Node::Leaf(Kind::AccountId),
            Node::Leaf(Kind::Json),
            ..,
        ] if view.name == "AccountView" && view.fields.as_slice() == ["id", "metadata"] => {
            Some((view.name.as_str(), 3))
        }
        [
            Node::Struct(view),
            Node::Leaf(Kind::AssetId),
            Node::Leaf(Kind::Quantity),
            ..,
        ] if view.name == "AssetView" && view.fields.as_slice() == ["id", "amount"] => {
            Some((view.name.as_str(), 3))
        }
        [
            Node::Struct(view),
            Node::Leaf(Kind::AssetDefinitionId),
            Node::Leaf(Kind::String),
            Node::Option,
            Node::Leaf(Kind::String),
            Node::Leaf(Kind::AccountId),
            Node::Leaf(Kind::Quantity),
            Node::Leaf(Kind::Json),
            ..,
        ] if view.name == "AssetDefinitionView"
            && view.fields.as_slice()
                == [
                    "id",
                    "name",
                    "description",
                    "owned_by",
                    "total_quantity",
                    "metadata",
                ] =>
        {
            Some((view.name.as_str(), 8))
        }
        [
            Node::Struct(view),
            Node::Leaf(Kind::DomainId),
            Node::Leaf(Kind::AccountId),
            Node::Leaf(Kind::Json),
            ..,
        ] if view.name == "DomainView"
            && view.fields.as_slice() == ["id", "owned_by", "metadata"] =>
        {
            Some((view.name.as_str(), 4))
        }
        [
            Node::Struct(view),
            Node::Leaf(Kind::NftId),
            Node::Leaf(Kind::AccountId),
            Node::Leaf(Kind::Json),
            ..,
        ] if view.name == "NftView" && view.fields.as_slice() == ["id", "owned_by", "content"] => {
            Some((view.name.as_str(), 4))
        }
        _ => None,
    }
}
fn is_core_query_view_name(name: &str) -> bool {
    matches!(
        name,
        "AccountView" | "AssetView" | "AssetDefinitionView" | "DomainView" | "NftView"
    )
}
fn entrypoint_node_child_count(node: &EntrypointValueTypeNodeV1) -> usize {
    match node {
        EntrypointValueTypeNodeV1::Struct(node) => node.fields.len(),
        EntrypointValueTypeNodeV1::Tuple(arity) => usize::from(*arity),
        EntrypointValueTypeNodeV1::Option | EntrypointValueTypeNodeV1::List(_) => 1,
        EntrypointValueTypeNodeV1::Result => 2,
        EntrypointValueTypeNodeV1::Leaf(_) => 0,
    }
}
/// Return the exact preorder range occupied by one structurally complete boundary-type subtree.
///
/// This cursor helper deliberately checks only flat-tree structure. Callers accepting an entire
/// untrusted schema must still use [`EntrypointValueTypeV1::validate`] to enforce identifiers,
/// capacities, reserved nominal shapes, and the V1 node/depth budgets.
#[must_use]
pub fn entrypoint_value_subtree_range_v1(
    nodes: &[EntrypointValueTypeNodeV1],
    start: usize,
) -> Option<std::ops::Range<usize>> {
    let mut index = start;
    let mut pending = 1_usize;
    while pending != 0 {
        let node = nodes.get(index)?;
        index = index.checked_add(1)?;
        pending = pending
            .checked_sub(1)?
            .checked_add(entrypoint_node_child_count(node))?;
    }
    Some(start..index)
}
fn entrypoint_subtree_end(nodes: &[EntrypointValueTypeNodeV1], start: usize) -> Option<usize> {
    entrypoint_value_subtree_range_v1(nodes, start).map(|range| range.end)
}
fn core_query_view_range(
    nodes: &[EntrypointValueTypeNodeV1],
    start: usize,
) -> Option<(&str, std::ops::Range<usize>)> {
    let (name, consumed) = core_query_view_nodes_name(nodes.get(start..)?)?;
    let expected_end = start.checked_add(consumed)?;
    let range = entrypoint_value_subtree_range_v1(nodes, start)?;
    (range.end == expected_end).then_some((name, range))
}
fn validate_reserved_nominal_shapes(schema: &EntrypointValueTypeV1) -> bool {
    use EntrypointValueKindV1 as Kind;
    use EntrypointValueTypeNodeV1 as Node;
    for (start, node) in schema.nodes.iter().enumerate() {
        let Node::Struct(node) = node else {
            continue;
        };
        if is_core_query_view_name(&node.name) {
            if core_query_view_range(&schema.nodes, start).is_none() {
                return false;
            }
            continue;
        }
        if node.name != "QueryPage" {
            continue;
        }
        if node.fields.as_slice() != ["items", "next_offset"] {
            return false;
        }
        let Some(root_range) = entrypoint_value_subtree_range_v1(&schema.nodes, start) else {
            return false;
        };
        let Some(list_start) = start.checked_add(1) else {
            return false;
        };
        let Some(Node::List(items)) = schema.nodes.get(list_start) else {
            return false;
        };
        if items.capacity != MAX_ENTRYPOINT_LIST_CAPACITY_V1 {
            return false;
        }
        let Some(element_start) = list_start.checked_add(1) else {
            return false;
        };
        let Some((_, element_range)) = core_query_view_range(&schema.nodes, element_start) else {
            return false;
        };
        let Some(list_range) = entrypoint_value_subtree_range_v1(&schema.nodes, list_start) else {
            return false;
        };
        let next_offset_start = element_range.end;
        let Some(next_offset_leaf) = next_offset_start.checked_add(1) else {
            return false;
        };
        let Some(next_offset_end) = next_offset_leaf.checked_add(1) else {
            return false;
        };
        if list_range.end != element_range.end
            || !matches!(schema.nodes.get(next_offset_start), Some(Node::Option))
            || !matches!(
                schema.nodes.get(next_offset_leaf),
                Some(Node::Leaf(Kind::Int))
            )
            || entrypoint_value_subtree_range_v1(&schema.nodes, next_offset_start)
                != Some(next_offset_start..next_offset_end)
            || root_range.end != next_offset_end
        {
            return false;
        }
    }
    true
}
struct RenderedEntrypointType {
    text: String,
    core_view: Option<String>,
    list_element_core_view: Option<String>,
}
fn take_rendered_entrypoint_children(
    rendered: &mut Vec<RenderedEntrypointType>,
    count: usize,
) -> Option<Vec<RenderedEntrypointType>> {
    if rendered.len() < count {
        return None;
    }
    let split = rendered.len() - count;
    let mut children = rendered.split_off(split);
    children.reverse();
    Some(children)
}
impl EntrypointValueTypeV1 {
    #[cfg(feature = "json")]
    fn from_decoded_json(
        decoded: DecodedEntrypointValueTypeV1,
    ) -> Result<Self, norito::json::Error> {
        let value = Self {
            nodes: decoded.nodes,
        };
        if !value.validate() {
            return Err(norito::json::Error::Message(
                "invalid V1 entrypoint value schema".to_owned(),
            ));
        }
        Ok(value)
    }
    fn analyze(&self) -> Option<EntrypointTypeAnalysisV1> {
        #[derive(Clone, Copy)]
        struct Frame {
            remaining: usize,
            suppress_words: bool,
        }
        if self.nodes.is_empty() || self.nodes.len() > MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES {
            return None;
        }
        let mut stack = Vec::<Frame>::new();
        let mut max_words = 0_usize;
        for (index, node) in self.nodes.iter().enumerate() {
            while stack.last().is_some_and(|frame| frame.remaining == 0) {
                stack.pop();
            }
            let suppress_words = if index == 0 {
                false
            } else {
                let parent = stack.last_mut()?;
                parent.remaining = parent.remaining.checked_sub(1)?;
                parent.suppress_words
            };
            let depth = stack.len().checked_add(1)?;
            if depth > MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH {
                return None;
            }
            match node {
                EntrypointValueTypeNodeV1::Struct(node) => {
                    if node.fields.is_empty()
                        || !is_canonical_kotodama_identifier(&node.name)
                        || node
                            .fields
                            .iter()
                            .any(|field| !is_canonical_kotodama_identifier(field))
                        || node
                            .fields
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            != node.fields.len()
                    {
                        return None;
                    }
                }
                EntrypointValueTypeNodeV1::Tuple(arity) if *arity < 2 => return None,
                EntrypointValueTypeNodeV1::List(list)
                    if !(MIN_ENTRYPOINT_LIST_CAPACITY_V1..=MAX_ENTRYPOINT_LIST_CAPACITY_V1)
                        .contains(&list.capacity) =>
                {
                    return None;
                }
                _ => {}
            }
            let is_handle = matches!(
                node,
                EntrypointValueTypeNodeV1::Option
                    | EntrypointValueTypeNodeV1::Result
                    | EntrypointValueTypeNodeV1::List(_)
            );
            if !suppress_words && (is_handle || matches!(node, EntrypointValueTypeNodeV1::Leaf(_)))
            {
                max_words = max_words.checked_add(1)?;
            }
            let children = entrypoint_node_child_count(node);
            if children != 0 {
                stack.push(Frame {
                    remaining: children,
                    suppress_words: suppress_words || is_handle,
                });
            }
        }
        while stack.last().is_some_and(|frame| frame.remaining == 0) {
            stack.pop();
        }
        if !stack.is_empty() || !validate_reserved_nominal_shapes(self) {
            return None;
        }
        Some(EntrypointTypeAnalysisV1 { max_words })
    }
    /// Validate tree shape, identifiers, list capacities, and recursive limits.
    #[must_use]
    pub fn validate(&self) -> bool {
        self.analyze().is_some()
    }
    /// Borrow the exact flat node slice for the subtree beginning at `start`.
    ///
    /// The full enclosing schema is validated first, so a returned slice is
    /// guaranteed to be inside one admissible V1 preorder tape.
    #[must_use]
    pub fn subtree_nodes(&self, start: usize) -> Option<&[EntrypointValueTypeNodeV1]> {
        if !self.validate() {
            return None;
        }
        let range = entrypoint_value_subtree_range_v1(&self.nodes, start)?;
        self.nodes.get(range)
    }
    /// Return the fixed ABI words emitted for this type.
    ///
    /// Every `Option`, `Result`, and `List` consumes one compiler-owned handle
    /// word; products flatten their children in declaration order.
    #[must_use]
    pub fn word_count(&self) -> Option<usize> {
        self.analyze().map(|analysis| analysis.max_words)
    }
    /// Return flattened ABI word roles in deterministic preorder.
    pub fn word_kinds(&self) -> Option<Vec<EntrypointValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut node_index = 0;
        let words = max_entrypoint_word_kinds(&self.nodes, &mut node_index)?;
        (node_index == self.nodes.len()).then_some(words)
    }
    /// Validate one active-only flat atom tape against this exact type.
    #[must_use]
    pub fn validate_atoms(&self, atoms: &[EntrypointValueAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }
        let mut node_index = 0;
        let mut atom_index = 0;
        walk_entrypoint_value_atoms(&self.nodes, atoms, &mut node_index, &mut atom_index, None)
            && node_index == self.nodes.len()
            && atom_index == atoms.len()
    }
    /// Return the actual flattened VM word roles selected by an active-only value.
    pub fn word_kinds_for_atoms(
        &self,
        atoms: &[EntrypointValueAtomV1],
    ) -> Option<Vec<EntrypointValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut node_index = 0;
        let mut atom_index = 0;
        let mut kinds = Vec::new();
        if !walk_entrypoint_value_atoms(
            &self.nodes,
            atoms,
            &mut node_index,
            &mut atom_index,
            Some(&mut kinds),
        ) || node_index != self.nodes.len()
            || atom_index != atoms.len()
        {
            return None;
        }
        Some(kinds)
    }
    /// Render the canonical public Kotodama type name represented by this schema.
    pub fn canonical_type_name(&self) -> Option<String> {
        if !self.validate() {
            return None;
        }
        let mut rendered = Vec::<RenderedEntrypointType>::new();
        for node in self.nodes.iter().rev() {
            let value = match node {
                EntrypointValueTypeNodeV1::Struct(node) => {
                    let children =
                        take_rendered_entrypoint_children(&mut rendered, node.fields.len())?;
                    let (text, core_view) = if node.name == "QueryPage" {
                        let view_name = children.first()?.list_element_core_view.clone()?;
                        (format!("QueryPage<{view_name}>"), None)
                    } else if is_core_query_view_name(&node.name) {
                        (node.name.clone(), Some(node.name.clone()))
                    } else {
                        (format!("struct {}", node.name), None)
                    };
                    RenderedEntrypointType {
                        text,
                        core_view,
                        list_element_core_view: None,
                    }
                }
                EntrypointValueTypeNodeV1::Tuple(arity) => {
                    let children =
                        take_rendered_entrypoint_children(&mut rendered, usize::from(*arity))?;
                    RenderedEntrypointType {
                        text: format!(
                            "({})",
                            children
                                .iter()
                                .map(|child| child.text.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        core_view: None,
                        list_element_core_view: None,
                    }
                }
                EntrypointValueTypeNodeV1::Option => {
                    let child = take_rendered_entrypoint_children(&mut rendered, 1)?.pop()?;
                    RenderedEntrypointType {
                        text: format!("Option<{}>", child.text),
                        core_view: None,
                        list_element_core_view: None,
                    }
                }
                EntrypointValueTypeNodeV1::Result => {
                    let mut children =
                        take_rendered_entrypoint_children(&mut rendered, 2)?.into_iter();
                    let ok = children.next()?;
                    let err = children.next()?;
                    RenderedEntrypointType {
                        text: format!("Result<{}, {}>", ok.text, err.text),
                        core_view: None,
                        list_element_core_view: None,
                    }
                }
                EntrypointValueTypeNodeV1::List(list) => {
                    let child = take_rendered_entrypoint_children(&mut rendered, 1)?.pop()?;
                    RenderedEntrypointType {
                        text: format!("List<{}, {}>", child.text, list.capacity),
                        core_view: None,
                        list_element_core_view: child.core_view,
                    }
                }
                EntrypointValueTypeNodeV1::Leaf(kind) => RenderedEntrypointType {
                    text: match kind {
                        EntrypointValueKindV1::Int => "int".to_owned(),
                        EntrypointValueKindV1::Decimal => "decimal".to_owned(),
                        EntrypointValueKindV1::Quantity => "quantity".to_owned(),
                        EntrypointValueKindV1::Bool => "bool".to_owned(),
                        EntrypointValueKindV1::String => "string".to_owned(),
                        EntrypointValueKindV1::Json => "Json".to_owned(),
                        EntrypointValueKindV1::Name => "Name".to_owned(),
                        EntrypointValueKindV1::AccountId => "AccountId".to_owned(),
                        EntrypointValueKindV1::AssetDefinitionId => "AssetDefinitionId".to_owned(),
                        EntrypointValueKindV1::AssetId => "AssetId".to_owned(),
                        EntrypointValueKindV1::DomainId => "DomainId".to_owned(),
                        EntrypointValueKindV1::NftId => "NftId".to_owned(),
                        EntrypointValueKindV1::DataSpaceId => "DataSpaceId".to_owned(),
                        EntrypointValueKindV1::Blob => "bytes".to_owned(),
                    },
                    core_view: None,
                    list_element_core_view: None,
                },
            };
            rendered.push(value);
        }
        (rendered.len() == 1).then(|| rendered.pop().expect("length checked").text)
    }
}
#[derive(Clone, Copy)]
struct EntrypointTypeAnalysisV1 {
    max_words: usize,
}
/// Flattened word role derived from a validated boundary schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum EntrypointValueWordKindV1 {
    /// One active-only compiler-owned Option/Result handle.
    Sum,
    /// One schema-bound canonical list-sequence pointer.
    List,
    /// Scalar or pointer leaf.
    Leaf(EntrypointValueKindV1),
}
/// Canonical wire atom in a public entrypoint value record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum EntrypointValueAtomV1 {
    /// Option/Result tag.
    Tag(bool),
    /// Boolean scalar.
    Bool(bool),
    /// Complete pointer-ABI TLV envelope for a typed leaf.
    Pointer(Vec<u8>),
    /// Canonical bounded sequence marker carrying the number of following items.
    ///
    /// The items immediately follow this marker in the enclosing record's one
    /// preorder atom tape. Each item is delimited by the list element schema,
    /// so no recursive atom ownership or end marker is needed.
    List(
        /// Number of schema-delimited item streams which immediately follow.
        u8,
    ),
}
/// Schema-bound canonical Norito payload supplied to a public entrypoint wrapper.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct EntrypointArgumentRecordV1 {
    /// Domain-separated hash of the exact encoded schema.
    pub schema_hash: [u8; 32],
    /// Flattened atoms in declaration and schema preorder.
    pub atoms: Vec<EntrypointValueAtomV1>,
}
/// Schema-bound canonical Norito payload returned by a nested contract call.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct EntrypointReturnRecordV1 {
    /// Domain-separated hash of the exact encoded return value schema.
    pub schema_hash: [u8; 32],
    /// Flattened canonical atoms in schema preorder.
    pub atoms: Vec<EntrypointValueAtomV1>,
}
/// One named field in a public entrypoint argument record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct EntrypointArgumentFieldV1 {
    /// Source-level parameter name used as the boundary object key.
    pub name: String,
    /// Canonical representation expected by the compiled implementation.
    pub ty: EntrypointValueTypeV1,
}
/// Compiler-emitted schema for one public entrypoint invocation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        crate::DeriveFastJson,
        crate::DeriveJsonSerialize,
        crate::DeriveJsonDeserialize
    )
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct EntrypointArgumentSchemaV1 {
    /// Fields in source declaration and ABI register order.
    pub fields: Vec<EntrypointArgumentFieldV1>,
}
impl EntrypointArgumentSchemaV1 {
    /// Validate source-field uniqueness and the argument ABI bounds.
    #[must_use]
    pub fn validate(&self) -> bool {
        if self.fields.is_empty() || self.fields.len() > MAX_ENTRYPOINT_ARGUMENTS {
            return false;
        }
        let mut names = std::collections::BTreeSet::new();
        self.fields.iter().all(|field| {
            is_canonical_kotodama_identifier(&field.name)
                && names.insert(field.name.as_str())
                && field.ty.validate()
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
    pub fn word_kinds(&self) -> Option<Vec<EntrypointValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut words = Vec::with_capacity(self.word_count_unchecked());
        for field in &self.fields {
            words.extend(field.ty.word_kinds()?);
        }
        Some(words)
    }
    /// Validate active-only atom variants for every declared field.
    #[must_use]
    pub fn validate_atoms(&self, atoms: &[EntrypointValueAtomV1]) -> bool {
        if !self.validate() {
            return false;
        }
        let mut atom_index = 0;
        for field in &self.fields {
            let mut node_index = 0;
            if !walk_entrypoint_value_atoms(
                &field.ty.nodes,
                atoms,
                &mut node_index,
                &mut atom_index,
                None,
            ) || node_index != field.ty.nodes.len()
            {
                return false;
            }
        }
        atom_index == atoms.len()
    }
    /// Return actual flattened VM word roles selected by this argument record.
    pub fn word_kinds_for_atoms(
        &self,
        atoms: &[EntrypointValueAtomV1],
    ) -> Option<Vec<EntrypointValueWordKindV1>> {
        if !self.validate() {
            return None;
        }
        let mut atom_index = 0;
        let mut kinds = Vec::new();
        for field in &self.fields {
            let mut node_index = 0;
            if !walk_entrypoint_value_atoms(
                &field.ty.nodes,
                atoms,
                &mut node_index,
                &mut atom_index,
                Some(&mut kinds),
            ) || node_index != field.ty.nodes.len()
            {
                return None;
            }
        }
        (atom_index == atoms.len() && kinds.len() <= MAX_ENTRYPOINT_ARGUMENT_WORDS).then_some(kinds)
    }
}
fn max_entrypoint_word_kinds(
    nodes: &[EntrypointValueTypeNodeV1],
    node_index: &mut usize,
) -> Option<Vec<EntrypointValueWordKindV1>> {
    let start = *node_index;
    let end = entrypoint_subtree_end(nodes, start)?;
    let mut rendered = Vec::<Vec<EntrypointValueWordKindV1>>::new();
    for node in nodes[start..end].iter().rev() {
        let child_count = entrypoint_node_child_count(node);
        if rendered.len() < child_count {
            return None;
        }
        let split = rendered.len() - child_count;
        let mut children = rendered.split_off(split);
        children.reverse();
        let words = match node {
            EntrypointValueTypeNodeV1::Struct(_) | EntrypointValueTypeNodeV1::Tuple(_) => {
                children.into_iter().flatten().collect()
            }
            EntrypointValueTypeNodeV1::Option | EntrypointValueTypeNodeV1::Result => {
                vec![EntrypointValueWordKindV1::Sum]
            }
            EntrypointValueTypeNodeV1::List(_) => {
                vec![EntrypointValueWordKindV1::List]
            }
            EntrypointValueTypeNodeV1::Leaf(kind) => {
                vec![EntrypointValueWordKindV1::Leaf(*kind)]
            }
        };
        rendered.push(words);
    }
    *node_index = end;
    (rendered.len() == 1).then(|| rendered.pop().expect("length checked"))
}
fn walk_entrypoint_value_atoms(
    nodes: &[EntrypointValueTypeNodeV1],
    atoms: &[EntrypointValueAtomV1],
    node_index: &mut usize,
    atom_index: &mut usize,
    mut kinds: Option<&mut Vec<EntrypointValueWordKindV1>>,
) -> bool {
    let start = *node_index;
    let Some(end) = entrypoint_subtree_end(nodes, start) else {
        return false;
    };
    // `(node start, emit one top-level ABI word)` actions are deliberately
    // iterative. Repeated list elements reuse the same schema subtree while
    // advancing one shared atom cursor, so neither schema nor value nesting
    // consumes the native call stack.
    let mut actions = vec![(start, true)];
    let mut cursor = *atom_index;
    while let Some((node_start, emit_kind)) = actions.pop() {
        let Some(node) = nodes.get(node_start) else {
            return false;
        };
        match node {
            EntrypointValueTypeNodeV1::Struct(node) => {
                let mut child = node_start + 1;
                let mut starts = Vec::with_capacity(node.fields.len());
                for _ in &node.fields {
                    starts.push(child);
                    let Some(next) = entrypoint_subtree_end(nodes, child) else {
                        return false;
                    };
                    child = next;
                }
                actions.extend(starts.into_iter().rev().map(|start| (start, emit_kind)));
            }
            EntrypointValueTypeNodeV1::Tuple(arity) => {
                let mut child = node_start + 1;
                let mut starts = Vec::with_capacity(usize::from(*arity));
                for _ in 0..*arity {
                    starts.push(child);
                    let Some(next) = entrypoint_subtree_end(nodes, child) else {
                        return false;
                    };
                    child = next;
                }
                actions.extend(starts.into_iter().rev().map(|start| (start, emit_kind)));
            }
            EntrypointValueTypeNodeV1::Option => {
                let Some(EntrypointValueAtomV1::Tag(active)) = atoms.get(cursor) else {
                    return false;
                };
                cursor += 1;
                if emit_kind && let Some(kinds) = kinds.as_deref_mut() {
                    kinds.push(EntrypointValueWordKindV1::Sum);
                }
                if *active {
                    actions.push((node_start + 1, false));
                }
            }
            EntrypointValueTypeNodeV1::Result => {
                let Some(EntrypointValueAtomV1::Tag(ok_active)) = atoms.get(cursor) else {
                    return false;
                };
                cursor += 1;
                if emit_kind && let Some(kinds) = kinds.as_deref_mut() {
                    kinds.push(EntrypointValueWordKindV1::Sum);
                }
                let ok_start = node_start + 1;
                let Some(err_start) = entrypoint_subtree_end(nodes, ok_start) else {
                    return false;
                };
                actions.push((if *ok_active { ok_start } else { err_start }, false));
            }
            EntrypointValueTypeNodeV1::List(list) => {
                let Some(EntrypointValueAtomV1::List(item_count)) = atoms.get(cursor) else {
                    return false;
                };
                cursor += 1;
                if *item_count > list.capacity {
                    return false;
                }
                if emit_kind && let Some(kinds) = kinds.as_deref_mut() {
                    kinds.push(EntrypointValueWordKindV1::List);
                }
                let element_start = node_start + 1;
                actions.extend(std::iter::repeat_n(
                    (element_start, false),
                    usize::from(*item_count),
                ));
            }
            EntrypointValueTypeNodeV1::Leaf(kind) => {
                let Some(atom) = atoms.get(cursor) else {
                    return false;
                };
                cursor += 1;
                let valid = matches!(
                    (kind, atom),
                    (EntrypointValueKindV1::Bool, EntrypointValueAtomV1::Bool(_))
                ) || (kind.is_pointer()
                    && matches!(atom, EntrypointValueAtomV1::Pointer(_)));
                if !valid {
                    return false;
                }
                if emit_kind && let Some(kinds) = kinds.as_deref_mut() {
                    kinds.push(EntrypointValueWordKindV1::Leaf(*kind));
                }
            }
        }
    }
    *node_index = end;
    *atom_index = cursor;
    true
}
/// Return whether `value` is an ASCII Kotodama identifier that does not collide
/// with a canonical V1 keyword or first-release forbidden source identifier.
///
/// Manifest and SDK boundary-schema validators use this predicate so schema
/// field names cannot reinterpret a language keyword as an ordinary binding.
#[must_use]
pub fn is_canonical_kotodama_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !KOTODAMA_V1_FORBIDDEN_SOURCE_IDENTIFIERS.contains(&value)
        && !matches!(
            value,
            "authorize"
                | "break"
                | "const"
                | "continue"
                | "decimal"
                | "else"
                | "enum"
                | "error"
                | "false"
                | "fn"
                | "for"
                | "hajimari"
                | "if"
                | "in"
                | "int"
                | "kaizen"
                | "kotoage"
                | "let"
                | "match"
                | "module"
                | "quantity"
                | "return"
                | "seiyaku"
                | "state"
                | "struct"
                | "trigger"
                | "true"
                | "var"
                | "view"
        )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn leaf(kind: EntrypointValueKindV1) -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Leaf(kind)],
        }
    }
    fn int_atom(value: u8) -> EntrypointValueAtomV1 {
        EntrypointValueAtomV1::Pointer(vec![value])
    }
    fn nested_list_schema(list_count: usize) -> EntrypointValueTypeV1 {
        let mut nodes = Vec::with_capacity(list_count.saturating_add(1));
        nodes.extend((0..list_count).map(|_| {
            EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: MIN_ENTRYPOINT_LIST_CAPACITY_V1,
            })
        }));
        nodes.push(EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int));
        EntrypointValueTypeV1 { nodes }
    }
    fn wide_tuple_schema(child_count: usize) -> EntrypointValueTypeV1 {
        let arity = u16::try_from(child_count).expect("test tuple arity fits u16");
        let mut nodes = Vec::with_capacity(child_count.saturating_add(1));
        nodes.push(EntrypointValueTypeNodeV1::Tuple(arity));
        nodes.extend(
            (0..child_count).map(|_| EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int)),
        );
        EntrypointValueTypeV1 { nodes }
    }
    fn account_view_schema() -> EntrypointValueTypeV1 {
        EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "AccountView".into(),
                    fields: vec!["id".into(), "metadata".into()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::AccountId),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Json),
            ],
        }
    }
    fn query_page_schema(view: &EntrypointValueTypeV1) -> EntrypointValueTypeV1 {
        let mut nodes = vec![
            EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                name: "QueryPage".into(),
                fields: vec!["items".into(), "next_offset".into()],
            }),
            EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 {
                capacity: MAX_ENTRYPOINT_LIST_CAPACITY_V1,
            }),
        ];
        nodes.extend(view.nodes.iter().cloned());
        nodes.extend([
            EntrypointValueTypeNodeV1::Option,
            EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
        ]);
        EntrypointValueTypeV1 { nodes }
    }
    #[test]
    fn exact_nested_type_roundtrips_and_counts_register_words() {
        let ty = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Receipt".into(),
                    fields: vec!["status".into(), "detail".into()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                EntrypointValueTypeNodeV1::Result,
                EntrypointValueTypeNodeV1::Option,
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                EntrypointValueTypeNodeV1::Tuple(2),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            ],
        };
        let encoded = norito::to_bytes(&ty).expect("encode exact boundary type");
        let decoded: EntrypointValueTypeV1 =
            norito::decode_from_bytes(&encoded).expect("decode exact boundary type");
        assert_eq!(decoded, ty);
        assert_eq!(decoded.word_count(), Some(2));
        assert_eq!(
            decoded.canonical_type_name().as_deref(),
            Some("struct Receipt")
        );
    }
    #[test]
    fn numeric_boundary_leaves_are_distinct_pointer_types() {
        for (kind, name) in [
            (EntrypointValueKindV1::Int, "int"),
            (EntrypointValueKindV1::Decimal, "decimal"),
            (EntrypointValueKindV1::Quantity, "quantity"),
        ] {
            assert!(kind.is_pointer());
            let schema = leaf(kind);
            assert_eq!(schema.canonical_type_name().as_deref(), Some(name));
            assert!(schema.validate_atoms(&[EntrypointValueAtomV1::Pointer(vec![1])]));
            assert!(!schema.validate_atoms(&[EntrypointValueAtomV1::Bool(true)]));
        }
        assert!(!EntrypointValueKindV1::Bool.is_pointer());
    }
    #[test]
    fn inactive_sum_payloads_are_absent_from_the_wire() {
        let schema = EntrypointArgumentSchemaV1 {
            fields: vec![EntrypointArgumentFieldV1 {
                name: "value".into(),
                ty: EntrypointValueTypeV1 {
                    nodes: vec![
                        EntrypointValueTypeNodeV1::Option,
                        EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
                    ],
                },
            }],
        };
        assert_eq!(schema.word_count(), Some(1));
        assert_eq!(
            schema.word_kinds(),
            Some(vec![EntrypointValueWordKindV1::Sum])
        );
        assert!(schema.validate_atoms(&[EntrypointValueAtomV1::Tag(false)]));
        assert_eq!(
            schema.word_kinds_for_atoms(&[EntrypointValueAtomV1::Tag(false)]),
            Some(vec![EntrypointValueWordKindV1::Sum])
        );
        assert!(!schema.validate_atoms(&[
            EntrypointValueAtomV1::Tag(false),
            EntrypointValueAtomV1::Pointer(vec![1]),
        ]));
    }
    #[test]
    fn recursive_lists_roundtrip_and_enforce_capacity() {
        let nested = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 3 }),
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Quantity),
            ],
        };
        assert!(nested.validate());
        assert_eq!(nested.word_count(), Some(1));
        assert_eq!(
            nested.canonical_type_name().as_deref(),
            Some("List<List<quantity, 2>, 3>")
        );
        assert_eq!(
            entrypoint_value_subtree_range_v1(&nested.nodes, 0),
            Some(0..3)
        );
        assert_eq!(
            entrypoint_value_subtree_range_v1(&nested.nodes, 1),
            Some(1..3)
        );
        assert_eq!(
            entrypoint_value_subtree_range_v1(&nested.nodes, 2),
            Some(2..3)
        );
        assert_eq!(entrypoint_value_subtree_range_v1(&nested.nodes, 3), None);
        assert_eq!(nested.subtree_nodes(1), Some(&nested.nodes[1..3]));
        let encoded = norito::to_bytes(&nested).expect("encode nested list schema");
        assert_eq!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded)
                .expect("decode nested list schema"),
            nested
        );
        for capacity in [0, 65] {
            let invalid = EntrypointValueTypeV1 {
                nodes: vec![
                    EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity }),
                    EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                ],
            };
            assert!(!invalid.validate());
            assert_eq!(invalid.subtree_nodes(0), None);
        }
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_string(&nested).expect("encode flat list schema JSON");
            assert!(json.contains("capacity"));
            assert!(
                !json.contains("element"),
                "the flat V1 wire schema must not retain recursive list ownership"
            );
            assert_eq!(
                norito::json::from_str::<EntrypointValueTypeV1>(&json)
                    .expect("decode flat list schema JSON"),
                nested
            );
        }
    }
    #[test]
    fn flat_schema_rejects_missing_children_and_trailing_roots() {
        let list = EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 1 });
        let int = EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int);
        for nodes in [
            Vec::new(),
            vec![list.clone()],
            vec![list.clone(), int.clone(), int.clone()],
            vec![EntrypointValueTypeNodeV1::Option],
            vec![EntrypointValueTypeNodeV1::Result, int.clone()],
            vec![EntrypointValueTypeNodeV1::Tuple(2), int.clone()],
            vec![int.clone(), int.clone()],
        ] {
            let schema = EntrypointValueTypeV1 { nodes };
            assert!(!schema.validate());
            assert_eq!(schema.word_count(), None);
            assert_eq!(schema.word_kinds(), None);
            assert_eq!(schema.canonical_type_name(), None);
            assert!(!schema.validate_atoms(&[]));
            let encoded = norito::to_bytes(&schema).expect("encode malformed flat schema fixture");
            assert!(matches!(
                norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded),
                Err(norito::Error::Message(message))
                    if message == "invalid V1 entrypoint value schema"
            ));
            #[cfg(feature = "json")]
            {
                let json =
                    norito::json::to_string(&schema).expect("encode malformed flat schema JSON");
                assert!(norito::json::from_str::<EntrypointValueTypeV1>(&json).is_err());
                assert!(norito::json::from_json_auto::<EntrypointValueTypeV1>(&json).is_err());
            }
        }
    }
    #[test]
    fn list_items_use_the_exact_following_subtree_and_advance_once() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Tuple(2),
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                EntrypointValueTypeNodeV1::Tuple(2),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String),
            ],
        };
        assert!(schema.validate());
        assert_eq!(schema.word_count(), Some(2));
        assert_eq!(
            schema.word_kinds(),
            Some(vec![
                EntrypointValueWordKindV1::List,
                EntrypointValueWordKindV1::Leaf(EntrypointValueKindV1::String),
            ])
        );
        let atoms = vec![
            EntrypointValueAtomV1::List(2),
            int_atom(1),
            EntrypointValueAtomV1::Bool(true),
            int_atom(2),
            EntrypointValueAtomV1::Bool(false),
            EntrypointValueAtomV1::Pointer(vec![0x10]),
        ];
        assert!(schema.validate_atoms(&atoms));
        assert_eq!(schema.word_kinds_for_atoms(&atoms), schema.word_kinds());
        let empty = vec![
            EntrypointValueAtomV1::List(0),
            EntrypointValueAtomV1::Pointer(Vec::new()),
        ];
        assert!(schema.validate_atoms(&empty));
        let invalid = [
            vec![
                EntrypointValueAtomV1::List(3),
                int_atom(1),
                EntrypointValueAtomV1::Bool(true),
                int_atom(2),
                EntrypointValueAtomV1::Bool(false),
                int_atom(3),
                EntrypointValueAtomV1::Bool(true),
                EntrypointValueAtomV1::Pointer(Vec::new()),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                int_atom(1),
                EntrypointValueAtomV1::Pointer(Vec::new()),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                int_atom(1),
                EntrypointValueAtomV1::Bool(true),
                EntrypointValueAtomV1::Bool(false),
                EntrypointValueAtomV1::Pointer(Vec::new()),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::Bool(true),
                int_atom(1),
                EntrypointValueAtomV1::Pointer(Vec::new()),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                int_atom(1),
                EntrypointValueAtomV1::Bool(true),
            ],
            vec![
                EntrypointValueAtomV1::List(2),
                int_atom(1),
                EntrypointValueAtomV1::Bool(true),
                EntrypointValueAtomV1::Pointer(Vec::new()),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                int_atom(2),
                EntrypointValueAtomV1::Bool(false),
                EntrypointValueAtomV1::Pointer(Vec::new()),
                EntrypointValueAtomV1::Bool(false),
            ],
        ];
        for atoms in invalid {
            assert!(
                !schema.validate_atoms(&atoms),
                "malformed list record {atoms:?}"
            );
            assert_eq!(schema.word_kinds_for_atoms(&atoms), None);
        }
    }
    #[test]
    fn nested_list_items_are_checked_without_recursive_schema_ownership() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 1 }),
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 1 }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        let valid = vec![
            EntrypointValueAtomV1::List(1),
            EntrypointValueAtomV1::List(1),
            int_atom(7),
        ];
        assert!(schema.validate_atoms(&valid));
        assert_eq!(
            schema.word_kinds_for_atoms(&valid),
            Some(vec![EntrypointValueWordKindV1::List])
        );
        for invalid in [
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::List(1),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::List(2),
                int_atom(1),
                int_atom(2),
            ],
            vec![EntrypointValueAtomV1::List(1), int_atom(7)],
            vec![
                EntrypointValueAtomV1::List(0),
                EntrypointValueAtomV1::List(0),
            ],
        ] {
            assert!(!schema.validate_atoms(&invalid));
        }
    }
    #[test]
    fn flat_list_marker_rejects_truncation_trailing_atoms_and_count_mismatches() {
        let schema = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::List(EntrypointListTypeNodeV1 { capacity: 2 }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        assert!(schema.validate_atoms(&[EntrypointValueAtomV1::List(0)]));
        assert!(
            schema.validate_atoms(&[EntrypointValueAtomV1::List(2), int_atom(1), int_atom(2),])
        );
        for malformed in [
            vec![EntrypointValueAtomV1::List(1)],
            vec![EntrypointValueAtomV1::List(0), int_atom(1)],
            vec![EntrypointValueAtomV1::List(1), int_atom(1), int_atom(2)],
            vec![EntrypointValueAtomV1::List(2), int_atom(1)],
            vec![
                EntrypointValueAtomV1::List(3),
                int_atom(1),
                int_atom(2),
                int_atom(3),
            ],
            vec![
                EntrypointValueAtomV1::List(1),
                EntrypointValueAtomV1::Bool(true),
            ],
        ] {
            assert!(!schema.validate_atoms(&malformed), "{malformed:?}");
            assert_eq!(schema.word_kinds_for_atoms(&malformed), None);
        }
    }
    #[test]
    fn flat_atom_tape_is_stack_safe_at_the_depth_limit() {
        let schema = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1);
        let mut atoms = Vec::with_capacity(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        atoms.extend(
            (0..MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1).map(|_| EntrypointValueAtomV1::List(1)),
        );
        atoms.push(int_atom(7));
        assert!(schema.validate_atoms(&atoms));
        assert_eq!(
            schema.word_kinds_for_atoms(&atoms),
            Some(vec![EntrypointValueWordKindV1::List])
        );
        let record = EntrypointArgumentRecordV1 {
            schema_hash: [0x5a; 32],
            atoms,
        };
        let encoded = norito::to_bytes(&record).expect("encode flat atom tape at depth limit");
        let decoded = norito::decode_from_bytes::<EntrypointArgumentRecordV1>(&encoded)
            .expect("decode flat atom tape at depth limit");
        assert_eq!(decoded, record);
        assert!(schema.validate_atoms(&decoded.atoms));
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode flat atom tape at depth limit"),
            encoded
        );
    }
    #[test]
    fn flat_schema_accepts_exact_node_budget_and_rejects_one_more() {
        let at_limit = wide_tuple_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES - 1);
        assert_eq!(at_limit.nodes.len(), MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES);
        assert!(at_limit.validate());
        assert_eq!(
            entrypoint_value_subtree_range_v1(&at_limit.nodes, 0),
            Some(0..MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES)
        );
        let encoded = norito::to_bytes(&at_limit).expect("encode max-node flat schema");
        assert_eq!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded)
                .expect("decode max-node flat schema"),
            at_limit
        );
        let over_limit = wide_tuple_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES);
        assert_eq!(
            over_limit.nodes.len(),
            MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES + 1
        );
        assert!(!over_limit.validate());
        let encoded = norito::to_bytes(&over_limit).expect("encode over-node-budget fixture");
        assert!(matches!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded),
            Err(norito::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_string(&over_limit).expect("encode over-node-budget JSON");
            assert!(norito::json::from_str::<EntrypointValueTypeV1>(&json).is_err());
            assert!(norito::json::from_json_auto::<EntrypointValueTypeV1>(&json).is_err());
        }
    }
    #[test]
    fn binary_schema_decode_accepts_the_flat_depth_limit_and_rejects_the_next_level() {
        let at_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1);
        assert_eq!(at_limit.nodes.len(), MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        assert!(at_limit.validate());
        let encoded = norito::to_bytes(&at_limit).expect("encode schema at flat depth limit");
        let decoded = norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded)
            .expect("decode schema at flat depth limit");
        assert_eq!(decoded, at_limit);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode schema at flat depth limit"),
            encoded,
            "custom validation must not change the canonical wire format"
        );
        let over_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        assert_eq!(
            over_limit.nodes.len(),
            MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH + 1
        );
        assert!(!over_limit.validate());
        let encoded = norito::to_bytes(&over_limit).expect("encode over-limit schema fixture");
        assert!(matches!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded),
            Err(norito::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
    }
    #[test]
    fn binary_schema_decode_rejects_deep_flat_input_without_native_recursion() {
        let deep = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES + 64);
        let encoded = norito::to_bytes(&deep).expect("encode adversarial flat schema");
        assert!(matches!(
            norito::decode_from_bytes::<EntrypointValueTypeV1>(&encoded),
            Err(norito::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
        let leaf_bytes =
            norito::to_bytes(&leaf(EntrypointValueKindV1::Bool)).expect("encode recovery fixture");
        norito::decode_from_bytes::<EntrypointValueTypeV1>(&leaf_bytes)
            .expect("failed flat-schema validation must not poison later decodes");
    }
    #[cfg(feature = "json")]
    #[test]
    fn json_schema_decode_enforces_recursive_and_structural_limits() {
        let at_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH - 1);
        let json = norito::json::to_string(&at_limit).expect("encode schema JSON at limit");
        let decoded = norito::json::from_str::<EntrypointValueTypeV1>(&json)
            .expect("decode schema JSON at limit");
        assert_eq!(decoded, at_limit);
        assert_eq!(
            norito::json::from_json_auto::<EntrypointValueTypeV1>(&json)
                .expect("fast decode schema JSON at limit"),
            at_limit
        );
        let over_limit = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_DEPTH);
        let json = norito::json::to_string(&over_limit).expect("encode over-limit schema JSON");
        assert!(matches!(
            norito::json::from_str::<EntrypointValueTypeV1>(&json),
            Err(norito::json::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
        assert!(norito::json::from_json_auto::<EntrypointValueTypeV1>(&json).is_err());
        let deep = nested_list_schema(MAX_ENTRYPOINT_ARGUMENT_TYPE_NODES + 64);
        let deep_json = norito::json::to_string(&deep).expect("encode adversarial flat JSON");
        assert!(matches!(
            norito::json::from_json::<EntrypointValueTypeV1>(&deep_json),
            Err(norito::json::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
        assert!(norito::json::from_json_auto::<EntrypointValueTypeV1>(&deep_json).is_err());
        assert!(matches!(
            norito::json::from_str::<EntrypointValueTypeV1>(&deep_json),
            Err(norito::json::Error::Message(message))
                if message == "invalid V1 entrypoint value schema"
        ));
        let value = norito::json::to_value(&deep).expect("convert adversarial flat schema");
        assert!(norito::json::from_value::<EntrypointValueTypeV1>(value).is_err());
        let leaf_value = norito::json::to_value(&leaf(EntrypointValueKindV1::Bool))
            .expect("convert recovery schema to JSON value");
        norito::json::from_value::<EntrypointValueTypeV1>(leaf_value)
            .expect("failed from_value validation must not poison later decodes");
    }
    #[cfg(feature = "json")]
    #[test]
    fn manual_fast_json_schema_decode_enforces_complete_document_depth() {
        let wrappers = norito::json::MAX_JSON_VALUE_NESTING_DEPTH - 1;
        let nested = format!("{}null{}", "[".repeat(wrappers), "]".repeat(wrappers));
        let input = format!(r#"{{"nodes":[],"unknown":{nested}}}"#);
        let mut walker = norito::json::TapeWalker::new(&input);
        let mut arena = norito::json::Arena::new();
        assert!(matches!(
            <EntrypointValueTypeV1 as norito::json::FastFromJson>::parse(
                &mut walker,
                &mut arena
            ),
            Err(norito::Error::Json(
                norito::json::Error::NestingDepthExceeded {
                    depth,
                    limit: norito::json::MAX_JSON_VALUE_NESTING_DEPTH,
                    context: "JSON value",
                }
            )) if depth == norito::json::MAX_JSON_VALUE_NESTING_DEPTH + 1
        ));
    }
    #[test]
    fn every_reserved_query_projection_has_one_exact_flat_shape() {
        use EntrypointValueKindV1 as Kind;
        use EntrypointValueTypeNodeV1 as Node;
        let views = vec![
            account_view_schema(),
            EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: "AssetView".into(),
                        fields: vec!["id".into(), "amount".into()],
                    }),
                    Node::Leaf(Kind::AssetId),
                    Node::Leaf(Kind::Quantity),
                ],
            },
            EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: "AssetDefinitionView".into(),
                        fields: vec![
                            "id".into(),
                            "name".into(),
                            "description".into(),
                            "owned_by".into(),
                            "total_quantity".into(),
                            "metadata".into(),
                        ],
                    }),
                    Node::Leaf(Kind::AssetDefinitionId),
                    Node::Leaf(Kind::String),
                    Node::Option,
                    Node::Leaf(Kind::String),
                    Node::Leaf(Kind::AccountId),
                    Node::Leaf(Kind::Quantity),
                    Node::Leaf(Kind::Json),
                ],
            },
            EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: "DomainView".into(),
                        fields: vec!["id".into(), "owned_by".into(), "metadata".into()],
                    }),
                    Node::Leaf(Kind::DomainId),
                    Node::Leaf(Kind::AccountId),
                    Node::Leaf(Kind::Json),
                ],
            },
            EntrypointValueTypeV1 {
                nodes: vec![
                    Node::Struct(EntrypointStructTypeNodeV1 {
                        name: "NftView".into(),
                        fields: vec!["id".into(), "owned_by".into(), "content".into()],
                    }),
                    Node::Leaf(Kind::NftId),
                    Node::Leaf(Kind::AccountId),
                    Node::Leaf(Kind::Json),
                ],
            },
        ];
        for view in views {
            let Node::Struct(root) = &view.nodes[0] else {
                unreachable!("projection fixture starts with a struct")
            };
            let name = root.name.clone();
            assert!(view.validate(), "valid reserved projection {name}");
            assert_eq!(view.canonical_type_name().as_deref(), Some(name.as_str()));
            let page = query_page_schema(&view);
            assert!(page.validate(), "valid reserved page {name}");
            let expected_page_name = format!("QueryPage<{name}>");
            assert_eq!(
                page.canonical_type_name().as_deref(),
                Some(expected_page_name.as_str())
            );
            let mut wrong_leaf = view.clone();
            let last = wrong_leaf.nodes.len() - 1;
            wrong_leaf.nodes[last] = Node::Leaf(Kind::Blob);
            assert!(!wrong_leaf.validate(), "forged reserved projection {name}");
            let mut wrong_page = page;
            let last = wrong_page.nodes.len() - 1;
            wrong_page.nodes[last] = Node::Leaf(Kind::String);
            assert!(!wrong_page.validate(), "forged next_offset for {name}");
        }
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn query_page_type_name_is_derived_from_its_structural_specialization() {
        let account_view = account_view_schema();
        assert!(account_view.validate());
        assert_eq!(
            account_view.canonical_type_name().as_deref(),
            Some("AccountView")
        );
        let account_option = EntrypointValueTypeV1 {
            nodes: std::iter::once(EntrypointValueTypeNodeV1::Option)
                .chain(account_view.nodes.iter().cloned())
                .collect(),
        };
        assert!(account_option.validate());
        assert_eq!(
            account_option.canonical_type_name().as_deref(),
            Some("Option<AccountView>")
        );
        let page = query_page_schema(&account_view);
        assert!(page.validate());
        assert_eq!(
            page.canonical_type_name().as_deref(),
            Some("QueryPage<AccountView>")
        );
        let encoded = norito::to_bytes(&page).expect("encode structural query-page schema");
        let decoded: EntrypointValueTypeV1 =
            norito::decode_from_bytes(&encoded).expect("decode structural query-page schema");
        assert_eq!(decoded, page);
        assert_eq!(
            decoded.canonical_type_name().as_deref(),
            Some("QueryPage<AccountView>")
        );
        let assert_reserved_rejected = |label: &str, schema: &EntrypointValueTypeV1| {
            assert!(!schema.validate(), "{label} must fail schema validation");
            assert_eq!(schema.word_count(), None, "{label} must have no ABI width");
            assert_eq!(
                schema.canonical_type_name(),
                None,
                "{label} must have no admissible public type name"
            );
        };
        let mut wrong_capacity = page.clone();
        let EntrypointValueTypeNodeV1::List(items) = &mut wrong_capacity.nodes[1] else {
            unreachable!("query-page fixture has an items list")
        };
        items.capacity = 32;
        assert_reserved_rejected("wrong QueryPage list capacity", &wrong_capacity);
        let mut unknown_view = page.clone();
        let EntrypointValueTypeNodeV1::Struct(view) = &mut unknown_view.nodes[2] else {
            unreachable!("query-page fixture has a projected struct")
        };
        view.name = "UnknownView".into();
        assert_reserved_rejected("unknown QueryPage projection", &unknown_view);
        let mut wrong_fields = page.clone();
        let EntrypointValueTypeNodeV1::Struct(view) = &mut wrong_fields.nodes[2] else {
            unreachable!("query-page fixture has a projected struct")
        };
        view.fields[1] = "content".into();
        assert_reserved_rejected("reserved view with wrong fields", &wrong_fields);
        let mut wrong_kind = page.clone();
        wrong_kind.nodes[3] = EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::DomainId);
        assert_reserved_rejected("reserved view with wrong leaf kind", &wrong_kind);
        let mut wrong_next_offset = page.clone();
        wrong_next_offset.nodes[6] = EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::String);
        assert_reserved_rejected("QueryPage with non-i64 next_offset", &wrong_next_offset);
        let mut ordinary_struct = unknown_view;
        let EntrypointValueTypeNodeV1::Struct(page_node) = &mut ordinary_struct.nodes[0] else {
            unreachable!("query-page fixture starts with a struct")
        };
        page_node.name = "Page".into();
        assert!(ordinary_struct.validate());
        assert_eq!(
            ordinary_struct.canonical_type_name().as_deref(),
            Some("struct Page"),
            "non-reserved user structs retain ordinary nominal rendering"
        );
    }
    #[test]
    fn argument_and_return_words_share_the_v1_register_window() {
        assert_eq!(MAX_ENTRYPOINT_RETURN_WORDS, 13);
        assert_eq!(MAX_ENTRYPOINT_ARGUMENT_WORDS, MAX_ENTRYPOINT_RETURN_WORDS);
        assert_eq!(ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1, 39);
        assert_eq!(
            MAX_ENTRYPOINT_RETURN_RECORD_BYTES + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1,
            MAX_ENTRYPOINT_BOUNDARY_BYTES
        );
        assert_eq!(leaf(EntrypointValueKindV1::Bool).word_count(), Some(1));
    }
    #[test]
    fn canonical_boundary_identifiers_reject_keywords_and_forbidden_spellings() {
        assert_eq!(KOTODAMA_V1_FORBIDDEN_SOURCE_IDENTIFIERS, &["Amount"]);
        assert!(!is_canonical_kotodama_identifier("Amount"));
        assert!(is_canonical_kotodama_identifier("amount"));
        for keyword in [
            "authorize",
            "break",
            "const",
            "continue",
            "decimal",
            "else",
            "enum",
            "error",
            "false",
            "fn",
            "for",
            "hajimari",
            "始まり",
            "if",
            "in",
            "int",
            "kaizen",
            "改善",
            "kotoage",
            "言挙げ",
            "let",
            "match",
            "module",
            "quantity",
            "return",
            "seiyaku",
            "誓約",
            "state",
            "struct",
            "trigger",
            "true",
            "var",
            "view",
        ] {
            assert!(
                !is_canonical_kotodama_identifier(keyword),
                "keyword `{keyword}` must not be a boundary identifier"
            );
        }
        for identifier in ["contract", "entry", "init", "upgrade", "_value2"] {
            assert!(
                is_canonical_kotodama_identifier(identifier),
                "retired declaration word `{identifier}` remains an ordinary identifier"
            );
        }
    }
    #[test]
    fn schemas_reject_empty_products_and_non_tuple_arities() {
        let empty_struct = EntrypointValueTypeV1 {
            nodes: vec![EntrypointValueTypeNodeV1::Struct(
                EntrypointStructTypeNodeV1 {
                    name: "Empty".to_owned(),
                    fields: Vec::new(),
                },
            )],
        };
        assert!(!empty_struct.validate());
        for arity in [0, 1] {
            let mut nodes = vec![EntrypointValueTypeNodeV1::Tuple(arity)];
            nodes.extend(
                (0..arity).map(|_| EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int)),
            );
            assert!(!EntrypointValueTypeV1 { nodes }.validate());
        }
    }
    #[test]
    fn schemas_reject_noncanonical_and_duplicate_identifiers() {
        for invalid in [
            "",
            "two words",
            "言挙げ",
            "with-dash",
            "9starts_with_digit",
            "seiyaku",
            "Amount",
        ] {
            let argument = EntrypointArgumentSchemaV1 {
                fields: vec![EntrypointArgumentFieldV1 {
                    name: invalid.to_owned(),
                    ty: leaf(EntrypointValueKindV1::Int),
                }],
            };
            assert!(!argument.validate(), "argument field `{invalid}` must fail");
            let structure = EntrypointValueTypeV1 {
                nodes: vec![
                    EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                        name: "Valid".to_owned(),
                        fields: vec![invalid.to_owned()],
                    }),
                    EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                ],
            };
            assert!(!structure.validate(), "struct field `{invalid}` must fail");
        }
        let invalid_struct_name = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Bad.Name".to_owned(),
                    fields: vec!["field".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
            ],
        };
        assert!(!invalid_struct_name.validate());
        let retired_struct_name = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Amount".to_owned(),
                    fields: vec!["amount".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Quantity),
            ],
        };
        assert!(!retired_struct_name.validate());
        let duplicate_fields = EntrypointValueTypeV1 {
            nodes: vec![
                EntrypointValueTypeNodeV1::Struct(EntrypointStructTypeNodeV1 {
                    name: "Valid".to_owned(),
                    fields: vec!["same".to_owned(), "same".to_owned()],
                }),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Int),
                EntrypointValueTypeNodeV1::Leaf(EntrypointValueKindV1::Bool),
            ],
        };
        assert!(!duplicate_fields.validate());
    }
}
